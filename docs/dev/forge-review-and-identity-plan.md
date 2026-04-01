# Forge Review System & Unified Identity Model

Development plan for refactoring forge to support the new review system, contributor model, and unified UUID v7 identity.
This document is authoritative — the agent implementing this should follow it as-is.

## Core principles

- **Trees are schemas.**
  No TOML, no JSON, no serialization formats.
  Entry names are keys, blobs (or subtrees) are values.
- **UUID v7 everywhere.**
  Every entity (issue, review, contributor, comment) is a ref keyed by UUID v7.
  Human-friendly identifiers (display IDs, handles) are resolved at the edges.
- **Git tells you the type.**
  `git cat-file -t <oid>` determines whether a reviewed object is a blob, tree, or commit.
  No explicit type fields.
- **History is the commit log.**
  State transitions, target updates, and metadata changes are commits on the entity's ref.
  No explicit version tracking structures.

---

## Entity model

### Contributors

```text
refs/forge/contributors/<uuid-v7> → commit → tree
├── handle              # blob: "alice" — mutable, must be unique across contributors
├── name                # blob: "Alice Smith"
├── email               # blob: "alice@example.com"
├── keys/
│   ├── <fingerprint>   # blob: public key material
│   └── ...
├── roles/
│   ├── admin           # empty blob — presence means role is granted
│   ├── maintainer      # empty blob
│   └── ...
```

Rules:

- `handle` is mutable.
  Renaming a contributor updates only this blob.
  All other refs reference the UUID, not the handle.
- `handle` must be unique.
  The CLI and MCP resolve handle → UUID at write time, UUID → handle at display time.
- Bootstrap: the first commit to `refs/forge/contributors/` is self-signed.
  It creates the first contributor with their key and an admin role.
- Key rotation: add a new entry under `keys/`, optionally remove the old one.
  Nothing else in the repo changes.
- Signature verification: scan `keys/` subtrees across contributors to map a commit's signing fingerprint → contributor UUID.

### Issues

```text
refs/forge/issues/<uuid-v7> → commit → tree
├── title               # blob: issue title
├── state               # blob: "open" | "closed"
├── author              # blob: <contributor-uuid>
├── body                # blob: markdown
├── labels/
│   ├── bug             # empty blob
│   └── ...
├── assignees/
│   ├── <contributor-uuid>   # empty blob
│   └── ...
```

Rules:

- Sequential display IDs are assigned at sync time (CAS push), not at creation.
  Offline-created issues use staging refs until sync.
