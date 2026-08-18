# Phase 0 findings — capture harness

Phase 0 builds instruments, not judgments (per `docs/ui-audit/phase-0.md`), so these are engineering findings about the harness and its environment, not UX opinions about the rendered pages.
Each entry cites the file:line or capture path that was actually checked while building/running `scripts/ui-audit/capture.mjs` and `crates/git-forge-ui/tests/fixture.rs`.

## F0-001 — The server is plain HTTP, not HTTPS, despite the printed URL

`docs/ui-audit/phase-0.md` states "The server speaks HTTPS with a local self-signed certificate" and instructs Playwright to use `ignoreHTTPSErrors: true`.
That's not what happens.
`git-forge-ui` binds a bare `tokio::net::TcpListener` and hands it straight to `topcoat::serve`, which never wraps it in TLS:

- `crates/git-forge-ui/src/bin/git-forge-ui.rs:41-46` -- `TcpListener::bind`,
  then `topcoat::serve(listener, router)` with no TLS layer in between.
- `crates/git-forge-ui/src/bin/git-forge-ui.rs:104` -- `ui_url` hardcodes the
  `https://` scheme in the string it prints, independent of how the listener
  was actually set up.
- `vendor/topcoat/crates/topcoat/src/serve.rs:37` -- `serve()`'s signature
  takes any `Listener`, with no certificate/TLS parameter anywhere in the
  function.

Verified two ways:

- `curl -vk --tlsv1.3 https://127.0.0.1:<port>/inbox` against a freshly
  booted server fails the TLS handshake (`ST_CONNECT, tlsv1 alert protocol
  version` from LibreSSL) -- there's no TLS listener to negotiate with.
- `crates/git-forge-ui/tests/ui.rs`'s own `HttpClient` (its `TcpStream`-based
  test harness) sends and receives plain, unencrypted HTTP/1.1 successfully
  against the same binary, and that's the only way the existing test suite
  talks to it.
- `capture.mjs` connecting over `http://` (not `https://`) against the
  printed port works end to end for all 66 route/theme captures in
  `target/ui-audit/phase0-verify/index.json`; the earlier attempt connecting
  over `https://` per the brief's instructions failed every single
  navigation with `net::ERR_SSL_PROTOCOL_ERROR`, `ignoreHTTPSErrors` or not.

`scripts/ui-audit/capture.mjs`'s `spawnServer` now rewrites the scheme to `http://` when it parses the printed URL, with a comment pointing back to this.
Later phases reusing the printed URL directly (rather than through `capture.mjs`) will hit the same wall.

## F0-002 — The brief's "30 GET routes" figure is right, but two are easy to drop from a route list

`docs/ui-audit/phase-0.md` says to walk "every GET route from the `#[page("...")]` attributes in `crates/git-forge-ui/src/pages/*.rs` -- 30 of them".
That count is accurate, confirmed by enumerating every non-`POST` `#[page(...)]` attribute across all 14 files in `crates/git-forge-ui/src/pages/` -- but two of the 30 are not obvious candidates for a hand-written route list and were missing from the first draft of `capture.mjs`'s route table:

- `/` -- `crates/git-forge-ui/src/pages/root.rs:5`.
  It 307-redirects to `/inbox` (also called out in the brief), which makes it tempting to treat as "not really a page" and skip; it's still one of the 30 `#[page(...)]` attributes and Playwright's `page.goto` follows the redirect transparently, landing on `/inbox`'s content.
