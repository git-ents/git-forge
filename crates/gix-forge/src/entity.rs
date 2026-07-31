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
use gix_store::{GixRefStore, Kind, RefPath, RefSegment, RepoStore, Typed};

use crate::{Error, open_store};

/// What one forge entity kind must declare.
pub trait Entity: Sized {
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

    /// Publish (or evolve) this entity's schema.
    fn ensure_schema(store: &RepoStore<'_>) -> Result<ObjectId, Error> {
        Ok(Self::kind(store).publish()?)
    }

    /// Store this entity at its own id.
    fn save(&self, store: &RepoStore<'_>) -> Result<ObjectId, Error> {
        let name = RefPath::new(self.id())?;
        Ok(Self::kind(store).put(&name, &self.to_stored())?)
    }

    /// Create a new entity at a fresh, store-assigned id. Returns the id.
    fn create_in_repo(&self, repo: &Repository) -> Result<String, Error> {
        let store = open_store(repo);
        Self::ensure_schema(&store)?;
        let commit = Self::kind(&store).write(&self.to_stored()).anonymous()?;
        Ok(gix_store::entity_name(commit).to_string())
    }

    /// Load the entity named `id`, or `None` if it doesn't exist.
    fn load(store: &RepoStore<'_>, id: &str) -> Result<Option<Self>, Error> {
        let name = RefPath::new(id)?;
        Ok(Self::kind(store)
            .get(&name)?
            .map(|stored| Self::from_stored(id.to_owned(), stored)))
    }

    /// [`ensure_schema`](Self::ensure_schema) then [`save`](Self::save)
    /// against the repository-backed store.
    fn save_in_repo(&self, repo: &Repository) -> Result<ObjectId, Error> {
        let store = open_store(repo);
        Self::ensure_schema(&store)?;
        self.save(&store)
    }

    /// [`load`](Self::load) against the repository-backed store.
    fn load_from_repo(repo: &Repository, id: &str) -> Result<Option<Self>, Error> {
        Self::load(&open_store(repo), id)
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

    /// Delete by id. Returns whether it existed.
    fn delete(repo: &Repository, id: &str) -> Result<bool, Error> {
        let store = open_store(repo);
        let name = RefPath::new(id)?;
        Ok(Self::kind(&store).remove(&name)?)
    }
}

impl<T: Entity> EntityOps for T {}
