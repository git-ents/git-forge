//! Generic CRUD shared by every forge entity kind.
//!
//! [`Entity`] declares what is genuinely specific to one kind: its name, its
//! stored payload shape, and how to move between the public struct and that
//! payload. [`EntityOps`] is the blanket implementation of everything else --
//! schema publishing, create/save/load/list/history/delete -- over
//! `gix-store`, so `Issue`, `Review`, and `Comment` each write only an
//! `Entity` impl instead of duplicating the whole CRUD surface.

use std::collections::BTreeMap;

use facet::Facet;
use gix::{ObjectId, Repository};
use gix_store::{
    ApplyError, EntityState, GixRefStore, Kind, NamedEntries, RefEdit, RefName, RefPath,
    RefSegment, RefStore, RepoStore, Typed,
};

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
            "release" => Capability::ReleaseCreate,
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
            "release" => Capability::ReleaseUpdate,
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
            "release" => Capability::ReleaseUpdate,
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
        let kind = Self::kind(store);
        let aliases = aliases_for::<Self>(store, &name)?;
        let commit = kind.put(&name, &self.to_stored())?;

        // A migrated repository can have several names for one entity. Keep
        // those caller names usable, but make them point at the one commit
        // produced by this update so a broad ref scan cannot turn an update
        // into two logical entities.
        for (alias, previous) in aliases {
            if alias == name {
                continue;
            }
            retarget(store, kind.reference(&alias), previous, commit)?;
        }
        Ok(commit)
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
            .read(&name)?
            .value()
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

    /// Every logical entity of this kind, ascending by its canonical id.
    ///
    /// `gix-store` deliberately exposes every source ref, including caller
    /// aliases. Forge treats refs carrying the same bound document tree as
    /// one entity for collection and query APIs, while [`load`](Self::load)
    /// keeps the name supplied by the caller for compatibility.
    fn load_all(store: &RepoStore<'_>) -> Result<Vec<Self>, Error> {
        let kind = Self::kind(store);
        let entries = logical_entries::<Self>(store)?;
        entries
            .into_iter()
            .map(|(name, commit)| {
                kind.read(commit)
                    .and_then(|state| match state {
                        EntityState::Present(entry) => {
                            Ok(Self::from_stored(name.to_string(), entry.value))
                        }
                        EntityState::Deleted(_) => Err(gix_store::Error::Deleted { commit }),
                        EntityState::Absent => unreachable!("a commit is always present"),
                    })
                    .map_err(Error::from)
            })
            .collect()
    }

    /// [`load_all`](Self::load_all) narrowed to the entities nested under
    /// `group`.
    fn load_all_under(store: &RepoStore<'_>, group: &RefPath) -> Result<Vec<Self>, Error> {
        let kind = Self::kind(store);
        let entries = logical_entries_under::<Self>(store, group)?;
        entries
            .into_iter()
            .map(|(name, commit)| {
                kind.read(commit)
                    .and_then(|state| match state {
                        EntityState::Present(entry) => {
                            Ok(Self::from_stored(name.to_string(), entry.value))
                        }
                        EntityState::Deleted(_) => Err(gix_store::Error::Deleted { commit }),
                        EntityState::Absent => unreachable!("a commit is always present"),
                    })
                    .map_err(Error::from)
            })
            .collect()
    }

    /// Convert bulk store entries into public entities, retaining one
    /// representative when the same publication commit has multiple names.
    fn rebuild(entries: NamedEntries<Self::Stored>) -> Vec<Self> {
        let mut seen = BTreeMap::new();
        for (name, entry) in entries {
            seen.entry(entry.commit)
                .or_insert_with(|| Self::from_stored(name.to_string(), entry.value));
        }
        seen.into_values().collect()
    }

    /// Every direct source name of this kind, including canonical refs and
    /// nested caller aliases. Unlike [`list`](Self::list), this does not
    /// collapse names that identify the same logical entity.
    fn source_names(repo: &Repository) -> Result<Vec<String>, Error> {
        let store = open_store(repo);
        let mut names: Vec<_> = source_entries::<Self>(&store)?
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect();
        names.sort();
        Ok(names)
    }

    /// Every canonical id of this kind, ascending. Caller aliases remain
    /// directly loadable, but are not reported as separate entities.
    fn list(repo: &Repository) -> Result<Vec<String>, Error> {
        let store = open_store(repo);
        Ok(logical_entries::<Self>(&store)?
            .into_iter()
            .map(|(name, _)| name.to_string())
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
        let kind = Self::kind(&store);
        let entity = Self::load(&store, id)?;
        let ownership = entity
            .as_ref()
            .map_or(Ownership::Owned, |entity| entity.ownership(authorization));
        authorization.check(Self::delete_capability(), ownership)?;

        let aliases = aliases_for::<Self>(&store, &name)?;
        if aliases.is_empty() {
            return Ok(false);
        }
        for (alias, _) in aliases {
            kind.remove(&alias)?;
        }
        Ok(true)
    }
}

impl<T: Entity> EntityOps for T {}

fn retarget(
    store: &RepoStore<'_>,
    name: RefName,
    expected: ObjectId,
    new: ObjectId,
) -> Result<(), Error> {
    store
        .refs()
        .apply(RefEdit::Update {
            name,
            expected,
            new,
        })
        .map_err(|error: ApplyError<_>| {
            Error::Store(gix_store::Error::Backend(Box::new(std::io::Error::other(
                error.to_string(),
            ))))
        })
}

