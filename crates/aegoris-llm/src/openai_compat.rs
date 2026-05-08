use async_trait::async_trait;
use serde::Deserialize;

use crate::error::{ProviderError, ProviderResult};
use crate::provider::{Completion, CompletionRequest, LanguageModel};

/// Adapter for any OpenAI-compatible chat-completions endpoint.
///
/// One adapter covers OpenAI, OpenRouter, Ollama, LM Studio, and DeepSeek
/// (the default: `base_url = https://api.deepseek.com`, `model = deepseek-flash`).
pub struct OpenAiCompatModel {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
    json_mode: bool,
}

impl OpenAiCompatModel {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::new(),
            json_mode: false,
        }
    }

    pub fn with_json_mode(mut self, enabled: bool) -> Self {
        self.json_mode = enabled;
        self
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

/// Default OpenAI-compatible endpoint for DeepSeek.
pub const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
/// Default DeepSeek model.
pub const DEFAULT_MODEL: &str = "deepseek-flash";

/// Construct the default DeepSeek model.
pub fn deepseek(api_key: impl Into<String>) -> OpenAiCompatModel {
    OpenAiCompatModel::new(DEFAULT_BASE_URL, api_key, DEFAULT_MODEL).with_json_mode(true)
}

#[async_trait]
impl LanguageModel for OpenAiCompatModel {
    async fn complete(&self, request: CompletionRequest) -> ProviderResult<Completion> {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user},
            ],
            "temperature": request.temperature,
            "stream": false,
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if request.json && self.json_mode {
            body["response_format"] = serde_json::json!({"type": "json_object"});
        }

        let mut last_transport: Option<ProviderError> = None;
        for _ in 0..3 {
            let response = self
                .client
                .post(self.endpoint())
                .bearer_auth(&self.api_key)
                .json(&body)
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    last_transport = Some(ProviderError::Transport(error.to_string()));
                    continue;
                }
            };

            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                return Err(ProviderError::Auth);
            }
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(ProviderError::RateLimit);
            }

            let text = response
                .text()
                .await
                .map_err(|e| ProviderError::Transport(e.to_string()))?;

            if !status.is_success() {
                return Err(ProviderError::Status {
                    status: status.as_u16(),
                    body: truncate(&text),
                });
            }

            let parsed: ChatResponse = serde_json::from_str(&text)
                .map_err(|e| ProviderError::Decode(format!("{e}: {}", truncate(&text))))?;

            let choice = parsed.choices.into_iter().next().ok_or_else(|| {
                ProviderError::Decode("response contained no choices".to_string())
            })?;

            if choice.finish_reason.as_deref() == Some("content_filter") {
                return Err(ProviderError::Refusal(
                    "provider filtered the completion".to_string(),
                ));
            }

            let content = choice.message.content.unwrap_or_default();
            if content.trim().is_empty() {
                return Err(ProviderError::Refusal(
                    "provider returned empty content".to_string(),
                ));
            }
            return Ok(Completion { text: content });
        }

        Err(last_transport
            .unwrap_or_else(|| ProviderError::Transport("request failed".to_string())))
    }

    fn name(&self) -> &str {
        &self.model
    }

    fn supports_json_mode(&self) -> bool {
        self.json_mode
    }
}

fn truncate(text: &str) -> String {
    const LIMIT: usize = 500;
    if text.len() <= LIMIT {
        text.to_string()
    } else {
        format!("{}…", &text[..LIMIT])
    }
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_is_joined_without_double_slash() {
        let model = OpenAiCompatModel::new("https://api.deepseek.com/", "k", "deepseek-flash");
        assert_eq!(
            model.endpoint(),
            "https://api.deepseek.com/chat/completions"
        );
    }

    #[test]
    fn parses_a_completion_response() {
        let raw = r#"{"choices":[{"message":{"content":"hello"},"finish_reason":"stop"}]}"#;
        let parsed: ChatResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.choices[0].message.content.as_deref(), Some("hello"));
    }
}
