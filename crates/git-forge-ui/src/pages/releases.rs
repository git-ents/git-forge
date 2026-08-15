//! Releases route (Handoff v5): a master/detail pane over annotated tags --
//! "an annotated tag with a typed body, like an issue, anchored to a tag."
//!
//! There is no `Release` entity, signature-verification, or CI/gate
//! prediction backend in this codebase yet -- that scope was explicitly
//! deferred. This route is a UI-only preview: the tag *identity* (name,
//! date, commit oid) is read from the real repository via `gix` when tags
//! exist there, but the title, body, attachment listing, and claims/quorum
//! data layered on top are fabricated sample content, not a real release
//! ledger or signature store. "Sign claim on this tag" below is a no-op
//! that only demonstrates the shared confirm step; it does not write a
//! real signature anywhere.
//!
//! Per the design handoff, whether a release is actually signable is a
//! question for the Gates route's (also sample) `release_signable` goal,
//! not something this page computes or asserts -- this page shows the raw
//! claim tally (real counting logic over the fabricated claims above) and
//! links out rather than rendering its own ship/no-ship verdict.

use std::collections::HashMap;

use topcoat::{
    Result,
    context::Cx,
    router::{content::Form, page, path_param},
    view::view,
};

use crate::{
    auth::authorization,
    confirm::confirm_step,
    pages::with_repo,
    shell::{Tab, route_shell, shell},
};

#[path_param(error = bad_request)]
struct ReleaseTag(String);

/// Signers required out of a release's claim slots for the tally shown per
/// release to read "signed" rather than "needs sigs" -- real counting logic
/// over the (fabricated) claims below, kept from the pre-handoff page. This
/// only colors the raw count; see the module docs for why this page stops
/// short of asserting a ship/no-ship verdict from it.
const QUORUM_THRESHOLD: usize = 2;
/// How many release rows the page always shows, real tags or not.
const RELEASE_CARDS: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClaimStatus {
    Valid,
    Revoked,
}

impl ClaimStatus {
    const fn css_class(self) -> &'static str {
        match self {
            Self::Valid => "ok",
            Self::Revoked => "danger",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Clone)]
struct SampleClaim {
    signer: &'static str,
    key: &'static str,
    date: &'static str,
    status: ClaimStatus,
}

#[derive(Clone)]
struct SampleAttachment {
    name: &'static str,
    size: &'static str,
}

/// A tag's real identity: name, tagged-commit date, and short oid.
struct ReleaseIdentity {
    tag: String,
    date: String,
    oid: String,
}

struct SampleRelease {
    identity: ReleaseIdentity,
    title: &'static str,
    body: &'static str,
    attachments: Vec<SampleAttachment>,
    claims: Vec<SampleClaim>,
}

impl SampleRelease {
    /// Claims with `status == Valid`.
    fn valid_count(&self) -> usize {
        self.claims
            .iter()
            .filter(|claim| claim.status == ClaimStatus::Valid)
            .count()
    }

    fn quorum_met(&self) -> bool {
        self.valid_count() >= QUORUM_THRESHOLD
    }
}

/// Three fabricated claim sets, cycled across whatever tag identities back
/// the rows: one clean 2-of-2, one 1-valid-1-revoked (below threshold), one
/// 2-valid-1-revoked (met, with a revoked claim still on record).
fn sample_claim_sets() -> [Vec<SampleClaim>; RELEASE_CARDS] {
    [
        vec![
            SampleClaim {
                signer: "maria@forge.dev",
                key: "ed25519·AA…f2",
                date: "Jul 28",
                status: ClaimStatus::Valid,
            },
            SampleClaim {
                signer: "priya@forge.dev",
                key: "ed25519·77…0b",
                date: "Jul 29",
                status: ClaimStatus::Valid,
            },
        ],
        vec![
            SampleClaim {
                signer: "maria@forge.dev",
                key: "ed25519·AA…f2",
                date: "Jun 14",
                status: ClaimStatus::Valid,
            },
            SampleClaim {
                signer: "devon@forge.dev",
                key: "ed25519·9c…41",
                date: "Jun 14",
                status: ClaimStatus::Revoked,
            },
        ],
        vec![
            SampleClaim {
                signer: "maria@forge.dev",
                key: "ed25519·AA…f2",
                date: "May 02",
                status: ClaimStatus::Valid,
            },
            SampleClaim {
                signer: "priya@forge.dev",
                key: "ed25519·77…0b",
                date: "May 03",
                status: ClaimStatus::Valid,
            },
            SampleClaim {
                signer: "devon@forge.dev",
                key: "ed25519·9c…41",
                date: "Apr 30",
                status: ClaimStatus::Revoked,
            },
        ],
    ]
}

