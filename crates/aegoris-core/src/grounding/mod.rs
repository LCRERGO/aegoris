//! Grounding: the invariant that every generated claim traces to a source fact.
//!
//! Two layers enforce it:
//!
//! 1. **Structural** — the `Claim` type cannot be built without evidence, and
//!    every cited fact must exist in the `FactStore`. This is free and needs no
//!    model.
//! 2. **Semantic** — an optional verifier checks that the claim's *meaning* is
//!    supported by its cited facts. The LLM-backed implementation lives in
//!    `aegoris-llm`.

use serde::{Deserialize, Serialize};

use crate::domain::fact::FactStore;
use crate::domain::Claim;
use crate::error::{CoreError, CoreResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    MissingEvidence,
    UnknownFact,
    EmptyText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundingIssue {
    pub claim_index: usize,
    pub kind: IssueKind,
    pub detail: String,
}

/// The outcome of verifying a single claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verification {
    pub claim_index: usize,
    pub supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Verification {
    pub fn supported(claim_index: usize) -> Self {
        Self {
            claim_index,
            supported: true,
            detail: None,
        }
    }

    pub fn unsupported(claim_index: usize, detail: impl Into<String>) -> Self {
        Self {
            claim_index,
            supported: false,
            detail: Some(detail.into()),
        }
    }
}

/// Checks claims against the facts they cite.
pub trait ClaimVerifier {
    fn verify(&self, claims: &[Claim], store: &FactStore) -> CoreResult<Vec<Verification>>;
}

/// Structural verification: evidence exists and every cited fact resolves.
#[derive(Debug, Clone, Default)]
pub struct StructuralVerifier;

impl ClaimVerifier for StructuralVerifier {
    fn verify(&self, claims: &[Claim], store: &FactStore) -> CoreResult<Vec<Verification>> {
        Ok(claims
            .iter()
            .enumerate()
            .map(|(index, claim)| {
                let issues = structural_check_one(claim, store);
                if issues.is_empty() {
                    Verification::supported(index)
                } else {
                    Verification::unsupported(
                        index,
                        issues
                            .into_iter()
                            .map(|issue| issue.detail)
                            .collect::<Vec<_>>()
                            .join("; "),
                    )
                }
            })
            .collect())
    }
}

/// How to react when claims fail verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundingPolicy {
    /// Abort the run with a report of unsupported claims.
    #[default]
    Strict,
    /// Drop unsupported claims and continue.
    Warn,
}

/// Apply a policy to verified claims, returning the claims that survive.
pub fn apply_policy(
    claims: Vec<Claim>,
    verifications: &[Verification],
    policy: GroundingPolicy,
) -> CoreResult<Vec<Claim>> {
    let unsupported: Vec<&Verification> = verifications.iter().filter(|v| !v.supported).collect();

    if unsupported.is_empty() {
        return Ok(claims);
    }

    match policy {
        GroundingPolicy::Strict => {
            let detail = unsupported
                .iter()
                .map(|v| {
                    format!(
                        "claim {}: {}",
                        v.claim_index,
                        v.detail.as_deref().unwrap_or("unsupported")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            Err(CoreError::grounding(format!(
                "{} claim(s) failed grounding verification: {detail}",
                unsupported.len()
            )))
        }
        GroundingPolicy::Warn => {
            let dropped: std::collections::HashSet<usize> =
                unsupported.iter().map(|v| v.claim_index).collect();
            Ok(claims
                .into_iter()
                .enumerate()
                .filter(|(index, _)| !dropped.contains(index))
                .map(|(_, claim)| claim)
                .collect())
        }
    }
}

/// Structural issues for every claim, indexed by position.
pub fn structural_check(claims: &[Claim], store: &FactStore) -> Vec<GroundingIssue> {
    claims
        .iter()
        .enumerate()
        .flat_map(|(claim_index, claim)| {
            structural_check_one(claim, store)
                .into_iter()
                .map(move |mut issue| {
                    issue.claim_index = claim_index;
                    issue
                })
        })
        .collect()
}

fn structural_check_one(claim: &Claim, store: &FactStore) -> Vec<GroundingIssue> {
    let mut issues = Vec::new();
    if claim.text().trim().is_empty() {
        issues.push(GroundingIssue {
            claim_index: 0,
            kind: IssueKind::EmptyText,
            detail: "claim text is empty".to_string(),
        });
    }
    if claim.evidence().is_empty() {
        issues.push(GroundingIssue {
            claim_index: 0,
            kind: IssueKind::MissingEvidence,
            detail: "claim has no evidence".to_string(),
        });
    }
    for evidence in claim.evidence() {
        if !store.contains(&evidence.fact_id) {
            issues.push(GroundingIssue {
                claim_index: 0,
                kind: IssueKind::UnknownFact,
                detail: format!("cited fact {} does not exist", evidence.fact_id),
            });
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence::Evidence;
    use crate::domain::ids::FactId;
    use crate::domain::{Fact, FactSource, Section};

    fn store() -> FactStore {
        FactStore::new(vec![Fact {
            id: FactId::generate(1),
            text: "Wrote the first algorithm".to_string(),
            source: FactSource::Summary,
            section: Section::Summary,
            tags: Vec::new(),
        }])
    }

    fn claim(fact_id: FactId) -> Claim {
        Claim::new(
            "Wrote the first algorithm",
            vec![Evidence::new(fact_id)],
            Section::Summary,
            None,
        )
        .unwrap()
    }

    #[test]
    fn known_fact_passes_structural_check() {
        let claims = vec![claim(FactId::generate(1))];
        let verifier = StructuralVerifier;
        let results = verifier.verify(&claims, &store()).unwrap();
        assert!(results[0].supported);
    }

    #[test]
    fn unknown_fact_fails_structural_check() {
        let claims = vec![claim(FactId::generate(99))];
        let verifier = StructuralVerifier;
        let results = verifier.verify(&claims, &store()).unwrap();
        assert!(!results[0].supported);
    }

    #[test]
    fn strict_policy_aborts_on_unsupported_claims() {
        let claims = vec![claim(FactId::generate(99))];
        let verifier = StructuralVerifier;
        let results = verifier.verify(&claims, &store()).unwrap();
        let err = apply_policy(claims, &results, GroundingPolicy::Strict).unwrap_err();
        assert!(matches!(err, CoreError::Grounding(_)));
    }

    #[test]
    fn warn_policy_drops_unsupported_claims() {
        let claims = vec![claim(FactId::generate(1)), claim(FactId::generate(99))];
        let verifier = StructuralVerifier;
        let results = verifier.verify(&claims, &store()).unwrap();
        let kept = apply_policy(claims, &results, GroundingPolicy::Warn).unwrap();
        assert_eq!(kept.len(), 1);
    }
}
