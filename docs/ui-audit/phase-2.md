# Phase 2 — Spacing and typography audit

Audit the spacing and typography of `git-forge-ui`, a server-rendered HTML app in this Rust workspace.
This is where the known prose double-spacing bug gets root-caused.

Requires Phase 0's capture output under `target/ui-audit/<run>/`.
If it is not there, stop and say so rather than improvising captures.

## Inputs, in this order

1. `crates/git-forge-ui/assets/style.css` — read once, in full.
   It is 66KB.
   It is linked from `src/shell.rs:305`, not inlined, so captured pages reference it rather than copying it.
   **Never re-read it, and never read it per-page.**
2. The `styles.json` computed-style dumps under `target/ui-audit/<run>/`.
3. The stripped `page.html` files, only when you need to see structure.

Do not open `page.png` in this phase.
Computed-style tables answer spacing questions more precisely than screenshots, at a fraction of the token cost, and they diff cleanly across a fix.

## Do this

**First**, derive the implicit design system from `style.css`: the spacing scale, the type scale, the line-height ratios, the color roles.
Write it into `docs/ui-audit/findings.md` under a "Derived system" heading.
Everything after is measured against it, which turns taste into arithmetic.

**Then** investigate the known bug: rendered AsciiDoc and Markdown appear double-spaced.
The renderers are `crates/git-forge-ui/src/render.rs` and the vendored `vendor/acdc/` AsciiDoc parser.
The first hypothesis to test is that generated markup nests a block wrapper around a `<p>` and both carry margin, so every block is spaced twice — but verify that against the computed styles rather than assuming it.
Identify the actual mechanism and name the specific rule or emitted element responsible.

**Fix at the source.**
Do not propose tuning margins to compensate.
If the renderer emits a structure the stylesheet did not anticipate, the finding *is* that mismatch.
Adding override rules would bury a single wrong assumption under many patches.

**Then** sweep the computed-style dumps for:

- elements whose spacing does not sit on the derived scale,
- inconsistent heading rhythm across pages,
- line-height that varies for the same semantic element in different contexts.

Check both the light and dark captures.

## Ground rules

- **Findings go in `docs/ui-audit/findings.md`**, appended the moment you find something.
  Never leave a finding only in conversation.
- **Only report what you can point at** — a `file:line` or a capture path.
- **Do not change code.**
  Finding and fixing are separate jobs.
- **Consistency findings outrank taste findings.**
  Mark taste `low`.
- Captures are Chromium 151 only.
  Nothing here says anything about Safari, which matters if the CSS leans on WebKit-specific behavior.

## Finding format

```markdown
### F-014 — Paragraph margin applied twice in rendered AsciiDoc

- **route:** /tree?path=guide.adoc
- **source:** crates/git-forge-ui/src/render.rs:88, assets/style.css:412
- **evidence:** target/ui-audit/<run>/light/tree-guide-adoc/styles.json —
  `div.paragraph` reports marginBottom 16px and its child `p` reports 16px
- **phase:** 2
- **severity:** high
- **proposal:** the wrapper is the block-level element; the inner `p` should
  carry no block margin
- **status:** open
```

`severity` is `high` (broken or misleading), `medium` (inconsistent with the app's own system), or `low` (aesthetic judgment).
`status` is `open`, `fixed`, `rejected`, or `needs-recapture`.
