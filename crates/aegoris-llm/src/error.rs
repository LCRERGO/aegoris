use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider authentication failed")]
    Auth,

    #[error("provider rate limit exceeded")]
    RateLimit,

    #[error("provider transport error: {0}")]
    Transport(String),

    #[error("provider returned an undecodable response: {0}")]
    Decode(String),

    #[error("provider returned status {status}: {body}")]
    Status { status: u16, body: String },

    #[error("provider declined the request: {0}")]
    Refusal(String),

    #[error("provider configuration error: {0}")]
    Config(String),
}

pub type ProviderResult<T> = Result<T, ProviderError>;

use thiserror::Error as ThisError;

use aegoris_core::CoreError;

/// Errors from the LLM-backed stages.
#[derive(Debug, ThisError)]
pub enum LlmError {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error(transparent)]
    Core(#[from] CoreError),

    #[error("failed to parse model output: {0}")]
    Parse(String),

    #[error("model output failed grounding: {0}")]
    Grounding(String),
}

pub type LlmResult<T> = Result<T, LlmError>;
