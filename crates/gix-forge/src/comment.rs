//! [`Comment`]: a forge doc embedding a `gix_anchor::Binding` subtree inline
//! (ARCHITECTURE.md, "git-forge": "comment = forge doc embedding a Binding
//! subtree; anchor id falls out as the hash of its identity subtree").
//!
//! A comment may be free-floating, attached to a subject, anchored to a
//! location, or both. A subject comment is grouped, at the store layer, under
//! the `(kind, id)` of the entity it comments on -- [`Commentable`] is the
//! sugar every commentable entity implements to get `add`/`show`/`list` for
//! its own comments for free, on top of [`Comment`]'s own
//! [`crate::entity::EntityOps`] CRUD.

use facet::Facet;
use gix::{ObjectId, Repository};
use gix_anchor::{Binding, LineRange, capture};
use gix_store::{RefPath, RefSegment};

use crate::entity::{Entity, EntityOps};
use crate::error::Error;
use crate::{Authorization, Ownership, Principal};

#[derive(Debug, Clone, Facet)]
pub struct Comment {
    pub id: String,
    /// The `(kind, id)` this comment is attached to, joined as
    /// `"<kind>:<id>"` -- e.g. `"issue:42"`; `None` for a free-floating
    /// comment.
    pub subject: Option<String>,
    pub author: String,
    pub body: String,
    /// Inline, per ARCHITECTURE.md: `None` when the comment has no anchor,
    /// `Some(Binding::Position(_))` for one anchored to a location.
    pub binding: Option<Binding>,
    pub edit: Option<String>,
}

#[derive(Debug, Facet)]
pub struct StoredComment {
    /// Empty means that the public comment has no subject; keeping this a
    /// string preserves the encoding of existing subject comments.
    subject: String,
    author: String,
    body: String,
    binding: Option<Binding>,
    edit: Option<String>,
}

impl Entity for Comment {
    const KIND: &'static str = "comment";
    type Stored = StoredComment;

    fn id(&self) -> &str {
        &self.id
    }

    fn to_stored(&self) -> StoredComment {
        StoredComment {
            subject: self.subject.clone().unwrap_or_default(),
            author: self.author.clone(),
            body: self.body.clone(),
            binding: self.binding.clone(),
            edit: self.edit.clone(),
        }
    }

    fn ownership(&self, authorization: &Authorization) -> Ownership {
        if let Principal::Member(member) = authorization.principal()
            && self.author == member.as_str()
        {
            Ownership::Owned
        } else {
            Ownership::NotOwned
        }
    }

    fn attribute_to(&mut self, principal: &str) {
        self.author = principal.to_owned();
    }

    fn from_stored(id: String, stored: StoredComment) -> Self {
        Self {
            id,
            subject: (!stored.subject.is_empty()).then_some(stored.subject),
            author: stored.author,
            body: stored.body,
            binding: stored.binding,
            edit: stored.edit,
        }
    }
}

impl Comment {
    /// Legacy anonymous creation entry point; always fails closed. Use
    /// [`Self::create_under_as`] for an authenticated write.
    pub fn create_under(
        _repo: &Repository,
        _subject_kind: &str,
        _subject_id: &str,
        _author: &str,
        _body: &str,
        _binding: Option<Binding>,
    ) -> Result<String, Error> {
        Err(Error::Unauthorized {
            capability: crate::Capability::CommentCreate,
        })
    }

    /// Create a comment under a subject after authorization.
    pub fn create_under_as(
        repo: &Repository,
        authorization: &Authorization,
        subject_kind: &str,
        subject_id: &str,
        body: &str,
        binding: Option<Binding>,
    ) -> Result<String, Error> {
        authorization.check(crate::Capability::CommentCreate, Ownership::NotApplicable)?;
        let store = crate::open_store(repo);
        Self::ensure_schema_as(&store, authorization)?;
        let group = subject_group(subject_kind, subject_id)?;
        let author = match authorization.principal() {
            Principal::Member(member) => member.as_str(),
            Principal::Anonymous => unreachable!("anonymous authorization was rejected"),
        };
        let stored = StoredComment {
            subject: format!("{subject_kind}:{subject_id}"),
            author: author.to_owned(),
            body: body.to_owned(),
            binding,
            edit: None,
        };
        let commit = Self::kind(&store).write(&stored).anonymous_under(&group)?;
        Ok(gix_store::entity_name_under(&group, commit).to_string())
    }

