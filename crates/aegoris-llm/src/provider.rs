use async_trait::async_trait;

use crate::error::ProviderResult;

/// A single completion request.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionRequest {
    pub system: String,
    pub user: String,
    /// Ask the provider for a JSON object response when it supports it.
    pub json: bool,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
}

impl CompletionRequest {
    pub fn new(system: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            user: user.into(),
            json: false,
            temperature: 0.0,
            max_tokens: None,
        }
    }

    pub fn json(mut self) -> Self {
        self.json = true;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub text: String,
}

/// A text-generation backend.
///
/// Implementations are the only place in aegoris that touch the network, which
/// keeps the deterministic core testable and offline.
#[async_trait]
pub trait LanguageModel: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> ProviderResult<Completion>;

    /// Human-readable provider/model identifier, for logs and audit records.
    fn name(&self) -> &str;

    /// Whether the backend honors a native JSON response mode.
    fn supports_json_mode(&self) -> bool {
        false
    }
}
