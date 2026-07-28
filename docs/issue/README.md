# Archived issues

Issue bodies exported verbatim from the `refs/forge/issue/*` entities in each
family repository, one Markdown file per issue, grouped by the repository the
issue belongs to.

## Why these exist

Issues are stored as `git-store` entities on refs, not as files. The Wave 1
format break changes the on-disk encoding, so every entity written under the
old encoding has to be re-stored to stay readable — and `git forge` is itself
how these issues are read. Exporting the bodies first means the record
survives the break, and survives the refs being deleted once an issue is
resolved.

These were read with `git ls-tree` / `git cat-file` plumbing rather than the
`git-forge` binary, so the export does not depend on any particular encoding
remaining readable.

## Fidelity

Bodies are byte-for-byte as stored; no reflowing, no spelling correction. Ten
issues had been edited after filing, and their superseded revisions are
preserved in a trailing section of the same file rather than discarded.

`docs/issue/` is excluded from `rumdl` and `typos` in `.config/` for exactly
this reason: a formatter that rewrites an archive defeats the archive.

## Contents

### git-store (18)

- [`0b4a9b27`](git-store/0b4a9b27.md) — The Schema: trailer is not a DAG edge, so fetched or pushed data arrives unreadable (open) — **resolved**
- [`24010918`](git-store/24010918.md) — put_schema accepts any document, so a kind can be published permanently broken or silently incompatible (open)
- [`3f0fd510`](git-store/3f0fd510.md) — Entity naming is unmodelled: store_anonymous is dead code patching a gap the schema should fill (open)
- [`4e2923ac`](git-store/4e2923ac.md) — A schema cannot express that a field is required, so put silently accepts partial values (open)
- [`5b39f084`](git-store/5b39f084.md) — Leaf blobs carry no trailing newline, so every git diff of stored data is half No-newline markers (open)
- [`5e77f97d`](git-store/5e77f97d.md) — Schema-read errors carry no location, while schema-write errors carry a full path (open)
- [`7c97f623`](git-store/7c97f623.md) — The CLI has no way to inspect the schema binding, diff two revisions, or emit machine-readable output (open)
- [`8d109650`](git-store/8d109650.md) — Unit-like values are invisible to every recursive Git tool, which breaks the git-is-the-query-language pitch (open) — **resolved**
- [`a5eae21d`](git-store/a5eae21d.md) — The repository landing page is facet-git-tree crate documentation, not a pitch for git-store (open) — **resolved**
- [`ad5a33d7`](git-store/ad5a33d7.md) — The schema authoring surface is a Rust enum leaked through facet-json, not a format anyone would hand-write (open)
- [`b73019c6`](git-store/b73019c6.md) — The specification stops at facet-git-tree, leaving the ref layout and schema binding - the actual contract - unspecified (open)
- [`cddc2bb1`](git-store/cddc2bb1.md) — One loose object per scalar and a single flat tree per sequence give a punishing cost profile at modest sizes (open)
- [`cf20051d`](git-store/cf20051d.md) — Store is a concrete type welded to a gix::Repository, and its configurable prefixes are write-only (open)
- [`d4f8aaaf`](git-store/d4f8aaaf.md) — SchemaDoc has no version marker of its own, so the self-hosting has no bootstrap or upgrade path (open)
- [`ddaec54e`](git-store/ddaec54e.md) — The value cargo feature gates correctness, not dependencies: the default build can only read lossily (open)
- [`e1f29b33`](git-store/e1f29b33.md) — Git tree entry modes are a free type channel the encoding never uses, which is why the dynamic read must guess (open)
- [`e5a9305e`](git-store/e5a9305e.md) — Positional ordinal keys make a single list insertion look like a rewrite of the whole list (open)
- [`fde28f29`](git-store/fde28f29.md) — First five minutes: git store --help fails, and a fresh repo gives you a blank prompt with no next step (open)

### git-anchor (16)

