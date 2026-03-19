+++
title = "Forge: A Git-Native Development Platform"
subtitle = "Design Specification"
version = "0.3.0"
date = 2026-03-18
status = "Draft"
+++

# Forge: A Git-Native Development Platform

## Foundation

Forge is a development platform built entirely on Git primitives.
Issues, code review, comments, approvals, access control, releases, and enforcement are stored as Git objects — refs, trees, blobs, and commits.
There is no database.
The Git repository is the database.

The design has two principles.
First, every piece of state is a Git object, reachable from a ref, signed where authorship matters.
Second, the data model separates entities from annotations.
Entities (issues, reviews) are standalone refs with their own lifecycles.
Annotations (comments, approvals) are metadata on objects — attached to the content they describe, not to the event that prompted them.
Relationships between entities are standalone metadata, not embedded fields.

Forge depends on [`git-metadata`](https://github.com/git-ents/git-metadata) for annotations and relational metadata. `git-metadata` extends Git's notes feature to tree-structured data, allowing arbitrary metadata to be attached to any object (blob, tree, or commit) without modifying history.


## Contributors

Contributors are the foundation of the identity model.
A contributor is a directory in a ref, keyed by a short human-chosen identifier:

```text
refs/forge/contributors → commit → tree
├── alice/
│   ├── name            # plain text: display name
│   └── emails          # plain text: one address per line
├── bob/
│   ├── name
│   └── emails
```

The directory name (`alice`, `bob`) is the contributor ID.
Every reference to a person anywhere in the system — issue author, assignee, comment attribution, approval signer, ACL entry — uses this ID string.

A contributor may have multiple email addresses (work, personal, etc.).
The `emails` blob lists one address per line.
Identity resolution matches any of them.

Adding a contributor is a signed commit to this ref.
The commit history is the audit trail: who added whom, when, signed by whom.

### Bootstrapping

The first contributor is seeded automatically on `git forge install` or on first entity creation.
The tool reads `user.name` and `user.email` from git config, prompts for an ID (or derives one from the name), and writes the first contributor entry.
The first commit is self-signed by the project creator.
This bootstraps trust.

### Identity Resolution

The CLI resolves the current user by matching `user.email` from git config against contributor `emails` entries.
On write operations (create issue, add comment), the tool looks up the current user's contributor ID automatically.
On read operations (display issue, show comment), the tool resolves contributor IDs to display names from `name`.

The contributor ID is the stable reference stored everywhere.
Display names are cosmetic, changeable, and resolved at render time.
`--assignee alice` works because `alice` is the contributor ID directly.

### Key Management (Future)

When signing matters, a contributor gains a `keys/` subtree:

```text
├── alice/
│   ├── name
│   ├── emails
│   └── keys/
│       ├── <fingerprint-1>.pub
│       └── <fingerprint-2>.pub
```

Key rotation is adding a new key and optionally removing the old one.
The ID `alice` does not change.
Nothing referencing `alice` anywhere in the system changes.
Signature verification maps fingerprint → contributor ID by scanning keys.

### Roles (Future)

When access control matters, a contributor gains a `roles` blob:

```text
├── alice/
│   ├── name
│   ├── emails
│   ├── keys/
│   └── roles           # plain text: one role per line
```

Roles map to permissions in `policy.toml`.
Same ID, same ref, additive change.

### External Identity (Future)

Long-term, a contributor ID could be a DID (`did:plc:...` or `did:web:alice.dev`).
The `keys/` subtree becomes optional — key discovery goes through DID resolution with local caching.
Everything else is unchanged.
The contributor data model accommodates this without migration.

Near-term, the key in the contributors ref is the identity.
No registration, no accounts.
A key pair is sufficient.


## Entity IDs

Entities (issues, reviews) use sequential integer IDs.
IDs are assigned by scanning existing refs — no counter ref is needed.

The next ID is `max(existing IDs) + 1`, determined by globbing `refs/forge/issue/*` (or `refs/forge/review/*`).
Scanning packed-refs or loose ref entries is fast — a project with 10,000 issues scans in microseconds.

The first commit on an entity ref is the creation event.
Author, timestamp, and signature are in the commit itself.
No separate audit log is needed.

### ID Assignment and Sync

ID assignment depends on whether forge refs are fresh locally:

**Refs are fresh (daemon running or recent sync).**
The CLI scans local refs, takes N+1, commits the entity ref, and returns immediately.
The daemon pushes the ref.
Conflicts are near-impossible because the local state is current and entity creation is infrequent.

If the daemon's push fails (someone else took N+1), the daemon re-scans, renames the local ref to N+2, pushes again, and notifies the user: `Issue renumbered: #43 → #44`.
This is rare.

Freshness is determined by the reflog timestamp on forge refs.
If the most recent fetch is within a configurable threshold (default: a few minutes for small teams, tighter for large projects), refs are considered fresh.

**Refs are stale (no daemon, offline).**
The CLI writes the full entity tree to a staging ref:

```text
refs/forge/staging/<random-uuid> → commit → tree
```

The user sees `Created issue (pending — number assigned on sync)`.
When connectivity returns, the daemon or `git forge sync` allocates the real number and notifies: `Staged issue assigned #43`.

**Server hooks (smart server).**
The client pushes to an inbox ref.
The server's post-receive hook reads the commit, assigns the next integer, creates the canonical ref, and deletes the inbox ref:

1. User pushes to `refs/inbox/<contributor-id>/issues/<anything>`.
2. Server post-receive hook reads it, assigns the next integer, creates `refs/forge/issue/<N+1>`, deletes the inbox ref.
3. Returns the assigned ID to the user (post-receive hook output goes back to the pusher over the transport).

**UI server (direct repo access).**
The UI has filesystem access.
Lock, scan, increment, write the entity ref, unlock.
No CAS retry loop, no hooks.

External contributors always use the inbox path — they cannot write to `refs/forge/` directly.


## Annotations

### Comments

A comment is a Git commit object stored under `refs/forge/comments/`.
Comments anchor to any Git object — blobs, commits, trees, or commit ranges — and support threading through the DAG.
See `docs/design/git-forge-comments.md` for the full data model, ref structure, threading, payloads, and plumbing examples.

```text
refs/forge/comments/issue/<id>       # comments on issues
refs/forge/comments/review/<id>      # comments on code reviews
refs/forge/comments/object           # comments on raw git objects (commits, blobs, trees)
```

Each ref points to the tip of a chronological commit chain.
Anchor information is carried as trailers in the commit message (`Anchor`, `Anchor-Type`, `Anchor-Range`, `Anchor-End`, `Resolved`, `Replaces`).
Threading uses second parents: a reply's second parent points at the comment it replies to.
Comments are immutable — edits and resolutions are new commits appended to the chain.

Relationships to other entities (issues, reviews, commits) are stored as relational metadata, not embedded in the comment.
See the Relational Metadata section.


### Approvals

Forge supports four levels of approval, each attesting to a different thing:

| Level | Object | Meaning |
|-------|--------|---------|
| File | blob OID | "This file is correct" |
| Tree | tree OID | "This subtree is correct" |
| Commit | patch-id | "This change is correct" |
| Range | range patch-id | "This overall change is correct" |

#### Change Approvals (patch-id and range patch-id)

A change approval is an annotation on a patch-id.
Using patch-id rather than commit oid means approvals survive rebases automatically — the same change before and after rebase produces the same patch-id.

```text
refs/metadata/approvals   (fanout by patch-id)
  <patch-id>/
    <contributor-id>     # toml: timestamp, type ("patch"|"range"), message (optional)
```

The metadata commit adding the entry is signed by the approver.
The approval is verifiable from the commit signature.

`git patch-id` is a Git built-in.
It hashes the diff content, ignoring line numbers and whitespace.
Two commits that represent the same change produce the same patch-id regardless of where they appear in history.

Range patch-id is computed over the full diff of a commit range: `git diff base..tip | git patch-id`.
This produces a single hash for the overall change, regardless of how many commits compose it.
Squashing the range produces the same range patch-id because the overall diff is identical.

Behavioral properties:

- **Rebase with no conflict:** same patches, same patch-ids, approvals carry over automatically.
  Same range diff, same range patch-id, range approval carries over.
- **Rebase with conflict resolution:** affected patches produce new patch-ids.
  Overall diff changes, new range patch-id.
  Both need re-approval.
- **Squash:** new combined diff per commit, new per-commit patch-id.
  But range patch-id unchanged — overall diff is the same.
- **Amend commit message only:** patch-id unchanged, approval survives.
- **Reorder commits without changing overall diff:** individual patch-ids may change, range patch-id survives.

When a reviewer approves a review (the coordination entity), the tooling bulk-approves every patch-id in the review's commit range and writes a range patch-id approval.
One storage model.
The review is the UX; the approvals are the data.
Policy decides which level to enforce.

#### State Approvals (blob OID and tree OID)

A state approval attests to the correctness of a file or subtree at a specific point in time.
It is an annotation on the blob or tree OID.

```text
refs/metadata/approvals   (fanout by oid)
  <blob-or-tree-oid>/
    <contributor-id>     # toml: timestamp, type ("blob"|"tree"), path, message (optional)
```

The `path` field records which file or directory was approved (a blob OID alone doesn't indicate location).
State approvals break on any change to the approved object — the OID changes, so a new approval is required.

Tree approvals are the monorepo primitive.
A subtree OID changes when anything under it changes, so it automatically invalidates.
Team A approves `services/auth/` as a tree OID; any change under that path produces a new tree OID requiring re-approval.

#### Approval Policy

The merge gate checks whichever approval level policy requires:

```toml
[branches.main]
merge_strategy = "squash"
approval_check = "range"              # check range patch-id
min_approvals = 1
exclude_author = true                 # author cannot self-approve
block_unresolved_comments = true

[branches.main.state_approval]
paths = ["crypto/*", "services/auth/*"]
type = "tree"                         # require tree approval for these paths
approvers = ["@crypto-team", "@auth-team"]

[branches.develop]
merge_strategy = "rebase"
approval_check = "per_patch"          # check individual patch-ids
min_approvals = 1
```

In permissive mode (no approval enforcement), approvals are recorded but not enforced.
The data is always granular.
A team switching from permissive to strict doesn't change their workflow — the approvals were already being recorded.

When a reviewer approves a review, the tooling bulk-writes approval entries for every patch-id in the review's commit range plus a range approval.
Individual patch-level or state-level approval is also possible for surgical sign-off.

Teams can approve asynchronously.
Auth team approves their subtree, billing team approves theirs, merge proceeds when all required approvals are satisfied.


## Relational Metadata

Relationships between entities are stored as `git-metadata` trees, not embedded in entity fields.
Both directions are first-class — no scanning, no derived indexes.

```text
refs/metadata/links/<entity-type>/<entity-id>/
  <related-type>:<related-id>    # blob: optional metadata about the relation
```

Both directions are stored:

```text
refs/metadata/links/issues/42/
  comment:abc123          # "comment abc123 references issue 42"
  review:7                # "review 7 references issue 42"

refs/metadata/links/comments/abc123/
  issue:42                # reverse direction
```

Each link is a Git tree entry.
Adding a link writes both directions in a single commit on the metadata ref — one atomic operation.

Relationships are independently authored and signed.
"User X linked comment C to issue 42" is a distinct, attributable action — not a side effect of editing a comment.

Reverse lookups are tree reads, not index scans.
"All comments referencing issue 42" is a tree listing of `refs/metadata/links/issues/42/`.


## Authorship

Git commits always carry an author (name, email, timestamp).
Forge does not duplicate this in entity trees.

The creator of an issue is the author of the first commit on `refs/forge/issue/<id>`.
The creator of a review is the author of the first commit on `refs/forge/review/<id>`.
The author of a comment is the author of the comment's commit.
The author of an approval is identified by the metadata tree path (keyed by contributor ID) and the commit signature.

Every read path that displays "who did this" resolves the commit's author email to a contributor ID via the contributors ref, then to a display name.
Every write path uses `repo.signature()` from git config.
No `author` blob is stored in any entity tree.

This means a reliable mapping from git commit author email to contributor ID is essential.
The `find_contributor_by_email` lookup in the contributors ref provides this.


## Data Model: Trees vs Blobs

The choice between tree entries and TOML blobs depends on who writes the data and whether they write concurrently.

**Use tree entries when different people write different entries.**
Approvals keyed by contributor ID, comments keyed by OID, links keyed by entity ID.
These are the cases where concurrency is common and conflicts would be spurious.
The tree structure makes independent writes auto-merge via three-way tree merge.
This is where `git-metadata`'s tree approach is correct.

**Use separate blobs within an entity tree when concurrent edits to different fields should auto-merge.**
Issue fields (title, state, body) are separate blobs in the issue tree.
Two people editing the same issue — one changing the title, the other closing it — should merge cleanly.
The daemon's auto-merge handles this because the edits touch disjoint tree paths.

**Use a single TOML blob when one person writes all fields together.**
Contributor meta (name, email) — one person sets this.
Review meta (target_branch, state) — set at creation, rarely edited.
These are coherent units written by a single actor.
Splitting them gains nothing.

The merge unit is the blob.
Put the merge boundary where the authorship boundary is.


## Entities

### Issues

An issue is a standalone ref with its own lifecycle.
It is not metadata on any object — it is an entity.

```text
refs/forge/issue/<issue-id> → commit → tree
├── title           # plain text: single-line title
├── state           # plain text: "open" or "closed"
├── body            # markdown
├── labels/         # dir: empty blobs whose names are the labels
└── assignees/      # dir: empty blobs whose names are contributor IDs
```

Each field is a separate blob to enable auto-merge when different people edit different fields concurrently (see Data Model section).

Each mutation — state change, title edit, label update, assignment — is a new commit on the issue's ref.
The commit history is the issue's audit log.
`git log refs/forge/issue/<id>` shows every change, who made it, and when.

Issue comments are conversation within the issue.
They are not the same as code comments.
Code comments are annotations on blobs, visible everywhere that content appears, blame-reanchored.
Issue comments are stored under `refs/forge/comments/issue/<id>`, scoped to the issue.

Relationships to other entities (commits, blob+line ranges, other issues, reviews) are stored as relational metadata, not embedded in the issue.

Ref-per-issue eliminates write contention.
Two people editing different issues never conflict.

#### Labels, Assignment, State

Labels are empty blobs in a `labels/` subtree.
No separate taxonomy system.
A label exists when someone uses it.

Assignment is empty blobs in an `assignees/` subtree, named by contributor ID.

State is a plain text blob: `open` or `closed`.
No intermediate states in the core model.
Extensions or conventions can add them via labels.


### Reviews

A review is a coordination entity — "please look at commits X..Y."
It references commits but is not metadata on any commit.
It has its own lifecycle independent of the commits it covers.

```text
refs/forge/review/<review-id> → commit → tree
├── meta            # toml: target_branch, state, created
├── description     # markdown
├── revisions/
│   ├── 001         # toml: head_commit, timestamp
│   ├── 002         # toml: head_commit, timestamp
│   └── ...
```

The `revisions/` entries record each time the author pushed new commits.
This provides "review rounds" — a reviewer can see what changed between revision 1 and revision 2.

State is `open`, `merged`, or `closed`.

A review does not contain comments or approvals.
It prompts them.
Comments land under `refs/forge/comments/review/<id>`.
Approvals land on patch-ids, range patch-ids, blob oids, or tree oids (via `git-metadata`).
The review is how you discover which commits to look at.
The comments and approvals are what you find when you look.

This means comments and approvals outlive the review that prompted them.
A comment on line 42 of `lib.rs` persists and follows that code regardless of which review prompted it.
Closing or merging a review does not resolve its comments.


## Checks

### Check Definitions

Check definitions live in the repository, versioned with the code:

```text
.forge/checks/
├── build.toml
├── lint.toml
└── test.toml
```

```toml
# build.toml
name = "build"
image = "rust:1.85"
run = "cargo build --release"
triggers = ["refs/heads/*", "refs/forge/queue/*"]
secrets = [
  { name = "CARGO_REGISTRY_TOKEN", type = "file" },
]
```

The check definition that runs is the one at the commit being checked — no external configuration that drifts from the code.

### Check Results

Check results are metadata on commits, keyed by run ID to support multiple runs and matrix builds:

```text
refs/metadata/checks/<commit-oid>/
  <run-id>/
    meta          # toml: name, state (pass|fail|running), started, finished,
                  #       runner_contributor_id, params (optional)
    log           # blob: raw output
```

`run-id` is a timestamp + contributor ID or a short random ID.
Every execution is a distinct tree entry.
Never overwrites.

Matrix builds use the `params` field to distinguish variants:

```toml
name = "build"
state = "pass"
params = { os = "linux", arch = "amd64" }
```

Reruns are new entries.
The old run's result is permanent history.
The merge gate queries all runs for a required check name and uses the most recent result.

### Runners

A runner is a contributor with a runner role.
It signs its results.
The check result commit on the metadata ref is signed by the runner's key.

```toml
[roles.runner]
push = ["refs/metadata/checks/*"]
approve = false
```

Runners poll a queue (the queue primitive already exists) or get notified by the server's post-receive hook.

### Merge Gate Integration

Policy declares required checks per branch:

```toml
[branches.main]
require_checks = ["build", "test"]
```

The pre-receive hook reads `refs/metadata/checks/<oid>` for the commits being pushed and verifies the required checks passed.

Check results do not survive rebase.
A rebased commit is a new OID, so it needs to be rechecked.
This is correct — the rebase could introduce failures.

### Local Execution

`git forge check run build` executes locally using the same definition.
Same inputs, same container image, reproducible.
The only difference is who signs the result.
Policy can require runner-signed results for merge but allow local runs for feedback.

### Check Policy Querying

The checks required to push to any branch are queryable from the repo itself:

```sh
git show main:.forge/policy.toml    # required checks for main
git show main:.forge/checks/build.toml  # what "build" does
```

No server query, no API.
Policy and check definitions are versioned with the code.


## Secrets

Secrets cannot be Git objects.
Git repos get cloned, forked, mirrored.
A secret in a ref is a secret on every machine that fetches.

### Design

Check definitions reference secrets by name.
Names are in Git; values are not.

```toml
# .forge/checks/deploy.toml
secrets = [
  { name = "AWS_ACCESS_KEY", type = "file" },
  { name = "DEPLOY_TOKEN", type = "file" },
]
```

The secret store is per-server, not per-repo.
Entries are encrypted at rest with a key derived from the server's own identity:

```text
/var/forge/secrets/<repo>/
  AWS_ACCESS_KEY          # encrypted blob
  DEPLOY_TOKEN            # encrypted blob
  meta                    # toml: who set it, when, ACL
```

The ACL specifies which runner contributor IDs can read each secret.

### Injection

The server injects secrets, not the runner.
This prevents a compromised runner from requesting arbitrary secrets.

1. Runner picks up a job from the queue and authenticates to the server with its key.
2. Server reads the check definition at that commit's tree itself, verifies the runner's key corresponds to a contributor in the ACL for each listed secret.
3. Server writes secrets to a tmpfs volume mounted into the container at `/run/forge/secrets/<n>`.
4. The mount is read-only.
   The container has no capability to remount.
   Network egress is restricted to what the check definition declares.
5. Runner spawns the container, executes, signs the result, pushes result metadata.
   Secrets are destroyed when the container exits.

tmpfs is memory-backed, never hits disk.
No environment variable exposure, no `/proc/<pid>/environ` leakage, no child process inheritance, no accidental logging.

The trust boundary is the check definition in the repo.
A PR that adds `secrets = [{ name = "PROD_DB_PASSWORD", type = "file" }]` to a check is visible and reviewable.
If someone merges a check that exfiltrates secrets, no runtime mechanism saves you.
The tmpfs mount eliminates accidental leaks, which are the common case.

### Management

```sh
git forge secret set AWS_ACCESS_KEY --value=...
git forge secret set DEPLOY_TOKEN --file=token.txt
git forge secret list
git forge secret grant AWS_ACCESS_KEY --runner=<contributor-id>
```

These are API calls to the server, not ref writes.
This is an honest exception to the "everything is a ref" principle.

### Audit Trail

Every secret read/write/grant event is logged to a server-maintained ref:

```text
refs/forge/meta/audit/secrets → commit → tree
├── 001-<ts>    # toml: action, secret_name, actor_contributor_id
├── 002-<ts>
```

Secret values are opaque.
The history of who touched what is in Git and signed.


## Access Control

### Roles and Policy

Roles are names.
Policy maps roles to permissions:

```toml
# policy.toml

[roles.admin]
push = ["refs/heads/*"]
approve = true
manage_issues = true
modify_contributors = true
modify_policy = true

[roles.maintainer]
push = ["refs/heads/*"]
approve = true
manage_issues = true

[roles.contributor]
push = ["refs/heads/feat/*"]
approve = false
manage_issues = true

[roles.reviewer]
push = []
approve = true
manage_issues = false

[roles.runner]
push = ["refs/metadata/checks/*"]
approve = false
```

Path-scoped permissions restrict which files a role can modify:

```toml
[roles.frontend]
push = ["refs/heads/*"]
paths = ["web/*", "css/*"]
```

Enforced in the pre-receive hook via `git diff --name-only <old> <new>`.

### Self-Protecting Policy

`policy.toml` is committed to the repository.
Changing it requires meeting the rules currently in effect.
You cannot weaken policy without satisfying the current policy's review and approval requirements.

```toml
[access.contributors]
modify = { require_role = "admin" }

[access.policy]
modify = { require_role = "admin", require_review = true }
```

### External Contributors

External contributors have no entry in `refs/forge/contributors`.
They cannot push to the repo directly.

The pre-receive hook allows anyone with a valid signature to push to a scoped inbox namespace:

```text
refs/inbox/<contributor-id-or-fingerprint>/<branch-name>
```

The hook verifies the push is signed and that the target ref is under the pusher's own namespace.
No role needed.
They can only write to their own namespace.

A maintainer reviews the code at that ref and decides whether to merge.
The inbox namespace is garbage-collected after resolution.

```toml
[inbox]
allow = "any"          # any valid signature
# allow = "known"      # only known contributors
# allow = "none"       # no external submissions
```

### Ref Visibility

Git's transport protocol advertises all refs on fetch.
Hiding refs from unauthorized users requires a server-side filter.

Git natively supports HTTPS via the smart HTTP protocol.
The ref advertisement is the response to `GET /info/refs?service=git-upload-pack`.
An HTTP middleware in front of `git-http-backend`:

1. Authenticates the user.
2. Looks up their role.
3. Filters the ref advertisement based on permissions.
4. Proxies everything else unchanged.

Public refs (issues, comments) are visible to everyone.
Private refs (policy details, contributor roles) are visible only to members with the appropriate role.

For pure Git over SSH without a custom server, all refs are visible to anyone with read access.
This is an honest limitation of the protocol.


## Enforcement

### Merge Gate

The pre-receive hook enforces policy on pushes to protected branches:

1. Verify push is signed by a known contributor.
2. Check the contributor's role permits pushing to this ref.
3. `git diff --name-only <old> <new>` — check path-scoped permissions if configured.
4. `git log <old>..<new>` — list commits in the range.
5. Check approvals per policy:
   - If `approval_check = "per_patch"`: `git patch-id` each commit, check `refs/metadata/approvals/<patch-id>`.
   - If `approval_check = "range"`: compute `git diff <old>..<new> | git patch-id`, check range approval.
   - If `state_approval` configured: extract blob/tree OIDs for specified paths from the merge commit, check for matching approvals.
6. Check `refs/metadata/checks/<oid>` for required checks on each commit.
7. Optionally check for unresolved comments on affected blob oids.
8. Accept or reject with a specific failure message listing what's missing.

### Merge Queue

The merge queue is a ref containing an ordered list of entries:

```text
refs/forge/queue/merge → commit → tree
├── 001-<review-id>     # toml: head_commit, submitted_by, timestamp
├── 002-<review-id>
└── ...
```

The server processes entries in order:

1. Take the first entry.
2. Rebase onto current target branch HEAD.
3. Trigger a build on the rebased commit.
4. On success, push to the target branch.
5. On failure, reject and notify.
   Advance to next entry.

Rebase before build is essential — a branch that passed checks against an older HEAD may fail against current HEAD.

Batching: the processor can rebase multiple entries as a stack, test the combined result, and push all on success.
On failure, bisect the batch to find the breaker.

Write serialization uses the pre-receive hook, which runs atomically one push at a time.

### Queue as a Primitive

The merge queue is an instance of a general queue:

```sh
git forge queue create <n>
git forge queue push <n> <ref>
git forge queue pop <n>
git forge queue list <n>
```

Processing hooks declare what happens when entries appear.
The merge queue's hook rebases and tests.
A CI queue's hook executes build actions.
A release pipeline chains queues.


## Sync and Transport

### Architecture

The daemon is the primary transport layer.
It is not optional infrastructure — it is the expected way forge refs move between local and remote.
The CLI and LSP are local tools that read and write refs.
The daemon makes those changes visible to others.

### Daemon

The daemon is a single long-lived process per repository.
It has two responsibilities:

**Outbound:** watch local forge refs (via inotify or polling on `.git/refs/forge/`), push changes to the remote on a short debounce.
Multiple local writes within a few seconds (e.g. five comments in rapid succession) are batched into one push.

**Inbound:** fetch remote forge refs on a periodic interval or via server-sent events.
Changes from collaborators appear locally without user action.

The daemon is trivial to start.
The editor plugin starts it.
`git forge status` starts it if it's not running.
It shuts down on idle.
It is the same process as the LSP server (see Editor Integration).

### Push Modes

Three modes, all producing the same commits.
The only variable is transport timing.

**Daemon running (default).**
The CLI commits locally and returns immediately.
The daemon notices the ref change and pushes within seconds.
The user never waits for network.

**No daemon, network available (fallback).**
`git forge sync` fetches and pushes all forge refs.
Manual, explicit.
The CLI should suggest starting the daemon: `tip: run git forge daemon for automatic sync`.

**Offline.**
The CLI commits locally.
Refs accumulate.
On reconnect, the daemon (or `git forge sync`) pushes everything.
The server auto-merges concurrent changes (see Metadata Push and Auto-Merge).

### Conflict Resolution

**Entity edits (issues, reviews).**
The daemon attempts a three-way tree merge on the entity ref.
Because entity fields are separate blobs (title, state, body, etc.), edits to different fields by different people merge cleanly.

When the merge fails (both people changed the title), the daemon uses last-write-wins: it keeps the remote version, discards the local edit, and notifies the user with the discarded content.
The user can re-apply their change if needed.
For entity metadata — titles, labels, state — this is appropriate.
The notification preserves intent.

**Metadata (comments, approvals, links).**
These almost never conflict because entries have unique paths (keyed by contributor ID, OID, or timestamp).
The server or daemon auto-merges via three-way tree merge.
See Metadata Push and Auto-Merge.


## Metadata Push and Auto-Merge

### Server Auto-Merge

Metadata refs are almost perfectly auto-mergeable.
Most metadata operations add new entries at unique paths in a tree.
Two people adding different comments, approvals, or links touch disjoint tree paths.

On non-fast-forward push to a metadata ref, the server:

1. Finds the merge base between the incoming commit and the current ref tip.
2. Performs a three-way tree merge.
3. If clean, creates a merge commit, updates the ref.
   Returns success to the pusher.
4. If conflicting (same path modified both sides), rejects with a message listing conflicting paths.

This is `git merge-tree` (or the equivalent plumbing), a Git built-in.

What conflicts in practice: two people resolving the same comment simultaneously (both write to `<comment-id>/resolved`), or two people editing the same entity's `meta` file.
Rare, and the right answer is rejection — the slower writer retries and sees the current state.

What never conflicts: adding comments (unique IDs → unique paths), adding approvals (unique contributor IDs → unique paths), adding links (unique paths per direction), adding replies (sequenced by timestamp in the filename).
That is the vast majority of metadata operations.

Merge commits on metadata refs create non-linear history.
This is fine — the history of a metadata ref is an audit log, not a development narrative.
DAG order with timestamps is sufficient for reconstruction.


## Reviews (Workflow)

### Creating a Review

A developer pushes a branch and runs `git forge review new`.
This:

1. Scans existing review IDs, takes N+1.
2. Creates `refs/forge/review/<review-id>` with metadata pointing at the branch's commit range relative to the target branch.
3. Records revision 001 with the current head commit.

### Updating a Review

The developer pushes new commits or rebases.
They run `git forge review edit` (or the daemon detects the branch update).
A new revision entry is added to the review ref.

### Reviewing

The reviewer opens the review.
The tooling:

1. Reads the review ref to get the commit range.
2. Shows the diffs.
3. Comments land under `refs/forge/comments/review/<id>` (not embedded in the review ref).
4. Approvals land as `git-metadata` on patch-ids, range patch-ids, blob oids, or tree oids (not on the review).

### Merging

The developer submits to the merge queue.
The queue processor verifies policy, rebases, builds, and pushes.
The review state transitions to `merged`.

### Stale Comments

There are no stale comments.
A comment is on the code, not on the review.
If the code changes, the comment is reanchored.
If the code is deleted, the comment is orphaned.
If the comment is no longer relevant, someone resolves it explicitly.


## Releases

### Releases as Workflows

A release is a repository maintenance workflow.
It is not a build step.
Given the commits since the last release, Forge determines the version bump, applies version changes, and generates the changelog.

`git forge release prepare`:

1. Parse conventional commits since the last tag.
2. Determine bump type (major, minor, patch).
3. Run version updaters (language-native tools).
4. Generate changelog.
5. Create a release branch with the changes.
6. Create a review ref for the release.

`git forge release publish`:

1. Tag the merged commit.
2. Attach build artifacts to the release ref.
3. Push.

### Release Refs

```text
refs/tags/v1.2.0            → signed commit
refs/forge/releases/v1.2.0  → tree
├── meta                     # toml: version, date
├── changelog                # markdown
├── artifacts/
│   ├── x86_64-linux/
│   └── aarch64-darwin/
└── signatures/
```

### Automation Modes

Fully manual, semi-automated, or continuous.
In semi-automated mode, every merge to main triggers `git forge release prepare`.
If there are releasable commits since the last tag, a release branch and review appear.
In continuous mode, preparation and publication happen automatically.
The review requirement in policy.toml is the control point.


## Notifications

Notifications have two layers:

**Durable record.**
Post-receive hooks write notification entries to a per-user namespace on new events (comment on your code, assignment, approval request).
These are Git objects — the audit trail and offline fallback.

**Real-time delivery.**
The HTTP server (the same layer that handles auth and ref filtering) pushes events to connected clients.
Missed events are recovered from the durable record.

The daemon subscribes to the server and surfaces notifications in the editor via the LSP.


## Search and Indexing

Scanning all refs for queries ("all open issues," "all unresolved comments on this file") is acceptable at small scale and slow at large scale.

Derived indexes are daemon-maintained or built on demand:

- **Open issues index:** list of issue IDs with state=open.
  Updated on every issue ref mutation.
- **Comments-by-blob index:** the `git-metadata` fanout provides this directly.
- **Links indexes:** the relational metadata trees provide direct lookup in both directions.
- **Approvals-by-review index:** list of patch-ids approved within a review's commit range.
  Derived from the review's revisions and the approvals ref.

Indexes are convenience.
The source of truth is always the refs.
A missing or stale index is rebuilt from refs.
Correctness never depends on the index.


## Porcelain

Forge has three interfaces, each serving a different interaction mode.

### Editor Integration (Primary: Code Review)

Code review is spatial — reading diffs, understanding context, leaving comments anchored to specific lines.
The editor is where the code already is.
Forge's data model (blob-anchored comments, patch-id approvals) maps directly to editor primitives.

The LSP server handles:

- **Comment display.**
  Comments on blob OIDs rendered as inline diagnostics at the relevant line ranges, reanchored via `git blame`.
- **Code actions.**
  Reply, resolve, approve — triggered from a diagnostic.
- **Commands.** `forge.approveReview`, `forge.createIssue`, etc., via `workspace/executeCommand`.
- **Hover.**
  Full comment thread, author, timestamp on hover.
- **Status.**
  CI results, review state in the status bar.

The LSP server is the same process as the daemon.
Both watch forge refs.
Both react to changes.
One process, one file watcher, one ref cache.

What LSP cannot do — custom panels, diff views, tree views, notification inboxes — requires a native editor extension per editor.
The LSP gives every editor inline comments and basic actions with minimal config.
Editor-specific extensions add richer UI.

Build order: LSP server first (works everywhere), VS Code extension second (richest API), Zed extension third.

### TUI (Secondary: Triage and Navigation)

A persistent, keyboard-driven, panel-based interface for scanning issues, processing notifications, and navigating reviews.
Think `lazygit`, not `git log`.

Dashboard: reviews needing attention, assigned issues, recent notifications.
Issue list with filtering and inline preview.
Review navigation with diff and threaded comments.
All read from local refs — instant, no spinner.

The TUI handles everything except the actual code review reading experience, which it hands off to the editor.

### CLI (Tertiary: Quick Actions and Scripting)

The CLI is a non-interactive tool for quick mutations and automation.
It does one thing per invocation, composes with unix tools, and supports `--json` output on every command.

The CLI never pushes, never fetches, never blocks on network.
It writes refs and returns.
The daemon handles all transport.

```text
git forge
├── status                    # overview: current branch review, CI, assigned issues, notifications

├── issue
│   ├── new [title]           # opens editor if title omitted (git-style: first line = title, blank, body)
│   ├── show <id> [--oneline]
│   ├── list [--state {open|closed|all}] [--label ...] [--assignee ...]
│   ├── edit <id> [--title] [--body]
│   ├── close <id>
│   ├── reopen <id>
│   ├── label <id> --add <label> | --remove <label>
│   ├── assign <id> --add <user> | --remove <user>
│   └── comment <id> [body]   # porcelain: routes to comment system scoped to this issue
│
├── review
│   ├── new [--target <branch>]
│   ├── show [id]             # defaults to current branch's review
│   ├── list [--state {open|merged|closed}]
│   ├── edit [id] [--title] [--description]
│   ├── approve [id]
│   ├── request-changes [id]
│   ├── merge [id]
│   ├── close [id]
│   └── comment [id] [body]   # porcelain: routes to comment system scoped to this review
│
├── comment                   # plumbing: annotate raw git objects
│   ├── new [target] [--body] [--anchor] [--anchor-type] [--range]
│   ├── reply <comment-oid> [--body]
│   ├── edit <comment-oid> [--body]
│   ├── resolve <comment-oid>
│   ├── list [target]
│   └── show [target] [--threads]
│
├── release
│   ├── prepare
│   ├── publish
│   ├── list
│   └── show <version>
│
├── check
│   ├── run <name>
│   ├── status [commit]
│   └── list
│
├── contributor
│   ├── add <id> [--name "..."] [--email "..."]
│   ├── list
│   └── show <id>
│
├── sync [remote]             # manual fallback: fetch + push all forge refs
├── install [remote] [--global]
└── daemon                    # start the sync/LSP daemon
```

Key design decisions:

- `issue comment` and `review comment` are porcelain that routes through the comment system scoped to the entity. `comment` (top-level) is plumbing for annotating raw git objects.
- `review` commands default to the current branch's active review when no ID is given.
- Editor templates use Git's own convention: first line is title, blank line, body.
  No TOML frontmatter.
- `show` with `--oneline` replaces the separate `status` subcommand per entity.
- `close`/`reopen`/`approve`/`merge` are direct workflow verbs, not flags on `edit`.
- `label` and `assign` support `--add`/`--remove` for incremental changes.


## UI Server

The UI server has direct filesystem access to the repository.
It does not go through the Git transport protocol.

**Entity creation.**
ID assignment is a filesystem lock + scan + increment.
No CAS retry loop, no hooks.
The UI is the fast path for entity creation.

**Read-optimized projections.**
The data model is append-only refs and metadata.
The UI maintains derived indexes in memory or on disk for fast querying:

- **Comment reanchoring visualization.**
  Blame-mapped comment threads inline on a file view.
- **Review rounds.**
  Diffing revision N against revision N-1 of a review.
  The revisions entries provide the commit pairs; the UI renders interdiffs.
- **Approval coverage.**
  Which patches in a review are approved and which aren't, aggregated into a progress view across all four approval levels.
- **Cross-reference graph.**
  The relational metadata trees provide both directions.
  The UI renders the full graph — "these 3 comments and 2 issues reference this commit."

Correctness never depends on the UI's indexes — they can always be rebuilt from refs.


## Server

The minimal Forge server is an HTTP middleware in front of `git-http-backend`.
It handles:

1. **Authentication.**
   Verifies the user's identity.
2. **Ref filtering.**
   Strips private refs from the advertisement based on role.
3. **Pre-receive hooks.**
   Enforces policy, signatures, approvals, path permissions, and check results.
4. **Post-receive hooks.**
   Triggers reanchoring, notification writes, index updates, inbox canonicalization.
5. **Real-time notifications.**
   Pushes events to connected clients.
6. **Merge queue processing.**
   Rebase, build, push.
7. **Metadata auto-merge.**
   Three-way tree merge on non-fast-forward metadata pushes.
8. **Secret management.**
   Encrypted storage, ACL enforcement, tmpfs injection for runners.

The server is stateless (except for the secret store).
All other state is in Git.
If the server goes away, the data is intact.
The server is a convenience, not a dependency.


## Migration

### GitHub App

A GitHub App bridges adoption.
It receives webhook events and writes refs via GitHub's API:

- Mirror issues and comments into Forge ref format.
- Update review refs on PR events.
- Write approval refs on PR review events.

Everything written as refs is portable.
When a team migrates off GitHub, the refs come with them.

### Sync

```sh
git forge sync github --repo=org/project --import
```

Issues, PRs, comments, labels, and milestones are read via the platform API and written as Forge refs.
Sync state is stored as a ref.

Imported contributors who have no signing keys in the forge model are recorded as read-only entries in the contributor tree — their historical activity is preserved, but they are not onboarded until they register with a key.