    /// Create a free-floating anchored comment after authorization.
    pub fn create_anchored_in_repo_as(
        repo: &Repository,
        authorization: &Authorization,
        revision: &str,
        path: &str,
        lines: Option<LineRange>,
        body: &str,
    ) -> Result<String, Error> {
        let anchor = capture(repo, revision, path, lines)?;
        Self {
            id: String::new(),
            subject: None,
            author: String::new(),
            body: body.to_owned(),
            binding: Some(Binding::Position(anchor)),
            edit: None,
        }
        .create_in_repo_as(repo, authorization)
    }

    /// Every comment in the repository, ascending by id.
    pub fn list_all(repo: &Repository) -> Result<Vec<Comment>, Error> {
        Self::load_all(&crate::open_store(repo))
    }

    /// Every comment stored under `<subject_kind>/<subject_id>`, ascending by
    /// id.
    pub fn list_under(
        repo: &Repository,
        subject_kind: &str,
        subject_id: &str,
    ) -> Result<Vec<Comment>, Error> {
        let store = crate::open_store(repo);
        let group = subject_group(subject_kind, subject_id)?;
        Self::load_all_under(&store, &group)
    }
}

fn subject_group(kind: &str, id: &str) -> Result<RefPath, Error> {
    let kind = RefSegment::new(kind)?;
    let id = RefSegment::new(id)?;
    Ok(RefPath::from(kind).join(&id))
}

