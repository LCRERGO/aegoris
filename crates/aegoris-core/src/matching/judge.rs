use crate::domain::job::Requirement;
use crate::domain::Fact;
use crate::error::CoreResult;

/// A tie-breaker for the genuinely ambiguous cases lexical and embedding
/// matching cannot separate.
///
/// The default implementation does nothing, which keeps the deterministic path
/// free of any model. An LLM-backed judge can be supplied for the tail.
pub trait RequirementJudge {
    /// Return a relevance score in `0.0..=1.0` for one fact/requirement pair.
    fn judge(&self, fact: &Fact, requirement: &Requirement) -> CoreResult<f32>;
}

#[derive(Debug, Clone, Default)]
pub struct NoopJudge;

impl RequirementJudge for NoopJudge {
    fn judge(&self, _fact: &Fact, _requirement: &Requirement) -> CoreResult<f32> {
        Ok(0.0)
    }
}
