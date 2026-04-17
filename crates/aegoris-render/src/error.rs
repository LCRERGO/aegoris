use thiserror::Error;

use aegoris_core::CoreError;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("rendering failed: {0}")]
    Failed(String),

    #[error(transparent)]
    Core(#[from] CoreError),
}

pub type RenderResult<T> = Result<T, RenderError>;
