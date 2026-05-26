use aegoris_core::domain::fact::FactStore;
use aegoris_core::domain::Claim;
use aegoris_core::grounding::Verification;
use serde::Deserialize;

use crate::error::LlmResult;
use crate::provider::{CompletionRequest, LanguageModel};

const SYSTEM_PROMPT: &str = include_str!("../../../prompts/v1/verify_system.txt");

/// Semantic grounding check backed by a language model.
///
/// This is the second layer of grounding: the structural layer already
/// guarantees that cited facts exist, and this pass checks that the claim's
/// meaning is actually supported by them.
pub struct LlmClaimVerifier<M: LanguageModel> {
    model: M,
}

impl<M: LanguageModel> LlmClaimVerifier<M> {
    pub fn new(model: M) -> Self {
        Self { model }
    }

    pub async fn verify_claims(
        &self,
        claims: &[Claim],
        store: &FactStore,
    ) -> LlmResult<Vec<Verification>> {
        if claims.is_empty() {
            return Ok(Vec::new());
        }

        let request = CompletionRequest::new(SYSTEM_PROMPT, build_user_prompt(claims, store))
            .json()
            .with_temperature(0.0)
            .with_max_tokens(2048);

        let completion = self.model.complete(request).await?;
        Ok(match parse_verdicts(&completion.text, claims.len()) {
            Ok(verdicts) => verdicts,
            Err(error) => (0..claims.len())
                .map(|index| Verification::unsupported(index, format!("verifier error: {error}")))
                .collect(),
        })
    }
}

fn build_user_prompt(claims: &[Claim], store: &FactStore) -> String {
    let mut out = String::from("Verify each claim against its cited facts.\n\n");
    for (index, claim) in claims.iter().enumerate() {
        out.push_str(&format!("Claim {index}: {}\n", claim.text()));
        out.push_str("Cited facts:\n");
        for evidence in claim.evidence() {
            match store.get(&evidence.fact_id) {
                Some(fact) => out.push_str(&format!("- [{}] {}\n", fact.id, fact.text)),
                None => out.push_str(&format!("- [{}] <missing>\n", evidence.fact_id)),
            }
        }
        out.push('\n');
    }
    out
}

fn parse_verdicts(raw: &str, claim_count: usize) -> Result<Vec<Verification>, String> {
    let cleaned = strip_code_fences(raw);
    let response: VerdictResponse =
        serde_json::from_str(cleaned).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut verifications: Vec<Verification> = (0..claim_count)
        .map(|index| Verification::unsupported(index, "no verdict returned"))
        .collect();

    for verdict in response.results {
        if verdict.index < claim_count {
            verifications[verdict.index] = if verdict.supported {
                Verification::supported(verdict.index)
            } else {
                Verification::unsupported(
                    verdict.index,
                    verdict.reason.unwrap_or_else(|| "unsupported".to_string()),
                )
            };
        }
    }
    Ok(verifications)
}

fn strip_code_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    let without_open = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    without_open
        .strip_suffix("```")
        .unwrap_or(without_open)
        .trim()
}

#[derive(Debug, Deserialize)]
struct VerdictResponse {
    #[serde(default)]
    results: Vec<Verdict>,
}

#[derive(Debug, Deserialize)]
struct Verdict {
    index: usize,
    supported: bool,
    #[serde(default)]
    reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use aegoris_core::domain::evidence::Evidence;
    use aegoris_core::domain::ids::FactId;
    use aegoris_core::domain::{Fact, FactSource, Section};

    fn store() -> FactStore {
        FactStore::new(vec![Fact {
            id: FactId::generate(1),
            text: "Cut latency by 40%".to_string(),
            source: FactSource::Summary,
            section: Section::Summary,
            tags: Vec::new(),
        }])
    }

    fn claim() -> Claim {
        Claim::new(
            "Cut latency by 40%",
            vec![Evidence::new(FactId::generate(1))],
            Section::Summary,
            None,
        )
        .unwrap()
    }

    #[test]
    fn parses_supported_and_unsupported_verdicts() {
        let raw = r#"{"results":[{"index":0,"supported":false,"reason":"metric missing"}]}"#;
        let verdicts = parse_verdicts(raw, 1).unwrap();
        assert!(!verdicts[0].supported);
        assert_eq!(verdicts[0].detail.as_deref(), Some("metric missing"));
    }

    #[test]
    fn missing_verdicts_default_to_unsupported() {
        let verdicts = parse_verdicts(r#"{"results":[]}"#, 2).unwrap();
        assert!(verdicts.iter().all(|v| !v.supported));
    }

    #[tokio::test]
    async fn verifier_uses_the_model() {
        use crate::fake::FakeLanguageModel;
        let model = FakeLanguageModel::always(r#"{"results":[{"index":0,"supported":true}]}"#);
        let verifier = LlmClaimVerifier::new(model);
        let verdicts = verifier.verify_claims(&[claim()], &store()).await.unwrap();
        assert!(verdicts[0].supported);
    }
}
