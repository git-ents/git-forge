# Changelog

## 0.0.1-alpha.1 (2026-04-03)


### Features

* Add approved_oids() Store method; update coverage to use it ([8b984df](https://github.com/git-mirdain/forge/commit/8b984df018dddb7cb74c0e9a9732cc14668e07d5))
* Add build_comment_tree (body/anchor/context/anchor-content) ([00b96f3](https://github.com/git-mirdain/forge/commit/00b96f3cd218043c2e64e26f8f302ce535d540b5))
* Add comment support ([bec6db0](https://github.com/git-mirdain/forge/commit/bec6db0b60c11788eb60d94c4cf4884ef07d72fe))
* Add comment support ([d1dc0ff](https://github.com/git-mirdain/forge/commit/d1dc0ff3f50417b0f3e3f159dc15919057075a0c))
* Add comment_thread_ref, Comment-Id trailer key ([00b96f3](https://github.com/git-mirdain/forge/commit/00b96f3cd218043c2e64e26f8f302ce535d540b5))
* Add comments to LSP and MCP ([bec6db0](https://github.com/git-mirdain/forge/commit/bec6db0b60c11788eb60d94c4cf4884ef07d72fe))
* Add context_lines and thread_id fields to Comment ([00b96f3](https://github.com/git-mirdain/forge/commit/00b96f3cd218043c2e64e26f8f302ce535d540b5))
* Add contributor configuration support ([220079b](https://github.com/git-mirdain/forge/commit/220079b12c448b0b31c34e4cfe8bc918721fea67))
* Add create_issue tool to forge-mcp ([b58ec44](https://github.com/git-mirdain/forge/commit/b58ec446b2eab7ccc1128c41cae7959c38cb2414))
* Add create_issue, update_issue, and update_review MCP tools ([b58ec44](https://github.com/git-mirdain/forge/commit/b58ec446b2eab7ccc1128c41cae7959c38cb2414))
* Add create_review and approve_review MCP tools ([8b984df](https://github.com/git-mirdain/forge/commit/8b984df018dddb7cb74c0e9a9732cc14668e07d5))
* Add create_thread, reply_to_thread, resolve_thread, edit_in_thread ([00b96f3](https://github.com/git-mirdain/forge/commit/00b96f3cd218043c2e64e26f8f302ce535d540b5))
* Add find_threads_by_object, rebuild_comments_index, index_lookup ([00b96f3](https://github.com/git-mirdain/forge/commit/00b96f3cd218043c2e64e26f8f302ce535d540b5))
* Add forge review merge subcommand ([8b984df](https://github.com/git-mirdain/forge/commit/8b984df018dddb7cb74c0e9a9732cc14668e07d5))
* Add forge-mcp crate scaffolding ([1e3e310](https://github.com/git-mirdain/forge/commit/1e3e31090153978505cb159d41a41e444eb9dcb4))
* Add forge-nvim ([220079b](https://github.com/git-mirdain/forge/commit/220079b12c448b0b31c34e4cfe8bc918721fea67))
* Add git-data dependencies ([9a64d34](https://github.com/git-mirdain/forge/commit/9a64d3422f1abf7d73a890f3b8a90ede662196f2))
* Add list_issue_comments MCP tool (Phase 2.3) ([398c6c9](https://github.com/git-mirdain/forge/commit/398c6c98f4dd732d8564f5b6d4a04e3874d05303))
* Add list_issues and get_issue MCP tools ([c7ce8d2](https://github.com/git-mirdain/forge/commit/c7ce8d227375c392e9821e1fe84464c36b5bd5de))
* Add list_thread_comments, list_all_thread_ids ([00b96f3](https://github.com/git-mirdain/forge/commit/00b96f3cd218043c2e64e26f8f302ce535d540b5))
* Add MCP issue tools to forge-mcp ([c7ce8d2](https://github.com/git-mirdain/forge/commit/c7ce8d227375c392e9821e1fe84464c36b5bd5de))
* Add MCP prompts to forge-mcp for slash command support ([ddaa0eb](https://github.com/git-mirdain/forge/commit/ddaa0eb43707bc9bc8f1cf0bea110a6506cfd623))
* Add MCP read tools for reviews and review comments ([0c5248f](https://github.com/git-mirdain/forge/commit/0c5248fa4b0a247b3b709e0b2a6bd9d112c895ac))
* Add path argument to forge review approve for single-object approval ([8b984df](https://github.com/git-mirdain/forge/commit/8b984df018dddb7cb74c0e9a9732cc14668e07d5))
* Add review support ([220079b](https://github.com/git-mirdain/forge/commit/220079b12c448b0b31c34e4cfe8bc918721fea67))
* Add update_issue tool to forge-mcp ([b58ec44](https://github.com/git-mirdain/forge/commit/b58ec446b2eab7ccc1128c41cae7959c38cb2414))
* Add update_review tool to forge-mcp ([b58ec44](https://github.com/git-mirdain/forge/commit/b58ec446b2eab7ccc1128c41cae7959c38cb2414))
* Add v2 comment thread API ([00b96f3](https://github.com/git-mirdain/forge/commit/00b96f3cd218043c2e64e26f8f302ce535d540b5))
* Add Zed extension ([aa48243](https://github.com/git-mirdain/forge/commit/aa48243b6fab5303173c2e4c016b086a742860b6))
* Approved_oids scan, review merge command, path-based approve, MCP review tools ([8b984df](https://github.com/git-mirdain/forge/commit/8b984df018dddb7cb74c0e9a9732cc14668e07d5))
* Phase 9 — replace MCP comment tools with v2 API ([7375a96](https://github.com/git-mirdain/forge/commit/7375a964db4d82070eaf77519e9511702b74afc5))
* Refactor contributors ([946cd63](https://github.com/git-mirdain/forge/commit/946cd63529a87a02cb8b9c90029056fbe223b3ab))
* Refactor reviews ([946cd63](https://github.com/git-mirdain/forge/commit/946cd63529a87a02cb8b9c90029056fbe223b3ab))


### Bug Fixes

* Treat empty state arg as absent in list-issues prompt ([21925e0](https://github.com/git-mirdain/forge/commit/21925e012f53ad7c95e8cbc18e837d067df9f225))


### Miscellaneous Chores

* Re-trigger release ([9d1e435](https://github.com/git-mirdain/forge/commit/9d1e435d2ab896d4f610be560e2969ce88245278))
* Trigger initial release ([b199406](https://github.com/git-mirdain/forge/commit/b19940683fde969fd3f429145e284570eec7f054))
