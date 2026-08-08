//! Local request authentication for the UI.

use gix_forge::{Authorization, Entity, EntityOps, Member, Principal};
use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::unauthorized, headers},
};

const AUTHORIZATION: &str = "authorization";
const BEARER_PREFIX: &str = "Bearer ";

/// Extract the authenticated principal from a UI request.
///
/// Extract the explicit bearer principal, or the member selected when the UI
/// router was built from the effective Git signing key.
#[must_use]
pub fn principal(cx: &Cx) -> Principal {
    explicit_principal(cx)
        .or_else(|| {
            app_context::<Option<String>>(cx)
                .as_deref()
                .map(Principal::member_id)
        })
        .unwrap_or_else(Principal::anonymous)
}

fn explicit_principal(cx: &Cx) -> Option<Principal> {
    headers(cx)
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_bearer)
        .map(Principal::member_id)
}

/// Return the authenticated principal for a request, if it has one.
#[must_use]
pub fn authenticated_principal(cx: &Cx) -> Option<Principal> {
    let principal = principal(cx);
    principal.is_authenticated().then_some(principal)
}

/// Authenticate a mutation against the repository's stored members.
///
/// A bearer value is only an identity claim; it becomes authenticated after a
/// matching member is loaded. Maintainers are the repository administrators.
pub(crate) async fn authenticated_member(cx: &Cx) -> Option<Member> {
    let member_id = authenticated_principal(cx)
        .and_then(|principal| principal.member_id_ref().map(ToString::to_string))?;
    let repo = app_context::<std::sync::Arc<gix::ThreadSafeRepository>>(cx).clone();
    tokio::task::spawn_blocking(move || {
        let repo = repo.to_thread_local();
        Member::load_from_repo(&repo, &member_id).ok().flatten()
    })
    .await
    .ok()
    .flatten()
}

pub(crate) async fn authorization(cx: &Cx) -> Result<Authorization> {
    let Some(member) = authenticated_member(cx).await else {
        return Err(unauthorized().into());
    };
    let authorization = Authorization::new(Principal::member(&member));
    Ok(if member.role == "maintainer" {
        authorization.administrator()
    } else {
        authorization
    })
}

pub(crate) async fn can_update<T: Entity>(cx: &Cx, entity: &T) -> bool {
    authenticated_member(cx)
        .await
        .map(|member| {
            let authorization = Authorization::new(Principal::member(&member));
            let authorization = if member.role == "maintainer" {
                authorization.administrator()
            } else {
                authorization
            };
            authorization
                .check(T::update_capability(), entity.ownership(&authorization))
                .is_ok()
        })
        .unwrap_or(false)
}

pub(crate) async fn can_create<T: Entity>(cx: &Cx) -> bool {
    authenticated_member(cx)
        .await
        .map(|member| {
            let authorization = Authorization::new(Principal::member(&member));
            authorization
                .check(T::create_capability(), gix_forge::Ownership::NotApplicable)
                .is_ok()
        })
        .unwrap_or(false)
}

pub(crate) async fn can_manage_members(cx: &Cx) -> bool {
    authenticated_member(cx)
        .await
        .is_some_and(|member| member.role == "maintainer")
}

pub(crate) fn authorization_error(error: &str) -> Option<topcoat::Error> {
    if error.starts_with("unauthorized:") {
        Some(unauthorized().into())
    } else if error.starts_with("forbidden:") {
        Some(topcoat::router::error::forbidden().into())
    } else {
        None
    }
}

fn parse_bearer(value: &str) -> Option<&str> {
    let identity = value.strip_prefix(BEARER_PREFIX)?;
    (!identity.is_empty() && !identity.chars().any(char::is_whitespace)).then_some(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_identity_constructs_a_member_principal() {
        let principal = parse_bearer("Bearer alice")
            .map(Principal::member_id)
            .expect("valid bearer identity");

        assert!(principal.is_authenticated());
        assert_eq!(
            principal.member_id_ref().expect("member id").as_str(),
            "alice"
        );
    }

    #[test]
    fn malformed_credentials_are_anonymous() {
        for value in ["", "Basic alice", "Bearer ", "Bearer alice bob"] {
            assert!(
                parse_bearer(value).is_none(),
                "{value:?} should be rejected"
            );
        }
    }
}
