# Phase 0 — Capture harness

Build tooling for a UX audit of `git-forge-ui`, a server-rendered HTML app in this Rust workspace (topcoat framework, one stylesheet at `crates/git-forge-ui/assets/style.css`, no client JavaScript).
No judgments in this phase — you are building instruments, not reading them.

## Environment facts you need

- **The server speaks HTTPS with a local self-signed certificate.**
  Playwright must use `browser.newContext({ ignoreHTTPSErrors: true })` or every navigation fails with `ERR_CERT_AUTHORITY_INVALID`.
- It prints its URL on stdout as `https://127.0.0.1:<port>`; `--port 0` binds ephemerally.
  See `Server::spawn` in `crates/git-forge-ui/tests/ui.rs:392`.
- Boot: `cargo run -p git-forge-ui -- --repo <fixture> --host 127.0.0.1
  --port 0`.
- **Theme is a cookie, not a media query.**
  `pages/theme.rs` writes a `theme` cookie (`light` / `dark`, absent means auto) which drives a `data-theme` attribute on `:root`.
  `emulateMedia` exercises only the auto branch and never reaches the explicit rules at `assets/style.css:124` and `:162`.
  Set the cookie.
- Playwright 1.62.1 and Chromium 151 are in the npx cache under `~/Library/Caches/ms-playwright/`.
  Neither is a project dependency.
- `/` 307-redirects to `/inbox`.

## Build two things

### 1. A fixture repo builder

`scripts/ui-audit/fixture.sh`, or a `#[test]`-gated helper in the `git-forge-ui` test tree — **not** a loose `.rs` file under `scripts/`, which this workspace has no way to build.
Start from the fixture in `crates/git-forge-ui/tests/ui.rs` (`ui_crawls_every_internal_route`) and extend it with content aimed at the three symptoms this audit is chasing:

- An `.adoc` and an `.md` file each exercising consecutive paragraphs, nested lists, tables, admonitions/blockquotes, fenced code, headings h1–h4, inline code, and links.
  Consecutive paragraphs are the important one.
- At least one entity in every empty state the UI can show: zero releases, zero
  gates, zero annotations.
- Two members with different roles, so permission-varied views render.
- One comment of each kind: attached to an issue, attached to a review, and one
  with no `subject` at all.

Stress cases — a 300-file directory, a 4000-line diff, a binary file, RTL text, a 200-character title — are useful but target none of the named symptoms.
Add them last, or leave them for a follow-up.

### 2. A capture script

`scripts/ui-audit/capture.mjs`.
It must:

- Take a run id (default: a timestamp) and write everything under `target/ui-audit/<run>/`.
  `target/` is gitignored and, unlike `/tmp`, survives across days — later phases read this output, possibly next week.
  **Never write to a fixed path that a re-run would overwrite**; Phase 4 diffs a later run against an earlier one.
- Boot `git-forge-ui` against the fixture repo and wait for the URL on stdout.
- Walk every GET route from the `#[page("...")]` attributes in
  `crates/git-forge-ui/src/pages/*.rs` — 30 of them; the 28 `#[page(POST ...)]`
  handlers are out of scope — substituting real fixture IDs for path
  parameters.
- Run the whole walk twice, once with `theme=light` and once `theme=dark`.
- For each route write three artifacts under
  `target/ui-audit/<run>/<theme>/<route>/`:
  - **`page.html`** — the DOM after load, stripped: drop `data:` URIs; keep one copy of each *distinct* inline SVG and drop duplicates; collapse repeated siblings to the first three plus a `<!-- N more -->` marker.
    Structure and `class` attributes must survive — they are the evidence.
    Target under 10KB per file.
  - **`styles.json`** — `getComputedStyle` for every element matching a configurable selector list, defaulting to `.paragraph, .paragraph > p, p, h1, h2, h3, h4, ul, ol, li, pre, code, blockquote, table, .admonition, button, .badge`.
    Record `marginTop`, `marginBottom`, `paddingTop`, `paddingBottom`, `lineHeight`, `fontSize`, `display`.
    Include each element's tag, class, and a short text excerpt so a finding can be traced back to a place on the page.
    Keep these under ~50KB per route; if a route blows past that, cap the per-selector element count and record the overflow rather than emitting thousands of rows.
  - **`page.png`** — a screenshot.
    Capture the viewport at 1280×900 *and*, for routes that render prose (`/tree` with a `.adoc` or `.md` path, issue and review detail), a full-page shot as well — vertical rhythm is exactly what sits below the fold.
- Write `target/ui-audit/<run>/index.json` listing every route captured, its
  HTTP status, and its artifact paths.

## Done when

The script runs end to end, `index.json` lists every GET route with a 2xx or a documented non-2xx status, and the stripped `page.html` files are small enough to read.
Report the artifact count and total on-disk size.
Do not analyze the output — that is a later phase.
