# Phase 4 — Fix pass

Apply the fixes recorded in `docs/ui-audit/findings.md` for `git-forge-ui`, a server-rendered HTML app in this Rust workspace.

## Do this

Work in severity order, highest first.
For each finding: make the change, mark it `status: fixed` in `findings.md` with a one-line note on what you did, and move on.
If a finding turns out to be wrong or not worth fixing, mark it `status: rejected` with the reason rather than silently skipping it.

Rules:

- **Fix causes, not symptoms** — particularly for the prose-spacing finding.
  If the fix is a compensating margin override, you have not found the cause.
- **Findings must be self-contained.**
  Do not open the capture corpus under `target/ui-audit/` to reconstruct what a finding meant; if a finding is not actionable on its own, mark it `status: needs-recapture` and continue. (You *will* read captures at the end, for the diff — that is different from using them as input to a judgment.)
- Skip `severity: low` aesthetic findings unless one is specifically called out
  as wanted.
- Run `cargo test -p git-forge-ui` and `cargo clippy -p git-forge-ui` when
  done.

## Then verify

Re-run the capture harness with a **new run id**:

```text
node scripts/ui-audit/capture.mjs --run post-fix
```

This must not overwrite the pre-fix run — the diff depends on both existing side by side under `target/ui-audit/`.
Diff the new `styles.json` files against the pre-fix ones to confirm the spacing changes landed and that nothing else moved unintentionally.
Report the diff summary: what changed, and whether any change was unexpected.

An unexpected diff on a route you did not touch is itself a finding — append it to `findings.md` as `phase: 4` rather than leaving it in the report only.