- [`0118e93d`](git-anchor/0118e93d.md) — The primitive-plus-consumer split does not hold: three fields in the anchor storage record exist only for the comment layer vocabulary (open)
- [`05477afe`](git-anchor/05477afe.md) — Anchor conflates committed and working-tree captures, so a worktree anchor cannot be projected onto a revision and fails with a false no-file-at-path error (open)
- [`07a4fef8`](git-anchor/07a4fef8.md) — Every read path is a linear scan of all refs plus a full decode of every note, so a single lookup costs the whole store; the LSP pays it on every keystroke (open)
- [`13439f72`](git-anchor/13439f72.md) — Reply ordering uses a nanosecond wall clock, which is false precision across machines; causal order is already in the data and unused (open)
- [`17ceddc9`](git-anchor/17ceddc9.md) — The stored record is an untagged tree whose type is recovered by sniffing entry names, with no format version and no shared envelope with the sibling git-store (open)
- [`1e191df7`](git-anchor/1e191df7.md) — Outdated is a catch-all: a whitespace reformat, a split span, and a verbatim move to another file all report the same terminal outcome as a genuine rewrite (open)
- [`26b5a4d6`](git-anchor/26b5a4d6.md) — The id-suffix grammar overloads git punctuation with two different meanings, has no long-form equivalent, and silently accepts an empty id (open)
- [`316fad99`](git-anchor/316fad99.md) — Eight requirement IDs cited normatively across gix-comment-lsp do not exist in the specification, and the entire comment and lens layer is unspecified (open)
- [`6684400a`](git-anchor/6684400a.md) — git anchor add on a span that already has a note silently replaces it: binding-keyed identity makes add an upsert (open)
- [`741e9fec`](git-anchor/741e9fec.md) — The compose flow launders a comment through a file in .git/ and depends on the client routing didSave back to this server; if it does not, the typed comment is lost with no error (open)
- [`7c0a7cfd`](git-anchor/7c0a7cfd.md) — Retention pins the anchored bytes but not the anchored commit, so exact projection can vanish; the fuzzy fallback that replaces it is strictly worse than a diff against the bytes already embedded (open)
- [`a22590a7`](git-anchor/a22590a7.md) — list is a dump, not a query surface: no path, state, or projection filters; thin JSON; exit code 0 for outdated and deleted; and it panics when piped to head (open)
- [`b1c4e009`](git-anchor/b1c4e009.md) — One ref per anchor, keyed by blob oid, gives inspectability but no rollup, no path index, and a ref count that scales with review volume (open)
- [`b99d4bea`](git-anchor/b99d4bea.md) — The editor never shows a comment whose file was renamed, and renders outdated comments at their capture-time line numbers (open)
- [`dbffc16c`](git-anchor/dbffc16c.md) — Anchors and comments do not travel: no fetch refspec, no push or fetch porcelain, and no mention of distribution anywhere in the repository (open)
- [`f5f2764d`](git-anchor/f5f2764d.md) — A comment author and date are read from the ref tip, so resolving or editing someone else comment reassigns its authorship (open)

### git-query (20)

