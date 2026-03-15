use crate::domain::ids::RequirementId;
use crate::domain::job::Requirement;
use crate::domain::Fact;
use crate::error::CoreResult;
use crate::matching::embedding::semantic_similarities;
use crate::matching::judge::RequirementJudge;
use crate::matching::lexical::LexicalScorer;
use crate::matching::{Embedder, FactScore, RelevanceScorer};

/// Three-stage matcher: lexical always, embeddings when available, judge for
/// the tail. Stages are additive and each is optional, so the scorer degrades
/// gracefully to pure lexical matching.
pub struct HybridScorer {
    lexical: LexicalScorer,
    embedder: Option<Box<dyn Embedder>>,
    judge: Option<Box<dyn RequirementJudge>>,
    lexical_weight: f32,
    semantic_weight: f32,
    judge_weight: f32,
    semantic_threshold: f32,
    judge_threshold: f32,
    max_score: f32,
}

impl Default for HybridScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridScorer {
    pub fn new() -> Self {
        Self {
            lexical: LexicalScorer::new(),
            embedder: None,
            judge: None,
            lexical_weight: 1.0,
            semantic_weight: 1.0,
            judge_weight: 1.0,
            semantic_threshold: 0.35,
            judge_threshold: 0.5,
            max_score: 10.0,
        }
    }

    pub fn with_embedder(mut self, embedder: Box<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    pub fn with_judge(mut self, judge: Box<dyn RequirementJudge>) -> Self {
        self.judge = Some(judge);
        self
    }

    pub fn with_semantic_threshold(mut self, threshold: f32) -> Self {
        self.semantic_threshold = threshold;
        self
    }

    pub fn lexical_only() -> Self {
        Self::new()
    }
}

impl RelevanceScorer for HybridScorer {
    fn score(&self, fact: &Fact, requirements: &[Requirement]) -> CoreResult<FactScore> {
        let lexical = self.lexical.score(fact, requirements)?;
        let mut score = lexical.score * self.lexical_weight;
        let mut matched: Vec<RequirementId> = lexical.matched;

        if let Some(embedder) = self.embedder.as_deref() {
            for (requirement_id, similarity) in
                semantic_similarities(embedder, &fact.text, requirements)?
            {
                if similarity >= self.semantic_threshold {
                    let weight = requirements
                        .iter()
                        .find(|r| r.id == requirement_id)
                        .map(|r| r.weight)
                        .unwrap_or(1.0);
                    score += similarity * weight * self.semantic_weight;
                    if !matched.contains(&requirement_id) {
                        matched.push(requirement_id);
                    }
                }
            }
        }

        if let Some(judge) = self.judge.as_deref() {
            for requirement in requirements {
                if matched.contains(&requirement.id) {
                    continue;
                }
                let verdict = judge.judge(fact, requirement)?;
                if verdict >= self.judge_threshold {
                    score += verdict * requirement.weight * self.judge_weight;
                    matched.push(requirement.id.clone());
                }
            }
        }

        Ok(FactScore {
            score: score.clamp(0.0, self.max_score),
            matched,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::FactId;
    use crate::domain::job::RequirementKind;
    use crate::domain::{FactSource, Section};
    use crate::matching::HashingEmbedder;

    fn fact(text: &str) -> Fact {
        Fact {
            id: FactId::generate(1),
            text: text.to_string(),
            source: FactSource::Summary,
            section: Section::Summary,
            tags: Vec::new(),
        }
    }

    fn req(id: usize, text: &str) -> Requirement {
        Requirement::new(RequirementId::generate(id), text, RequirementKind::Skill)
    }

    #[test]
    fn lexical_only_matches_expected() {
        let scorer = HybridScorer::lexical_only();
        let requirements = vec![req(1, "Rust"), req(2, "Kubernetes")];
        let score = scorer.score(&fact("Rust services"), &requirements).unwrap();
        assert!(score.matched.contains(&RequirementId::generate(1)));
        assert!(!score.matched.contains(&RequirementId::generate(2)));
    }

    #[test]
    fn embedding_stage_adds_similarity() {
        let scorer = HybridScorer::new().with_embedder(Box::new(HashingEmbedder::default()));
        let requirements = vec![req(1, "Rust async")];
        let score = scorer.score(&fact("Rust async"), &requirements).unwrap();
        assert!(score.score >= 1.0);
    }

    #[test]
    fn judge_only_contributes_for_unmatched_requirements() {
        struct Always;
        impl RequirementJudge for Always {
            fn judge(&self, _f: &Fact, _r: &Requirement) -> CoreResult<f32> {
                Ok(1.0)
            }
        }
        let scorer = HybridScorer::new().with_judge(Box::new(Always));
        let requirements = vec![req(1, "COBOL")];
        let score = scorer.score(&fact("Rust services"), &requirements).unwrap();
        assert!(score.matched.contains(&RequirementId::generate(1)));
    }
}
