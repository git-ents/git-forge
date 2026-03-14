//! `git2::Repository` implementation of [`Reviews`].

use git2::Repository;

use crate::reviews::{Review, ReviewState, Reviews};

impl Reviews for Repository {
    fn list_reviews(&self) -> Result<Vec<Review>, git2::Error> {
        todo!()
    }

    fn list_reviews_by_state(&self, _state: ReviewState) -> Result<Vec<Review>, git2::Error> {
        todo!()
    }

    fn find_review(&self, _id: u64) -> Result<Option<Review>, git2::Error> {
        todo!()
    }

    fn create_review(
        &self,
        _target_branch: &str,
        _description: &str,
        _head_commit: git2::Oid,
    ) -> Result<u64, git2::Error> {
        todo!()
    }

    fn update_review(
        &self,
        _id: u64,
        _description: Option<&str>,
        _state: Option<ReviewState>,
    ) -> Result<(), git2::Error> {
        todo!()
    }

    fn add_revision(&self, _id: u64, _head_commit: git2::Oid) -> Result<(), git2::Error> {
        todo!()
    }

    fn revision_range(
        &self,
        _review: &Review,
        _revision_index: usize,
    ) -> Result<(git2::Oid, git2::Oid), git2::Error> {
        todo!()
    }
}
