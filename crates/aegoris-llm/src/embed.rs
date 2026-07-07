use async_trait::async_trait;
use serde::Deserialize;

use crate::error::{ProviderError, ProviderResult};

/// An embedding backend. Unlike [`crate::LanguageModel`], this is used to
/// prefetch vectors before the deterministic pipeline runs.
#[async_trait]
pub trait AsyncEmbedder: Send + Sync {
    async fn embed(&self, texts: &[String]) -> ProviderResult<Vec<Vec<f32>>>;

    fn name(&self) -> &str;
}

/// Adapter for any OpenAI-compatible `/embeddings` endpoint.
///
/// DeepSeek does not offer an embeddings API, so this is normally pointed at
/// OpenAI, a local Ollama instance, or another OpenAI-compatible provider via
/// `--embeddings-base-url`.
pub struct OpenAiCompatEmbedder {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAiCompatEmbedder {
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
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/embeddings", self.base_url)
    }
}

#[async_trait]
impl AsyncEmbedder for OpenAiCompatEmbedder {
    async fn embed(&self, texts: &[String]) -> ProviderResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let body = serde_json::json!({
            "model": self.model,
            "input": texts,
        });

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

            let parsed: EmbeddingResponse = serde_json::from_str(&text)
                .map_err(|e| ProviderError::Decode(format!("{e}: {}", truncate(&text))))?;

            let mut data = parsed.data;
            data.sort_by_key(|entry| entry.index);
            let vectors: Vec<Vec<f32>> = data.into_iter().map(|entry| entry.embedding).collect();

            if vectors.len() != texts.len() {
                return Err(ProviderError::Decode(format!(
                    "expected {} embeddings, received {}",
                    texts.len(),
                    vectors.len()
                )));
            }
            return Ok(vectors);
        }

        Err(last_transport
            .unwrap_or_else(|| ProviderError::Transport("request failed".to_string())))
    }

    fn name(&self) -> &str {
        &self.model
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
struct EmbeddingResponse {
    #[serde(default)]
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_sorts_embeddings_by_index() {
        let raw = r#"{"data":[
            {"index":1,"embedding":[0.0,1.0]},
            {"index":0,"embedding":[1.0,0.0]}
        ]}"#;
        let parsed: EmbeddingResponse = serde_json::from_str(raw).unwrap();
        let mut data = parsed.data;
        data.sort_by_key(|entry| entry.index);
        assert_eq!(data[0].embedding, vec![1.0, 0.0]);
        assert_eq!(data[1].embedding, vec![0.0, 1.0]);
    }

    #[test]
    fn endpoint_is_joined_without_double_slash() {
        let embedder =
            OpenAiCompatEmbedder::new("https://api.openai.com/v1/", "k", "text-embedding-3-small");
        assert_eq!(embedder.endpoint(), "https://api.openai.com/v1/embeddings");
    }
}