- `author` and entries in `assignees/` are contributor UUIDs, never fingerprints or handles.
- State transitions are commits on the ref. `git log` on the ref is the audit log.
- Comments are separate entities under `refs/forge/comments/<uuid-v7>`, linked to the issue via relational metadata (`Related-To` trailer on the comment's root commit).

### Reviews

```text
refs/forge/reviews/<uuid-v7> → commit → tree
├── title               # blob: review title
├── state               # blob: "open" | "closed" | "merged" | "draft"
├── author              # blob: <contributor-uuid>
├── body                # blob: markdown description
├── labels/
│   ├── needs-review    # empty blob
│   └── ...
├── assignees/
│   ├── <contributor-uuid>   # empty blob
│   └── ...
├── target/
│   ├── head            # blob: <oid> — always present
│   └── base            # blob: <oid> — optional; presence implies range review
├── objects/
│   ├── <oid>           # empty blob — keeps objects reachable for GC
│   ├── <oid>
│   └── ...
├── approvals/
│   ├── <oid>/
│   │   ├── <contributor-uuid>   # empty blob
│   │   └── ...
│   └── ...
```

#### target/

Defines the review scope. `cat-file -t` on `target/head` determines the review type:

| `head` type | `base` present? | Review kind |
|---|---|---|
| blob | no | Single file review |
| tree | no | Directory/crate review |
| commit | no | Single commit review |
| commit | yes | Commit range review (base..head) |

- `base`, if present, must also be a commit.
- Updating the target (force-push, retarget) is a new commit on the review ref that changes `target/head` and/or `target/base`.
  The previous target is in the ref's commit history.

#### objects/

A manifest of what `target/` specifies.
Keeps the referenced OIDs reachable for GC.

| Review kind | `objects/` contains |
|---|---|
| blob | the single blob OID |
| tree | the single tree OID |
| commit | the single commit OID |
| range (base..head) | every commit OID in the range |

`objects/` is derived mechanically from `target/`.
It is not a place to add arbitrary extra objects.
Updating `objects/` (new commit on the review ref) replaces the set.

#### approvals/

Approvals are scoped to the review.
Each entry in `approvals/` is keyed by an OID from `objects/`, and contains contributor UUIDs as sub-entries.

- Approve a specific object: add your contributor UUID under `approvals/<oid>/`.
- Approve everything: add your contributor UUID under every OID in `objects/`.
  The CLI handles this: `forge review approve` writes all; `forge review approve <path>` resolves path → OID and writes one.
- "Is this review fully approved?" is derived at read time: for each OID in `objects/`, check `approvals/<oid>/` for sufficient entries.
  Policy defines "sufficient."

The signed commit that adds an approval entry is the cryptographic proof.
The commit's signing key maps to the contributor UUID via the contributor's `keys/` subtree.

**There is no global approval namespace.**
Approvals do not exist outside a review context.
To approve existing code without a prior review request, create a review targeting the relevant objects and approve it.

#### Review lifecycle

- **open**: accepting comments and approvals.
- **draft**: not ready for review.
  Assignees are not expected to act.
- **merged**: terminal.
  The target was incorporated.
  On merge, a commit records `merged_as` (blob containing the merge commit OID) if applicable.
- **closed**: terminal without merge.

State transitions are commits on the review ref.
Automatic closing (e.g., branch merged externally) is handled at another layer — not part of this design.

#### Who creates reviews?

Anyone.
The review author is not necessarily the code author.
Use cases:

- Author opens review for their own branch (common case).
- Maintainer opens review targeting someone else's commits.
- Tech lead opens review targeting existing code on main for audit.
- Anyone opens a review to approve something — the review is both the request and the record.

### Comments

Comments are unchanged from the current design.
Included here for completeness.

```text
refs/forge/comments/<uuid-v7> → commit → tree
├── anchor              # blob: blob OID, start_line, end_line
├── context             # blob: surrounding source lines
├── body                # blob: markdown
├── suggestion          # blob: optional replacement text
├── attachments/        # subtree: optional
│   └── ...
```

Trailers on commits: `Anchor-Blob: <oid>`, `Comment-Id: <uuid-v7>`, `Related-To: <entity-type>/<uuid-v7>`, `Resolved: true`, `Replaces: <commit-oid>`.

Threading: first-parent chain is the thread.
Replies append as new commits on the same ref.

---

## What changes from the current codebase

### Identity model

**Before:** Fingerprints used as identity throughout.
Contributor entries keyed by fingerprint.

**After:** Contributor UUIDs used everywhere.
Fingerprints are verification mechanisms stored under `keys/`.
Handle is a mutable display name.

This is a breaking change.
Every ref that stores an author, assignee, or approval identity must be migrated from fingerprint to contributor UUID.

### Review model

**Before:** Reviews stored comments and approvals differently (or not at all — check current implementation).

**After:** Reviews have `target/`, `objects/`, and `approvals/` subtrees.
No embedded comments.
Approvals are per-OID within the review.

### Approval model

**Before:** Global `refs/metadata/approvals` namespace with fanout by patch-id.

**After:** No global approval namespace.
Approvals live on review refs under `approvals/<oid>/<contributor-uuid>`.
The merge gate queries review refs, not a flat index.

### Tree-as-schema

**Before:** TOML `meta` files for structured data.

**After:** Tree entries are the schema.
Each field is a blob or subtree.
No serialization format.

This applies to issues, reviews, and contributors.
Every entity type must be migrated.

---

## Implementation phases

### Phase 0: Design doc

Commit this document to `docs/design/forge-review-and-identity-plan.md`.
Mark any superseded design docs.

### Phase 1: Contributor model

Introduce the new contributor ref structure.

1. Define contributor types: `ContributorId` (UUID v7 newtype), `Handle` (validated string newtype).
2. Implement `refs/forge/contributors/<uuid-v7>` with tree-as-schema layout.
3. Implement handle → UUID resolution (scan contributors, cache in memory).
4. Implement fingerprint → contributor UUID resolution (scan `keys/` subtrees).
5. Bootstrap path: first contributor is self-signed, gets admin role.
6. CLI: `forge contributor add <handle>`, `forge contributor list`, `forge contributor show <handle>`, `forge contributor rename <old> <new>`.

**Acceptance:** Can create contributors, resolve handles to UUIDs, verify commit signatures map to contributor identities.

### Phase 2: Tree-as-schema for issues

Migrate issue storage from TOML `meta` blob to tree entries.

1. Replace `meta` TOML blob with individual tree entries: `title`, `state`, `author`, `body`, `labels/`, `assignees/`.
2. `author` and `assignees/` entries store contributor UUIDs, not fingerprints.
3. Update all read/write paths in the issue executor.
4. Update CLI display to resolve UUIDs → handles.
5. Write migration: read old format, write new format, for any existing issues.

**Acceptance:** `forge issue create`, `forge issue list`, `forge issue show` all work with new tree layout.
Old issues are migrated on first write (or via explicit migration command).

### Phase 3: Review entity

Implement the review ref structure from scratch (or refactor existing).

1. Define review types: tree layout with `title`, `state`, `author`, `body`, `labels/`, `assignees/`, `target/`, `objects/`, `approvals/`.
2. Implement review creation: `forge review create --head <oid>` (single object) or `forge review create --base <oid> --head <oid>` (range).
3. On creation, populate `objects/` mechanically from `target/` — for a range, enumerate every commit in base..head; for a single object, just that OID.
4. Implement state transitions as commits on the review ref.
5. Implement target updates (new commit changing `target/head` and/or `target/base`, updating `objects/`).

**Acceptance:** Can create reviews targeting blobs, trees, commits, and commit ranges. `git cat-file -t` on `target/head` correctly identifies review type. `objects/` keeps referenced OIDs reachable.

### Phase 4: Approval system

Implement approvals scoped to reviews.

1. `forge review approve [<path>]` — resolves path to OID (or approves all if no path), writes contributor UUID under `approvals/<oid>/`.
2. The commit adding the approval must be signed.
   Signature maps to contributor UUID.
3. `forge review status` — for each OID in `objects/`, check `approvals/<oid>/` for entries.
   Display coverage.
4. Remove the global `refs/metadata/approvals` namespace if it exists.
   All approval queries go through review refs.

**Acceptance:** Can approve individual objects or all objects in a review.
`forge review status` shows per-object approval coverage.
No global approval refs exist.

### Phase 5: Derived approval index

The merge gate needs to answer: "for the diff between base and tip of this branch, are all objects approved in some review?"

1. Implement a derived index: `refs/forge/index/approvals-by-oid` mapping OIDs → review UUIDs that contain approvals for them.
2. Index is rebuilt from review refs.
   It is a cache, not truth.
3. Merge gate: compute objects in diff, check index, verify against actual review refs.

This is an optimization phase.
Without the index, the gate scans all review refs.
With the index, it's a lookup.
Defer if the number of reviews is small enough that scanning is fast.

**Acceptance:** Merge gate can determine whether a branch's changes are sufficiently approved.

### Phase 6: CLI polish

1. `forge review list` — list reviews with display IDs, titles, states, target types.
2. `forge review show <id>` — display review details, target, object list, approval status.
3. `forge review approve` / `forge review approve <path>` — as above.
4. `forge review create` — interactive or flag-based.
5. `forge review close` / `forge review merge`.
6. Ensure all commands resolve handles ↔ UUIDs transparently.

### Phase 7: MCP server

Expose review operations through the forge MCP server.

1. `list_reviews`, `get_review`, `create_review`, `approve_review`.
2. Approval status as structured data for LLM consumption.
3. Review targets described with object type and path hints.

### Phase 8: Cleanup

1. Remove old review code paths.
2. Remove global approval namespace code.
3. Remove TOML meta parsing for issues (if migration is complete).
4. Remove fingerprint-as-identity code paths.
5. Update all documentation.

**Acceptance:** `git grep` for old patterns returns nothing.
All tests pass.

---

## Ordering and dependencies

```text
Phase 0 (doc)
  │
Phase 1 (contributors) ──→ Phase 2 (issues tree-as-schema)
  │                              │
  └──────────────────────→ Phase 3 (review entity)
                                 │
                           Phase 4 (approvals)
                                 │
                           Phase 5 (derived index)
                                 │
                           Phase 6 (CLI) ──→ Phase 7 (MCP)
                                               │
                                         Phase 8 (cleanup)
```

Phase 1 must land first — everything else depends on contributor UUIDs existing.
Phases 2 and 3 can be parallelized.
Phase 4 depends on Phase 3.
Phases 6–7 are polish.
Phase 8 is last.

---

## Non-goals for this plan

- **GitHub sync.**
  Review and approval sync with GitHub is a separate concern.
- **Comment system changes.**
  The comment system is already redesigned separately.
  This plan assumes comments are independent entities linked via relational metadata.
- **Automatic closing.**
  Reviews being auto-closed when a branch merges is handled at another layer.
- **Web UI.**
  The SQLite-indexed web UI consumes review data via the same `git for-each-ref` + tree-read path.
- **Policy engine.**
  The full policy system (required approvals, path-scoped permissions, branch protection) is designed but not implemented in this plan.
  The approval structure supports it; the enforcement layer is separate work.
- **External identity / DIDs.**
  The contributor model supports adding a `did` blob in the future.
  Not part of this plan.

---

## Key design decisions and rationale

**Why UUID v7 for contributors (not just issues/reviews)?**
Handles are mutable (renames).
UUIDs are permanent.
Every reference to a contributor throughout the repo uses the UUID.
Renaming is a single blob update on the contributor ref.
Without UUIDs, renaming requires rewriting every ref that mentions the contributor.

**Why no global approval namespace?**
Approvals without a defined scope are ambiguous.
"I approved this blob" — in what context?
The review provides the framing.
The merge gate has a clear contract: find reviews targeting the relevant objects with sufficient approvals.
Not "scan all approvals and hope they cover the diff."

**Why approvals are per-OID within a review?**
Partial approval is useful.
A reviewer confident about 3 of 5 crates approves those 3.
The review shows exactly what's covered.
Bulk approval (`forge review approve` with no path) writes all OIDs in one commit for the common case.

**Why tree-as-schema instead of TOML?**
Git already provides a key-value store with nesting (trees).
Serialization adds a layer that doesn't earn its keep.
Tree entries are individually addressable, diffable, and mergeable.
A TOML blob is opaque to git's merge machinery.

**Why `objects/` exists separately from `target/`?**
`objects/` is a manifest of what `target/` specifies — it prevents garbage collection by making the OIDs reachable from the review ref.
`target/` defines the semantic scope (head, optionally base).
`objects/` materializes the full set of objects that scope implies.
They serve different purposes: `target/` is the definition, `objects/` is the enumeration.

**Why reviews are distinct from issues (not a unified entity with optional target)?**
They have different lifecycles (`merged` state), different required fields (`target/`, `objects/`, `approvals/`), and different read patterns.
Sharing infrastructure (tree-as-schema, UUID refs, display IDs) is sufficient.
Forcing them into one entity type muddies both.
