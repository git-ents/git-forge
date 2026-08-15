//! [`Issue`]: a forge doc tracking a title, body, labels, and people.

use facet::Facet;

use crate::comment::Commentable;
use crate::entity::Entity;
use crate::{Authorization, Ownership};

#[derive(Debug, Clone, Facet)]
pub struct Issue {
    pub id: String,
    pub status: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub assignees: Vec<String>,
    pub reporters: Vec<String>,
    pub edit: Option<String>,
}

#[derive(Debug, Facet)]
pub struct StoredIssue {
    status: String,
    title: String,
    body: String,
    labels: Vec<String>,
    assignees: Vec<String>,
    reporters: Vec<String>,
    #[facet(default)]
    edit: Option<String>,
}

impl Entity for Issue {
    const KIND: &'static str = "issue";
    type Stored = StoredIssue;

    fn id(&self) -> &str {
        &self.id
    }

    fn to_stored(&self) -> StoredIssue {
        StoredIssue {
            status: self.status.clone(),
            title: self.title.clone(),
            body: self.body.clone(),
            labels: self.labels.clone(),
            assignees: self.assignees.clone(),
            reporters: self.reporters.clone(),
            edit: self.edit.clone(),
        }
    }

    fn ownership(&self, authorization: &Authorization) -> Ownership {
        if let crate::Principal::Member(member) = authorization.principal()
            && self
                .reporters
                .iter()
                .any(|reporter| reporter == member.as_str())
        {
            Ownership::Owned
        } else {
            Ownership::NotOwned
        }
    }

    fn attribute_to(&mut self, principal: &str) {
        self.reporters = vec![principal.to_owned()];
    }

    fn from_stored(id: String, stored: StoredIssue) -> Self {
        Self {
            id,
            status: stored.status,
            title: stored.title,
            body: stored.body,
            labels: stored.labels,
            assignees: stored.assignees,
            reporters: stored.reporters,
            edit: stored.edit,
        }
    }
}

