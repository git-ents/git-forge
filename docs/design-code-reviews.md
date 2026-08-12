# Design: code reviews

## Summary

A code review is an issue-like, mutable `gix-store` document with a typed `target` field.
Interactions with the review are separate objects:

- comments are `Comment` entities linked to the review;
- code comments are review comments with an optional inline anchor binding;
- approvals and denials are signed `gix-attest` claims about the review target.

The review document does not contain verdicts, passes, approval counts, or merge policy.
Queries and gates may interpret these objects later, but they are separate features.

## Review document

Keep `Review` under:

```text
refs/forge/review/<review-id>
```

Its behavior is the same as an issue: typed storage, ordinary entity CRUD, Git-backed history, and comments through `Commentable`.
The current shape is already close:

```rust
pub struct Review {
    pub id: String,
    pub status: String,
    pub body: String,
    pub reviewers: Vec<String>,
    pub requesters: Vec<String>,
    pub target: ReviewTarget,
    pub edit: Option<String>,
}
```

Some workflow fields may change independently, but `target` remains part of the review document.
Updating the document does not rewrite its comments or signed decisions.

## Review target

`ReviewTarget` is a typed description of the exact Git material being reviewed.
It does not need an anchor: blobs, trees, and commits already have immutable content identities.

The target vocabulary should cover these shapes:

```rust
pub enum ReviewTarget {
    Blobs(Vec<PinnedBlob>),
    Trees(Vec<PinnedTree>),
    TreeRange {
        base: PinnedTree,
        tip: PinnedTree,
    },
    Commits(Vec<GitOid>),
    CommitRange {
        base: GitOid,
        tip: GitOid,
    },
}
```

Single-object targets are one-element collections.
A GitHub-style pull request target is normally `CommitRange { base, tip }`, where `base` is the target branch commit and `tip` is the feature branch commit.
Forge records those endpoints; ancestry and diff interpretation belong elsewhere.

The exact Rust wrappers and serialized variant names can be finalized during implementation.
The important distinction is between direct Git object links and scalar commit IDs.

### Object retention

Blob and tree targets should be embedded as real entries in the review's typed target tree.
Those Git edges make the objects reachable from the review ref.
A tree range is a small structured tree whose `base` and `tip` entries point directly to the endpoint trees.

`facet-git-tree` already supports direct tree links through `RawTree`.
It needs the equivalent typed passthrough for blobs, or a target-specific structured-tree codec that preserves blob entries.

Commits cannot be retained by ordinary blob or tree entries.
Gitlinks name commits but are not repository reachability edges.
The initial model therefore assumes reviewed commits remain reachable through repository refs.
If repositories need stronger retention later, they can add explicit keep refs such as `refs/jj/keep/*` without changing the review interaction model.

## Comments

Comments remain ordinary forge `Comment` entities linked by subject:

```text
review:<review-id>
```

Two existing forms cover review discussion:

- a review-level comment has the review subject and no binding;
- a code comment has the review subject and an inline `gix_anchor::Binding` locating the relevant code.

Anchors are useful for code locations that may be projected through edits.
They are not the identity or storage mechanism for the review target itself.

## Approvals and denials

An approval or denial is a separate signed attestation, not a mutation of `Review`.
Forge owns a small typed payload:

```rust
pub struct ReviewDecision {
    pub review_id: String,
    pub verdict: ReviewVerdict,
    pub message: Option<String>,
}

pub enum ReviewVerdict {
    Approve,
    Deny,
}
```

The attest envelope targets the reviewed Git material itself:

```text
target: exact blob, tree, tree range, commit, or commit range
payload kind: review-decision
payload: ReviewDecision
signer: reviewer's signing key
```

For a single blob, tree, or commit, the target ID is its Git object ID.
For a collection or range, the target ID is the store hash of its canonical typed target descriptor, following `gix-attest`'s existing compound-target convention.
The target kind distinguishes these shapes.

`review_id` links the interaction to the review discussion and workflow.
The attest target says exactly what material the signer approved or denied.

Each user publishes an independent signed claim.
Claims are append-only interactions; they are not embedded in the mutable review document and do not contend on the review ref.
Revocation, if used, is another attest-chain operation.

## API sketch

The forge API should expose the three concepts directly:

```rust
Review::create_in_repo_as(repo, auth, review)
review.add_comment_as(repo, auth, body)
review.add_anchored_comment_as_at(repo, auth, revision, path, lines, body)
ReviewDecision::approve_as(repo, auth, review_id, message)
ReviewDecision::deny_as(repo, auth, review_id, message)
```

Publishing a decision loads the review, derives the attest target from its typed `target` field, writes the typed payload, and signs the claim.
The signing identity comes from authenticated configuration, never from a caller-supplied author string.

## Explicitly separate features

This design does not define:

- reviewer eligibility or self-approval policy;
- quorum or required approval counts;
- how conflicting or repeated decisions are interpreted;
- whether a review is currently approved;
- whether a branch is mergeable;
- query predicates or cache invalidation;
- merge gates or `git-effect` execution;
- carrying approval onto changed targets.

Those are query and gate concerns built over the stored review, comments, and attestations.
They do not require additional review-document machinery.

## Implementation plan

1. Finalize the small `ReviewTarget` vocabulary and canonical compound-target encoding.
2. Add direct typed blob retention alongside the existing `RawTree` support.
3. Adapt the existing `ReviewTarget` variants without changing the review entity model.
4. Register the `review-decision` payload schema and integrate `gix-attest` into forge.
5. Add authenticated approve and deny library operations and CLI commands.
6. Test review comments, code comments, object retention, target derivation, signing, and revocation independently.

## Tests

The core feature should verify:

- every target shape round-trips through `gix-store`;
- blob and tree objects remain reachable through the review document tree;
- commit targets preserve their exact IDs without claiming reachability;
- review-level and anchored code comments remain linked to the review;
- approval and denial payloads link to the review but attest the exact target;
- different users can publish independent decisions for the same target;
- editing or deleting a review does not rewrite existing interaction objects;
- malformed targets and unsigned or invalidly signed decisions are rejected.

## Open target details

Before implementation, settle only the representation details required for stable target IDs:

- exact names and shapes of collection and range variants;
- ordering rules for multi-object targets;
- whether blob and subtree paths are descriptive fields;
- canonical compound-target hashing;
- the repository invariant for commit retention and whether keep refs are needed.
