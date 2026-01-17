use thiserror::Error;

/// Errors originating in the deterministic core.
///
/// The core performs no I/O and no async work, so every failure here is a
/// data-shape or invariant failure rather than a transport failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("grounding error: {0}")]
    Grounding(String),

    #[error("config error: {0}")]
    Config(String),
}

impl CoreError {
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn grounding(msg: impl Into<String>) -> Self {
        Self::Grounding(msg.into())
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
