pub mod artifact;
pub mod claim;
pub mod curation;
pub mod evidence;
pub mod fact;
pub mod ids;
pub mod job;
pub mod profile;

use serde::{Deserialize, Serialize};

/// A named region of a resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    #[default]
    Summary,
    Experience,
    Projects,
    Skills,
    Education,
    Certifications,
}

impl Section {
    pub fn title(&self) -> &'static str {
        match self {
            Section::Summary => "Summary",
            Section::Experience => "Experience",
            Section::Projects => "Projects",
            Section::Skills => "Skills",
            Section::Education => "Education",
            Section::Certifications => "Certifications",
        }
    }

    /// Machine-readable name, matching the serde representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Section::Summary => "summary",
            Section::Experience => "experience",
            Section::Projects => "projects",
            Section::Skills => "skills",
            Section::Education => "education",
            Section::Certifications => "certifications",
        }
    }
}

pub use artifact::{Artifact, ArtifactKind, OutputFormat};
pub use claim::Claim;
pub use curation::{CurationPlan, ScoredFact};
pub use evidence::Evidence;
pub use fact::{Fact, FactSource, FactStore};
pub use ids::{FactId, RequirementId, RoleId};
pub use job::{JobDescription, Requirement, RequirementKind};
pub use profile::{Certification, Contact, Education, Profile, Project, Role, Skill};
