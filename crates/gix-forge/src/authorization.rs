//! Typed authorization decisions for forge mutations.

use std::fmt;

use crate::{Error, Member};

/// The identity used when authorizing a forge operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// No authenticated forge member.
    Anonymous,
    /// A forge member, identified by [`Member::id`].
    Member(MemberId),
}

impl Principal {
    /// Create an unauthenticated principal.
    #[must_use]
    pub const fn anonymous() -> Self {
        Self::Anonymous
    }

    /// Create a member principal from a member's stored identity.
    #[must_use]
    pub fn member(member: &Member) -> Self {
        Self::Member(MemberId::from(member.id.as_str()))
    }

    /// Create an authenticated member principal from its identity.
    #[must_use]
    pub fn member_id(id: impl Into<MemberId>) -> Self {
        Self::Member(id.into())
    }

    /// Return whether this principal represents an authenticated member.
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        matches!(self, Self::Member(_))
    }

    /// Return the authenticated member identity, if present.
    #[must_use]
    pub const fn member_id_ref(&self) -> Option<&MemberId> {
        match self {
            Self::Anonymous => None,
            Self::Member(id) => Some(id),
        }
    }
}

/// A member identity used by authorization decisions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberId(String);

impl MemberId {
    /// Return the member id.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for MemberId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

impl From<String> for MemberId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl fmt::Display for MemberId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A mutation capability on a forge resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    IssueCreate,
    IssueUpdate,
    IssueDelete,
    ReviewCreate,
    ReviewUpdate,
    ReviewDelete,
    CommentCreate,
    CommentUpdate,
    CommentDelete,
    MemberCreate,
    MemberUpdate,
    MemberDelete,
    ForgeInstall,
}

impl Capability {
    fn requires_administrator(self) -> bool {
        matches!(
            self,
            Self::MemberCreate | Self::MemberUpdate | Self::MemberDelete | Self::ForgeInstall
        )
    }

    fn requires_ownership(self) -> bool {
        matches!(
            self,
            Self::IssueUpdate
                | Self::IssueDelete
                | Self::ReviewUpdate
                | Self::ReviewDelete
                | Self::CommentUpdate
                | Self::CommentDelete
        )
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::IssueCreate => "create issue",
            Self::IssueUpdate => "update issue",
            Self::IssueDelete => "delete issue",
            Self::ReviewCreate => "create review",
            Self::ReviewUpdate => "update review",
            Self::ReviewDelete => "delete review",
            Self::CommentCreate => "create comment",
            Self::CommentUpdate => "update comment",
            Self::CommentDelete => "delete comment",
            Self::MemberCreate => "create member",
            Self::MemberUpdate => "update member",
            Self::MemberDelete => "delete member",
            Self::ForgeInstall => "install forge data",
        };
        f.write_str(name)
    }
}

/// Whether the principal owns the target resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// The principal owns the target.
    Owned,
    /// The principal does not own the target.
    NotOwned,
    /// Ownership is not applicable, such as for creation.
    NotApplicable,
}

/// Authorization context for a single principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorization {
    principal: Principal,
    administrator: bool,
}

impl Authorization {
    /// Authorize operations for a non-administrative principal.
    #[must_use]
    pub fn new(principal: Principal) -> Self {
        Self {
            principal,
            administrator: false,
        }
    }

    /// Mark this explicitly supplied principal as an administrator.
    ///
    /// This does not inspect [`Member::role`]; callers must establish this
    /// status at the authentication/policy boundary.
    #[must_use]
    pub fn administrator(mut self) -> Self {
        self.administrator = true;
        self
    }

    /// The principal being authorized.
    #[must_use]
    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    /// Check a capability, returning an actionable forge error on denial.
    pub fn check(&self, capability: Capability, ownership: Ownership) -> Result<(), Error> {
        if matches!(self.principal, Principal::Anonymous) {
            return Err(Error::Unauthorized { capability });
        }

        if self.administrator
            || (!capability.requires_administrator()
                && (!capability.requires_ownership() || ownership == Ownership::Owned))
        {
            return Ok(());
        }

        let reason = if capability.requires_administrator() {
            "member management requires an administrator"
        } else {
            "resource ownership or administrator access is required"
        };
        Err(Error::Forbidden { capability, reason })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_principals_cannot_mutate() {
        let error = Authorization::new(Principal::Anonymous)
            .check(Capability::IssueCreate, Ownership::NotApplicable)
            .expect_err("anonymous writes must be rejected");
        assert!(matches!(error, Error::Unauthorized { .. }));
    }

    #[test]
    fn members_can_create_and_manage_owned_resources() {
        let authorization = Authorization::new(Principal::member_id("alice"));
        authorization
            .check(Capability::IssueCreate, Ownership::NotApplicable)
            .expect("members can create issues");
        authorization
            .check(Capability::IssueUpdate, Ownership::Owned)
            .expect("members can update owned issues");
        assert!(
            authorization
                .check(Capability::IssueDelete, Ownership::NotOwned)
                .is_err()
        );
    }

    #[test]
    fn only_explicit_administrators_can_manage_members() {
        let member = Authorization::new(Principal::member_id("alice"));
        assert!(
            member
                .check(Capability::MemberCreate, Ownership::NotApplicable)
                .is_err()
        );

        Authorization::new(Principal::member_id("alice"))
            .administrator()
            .check(Capability::MemberDelete, Ownership::NotApplicable)
            .expect("explicit administrators can manage members");
    }
}
