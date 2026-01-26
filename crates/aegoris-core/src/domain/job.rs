use serde::{Deserialize, Serialize};

use crate::domain::ids::RequirementId;

/// A target posting. `raw` is preserved verbatim; `requirements` are the
/// atomic needs extracted from it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct JobDescription {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    pub raw: String,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
}

/// An atomic need extracted from a job description.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    pub id: RequirementId,
    pub text: String,
    pub kind: RequirementKind,
    /// Relative importance, defaulting to `1.0`.
    pub weight: f32,
}

impl Requirement {
    pub fn new(id: RequirementId, text: impl Into<String>, kind: RequirementKind) -> Self {
        Self {
            id,
            text: text.into(),
            kind,
            weight: 1.0,
        }
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementKind {
    Skill,
    Experience,
    Education,
    Certification,
    Responsibility,
    Other,
}
