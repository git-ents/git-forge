//! `git2::Repository` implementation of [`Comments`].

use git2::Repository;

use crate::{Comment, Comments, NewComment};

impl Comments for Repository {
    fn comments_on(&self, _ref_name: &str) -> Result<Vec<Comment>, git2::Error> {
        todo!()
    }

    fn find_comment(
        &self,
        _ref_name: &str,
        _oid: git2::Oid,
    ) -> Result<Option<Comment>, git2::Error> {
        todo!()
    }

    fn add_comment(
        &self,
        _ref_name: &str,
        _comment: &NewComment,
    ) -> Result<git2::Oid, git2::Error> {
        todo!()
    }
}
