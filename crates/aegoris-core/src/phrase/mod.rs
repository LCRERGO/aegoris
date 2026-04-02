//! Phrasing: turning a `CurationPlan` into grounded `Claim`s.
//!
//! The deterministic implementation ([`TemplatePhraser`]) is always available
//! and needs no network. The LLM-backed implementation lives in `aegoris-llm`.

pub mod template;

use crate::domain::artifact::ArtifactKind;
use crate::domain::fact::FactStore;
use crate::domain::job::JobDescription;
use crate::domain::profile::Profile;
use crate::domain::CurationPlan;
use crate::error::CoreResult;

pub use template::TemplatePhraser;

/// Everything a phraser needs to produce claims for one artifact.
pub struct PhraseContext<'a> {
    pub kind: ArtifactKind,
    pub profile: &'a Profile,
    pub jd: &'a JobDescription,
    pub plan: &'a CurationPlan,
    pub store: &'a FactStore,
}

/// Turns selected facts into grounded, artifact-shaped claims.
pub trait Phraser {
    fn phrase(&self, context: &PhraseContext<'_>) -> CoreResult<Vec<crate::domain::Claim>>;
}