/// Fabricated attachment listings, one set per release row.
fn sample_attachment_sets() -> [Vec<SampleAttachment>; RELEASE_CARDS] {
    [
        vec![
            SampleAttachment { name: "git-forge-ui-x86_64-linux.tar.gz", size: "18.4 MB" },
            SampleAttachment { name: "git-forge-ui-aarch64-macos.tar.gz", size: "17.9 MB" },
            SampleAttachment { name: "git-forge-ui-x86_64-windows.zip", size: "19.1 MB" },
            SampleAttachment { name: "SHA256SUMS", size: "412 B" },
        ],
        vec![
            SampleAttachment { name: "git-forge-ui-x86_64-linux.tar.gz", size: "18.1 MB" },
            SampleAttachment { name: "git-forge-ui-aarch64-macos.tar.gz", size: "17.6 MB" },
            SampleAttachment { name: "git-forge-ui-x86_64-windows.zip", size: "18.8 MB" },
            SampleAttachment { name: "SHA256SUMS", size: "412 B" },
        ],
        vec![
            SampleAttachment { name: "git-forge-ui-x86_64-linux.tar.gz", size: "16.9 MB" },
            SampleAttachment { name: "git-forge-ui-aarch64-macos.tar.gz", size: "16.4 MB" },
            SampleAttachment { name: "SHA256SUMS", size: "298 B" },
        ],
    ]
}

const SAMPLE_TITLES: [&str; RELEASE_CARDS] = [
    "Bulk query cache, dark mode polish",
    "Patch: annotation confidence rebind fix",
    "Initial annotations API preview",
];

const SAMPLE_BODIES: [&str; RELEASE_CARDS] = [
    "Introduces a store-side schema cache that cuts bulk forge queries from roughly 340ms to 130ms on a real repository, plus the light/dark theme pass across every route. No schema or ref-layout changes; safe to adopt without a migration step.",
    "Fixes a rounding error in the quorum indicator that could show one segment short on narrow panes, and rebinds two annotation confidence states that drifted after a HEAD move. No API changes.",
    "First preview of the annotations API: bind a comment to a line range and track whether it stays bound, rebinds, or orphans as history moves. Feedback welcome before the API is considered stable.",
];

/// Fallback tag identities, used for any row slot a real tag didn't fill.
const SAMPLE_IDENTITIES: [(&str, &str, &str); RELEASE_CARDS] = [
    ("v0.4.0", "2026-07-28", "9b31d4a"),
    ("v0.3.2", "2026-06-14", "4c02e17"),
    ("v0.2.9", "2026-05-03", "d81aa4e"),
];

