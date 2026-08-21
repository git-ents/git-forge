# Phase 3 — Visual markers, states, and affordances

Audit the visual presentation of `git-forge-ui`, a server-rendered HTML app in this Rust workspace.
This is the only phase that spends tokens on images.

Requires Phase 0's capture output under `target/ui-audit/<run>/`.
Read `docs/ui-audit/findings.md` first so you do not re-report what Phases 1 and 2 already recorded.

## Budget

Start from `target/ui-audit/<run>/index.json` and the stripped `page.html` files to decide *which* screenshots are worth opening.
Then open only those — target 10–15 images total, both themes represented.
Each screenshot costs roughly 1–2k tokens; reading all 60 captures will exhaust the context before the audit finishes.

## References to measure against

- `Git Forge v5.dc.html` at the repo root is the v5 design comp.
  Note it is untracked — `*.html` is gitignored — so confirm it is present before relying on it, and say so if it is missing.
- The layout shell (icon rail, top bar, split-pane grid) described in `docs/design-git-forge-ui.adoc`.
  Use it for *layout structure only*: the document is v1.0 and stale on routes, scope, and page inventory.

Findings phrased as divergence from those references are worth more than unanchored aesthetic opinion.

## Look for

1. **Visual markers** — icons, badges, status dots, avatars: alignment with
   adjacent text baselines, consistent sizing, consistent meaning for a given
   marker across pages.
2. **State coverage** per route — empty, error, permission-denied, overflow.
   The app renders server-side and ships no client JavaScript, so there is no loading state; check instead that each *rendered* state has a designed appearance rather than falling through to unstyled markup.
3. **Interactive affordances** — hover, focus, active, disabled.
   Keyboard focus rings especially: they are easy to lose to a CSS reset.
4. **Alignment and hierarchy** in the shell chrome.
5. **Dark mode parity** — contrast failures and hardcoded colors that ignore the theme.
   Theme is driven by a `data-theme` attribute on `:root` (`assets/style.css:124`, `:162`), so a hardcoded color is one that appears outside those blocks.

## Ground rules

- **Findings go in `docs/ui-audit/findings.md`**, appended the moment you find something.
  Never leave a finding only in conversation.
- **Only report what you can point at** — every finding needs a screenshot path
  or a `file:line`.
- **Do not change code.**
  Finding and fixing are separate jobs.
- **Keep taste separate from inconsistency.**
  Mark aesthetic judgments `severity: low`; the fix pass acts on the objective ones.
- Captures are Chromium 151 only.
  Nothing here says anything about Safari.

## Finding format

```markdown
### F-021 — Status dot sits above the text baseline on issue rows

- **route:** /issues
- **source:** crates/git-forge-ui/src/pages/issues.rs:298
- **evidence:** target/ui-audit/<run>/light/issues/page.png — dot is 3px high
  relative to the adjacent title; the same marker aligns correctly on /reviews
- **phase:** 3
- **severity:** medium
- **proposal:** match the vertical-align treatment used on the reviews list
- **status:** open
```

`severity` is `high` (broken or misleading), `medium` (inconsistent with the app's own system), or `low` (aesthetic judgment).
`status` is `open`, `fixed`, `rejected`, or `needs-recapture`.
