# Changelog

## 0.0.1 (2026-03-15)


### Features

* Add args to IssueCommand variants (title, body, label, assignee, state, id) ([31b2b87](https://github.com/git-ents/git-forge/commit/31b2b8746f9a1a08021611ef847bf1df2995f016))
* Add criterion benchmarks to each library crate ([cedc2dd](https://github.com/git-ents/git-forge/commit/cedc2dd2164c6991f23522c519f72ac112754825))
* Add IssueOpts for customizable ref prefix ([b860f06](https://github.com/git-ents/git-forge/commit/b860f06bbd5e4bc11c31e7caf7ca3fb5f153db7c))
* Add StateArg value enum for open/closed filtering ([31b2b87](https://github.com/git-ents/git-forge/commit/31b2b8746f9a1a08021611ef847bf1df2995f016))
* Add tests for Issues trait (create, find, list, list_by_state) ([3d7f773](https://github.com/git-ents/git-forge/commit/3d7f773dae5ec11c621cef10b648ced91b578378))
* Implement create_issue for git2::Repository ([0d55408](https://github.com/git-ents/git-forge/commit/0d554081b043e345f25cea13a3095d4666e7d215))
* Implement find_issue for git2::Repository ([670ec2d](https://github.com/git-ents/git-forge/commit/670ec2d607aede942272bfc4588f06cb06e79228))
* Implement issue cli and exe dispatch ([31b2b87](https://github.com/git-ents/git-forge/commit/31b2b8746f9a1a08021611ef847bf1df2995f016))
* Implement list_issues for git2::Repository ([b330f02](https://github.com/git-ents/git-forge/commit/b330f0243df6bc4b1c7c2a1ea0f0c74cb9252bd9))
* Implement list_issues_by_state for git2::Repository ([681dd8a](https://github.com/git-ents/git-forge/commit/681dd8a2b77b77e1a9e4e988315cf7ad9079c03f))
* Implement run_inner dispatching to Issues trait methods ([31b2b87](https://github.com/git-ents/git-forge/commit/31b2b8746f9a1a08021611ef847bf1df2995f016))
* Implement update_issue method for git2 repository backend ([cc9a4a8](https://github.com/git-ents/git-forge/commit/cc9a4a81b0c021e48c29c339a63f6e09f0dd6064))


### Bug Fixes

* Stub read_comments to return empty vec instead of todo!() ([3d7f773](https://github.com/git-ents/git-forge/commit/3d7f773dae5ec11c621cef10b648ced91b578378))


### Miscellaneous Chores

* Trigger initial release ([b199406](https://github.com/git-ents/git-forge/commit/b19940683fde969fd3f429145e284570eec7f054))
