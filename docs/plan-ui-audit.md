# Plan: `git-forge-ui` UX/UI cleanup audit

The rationale, the scaffolding, and the environment facts.
**The runnable prompts live in `docs/ui-audit/`** — one self-contained file per phase, each meant to be pasted into a fresh session on its own.
An agent running a phase does not need this document; a human deciding what to run does.

## Why

Ad-hoc use of the UI surfaced several defect classes that nobody has swept systematically:

- **Prose spacing.**
  Rendered AsciiDoc and Markdown appear double-spaced.
- **Copy.**
  The same concept is named several ways — a comment with no `subject` appears as "Free-floating comment", "Free-floating discussion", and lowercase "free-floating" across `pages/comments.rs`, `pages/issues.rs`, and `pages/reviews.rs`.
- **Visual markers.**
  Icons, badges, and status indicators are misaligned or inconsistent.

The goal is one deliberate cleanup pass, not a redesign.

## Phases

| Phase | Job | Depends on |
|---|---|---|
| [0](ui-audit/phase-0.md) | Capture harness | — |
| [1](ui-audit/phase-1.md) | Copy and nomenclature | — |
| [2](ui-audit/phase-2.md) | Spacing and typography | 0 |
| [3](ui-audit/phase-3.md) | Visual markers, states, affordances | 0 |
| [4](ui-audit/phase-4.md) | Fix pass | 1, 2, 3 |
| [5](ui-audit/phase-5.md) | Regression guard (optional) | 4 |

Phases 0 and 1 are independent and can run in parallel.
Phase 1 needs no tooling, so it is the cheapest place to start.

Phases share state only through `docs/ui-audit/findings.md` on disk, never through conversation.
That is what makes this a handoff: Phase 1 can run today and Phase 2 next week in a different session with nothing lost.

## What already exists (reuse, don't rebuild)

| Asset | Location | Use |
|---|---|---|
| Route inventory | `#[page(...)]` attributes in `crates/git-forge-ui/src/pages/*.rs` | 30 GET pages, 28 POST handlers |
| Fixture repo builder | `tests/ui.rs`, `ui_crawls_every_internal_route` | Temp repo with `README.md`, `guide.adoc`, `src/lib.rs`, `notes/plain.txt`, two members, an issue, a review |
| Server spawner | `Server::spawn`, `tests/ui.rs:392` | Boots the binary on an ephemeral port, reads the URL off stdout |
| Design comp | `Git Forge v5.dc.html` (repo root, untracked) | Visual reference for the v5 redesign |
| Design intent | `docs/design-git-forge-ui.adoc` | Layout-shell structure only — see the caveat below |
| Stylesheet | `crates/git-forge-ui/assets/style.css` | 66KB, single source of all styling, linked from `shell.rs:305` |

**The design doc is badly stale.**
It is v1.0, dated 2026-08-06, and it commits to a read-only UI with no create/comment/approve/close — there are 28 POST handlers.
It describes `/` as a dashboard; `/` now 307-redirects to `/inbox`.
It says file browsing was intentionally dropped; `pages/files.rs` exists.
Five current pages (annotations, gates, releases, palette, theme) postdate it.
It is usable as a record of *layout intent* and nothing else.
Phase 1 is explicitly told not to treat it as a naming authority, because doing so manufactures false findings.

**The design comp is untracked** — the repo `.gitignore` has `*.html`.
It is present locally; a fresh clone will not have it.

## Environment

Three facts that will otherwise cost an agent a debugging cycle each:

- **The server speaks HTTPS with a self-signed local certificate.**
  Playwright needs `ignoreHTTPSErrors: true` or every navigation fails with `ERR_CERT_AUTHORITY_INVALID`.
- **Theme is a cookie, not a media query.**
  `pages/theme.rs` writes a `theme` cookie driving a `data-theme` attribute on `:root`.
  Playwright's `emulateMedia` reaches only the auto branch, never the explicit rules at `style.css:124` and `:162`.
- **Captures go under `target/ui-audit/<run>/`**, not `/tmp`.
  `target/` is gitignored, and unlike `/tmp` it survives across days — which matters when Phase 3 runs a week after Phase 0.
  Each run gets its own id, because Phase 4 diffs a post-fix run against the pre-fix one.

Playwright 1.62.1 and Chromium 151 are in the npx cache under `~/Library/Caches/ms-playwright/`; neither is a project dependency.
There is no Playwright MCP registered with Claude Code — Zed's `mcp-server-playwright` extension serves Zed's own agent panel only.
Add one with `claude mcp add playwright -- npx @playwright/mcp@latest` if interactive browsing is wanted alongside the scripted capture.

Boot the UI by hand with:

```text
cargo run -p git-forge-ui -- --repo <fixture> --port 5050
```

## Context discipline

The corpus is large enough to exhaust a context window before the audit finishes: a file-listing page can exceed 100KB of HTML, and each screenshot costs 1–2k tokens.
Four rules, restated inside each phase prompt that needs them:

1. **Shrink at capture time.**
   Strip `data:` URIs; keep one copy of each *distinct* inline SVG and drop duplicates; collapse repeated siblings to the first three plus a `<!-- N more -->` marker.
   Structure and `class` attributes must survive — they are the evidence.
2. **Prefer text to pixels where text is exact.**
   Computed-style tables answer spacing and typography questions more precisely than screenshots, at a fraction of the cost, and they diff cleanly across a fix.
   Reserve images for genuinely spatial questions: alignment, hierarchy, iconography.
3. **Read `style.css` once.**
   It is linked, not inlined, so captured pages reference it rather than copying it.
   Never read it per-page.
4. **Findings live on disk.**
   Append to `docs/ui-audit/findings.md` the moment something is found.
   Context is then disposable between phases.

The same logic applies to this document: each phase prompt inlines the rules and the finding format it needs, so no phase ever loads another phase's instructions.

## Guardrails, restated in every phase

- **Only report what you can point at.**
  A finding without a `file:line` or a capture path is speculation.
  This is the main defense against invented problems.
- **Audit phases do not change code.**
  Finding and fixing are separate jobs; interleaving them corrupts both.
- **Consistency findings outrank taste findings.**
  "These two buttons disagree" is verifiable.
  "This feels unbalanced" is not, and should be rare and marked `low`.
- Captures are Chromium 151 only.
  Nothing here says anything about Safari, which matters if the CSS leans on WebKit-specific behavior.

## `findings.md` format

One entry per finding, appended by every phase, at `docs/ui-audit/findings.md`:

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

`severity` is one of `high` (broken or misleading), `medium` (inconsistent with the app's own system), `low` (aesthetic judgment).
`status` is one of `open`, `fixed`, `rejected`, `needs-recapture`.
