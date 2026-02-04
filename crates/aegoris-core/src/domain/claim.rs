use serde::{Deserialize, Serialize};

use crate::domain::evidence::Evidence;
use crate::domain::ids::RoleId;
use crate::domain::Section;
use crate::error::{CoreError, CoreResult};

/// Generated text that must cite at least one `Fact`.
///
/// Fields are private and construction is only possible through [`Claim::new`]
/// (or validated deserialization), so the grounding invariant holds by
/// construction: there is no way to build an unevidenced claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ClaimWire")]
pub struct Claim {
    text: String,
    evidence: Vec<Evidence>,
    section: Section,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    role_id: Option<RoleId>,
}

impl Claim {
    pub fn new(
        text: impl Into<String>,
        evidence: Vec<Evidence>,
        section: Section,
        role_id: Option<RoleId>,
    ) -> CoreResult<Self> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(CoreError::grounding("claim text is empty"));
        }
        if evidence.is_empty() {
            return Err(CoreError::grounding(format!(
                "claim has no evidence: {text:?}"
            )));
        }
        Ok(Self {
            text,
            evidence,
            section,
            role_id,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    pub fn section(&self) -> Section {
        self.section
    }

    pub fn role_id(&self) -> Option<&RoleId> {
        self.role_id.as_ref()
    }

    /// Every fact ID this claim asserts support from.
    pub fn fact_ids(&self) -> impl Iterator<Item = &crate::domain::ids::FactId> {
        self.evidence.iter().map(|e| &e.fact_id)
    }
}

#[derive(Deserialize)]
struct ClaimWire {
    text: String,
    #[serde(default)]
    evidence: Vec<Evidence>,
    #[serde(default)]
    section: Section,
    #[serde(default)]
    role_id: Option<RoleId>,
}

impl TryFrom<ClaimWire> for Claim {
    type Error = CoreError;

    fn try_from(wire: ClaimWire) -> Result<Self, Self::Error> {
        Claim::new(wire.text, wire.evidence, wire.section, wire.role_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::FactId;

    #[test]
    fn claim_without_evidence_is_rejected_by_constructor() {
        let err = Claim::new("Did a thing", vec![], Section::Experience, None).unwrap_err();
        assert!(matches!(err, CoreError::Grounding(_)));
    }

    #[test]
    fn claim_without_evidence_is_rejected_by_deserialization() {
        let json = r#"{"text":"Did a thing","section":"experience"}"#;
        let err = serde_json::from_str::<Claim>(json).unwrap_err();
        assert!(err.to_string().contains("no evidence"));
    }

    #[test]
    fn claim_with_evidence_round_trips() {
        let claim = Claim::new(
            "Cut p99 latency by 40%",
            vec![Evidence::new(FactId::generate(2))],
            Section::Experience,
            None,
        )
        .unwrap();
        let json = serde_json::to_string(&claim).unwrap();
        let back: Claim = serde_json::from_str(&json).unwrap();
        assert_eq!(claim, back);
    }

    #[test]
    fn empty_claim_text_is_rejected() {
        let err = Claim::new(
            "   ",
            vec![Evidence::new(FactId::generate(1))],
            Section::Summary,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::Grounding(_)));
    }
}
