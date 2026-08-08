//! Generic CRUD shared by every forge entity kind.
//!
//! [`Entity`] declares what is genuinely specific to one kind: its name, its
//! stored payload shape, and how to move between the public struct and that
//! payload. [`EntityOps`] is the blanket implementation of everything else --
//! schema publishing, create/save/load/list/history/delete -- over
//! `gix-store`, so `Issue`, `Review`, and `Comment` each write only an
//! `Entity` impl instead of duplicating the whole CRUD surface.

use facet::Facet;
use gix::{ObjectId, Repository};
use gix_store::{GixRefStore, Kind, NamedEntries, RefPath, RefSegment, RepoStore, Typed};

use crate::{Authorization, Capability, Error, Ownership, Principal, open_store};

/// What one forge entity kind must declare.
pub trait Entity: Sized + Clone {
    /// The `gix-store` kind name: entities live under `refs/forge/<KIND>/*`.
    const KIND: &'static str;

    /// The facet-derived value actually written to the store. Distinct from
    /// `Self` because the id is a ref-path component, not a stored field.
    type Stored: for<'a> Facet<'a>;

    /// This entity's stable id.
    fn id(&self) -> &str;

    /// The value to write to the store.
    fn to_stored(&self) -> Self::Stored;

    /// Rebuild the public entity from a stored value read back under `id`.
    fn from_stored(id: String, stored: Self::Stored) -> Self;

    /// Whether `authorization` owns this entity.
    fn ownership(&self, authorization: &Authorization) -> Ownership {
        match authorization.principal() {
            Principal::Member(member) if self.owner_ids().contains(&member.as_str()) => {
                Ownership::Owned
            }
            _ => Ownership::NotOwned,
        }
    }

    /// Existing entity fields that identify its owners.
    fn owner_ids(&self) -> Vec<&str> {
        Vec::new()
    }

    /// Add the authenticated principal to an existing attribution field.
    fn attribute_to(&mut self, _principal: &str) {}
}

type EntityKind<'s, 'r, T> =
    Kind<'s, Typed<<T as Entity>::Stored>, GixRefStore<'r>, &'r gix::OdbHandle>;

