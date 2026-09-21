//! LLM providers and model-backed stages for aegoris.
//!
//! Everything that talks to a model lives here. The deterministic core
//! (`aegoris-core`) stays offline; this crate is the only place with a network
//! dependency, and its tests run against [`FakeLanguageModel`].

pub mod embed;
pub mod error;
pub mod fake;
pub mod openai_compat;
pub mod phrase;
pub mod provider;
pub mod structure;
pub mod verify;

pub use embed::{AsyncEmbedder, OpenAiCompatEmbedder};
pub use error::{LlmError, LlmResult, ProviderError, ProviderResult};
pub use fake::FakeLanguageModel;
pub use openai_compat::{deepseek, OpenAiCompatModel, DEFAULT_BASE_URL, DEFAULT_MODEL};
pub use phrase::LlmPhraser;
pub use provider::{Completion, CompletionRequest, LanguageModel};
pub use structure::LlmProfileStructurer;
pub use verify::LlmClaimVerifier;
