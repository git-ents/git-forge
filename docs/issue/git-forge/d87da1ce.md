---
id: d87da1ce
repo: git-forge
ref: refs/forge/issue/d87da1ce
title: "The published schema describes a different type than the one actually stored, and is republished on every write"
status: open
labels: ["schema", "high"]
reporters: ["Claude:claude-opus-5"]
assignees: []
updated: 2026-07-28T14:54:52+09:00
revisions:
  - commit: d87da1ce10f7889406bc7bb37b52e7e661fe05d2
    date: 2026-07-28T10:39:11+09:00
  - commit: 36faac447b99b6f3523d11df6aabc58f656b2f65
    date: 2026-07-28T14:54:52+09:00
---

# The published schema describes a different type than the one actually stored, and is republished on every write

**What**

`ensure_schema` publishes `schema_of::<Issue>()` while `save` serializes
`StoredIssue`. These are different types: `Issue` has eight fields including a
required `id: String`; `StoredIssue` has seven and no `id`. So the schema
published at `refs/schema/issue` -- the public contract that makes the data
readable by a tool other than this one -- does not describe the bytes on disk.
Same pattern for `Review` and `StoredReview`.

Separately, both `create_in_repo` and `save_in_repo` call `ensure_schema`
unconditionally, and `put_schema` always commits forward, so a new schema
commit is minted on every single write.

**Where**

- `crates/gix-forge/src/lib.rs:100` -- `schema_of::<Issue>()`
- `crates/gix-forge/src/lib.rs:107` -- `to_value(&StoredIssue::from(self))`
- `crates/gix-forge/src/lib.rs:115` and `:138` -- `ensure_schema` on the write path
- `crates/gix-forge/src/lib.rs:320` and `:327` -- the same for `Review`

**Evidence**

Published schema field names, read straight out of the repo:

    refs/schema/issue:defs/Issue/Struct/0000/name  ->  id
    0001 status  0002 title  0003 body  0004 labels
    0005 assignees  0006 reporters  0007 edit

The stored tree for a real issue:

    100644 blob  body
    100644 blob  reporters/0000
    100644 blob  status
    100644 blob  title

No `id`. The schema promises a required field the data never contains.

Six issues created in sequence produced six commits on `refs/schema/issue`,
all with the identical tree `3603067857d7...`. So `schema_history` is a write
log, not an evolution log, and you cannot group data by the schema version it
was written under -- every data commit points at a different schema commit
even though the schema never changed.

**Why it matters**

Your data is not locked in a vendor database only holds if the format is
discoverable without this binary. Right now the discoverable description is
wrong, so a third-party reader that trusts it looks for `id` and fails. That
undercuts the emotional core of the pitch precisely where somebody technical
would poke at it.

The per-write republication compounds the transport fragility filed separately:
the set of schema commits a clone must have to read the data grows without
bound and changes on every write. Publish once and that failure mode nearly
disappears.

There is also no version marker anywhere in the record, and `status` is typed
as a bare `String` in the schema rather than an enum, so nothing published
tells a reader that `open` and `closed` are the only legal values. `edit` is in
the published contract but the CLI sets it to `None` on every path and wipes it
on every edit -- a dead field in a public interface.

**Options**

- Option A -- Publish `schema_of::<StoredIssue>()` and register it under the
  name `Issue`. One-line fix for the mismatch; does nothing for the other
  problems. Trade-off: the type whose name appears in the schema is private.
