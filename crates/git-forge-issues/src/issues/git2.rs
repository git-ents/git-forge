//! `git2::Repository` implementation of [`Issues`].

use git2::Repository;

use crate::issues::{Issue, IssueState, Issues};

impl Issues for Repository {
    fn list_issues(&self) -> Result<Vec<Issue>, git2::Error> {
        todo!()
    }

    fn list_issues_by_state(&self, _state: IssueState) -> Result<Vec<Issue>, git2::Error> {
        todo!()
    }

    fn find_issue(&self, _id: u64) -> Result<Option<Issue>, git2::Error> {
        todo!()
    }

    fn create_issue(
        &self,
        _title: &str,
        _body: &str,
        _labels: &[String],
        _assignees: &[String],
    ) -> Result<u64, git2::Error> {
        todo!()
    }

    fn update_issue(
        &self,
        _id: u64,
        _title: Option<&str>,
        _body: Option<&str>,
        _labels: Option<&[String]>,
        _assignees: Option<&[String]>,
        _state: Option<IssueState>,
    ) -> Result<(), git2::Error> {
        todo!()
    }

    fn add_issue_comment(&self, _id: u64, _author: &str, _body: &str) -> Result<(), git2::Error> {
        todo!()
    }
}
