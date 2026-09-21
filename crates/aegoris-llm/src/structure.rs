use aegoris_core::domain::Profile;

use crate::error::{LlmError, LlmResult};
use crate::provider::{CompletionRequest, LanguageModel};

const SYSTEM_PROMPT: &str = include_str!("../../../prompts/v1/profile_system.txt");

/// Profile structuring backed by a `LanguageModel`.
///
/// This is the opt-in fallback for PDF profiles that the deterministic
/// LinkedIn parser cannot read. It is only ever invoked when the user asks for
/// it explicitly; the deterministic path remains the default. The model's
/// output is validated structurally and retried on malformed responses.
pub struct LlmProfileStructurer<M: LanguageModel> {
    model: M,
    max_attempts: usize,
}

impl<M: LanguageModel> LlmProfileStructurer<M> {
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

    pub async fn structure(&self, text: &str) -> LlmResult<Profile> {
        let mut correction: Option<String> = None;

        for _ in 0..self.max_attempts {
            let user = match &correction {
                Some(note) => format!("{text}\n\nCORRECTION: {note}"),
                None => text.to_string(),
            };
            let request = CompletionRequest::new(SYSTEM_PROMPT, user)
                .json()
                .with_temperature(0.0)
                .with_max_tokens(4096);

            let completion = self.model.complete(request).await?;
            match parse_profile(&completion.text) {
                Ok(profile) => return Ok(profile),
                Err(error) => correction = Some(error),
            }
        }

        Err(LlmError::Parse(format!(
            "model failed to produce a valid profile after {} attempts",
            self.max_attempts
        )))
    }
}

fn parse_profile(raw: &str) -> Result<Profile, String> {
    let cleaned = strip_code_fences(raw);
    let profile: Profile =
        serde_json::from_str(cleaned).map_err(|error| format!("invalid JSON: {error}"))?;
    if profile.name.trim().is_empty() {
        return Err("profile.name must not be empty".to_string());
    }
    Ok(profile)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_profile() {
        let raw = r#"{"name":"Ada Lovelace","roles":[{"id":"r_1","title":"Engineer","organization":"Analytical Engines"}]}"#;
        let profile = parse_profile(raw).unwrap();
        assert_eq!(profile.name, "Ada Lovelace");
        assert_eq!(profile.roles[0].organization, "Analytical Engines");
    }

    #[test]
    fn strips_code_fences() {
        let raw = "```json\n{\"name\":\"Grace Hopper\"}\n```";
        assert_eq!(parse_profile(raw).unwrap().name, "Grace Hopper");
    }

    #[test]
    fn rejects_a_missing_name() {
        assert!(parse_profile(r#"{"name":"  "}"#).is_err());
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_profile("not json").is_err());
    }
}