- [`046b3ecf`](git-query/046b3ecf.md) — No revspec resolution: every Rev and Oid argument must be a full 40-hex object id, so the README demo cannot run (open)
- [`08d48d0a`](git-query/08d48d0a.md) — The mode-violation error points the user at git query predicates, which does not print modes (open)
- [`1071410b`](git-query/1071410b.md) — git query predicates omits modes, types, and every builtin, so bind and bind_fuzzy are undiscoverable (open)
- [`16b85bb6`](git-query/16b85bb6.md) — rules add stores before validating, so one bad module wedges every query in the repository, and there is no rules rm (open)
- [`3fbf1987`](git-query/3fbf1987.md) — The Nemo dependency is a pinned nightly plus an unpublishable git rev with 484 lockfile entries, and there is no vendoring or offline fallback (open)
- [`47bc3643`](git-query/47bc3643.md) — The gate story has no artifact: no hook, no CI snippet, and gix-query exports neither the footprint nor the acyclicity verdict it is credited with (open)
- [`49e0324a`](git-query/49e0324a.md) — A stored rule module records only its source text: no language version, no declared dependencies, no way to tell a stale module from a broken one (open)
- [`560bb11b`](git-query/560bb11b.md) — No epoch CAS and no recorded program-identity snapshot: concurrent rule pushes race, and a decision cannot be replayed (open)
- [`62fbac7d`](git-query/62fbac7d.md) — Every demand round re-parses the program and re-serializes the whole fact set to CSV from cold, and explain pays for a third full run (open)
- [`7178f702`](git-query/7178f702.md) — Tier 2 is built in the library and absent from the CLI, along with every flag the spec defines for run (open)
- [`786a9937`](git-query/786a9937.md) — Nine crates for 18k lines: one is orphaned, one is a 163-line facade, and the boundary doing the most work is undersold (open)
- [`a3124179`](git-query/a3124179.md) — Every diagnostic computes a span and every renderer throws it away: no line, no column, no caret (open)
- [`a6238508`](git-query/a6238508.md) — Exit code 3 and the row cap are stated as behavior in the README and the normative spec, but no producer exists (open)
- [`a95c9bbc`](git-query/a95c9bbc.md) — Author order is the SIPS but the author gets no feedback: no plan output, no cost warning, no way to see the rewrite (open)
- [`cde2b225`](git-query/cde2b225.md) — Output has no ordering contract, no column headers, and no machine-readable format, while the tests sort before asserting (open)
- [`d0309410`](git-query/d0309410.md) — Derived predicates get one required-bound ArgSet unioned over their rules, while EDB predicates get a ModeSet: no mode polymorphism above the base layer (open)
- [`d1c29128`](git-query/d1c29128.md) — The cache key does not exist in code, and the key the spec describes omits the Nemo pin (open)
- [`d6f515a3`](git-query/d6f515a3.md) — No trust root for refs/meta/rules/*: the kernel that would supply one is an orphan crate with no dependents and no hook (open)
- [`feabf985`](git-query/feabf985.md) — explain is fully built in gix-query-eval and reachable from nothing: no subcommand, no library export (open)
- [`ffd96170`](git-query/ffd96170.md) — The README calls the engine boundary a seam; DEVPLAN 2.11 explicitly rejects one, and the real boundary is program text plus CSV (open)

### git-forge (15)

- [`02b94000`](git-forge/02b94000.md) — Issue bodies are parsed as AsciiDoc but nothing says so, and show is a lossy re-render with no raw escape hatch (open)
- [`1f25493d`](git-forge/1f25493d.md) — Concurrent edits on two clones cannot be merged: force-fetch silently loses one side, non-force fetch is rejected outright (open)
- [`22b36108`](git-forge/22b36108.md) — Forge data does not travel: a fresh clone reports zero issues because there is no fetch or push refspec for refs/forge/* (open)
- [`2a27866f`](git-forge/2a27866f.md) — query is two disconnected surfaces: ad-hoc in-process scans that duplicate a real Datalog engine, and a Datalog engine nobody can discover how to address (open)
- [`3a235525`](git-forge/3a235525.md) — No machine-readable output: list is a box-drawing table even off a TTY, show panics on a closed pipe, and nothing sorts or filters (open)
- [`456fee55`](git-forge/456fee55.md) — Two competing identity systems and no place to declare a project vocabulary for labels, assignees or members (open)
- [`5f152df4`](git-forge/5f152df4.md) — Every write discards the commit message, and issue log is strictly worse than git log -p on the same ref (open)
- [`6872199e`](git-forge/6872199e.md) — Id resolution is inconsistent across verbs, and issue log silently reports an empty history for a prefix it failed to resolve (open)
- [`8990ad01`](git-forge/8990ad01.md) — Comments are a single overwritable edit-reason slot, not a thread, so the forge has no discussion surface (open)
- [`94c80ae5`](git-forge/94c80ae5.md) — Entity creation validates the wrong things: bodies are mandatory while titles are optional, and review targets accept any string as a commit oid (open)
- [`a5a99ef1`](git-forge/a5a99ef1.md) — install is named after installing the tool but initialises the repository, and is redundant for everything except the rules that do not work (open)
- [`bc5de31d`](git-forge/bc5de31d.md) — No README and no help text on any subcommand or flag, so the project is undiscoverable from both the repo and the binary (open)
- [`cc2cec6f`](git-forge/cc2cec6f.md) — rm silently and irrecoverably destroys an entity, which is almost never what a forge user wants and is not what close means (open)
- [`d87da1ce`](git-forge/d87da1ce.md) — The published schema describes a different type than the one actually stored, and is republished on every write (open)
- [`e25192e7`](git-forge/e25192e7.md) — Shipped review rules query a claims namespace git-forge never writes, so review data can never affect a policy answer (open)

## Resolved

Three issues were fixed together in the Wave 1 format-break window, by
`git-store` commit `647869c`:

- `0b4a9b27` — bind a data commit's schema by subtree, not by trailer
- `8d109650` — encode unit enum variants as blobs; mark otherwise-empty trees
- `a5eae21d` — promote the git-store pitch to the repository root

Their `status:` frontmatter still reads `open` because that is what the ref
said at export time; the export records the refs faithfully rather than
editorialising them.
