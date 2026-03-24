# 💬 `git-forge-comment`

*Code, review, and issue comments anchored to Git objects.*

> [!CAUTION]
> This project is in active development and has not yet been published to crates.io.
> Please file a [new issue] for any misbehaviors you find!

[new issue]: https://github.com/git-ents/git-forge/issues/new

## Overview

This crate implements comments for the `git-forge` workspace.
Comments are stored as Git commit objects under `refs/forge/comments/`, with each comment anchored to a specific Git object via an `Anchor`:

- **Blob** — a comment on a specific file at a specific revision.
- **Commit** — a comment on a single commit.
- **Tree** — a comment on a tree (e.g. a project snapshot).
- **CommitRange** — a comment spanning a range of commits.

The `Comments` trait provides the full lifecycle: listing, finding, adding, replying, resolving, and editing comments.
Issue-scoped and review-scoped comments are namespaced under `refs/forge/comments/issues/<id>` and `refs/forge/comments/reviews/<id>` respectively.

## Example

```rust
use git2::Repository;
use git_forge_comment::{Anchor, Comments};

let repo = Repository::open(".")?;
let commit = repo.head()?.peel_to_commit()?;

// Add a comment anchored to the HEAD commit.
let anchor = Anchor::Commit(commit.id());
repo.add_comment(&anchor, "Looks good to me.", None)?;

// List all comments on HEAD.
for comment in repo.comments_on(&anchor)? {
    println!("{}: {}", comment.author, comment.body);
}
```

## Installation

```shell
cargo add --git https://github.com/git-ents/git-forge.git git-forge-comment
```
