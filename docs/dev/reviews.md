# Reviews: Development Plan

## Status

Types and trait defined.
All `git2` impl methods and `exe.rs` dispatch are `todo!()`.
Reference implementation: `git-forge-issue`.

---

## Ref Layout

```text
refs/forge/review/<review-id>  →  commit  →  tree
├── meta          plain text: author, target_branch, state, created
├── title         plain text: single-line title
├── description   markdown
└── revisions/
    ├── 001       plain text: head_commit, timestamp
    ├── 002
    └── ...
```

Each mutation is a new commit on the review's ref.
The commit history is the audit log.

### `meta` format

```text
author = <fingerprint>
target_branch = refs/heads/main
state = open
created = 2026-01-01T00:00:00Z
```

Plain key = value, one per line, no quotes.
Parse with `line.split_once('=')` and trim.

### Revision blob format

```text
head_commit = <oid>
timestamp = 2026-01-01T00:00:00Z
```

---

## Step 1 — `Cargo.toml`

Add missing dependencies (mirror `git-forge-issue`):

```toml
[dependencies]
git-forge-comment = { workspace = true }
git-forge-core    = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

---

## Step 2 — `reviews.rs`: add `title` and `ReviewOpts`

`cli.rs` already has `--title` on `Edit`.
Add `title: String` to `ReviewMeta` so it is stored and round-tripped.

Add `ReviewOpts` parallel to `IssueOpts`:

```rust
pub struct ReviewOpts {
    pub ref_prefix: String,
}
impl Default for ReviewOpts {
    fn default() -> Self {
        Self { ref_prefix: REVIEWS_REF_PREFIX.to_string() }
    }
}
```

Update all trait method signatures to accept `Option<&ReviewOpts>`.

---

## Step 3 — `reviews/git2.rs`: implement `Reviews for Repository`

### Private helpers

```rust
fn blob_content<'r>(repo: &'r Repository, tree: &git2::Tree<'r>, name: &str)
    -> Result<Option<String>, git2::Error>
