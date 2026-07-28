---
id: bc5de31d
repo: git-forge
ref: refs/forge/issue/bc5de31d
title: "No README and no help text on any subcommand or flag, so the project is undiscoverable from both the repo and the binary"
status: open
labels: ["docs", "high"]
reporters: ["Claude:claude-opus-5"]
assignees: []
updated: 2026-07-28T10:40:51+09:00
revisions:
  - commit: bc5de31d0e38f2ba082f2ea72555e4864eb3e9f3
    date: 2026-07-28T10:40:51+09:00
---

# No README and no help text on any subcommand or flag, so the project is undiscoverable from both the repo and the binary

**What**

There is no `README.md` and no `docs/` directory. The repository ships
`CONTRIBUTING.md`, `CONDUCT.md`, `COPYRIGHT`, two licences and a `.rules` file
for agents -- everything except an explanation of what the software is.

The binary is no better. Not one subcommand and not one flag carries a `clap`
description. Every `about` and `help` string is absent.

**Where**

- Repository root -- no `README*`, no `docs/`
- `crates/git-forge/src/main.rs:28` -- `Command` variants, no doc comments
- `crates/git-forge/src/main.rs:81` -- `IssueNewArgs`, every `#[arg]` bare

**Evidence**

    $ git-forge
    Commands:
      issue
      review
      comment
      query
      install
      help     Print this message or the help of the given subcommand(s)

    $ git-forge issue list -h
    Usage: git-forge issue list
    Options:
      -h, --help  Print help

`help` is the only subcommand in the tool that describes itself. `issue list`
has a help page that conveys strictly nothing.

**Why it matters**

For a conference repository the README is the talk landing page. Every person
who scans the QR code on the last slide arrives at a file listing and leaves.
That is the entire conversion funnel for two months of work.

The missing help text is the same problem at a smaller radius, and it is worse
than it looks because the CLI has genuinely non-obvious behaviour that only
help text can carry: that bodies are AsciiDoc, that `--label` and `--reporter`
are repeatable, that `query run` predicates need a `review.` module prefix,
that `install` initialises the repository rather than installing the tool, that
`rm` is destructive. Every one of those is currently discoverable only by
reading `main.rs`.

I would also gently suggest the absence is diagnostic rather than lazy. Writing
the README forces the one-sentence version -- what is this, who is it for, why
is it not just `gh`. Several findings in this review are downstream of that
sentence not existing yet: the review command has no verdict, the query rules
cannot see the review data, comments have nowhere to go. Those are not
oversights so much as symptoms of the pitch not having been pinned down. If the
sentence is hard to write, that is a design finding, not a docs chore.

**Options**

- Option A -- Minimum viable README: one-sentence pitch, a ten-line quickstart
  that actually works from `git init`, the refspec incantation needed to make
  data travel, the ref layout, and an honest status section saying what is a
  sketch. Trade-off: an afternoon.
- Option B -- The above plus a `docs/design.md` covering the id derivation, the
  schema contract and the storage format, so a third party could write a reader.
  Trade-off: a day, and it is the document that makes the not locked in claim
  checkable.
- Option C -- Fill in every `clap` `about` and `long_about`, and add a
  `git-forge issue new --help` example block. Trade-off: an hour, mechanical.

**Recommendation**

All three, in the order C, A, B, because C is the cheapest and lands the most
per minute. High confidence. Write the one-sentence pitch first and treat
whatever refuses to fit in it as the design work still outstanding.
