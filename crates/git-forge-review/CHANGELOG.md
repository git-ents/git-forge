# Changelog

## 0.0.1 (2026-03-19)


### Features

* `Replaces:` trailer for non-destructive edits ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Add --fetch flag ([3c0b008](https://github.com/git-ents/git-forge/commit/3c0b0088537b2e8b49e63a60cf65d37a5875ba19))
* Add --no-fetch flag ([3c0b008](https://github.com/git-ents/git-forge/commit/3c0b0088537b2e8b49e63a60cf65d37a5875ba19))
* Add --no-push flag and fetch/push forge refs on mutations ([e5b43e0](https://github.com/git-ents/git-forge/commit/e5b43e081ca19fab840b389e803aee617f80a98b))
* Add `git forge comment view` subcommand ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Add `git forge comment` (new, reply, view, edit) ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Add `git forge comment` subcommand with full CRUD and anchor support ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Add criterion benchmarks to each library crate ([cedc2dd](https://github.com/git-ents/git-forge/commit/cedc2dd2164c6991f23522c519f72ac112754825))
* Allow commenting on any git object via &lt;kind&gt;/&lt;id&gt; target ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Anchor comments to blobs with union line ranges (comma-separated) ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Auto-open editor when stdin is a TTY (drops explicit --interactive) ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Default comment target to `commit/<HEAD>` ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Expand comment CLI and unify refs under refs/forge/ ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Infer anchor type from git object kind; resolve paths via HEAD tree ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Integrate comments into issues via `Issues::add_issue_comment` ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Issue assign &lt;id&gt; --add/--remove ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue close &lt;id&gt; ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue comment &lt;id&gt; [body] ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue label &lt;id&gt; --add/--remove ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue reopen &lt;id&gt; ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Move issue/review refs under `refs/forge/` ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Move refs/issue/ to refs/forge/issue/ ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Move refs/meta/reviews/ to refs/forge/review/ ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Release prepare/publish ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Rename `comment add` to `comment new` ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Review approve/reject/merge/close/comment ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Separate --no-fetch flag from --no-push ([3c0b008](https://github.com/git-ents/git-forge/commit/3c0b0088537b2e8b49e63a60cf65d37a5875ba19))


### Bug Fixes

* Resolve clippy warnings (format_push_string, resolve_editor, let-else) ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Validate line ranges in `build_anchor`; remove COMMENT_EDITMSG on success ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))


### Miscellaneous Chores

* Trigger initial release ([b199406](https://github.com/git-ents/git-forge/commit/b19940683fde969fd3f429145e284570eec7f054))