/// Shared CRUD over any [`Entity`]. Blanket-implemented; never implement
/// this by hand.
pub trait EntityOps: Entity {
    /// A handle on this entity's kind in `store`.
    fn kind<'a>(store: &'a RepoStore<'a>) -> EntityKind<'a, 'a, Self> {
        store.kind(RefSegment::new(Self::KIND).expect("built-in ref segment is valid"))
    }

    /// The capability required to create this entity kind.
    fn create_capability() -> Capability {
        match Self::KIND {
            "issue" => Capability::IssueCreate,
            "review" => Capability::ReviewCreate,
            "comment" => Capability::CommentCreate,
            "member" => Capability::MemberCreate,
            _ => unreachable!("built-in entity kind has a capability"),
        }
    }

    /// The capability required to update this entity kind.
    fn update_capability() -> Capability {
        match Self::KIND {
            "issue" => Capability::IssueUpdate,
            "review" => Capability::ReviewUpdate,
            "comment" => Capability::CommentUpdate,
            "member" => Capability::MemberUpdate,
            _ => unreachable!("built-in entity kind has a capability"),
        }
    }

    /// The capability required to delete this entity kind.
    fn delete_capability() -> Capability {
        match Self::KIND {
            "issue" => Capability::IssueDelete,
            "review" => Capability::ReviewDelete,
            "comment" => Capability::CommentDelete,
            "member" => Capability::MemberDelete,
            _ => unreachable!("built-in entity kind has a capability"),
        }
    }

    /// Publish (or evolve) this entity's schema.
    fn ensure_schema(_store: &RepoStore<'_>) -> Result<ObjectId, Error> {
        Err(Error::Unauthorized {
            capability: Self::create_capability(),
        })
    }

    /// Publish (or evolve) this entity's schema after authorization.
    fn ensure_schema_as(
        store: &RepoStore<'_>,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(Self::create_capability(), Ownership::NotApplicable)?;
        Ok(Self::kind(store).publish()?)
    }

    /// Store this entity at its own id.
    fn save(&self, _store: &RepoStore<'_>) -> Result<ObjectId, Error> {
        Err(Error::Unauthorized {
            capability: Self::update_capability(),
        })
    }

    /// Store this entity at its own id after authorization.
    fn save_as(
        &self,
        store: &RepoStore<'_>,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(Self::update_capability(), self.ownership(authorization))?;
        let name = RefPath::new(self.id())?;
        Ok(Self::kind(store).put(&name, &self.to_stored())?)
    }

    /// Legacy creation entry point. Anonymous writes are rejected.
    fn create_in_repo(&self, _repo: &Repository) -> Result<String, Error> {
        Err(Error::Unauthorized {
            capability: Self::create_capability(),
        })
    }

    /// Create a new entity after authorization. Returns the assigned id.
    fn create_in_repo_as(
        &self,
        repo: &Repository,
        authorization: &Authorization,
    ) -> Result<String, Error> {
        authorization.check(Self::create_capability(), Ownership::NotApplicable)?;
        let store = open_store(repo);
        Self::ensure_schema_as(&store, authorization)?;
        let mut entity = self.clone();
        if let Principal::Member(member) = authorization.principal() {
            entity.attribute_to(member.as_str());
        }
        let commit = Self::kind(&store).write(&entity.to_stored()).anonymous()?;
        Ok(gix_store::entity_name(commit).to_string())
    }

    /// Load the entity named `id`, or `None` if it doesn't exist.
    fn load(store: &RepoStore<'_>, id: &str) -> Result<Option<Self>, Error> {
        let name = RefPath::new(id)?;
        Ok(Self::kind(store)
            .get(&name)?
            .map(|stored| Self::from_stored(id.to_owned(), stored)))
    }

    /// Legacy update entry point. Anonymous writes are rejected.
    fn save_in_repo(&self, _repo: &Repository) -> Result<ObjectId, Error> {
        Err(Error::Unauthorized {
            capability: Self::update_capability(),
        })
    }

    /// Publish the schema and store this entity after authorization.
    fn save_in_repo_as(
        &self,
        repo: &Repository,
        authorization: &Authorization,
    ) -> Result<ObjectId, Error> {
        authorization.check(Self::update_capability(), self.ownership(authorization))?;
        let store = open_store(repo);
        Self::ensure_schema_as(&store, authorization)?;
        self.save_as(&store, authorization)
    }

    /// [`load`](Self::load) against the repository-backed store.
    fn load_from_repo(repo: &Repository, id: &str) -> Result<Option<Self>, Error> {
        Self::load(&open_store(repo), id)
    }

    /// Every entity of this kind, ascending by id, in one pass: `gix-store`
    /// pairs each name with the commit its ref already points at, so no ref
    /// is resolved twice and the schema those commits bind is parsed once for
    /// the whole scan.
    fn load_all(store: &RepoStore<'_>) -> Result<Vec<Self>, Error> {
        Ok(Self::rebuild(Self::kind(store).entries()?))
    }

    /// [`load_all`](Self::load_all) narrowed to the entities nested under
    /// `group`.
    fn load_all_under(store: &RepoStore<'_>, group: &RefPath) -> Result<Vec<Self>, Error> {
        Ok(Self::rebuild(Self::kind(store).entries_under(group)?))
    }

    /// The one place stored entries become public entities.
    fn rebuild(entries: NamedEntries<Self::Stored>) -> Vec<Self> {
        entries
            .into_iter()
            .map(|(name, entry)| Self::from_stored(name.to_string(), entry.value))
            .collect()
    }

    /// Every id of this kind, ascending.
    fn list(repo: &Repository) -> Result<Vec<String>, Error> {
        Ok(Self::kind(&open_store(repo))
            .list()?
            .into_iter()
            .map(|path| path.to_string())
            .collect())
    }

    /// Version history, tip-first.
    fn history(repo: &Repository, id: &str) -> Result<Vec<ObjectId>, Error> {
        let store = open_store(repo);
        let name = RefPath::new(id)?;
        Ok(Self::kind(&store).history(&name)?)
    }

    /// Legacy deletion entry point. Anonymous writes are rejected.
    fn delete(_repo: &Repository, _id: &str) -> Result<bool, Error> {
        Err(Error::Unauthorized {
            capability: Self::delete_capability(),
        })
    }

    /// Delete by id after authorization. Returns whether it existed.
    fn delete_as(
        repo: &Repository,
        id: &str,
        authorization: &Authorization,
    ) -> Result<bool, Error> {
        let store = open_store(repo);
        let name = RefPath::new(id)?;
        let entity = Self::load(&store, id)?;
        let ownership = entity
            .as_ref()
            .map_or(Ownership::Owned, |entity| entity.ownership(authorization));
        authorization.check(Self::delete_capability(), ownership)?;
        Ok(Self::kind(&store).remove(&name)?)
    }
}

impl<T: Entity> EntityOps for T {}
