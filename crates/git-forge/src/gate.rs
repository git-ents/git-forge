//! Declarative forge gates and signed promotion of their derived verdicts.

use std::{
    collections::hash_map::DefaultHasher,
    fmt,
    hash::{Hash, Hasher},
};

use facet::Facet;
use gix::ObjectId;
use gix_attest::{AttestKey, Claims, Envelope, Target, register_claim_schema, register_key_schema};
use gix_store::{RefSegment, RepoStore, schema_of};

use crate::decision::SigningKey;
use crate::{Authorization, Capability, Error, Ownership};

/// The three built-in gate names.
pub const GATE_NAMES: [&str; 3] = ["merge-gate", "release_signable", "docs-coverage"];

/// The declarative rule source for the merge gate.
pub const MERGE_RULES: &str = "pub reviewed(B).\npub gate_merge(Rev).\nreviewed(B) :- claim(C), kind(C, review), target(C, B), verdict(C, \"approve\"), anchor(B).\ngate_merge(Rev) :- commit(Rev), !unreviewed(Rev).\nunreviewed(Rev) :- tree_entry(Rev, _, B), !reviewed(B).";

/// The declarative rule source for the release gate. It consumes verified
/// attest claims written by Task 2's release claim ledger.
pub const RELEASE_RULES: &str = "pub gate_release(Oid).\npub release_signable(Oid).\ngate_release(Oid) :- claim(C), kind(C, \"release-claim\"), target(C, Oid).\nrelease_signable(Oid) :- gate_release(Oid).";

/// The declarative rule source for the documentation gate.
pub const DOCS_RULES: &str =
    "pub gate_docs(Rev).\ngate_docs(Rev) :- commit(Rev), tree_entry(Rev, \"README.md\", _).";

/// A gate's derived verdict and its input snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    /// The gate name.
    pub name: String,
    /// The subject object id used by the rule.
    pub subject: String,
    /// Whether the rule derived the gate predicate for the subject.
    pub passed: bool,
    /// The current rule module tip.
    pub rule_ref: Option<ObjectId>,
    /// A stable digest of the refs read by this gate.
    pub input_digest: String,
}

impl GateResult {
    /// The target id used by a signed promotion claim.
    pub fn target(&self) -> Result<ObjectId, Error> {
        ObjectId::from_hex(self.subject.as_bytes())
            .map_err(|_| Error::InvalidTarget(self.subject.clone()))
    }
}

