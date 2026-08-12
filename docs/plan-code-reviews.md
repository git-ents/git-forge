# Code review implementation plan

Scope: `git-forge`, with only the supporting changes required in `git-store` and the existing `gix-attest` integration points.
This plan does not implement queries, gates, mergeability, quorum, reviewer eligibility, or policy.

## Confirmed state

### Already implemented

- `Review` is an issue-like mutable `gix-store` entity with CRUD, schema publication, history, deletion, and authenticated ownership.
  - `crates/gix-forge/src/review.rs`
  - `crates/gix-forge/src/entity.rs`
- `Review` already implements `Commentable`.
- `Comment` is a separate entity linked by `review:<review-id>` and already persists optional `gix_anchor::Binding`.
  - `crates/gix-forge/src/comment.rs`
- Review CRUD CLI commands already exist.
  - `crates/git-forge/src/main.rs`
- `facet-git-tree::RawTree` embeds an existing Git tree by OID and preserves a tree entry.
- `gix-attest` already provides opaque `{ kind, id }` targets, typed envelopes, target-key hashing through `facet-git-tree` normal form, claim chains, signing through `gix-store::Signer`, and revocation.
- `gix-store` already records schema migrations when an evolved schema can be derived from its predecessor.

### Genuine gaps

- `ReviewTarget` currently has scalar `Blob`, `Tree`, and `Commit` variants plus string pair/range variants.
  It lacks the required five shapes:
  - blobs;
  - trees;
  - tree ranges;
  - commits;
  - commit ranges.
- Collections are not modeled or canonically ordered.
- Blob/tree target fields currently store strings, so they do not create Git reachability edges from the review document.
- `facet-git-tree` has `RawTree` but no typed passthrough for an already-existing Git blob.
  This should be a small parallel extension, not a new hashing scheme: generic typed blobs already serialize, but preserving an existing blob OID requires an explicit blob-entry passthrough.
- Forge has no `ReviewDecision` payload, decision schema registration, signed approval/denial API, or `gix-attest` dependency.
- Forge opens entities with `refs/forge/...`; attest claims must use a store with `gix-attest::layout()` so claims land under `refs/claims/<target-key>`.
- The existing signer seam is sufficient (`gix-store::Signer`), but forge has no production path that connects authenticated Git signing configuration to a signed attest claim.
  Unsigned decisions must be rejected.
- Existing review schemas and documents persist the old target shape.
  This is a real compatibility concern; use `gix-store` migration/upcast support only as required by inspection.

### No required changes currently identified in `gix-attest`

`gix-attest::Target` already accepts the exact target kind and an OID.
Forge should own the typed target descriptor and derive collection/range IDs through `facet-git-tree` typed-tree hashing, then pass the resulting OID to attest.
Do not add attest-specific review payload logic or a second compound-hashing scheme unless implementation exposes a concrete API defect.

## Ordered phases

1. Add the minimal typed Git blob passthrough in `git-store`.
2. Define and test the canonical `ReviewTarget` model.
3. Integrate blob/tree retention and persisted-schema compatibility.
4. Add signed review decisions.
5. Add CLI approval/denial commands.
6. Run interaction-isolation and full integration validation.

## Luna tasks

### A. Add typed Git blob passthrough

**Write scope:** only `git-store/crates/facet-git-tree/src/` and its focused tests.

- **Objective:** preserve an existing Git blob OID as a blob entry inside a typed tree.
- **Inspect/modify:**
  - `git-store/crates/facet-git-tree/src/lib.rs`
  - `git-store/crates/facet-git-tree/src/raw_tree.rs`
  - `git-store/crates/facet-git-tree/src/classify.rs`
  - `git-store/crates/facet-git-tree/src/ser.rs`
  - `git-store/crates/facet-git-tree/src/de.rs`
  - focused tests under `git-store/crates/facet-git-tree/tests/`
