# Design: anchored code reviews

## Summary

A code review is a mutable, typed `gix-store` document whose scope is captured as one or more inline `git-anchor` bindings.
A reviewer’s approval or request for changes is **not** a field on that document: it is a separate, signed `git-attest` claim over the review scope.

This gives the feature four clear responsibilities:

- **Store** owns the review request document, schema, history, and CAS writes.
- **Anchor** owns durable content identity and projection through repository history.
- **Attest** owns signed reviewer passes, key validity, chaining, and revocation primitives.
- **Query** joins reviews, projected anchors, members, and verified passes to derive facts such as `approved`, `reviewed`, and `mergeable`.

The central invariant is:

> A mutable review describes a request; only a verified, policy-eligible pass can count as a reviewer decision.

This proposal is intentionally explicit about what is not yet implemented. `gix-forge` currently has typed reviews and anchored comments, but its `ReviewTarget` contains bare object-id strings, it does not depend on `gix-attest`, and its installed review rules are not backed by real forge-emitted claims.

## Goals and non-goals

### Goals

1. Review immutable Git material or a precisely defined change scope.
2. Preserve review scope as history changes, with bound, rebound, or orphaned outcomes rather than silently changing meaning.
3. Support these scopes:
   - one or more blobs;
   - one or more trees/subtrees;
   - a tree range;
   - one or more commits;
   - a commit range.
4. Allow multiple independent passes by different users.
5. Make approvals auditable, cryptographically verifiable, revocable, and policy-dependent.
6. Keep existing review comments as separate entities, with their inline bindings and review subject.
7. Make exact versus projected approval visible to query clients and the UI.

### Non-goals

- Storing the current approval verdict on `Review`.
- Making a review pass imply approval of future commits automatically.
- Defining one universal merge policy in `gix-forge`.
- Embedding comments or passes into the review document.
- Reimplementing anchor identity, projection, or signature verification in forge.

## Proposed data model

### Review request

Keep `Review` as the user-facing workflow document under:

```text
refs/forge/review/<review-id>
```

The existing fields remain conceptually appropriate:

```rust
pub struct Review {
    pub id: String,
    pub status: ReviewStatus,
    pub body: String,
    pub reviewers: Vec<MemberId>,
    pub requesters: Vec<MemberId>,
    pub scope: ReviewScope,
    pub edit: Option<String>,
}
```

`id` remains the ref-path identity and is omitted from `StoredReview`. `requesters` continue to determine ownership for ordinary review-document updates; being listed as a reviewer does not grant update or delete authority.

`status` describes the request lifecycle (`draft`, `open`, `closed`, `superseded`), not the result of any reviewer pass.
The exact enum and compatibility defaults should be finalized before schema publication; replacing the current free-form `String` is desirable but can be staged separately.

### Review scope

Replace the current stringly `ReviewTarget` semantics with a typed `ReviewScope` whose serialized form contains a versioned, canonical identity portion and descriptive hints.
The identity portion must use the store’s restricted identity normal form and must not be derived from `Display` output or an evolving general-purpose Facet representation.

Conceptually:

```rust
pub enum ReviewScope {
    Blobs { items: Vec<AnchorBinding> },
    Trees { items: Vec<AnchorBinding> },
    TreeRange { base: AnchorBinding, tip: AnchorBinding },
    Commits { items: Vec<AnchorBinding> },
    CommitRange { base: AnchorBinding, tip: AnchorBinding },
}
```

`AnchorBinding` is a forge-level wrapper around the applicable `gix_anchor::Binding` shape.
It is not a stored anchor reference and it does not contain a separately persisted anchor id.
Each binding is embedded in the review document, so the review cannot dangle if an index or derived anchor lookup disappears.

The concrete binding constructors must be supplied by `gix-anchor` for object and range scopes where the current `Position` binding is insufficient.
They should capture, at minimum:

- the genesis revision at which the scope was captured;
- the object/path/span identity needed for stable matching;
- algorithm and capture hints as non-identity metadata;
- endpoint ordering and range semantics for composite scopes.

A scope has one identity per anchored item.
For a multi-blob or multi-tree review, the canonical identity is an ordered or explicitly sorted list of child identities; the choice must be frozen and tested.
The order should normally be canonicalized so equivalent sets cannot produce different target keys.

#### Scope semantics

The five variants have these meanings:

- **Blobs**: approval covers the listed blob contents, optionally with path hints when path-at-capture is part of the contract.
- **Trees**: approval covers the listed tree or subtree contents.
  A subtree must include its normalized path; a root tree alone is not enough to identify a subtree.
- **TreeRange**: approval covers the transition from a captured base tree to a captured tip tree.
  It is not an arbitrary pair of unrelated trees.
- **Commits**: approval covers the listed commit identities as individual immutable objects.
- **CommitRange**: approval covers a directed transition from `base` to `tip`, with explicit ancestry, endpoint inclusion, and traversal policy.

