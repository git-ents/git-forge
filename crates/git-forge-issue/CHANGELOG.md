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
* Allow commenting on any git object via &lt;kind&gt;/&lt;id&gt; target ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Anchor comments to blobs with union line ranges (comma-separated) ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Auto-detect TTY via std::io::IsTerminal for editor dispatch ([6b27821](https://github.com/git-ents/git-forge/commit/6b2782192631d91f581c7eb51dac0f2dbe060116))
* Auto-open editor when stdin is a TTY ([6b27821](https://github.com/git-ents/git-forge/commit/6b2782192631d91f581c7eb51dac0f2dbe060116))
* Auto-open editor when stdin is a TTY (drops explicit --interactive) ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Default comment target to `commit/<HEAD>` ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Expand comment CLI and unify refs under refs/forge/ ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Implement add_issue_comment via Comments trait ([982985d](https://github.com/git-ents/git-forge/commit/982985dfbcb75a3b182b91ba9f6a4f8afa68c67d))
* Infer anchor type from git object kind; resolve paths via HEAD tree ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Integrate comments into issues via `Issues::add_issue_comment` ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Integrate git-forge-comment into git-forge-issue ([982985d](https://github.com/git-ents/git-forge/commit/982985dfbcb75a3b182b91ba9f6a4f8afa68c67d))
* Issue assign &lt;id&gt; --add/--remove ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue close &lt;id&gt; ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue comment &lt;id&gt; [body] ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue label &lt;id&gt; --add/--remove ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Issue reopen &lt;id&gt; ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Move issue/review refs under `refs/forge/` ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Move refs/issue/ to refs/forge/issue/ ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Move refs/meta/reviews/ to refs/forge/review/ ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Populate Issue::comments from comments_on ([982985d](https://github.com/git-ents/git-forge/commit/982985dfbcb75a3b182b91ba9f6a4f8afa68c67d))
* Release prepare/publish ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Remove --interactive from comment new, comment reply, issue new ([6b27821](https://github.com/git-ents/git-forge/commit/6b2782192631d91f581c7eb51dac0f2dbe060116))
* Rename `comment add` to `comment new` ([ff66358](https://github.com/git-ents/git-forge/commit/ff6635809c240f8393685d73f0060f772ce4d97f))
* Review approve/reject/merge/close/comment ([bb82698](https://github.com/git-ents/git-forge/commit/bb826983c3abb0474d2134ea77531ddf92a1cb4a))
* Separate --no-fetch flag from --no-push ([3c0b008](https://github.com/git-ents/git-forge/commit/3c0b0088537b2e8b49e63a60cf65d37a5875ba19))
* Wire git2_credentials to fetch and push operations ([a17fa5d](https://github.com/git-ents/git-forge/commit/a17fa5df0836396c3380be55e94263e80fc3f0d6))


### Bug Fixes

* Correct import paths in issue tests ([6c2a746](https://github.com/git-ents/git-forge/commit/6c2a7466dfddef0e0dba1eab65df0d1a15463596))
* Correct typo in benchmarks ([35ea22c](https://github.com/git-ents/git-forge/commit/35ea22ca70ad24d6fff15bb2d5b4ffcc3b1ad268))
* Derive IssueMeta::author from commit author signature ([1623036](https://github.com/git-ents/git-forge/commit/162303689a8c298affd0faa83165976f09f685aa))
* Drop author blob from issue tree; read from commit signature ([1623036](https://github.com/git-ents/git-forge/commit/162303689a8c298affd0faa83165976f09f685aa))
* Remove + from FORGE_REFSPEC in contributor, comment, issue, install ([2e42cc7](https://github.com/git-ents/git-forge/commit/2e42cc7a5ac9014022a140abf5cb4bae22ccfb83))
* Remove author blob from issue tree on create and update ([1623036](https://github.com/git-ents/git-forge/commit/162303689a8c298affd0faa83165976f09f685aa))
* Remove force-fetch from forge refspecs ([2e42cc7](https://github.com/git-ents/git-forge/commit/2e42cc7a5ac9014022a140abf5cb4bae22ccfb83))
* Resolve all clippy warnings ([e56e372](https://github.com/git-ents/git-forge/commit/e56e3726bf88ee9377595212fe4493ac5ed1858f))
* Resolve clippy warnings (format_push_string, resolve_editor, let-else) ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))
* Set user.name/email in git-forge-issue unit test helpers ([51c3f8d](https://github.com/git-ents/git-forge/commit/51c3f8d4f7934cf04a1164cd73b0ff46128efc02))
* Validate line ranges in `build_anchor`; remove COMMENT_EDITMSG on success ([8e13aea](https://github.com/git-ents/git-forge/commit/8e13aea04ef011d6af2beb297be88ef73c78595d))


### Miscellaneous Chores

* Trigger initial release ([b199406](https://github.com/git-ents/git-forge/commit/b19940683fde969fd3f429145e284570eec7f054))
