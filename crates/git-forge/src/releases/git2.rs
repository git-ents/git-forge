//! `git2::Repository` implementation of [`Releases`].

use git2::Repository;

use crate::releases::{NewArtifact, Release, ReleasePlan, Releases};

impl Releases for Repository {
    fn list_releases(&self) -> Result<Vec<Release>, git2::Error> {
        todo!()
    }

    fn find_release(&self, _version: &str) -> Result<Option<Release>, git2::Error> {
        todo!()
    }

    fn prepare_release(&self) -> Result<Option<ReleasePlan>, git2::Error> {
        todo!()
    }

    fn apply_release_plan(
        &self,
        _plan: &ReleasePlan,
        _target_branch: &str,
    ) -> Result<u64, git2::Error> {
        todo!()
    }

    fn publish_release(
        &self,
        _version: &str,
        _commit_oid: git2::Oid,
        _changelog: &str,
        _artifacts: &[NewArtifact],
    ) -> Result<(), git2::Error> {
        todo!()
    }

    fn attach_artifact(&self, _version: &str, _artifact: &NewArtifact) -> Result<(), git2::Error> {
        todo!()
    }
}
