# Phase 5 — Regression guard (optional)

Lock in the Phase 4 fixes so a future change cannot silently undo them.

The specific risk: the prose double-spacing bug lives in `crates/git-forge-ui/src/render.rs` or the vendored `vendor/acdc/` AsciiDoc parser.
A vendored-dependency bump can reintroduce it with no local diff to review.

## Do this

`crates/git-forge-ui/tests/ui.rs` already builds a fixture repo, spawns the server, and crawls every internal route (`ui_crawls_every_internal_route`).
Extend it rather than starting a new test harness.

Add a snapshot test over the prose-rendering routes — the `.adoc` and `.md` paths under `/tree`, plus issue and review detail, which render comment bodies through the same path.
Snapshot the rendered HTML of the prose container only, not the whole page: the shell chrome changes for unrelated reasons and would make the snapshot noisy enough that people stop reading the diff.

Normalize before comparing: strip commit hashes, timestamps, and generated ids, so the test fails for markup-structure changes and nothing else.

Check whether the workspace already has a snapshot convention (`insta`, or committed expected-output files) and follow it.
Do not add a new snapshot dependency if one is already in use.

## Done when

`cargo test -p git-forge-ui` passes, and deliberately reverting one Phase 4 render fix makes the new test fail.
Verify that second part — a snapshot test that cannot fail is worse than no test, because it reads as coverage.