The design should reject or explicitly mark invalid ranges when endpoints are missing or the required ancestry relation cannot be established.
It must define whether equal endpoints represent an empty scope and whether merge traversal is first-parent or full graph traversal.
Those semantics are part of the target identity/policy contract, not UI behavior.

The existing variants `BaseTipTreePair` and `BaseTipCommitPair` can be read through a compatibility adapter, but new documents should use the versioned scope vocabulary.
Existing target strings must never be reinterpreted under new rules.

### Anchor identity and projection

The review stores bindings inline, following the existing `Comment.binding` pattern.
The anchor id is computed from the binding identity subtree when needed; it is not an additional field and must not be used as a foreign key to a stored anchor entity.

At capture time, the API should require an explicit repository revision (defaulting to the resolved HEAD only in CLI convenience paths) and return the captured scope plus its genesis revision.
At read/query time, clients choose a projection revision:

```text
project(scope, revision) -> candidates with oracle and confidence
```

The anchor oracle supplies candidates; policy decides whether a candidate is acceptable.
The UI should distinguish at least:

- `exact`: the original identity is present;
- `rebound`: a projected candidate is being used, with oracle and confidence;
- `orphaned`: no acceptable candidate exists;
- `ambiguous`: multiple candidates require a manual pin.

A rebound scope must not silently count as an exact approval.
Query should expose both the projection and its confidence so a repository rule can choose a threshold or require a manual rebind pin.
A pin is an attest claim or other authoritative effect, not a mutation of the original review document.

## Review passes

### Separate signed assertion

Add a forge-owned typed payload, for example:

```rust
pub struct ReviewPass {
    pub review_id: String,
    pub verdict: ReviewVerdict,
    pub summary: Option<String>,
    pub protocol: String,
}

pub enum ReviewVerdict {
    Approve,
    RequestChanges,
    Comment,
}
```

The pass is serialized under a dedicated schema/payload kind such as `review-pass`.
It is not a `Review` entity and should not be written through ordinary review `save` semantics.

The attest envelope targets the canonical identity of the complete review scope:

```text
kind: review-scope-v1
id: hash(canonical scope identity)
payload kind: review-pass
payload: ReviewPass tree
signer: signing-key claim
chain: target claim chain
```

`review_id` provides discussion and workflow context.
It is not a substitute for the attest target.
If a pass includes a duplicate scope descriptor for export/debugging, query must reject a payload/envelope mismatch rather than choosing one silently.

The pass should be append-only.
Revoking a pass creates a revocation claim; deleting or editing the review request must not erase the historical decision.
Multiple users can therefore publish independent passes on the same scope without contending on the mutable review ref.

### “By other users” policy

A cryptographically valid signer is not automatically a forge member.
The query layer needs an explicit bridge from signing-key claims to member identities, for example:

```text
member_key(Member, KeyClaim)
```

A pass counts only when the selected policy establishes all required conditions:

1. the signature and claim chain verify;
2. the signing key is valid under the policy’s as-of point;
3. the key is associated with a forge member;
4. the member is eligible for this review/repository;
5. self-approval is accepted or rejected according to policy;
6. the pass has not been revoked;
7. the target is exact or its projection satisfies the configured confidence/pin policy.

The mutable `reviewers` list is an input to eligibility, not proof of identity or authority.
A future repository policy may use code owners, teams, or required reviewer sets instead.

## Query vocabulary and derived policy

Forge should contribute typed facts rather than force consumers to parse `ReviewTarget` display strings:

```text
review(R)
review_status(R, Status)
review_requester(R, Member)
review_reviewer(R, Member)
review_scope(R, ScopeKey)
review_scope_kind(R, Kind)

review_pass(C)
review_pass_review(C, R)
review_pass_verdict(C, Verdict)

claim_verified(C)
claim_target(C, ScopeKey)
claim_signer(C, KeyClaim)
member_key(Member, KeyClaim)
```

The query host should decode `review-pass`, not treat the mutable `review` document as a verdict payload.
This fixes the current mismatch where installed review rules refer to a `verdict` field that the actual `Review` does not contain and forge has no attest integration to populate.

Derived predicates should be policy modules, versioned under the query rule refs:

```text
exact_pass(Scope, Claim)
projected_pass(Scope, Claim, Confidence)
approval(Scope, Member)
reviewed(Scope)
mergeable(Commit)
```

A simple policy could require one eligible exact approval and no effective request for changes.
Another could require two distinct code-owner approvals.
Neither should be hard-coded into `Review`, `gix-attest`, or the anchor oracle.

For ranges, the rule must state whether approval covers the range atomically or all changed material.
If the latter is desired, query needs an explicit relation such as:

```text
changed_item(CommitRange, BlobOrTreeScope)
covers(ReviewScope, BlobOrTreeScope)
```

An approval for `base..tip` must not automatically apply to `base..new-tip`.
Projection can be offered as a named policy mode, but it must preserve confidence and orphan/ambiguity diagnostics.

