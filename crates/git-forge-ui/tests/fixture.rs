//! UI-audit fixture builder (docs/ui-audit/phase-0.md).
//!
//! `#[ignore]`d by default so `cargo test` doesn't run it. Invoked directly
//! by `scripts/ui-audit/capture.mjs`:
//!
//! ```sh
//! UI_AUDIT_REPO=/path/to/dir cargo test -p git-forge-ui --test fixture \
//!   -- --ignored --exact build_fixture
//! ```
//!
//! `UI_AUDIT_REPO` must point at an empty (or nonexistent) directory; it is
//! created if missing. The directory is left in place -- unlike the other
//! integration test, this one is not backed by a `tempdir()` -- because the
//! capture script boots `git-forge-ui` against it after this process exits.
//!
//! Content here is aimed at the three symptoms the audit is chasing
//! (consecutive-paragraph spacing, list/table/admonition rhythm, permission-
//! varied views) plus the three list empty states (releases, gates,
//! annotations all deliberately stay at zero). Stress cases (huge diffs,
//! binaries, RTL, 300-file directories) are left for a follow-up per the
//! phase-0 brief.

use gix_forge::{Authorization, Comment, EntityOps, Issue, Member, Principal, Review, ReviewTarget};
use test_support::{commit_file, init_repo};

const MARKDOWN: &str = r#"# Prose fixture (Markdown)

This is the first paragraph of a run of consecutive paragraphs. It exists to
let the audit measure the vertical gap the stylesheet puts between adjacent
`<p>` elements with nothing -- no heading, no list -- between them.

This is the second consecutive paragraph, immediately following the first
with a single blank line in the source, exactly as a human would write it.

This is a third consecutive paragraph, so the audit can compare the first
gap against the second and check they match.

## Lists

A nested list, two levels deep:

- Top-level item one
- Top-level item two
  - Nested item two-a
  - Nested item two-b
- Top-level item three

An ordered list:

1. First step
2. Second step
3. Third step

## Table

| Column A | Column B | Column C |
| -------- | -------- | -------- |
| a1       | b1       | c1       |
| a2       | b2       | c2       |

## Admonition

> **Note**
> This is a blockquote used as an admonition. It should read distinctly from
> a plain paragraph.

## Code

Inline code like `let x = 1;` sits inside a sentence here.

```rust
fn fixture() -> &'static str {
    "fenced code block"
}
```

### An h3 heading

Text under an h3.

#### An h4 heading

Text under an h4, followed by a [link to the repository root](/tree) and a
[link to the issues list](/issues).
"#;

const ASCIIDOC: &str = r#"= Prose fixture (AsciiDoc)

This is the first paragraph of a run of consecutive paragraphs in AsciiDoc.
It mirrors the Markdown fixture so the audit can compare paragraph rhythm
across both preview renderers.

This is the second consecutive paragraph, directly following the first with
a single blank line in the source.

This is a third consecutive paragraph, for the same before/after comparison
as the Markdown fixture.

== Lists

A nested list, two levels deep:

* Top-level item one
* Top-level item two
** Nested item two-a
** Nested item two-b
* Top-level item three

An ordered list:

. First step
. Second step
. Third step

== Table

[cols="1,1,1"]
|===
|Column A |Column B |Column C

|a1
|b1
|c1

|a2
|b2
|c2
|===

== Admonition

NOTE: This is a native AsciiDoc admonition block.

[quote]
____
This is a blockquote-style quote block, for comparison against the NOTE
admonition above.
____

== Code

Inline code like `let x = 1;` sits inside a sentence here.

[source,rust]
----
fn fixture() -> &'static str {
    "fenced code block"
}
----

=== An h3-equivalent heading

Text under a level-3 heading.

==== An h4-equivalent heading

Text under a level-4 heading, followed by a link to the
link:/tree[repository root] and a link:/issues[issues list].
"#;

