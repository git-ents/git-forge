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
    use crate::{Authorization, Principal, open_store};

    fn auth() -> Authorization {
        Authorization::new(Principal::member_id("alice"))
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