/// Compute a current fingerprint of rule and gate input refs. There is no
/// persisted result cache yet, so this is displayed as a freshness key rather
/// than being used to claim a cache hit.
pub fn input_digest(repo: &gix::Repository, rule_ref: Option<ObjectId>) -> Result<String, Error> {
    let mut hasher = DefaultHasher::new();
    rule_ref.hash(&mut hasher);
    repo.head_id()
        .map_err(|error| Error::QueryRules(error.to_string()))?
        .detach()
        .hash(&mut hasher);
    let references = repo
        .references()
        .map_err(|error| Error::QueryRules(error.to_string()))?;
    let mut refs = references
        .all()
        .map_err(|error| Error::QueryRules(error.to_string()))?;
    while let Some(reference) = refs
        .next()
        .transpose()
        .map_err(|error| Error::QueryRules(error.to_string()))?
    {
        let name = reference.name().to_string();
        if [
            "refs/claims/",
            "refs/anchors/",
            "refs/forge/review/",
            "refs/tags/",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
        {
            name.hash(&mut hasher);
            reference.id().detach().hash(&mut hasher);
        }
    }
    Ok(format!("{:016x}", hasher.finish()))
}

/// Publish the built-in gate rule modules after authorization.
pub fn install_rules(repo: &gix::Repository, authorization: &Authorization) -> Result<(), Error> {
    authorization.check(Capability::ForgeInstall, Ownership::NotApplicable)?;
    let store =
        gix_query::RuleStore::open(repo).map_err(|error| Error::QueryRules(error.to_string()))?;
    store
        .put("merge_gate", MERGE_RULES)
        .map_err(|e| Error::QueryRules(e.to_string()))?;
    store
        .put("release_signable", RELEASE_RULES)
        .map_err(|e| Error::QueryRules(e.to_string()))?;
    store
        .put("docs_coverage", DOCS_RULES)
        .map_err(|e| Error::QueryRules(e.to_string()))?;
    Ok(())
}

fn rule_ref(repo: &gix::Repository, name: &str) -> Result<ObjectId, Error> {
    let ref_name = format!("refs/meta/rules/{name}");
    repo.try_find_reference(ref_name.as_str())
        .map_err(|error| Error::QueryRules(error.to_string()))?
        .map(|reference| reference.id().detach())
        .ok_or_else(|| Error::QueryRules(format!("rule `{name}` is not installed")))
}

/// Evaluate all built-in gates in one page evaluation pass.
pub fn evaluate(repo: &gix::Repository) -> Result<Vec<GateResult>, Error> {
    let head = repo
        .head_id()
        .map_err(|error| Error::QueryRules(error.to_string()))?;
    let mut results = Vec::with_capacity(3);
    for (name, module, predicate) in [
        ("merge-gate", "merge_gate", "merge_gate.gate_merge"),
        ("docs-coverage", "docs_coverage", "docs_coverage.gate_docs"),
    ] {
        let rule_ref = rule_ref(repo, module)?;
        let goal = format!("commit(\"{head}\"), {predicate}(\"{head}\")");
        let passed = !crate::query_goal(repo, &goal, &[])?.is_empty();
        results.push(GateResult {
            name: name.to_owned(),
            subject: head.to_string(),
            passed,
            input_digest: input_digest(repo, Some(rule_ref))?,
            rule_ref: Some(rule_ref),
        });
    }
    let release_rule = rule_ref(repo, "release_signable")?;
    for target in crate::Release::annotated_tags(repo)? {
        let goal = format!("release_signable.release_signable(\"{}\")", target.oid);
        let passed = !crate::query_goal(repo, &goal, &[])?.is_empty();
        results.push(GateResult {
            name: "release_signable".to_owned(),
            subject: target.oid,
            passed,
            input_digest: input_digest(repo, Some(release_rule))?,
            rule_ref: Some(release_rule),
        });
    }
    Ok(results)
}

#[derive(Debug, Clone, Facet)]
struct StoredGateClaim {
    gate: String,
    subject: String,
    passed: bool,
    rule_ref: String,
    input_digest: String,
}

/// Sign one derived gate verdict as an attest claim.
pub fn promote_as(
    repo: &gix::Repository,
    authorization: &Authorization,
    result: &GateResult,
) -> Result<ObjectId, Error> {
    authorization.check(Capability::GatePromote, Ownership::NotApplicable)?;
    let key = SigningKey::from_config(repo)?;
    let public = key.public_openssh()?;
    let store = RepoStore::open_with_layout(repo, gix_attest::layout()).with_signer(key);
    register_claim_schema(&store)?;
    register_key_schema(&store)?;
    let kind = RefSegment::new("gate-verdict").expect("gate-verdict is valid");
    store
        .kind::<StoredGateClaim>(kind.clone())
        .schema()
        .put(&schema_of::<StoredGateClaim>().map_err(gix_store::Error::from)?)?;
    let key_claim = Claims::open(&store).add_key(&AttestKey::from_openssh(&public, false)?)?;
    let payload = store
        .kind::<StoredGateClaim>(kind)
        .compile(&StoredGateClaim {
            gate: result.name.clone(),
            subject: result.subject.clone(),
            passed: result.passed,
            rule_ref: result
                .rule_ref
                .map_or_else(|| "absent".to_owned(), |id| id.to_string()),
            input_digest: result.input_digest.clone(),
        })?;
    let claim = Claims::open(&store).sign(&Envelope {
        target: Target {
            kind: "gate".into(),
            id: result.target()?.into(),
        },
        payload: payload.into(),
        payload_kind: "gate-verdict".into(),
        key: key_claim.into(),
    })?;
    match store.signature(claim)? {
        Some(signature) if !signature.is_empty() => Ok(claim),
        _ => Err(Error::UnsignedRelease),
    }
}

impl fmt::Display for GateResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({}) = {}", self.name, self.subject, self.passed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_gate_modules_assemble_together() {
        let directory = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(directory.path());
        let repo = gix::open(directory.path()).expect("open repository");
        let authorization =
            Authorization::new(crate::Principal::member_id("maintainer")).administrator();
        install_rules(&repo, &authorization).expect("publish gate modules");
        gix_query::checked_program_with(&repo, &crate::ForgeFacts::registry())
            .expect("gate modules validate against forge facts");
    }

    #[test]
    fn gates_evaluate_on_a_committed_forge_repo() {
        let directory = tempfile::tempdir().expect("tempdir");
        test_support::init_repo(directory.path());
        test_support::commit_file(directory.path(), "README.md", "# gates\n", "add readme");
        let repo = gix::open(directory.path()).expect("open repository");
        let authorization =
            Authorization::new(crate::Principal::member_id("maintainer")).administrator();
        install_rules(&repo, &authorization).expect("publish gate modules");
        evaluate(&repo).expect("gate evaluation");
    }
}
