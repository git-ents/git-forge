# Handoff: Git Forge UI (v5)

## Overview

A single-workspace git forge UI whose UX is grounded in the actual data model — refs pointing at immutable objects, and signed statements that outlive code changes — rather than imitating a generic PR tool.
It is designed as the counterpart to Zed: Zed is where code is written and browsed; Forge is the durable, shared, signed record of what was said and decided about it.

## About the Design Files

The bundled `.dc.html` file is a **design reference built in HTML** — an interactive prototype showing intended structure, states, and behavior.
It is not production code to copy directly.
The task is to **recreate this design in the target codebase's existing environment** (React, Vue, Swift, etc.) using its established patterns, components, and libraries — or, if no frontend exists yet, to pick the framework best suited to the project and implement there.
Open the `.dc.html` file directly in a browser to interact with it; it is self-contained.

## Fidelity

**High-fidelity.**
Colors, type, spacing, and states below are literal values pulled from the design file — implement pixel-for-pixel, not "in the spirit of."

## Domain context

Anchors are the git-anchor content-addressed binding scheme (`github.com/git-ents/git-anchor`, see `github.md` at the project root): a comment or claim binds to a `(kind, coordinates)` pair captured at some oid, and an oracle chain re-projects that binding onto new revisions with a confidence score. `bound`/`projected` (clean re-projection above threshold), `drifted`/`rebound` (resolved but the underlying code changed), `orphaned` (no oracle cleared threshold — needs a human), `pinned` (fixed forever by a signed attest claim, oracles can no longer override it).

## Surfaces (routes)

Left icon rail (52px, inline SVG paths — do not use a sprite `<use>`) switches between eight routes, plus an always-present "Local" strip that is never a route.

1. **Inbox** — grouped by *lifecycle event*, not urgency: Asked (needs your reply), Said (replies to you), Decided (signed since you last looked), Drifted (bindings that moved), Blocked (gates/reviews stuck on you).
   A "while you were away" digest block sits above the groups.
   This is the app's front door — home to reasoning about what happened, not a generic notification feed.
2. **Code** — a lens onto annotations, not a file-authoring surface.
   Left rail lists files with per-file annotation notes; selecting one shows syntax-highlighted lines with inline comment threads interleaved between rows, each thread carrying a binding-state badge (bound/drifted/orphaned/pinned) and anchor id.
   A banner states plainly: "Full source lives in Zed — this view exists to show comments and bindings alongside it, not to replace it."
   Deep links run **Zed → Forge**: a comment in Zed (via LSP) opens straight to its Annotations detail in Forge — never the reverse.