/// Sugar every commentable entity implements: `(kind, id)` is all that's
/// genuinely specific, everything else -- storage, anchoring, listing --
/// is shared.
pub trait Commentable {
    /// The `(kind, id)` pair that scopes this entity's own comment group,
    /// e.g. `("issue", "42")`. `kind` is that entity's own [`Entity::KIND`].
    fn comment_subject(&self) -> (&'static str, &str);

    /// Legacy anonymous comment entry point; always fails closed. Use
    /// [`Self::add_comment_as`] for an authenticated write.
    fn add_comment(&self, repo: &Repository, author: &str, body: &str) -> Result<String, Error> {
        let (kind, id) = self.comment_subject();
        Comment::create_under(repo, kind, id, author, body, None)
    }

    /// Legacy anonymous anchored-comment entry point; always fails closed. Use
    /// [`Self::add_anchored_comment_as`] for an authenticated write.
    fn add_anchored_comment(
        &self,
        repo: &Repository,
        path: &str,
        lines: Option<LineRange>,
        author: &str,
        body: &str,
    ) -> Result<String, Error> {
        let (kind, id) = self.comment_subject();
        let anchor = capture(repo, "HEAD", path, lines)?;
        Comment::create_under(
            repo,
            kind,
            id,
            author,
            body,
            Some(Binding::Position(anchor)),
        )
    }

    /// Every comment attached to this entity.
    fn get_comments(&self, repo: &Repository) -> Result<Vec<Comment>, Error> {
        let (kind, id) = self.comment_subject();
        Comment::list_under(repo, kind, id)
    }

    /// Attach a thread-level comment after authorization.
    fn add_comment_as(
        &self,
        repo: &Repository,
        authorization: &Authorization,
        body: &str,
    ) -> Result<String, Error> {
        let (kind, id) = self.comment_subject();
        Comment::create_under_as(repo, authorization, kind, id, body, None)
    }

    /// Attach an anchored comment after authorization, against `HEAD`.
    /// `lines: None` anchors the whole file.
    fn add_anchored_comment_as(
        &self,
        repo: &Repository,
        authorization: &Authorization,
        path: &str,
        lines: Option<LineRange>,
        body: &str,
    ) -> Result<String, Error> {
        self.add_anchored_comment_as_at(repo, authorization, "HEAD", path, lines, body)
    }

    /// Attach an anchored comment after authorization, against `revision`.
    /// `lines: None` anchors the whole file.
    fn add_anchored_comment_as_at(
        &self,
        repo: &Repository,
        authorization: &Authorization,
        revision: &str,
        path: &str,
        lines: Option<LineRange>,
        body: &str,
    ) -> Result<String, Error> {
        let (kind, id) = self.comment_subject();
        let anchor = capture(repo, revision, path, lines)?;
        Comment::create_under_as(
            repo,
            authorization,
            kind,
            id,
            body,
            Some(Binding::Position(anchor)),
        )
    }
}

/// The genesis commit a [`Binding::Position`] anchor was captured against, or
/// `None` for any other binding shape -- what a caller reads to jump to an
/// anchored comment's location, without duplicating `gix_anchor`'s own
/// vocabulary.
#[must_use]
pub fn binding_genesis(binding: &Binding) -> Option<ObjectId> {
    match binding {
        Binding::Position(anchor) => Some(ObjectId::from(anchor.identity.genesis_rev)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::Issue;
    use crate::{Authorization, Principal};

    fn auth() -> Authorization {
        Authorization::new(Principal::member_id("alice"))
    }

    #[test]
    fn thread_comment_round_trips_with_no_binding() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");

        let issue = Issue {
            id: "issue-1".to_string(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: "b".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };

        let id = issue
            .add_comment_as(&repo, &auth(), "looks good")
            .expect("add comment");

        let loaded = Comment::load_from_repo(&repo, &id)
            .expect("load comment")
            .expect("comment exists");
        assert_eq!(loaded.author, "alice");
        assert_eq!(loaded.body, "looks good");
        assert_eq!(loaded.subject.as_deref(), Some("issue:issue-1"));
        assert!(loaded.binding.is_none());

        let comments = issue.get_comments(&repo).expect("list comments");
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, id);
    }

    #[test]
    fn anchored_comment_embeds_a_position_binding() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::commit_file(dir.path(), "file.txt", "one\ntwo\nthree\n", "add file");
        let repo = gix::open(dir.path()).expect("open repo");

        let issue = Issue {
            id: "issue-2".to_string(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: "b".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };

        let range = LineRange { start: 2, end: 2 };
        let id = issue
            .add_anchored_comment_as(&repo, &auth(), "file.txt", Some(range), "what is this?")
            .expect("add anchored comment");

        let loaded = Comment::load_from_repo(&repo, &id)
            .expect("load comment")
            .expect("comment exists");
        let Some(Binding::Position(anchor)) = &loaded.binding else {
            panic!("expected a Position binding, got {:?}", loaded.binding);
        };
        assert_eq!(anchor.identity.path, "file.txt");
        assert_eq!(loaded.subject.as_deref(), Some("issue:issue-2"));
        assert!(binding_genesis(loaded.binding.as_ref().unwrap()).is_some());
    }

    #[test]
    fn a_free_floating_anchor_has_no_subject() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::commit_file(dir.path(), "file.txt", "one\ntwo\nthree\n", "add file");
        let repo = gix::open(dir.path()).expect("open repo");
        let anchor = capture(
            &repo,
            "HEAD",
            "file.txt",
            Some(LineRange { start: 1, end: 1 }),
        )
        .expect("capture anchor");

        let id = Comment {
            id: String::new(),
            subject: None,
            author: "spoofed".to_owned(),
            body: "file-level note".to_owned(),
            binding: Some(Binding::Position(anchor)),
            edit: None,
        }
        .create_in_repo_as(&repo, &auth())
        .expect("create free-floating comment");

        let loaded = Comment::load_from_repo(&repo, &id)
            .expect("load comment")
            .expect("comment exists");
        assert!(loaded.subject.is_none());
        assert_eq!(loaded.author, "alice");
        assert!(matches!(loaded.binding, Some(Binding::Position(_))));
    }

    #[test]
    fn comments_are_grouped_per_subject_and_do_not_leak_across_entities() {
        let dir = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(dir.path());
        let repo = gix::open(dir.path()).expect("open repo");

        let a = Issue {
            id: "a".to_string(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: "b".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };
        let b = Issue {
            id: "b".to_string(),
            status: "open".to_string(),
            title: "t".to_string(),
            body: "b".to_string(),
            labels: vec![],
            assignees: vec![],
            reporters: vec![],
            edit: None,
        };

        a.add_comment_as(&repo, &auth(), "on a").expect("comment a");
        b.add_comment_as(&repo, &auth(), "on b").expect("comment b");

        assert_eq!(a.get_comments(&repo).expect("comments a").len(), 1);
        assert_eq!(b.get_comments(&repo).expect("comments b").len(), 1);
        assert_eq!(
            Comment::list(&repo).expect("list all comments").len(),
            2,
            "the flat top-level listing still sees every comment"
        );
    }
}
