use crate::comment::{Anchor, format_trailers, parse_trailers};

#[test]
fn parse_trailers_no_trailers() {
    let (body, trailers) = parse_trailers("just a plain body");
    assert_eq!(body, "just a plain body");
    assert!(trailers.is_empty());
}

#[test]
fn parse_trailers_known_keys() {
    let msg = "comment body\n\nAnchor: deadbeef\nAnchor-Range: 10-20";
    let (body, trailers) = parse_trailers(msg);
    assert_eq!(body, "comment body");
    assert_eq!(trailers.get("Anchor").unwrap(), "deadbeef");
    assert_eq!(trailers.get("Anchor-Range").unwrap(), "10-20");
}

#[test]
fn parse_trailers_only_trailers() {
    let msg = "Resolved: true";
    let (body, trailers) = parse_trailers(msg);
    assert!(body.is_empty());
    assert_eq!(trailers.get("Resolved").unwrap(), "true");
}

#[test]
fn parse_trailers_unknown_key_stays_in_body() {
    let msg = "comment body\n\nSigned-off-by: someone@example.com";
    let (body, trailers) = parse_trailers(msg);
    assert_eq!(body, "comment body\n\nSigned-off-by: someone@example.com");
    assert!(trailers.is_empty());
}

#[test]
fn parse_trailers_mixed_known_unknown_stays_in_body() {
    let msg = "body\n\nAnchor: deadbeef\nSigned-off-by: someone";
    let (body, trailers) = parse_trailers(msg);
    // Mixed paragraph has an unknown key, so the whole block stays in body.
    assert_eq!(body, "body\n\nAnchor: deadbeef\nSigned-off-by: someone");
    assert!(trailers.is_empty());
}

#[test]
fn parse_trailers_multiline_body_with_colons() {
    let msg = "This has Key: value-like text in it\n\nResolved: true";
    let (body, trailers) = parse_trailers(msg);
    assert_eq!(body, "This has Key: value-like text in it");
    assert_eq!(trailers.get("Resolved").unwrap(), "true");
}

#[test]
fn parse_trailers_github_id() {
    let msg = "imported comment\n\nGithub-Id: 42";
    let (body, trailers) = parse_trailers(msg);
    assert_eq!(body, "imported comment");
    assert_eq!(trailers.get("Github-Id").unwrap(), "42");
}

#[test]
fn parse_trailers_replaces() {
    let msg = "updated body\n\nReplaces: abc123";
    let (body, trailers) = parse_trailers(msg);
    assert_eq!(body, "updated body");
    assert_eq!(trailers.get("Replaces").unwrap(), "abc123");
}

#[test]
fn format_trailers_empty() {
    let result = format_trailers(None, false, None);
    assert!(result.is_empty());
}

#[test]
fn format_trailers_resolved() {
    let result = format_trailers(None, true, None);
    assert_eq!(result, "Resolved: true");
}

#[test]
fn format_trailers_replaces() {
    let result = format_trailers(None, false, Some("abc123"));
    assert_eq!(result, "Replaces: abc123");
}

#[test]
fn format_trailers_object_anchor() {
    let anchor = Anchor::Object {
        oid: "deadbeef".to_string(),
        range: Some("10-20".to_string()),
    };
    let result = format_trailers(Some(&anchor), false, None);
    assert!(result.contains("Anchor: deadbeef"));
    assert!(result.contains("Anchor-Range: 10-20"));
}

#[test]
fn format_trailers_commit_range_anchor() {
    let anchor = Anchor::CommitRange {
        start: "aaa".to_string(),
        end: "bbb".to_string(),
    };
    let result = format_trailers(Some(&anchor), false, None);
    assert!(result.contains("Anchor: aaa"));
    assert!(result.contains("Anchor-End: bbb"));
}

#[test]
fn format_trailers_combined() {
    let anchor = Anchor::Object {
        oid: "deadbeef".to_string(),
        range: None,
    };
    let result = format_trailers(Some(&anchor), true, Some("orig123"));
    assert!(result.contains("Anchor: deadbeef"));
    assert!(result.contains("Resolved: true"));
    assert!(result.contains("Replaces: orig123"));
}

#[test]
fn parse_trailers_colon_in_value() {
    let msg = "body\n\nAnchor: abc:def";
    let (body, trailers) = parse_trailers(msg);
    assert_eq!(body, "body");
    assert_eq!(trailers.get("Anchor").unwrap(), "abc:def");
}

#[test]
fn parse_trailers_empty_body_with_double_newline() {
    let msg = "\n\nResolved: true";
    let (body, trailers) = parse_trailers(msg);
    assert!(body.is_empty());
    assert_eq!(trailers.get("Resolved").unwrap(), "true");
}

#[test]
fn parse_trailers_multiple_paragraphs_body() {
    let msg = "first paragraph\n\nsecond paragraph\n\nAnchor: deadbeef";
    let (body, trailers) = parse_trailers(msg);
    assert_eq!(body, "first paragraph\n\nsecond paragraph");
    assert_eq!(trailers.get("Anchor").unwrap(), "deadbeef");
}

#[test]
fn format_trailers_object_anchor_no_range() {
    let anchor = Anchor::Object {
        oid: "deadbeef".to_string(),
        range: None,
    };
    let result = format_trailers(Some(&anchor), false, None);
    assert_eq!(result, "Anchor: deadbeef");
    assert!(!result.contains("Anchor-Range"));
}