Query cache inputs must include review refs, claim/key/member refs, rule ref state, and any Git/anchor state used for projection.
A revocation, key association change, new pass, or rule change must invalidate a previously derived approval.

## API and UX proposal

### Library API

Add explicit capture and pass operations while retaining synchronous `&gix::Repository` boundaries:

```rust
Review::create_with_scope_as(repo, auth, draft, capture) -> Result<ReviewId>
Review::scope_at(repo, review, revision) -> Result<ProjectedScope>
ReviewPass::publish_as(repo, auth, review_id, verdict, summary) -> Result<ClaimId>
ReviewPass::revoke_as(repo, auth, claim_id, reason) -> Result<ClaimId>
```

Publishing must atomically stage the typed payload, create/sign the attest envelope, append it with CAS, and return only after the claim is authoritative.
A failed step must not report a created pass.

Suggested capabilities are `ReviewPassCreate` and `ReviewPassRevoke`; pass creation is not review-document ownership.
The signing key must be configured and associated with the authenticated member.

### CLI and UI

Extend review creation to capture a scope from an explicit revision and one or more typed selectors.
Preserve current target syntax as a compatibility input, but display the canonical scope kind and projection state.

Review detail should show:

- captured revision and scope items;
- exact/rebound/orphaned/ambiguous status;
- each pass, signer member, verdict, verification status, and revocation status;
- the policy explanation for whether the review is currently `reviewed`;
- comments separately, including their existing anchored locations.

“Approve” and “Request changes” must be distinct signed actions, not mutations of `Review.status`.
The UI should never display a pass as trusted solely because a signer string was supplied by the client.

## Schema and migration plan

1. **Stabilize scope identity.**
   Define `review-scope-v1`, canonical ordering, path normalization, OID validation, range semantics, and golden target-key tests.
   Keep existing `ReviewTarget` readable through a compatibility adapter.
2. **Add inline scope bindings.**
   Extend `StoredReview` additively with an optional versioned scope/binding field.
   Old reviews remain readable and are shown as legacy, exact-object targets without rebound semantics until recaptured.
3. **Add the `review-pass` payload schema.**
   Register it in forge and add the attest dependency/integration.
   Do not make merge gates depend on it yet.
4. **Wire query facts and rules.**
   Decode verified passes, associate keys to members, define revocation/as-of policy, and expose explainable exact/projected results.
5. **Enable policy enforcement.**
   Update the versioned `reviewed`/`mergeable` rules and only then connect them to `git-effect` gates.
6. **Offer recapture/rebind.**
   Never rewrite old bindings or target keys.
   A new capture or manual pin creates a new authoritative record/claim and leaves the original audit trail intact.

Changing target identity, range ordering, path inclusion, or OID normalization requires `review-scope-v2`; existing attest claims must retain their v1 meaning forever.

## Testing strategy

### Store and anchor

- Round-trip every scope variant through the published schema.
- Verify old review documents remain readable and retain their ids.
- Verify bindings are inline and no anchor foreign key is required.
- Capture at a known revision, project after edits, and distinguish exact, rebound, orphaned, and ambiguous outcomes.
- Verify canonical ordering and deterministic scope keys.
- Reject invalid object types, missing objects, malformed paths, invalid ranges, and ambiguous target descriptors.

### Attest and identity

- Valid signed passes become facts; bad or missing signatures do not.
- A pass from an unassociated key does not count as a member pass.
- Self-approval follows the configured policy.
- Revocation removes a pass from effective policy without deleting its history.
- Key rotation and as-of validity produce the documented result.
- Concurrent pass writers preserve both claims through CAS chaining.
- Payload kind is exactly `review-pass`; request documents cannot be decoded as passes.

### Query and end-to-end

- Exact approvals satisfy the selected one- and multi-reviewer policies.
- Request-changes blocks a policy when configured to do so.
- Duplicate passes by one member do not satisfy a distinct-reviewer quorum.
- A changed exact target invalidates the old approval.
- A projected approval reports confidence and never silently becomes exact.
- Claim, key, member, review, anchor, and rule changes invalidate cached results.

The primary integration fixture should create a review, have an eligible second member publish a signed pass, verify `approval`/`reviewed`, revoke the pass, and verify the derived result disappears.
It should then change the target and demonstrate that exact approval does not carry forward.

## Open decisions

Before implementation, the following must be recorded as protocol decisions rather than left to individual callers:

- Whether multi-item scopes are sets or ordered sequences.
- Whether blob path is identity or display-only metadata.
- Whether a tree means a root tree, a subtree, or both.
- Commit-range ancestry, endpoint inclusion, and merge traversal rules.
- Whether projected approvals are supported initially or require a manual pin.
- Member-to-key binding and key revocation semantics.
- Whether `ReviewPass.summary` is included in the signed payload or stored only as discussion.
- Which repository policies define eligibility, quorum, self-approval, and stale approvals.

Resolving these decisions first prevents the most dangerous failure modes: treating mutable workflow state as signed authority, carrying an approval across a changed target, or allowing two consumers to derive different identities from the same review scope.