```

Same as issue crate.

```rust
fn parse_kv(content: &str) -> HashMap<&str, &str>
```

Split each line on `" = "`, collect into a map.

```rust
fn read_revisions(repo: &Repository, tree: &git2::Tree<'_>)
    -> Result<Vec<Revision>, git2::Error>
```

Get `revisions/` subtree.
Iterate entries in name order.
For each, read blob, parse `head_commit` and `timestamp`.
Index is the entry name (e.g. `"001"`).

```rust
fn review_from_ref(repo: &Repository, reference: &git2::Reference<'_>, prefix: &str)
    -> Result<Option<Review>, git2::Error>
```

Same shape as `issue_from_ref`.
Strip prefix, parse numeric ID, peel to commit, read tree, load all fields.

### Methods

**`list_reviews`** — glob `{prefix}*`, collect via `review_from_ref`, sort by ID.

**`list_reviews_by_state`** — same glob, read `meta` blob and check `state =` before full load to avoid unnecessary work.

**`find_review`** — `find_reference(&format!("{prefix}{id}"))`, delegate to `review_from_ref`.

**`create_review`**

1. Scan existing refs for `max_id`, take `max_id + 1`.
2. Author from `repo.signature()?.name()`.
3. `created` / `timestamp` from `std::time::SystemTime::now()`, formatted as RFC 3339.
   Implement a small `fn now_rfc3339() -> String` helper using only `std`.
4. Build `meta` blob, `title` blob, `description` blob.
5. Build `revisions/001` blob.
6. Build `revisions/` subtree; build root tree.
7. Commit to `{prefix}{id}` with no parents, message `"create review {id}"`.
8. Return `id`.

**`update_review`**

1. `find_review(id)`; error if not found.
2. Apply optional `title`, `description`, `state`.
3. Rebuild tree; commit with parent = current ref tip.
4. Message: `"update review {id}"`.

**`add_revision`**

1. `find_review(id)`; error if not found.
2. Next index = `revisions.len() + 1`, zero-padded to 3 digits.
3. Build new revision blob.
4. Read current `revisions/` subtree from the existing commit; insert new entry.
5. Rebuild root tree preserving `meta`, `title`, `description`.
6. Commit with parent = current tip.
   Message: `"revision {index} for review {id}"`.

**`revision_range`**

1. Get `head_commit` OID from `review.revisions[revision_index]`.
2. Resolve `review.meta.target_branch` ref to a commit OID.
3. `repo.merge_base(target_oid, head_oid)` → base OID.
4. Return `(base, head)`.

---

## Step 4 — `exe.rs`

Follow `git-forge-issue/src/exe.rs` exactly.
Use an `Executor(git2::Repository)` struct.

Editor template format (same convention as issues — title on first line, blank line, body):

```text
My review title

Describe what this review covers.
```

Parse with the same `parse_issue_template` approach: first line = title, rest after blank = description.

### Commands

| Command | Action |
|---|---|
| `New { target }` | Resolve target branch (default: `refs/heads/main`). Get HEAD. Open editor for title + description. Call `create_review`. Print `"Created review #{id}"`. |
| `Show { id, oneline }` | `find_review`. Print title, state, author, target, created, revisions list, then description. `--oneline`: `"#{id} [{state}] {title}"`. |
| `List { state }` | `list_reviews_by_state` or `list_reviews`. One line per review. |
| `Edit { id, title, description }` | If flags given, call `update_review` directly. Otherwise open editor pre-filled with current title + description. |
| `Approve { id }` | Stub: `eprintln!("approve: not yet implemented")` + `process::exit(1)`. Needs `git-metadata` integration. |
| `Reject { id }` | Same stub. |
| `Merge { id }` | `update_review(id, None, None, Some(ReviewState::Merged))`. Print `"Merged review #{id}"`. |
| `Close { id }` | `update_review(id, None, None, Some(ReviewState::Closed))`. Print `"Closed review #{id}"`. |
| `Comment { id, body }` | Resolve body (arg, editor, or stdin). Anchor = `Commit(latest_revision.head_commit)`. Call `repo.add_comment(&review_comments_ref(id), &anchor, &body)`. |

---

## Step 5 — Tests (`src/tests/`)

Add `#[cfg(test)] mod tests;` to `lib.rs`.
Create `src/tests.rs` as module file.

Use the same `fn repo() -> (TempDir, Repository)` fixture:

```rust
fn repo() -> (TempDir, Repository) {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    {
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
        drop(cfg);
        let sig = repo.signature().unwrap();
        let tree_oid = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();
    }
    (dir, repo)
}
```

`revision_range` tests require a repo with an actual commit graph (two branches sharing a base).

### `tests/new_review.rs`

- `assigns_id_one_for_first`
- `assigns_sequential_ids`
- `stores_title_target_branch_description`
- `new_review_is_open`
- `stores_first_revision`
- `custom_ref_prefix`

### `tests/find_review.rs`

- `returns_none_for_missing`
- `returns_none_for_nonexistent_id`
- `finds_by_id`
- `finds_correct_among_many`
- `respects_custom_ref_prefix`

### `tests/list_reviews.rs`

- `returns_all_sorted_by_id`
- `list_by_state_open`
- `list_by_state_merged`

### `tests/update_review.rs`

- `updates_title`
- `updates_description`
- `updates_state_to_closed`
- `updates_state_to_merged`
- `update_preserves_unset_fields`

### `tests/add_revision.rs`

- `adds_second_revision`
- `revision_indices_are_sequential_and_zero_padded`
- `revision_stores_head_commit`

### `tests/revision_range.rs`

- `returns_merge_base_as_base`
- `returns_head_commit_as_tip`

---

## Execution Order

1. `Cargo.toml` — add deps
2. `reviews.rs` — add `title` to `ReviewMeta`, add `ReviewOpts`, update trait signatures
3. `reviews/git2.rs` — implement all methods
4. `src/tests/` — write tests; run `cargo test -p git-forge-review`
5. `exe.rs` — implement all commands
6. `cargo clippy -p git-forge-review` — fix warnings
7. Commit