impl Commentable for Issue {
    fn comment_subject(&self) -> (&'static str, &str) {
        (Issue::KIND, &self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityOps;
    use crate::{Authorization, Principal, open_store, query_goal, query_predicate};
    use gix_store::{RefEdit, RefSegment, RefStore};

    fn auth() -> Authorization {
        Authorization::new(Principal::member_id("alice"))
    }

    mod legacy {
        use super::*;

        #[derive(Debug, Facet)]
        pub(super) struct StoredIssue {
            pub(super) status: String,
            pub(super) title: String,
            pub(super) body: String,
            pub(super) labels: Vec<String>,
            pub(super) assignees: Vec<String>,
            pub(super) reporters: Vec<String>,
        }
    }

    /// Build the shape produced when a canonicalized issue keeps its original
    /// caller ref. The old schema is real store data, not a newly-created
    /// current-schema issue: publishing the current schema records the
    /// migration edge and exercises the dependency's supported schema pin.
    fn migrated_alias_fixture() -> (tempfile::TempDir, gix::Repository, String, String) {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let store = open_store(&repo);
        let issue_segment = RefSegment::new(Issue::KIND).expect("issue segment");
        let legacy_kind = store.kind::<legacy::StoredIssue>(issue_segment);
        let legacy_schema = gix_store::schema_of::<legacy::StoredIssue>().expect("legacy schema");
        legacy_kind
            .schema()
            .put(&legacy_schema)
            .expect("publish legacy schema");

        let legacy = legacy::StoredIssue {
            status: "open".to_owned(),
            title: "Migrated issue".to_owned(),
            body: "The migrated body contains the sentinel".to_owned(),
            labels: vec!["bug".to_owned()],
            assignees: vec!["alice".to_owned()],
            reporters: vec!["alice".to_owned()],
        };
        let commit = legacy_kind
            .write(&legacy)
            .anonymous()
            .expect("write legacy issue");

        Issue::ensure_schema_as(&store, &auth()).expect("publish current schema");
        let current_kind = Issue::kind(&store);
        let commit_name = gix_store::entity_name(commit);
        let canonical = gix_store::RefPath::new(
            repo.find_commit(commit)
                .expect("legacy commit")
                .tree_id()
                .expect("legacy root tree")
                .detach()
                .to_string(),
        )
        .expect("canonical tree id");
        let canonical_ref = current_kind.reference(&canonical);
        if let Some(expected) = store
            .refs()
            .read(&canonical_ref)
            .expect("read canonical ref")
        {
            store
                .refs()
                .apply(RefEdit::Delete {
                    name: canonical_ref.clone(),
                    expected,
                })
                .expect("remove auto-published canonical ref");
        }
        store
            .refs()
            .apply(RefEdit::Create {
                name: canonical_ref,
                new: commit,
            })
            .expect("publish canonical ref");
        store
            .refs()
            .apply(RefEdit::Delete {
                name: current_kind.reference(&commit_name),
                expected: commit,
            })
            .expect("remove pre-migration ref name");
        let alias = gix_store::RefPath::new("legacy-caller-id").expect("alias id");
        store
            .refs()
            .apply(RefEdit::Create {
                name: current_kind.reference(&alias),
                new: commit,
            })
            .expect("publish caller alias");

        (dir, repo, canonical.to_string(), alias.to_string())
    }

    #[test]
    fn migrated_aliases_are_one_issue_across_crud_and_broad_queries() {
        let (_dir, repo, canonical, alias) = migrated_alias_fixture();
        let store = open_store(&repo);
        assert_eq!(
            Issue::kind(&store)
                .schema()
                .history()
                .expect("schema history")
                .len(),
            2,
            "fixture must include the legacy and current schema commits"
        );

        Issue::kind(&store)
            .get_migrated(&gix_store::RefPath::new(&alias).expect("alias path"))
            .expect("current-schema migrated read")
            .expect("migrated alias exists");
        let loaded = Issue::load(&store, &alias)
            .expect("load alias")
            .expect("alias exists");
        assert_eq!(loaded.id, alias, "direct loads preserve the caller id");
        assert_eq!(loaded.title, "Migrated issue");
        assert_eq!(
            loaded.edit, None,
            "legacy schema leaves the added optional field absent"
        );

        let source_names = Issue::source_names(&repo).expect("source names");
        assert_eq!(source_names.len(), 2);
        assert!(source_names.contains(&canonical));
        assert!(source_names.contains(&alias));
        assert_eq!(Issue::list(&repo).expect("list"), vec![alias.clone()]);
        let all = Issue::load_all(&store).expect("load all");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, alias);

        let canonical_history = Issue::history(&repo, &canonical).expect("canonical history");
        assert_eq!(
            Issue::history(&repo, &alias).expect("alias history"),
            canonical_history
        );
        assert_eq!(canonical_history.len(), 1);

        let issue_rows = query_goal(&repo, "issue(Id)", &["Id"]).expect("issue query");
        assert_eq!(issue_rows.len(), 1);
        assert_eq!(issue_rows[0][0].to_string(), alias);
        let status_rows =
            query_goal(&repo, "issue_status(Id, Status)", &["Id", "Status"]).expect("status query");
        assert_eq!(status_rows.len(), 1);
        let body_rows = query_goal(&repo, "issue_body_contains(Id, \"sentinel\")", &["Id"])
            .expect("body query");
        assert_eq!(body_rows.len(), 1);
        assert_eq!(
            crate::search_issue(&repo, Some("alice"), Some("SENTINEL")).expect("search"),
            vec![alias.clone()]
        );
        assert_eq!(
            query_predicate(&repo, "issue", &[])
                .expect("predicate")
                .len(),
            1
        );

        // Adding another alias after the first query invalidates gix-store's
        // source-ref index, but the broad issue/* fact scan still emits one
        // logical row rather than one row per alias.
        let extra_alias = gix_store::RefPath::new("second-legacy-alias").expect("second alias");
        let commit = Issue::kind(&store)
            .get_entry(&gix_store::RefPath::new(&canonical).expect("canonical path"))
            .expect("read canonical entry")
            .expect("canonical entry")
            .commit;

        store
            .refs()
            .apply(RefEdit::Create {
                name: Issue::kind(&store).reference(&extra_alias),
                new: commit,
            })
            .expect("publish second alias");
        assert_eq!(
            Issue::list(&repo).expect("list after alias"),
            vec![alias.clone()]
        );
        assert_eq!(
            query_goal(&repo, "issue(Id)", &["Id"])
                .expect("query after alias")
                .len(),
            1
        );

        let mut changed = Issue::load(&store, &alias)
            .expect("load alias")
            .expect("alias exists");
        changed.status = "closed".to_owned();
        changed
            .save_as(&store, &auth())
            .expect("save through alias");
        assert_eq!(
            Issue::list(&repo).expect("list after save"),
            vec![alias.clone()]
        );
        assert_eq!(
            Issue::load(&store, &canonical)
                .expect("load canonical")
                .unwrap()
                .status,
            "closed"
        );
        assert_eq!(
            Issue::load(&store, &alias)
                .expect("load alias")
                .unwrap()
                .status,
            "closed"
        );
        assert_eq!(
            Issue::history(&repo, &canonical).expect("history canonical"),
            Issue::history(&repo, &alias).expect("history alias")
        );

        assert!(Issue::delete_as(&repo, &alias, &auth()).expect("delete alias"));
        assert!(Issue::list(&repo).expect("list after delete").is_empty());
        assert!(
            Issue::load(&store, &canonical)
                .expect("load canonical after delete")
                .is_none()
        );
        assert!(
            Issue::load(&store, &alias)
                .expect("load alias after delete")
                .is_none()
        );
        assert!(
            query_goal(&repo, "issue(Id)", &["Id"])
                .expect("query after delete")
                .is_empty()
        );
    }

    #[test]
    fn alias_only_legacy_refs_are_discoverable_without_a_canonical_ref() {
        let (_dir, repo, canonical, alias) = migrated_alias_fixture();
        let store = open_store(&repo);
        let canonical_path = gix_store::RefPath::new(&canonical).expect("canonical path");
        let commit = Issue::kind(&store)
            .get_entry(&canonical_path)
            .expect("read canonical ref")
            .expect("canonical ref exists")
            .commit;

        // Leave the migrated caller ref as the only source ref. The old kind
        // index may still describe the removed canonical ref, so discovery
        // must read the compatibility refs rather than trust that cache.
        store
            .refs()
            .apply(RefEdit::Delete {
                name: Issue::kind(&store).reference(&canonical_path),
                expected: commit,
            })
            .expect("remove canonical ref");

        assert_eq!(Issue::list(&repo).expect("list"), vec![alias.clone()]);
        let all = Issue::load_all(&store).expect("load all");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, alias);
        assert_eq!(
            query_goal(&repo, "issue(Id)", &["Id"])
                .expect("query")
                .len(),
            1
        );
    }