/// Real tag identities from the open repository, most-recently-tagged
/// commit first, capped at `RELEASE_CARDS`. Best-effort: any reference or
/// peel failure just drops that tag rather than failing the page.
fn real_tag_identities(repo: &gix::Repository) -> Vec<ReleaseIdentity> {
    let Ok(references) = repo.references() else {
        return Vec::new();
    };
    let Ok(tags) = references.tags() else {
        return Vec::new();
    };
    let mut identities = tags
        .flatten()
        .filter_map(|mut reference| {
            let name = reference.name().shorten().to_string();
            let commit = reference.peel_to_commit().ok()?;
            let time = commit.time().ok();
            let seconds = time.map(|time| time.seconds).unwrap_or_default();
            let date = time.map_or_else(
                || "unknown date".to_owned(),
                |time| time.format_or_unix(gix::date::time::format::SHORT),
            );
            let oid = commit.id.to_hex_with_len(7).to_string();
            Some((
                seconds,
                ReleaseIdentity {
                    tag: name,
                    date,
                    oid,
                },
            ))
        })
        .collect::<Vec<_>>();
    identities.sort_by_key(|(seconds, _)| std::cmp::Reverse(*seconds));
    identities
        .into_iter()
        .map(|(_, identity)| identity)
        .take(RELEASE_CARDS)
        .collect()
}

async fn release_samples(cx: &Cx) -> Vec<SampleRelease> {
    let real = with_repo(cx, |repo| Ok(real_tag_identities(repo)))
        .await
        .unwrap_or_default();
    let mut real = real.into_iter();
    let claim_sets = sample_claim_sets();
    let attachment_sets = sample_attachment_sets();
    (0..RELEASE_CARDS)
        .map(|index| {
            let identity = real.next().unwrap_or_else(|| {
                let (tag, date, oid) = SAMPLE_IDENTITIES[index];
                ReleaseIdentity {
                    tag: tag.to_owned(),
                    date: date.to_owned(),
                    oid: oid.to_owned(),
                }
            });
            SampleRelease {
                identity,
                title: SAMPLE_TITLES[index],
                body: SAMPLE_BODIES[index],
                attachments: attachment_sets[index].clone(),
                claims: claim_sets[index].clone(),
            }
        })
        .collect()
}

/// The nav pane: sticky header, intro note, and one row per release. Each
/// row's sub-line states the raw claim tally as a fact ("2/3 signed"),
/// colored by the same real threshold check the old quorum bar used --
/// see the module docs for why the detail pane itself stops there.
async fn release_nav(cx: &Cx, release_list: &[SampleRelease], selected: Option<&str>) -> Result {
    view! { cx =>
        <div class="route-nav-header">
            <h2>"Releases"</h2>
            <span class="route-nav-action is-disabled" title="No draft-release backend yet -- releases are read from this repository's real tags">"+ draft"</span>
        </div>
        <p class="route-nav-note">"an annotated tag with a typed body — like an issue, anchored to a tag"</p>
        for release in release_list {
            let valid = release.valid_count();
            let total = release.claims.len();
            let quorum_class = format!("quorum-label {}", if release.quorum_met() { "ok" } else { "warn" });
            let claims_label = format!("{valid}/{total} signed");
            let tag = release.identity.tag.as_str();
            let row_class = if selected == Some(tag) { "nav-row active" } else { "nav-row" };
            <a class=(row_class) href=(format!("/releases/{tag}"))>
                <div class="nav-row-top">
                    <span class="nav-row-title oid">(tag)</span>
                    <span class="nav-row-aside">(release.attachments.len()) " artifacts"</span>
                </div>
                <span class="nav-row-sub">(release.identity.date.as_str()) " · " <span class=(quorum_class)>(claims_label)</span></span>
            </a>
        }
    }
}

