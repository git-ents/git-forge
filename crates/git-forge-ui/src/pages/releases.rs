//! Releases route (packet E).
//!
//! There is no `Release` entity, signature-verification, or CI/gate
//! prediction backend in this codebase yet -- that scope was explicitly
//! deferred. This route is a UI-only preview: the tag *identity* (name,
//! date, commit oid) is read from the real repository via `gix` when tags
//! exist there, but the claims/quorum/artifact data layered on top is
//! fabricated sample content, not a real signature ledger. "Publish
//! release" below is a no-op that only demonstrates the shared confirm
//! step; it does not write a real signature anywhere.

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
    shell::{Tab, shell},
};

#[path_param(error = bad_request)]
struct ReleaseTag(String);

/// Signers required out of the (up to) three claim slots shown per release.
const QUORUM_THRESHOLD: usize = 2;
/// Number of segments drawn in the quorum bar, i.e. the "3" in "2 of 3".
const QUORUM_SLOTS: usize = 3;
/// How many release cards the page always shows, real tags or not.
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

/// A tag's real identity: name, tagged-commit date, and short oid.
struct ReleaseIdentity {
    tag: String,
    date: String,
    oid: String,
}

struct SampleRelease {
    identity: ReleaseIdentity,
    artifacts: u32,
    claims: Vec<SampleClaim>,
}

impl SampleRelease {
    /// Claims with `status == Valid`, capped at the number of bar segments.
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
/// the cards: one clean 2-of-2, one 1-valid-1-revoked (below threshold), one
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

const SAMPLE_ARTIFACT_COUNTS: [u32; RELEASE_CARDS] = [4, 4, 3];

/// Fallback tag identities, used for any card slot a real tag didn't fill.
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
                artifacts: SAMPLE_ARTIFACT_COUNTS[index],
                claims: claim_sets[index].clone(),
            }
        })
        .collect()
}

async fn release_card(cx: &Cx, release: &SampleRelease) -> Result {
    let valid = release.valid_count();
    let met = release.quorum_met();
    let quorum_class = if met { "ok" } else { "warn" };
    let quorum_word = if met { "quorum met" } else { "needs sigs" };
    let quorum_label = format!("{valid}/{QUORUM_THRESHOLD} {quorum_word}");
    let tag = release.identity.tag.clone();
    let publish_action = format!("/releases/{tag}/publish");
    view! { cx =>
        <div class="release-card">
            <div class="release-header">
                <strong>(release.identity.tag.as_str())</strong>
                <span class="muted">
                    (release.identity.date.as_str()) " · " <code class="oid">(release.identity.oid.as_str())</code> " · " (release.artifacts) " artifacts"
                </span>
                <div class="release-quorum">
                    <div class="quorum-bar">
                        for slot in 0..QUORUM_SLOTS {
                            <span class=(if slot < valid { "quorum-seg filled" } else { "quorum-seg" })></span>
                        }
                    </div>
                    <span class=(format!("quorum-label {quorum_class}"))>(quorum_label)</span>
                    <form class="inline-form" action=(publish_action) method="post">
                        <button type="submit">"Publish release"</button>
                    </form>
                </div>
            </div>
            <ul class="claim-list">
                for claim in &release.claims {
                    <li class="claim-row">
                        <span>(claim.signer)</span>
                        <span class="muted">(claim.key) " · " (claim.date)</span>
                        <span class=(format!("status {}", claim.status.css_class()))>(claim.status.label())</span>
                    </li>
                }
            </ul>
        </div>
    }
}

#[page("/releases")]
async fn releases(cx: &Cx) -> Result {
    let releases = release_samples(cx).await;
    let mut cards = Vec::with_capacity(releases.len());
    for release in &releases {
        cards.push(release_card(cx, release).await?);
    }
    let content = (view! { cx =>
        <p class="muted release-intro">"A release is valid once its signature quorum is met -- 2 of 3 maintainers. Tag names, dates, and commit oids below come from this repository's real tags where available; artifact counts and signature claims are sample data previewing the intended design (there is no signing or quorum backend yet)."</p>
        <div class="release-list">
            for card in cards {
                (card)
            }
        </div>
    })?;
    view! { shell(active: Some(Tab::Releases), title: "Releases", keyword: None, child: content) }
}

#[page(POST "/releases/{tag}/publish")]
async fn release_publish(cx: &Cx, Form(input): Form<HashMap<String, String>>) -> Result {
    authorization(cx).await?;
    let tag = path_param::<ReleaseTag>(cx)?.clone();
    let summary = (view! { cx =>
        "Preview only: this records nothing. There is no release-signing backend yet, so publishing "
        <code>(tag.as_str())</code>
        " here only demonstrates the confirm step."
    })?;
    if let Some(confirm) = confirm_step(
        cx,
        &input,
        &format!("/releases/{tag}/publish"),
        &format!("refs/tags/{tag}"),
        "/releases",
        summary,
    )
    .await?
    {
        return view! { shell(active: Some(Tab::Releases), title: "Confirm release publish", keyword: None, child: confirm) };
    }
    let content = (view! { cx =>
        <section class="success-panel">
            <span class="success-icon">"✓"</span>
            <p class="eyebrow">"PREVIEW"</p>
            <h2>"Publish confirmed (no-op)"</h2>
            <p class="muted">"This UI has no release-signing backend yet, so nothing was actually signed or published for " <code>(tag.as_str())</code> "."</p>
            <a class="button-link" href="/releases">"Back to releases"</a>
        </section>
    })?;
    view! { shell(active: Some(Tab::Releases), title: "Release publish (preview)", keyword: None, child: content) }
}