    #[test]
    fn canonical_aliases_deduplicate_publications_without_hiding_distinct_entities() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let first = Issue {
            id: String::new(),
            status: "open".to_owned(),
            title: "First".to_owned(),
            body: "first identity".to_owned(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };
        let first_id = first
            .create_in_repo_as(&repo, &auth())
            .expect("create first issue");
        let store = open_store(&repo);
        let first_path = gix_store::RefPath::new(&first_id).expect("first path");
        let first_value = Issue::load(&store, &first_id)
            .expect("load first")
            .expect("first exists");

        // Publish the same bound document in another commit under a caller
        // name. The document tree, not either publication commit, is the
        // logical identity and must therefore produce one result.
        let publication_alias =
            gix_store::RefPath::new("caller-publication").expect("publication alias");
        Issue::kind(&store)
            .put(&publication_alias, &first_value.to_stored())
            .expect("publish second first-issue commit");
        let caller_alias = gix_store::RefPath::new("caller-alias").expect("caller alias");
        let first_commit = Issue::kind(&store)
            .get_entry(&first_path)
            .expect("read first")
            .expect("first ref exists")
            .commit;
        store
            .refs()
            .apply(RefEdit::Create {
                name: Issue::kind(&store).reference(&caller_alias),
                new: first_commit,
            })
            .expect("publish caller alias");

        let second = Issue {
            id: String::new(),
            status: "open".to_owned(),
            title: "Second".to_owned(),
            body: "second identity".to_owned(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };
        let second_id = second
            .create_in_repo_as(&repo, &auth())
            .expect("create second issue");

        let ids = Issue::list(&repo).expect("list");
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&first_id));
        assert!(ids.contains(&second_id));
        let all = Issue::load_all(&store).expect("load all");
        assert_eq!(all.len(), 2);
        assert_eq!(
            all.iter()
                .find(|issue| issue.title == "First")
                .map(|issue| issue.id.as_str()),
            Some(first_id.as_str()),
            "the commit-named compatibility alias remains the primary id",
        );
        assert_eq!(
            query_goal(&repo, "issue(Id)", &["Id"])
                .expect("query")
                .len(),
            2
        );
        assert_eq!(
            crate::search_issue(&repo, None, Some("second identity")).expect("search"),
            vec![second_id]
        );
    }

    #[test]
    fn issue_round_trip_through_store() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let store = open_store(&repo);

        Issue::ensure_schema_as(&store, &auth()).expect("publish issue schema");

        let issue = Issue {
            id: "issue-1".to_string(),
            status: "open".to_string(),
            title: "Round trip issue".to_string(),
            body: "round trip issue".to_string(),
            labels: vec!["bug".to_string(), "P1".to_string()],
            assignees: vec!["alice".to_string()],
            reporters: vec!["alice".to_string()],
            edit: Some("initial edit note".to_string()),
        };

        issue.save_as(&store, &auth()).expect("save issue");
        let loaded = Issue::load(&store, &issue.id)
            .expect("load issue")
            .expect("issue exists");

        assert_eq!(loaded.id, issue.id);
        assert_eq!(loaded.status, issue.status);
        assert_eq!(loaded.title, issue.title);
        assert_eq!(loaded.body, issue.body);
        assert_eq!(loaded.labels, issue.labels);
        assert_eq!(loaded.assignees, issue.assignees);
        assert_eq!(loaded.reporters, issue.reporters);
        assert_eq!(loaded.edit, issue.edit);
    }

    #[test]
    fn create_in_repo_assigns_an_id_that_lists_and_loads() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");

        let issue = Issue {
            id: String::new(),
            status: "open".to_string(),
            title: "Anonymous issue".to_string(),
            body: "body".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec!["spoofed".to_owned()],
            edit: None,
        };
        let id = issue
            .create_in_repo_as(&repo, &auth())
            .expect("create issue");
        assert!(Issue::list(&repo).expect("list issues").contains(&id));
        let loaded = Issue::load_from_repo(&repo, &id)
            .expect("load issue")
            .expect("issue exists");
        assert_eq!(loaded.title, "Anonymous issue");
        assert_eq!(loaded.reporters, vec!["alice"]);
    }

    #[test]
    fn unauthorized_mutations_fail_and_authorized_mutations_succeed() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");
        let anonymous = Authorization::new(Principal::Anonymous);
        let alice = auth();
        let bob = Authorization::new(Principal::member_id("bob"));
        let mut issue = Issue {
            id: String::new(),
            status: "open".to_string(),
            title: "authorized issue".to_string(),
            body: "body".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };

        assert!(issue.create_in_repo_as(&repo, &anonymous).is_err());
        let id = issue
            .create_in_repo_as(&repo, &alice)
            .expect("create issue");
        issue.id = id.clone();
        issue.reporters.push("alice".to_string());
        issue.status = "closed".to_string();
        assert!(issue.save_in_repo_as(&repo, &bob).is_err());
        issue.save_in_repo_as(&repo, &alice).expect("update issue");
        assert!(Issue::delete_as(&repo, &id, &bob).is_err());
        assert!(Issue::delete_as(&repo, &id, &alice).expect("delete issue"));
    }

    #[test]
    fn history_and_delete_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");

        let mut issue = Issue {
            id: "issue-2".to_string(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: "b".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec!["alice".to_string()],
            edit: None,
        };
        issue.save_in_repo_as(&repo, &auth()).expect("save");
        issue.status = "closed".to_string();
        issue.save_in_repo_as(&repo, &auth()).expect("save again");

        let history = Issue::history(&repo, &issue.id).expect("history");
        assert_eq!(history.len(), 2);

        assert!(Issue::delete_as(&repo, &issue.id, &auth()).expect("delete"));
        assert!(
            Issue::load_from_repo(&repo, &issue.id)
                .expect("load after delete")
                .is_none()
        );
        assert!(!Issue::delete_as(&repo, &issue.id, &auth()).expect("delete missing"));
    }
}