/// The detail pane for one release: identity + typed body, attachments,
/// and the claims ledger, deferring any shippability read to Gates.
async fn release_detail(cx: &Cx, release: &SampleRelease) -> Result {
    let sign_action = format!("/releases/{}/publish", release.identity.tag);
    view! { cx =>
        <article class="route-main-width-md">
            <div class="detail-meta">
                <div class="detail-meta-group">
                    <span class="status neutral">"TAGGED"</span>
                    <strong class="oid">(release.identity.tag.as_str())</strong>
                </div>
                <span class="muted">"target " <span class="oid">(release.identity.oid.as_str())</span></span>
            </div>
            <h1>(release.title)</h1>
            <p class="muted">"tagged " (release.identity.date.as_str())</p>
            <div class="body-copy">(release.body)</div>

            <h3>"Attachments"</h3>
            <div class="attachment-list">
                for attachment in &release.attachments {
                    <div class="attachment-row">
                        <span>(attachment.name)</span>
                        <span class="muted">(attachment.size)</span>
                    </div>
                }
            </div>

            <h3>"Claims — signed statements about " <span class="oid">(release.identity.oid.as_str())</span></h3>
            <ul class="claim-list panel-block">
                for claim in &release.claims {
                    let row_class = if claim.status == ClaimStatus::Revoked { "claim-row revoked" } else { "claim-row" };
                    <li class=(row_class)>
                        <strong>(claim.signer)</strong>
                        <span class=(format!("claim-verdict {}", claim.status.css_class()))>(claim.status.label())</span>
                        <span class="muted">(claim.date)</span>
                        <span class="claim-key">(claim.key)</span>
                    </li>
                }
            </ul>
            <p class="muted">"See " <a href="/gates">"release_signable"</a> " in Gates for whether this can ship — quorum, if any, is computed there, not fabricated here."</p>

            <div class="detail-actions">
                <form class="inline-form" action=(sign_action) method="post">
                    <button type="submit">"Sign claim on this tag"</button>
                </form>
            </div>
        </article>
    }
}

async fn release_placeholder() -> Result {
    view! {
        <div class="placeholder-detail">
            <p class="eyebrow">"RELEASE DETAIL"</p>
            <h2>"Select a release"</h2>
            <p class="muted">"Choose a tag from the list to read its body, attachments, and claims."</p>
        </div>
    }
}

#[page("/releases")]
async fn releases(cx: &Cx) -> Result {
    let release_list = release_samples(cx).await;
    let nav = release_nav(cx, &release_list, None).await?;
    let detail = release_placeholder().await?;
    view! { route_shell(active: Tab::Releases, title: "Releases", nav_width: "19rem", nav: nav, detail: detail) }
}

#[page("/releases/{release_tag}")]
async fn release_page(cx: &Cx) -> Result {
    let tag = path_param::<ReleaseTag>(cx)?.clone();
    let release_list = release_samples(cx).await;
    let nav = release_nav(cx, &release_list, Some(tag.as_str())).await?;
    let detail = match release_list.iter().find(|release| release.identity.tag == tag) {
        Some(release) => release_detail(cx, release).await?,
        None => (view! { <p class="empty">"Release not found."</p> })?,
    };
    view! { route_shell(active: Tab::Releases, title: "Release", nav_width: "19rem", nav: nav, detail: detail) }
}

#[page(POST "/releases/{release_tag}/publish")]
async fn release_publish(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    authorization(cx).await?;
    let tag = path_param::<ReleaseTag>(cx)?.clone();
    let summary = (view! { cx =>
        "Preview only: this records nothing. There is no signature-ledger backend yet, so signing a claim on "
        <code>(tag.as_str())</code>
        " here only demonstrates the confirm step."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/releases/{tag}/publish"),
        &format!("refs/tags/{tag}"),
        &format!("/releases/{tag}"),
        summary,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Releases), title: "Confirm claim signature", keyword: None, child: confirm) };
    }
    let content = (view! { cx =>
        <section class="success-panel">
            <span class="success-icon">"✓"</span>
            <p class="eyebrow">"PREVIEW"</p>
            <h2>"Claim signature confirmed (no-op)"</h2>
            <p class="muted">"This UI has no signature-ledger backend yet, so nothing was actually signed for " <code>(tag.as_str())</code> "."</p>
            <a class="button-link" href=(format!("/releases/{tag}"))>"Back to release"</a>
        </section>
    })?;
    view! { shell(active: Some(Tab::Releases), title: "Claim signature (preview)", keyword: None, child: content) }
}