- Option B -- Keep the split but name the layering: `id` is derived from the
  genesis commit, so it cannot appear in that commit's own tree -- `StoredIssue`
  stays id-less, and identity is the address (the genesis commit oid, reachable
  at `refs/forge/issue/<id>`). `Issue { id, ..StoredIssue }` is hydration:
  `load` fills `id` from the ref, as it already does. A generic wrapper such as
  `Loaded<T> { id, value }` would generalize this to `Review` and every future
  kind for free. Trade-off: two types remain, but the published contract
  (`StoredIssue`'s schema) then exactly matches the disk.
- Option C -- Keep `ensure_schema` on the write path but make `put_schema` a
  no-op when the serialized tree is unchanged, so per-write republication stops
  without requiring an extra setup step. Add an explicit version field to the
  record. Trade-off: still needs a tree diff on every write, but it makes the
  schema ref mean something and needs no mandatory `install`.

**Recommendation**

B and C together. High confidence on both -- the `Issue` / `StoredIssue` split
has a legitimate cause (`id` is derived from the genesis commit, so it cannot
appear in that commit's own tree) and should be named rather than collapsed.
Publishing `StoredIssue`'s schema makes the published contract exactly match
the disk, which dissolves the mismatch rather than papering over it. This buys
an invariant worth a slide: every commit on an entity ref is a complete,
schema-valid value of its kind, and `git log -p` is a pure field-level audit
trail with no synthetic revisions, ever. The precedent is git itself: a commit
does not contain its own oid, and you never miss it, because everything that
hands you the object also hands you its name.


## Superseded revisions

Earlier versions of this issue, preserved because deleting the
ref would otherwise discard them.

### d87da1ce — 2026-07-28T10:39:11+09:00

- title: The published schema describes a different type than the one actually stored, and is republished on every write
- status: open
- labels: schema, high
- reporters: Claude:claude-opus-5

**What**

`ensure_schema` publishes `schema_of::<Issue>()` while `save` serializes
`StoredIssue`. These are different types: `Issue` has eight fields including a
required `id: String`; `StoredIssue` has seven and no `id`. So the schema
published at `refs/schema/issue` -- the public contract that makes the data
readable by a tool other than this one -- does not describe the bytes on disk.
Same pattern for `Review` and `StoredReview`.

Separately, both `create_in_repo` and `save_in_repo` call `ensure_schema`
unconditionally, and `put_schema` always commits forward, so a new schema
commit is minted on every single write.

**Where**

- `crates/gix-forge/src/lib.rs:100` -- `schema_of::<Issue>()`
- `crates/gix-forge/src/lib.rs:107` -- `to_value(&StoredIssue::from(self))`
- `crates/gix-forge/src/lib.rs:115` and `:138` -- `ensure_schema` on the write path
- `crates/gix-forge/src/lib.rs:320` and `:327` -- the same for `Review`

**Evidence**

Published schema field names, read straight out of the repo:

    refs/schema/issue:defs/Issue/Struct/0000/name  ->  id
    0001 status  0002 title  0003 body  0004 labels
    0005 assignees  0006 reporters  0007 edit

The stored tree for a real issue:

    100644 blob  body
    100644 blob  reporters/0000
    100644 blob  status
    100644 blob  title

No `id`. The schema promises a required field the data never contains.

Six issues created in sequence produced six commits on `refs/schema/issue`,
all with the identical tree `3603067857d7...`. So `schema_history` is a write
log, not an evolution log, and you cannot group data by the schema version it
was written under -- every data commit points at a different schema commit
even though the schema never changed.

**Why it matters**

Your data is not locked in a vendor database only holds if the format is
discoverable without this binary. Right now the discoverable description is
wrong, so a third-party reader that trusts it looks for `id` and fails. That
undercuts the emotional core of the pitch precisely where somebody technical
would poke at it.

The per-write republication compounds the transport fragility filed separately:
the set of schema commits a clone must have to read the data grows without
bound and changes on every write. Publish once and that failure mode nearly
disappears.

There is also no version marker anywhere in the record, and `status` is typed
as a bare `String` in the schema rather than an enum, so nothing published
tells a reader that `open` and `closed` are the only legal values. `edit` is in
the published contract but the CLI sets it to `None` on every path and wipes it
on every edit -- a dead field in a public interface.

**Options**

- Option A -- Publish `schema_of::<StoredIssue>()` and register it under the
  name `Issue`. One-line fix for the mismatch; does nothing for the other
  problems. Trade-off: the type whose name appears in the schema is private.
- Option B -- Drop the `Issue` / `StoredIssue` split entirely by making `id`
  non-stored (the ref name already carries it), so there is exactly one type
  and one schema. Trade-off: needs whatever `facet` offers for skipping a field.
- Option C -- Move `ensure_schema` off the write path into `install`, and make
  `put_schema` a no-op when the serialized tree is unchanged. Add an explicit
  version field to the record. Trade-off: `install` becomes mandatory, which is
  a real UX cost, but it makes the schema ref mean something.

**Recommendation**

B and C together. High confidence on both -- these are not judgement calls. The
duplicated-type pattern is the kind of thing that reads as an accident on a
slide, and this repo is the live proof that the `git-store` schema machinery is
pleasant to build on. Right now it proves the opposite: the machinery was easy
enough to call that it got called wrongly, on the hot path, without anything
catching it. That is worth saying out loud to the `git-store` authors, who are
you.
