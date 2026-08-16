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
use gix_anchor::{Binding, LineRange, RebindPin, capture};
use gix_attest::{Claims, Envelope, Target, register_claim_schema, register_key_schema};
use gix_query::Value;
use gix_store::{RefPath, RefSegment, RepoStore, schema_of};

use crate::entity::{Entity, EntityOps};
use crate::error::Error;
use crate::{Authorization, Capability, Ownership, Principal};

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

/// The selected binding state returned by query's `bind/5` lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationState {
    /// The original anchor is on the requested revision.
    Bound,
    /// A signed rebind pin selected the location.
    Pinned,
    /// An oracle selected a location different from the original anchor.
    Drifted,
    /// No candidate was produced by the bind query.
    Orphaned,
}

/// The location selected by `bind/5` for an annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationBinding {
    /// The selected path.
    pub path: String,
    /// The selected byte span.
    pub span: (u64, u64),
    /// The oracle selected by query.
    pub oracle: String,
    /// Query confidence in basis points.
    pub confidence: i64,
}

impl Comment {
    /// Resolve an anchored comment through the real anchor-oracle chain and
    /// query `bind/5`. `None` means the comment is unattached.
    pub fn annotation_binding(
        repo: &Repository,
        comment: &Comment,
        revision: ObjectId,
    ) -> Result<Option<AnnotationBinding>, Error> {
        let Some(Binding::Position(anchor)) = comment.binding.as_ref() else {
            return Ok(None);
        };
        let anchor_id = anchor.id()?.to_string();
        if anchor.identity.genesis_rev == revision.into() {
            return Ok(Some(AnnotationBinding {
                path: anchor.identity.path.clone(),
                span: (anchor.identity.span.start, anchor.identity.span.end),
                oracle: "anchor".to_owned(),
                confidence: 100,
            }));
        }
        let rows = gix_query::run_predicate(
            repo,
            "bind",
            &[
                (0, Value::sym(anchor_id)),
                (1, Value::sym(revision.to_string())),
            ],
        )?;
        let Some(row) = rows.into_iter().next() else {
            return Ok(Some(AnnotationBinding {
                path: anchor.identity.path.clone(),
                span: (anchor.identity.span.start, anchor.identity.span.end),
                oracle: String::new(),
                confidence: 0,
            }));
        };
        let location = row
            .first()
            .and_then(Value::as_sym)
            .ok_or_else(|| Error::QueryRules("bind returned no location".to_owned()))?;
        let (path, span) = parse_location(location)?;
        Ok(Some(AnnotationBinding {
            path,
            span,
            oracle: row
                .get(1)
                .and_then(Value::as_sym)
                .unwrap_or_default()
                .to_owned(),
            confidence: row.get(2).and_then(Value::as_int).unwrap_or_default(),
        }))
    }

    /// Read the annotation state from query's selected bind, with pins taking
    /// precedence over the original capture and oracle drift.
    pub fn annotation_state(
        repo: &Repository,
        comment: &Comment,
        revision: ObjectId,
    ) -> Result<Option<AnnotationState>, Error> {
        let Some(binding) = Self::annotation_binding(repo, comment, revision)? else {
            return Ok(None);
        };
        if binding.confidence == 0 {
            return Ok(Some(AnnotationState::Orphaned));
        }
        if binding.oracle == "pin" {
            return Ok(Some(AnnotationState::Pinned));
        }
        if let Some(Binding::Position(anchor)) = comment.binding.as_ref()
            && anchor.identity.genesis_rev == revision.into()
        {
            return Ok(Some(AnnotationState::Bound));
        }
        Ok(Some(AnnotationState::Drifted))
    }

