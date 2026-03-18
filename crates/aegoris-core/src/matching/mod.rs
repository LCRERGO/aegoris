//! Relevance matching between profile facts and job requirements.

pub mod embedding;
pub mod hybrid;
pub mod judge;
pub mod lexical;

use serde::{Deserialize, Serialize};

use crate::domain::ids::RequirementId;
use crate::domain::job::Requirement;
use crate::domain::Fact;
use crate::error::CoreResult;

pub use embedding::{cosine_similarity, CachedEmbedder, Embedder, HashingEmbedder};
pub use hybrid::HybridScorer;
pub use judge::{NoopJudge, RequirementJudge};
pub use lexical::LexicalScorer;

/// The relevance of one fact to the target requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactScore {
    pub score: f32,
    #[serde(default)]
    pub matched: Vec<RequirementId>,
}

impl FactScore {
    pub fn zero() -> Self {
        Self {
            score: 0.0,
            matched: Vec::new(),
        }
    }
}

/// Scores how relevant a fact is to a set of requirements.
///
/// Implementations are pure and deterministic so they can be tested without
/// fixtures or network access.
pub trait RelevanceScorer {
    fn score(&self, fact: &Fact, requirements: &[Requirement]) -> CoreResult<FactScore>;
}