- `/palette` -- `crates/git-forge-ui/src/pages/palette.rs:133`.
  Per the comment at `palette.rs:1-14`, the palette opens via a same-page URL fragment (`href="#palette"`, a CSS `:target` reveal) rather than a normal navigation, so a link-following crawler (like `tests/ui.rs`'s `Crawler`, see `crawler.enqueue_links` at `tests/ui.rs:534`) never issues a GET for it as a distinct request -- there's no plain `<a href="/palette">` anywhere in the shell to follow.
  It only shows up if a route list is built from the `#[page(...)]` attributes directly rather than from crawling links, which is what `capture.mjs` now does.

`scripts/ui-audit/capture.mjs`'s `routes()` includes both.
The final run (`target/ui-audit/phase0-verify/index.json`) has 31 distinct paths (the 30 base routes plus `/tree?...&view=preview`/`...&view=source` counted separately from bare `/tree`, per the brief's instruction to capture prose views), 66 total captures across both themes, all with a 2xx or documented non-2xx status.

## F0-003 — Unknown `{release_tag}` / `{*annotation_id}` don't 404; only unknown `{release_tag}/edit` does

The fixture deliberately keeps releases and annotations at zero entities (an empty state named directly in the brief), which means there's no real ID to substitute into `/releases/{release_tag}` or `/annotations/{*annotation_id}` when walking routes mechanically.
Empirically, an unresolvable ID does not 404 on the detail routes, only on the edit route:

- `crates/git-forge-ui/src/pages/releases.rs:161-174` (`release_page`) looks
  up the tag in `rows` and falls back to an inline `"Annotated tag not
  found."` panel inside a normal `200`-status `route_shell` render when it
  isn't found, rather than returning an HTTP error.
- Verified in `target/ui-audit/phase0-verify/index.json`:
  `GET /releases/no-such-release` -> `200` (both themes), but
  `GET /releases/no-such-release/edit` -> `404` (both themes) -- the edit
  handler (`releases.rs:230-248`) does return an error for a tag that isn't
  found, unlike the detail page.
  `GET /annotations/no-such-annotation` -> `200` (both themes) similarly.

This matters for later phases: a crawler that treats "any non-2xx" as the signal for "this ID doesn't exist" will misclassify these two detail routes as successes even though they're rendering a not-found panel, not the requested entity.
`capture.mjs` records `expectNon2xx` per route so this distinction is explicit in `index.json` rather than inferred from status code alone.

## F0-004 — `page.html` size target (soft) is missed by 3 of 66 captures, all due to the shared nav rail

`docs/ui-audit/phase-0.md` asks for `page.html` "under 10KB per file" and specifies collapsing "repeated siblings to the first three plus a `<!-- N more -->` marker."
In `target/ui-audit/phase0-verify`, 3 of 66 `page.html` files exceed 10KB (largest observed: 12KB, e.g. `light/reviews_audit-review-1/page.html`).
The primary-nav rail (`class="nav-link"` anchors, one per tab, each with a distinct inline SVG icon) is not collapsed by the sibling rule as implemented, because the active tab's anchor carries an extra `active` class (`class="nav-link active"`), which breaks the run of identical `tag+class` siblings before it reaches a run longer than 3 -- verified by grepping `class="nav-link` in that file's stripped output, which shows 3 plain `nav-link` anchors, then one `nav-link active`, then 3 more plain `nav-link` anchors (never more than 3 consecutive identical shapes).
This is the intended behavior of the spec as written (collapsing by exact `tag+class` match), not a bug in the stripping code; it's flagged here because a stricter collapse (e.g. ignoring `active`) would remove real distinguishing structure from the "evidence" the brief says must survive.
All files are still comfortably "small enough to read" (the "Done when" bar), so this wasn't treated as a blocker.

## Artifact count and size (verification run)

Run id `phase0-verify`, `target/ui-audit/phase0-verify/`:

- 66 route captures (31 distinct paths x light/dark), all `2xx`, or `404`/etc.
  marked `expectNon2xx: true` in `index.json` -- 0 undocumented non-2xx
  statuses.
- 203 files on disk, 5.5 MB total (`du -sh target/ui-audit/phase0-verify`).
- `page.html`: 63 of 66 under 10KB (see F0-004 for the other 3).
- `styles.json`: largest observed 24KB (`.../guide.adoc.../styles.json`),
  under the brief's ~50KB budget with room to spare.
- `page.png` (1280x900) for all 66 captures, plus `page-full.png` (full
  page) for the 10 prose routes (`/tree` with the `.md`/`.adoc` paths, the
  issue detail, and the review detail) x 2 themes = 20 extra screenshots.

## Deliverables

- `crates/git-forge-ui/tests/fixture.rs` -- `#[ignore]`d `build_fixture` test; invoked directly (`cargo test -p git-forge-ui --test fixture -- --ignored --exact build_fixture`) with `UI_AUDIT_REPO` set.
  Builds the prose `.md`/`.adoc` fixtures (consecutive paragraphs, nested lists, tables, admonitions, fenced code, h1-h4, inline code, links), two members with different roles, and one comment each attached to an issue, attached to a review, and with no subject -- while deliberately leaving releases, gate rules, and bound comments at zero so those empty states render.
- `scripts/ui-audit/capture.mjs` -- boots the fixture + server, walks all 30 `#[page(...)]` GET routes (31 distinct paths counting `/tree`'s prose query variants) x 2 themes, writes `page.html`/`styles.json`/`page.png` (+ `page-full.png` for prose routes) per route under `target/ui-audit/<run>/<theme>/<route>/`, and `index.json` at the run root.
  Resolves `playwright` from the npx cache (`~/.npm/_npx/*`) since it isn't a project dependency, per the brief.
