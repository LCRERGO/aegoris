use serde::{Deserialize, Serialize};

/// Which document an artifact is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Resume,
    CoverLetter,
}

impl ArtifactKind {
    pub fn slug(&self) -> &'static str {
        match self {
            ArtifactKind::Resume => "resume",
            ArtifactKind::CoverLetter => "cover-letter",
        }
    }
}

/// The serialization a rendered artifact is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Markdown,
    Ats,
    Latex,
    Pdf,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Markdown => "md",
            OutputFormat::Ats => "txt",
            OutputFormat::Latex => "tex",
            OutputFormat::Pdf => "pdf",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "md" | "markdown" => Some(OutputFormat::Markdown),
            "txt" | "text" | "ats" => Some(OutputFormat::Ats),
            "tex" | "latex" => Some(OutputFormat::Latex),
            "pdf" => Some(OutputFormat::Pdf),
            _ => None,
        }
    }
}

/// A rendered document, ready to be written to disk.
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub format: OutputFormat,
    pub content: Vec<u8>,
}

impl Artifact {
    pub fn new(kind: ArtifactKind, format: OutputFormat, content: impl Into<Vec<u8>>) -> Self {
        Self {
            kind,
            format,
            content: content.into(),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.content).ok()
    }
}