- **Changes:** follow `RawTree`'s existing pattern with the smallest `RawBlob`-style type; classify it before generic scalar handling; serialize as `EntryKind::Blob`; deserialize only after verifying the referenced object is a blob; leave `RawTree` unchanged.
- **Acceptance:** round-trip preserves the supplied blob OID and emits an actual blob entry; tree passthrough remains an actual tree entry; non-blob input is rejected.
- **Validation:**

  ```sh
  cargo test --manifest-path ../git-store/Cargo.toml -p facet-git-tree
  ```

### B. Define canonical `ReviewTarget`

**Write scope:** `git-forge/crates/gix-forge/src/review.rs` and its tests.

- **Objective:** implement the fixed five target shapes with exact Git-object identity.
- **Inspect:** current `review.rs`; `facet-git-tree` normal-form APIs; `gix-attest/src/envelope.rs`.
- **Changes:**
  - Add typed wrappers for pinned blobs/trees and exact Git OIDs.
  - Model `Blobs(Vec<_>)`, `Trees(Vec<_>)`, `TreeRange { base, tip }`, `Commits(Vec<_>)`, and `CommitRange { base, tip }`.
  - Keep paths descriptive; identity comes from OIDs.
  - Canonically sort unordered collections before serialization and hashing.
  - Reject malformed OIDs and invalid empty collections.
  - Add a typed target descriptor serialized and hashed through existing `facet-git-tree` APIs.
  - Use direct OIDs for single-object targets and the typed descriptor hash for collections/ranges.
  - Keep commit targets as scalar OIDs; do not use gitlinks for retention.
  - Update parse/display only as needed for the new public vocabulary.
- **Acceptance:** every shape round-trips; collection order does not affect identity; OID changes affect identity; descriptive paths do not become identity-bearing; malformed targets fail clearly.
- **Validation:**

  ```sh
  cargo test -p gix-forge review
  ```

### C. Integrate target retention

**Write scope:** `git-forge/crates/gix-forge/src/review.rs` tests and implementation only, after A and B.

- **Objective:** make blob/tree target objects reachable from the review document tree.
- **Changes:**
  - Encode blob targets with the new blob passthrough.
  - Encode tree targets with `RawTree`.
  - Encode tree ranges as a structured tree with direct `base` and `tip` tree entries.
  - Preserve commit IDs without claiming reachability.
  - Canonicalize before storing and before deriving any target ID.
- **Acceptance:** review trees contain direct blob/tree edges; tree-range endpoints are direct tree entries; commit targets remain exact IDs only.
- **Validation:**

  ```sh
  cargo test -p gix-forge review
  ```

### D. Resolve persisted review-schema compatibility

**Write scope:** `git-forge/crates/gix-forge/src/review.rs` and narrowly scoped migration fixtures/tests only.
Do not modify generic migration infrastructure.

- **Objective:** keep existing `refs/forge/review/*` documents readable after the target-shape change.
- **Inspect:**
  - `git-store/crates/gix-store/src/kind.rs`
  - `git-store/crates/gix-store/src/migrate.rs`
  - `git-store/crates/facet-git-tree/src/migration/`
  - existing schema-evolution tests in `git-store`
- **Changes:** determine whether old target variants can be upcast by the existing migration system.
  If yes, publish the derived migration and test every old variant.
  If not, add only a private compatibility decode/upcast at the review boundary.
  Do not rewrite existing review objects or attestations.
- **Acceptance:** old persisted reviews load or return an explicit compatibility error; new reviews use only the canonical model; no identity is silently changed.
- **Validation:**

  ```sh
  cargo test -p gix-forge review
  cargo test --manifest-path ../git-store/Cargo.toml -p gix-store
  ```

### E. Add signed review decisions

**Write scope:** new `git-forge/crates/gix-forge/src/decision.rs`, `crates/gix-forge/Cargo.toml`, `src/lib.rs`, and `src/error.rs`.
No CLI files.

- **Objective:** publish approvals and denials as separate signed attest claims.
- **Inspect:**
  - `git-anchor/crates/gix-attest/src/envelope.rs`
  - `git-anchor/crates/gix-attest/src/chain.rs`
  - `git-anchor/crates/gix-attest/src/schema.rs`
  - `git-store/crates/gix-refstore/src/signer.rs`
  - existing Git signing-key/configuration code in `crates/git-forge/src/main.rs`
