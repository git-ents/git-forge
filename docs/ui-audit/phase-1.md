# Phase 1 — Copy and nomenclature audit

Audit the user-facing copy of `git-forge-ui`, a server-rendered HTML app in this Rust workspace.
This phase needs no tooling and no browser.

The core technique: extract every user-visible string into one flat list.
Inconsistent terminology is invisible page-by-page and obvious when several hundred labels sit adjacent.

## Do this

Extract every user-visible string from `crates/git-forge-ui/src/pages/*.rs` and `src/shell.rs` — button labels, headings, link text, placeholder text, empty-state messages, error messages, confirmation prompts, `aria-label` and `title` attributes.
Produce one flat list, each entry with its `file:line`.

Then review the list as a whole against these criteria:

1. **One term per concept.**
   Flag any concept named two ways across the app.
2. **Consistent grammatical form.**
   Buttons should agree with each other — imperative verbs ("Add comment") or nouns, not a mix.
3. **Sentence case discipline**, applied consistently.
4. **No leaked internal vocabulary** — labels naming an implementation concept rather than something a user recognizes.
   Note that a term can *look* internal and be legitimate domain vocabulary; check what it means in the code before flagging it.
5. **Error and empty-state messages** should say what happened and what to do
   next.
6. **Truncation and overflow** behavior on long strings.

Cross-check terminology against the `gix-forge` domain types (`Issue`, `Review`, `Comment`, `Annotation`, `Member`, `Gate`, `Release`) — where the UI and the domain model disagree on a name, say so.

**Do not cross-check against `docs/design-git-forge-ui.adoc` except for layout-shell vocabulary.**
That document is v1.0 and badly stale: it commits to a read-only UI with no mutations (there are 28 POST handlers), describes `/` as a dashboard (it 307-redirects to `/inbox`), and says file browsing was intentionally dropped (`pages/files.rs` exists).
Treating it as a naming authority will manufacture false findings.

## Ground rules

- **Findings go in `docs/ui-audit/findings.md`**, appended the moment you find something.
  Never leave a finding only in conversation.
- **Only report what you can point at.**
  No `file:line`, no finding.
  This is the main defense against invented problems.
- **Do not change code.**
  Finding and fixing are separate jobs; interleaving them corrupts both.
- **Consistency findings outrank taste findings.**
  "These two buttons disagree" is verifiable; "this feels unbalanced" is not, and should be rare and marked `low`.

## Finding format

```markdown
### F-007 — The no-subject comment concept is written three ways

- **source:** crates/git-forge-ui/src/pages/comments.rs:57, :211, :255
- **evidence:** "Free-floating comment", "Free-floating discussion", and
  lowercase "free-floating" all name the same thing — a comment whose
  `subject` is `None`
- **phase:** 1
- **severity:** medium
- **proposal:** pick one term and one casing; apply at all seven sites
  (also issues.rs:352, reviews.rs:515)
- **status:** open
```

`severity` is `high` (broken or misleading), `medium` (inconsistent with the app's own system), or `low` (aesthetic judgment).
`status` is `open`, `fixed`, `rejected`, or `needs-recapture`.
