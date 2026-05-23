use aegoris_core::domain::artifact::ArtifactKind;
use aegoris_core::domain::evidence::Evidence;
use aegoris_core::domain::ids::{FactId, RoleId};
use aegoris_core::domain::{Claim, Section};
use aegoris_core::phrase::PhraseContext;
use serde::Deserialize;

use crate::error::{LlmError, LlmResult};
use crate::provider::{CompletionRequest, LanguageModel};

const SYSTEM_PROMPT: &str = include_str!("../../../prompts/v1/phrase_system.txt");

/// Phrasing backed by a `LanguageModel`.
///
/// The model is only ever asked to *rephrase* facts that curation already
/// selected; it is never the source of content. Output is validated
/// structurally and retried on malformed or unevidenced responses.
pub struct LlmPhraser<M: LanguageModel> {
    model: M,
    max_attempts: usize,
}

impl<M: LanguageModel> LlmPhraser<M> {
    pub fn new(model: M) -> Self {
        Self {
            model,
            max_attempts: 3,
        }
    }

    pub fn with_max_attempts(mut self, attempts: usize) -> Self {
        self.max_attempts = attempts.max(1);
        self
    }

    pub async fn phrase(&self, context: &PhraseContext<'_>) -> LlmResult<Vec<Claim>> {
        let base_user = build_user_prompt(context);
        let mut correction: Option<String> = None;

        for _ in 0..self.max_attempts {
            let user = match &correction {
                Some(note) => format!("{base_user}\n\nCORRECTION: {note}"),
                None => base_user.clone(),
            };
            let request = CompletionRequest::new(SYSTEM_PROMPT, user)
                .json()
                .with_temperature(0.0)
                .with_max_tokens(2048);

            let completion = self.model.complete(request).await?;
            match parse_claims(&completion.text) {
                Ok(claims) => return Ok(claims),
                Err(error) => correction = Some(error),
            }
        }

        Err(LlmError::Parse(format!(
            "model failed to produce valid grounded JSON after {} attempts",
            self.max_attempts
        )))
    }
}

fn build_user_prompt(context: &PhraseContext<'_>) -> String {
    let mut out = String::new();

    let artifact = match context.kind {
        ArtifactKind::Resume => "resume",
        ArtifactKind::CoverLetter => "cover letter",
    };
    out.push_str(&format!("Write a {artifact}.\n\n"));

    if let Some(title) = &context.jd.title {
        out.push_str(&format!("Target role: {title}\n"));
    }
    if let Some(organization) = &context.jd.organization {
        out.push_str(&format!("Organization: {organization}\n"));
    }

    out.push_str("\nRequirements:\n");
    for requirement in &context.plan.requirements {
        out.push_str(&format!("- [{}] {}\n", requirement.id, requirement.text));
    }

    out.push_str("\nCandidate facts (cite these IDs):\n");
    for fact in context.plan.selected_facts(context.store) {
        out.push_str(&format!(
            "- [{}] ({}) {}\n",
            fact.id,
            fact.section.as_str(),
            fact.text
        ));
    }

    out.push_str("\nReturn JSON only, with every claim citing at least one of the fact IDs above.");
    out
}

fn parse_claims(raw: &str) -> Result<Vec<Claim>, String> {
    let cleaned = strip_code_fences(raw);
    let response: ClaimsResponse =
        serde_json::from_str(cleaned).map_err(|e| format!("invalid JSON: {e}"))?;

    if response.claims.is_empty() {
        return Err("response contained no claims".to_string());
    }

    let mut claims = Vec::with_capacity(response.claims.len());
    for (index, raw_claim) in response.claims.into_iter().enumerate() {
        if raw_claim.fact_ids.is_empty() {
            return Err(format!("claim {index} cited no fact IDs"));
        }
        let evidence = raw_claim.fact_ids.into_iter().map(Evidence::new).collect();
        let claim = Claim::new(
            raw_claim.text,
            evidence,
            raw_claim.section,
            raw_claim.role_id,
        )
        .map_err(|e| format!("claim {index} invalid: {e}"))?;
        claims.push(claim);
    }
    Ok(claims)
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
struct ClaimsResponse {
    #[serde(default)]
    claims: Vec<RawClaim>,
}

#[derive(Debug, Deserialize)]
struct RawClaim {
    text: String,
    #[serde(default)]
    fact_ids: Vec<FactId>,
    #[serde(default)]
    section: Section,
    #[serde(default)]
    role_id: Option<RoleId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_claim_json() {
        let raw = r#"{"claims":[{"text":"Wrote the first algorithm","fact_ids":["f_1"],
            "section":"experience","role_id":"r_1"}]}"#;
        let claims = parse_claims(raw).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].text(), "Wrote the first algorithm");
        assert_eq!(claims[0].role_id(), Some(&RoleId::new("r_1")));
    }

    #[test]
    fn strips_markdown_code_fences() {
        let raw = "```json\n{\"claims\":[{\"text\":\"X\",\"fact_ids\":[\"f_1\"]}]}\n```";
        assert_eq!(parse_claims(raw).unwrap().len(), 1);
    }

    #[test]
    fn rejects_unevidenced_claims() {
        let raw = r#"{"claims":[{"text":"Invented a metric","fact_ids":[]}]}"#;
        let error = parse_claims(raw).unwrap_err();
        assert!(error.contains("no fact IDs"));
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_claims("not json").is_err());
    }
}