- **Changes:**
  - Add `ReviewDecision { review_id, verdict, message }` and `ReviewVerdict::{Approve,Deny}`.
  - Add `gix-attest` as a workspace and forge dependency.
  - Register the forge-owned `review-decision` payload schema under the forge schema layout.
  - Open claims with `gix-attest::layout()`.
  - Load the review and derive the exact attest target from its canonical target descriptor.
  - Serialize the payload through typed `gix-store`, then call `Claims::sign` with payload kind `review-decision`.
  - Connect authenticated signing configuration to the existing `gix-store::Signer` seam.
    Reject missing signers; never use a caller-supplied author string as identity.
  - Expose authenticated `approve_as` and `deny_as` operations.
  - Keep verdicts out of `Review`; do not add policy or validity interpretation.
- **Acceptance:** claims carry `review_id`; target kinds and IDs exactly identify the reviewed material; independent users can append claims; unsigned publication fails; revocation remains an attest-chain operation.
- **Validation:**

  ```sh
  cargo test -p gix-forge decision
  cargo test --manifest-path ../git-anchor/Cargo.toml -p gix-attest
  ```

### F. Add CLI approval/denial commands

**Write scope:** `git-forge/crates/git-forge/src/main.rs` and CLI tests only.

- **Objective:** expose the decision API without adding policy behavior.
- **Changes:** add review approve/deny subcommands accepting review ID and optional message; resolve IDs through existing helpers; call authenticated library operations; print the claim ID; reject anonymous or unsigned publication.
- **Acceptance:** both commands produce independently verifiable claim commits; existing CRUD output and behavior remain unchanged.
- **Validation:**

  ```sh
  cargo test -p git-forge --test cli review
  ```

### G. Verify comments and interaction isolation

**Write scope:** tests in `crates/gix-forge/src/review.rs`, `comment.rs`, and relevant CLI tests.
No production overlap unless a test identifies a genuine missing API.

- **Objective:** lock down the already-designed separation of mutable reviews, comments, and attestations.
- **Changes:** test review-level comments with no binding; anchored comments with `Binding::Position`; `review:<id>` subjects; review edits/deletes do not rewrite or delete comments or decisions.
- **Acceptance:** comments remain separate entities and anchors remain embedded `Binding` values.
- **Validation:**

  ```sh
  cargo test -p gix-forge comment review
  ```

## Parallel-safe execution

- **Parallel group 1:**
  - A: `git-store/crates/facet-git-tree/src/**`
  - G: tests only for current comment behavior
  - D: migration feasibility inspection only; implementation waits for B
- **Sequential:** B, then C and D.
- **Parallel group 2 after B–D:** E; CLI argument/test preparation may be separate, but F wiring waits for E.
- **Sequential:** F, then final G coverage.

Production write scopes are disjoint: A modifies only `git-store`; B–D modify review target code; E modifies decision/dependency/error/export files; F modifies only the CLI; G is test-only.

## Integration order and final validation

1. Merge A.
2. Merge B and freeze target serialization/hash fixtures.
3. Merge C.
4. Apply D only where persisted-schema inspection requires it.
5. Merge E and verify signed claims against exact targets.
6. Merge F.
7. Merge G.
8. Run:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path ../git-store/Cargo.toml -p facet-git-tree
cargo test --manifest-path ../git-store/Cargo.toml -p gix-store
cargo test --manifest-path ../git-anchor/Cargo.toml -p gix-attest
```

Final validation must cover all target shapes, canonical collection ordering, blob/tree reachability, non-retained commit targets, comments and anchors, signed approval/denial claims, independent decision chains, revocation compatibility, old persisted reviews, and the absence of verdict/policy fields on `Review`.

## Remaining implementation dependency

The only unresolved implementation detail is the concrete signer adapter/configuration path: reuse an existing `git-store` signer adapter if available, otherwise add the smallest forge-side adapter around the configured Git signing key.
It must use `gix-store::Signer`, authenticated configuration, and the existing attest signing flow; it must not introduce a new signing protocol.
