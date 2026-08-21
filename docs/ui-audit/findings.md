# Git Forge UI audit — combined findings

## Phase 1 findings — copy and nomenclature audit

Scope: `crates/git-forge-ui/src/pages/*.rs` and `crates/git-forge-ui/src/shell.rs`.
Cross-checked against `gix_forge` domain types (`Issue`, `Review`, `Comment`, `AnnotationState`/`AnnotationBinding`, `Member`, `Gate`, `Release`) and, where noted, `crates/git-forge-ui/assets/style.css`.

---

## F1-001 — The no-subject comment concept is written five ways

- **source:** crates/git-forge-ui/src/pages/comments.rs:57, :61, :65, :211, :255; crates/git-forge-ui/src/pages/annotations.rs:161, :212; crates/git-forge-ui/src/pages/issues.rs:352; crates/git-forge-ui/src/pages/reviews.rs:515
- **evidence:** the `<option>` label and link text both read "Free-floating comment" (comments.rs:57/61/65, issues.rs:352, reviews.rs:515); the comment-detail and annotation-detail empty-subject copy reads "Free-floating discussion" (comments.rs:211, annotations.rs:212); the edit-form context line and the annotation nav's subject fallback both use lowercase "free-floating" as a bare adjective (comments.rs:255, annotations.rs:161).
  Same concept — a `Comment` whose `subject` is `None` — named with two different nouns ("comment" vs. "discussion") and two different casings, sometimes on the same route (annotations.rs uses both "Free-floating discussion" at :212 and lowercase "free-floating" at :161).
- **phase:** 1
- **severity:** medium
- **proposal:** pick one noun ("comment" or "discussion") and one casing rule, and apply it at all nine sites.
- **status:** fixed — the two "Free-floating discussion" sites (comments.rs:211, annotations.rs:212) now read "Free-floating comment", matching the noun already used at the other seven sites.
  The Title-Case-in-UI-copy vs. lowercase-as-bare-adjective split (comments.rs:255, annotations.rs:161) was left as-is: it's ordinary sentence casing, not an inconsistency independent of the noun mismatch.

### F1-002 — Comment forms break the "Save X" convention every other entity form follows

- **source:** crates/git-forge-ui/src/pages/issues.rs:99; crates/git-forge-ui/src/pages/reviews.rs:208; crates/git-forge-ui/src/pages/members.rs:79; crates/git-forge-ui/src/pages/releases.rs:58; crates/git-forge-ui/src/pages/comments.rs:82, :260
- **evidence:** `issue_form`, `review_form`, `member_form`, and `release_form` all submit with `"Save <entity>"` for both create and edit (the same form fn is reused by both create and edit routes for each).
  `comment_form`, used only for *creating* a comment, instead submits `"Publish comment"` (comments.rs:82), while the separate comment *edit* form (comments.rs:260) reverts to `"Save comment"`.
  Comment is the only entity where create and edit use different verbs, and its create verb ("Publish") doesn't match any other entity's create verb ("Save").
- **phase:** 1
- **severity:** medium
- **proposal:** either make Comment's create button say "Save comment" to match the rest of the app, or apply "Publish" consistently to comment creation everywhere comments are created (see F1-003).
- **status:** fixed — resolved together with F1-003: comments.rs:82's create button now reads "Comment" (was "Publish comment"), matching the label already used at every other comment-creation entry point (issues.rs:376, reviews.rs:539, files.rs:743).
  The comment *edit* form still says "Save comment" (comments.rs:260), so create/edit remain two different verbs by design — same as every other entity, where "create" and "edit" both say "Save X" only because they share one form fn; Comment's create and edit are genuinely separate forms and now differ only in the same way "Comment" (create) vs.
  "Save comment" (edit) reads naturally, not arbitrarily.

### F1-003 — Three different button labels for "submit a new comment"

- **source:** crates/git-forge-ui/src/pages/issues.rs:376; crates/git-forge-ui/src/pages/reviews.rs:539; crates/git-forge-ui/src/pages/files.rs:743; crates/git-forge-ui/src/pages/comments.rs:82
- **evidence:** the inline reply form on an issue detail page submits with bare `"Comment"` (issues.rs:376), on a review detail page also `"Comment"` (reviews.rs:539), and the per-line/per-file composer on the Code route also `"Comment"` (files.rs:743) — but the dedicated "New comment" page reachable from all three of those same contexts via the "Free-floating comment" link submits with `"Publish comment"` (comments.rs:82).
  A user posting a comment sees one verb or the other depending only on entry point.
- **phase:** 1
- **severity:** medium
- **proposal:** standardize on one label; "Comment" is already the majority.
- **status:** fixed — see F1-002; comments.rs:82 now reads "Comment", matching issues.rs:376, reviews.rs:539, and files.rs:743.
  All four comment-creation entry points now submit with the same label.

### F1-004 — Ahead/behind indicator is a static placeholder wearing a real-data title

- **source:** crates/git-forge-ui/src/shell.rs:262
- **evidence:** `<span class="ahead-behind" title="Ahead/behind against upstream">"↑– ↓–"</span>` — the topbar (rendered on every page) always shows the literal glyphs `↑– ↓–`; no branch/upstream comparison is computed anywhere in `shell.rs`.
  The `title` promises a live counter but is a fixed placeholder in every render.