fn logical_entries<T: Entity>(store: &RepoStore<'_>) -> Result<Vec<(RefPath, ObjectId)>, Error> {
    logical_entries_from(store, source_entries::<T>(store)?)
}

fn logical_entries_under<T: Entity>(
    store: &RepoStore<'_>,
    group: &RefPath,
) -> Result<Vec<(RefPath, ObjectId)>, Error> {
    let entries = source_entries::<T>(store)?
        .into_iter()
        .filter(|(name, _)| {
            name.segments().len() > group.segments().len()
                && name.segments().starts_with(group.segments())
        })
        .collect();
    logical_entries_from(store, entries)
}

/// Read the refs themselves instead of a materialized kind index. Indexes are
/// an optimization and older repositories can have valid indexes that predate
/// a migrated compatibility alias.
fn source_entries<T: Entity>(store: &RepoStore<'_>) -> Result<Vec<(RefPath, ObjectId)>, Error> {
    let prefix = store
        .layout()
        .data
        .child(&RefSegment::new(T::KIND).expect("built-in ref segment is valid"));
    store
        .refs()
        .prefixed(&prefix)
        .map_err(|error| Error::Store(gix_store::Error::Backend(Box::new(error))))
        .map(|entries| {
            entries
                .into_iter()
                .filter_map(|(name, commit)| name.relative_to(&prefix).map(|name| (name, commit)))
                .collect()
        })
}

fn logical_entries_from(
    store: &RepoStore<'_>,
    entries: Vec<(RefPath, ObjectId)>,
) -> Result<Vec<(RefPath, ObjectId)>, Error> {
    let mut by_identity: BTreeMap<ObjectId, Vec<(RefPath, ObjectId)>> = BTreeMap::new();
    for (name, commit) in entries {
        // A publication commit is not the entity identity: two refs can point
        // at different commits carrying the same bound document. The document
        // tree is the stable identity used by the store compatibility layer.
        let identity = document_identity(store.objects(), commit).unwrap_or(commit);
        by_identity
            .entry(identity)
            .or_default()
            .push((name, commit));
    }

    let mut logical: Vec<_> = by_identity
        .into_values()
        .map(|refs| {
            let name = canonical_name(store.objects(), &refs);
            let commit = refs
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .map_or(refs[0].1, |(_, commit)| *commit);
            (name, commit)
        })
        .collect();
    logical.sort_by(|(left, _), (right, _)| left.cmp(right));
    Ok(logical)
}

fn aliases_for<T: Entity>(
    store: &RepoStore<'_>,
    name: &RefPath,
) -> Result<Vec<(RefPath, ObjectId)>, Error> {
    let entries = source_entries::<T>(store)?;
    let Some((_, target)) = entries.iter().find(|(candidate, _)| candidate == name) else {
        return Ok(Vec::new());
    };
    let identity = document_identity(store.objects(), *target).unwrap_or(*target);
    Ok(entries
        .into_iter()
        .filter(|(_, commit)| {
            document_identity(store.objects(), *commit).unwrap_or(*commit) == identity
        })
        .collect())
}

/// Select the stable public name used by collection APIs for one commit's
/// refs. Caller aliases remain visible to forge users; canonical document-tree
/// names are used only when no alias exists. The lexical choice is
/// deterministic when several aliases address the same document.
fn canonical_name(objects: &gix::OdbHandle, refs: &[(RefPath, ObjectId)]) -> RefPath {
    // Anonymous creation retains the old commit-named path as a direct
    // compatibility alias. Prefer it over caller-chosen aliases so the
    // primary id returned by creation remains stable.
    if let Some((name, _)) = refs.iter().find(|(name, commit)| {
        name.segments().len() == 1 && *name == gix_store::entity_name(*commit)
    }) {
        return name.clone();
    }

    let mut aliases: Vec<_> = refs
        .iter()
        .filter(|(name, commit)| {
            let markers = commit_markers(objects, *commit);
            !markers
                .iter()
                .skip(1)
                .step_by(2)
                .any(|marker| name.to_string() == marker.to_string())
        })
        .map(|(name, _)| name.clone())
        .collect();
    aliases.sort();
    aliases
        .into_iter()
        .next()
        .unwrap_or_else(|| refs[0].0.clone())
}

/// The complete bound document tree is the compatibility identity shared by
/// separate publication commits for one entity.
fn document_identity(objects: &gix::OdbHandle, commit: ObjectId) -> Option<ObjectId> {
    commit_markers(objects, commit).get(1).copied()
}

/// Return the commit and root-tree ids along the first-parent chain. A
/// migrated canonical ref names an earlier root tree, so looking only at the
/// current commit would make an alias win after the first update.
fn commit_markers(objects: &gix::OdbHandle, mut commit: ObjectId) -> Vec<ObjectId> {
    use gix::objs::{Find, Kind as ObjectKind};

    let mut markers = Vec::new();
    let mut buffer = Vec::new();
    for _ in 0..1024 {
        markers.push(commit);
        let Some(data) = objects
            .try_find(commit.as_ref(), &mut buffer)
            .ok()
            .flatten()
        else {
            break;
        };
        if data.kind != ObjectKind::Commit {
            break;
        }
        let Ok(parsed) = gix::objs::CommitRef::from_bytes(data.data, gix::hash::Kind::Sha1) else {
            break;
        };
        markers.push(parsed.tree());
        let Some(parent) = parsed.parents().next() else {
            break;
        };
        commit = parent;
    }
    markers
}