    /// Sign a rebind pin for the best current `bind/5` result.
    pub fn rebind_pin_as(
        repo: &Repository,
        authorization: &Authorization,
        comment: &Comment,
        revision: ObjectId,
    ) -> Result<ObjectId, Error> {
        authorization.check(Capability::AnnotationRebind, Ownership::NotApplicable)?;
        let Some(Binding::Position(anchor)) = comment.binding.as_ref() else {
            return Err(Error::QueryRules(
                "an unattached comment cannot be pinned".to_owned(),
            ));
        };
        let binding = Self::annotation_binding(repo, comment, revision)?
            .filter(|binding| binding.confidence > 0)
            .ok_or_else(|| Error::QueryRules("bind produced no repair candidate".to_owned()))?;
        publish_annotation_claim(repo, "rebind-pin", anchor.id()?.into(), |store| {
            let payload = RebindPin {
                rev: revision.into(),
                path: binding.path,
                span: binding.span,
            };
            let kind = RefSegment::new(gix_anchor::REBIND_PIN_KIND).expect("built-in kind");
            store
                .kind::<RebindPin>(kind)
                .schema()
                .put(&schema_of::<RebindPin>().map_err(gix_store::Error::from)?)?;
            Ok(store
                .kind::<RebindPin>(
                    RefSegment::new(gix_anchor::REBIND_PIN_KIND).expect("built-in kind"),
                )
                .compile(&payload)?)
        })
    }

    /// Sign an obsolete claim for an anchored annotation.
    pub fn obsolete_as(
        repo: &Repository,
        authorization: &Authorization,
        comment: &Comment,
    ) -> Result<ObjectId, Error> {
        authorization.check(Capability::AnnotationObsolete, Ownership::NotApplicable)?;
        let Some(Binding::Position(anchor)) = comment.binding.as_ref() else {
            return Err(Error::QueryRules(
                "an unattached comment cannot be marked obsolete".to_owned(),
            ));
        };
        publish_annotation_claim(repo, "obsolete-annotation", anchor.id()?.into(), |store| {
            let kind = RefSegment::new("obsolete-annotation").expect("built-in kind");
            store
                .kind::<ObsoleteAnnotation>(kind)
                .schema()
                .put(&schema_of::<ObsoleteAnnotation>().map_err(gix_store::Error::from)?)?;
            Ok(store
                .kind::<ObsoleteAnnotation>(
                    RefSegment::new("obsolete-annotation").expect("built-in kind"),
                )
                .compile(&ObsoleteAnnotation {
                    comment_id: comment.id.clone(),
                })?)
        })
    }

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

#[derive(Debug, Facet)]
struct ObsoleteAnnotation {
    comment_id: String,
}

fn parse_location(value: &str) -> Result<(String, (u64, u64)), Error> {
    let (path, span) = value
        .rsplit_once(':')
        .ok_or_else(|| Error::QueryRules(format!("invalid bind location `{value}`")))?;
    let (start, end) = span
        .split_once('-')
        .ok_or_else(|| Error::QueryRules(format!("invalid bind span `{span}`")))?;
    let start = start
        .parse()
        .map_err(|_| Error::QueryRules(format!("invalid bind start `{start}`")))?;
    let end = end
        .parse()
        .map_err(|_| Error::QueryRules(format!("invalid bind end `{end}`")))?;
    Ok((path.to_owned(), (start, end)))
}

fn publish_annotation_claim<F>(
    repo: &Repository,
    payload_kind: &str,
    anchor_id: ObjectId,
    payload: F,
) -> Result<ObjectId, Error>
where
    F: FnOnce(&RepoStore<'_>) -> Result<ObjectId, Error>,
{
    let signer = crate::decision::SigningKey::from_config(repo)?;
    let store = RepoStore::open_with_layout(repo, gix_attest::layout()).with_signer(signer);
    register_claim_schema(&store)?;
    register_key_schema(&store)?;
    if payload_kind == gix_anchor::REBIND_PIN_KIND {
        gix_anchor::register_rebind_pin_schema(&store)?;
    }
    let payload_oid = payload(&store)?;
    let envelope = Envelope {
        target: Target {
            kind: "anchor".to_owned(),
            id: anchor_id.into(),
        },
        payload: payload_oid.into(),
        payload_kind: payload_kind.to_owned(),
        key: ObjectId::null(gix::hash::Kind::Sha1).into(),
    };
    let claim = Claims::open(&store).sign(&envelope)?;
    if store
        .signature(claim)?
        .is_some_and(|signature| !signature.is_empty())
    {
        Ok(claim)
    } else {
        Err(Error::UnsignedDecision)
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