- **phase:** 1
- **severity:** high
- **proposal:** wire this up to a real ahead/behind count, or remove/hide the misleading title until it's real — the same discipline `files.rs`'s blame column already applies to unimplemented data (files.rs:656-660).
- **status:** fixed — `shell.rs` now computes a real ahead/behind count via `gix` (`HEAD`'s upstream tracking ref + two `rev_walk`/`with_hidden` counts) in a new `ahead_behind()` helper (shell.rs:146-176); the topbar (shell.rs:293-300) only renders the `<span>` when an upstream exists, so the indicator is gone entirely for detached/untracked branches instead of showing a static placeholder.

### F1-005 — Comment dead-ends label the Code route "home," contradicting the app's own front-door

- **source:** crates/git-forge-ui/src/pages/comments.rs:229, :364; crates/git-forge-ui/src/pages/root.rs:1-8
- **evidence:** `comment_detail`'s not-found branch (comments.rs:229) and `comment_delete`'s not-found branch (comments.rs:364) both render `<a href="/tree">"Return home"</a>`.
  `/tree` is the Code route.
  But `root.rs:3-4`'s own doc comment states: "The workspace has no dashboard; `/` exists only to send visitors somewhere real.
  Inbox is the app's front door," and `/` 307-redirects to `/inbox`.
  "Return home" pointing at Code, not Inbox, contradicts the app's own stated information architecture.
- **phase:** 1
- **severity:** medium
- **proposal:** link "Return home" to `/inbox`, or relabel the link to name what `/tree` actually is ("Back to files").
- **status:** fixed — comments.rs:229, :364 (both not-found branches) and comments.rs:361 (delete-success panel, same "Return home" pattern, not separately cited by the finding but sharing the identical bug) now link to `/inbox` instead of `/tree`.

### F1-006 — Comment error heading splits "added" vs. "saved" for the same failure across contexts

- **source:** crates/git-forge-ui/src/pages/issues.rs:541, :573; crates/git-forge-ui/src/pages/reviews.rs:852, :884; crates/git-forge-ui/src/pages/comments.rs:49, :286, :332; crates/git-forge-ui/src/pages/files.rs:740, :764
- **evidence:** when posting a comment on an issue or review fails, the panel heading reads `"Comment could not be added"` (issues.rs:541/573, reviews.rs:852/884).
  For a free-floating comment or a file/line comment, the identical outcome instead reads `"Comment could not be saved"` (comments.rs:49/286/332, files.rs:740/764).
  Two verbs describe the same failure depending only on which route the comment is attached through.
- **phase:** 1
- **severity:** medium
- **proposal:** pick one verb; "could not be saved" already covers both create and edit elsewhere (F1-002), so it's the more consistent choice.
- **status:** fixed — issues.rs:541,573 and reviews.rs:852,884 now read "Comment could not be saved", matching comments.rs and files.rs.

### F1-007 — Filtered-empty state on Annotations gives no way back, unlike every comparable empty state

- **source:** crates/git-forge-ui/src/pages/annotations.rs:156-158; crates/git-forge-ui/src/pages/issues.rs:120-128; crates/git-forge-ui/src/pages/reviews.rs:229-237
- **evidence:** when a filter on `/annotations?state=...` matches nothing, `annotation_nav` renders only `<p class="route-nav-note">"No annotations match this filter."</p>` (annotations.rs:156-158) — no link back to "All".
  Every comparable empty state pairs the message with an actionable next step: `issue_list`'s empty panel offers "New issue"/"Open the first issue" or an explanation (issues.rs:120-128); `review_list`'s offers "Request a review" or an explanation (reviews.rs:229-237).
  Annotations' filtered-empty case states a problem without offering the obvious fix.
- **phase:** 1
- **severity:** medium
- **proposal:** add a link back to `/annotations` ("All") inside the empty-filter message.
- **status:** fixed — annotations.rs:157 now renders `"No annotations match this filter. "` followed by a `<a href=(state_filter_href("all"))>"Show all"</a>` link, matching the "obvious next step" pattern the finding pointed to in issues.rs/reviews.rs.

### F1-008 — Signing key overflow: the detail-pane copy has no wrap/truncation protection the nav-pane copy already has

- **source:** crates/git-forge-ui/src/pages/members.rs:178, :209; crates/git-forge-ui/assets/style.css:594-601, :3074-3079
- **evidence:** the same `member.signing_key` string is rendered twice: the nav-pane row (members.rs:178) uses class `nav-row-sub`, defined at style.css:594-601 with `white-space: nowrap; overflow: hidden; text-overflow: ellipsis;`.
  The detail pane (members.rs:209) uses class `trust-key`, defined at style.css:3074-3079 with no `overflow`/`white-space`/`overflow-wrap` rule at all.
  A long OpenPGP/SSH key (the form itself invites either format, members.rs:58) has truncation protection only in its smaller preview, not where it's shown at full size.
- **phase:** 1
- **severity:** low
- **proposal:** add `overflow-wrap: anywhere` to `.trust-key`.
- **status:** fixed — added `overflow-wrap: anywhere;` to `.trust-key` (style.css:3080-3082), so a long signing key now wraps within the detail pane instead of overflowing, matching the wrap/truncation protection `.nav-row-sub` already had in the smaller preview.

### F1-009 — The "Trust" tab renames `Member` without saying so anywhere in its own copy

- **source:** crates/git-forge-ui/src/shell.rs:27, :51, :68; crates/git-forge-ui/src/pages/members.rs:47-49, :159
- **evidence:** the rail tab and route title are "Trust" (shell.rs Tab::Trust label at :51, href `/members` at :68; nav header "Trust" at members.rs:159), but the domain type is `gix_forge::Member`, and the route's own form copy says "member" throughout: "REPOSITORY ACCESS" / "Add a trusted member" (members.rs:47-49), plus page titles "Add member," "Edit member," etc. (members.rs:305/312/327/339/353/361/391/406/418/435/443/461/479).
  Nothing in the UI states that "Trust" and "member directory" are the same feature.
- **phase:** 1
- **severity:** low
- **proposal:** either rename the tab "Members" to match the domain type and the page's own copy, or add a one-line subhead explaining the mapping.
- **status:** fixed — took the subhead route, matching the existing `route-nav-note` pattern used by `gates.rs`/`releases.rs`: added `<p class="route-nav-note">"members with repository access"</p>` under the "Trust" nav header (members.rs:158-164), so the tab now discloses its mapping to `Member` without renaming the tab itself.

### F1-010 — "Annotations" is a UI-only name for a filtered view of `Comment`, not a distinct domain entity

- **source:** crates/git-forge-ui/src/pages/annotations.rs:1-15; crates/gix-forge/src/comment.rs:95, :108; crates/git-forge-ui/src/shell.rs:24, :36, :48, :65, :83-85
- **evidence:** the rail tab, route title, and URL are all "Annotations" (shell.rs Tab::Annotations), but `gix_forge` has no `Annotation` entity — only `Comment`, plus `AnnotationState`/`AnnotationBinding` (gix-forge/src/comment.rs:95,108) describing a `Comment`'s binding health.
  `annotations.rs`'s own module doc (annotations.rs:1-15) confirms this route is "the unified surface for every comment, anchored or free-floating."
  Not a bug — the module doc discloses it — but a genuine UI/domain-model name disagreement: nothing user-facing on the route ever explains that "Annotation" means "Comment" (comment detail pages say "Comment by X," e.g. comments.rs:207, annotations.rs:206).
- **phase:** 1
- **severity:** low
- **proposal:** no code change needed; if help/onboarding copy is ever added, state the mapping explicitly ("Annotations shows every comment, including ones anchored to code").
- **status:** reviewed, no code change needed (see proposal)
</content>

## Phase 2 findings — spacing and typography

Scope: `crates/git-forge-ui/assets/style.css` (read once in full, 3195 lines), computed-style dumps under `target/ui-audit/phase0-verify/{light,dark}/*/styles.json`, and `page.html` structure only where needed.
`page.png` was not opened, per phase instructions.

## Derived system

**Spacing scale.**
`style.css` has no custom properties for spacing (only color tokens are declared as `--vars`; spacing is a literal `rem` value on every declaration).
Sampling every `margin`/`padding`/`gap` rem value in the file gives:

```text
0.05 0.1 0.12 0.13 0.15 0.18 0.2 0.22 0.25 0.28 0.3 0.35 0.4 0.45 0.5 0.55 0.6
0.65 0.7 0.75 0.8 0.85 0.9 1 1.05 1.1 1.25 1.35 1.5 1.75 2  (rem)
```

This is effectively **every multiple of 0.05rem (0.8px at the 16px root) from 0.05 to 1.1rem**, plus a handful of larger round values (1.25, 1.5, 1.75, 2rem) used for section/page-level spacing.
There is no discrete step system (no 4pt/8pt grid) — the "scale" is a continuum in 0.05rem increments below 1rem.
Treat `0.05rem` as the implicit base unit for anything under 1rem, and whole `0.25rem` steps above it.

**Type scale.**
Distinct `font-size` values actually used, with occurrence counts (`grep -oE 'font-size: [0-9.]+rem' assets/style.css | sort | uniq -c`):

```text
0.58×1  0.6×2  0.62×7  0.63×4  0.64×5  0.65×2  0.66×9  0.68×15  0.7×5  0.72×10
0.74×7  0.75×4  0.76×10  0.78×12  0.8×9  0.82×6  0.84×1  0.85×1  0.86×1  0.88×2
0.9×4  0.95×2  1×3  1.15×1  1.2×1  1.3×2  1.35×2  1.5×1  1.6×2  1.7×1  (rem)
```

28 distinct values, again in ~0.01–0.02rem increments through the small end (0.58–0.9rem) rather than a small closed set (e.g. xs/sm/base/lg).
This is a real, load-bearing part of this audit's method: because the scale is this granular, "does this sit on the scale" is nearly always true by construction — so scale-membership sweeps below are only useful for values that don't even match a used step, not for judging whether a size is "clean."

**Line-height ratios.**
Root sets `line-height: 1.6` (`style.css:81`) as the document default (monospace UI chrome).
Prose contexts override this per-block:

- `.body-copy` (Lora serif prose, `style.css:1003-1008`): `line-height: 1.75`.
- `.rendered-file` (markdown/AsciiDoc preview body, `style.css:1699-1706`): `line-height: 1.75`.
- `.rendered-file h1..h6` (`style.css:1713-1722`): `line-height: 1.25`, a distinct tight heading ratio.
- `code`/`pre`/`.code-view`/`.source-view` (e.g. `style.css:1238-1244`, `1486-1492`, `1799-1806`): `line-height: 1.65`, a third ratio for monospace code blocks.
- `textarea` (`style.css:777-781`): `line-height: 1.55`.
- `.target-explain` (`style.css:2608-2613`): `line-height: 1.55`.

So there are effectively four line-height ratios in play: `1.6` (UI chrome default), `1.65` (code), `1.75` (prose body), `1.25` (prose headings), plus a lone `1.55` used in two unrelated places (form textarea, review target explanation) that doesn't obviously belong to either the UI or prose family.

**Color roles.**
Canonical tokens (`style.css:11-45` dark default, `:124-161` light pin, `:84-123` light media-query default) follow a `--bg/--rail/--surface/--row` surface ladder, a `--text/--text2/--muted/--dim/--faint/--fainter` text-emphasis ladder, one `--acc` accent plus `-h`/`-on`/`-b`/`-t` hover/on-accent/border/tint variants, and `--ok/--warn/--bad` status colors with the same `-b`/`-t` suffix convention.
A large "legacy alias" block (`style.css:47-76`) forwards old pre-v5 names (`--border`, `--text-muted`, `--accent`, `--danger`, `--mauve`, `--syntax-*`, etc.) onto the canonical names; every rule below roughly line 200 still reads the legacy names, so the canonical names are effectively unused outside `:root` itself and the Handoff-v5 sections added after the rename (Annotations, Inbox, Gates, Trust: `style.css:613-652`, `2701-2907`), which use the new names directly.
Naming-debt observation, not a spacing/typography bug — noted only because it explains why some rules read `var(--accent)` and others `var(--acc)` for the same color.

---

## Findings

### F2-001 — AsciiDoc list items get an extra 16px of trailing margin that Markdown list items don't

- **route:** `/tree?ref=HEAD&path=guide.adoc&view=preview` (also affects any AsciiDoc body rendered through `.rendered-file`/`.body-copy.rendered-file`: Issues and Reviews descriptions)
- **source:**
  - `crates/git-forge-ui/src/render.rs:31` (`render_asciidoc`, using the `asciidoc-html5` crate — see identity note below) vs `render.rs:22` (`render_markdown`, using `comrak`)
  - `crates/git-forge-ui/assets/style.css:1738-1742` — `.rendered-file p, .rendered-file ul, .rendered-file ol { margin: 0 0 1rem; }` is a descendant selector, so it matches `p` at any nesting depth, including inside `<li>`
  - call sites feeding AsciiDoc through the same `.rendered-file` class: `crates/git-forge-ui/src/pages/issues.rs:343`, `crates/git-forge-ui/src/pages/reviews.rs:494`, `crates/git-forge-ui/src/pages/files.rs:937`
- **evidence:**
  - `target/ui-audit/phase0-verify/light/tree__ref=HEAD_path=guide_adoc_view=preview/page.html` — list markup is `<li><p>Top-level item one</p></li>` (every `<li>` wraps its text in a `<p>`)
  - `target/ui-audit/phase0-verify/light/tree__ref=HEAD_path=README_md_view=preview/page.html` — the equivalent Markdown list is `<li>Top-level item one</li>` (no `<p>` wrapper; comrak omits it for a "tight" list)
  - `target/ui-audit/phase0-verify/light/tree__ref=HEAD_path=guide_adoc_view=preview/styles.json` → `p` array: the `<p>` with text `"Top-level item one"` reports `marginBottom: "16px"`, `marginTop: "0px"`; that same file's `li` array reports `marginBottom: "0px"` for the corresponding item, i.e. the visible gap after each AsciiDoc list item comes entirely from the nested `<p>`'s margin, since `<li>` itself contributes none
  - `target/ui-audit/phase0-verify/light/tree__ref=HEAD_path=README_md_view=preview/styles.json` → `li` array for the same item also reports `marginBottom: "0px"`, and there is no nested `<p>` to contribute a margin, so the Markdown list has no per-item trailing gap at all
  - identical in `dark/tree__ref=HEAD_path=guide_adoc_view=preview/styles.json` — not a light/dark-specific issue
- **mechanism:** the stylesheet's `.rendered-file p` margin rule was written assuming `<p>` only appears as a direct block between other blocks (as comrak emits it).
  The AsciiDoc converter (`asciidoc-html5` crate, not the vendored `vendor/acdc` parser — see note) always wraps list-item text in a `<p>`, even for simple one-line items.
  The descendant selector then applies the block-paragraph margin *inside* every list item, so AsciiDoc-rendered lists carry 16px of extra space after each item that the equivalent Markdown list doesn't.
  Top-level consecutive paragraphs (outside any list) are not doubled — both renderers give consecutive `<p>` a single 16px gap (verified in the same `styles.json` files), so the "double-spacing" is specifically a list-rhythm bug, not a generic paragraph bug.
- **note on renderer identity:** `crates/git-forge-ui/Cargo.toml:45` pins `asciidoc-html5 = "=0.1.5"` (crates.io), and `render.rs:3` imports `asciidoc_html5::{..., convert_with}`.
  The `vendor/acdc/` workspace members (`Cargo.toml:9-11`) are a separate, unrelated AsciiDoc implementation not wired into `render_asciidoc`.
  The phase-2 brief's premise that the vendored `acdc` parser is the renderer in play does not hold for this code path — worth flagging so a fix isn't aimed at the wrong crate.
- **phase:** 2
- **severity:** high (visibly wrong list rhythm, and it's the mechanism behind the reported bug; not just aesthetic)
- **proposal:** fix at the source rather than overriding margins: either (a) scope the stylesheet rule to non-nested paragraphs (e.g. `.rendered-file > p` plus explicit handling for paragraphs that are the sole/first child of `li`, `blockquote`, etc., rather than a blanket descendant selector), or (b) if `asciidoc-html5` can be configured to omit the `<p>` wrapper for simple list items (matching comrak's "tight list" behavior), prefer that so the two renderers emit comparable structure for comparable input.
  Do not add a `.rendered-file li p { margin: 0 }` patch on top of the existing rule — that buries the mismatch under an override instead of fixing the wrong assumption that a blanket descendant selector is safe for block margins.
- **status:** fixed — took option (a): `.rendered-file p, .rendered-file ul, .rendered-file ol { margin: 0 0 1rem; }` (style.css:1738-1742) is now `.rendered-file p:not(li > p), .rendered-file ul, .rendered-file ol { margin: 0 0 1rem; }`, so the block-paragraph margin never applies to a `<p>` nested directly in an `<li>` in the first place.
  Excluding it from that selector alone left the UA default paragraph margin (`1em`, computed against `.rendered-file`'s own `0.9rem` font-size = 14.4px) filling the gap instead of 16px, so a second rule, `.rendered-file li > p { margin: 0; }` (style.css:1743-1749), states explicitly what the excluded case should be — verified via `post-fix` capture: `styles.json` for `tree?...path=guide.adoc...` now reports `marginBottom: "0px"` for every list-item `<p>` (was `16px` pre-fix), matching Markdown's `<li>` (also `0px`, no nested `<p>`).
  `asciidoc-html5`'s `Options` (checked `asciidoc-html5-0.1.5/src/options.rs` in the cargo registry cache) has no per-list "tight" toggle, so option (b) wasn't available.

### F2-002 — Type scale has 28 distinct font-size values in ~0.01–0.02rem increments

- **route:** whole-app (style-source finding, not route-specific)
- **source:** `crates/git-forge-ui/assets/style.css` — e.g. `0.62rem` (7 uses, incl. `:342`, `:528`), `0.63rem` (4 uses, incl. `:534`), `0.64rem` (5 uses, incl. `:1350`), `0.65rem` (2 uses, incl. `:2281`), `0.66rem` (9 uses, incl. `:598`) all sit within 0.04rem of each other and are used for what reads as the same "extra small label/meta text" role in different components (nav-row-sub, ref-link meta, badge labels, thread-key, etc.)
- **evidence:** `grep -oE "font-size: [0-9.]+rem" crates/git-forge-ui/assets/style.css | sort | uniq -c` returns 28 distinct values between `0.58rem` and `1.7rem`, most below `0.9rem` and spaced by 0.01–0.02rem
- **phase:** 2
- **severity:** low (no visible breakage — differences at this granularity are sub-pixel at typical zoom — this is a taste/maintainability finding, not a correctness one)
- **proposal:** collapse the sub-`0.9rem` cluster onto a small closed set (e.g. `0.62rem`/`0.68rem`/`0.72rem`/`0.78rem`/`0.8rem`) so new component CSS has a real scale to pick from instead of a slightly-different ad hoc value each time.
- **status:** fixed — remapped every `font-size` value in `[0.58rem, 0.9rem)` throughout `style.css` to its nearest neighbor in the proposed set `{0.62, 0.68, 0.72, 0.78, 0.8}rem` (`0.58`/`0.6`/`0.63`/`0.64`→`0.62`, `0.65`/`0.66`→`0.68`, `0.7`/`0.74`→`0.72`, `0.75`/`0.76`→`0.78`, `0.82`/`0.84`/`0.85`/`0.86`/`0.88`→`0.8`); values `≥0.9rem` were left untouched.
  Post-fix `grep -oE "font-size: [0-9.]+rem" assets/style.css | sort | uniq -c` shows the sub-`0.9rem` cluster is now exactly the 5-value closed set (19/26/22/26/20 uses respectively), down from 19 distinct values.

### F2-003 — Detail-pane primary `<h1>` heading has three different bottom margins across routes

- **route:** `/gates/merge-gate`, `/reviews/audit-review-1`, `/members/alice`
- **source:**
  - `crates/git-forge-ui/src/pages/gates.rs:63` — bare `<h1>` inside `.gate-heading`, gets only the base rule `h1 { margin-bottom: 0 }` (`style.css:248-251`); `.gate-heading h1` only overrides `font-size` (`style.css:2932-2934`), not margin
  - `.review-title` — `style.css:2563-2566` — `margin: 0 0 0.3rem` (4.8px at root 16px)
  - `.trust-title` — `style.css:3055-3060` — `margin: 0` explicitly
- **evidence:**
  - `target/ui-audit/phase0-verify/light/gates_merge-gate/styles.json` → `h1` (no class): `marginBottom: "0px"`
  - `target/ui-audit/phase0-verify/light/reviews_audit-review-1/styles.json` → `h1.review-title`: `marginBottom: "4.8px"`
  - `target/ui-audit/phase0-verify/light/members_alice/styles.json` → `h1.trust-title`: `marginBottom: "0px"`
  - same values reproduced in the `dark/` captures for all three routes
- **phase:** 2
- **severity:** medium (inconsistent with the app's own per-route detail-heading pattern — these three headings all play the same structural role: "primary heading of a route's detail pane" — but two land on 0 and one lands on 0.3rem for no evident reason)
- **proposal:** give `.review-title` a `margin-bottom: 0` to match `.gate-heading h1` and `.trust-title`, or establish one shared class for this role instead of three separately-tuned selectors.
- **status:** fixed — cause was that `.review-title`'s margin-bottom did double duty as the gap before `.review-subtitle` (which had no margin-top of its own), whereas `.gate-subline`/`.trust-key` own their gap via margin-top and their headings sit at margin-bottom: 0.
  Zeroing `.review-title`'s margin outright (style.css:2563-2566) without compensating would have collapsed the h1/subtitle gap, so the fix moved the 0.3rem to `.review-subtitle`'s margin-top (style.css:2567-2569) instead — same visible spacing, but now all three detail-pane `<h1>`s are `margin-bottom: 0` and the gap-to-next-element is consistently owned by the following sibling, matching the gate/trust convention.

### F2-004 — AsciiDoc block wrappers reach the client as classless `<div>`s (ammonia strips their classes)

- **route:** `/tree?ref=HEAD&path=guide.adoc&view=preview`
- **source:** `crates/git-forge-ui/src/render.rs:36-40` (`clean(&convert_with(...))` — `ammonia::clean` with default config)
- **evidence:** `target/ui-audit/phase0-verify/light/tree__ref=HEAD_path=guide_adoc_view=preview/page.html` shows `<div class="rendered-file"><div>` then two more nested unclassed `<div>`s before the first `<p>` — three levels of unclassed `<div>` wrap each paragraph/section, where the `asciidoc-html5` converter's own output would normally carry classes like `paragraph`, `sect1`, etc.
  The capture tooling's own `styles.json` confirms this: it queries `.paragraph` and `.paragraph > p` and gets `"total": 0, "captured": 0` in every AsciiDoc capture (e.g. `target/ui-audit/phase0-verify/light/tree__ref=HEAD_path=guide_adoc_view=preview/styles.json`), i.e. those classes don't exist anywhere in the delivered DOM.
- **phase:** 2
- **severity:** low (currently harmless — the unclassed `div`s carry no margin/padding since nothing targets bare `div`, so they don't contribute to F2-001 — but it forecloses ever writing AsciiDoc-block-specific CSS without first adjusting the sanitizer allowlist, and it's the reason F2-001's fix has to happen at the selector/converter level rather than by targeting `.paragraph` directly)
- **proposal:** none required to fix current spacing; flagged for awareness in case a future pass wants block-level AsciiDoc styling (admonitions, quote blocks, etc.) distinct from Markdown's.
- **status:** reviewed, no code change needed (see proposal)

### F2-005 — Nav-column `<h2>` and detail-pane `<h2>` use different font sizes under the same bare selector, by design (recorded to close out the sweep, not a bug)

- **route:** `/annotations`, `/gates`, `/members`, `/releases`, `/reviews`
- **source:** `crates/git-forge-ui/assets/style.css:252-255` (base `h2 { font-size: 1.15rem }`) vs `style.css:523-525` (`.route-nav-header h2 { font-size: 1rem }`)
- **evidence:** `target/ui-audit/phase0-verify/light/gates/styles.json` → `h2` array has two font-sizes present, `16px` (1rem, nav header) and `18.4px` (1.15rem, detail pane); same split reproduced in `annotations`, `members`, `releases`, `reviews`, and their `dark/` equivalents; margin-bottom is consistently `4px` (0.25rem, the shared base-rule value) across both groups, so only the size differs, not the rhythm
- **phase:** 2
- **severity:** low (traced to an explicit, purposeful override — `.route-nav-header h2` — not scale drift; recorded only so this doesn't get re-flagged as an open question in a later pass)
- **proposal:** none.
- **status:** rejected

## Phase 3 Findings — Visual markers, states, and affordances

Reference check: `Git Forge v5.dc.html` is present at the repo root (confirmed via `ls`, 132356 bytes, untracked as expected).
Its embedded `:root` token block is byte-for-byte identical to `crates/git-forge-ui/assets/style.css:5-40` and the `[data-theme]` overrides at `style.css:124` / `:162`, so the running app's palette matches the v5 comp; findings below that reference "the comp" mean this file.

Capture corpus used: `target/ui-audit/phase0-verify/index.json` (66 captures, light+dark, Chromium 151).
Screenshots opened: light/dark `root`, `issues`, `issues/audit-issue-1`, `reviews`, `reviews/audit-review-1/decision`, `gates`, `releases/no-such-release`, `annotations/no-such-annotation`, `palette`, `members/alice`, `inbox`, `search`, `comments/new`, `tree?...view=source`, `reviews/new` — 16 images, both themes represented.

---

## F3-001 — Raw HTML markup leaks as literal text on the review decision form

- **route:** /reviews/audit-review-1/decision
- **source:** crates/git-forge-ui/src/pages/reviews.rs:644 (also :183, :201, :203 — same
  pattern, not independently screenshot-verified because those labels sit behind the
  "sign in to request a review" auth gate on `/reviews/new`, see
  `target/ui-audit/phase0-verify/light/reviews_new/page.png`)
- **evidence:** `target/ui-audit/phase0-verify/light/reviews_audit-review-1_decision/page.png` — the field label reads literally `Message <span class="muted">(optional)</span>` instead of rendering "Message (optional)" with "(optional)" dimmed.
  The label string is being passed through as a single escaped text node rather than as markup, so the tag syntax itself is visible to the user.
- **phase:** 3
- **severity:** high
- **proposal:** build the label as actual child markup (`"Message " <span class="muted">"(optional)"</span>`)
  instead of a single string containing an embedded `<span>`, matching how other
  templated content in this codebase composes markup.
- **status:** fixed — converted every label using this pattern to real child markup (not just the three cited sites, which all share the same root cause): reviews.rs:183,201,203,644; comments.rs:70; issues.rs:75,90,92,94; releases.rs:55.
  Each now reads `<label for="...">"Field " <span class="muted">"(qualifier)"</span></label>` instead of a single string with an embedded `<span>` tag.
  `cargo build -p git-forge-ui` passes.

### F3-002 — Error states render with three different, inconsistent visual treatments

- **route:** /gates (error), /releases/no-such-release (not-found), vs. /issues,
  /comments, /query, /annotations, /local (established error component)
- **source:**
  - crates/git-forge-ui/src/pages/gates.rs:90-95 — bare `<h1>"Gates unavailable"</h1>`
    - `<p class="muted">` with no border/box.
  - crates/git-forge-ui/src/pages/releases.rs:172 — `<p class="empty">"Annotated tag not found."</p>`,
    styled only by `.empty { color: var(--text-muted); }` (assets/style.css:1225-1227), i.e.
    dimmed text with no container at all.
  - Compare to the app's actual designed error component, `.error-panel`
    (assets/style.css:1212-1224, bordered/padded box), used consistently in
    crates/git-forge-ui/src/pages/issues.rs:68,421,466,518,541,573,599,
    comments.rs:49,232,269,286,332,370, query.rs:109, annotations.rs:307,343, local.rs:184,
    files.rs:475,763.
- **evidence:** `target/ui-audit/phase0-verify/light/gates/page.png` and
  `target/ui-audit/phase0-verify/light/releases_no-such-release/page.png` — neither shows
  any of the bordered-box/icon treatment the rest of the app uses for errors; both fall
  through to plain unstyled text, which is exactly the kind of "no designed appearance"
  gap Phase 3 is asked to check for on a JS-free, server-rendered app.
- **phase:** 3
- **severity:** medium
- **proposal:** route these two failure paths through `.error-panel` (or a dedicated
  not-found variant) instead of ad hoc `<h1>`/`<p class="empty">` markup.
- **status:** fixed — gates.rs:91-96 (a genuine failure — gate evaluation errored) now renders through
  `.error-panel` (`<strong>` heading + `<p>`), same as issues/comments/query/annotations/local. releases.rs:172
  is a "the requested tag doesn't exist" case, not a failure, so instead of `.error-panel` it was routed
  through the app's actual established not-found component, `.empty-panel` with an `<h2>` + back link
  (`<div class="empty-panel"><h2>"Annotated tag not found"</h2><a href="/releases">"Back to releases"</a></div>`),
  matching issues.rs:418, reviews.rs:727, members.rs:384/469 rather than being folded into the error
  treatment it doesn't semantically belong to.

### F3-003 — Modal/overlay scrim and shadow hardcode colors instead of using the app's own scrim/shadow tokens

- **route:** N/A at capture time — the confirm-action modal and `/palette` command overlay
  are opened via `:target`/`.is-open`, which the phase-0 capture script does not trigger
  (`target/ui-audit/phase0-verify/light/palette/page.png` shows the plain "Jump" fallback
  page, not the open overlay), so this is a source-level finding, not a screenshot one.
- **source:** assets/style.css:1837 `.confirm-scrim { background: rgba(6, 8, 4, 0.6); }`,
  :1845 `.confirm-modal { box-shadow: 0 20px 60px #000000a0; }`, :2341 `.palette-scrim`
  same `rgba(6, 8, 4, 0.6)`, :2354 `.palette-panel` same `#000000a0` shadow — all four sit
  well outside the `[data-theme]` blocks (:124/:162) yet the file already defines `--scrim`
  and `--shadow` custom properties for exactly this purpose, each with distinct light and
  dark values (`--scrim: #0a0b08cc` dark / `#3a382ea8` light, `--shadow: #00000080` dark /
  `#2a251826` light, assets/style.css:33-34 and :100-101). `Git Forge v5.dc.html` also
  defines these tokens but its scrim/shadow rules were not diffed further since the comp
  is a static single-state document.
- **phase:** 3
- **severity:** medium
- **proposal:** replace the four hardcoded values with `var(--scrim)` / `var(--shadow)` so
  the confirm dialog (used for destructive actions) and command palette pick up the
  theme-appropriate scrim weight instead of a single dark-mode-tuned value in both themes.
- **status:** fixed — all four sites (style.css: two `.confirm-scrim`/`.palette-scrim` `background`
  declarations, two `.confirm-modal`/`.palette-panel` `box-shadow` declarations) now read
  `var(--scrim)` / `var(--shadow)` instead of the literal `rgba(6, 8, 4, 0.6)` / `#000000a0`.

### F3-004 — `.danger-button:hover` hardcodes a color instead of using a token

- **route:** any page with a delete action (e.g. /issues/audit-issue-1, /comments/*)
- **source:** assets/style.css:829 `.danger-button:hover { background: #c14e44; }`, inside
  a rule block that otherwise reads `--danger` (`var(--danger)` at :826). `#c14e44` is not
  present anywhere in `Git Forge v5.dc.html`, so the comp doesn't call for this literal
  either — it looks like a one-off value that was never tokenized.
- **phase:** 3
- **severity:** medium
- **proposal:** derive the hover color from `--bad`/`--danger` (e.g. `color-mix`, already
  used elsewhere in this file at assets/style.css:1157) instead of a literal hex.
- **status:** fixed — `.danger-button:hover` (style.css:829-831) now reads
  `background: color-mix(in srgb, var(--danger) 85%, black);` instead of the literal `#c14e44`,
  so the hover state derives from the token and darkens correctly in both themes
  (`--danger` resolves to `--bad`, `#e0655a` dark / `#b02f24` light).

### F3-005 — No `:disabled`/`[disabled]` styling exists anywhere in the stylesheet

- **route:** none of the 66 captured pages happen to render a `disabled` attribute
  (`grep -rc disabled target/ui-audit/phase0-verify/*/*/page.html` returns no matches), so
  this is a latent gap rather than an observed regression.
- **source:** `grep -nE ':disabled|\[disabled\]' crates/git-forge-ui/assets/style.css`
  returns nothing in the 3194-line file; the only global interaction-state rule is the
  `:focus-visible` block at assets/style.css:224-229, with no disabled counterpart.
- **phase:** 3
- **severity:** low
- **proposal:** add a `:disabled`/`[disabled]` rule (reduced opacity + `cursor: not-allowed`,
  using existing `--faint`/`--fainter` tokens) before any disabled control ships, since the
  browser default will not respect the dark theme.
- **status:** fixed — added a global `:disabled, [disabled] { opacity: 0.5; color: var(--faint); cursor: not-allowed; }` rule (style.css, right after the `:focus-visible` block at :224-229), so any disabled control will respect the dark theme instead of falling back to the browser default.

### F3-006 — Same-meaning "open/closed" marker uses two different visual vocabularies on Issues vs. Reviews

- **route:** /issues vs. /reviews
- **source:** crates/git-forge-ui/src/pages/issues.rs:132 — open state is a colored
  `<span class="status-dot ok">` placed before the title (row-top, baseline-aligned with
  `strong.row-title`). crates/git-forge-ui/src/pages/reviews.rs:241-254 — open state is
  instead a text pill `<span class="nav-row-status ok">"open"</span>` in the row's
  top-right corner (assets/style.css:2687-2699 `.nav-row-status` / `.nav-row-status.ok`),
  with no dot at all.
- **evidence:** `target/ui-audit/phase0-verify/light/issues/page.png` (green dot to the
  left of "UI audit issue") vs. `target/ui-audit/phase0-verify/light/reviews/page.png`
  (green "OPEN" text label at the top-right of the row, no dot) — same underlying concept
  (item open/closed) rendered with unrelated marker types and unrelated placement across
  two adjacent, structurally similar list views.
- **phase:** 3
- **severity:** medium
- **proposal:** pick one marker convention (dot next to title, or corner label) for
  open/closed state and apply it to both lists.
- **status:** fixed — standardized on "dot next to title" (the convention Issues already used, and which Gates also already uses via `.nav-row-top > .nav-row-dot + .nav-row-title`, gates.rs:42).
  Reviews' row (reviews.rs:239-258) dropped the `.nav-row-status` corner-pill span entirely and now wraps `.nav-row-dot` + `.nav-row-title` in `.nav-row-top` the same way Gates does, reusing the row's existing `--nav-edge` custom property (already being set per-row for the left-edge accent, reviews.rs `edge_style`) so the dot picks up the same ok/faint color the edge stripe already encoded — no new color logic needed.
  The anchor-kind pill that used to share the top row with the status pill now sits to the row's right via an inline `margin-left:auto` (kept out of a shared CSS class to avoid colliding with `.anchor-kind`'s own color/font-size rules).
  `.nav-row-status`/`.nav-row-status.ok`/`.nav-row-status.neutral` (style.css:2687-2699) are now unused by any page and were left in place rather than deleted, since removing dead CSS wasn't part of this finding's scope.

### F3-007 — Keyboard focus rings and hover/active affordances are consistently present (no finding, noted for completeness)

- **route:** shell-wide
- **source:** assets/style.css:224-229 defines one global `button/a/input/select/textarea/.source-view:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }` rule, and every interactive class checked (`.nav-link`, `.rail-icon-btn`, `.theme-toggle`, `.button-link`, `.danger-button`, `.list-row`, `.palette-trigger`, `.palette-row`, `.view-toggle a`, `.line-number`, `.line-comment`) is either a native `<a>`/`<button>` covered by that global rule or has its own explicit `:focus-visible` rule (e.g. assets/style.css:1472-1473, :2310-2311, :2375-2376, :2386-2387, :2417-2418).
  Checked because Phase 3's brief specifically calls out focus rings as easy to lose to a CSS reset; did not find that here across the icon rail, top bar, or list rows.
- **evidence:** `target/ui-audit/phase0-verify/light/root/page.png` /
  `target/ui-audit/phase0-verify/dark/root/page.png` — rail icons, theme toggle, and
  search box all use the same visible-focus-capable elements.
- **phase:** 3
- **severity:** low
- **proposal:** none — recorded so a future pass doesn't re-spend budget re-checking this.
- **status:** rejected
