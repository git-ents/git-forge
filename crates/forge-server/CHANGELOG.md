# Changelog

## 0.0.1-alpha.1 (2026-04-03)


### Features

* Add comment support ([bec6db0](https://github.com/git-mirdain/forge/commit/bec6db0b60c11788eb60d94c4cf4884ef07d72fe))
* Add comments to LSP and MCP ([bec6db0](https://github.com/git-mirdain/forge/commit/bec6db0b60c11788eb60d94c4cf4884ef07d72fe))
* Add contributor configuration support ([220079b](https://github.com/git-mirdain/forge/commit/220079b12c448b0b31c34e4cfe8bc918721fea67))
* Add discover_github_configs to enumerate configured remotes ([86911da](https://github.com/git-mirdain/forge/commit/86911daea4eb3851789125529ec3867961f998de))
* Add forge-nvim ([220079b](https://github.com/git-mirdain/forge/commit/220079b12c448b0b31c34e4cfe8bc918721fea67))
* Add GitHubAdapter implementing RemoteSync in forge-github ([86911da](https://github.com/git-mirdain/forge/commit/86911daea4eb3851789125529ec3867961f998de))
* Add refs/forge/index/comments-by-comment (comment OID → thread UUID) ([820bfb1](https://github.com/git-mirdain/forge/commit/820bfb19725891308bd433b3a6f8fc35dff6df0d))
* Add RemoteSync trait and SyncReport to git-forge ([86911da](https://github.com/git-mirdain/forge/commit/86911daea4eb3851789125529ec3867961f998de))
* Add review support ([220079b](https://github.com/git-mirdain/forge/commit/220079b12c448b0b31c34e4cfe8bc918721fea67))
* Add SyncReport.unexportable field ([2426f11](https://github.com/git-mirdain/forge/commit/2426f11a075581cde58487f2e1d55abce6c1ad82))
* Add Zed extension ([aa48243](https://github.com/git-mirdain/forge/commit/aa48243b6fab5303173c2e4c016b086a742860b6))
* Extend forge-server sync loop for reviews and comments ([04fd7b7](https://github.com/git-mirdain/forge/commit/04fd7b72c843e3de208c5e53357b854bc9ac2df2))
* Forge-server rebuilds the full index after each sync pass ([820bfb1](https://github.com/git-mirdain/forge/commit/820bfb19725891308bd433b3a6f8fc35dff6df0d))
* Implement forge-server sync daemon (step 1.9) ([86911da](https://github.com/git-mirdain/forge/commit/86911daea4eb3851789125529ec3867961f998de))
* Remove thread ID from comment subcommands; incremental index writes ([820bfb1](https://github.com/git-mirdain/forge/commit/820bfb19725891308bd433b3a6f8fc35dff6df0d))
* Scaffold forge-server crate with sync loop ([86911da](https://github.com/git-mirdain/forge/commit/86911daea4eb3851789125529ec3867961f998de))
* Update both indexes incrementally on every comment write ([820bfb1](https://github.com/git-mirdain/forge/commit/820bfb19725891308bd433b3a6f8fc35dff6df0d))


### Bug Fixes

* Show full error chain in sync and CLI error messages ([f1b3946](https://github.com/git-mirdain/forge/commit/f1b3946cc735a0b7e36a5a713d69a1b1017c29bc))
* Skip unexportable reviews instead of sending raw OIDs to GitHub ([2426f11](https://github.com/git-mirdain/forge/commit/2426f11a075581cde58487f2e1d55abce6c1ad82))


### Miscellaneous Chores

* Re-trigger release ([9d1e435](https://github.com/git-mirdain/forge/commit/9d1e435d2ab896d4f610be560e2969ce88245278))
* Trigger initial release ([b199406](https://github.com/git-mirdain/forge/commit/b19940683fde969fd3f429145e284570eec7f054))