/// Build the phase-0 UI-audit fixture repository at `UI_AUDIT_REPO`.
///
/// `#[ignore]` keeps this out of the default `cargo test` run; it is a
/// content builder, not an assertion.
#[test]
#[ignore = "invoked directly by scripts/ui-audit/capture.mjs"]
fn build_fixture() {
    let repo_dir = std::env::var_os("UI_AUDIT_REPO")
        .map(std::path::PathBuf::from)
        .expect("set UI_AUDIT_REPO to the target directory before running this test");
    std::fs::create_dir_all(&repo_dir).expect("create UI_AUDIT_REPO");
    assert!(
        std::fs::read_dir(&repo_dir).unwrap().next().is_none(),
        "UI_AUDIT_REPO ({repo_dir:?}) is not empty; remove it or pass a fresh directory"
    );

    init_repo(&repo_dir);
    commit_file(&repo_dir, "README.md", MARKDOWN, "add markdown prose fixture");
    commit_file(&repo_dir, "guide.adoc", ASCIIDOC, "add asciidoc prose fixture");
    commit_file(
        &repo_dir,
        "src/lib.rs",
        "pub fn ui_audit_fixture() -> &'static str {\n    \"rust\"\n}\n\npub fn second() -> u8 {\n    2\n}\n",
        "add rust source",
    );
    commit_file(
        &repo_dir,
        "notes/plain.txt",
        "plain text for the source fallback\n",
        "add plain text",
    );

    let repo = gix::open(&repo_dir).expect("open fixture repo");

    // Two members with different roles, for permission-varied views: alice
    // is a maintainer (can delete/administer), bob is a plain member.
    let admin = Authorization::new(Principal::member_id("alice")).administrator();
    Member {
        id: "alice".to_owned(),
        signing_key: "alice".to_owned(),
        role: "maintainer".to_owned(),
    }
    .save_in_repo_as(&repo, &admin)
    .unwrap();
    Member {
        id: "bob".to_owned(),
        signing_key: "bob".to_owned(),
        role: "member".to_owned(),
    }
    .save_in_repo_as(&repo, &admin)
    .unwrap();

    let alice = Authorization::new(Principal::member_id("alice"));
    let bob = Authorization::new(Principal::member_id("bob"));

    Issue {
        id: "audit-issue-1".to_owned(),
        status: "open".to_owned(),
        title: "UI audit issue".to_owned(),
        body: "searchable ui audit issue".to_owned(),
        labels: vec!["ui-audit".to_owned()],
        assignees: vec!["bob".to_owned()],
        reporters: vec!["alice".to_owned()],
        edit: None,
    }
    .save_in_repo_as(&repo, &alice)
    .unwrap();

    Review {
        id: "audit-review-1".to_owned(),
        status: "open".to_owned(),
        body: "searchable ui audit review".to_owned(),
        reviewers: vec!["bob".to_owned()],
        requesters: vec!["alice".to_owned()],
        target: ReviewTarget::parse("commit:0000000000000000000000000000000000000000").unwrap(),
        edit: None,
    }
    .save_in_repo_as(&repo, &alice)
    .unwrap();

    // One comment of each subject kind: attached to the issue, attached to
    // the review, and one free-floating comment with no subject at all.
    // None carry a position `binding`, so /annotations (which lists only
    // comments with a binding) stays at its zero/empty state -- releases and
    // gates are left with zero entities/rules for the same reason.
    Comment {
        id: "audit-comment-issue-1".to_owned(),
        subject: Some("issue:audit-issue-1".to_owned()),
        author: "bob".to_owned(),
        body: "A comment attached to the audit issue.".to_owned(),
        binding: None,
        edit: None,
    }
    .save_in_repo_as(&repo, &bob)
    .unwrap();

    Comment {
        id: "audit-comment-review-1".to_owned(),
        subject: Some("review:audit-review-1".to_owned()),
        author: "alice".to_owned(),
        body: "A comment attached to the audit review.".to_owned(),
        binding: None,
        edit: None,
    }
    .save_in_repo_as(&repo, &alice)
    .unwrap();

    Comment {
        id: "audit-comment-freefloating-1".to_owned(),
        subject: None,
        author: "alice".to_owned(),
        body: "A free-floating comment with no subject at all.".to_owned(),
        binding: None,
        edit: None,
    }
    .save_in_repo_as(&repo, &alice)
    .unwrap();

    eprintln!("ui-audit fixture built at {}", repo_dir.display());
}