3. **Issues** — standard master/detail, signed comment threads, anchor location where applicable.
4. **Reviews** — target pinned by oid (not a moving branch); four bands: request (what's being asked), discussion (threads), decision ledger (signed verdicts, stale ones shown as quiet muted lines once the target has moved), and status (owner-set manually, never computed — with a visible disclaimer that it's a human's assertion, not an automated check).
5. **Annotations** (formerly "Margins"/"Remarks") — the unified surface for every comment, anchored or free-floating, repo-wide.
   Filterable by binding health (bound/drifted/orphaned/pinned/unattached).
   Selecting an orphaned/drifted item offers "repair mode": pick from oracle candidate spans and pin, or mark obsolete — each a signed attest claim.
   Inbox's "drifted" group deep-links here to enter repair mode directly.
6. **Gates** — CI/merge-gate state, independent from Releases.
7. **Releases** — typed documents: annotated tags with a body and optional attachments, each with its own signed-claim ledger (signer, key, date, valid/revoked) and quorum bar — structurally similar to Reviews' ledger but a distinct concept (a release is a claim about a point in history, not a request for one).
8. **Trust** — members directory: identities, key fingerprints, and orphaned attributions (signed history from identities no longer active/known).
9. **Local strip** (ambient, not a route) — always visible, shows how uncommitted local work would affect the signed record if pushed (dirty files, unpushed commits, gate predictions).
   It is a lens on consequence, not a preflight checklist.

Query is unified into one search/command surface (⌘K), indexing repos, refs, paths, issues, reviews, and annotations.

## Non-negotiables

1. **Density over comfort.**
   Engineers reading code and signed records.
   Small type (12–13px chrome), tight line-height, hairline dividers, no decorative whitespace.
2. **Semantic color is load-bearing, never repurposed.**
   Green = signed/valid/approved/bound.
   Amber = dirty/drifted/needs-attention.
   Red = unsigned/fail/revoked.
   Blue = object ids.
   Mauve = commit-kind anchors / ui-scope labels.
   Lavender accent = brand/active-nav/primary-action *only* — never used to mean "good."
3. **Square-ish geometry.**
   8px cards, 10px modals, 5px controls, 3–4px pills/status dots at 50%.
   No large radii, shadows only on modals.
4. **Monospace is the default face** (`ui-monospace, "SF Mono", Menlo, Consolas, monospace`) for all chrome, code, refs, ids, metadata.
   **Lora** (serif, Google Fonts 400–700+italic) only for page headings, issue/review titles, and comment/issue body prose — weight 600, `letter-spacing:-.01em` on headings.
   `system-ui` only where already used for small labels.
5. **Object ids are always visible inline** — commit oids, anchor ids, key fingerprints — never hidden behind tooltips.
6. **Every mutating action is signed and routes through one shared confirm dialog** showing the target ref and signing identity.
   No silent writes.
   The confirm dialog should show the actual ref that will move (e.g. `refs/forge/comment/8fa02c1`, `refs/tags/<tag>`) — this is a "write confirmation showing the actual ref move," not a generic "Are you sure?".
7. **History is available but not in your face.**
   Edited items show an "edited N×" link that reveals prior signed versions on demand, rather than a visible diff/changelog inline.
8. **Reviews never compute status.**
   The status band is a manual field the owner sets, always paired with a small disclaimer that it is asserted, not derived.
9. **Light/dark theme supported via `data-theme` attribute** (see token table); default follows `prefers-color-scheme`.

## Design Tokens

### Color — dark (default)

| Token | Value | Use |
| --- | --- | --- |
| `--bg` | `#12140f` | app background |
| `--rail` | `#0e100b` | icon rail, list panes, inputs, card headers |
| `--surface` | `#161a12` | cards, top bar, modals |
| `--row` | `#1c2016` | hover fill, in-card row background |
| `--line` | `#2b2f24` | hairlines, control borders |
| `--line2` | `#3d4232` | hover borders, stronger dividers |
| `--text` | `#e8e6d9` | primary text |
| `--text2` | `#d8d5c6` | secondary text |
| `--muted` | `#a8ab99` | metadata |
| `--dim` | `#8b8f7d` | secondary labels |
| `--faint` | `#5d6152` | placeholders, least-important ids |
| `--fainter` | `#4d5144` | barely-there metadata |
| `--acc` | `#b3a3e6` | active nav, primary buttons, links, branch pill |
| `--acc-h` | `#c9bdf0` | link/accent hover |
| `--acc-on` | `#12140f` | text on accent fill |
| `--acc-b` | `#b3a3e640` | accent border/selection tint |
| `--acc-t` | `#b3a3e614` | active row fill |
| `--ok` | `#8fbf5f` | signed/valid/approved/bound/would-pass |
| `--ok-b` / `--ok-t` | `#8fbf5f55` / `#8fbf5f14` | ok border / bg tint |
| `--warn` | `#dfae4f` | dirty/drifted/changes-requested |
| `--warn-b` | `#dfae4f44` | warn border/bg |
| `--bad` | `#e0655a` | unsigned/revoked/would-fail |
| `--bad-b` | `#e0655a55` | bad border/bg |
| `--oid` | `#6ea3d8` | commit/anchor/object ids, blob-kind anchors |
| `--pink` | `#c792ba` | commit-kind anchors, macro/attribute syntax |
| `--scrim` | `#0a0b08cc` | modal overlay |
| `--shadow` | `#00000080` | modal shadow color |

Syntax tokens: keyword `--s-key #e0655a` · function `--s-fn #8fbf5f` · type `--s-type #dfae4f` · string `--s-str #b3c968` · const/macro `--s-const #c792ba` · operator `--s-op #9a9e8a` · comment `--s-cmt #6b705c`.

### Color — light (`[data-theme="light"]` or `prefers-color-scheme: light`)

| Token | Value |
| --- | --- |
| `--bg` | `#f7f6f0` |
| `--rail` | `#efeee4` |
| `--surface` | `#ffffff` |
| `--row` | `#f2f1e7` |
| `--line` / `--line2` | `#e0ddce` / `#c6c3b0` |
| `--text` / `--text2` | `#24261d` / `#33362a` |
| `--muted` / `--dim` / `--faint` / `--fainter` | `#55594a` / `#6b7060` / `#878c79` / `#9ba08d` |
| `--acc` / `--acc-h` / `--acc-on` | `#6247ab` / `#4c3690` / `#ffffff` |
| `--ok` / `--warn` / `--bad` | `#4a7420` / `#8a6209` / `#b02f24` |
| `--oid` / `--pink` | `#265b90` / `#8a4a7c` |

Status pill pattern (either theme): text in the semantic color, background same hex + low alpha, uppercase, weight 700, `letter-spacing:.05em`, 4px radius.

### Type

- Mono (default): `ui-monospace, "SF Mono", Menlo, Consolas, monospace`.
- Serif: `Lora` — headings, titles, prose bodies only.
- Scale (rem): section heading 1.1–1.5 · card title .9 · body prose .82–.95 · row text .76–.82 · metadata .68–.72 · pills/chips .62–.66.
  Chrome line-height 1.6–1.65; prose 1.75.

### Geometry

| Element | Radius |
| --- | --- |
| Cards, panels | 8px |
| Modals, palette | 10px |
| Buttons, inputs, textareas, selects | 5px |
| Pills, chips, kbd | 3–4px |
| Rail count badge | 7px |
| Status/quorum dots | 50% |

Fixed dims: icon rail 52px, rail buttons 36×36 · top bar 44px · Code file rail 15rem · content max-widths ~44–64rem depending on view.
Shadows only on modals (`0 20px 60px var(--shadow)`); overlay scrim `var(--scrim)`.

### Interaction

- Active nav: accent color + `--acc-t` fill + left edge indicator.
- Row hover: `--row` fill.
  Control hover: border → `--line2`, text → `--text`.
- ⌘K / Ctrl+K opens the command palette; Escape closes palette and any open confirm dialog.
- Transitions 150–200ms ease, hover/expand only.

## Data model (as exercised by the design)

```text
Ref/Anchor    { kind: blob|blob-range|tree|tree-range|commit|commit-range,
                targetOid, coordinates, capturedAt,
                state: bound|projected|drifted|orphaned|pinned, confidence }
Comment       { author, age, body, anchor?: Anchor, isFree: bool,
                editHistory[] }   // "edited N×" reveals editHistory
Review        { id, title, author, targetOid, targetShorthand,
                request, threads[], decisions[{signer, key, when,
                verdict, stale: bool}], status /* owner-set, not computed */ }
Release       { tag, date, oid, body, attachments[],
                claims[{signer, key, date, status: valid|revoked}], quorum }
Gate          { name, pass, detail, lastRun }
Member        { name, keyFingerprint, active: bool, orphanedAttributions[] }
Local         { dirty[], unpushed[], gatePredictions[] }  // ambient strip, not a route
SignedAction  { ref, identity }   // the only mutation shape; every write confirms this
```

## Files

- `Git Forge v5.dc.html` — full interactive design reference, all surfaces.
- `github.md` — source repo used for anchor/projection domain vocabulary.

Ask the user if they'd like screenshots of each surface added before implementation.
