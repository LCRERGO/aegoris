use std::collections::HashSet;

use crate::domain::job::Requirement;
use crate::domain::Fact;
use crate::error::CoreResult;
use crate::matching::{FactScore, RelevanceScorer};

const STOPWORDS: &[&str] = &[
    "a",
    "an",
    "and",
    "are",
    "as",
    "at",
    "be",
    "by",
    "for",
    "from",
    "in",
    "into",
    "is",
    "it",
    "of",
    "on",
    "or",
    "our",
    "that",
    "the",
    "their",
    "to",
    "with",
    "you",
    "your",
    "we",
    "will",
    "have",
    "has",
    "using",
    "use",
    "strong",
    "proficient",
    "proficiency",
    "experience",
    "experienced",
    "ability",
    "able",
    "work",
    "working",
    "plus",
    "nice",
    "must",
    "should",
];

/// A small alias map so `k8s`, `js`, `postgres`, etc. match their canonical form.
const SYNONYMS: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("ts", "typescript"),
    ("k8s", "kubernetes"),
    ("postgres", "postgresql"),
    ("postgre", "postgresql"),
    ("golang", "go"),
    ("py", "python"),
    ("ml", "machine-learning"),
    ("ai", "artificial-intelligence"),
    ("ci", "continuous-integration"),
    ("cd", "continuous-delivery"),
    ("aws", "amazon-web-services"),
    ("gcp", "google-cloud"),
    ("rest", "rest-api"),
    ("apis", "api"),
    ("microservices", "microservice"),
    ("tests", "testing"),
    ("tdd", "test-driven-development"),
];

/// Deterministic lexical scorer: token overlap with synonym normalization.
#[derive(Debug, Clone, Default)]
pub struct LexicalScorer;

impl LexicalScorer {
    pub fn new() -> Self {
        Self
    }
}

impl RelevanceScorer for LexicalScorer {
    fn score(&self, fact: &Fact, requirements: &[Requirement]) -> CoreResult<FactScore> {
        let fact_terms = normalize_terms(&fact.text);
        if fact_terms.is_empty() {
            return Ok(FactScore::zero());
        }

        let mut total = 0.0f32;
        let mut matched = Vec::new();

        for requirement in requirements {
            let req_terms = normalize_terms(&requirement.text);
            if req_terms.is_empty() {
                continue;
            }
            let overlap = req_terms.intersection(&fact_terms).count();
            if overlap == 0 {
                continue;
            }
            // Square-root denominator: long requirements should not be
            // impossible to satisfy, but a one-token coincidence shouldn't
            // dominate either.
            let contribution =
                (overlap as f32 / (req_terms.len() as f32).sqrt()) * requirement.weight;
            total += contribution.min(1.0);
            matched.push(requirement.id.clone());
        }

        Ok(FactScore {
            score: total,
            matched,
        })
    }
}

/// Normalize text into canonical term tokens for overlap comparison.
pub fn normalize_terms(text: &str) -> HashSet<String> {
    crate::domain::fact::tokenize(text)
        .into_iter()
        .filter(|token| !STOPWORDS.contains(&token.as_str()))
        .map(|token| stem(&canonical(&token)))
        .collect()
}

fn canonical(token: &str) -> String {
    for (alias, canonical) in SYNONYMS {
        if token == *alias {
            return (*canonical).to_string();
        }
    }
    token.to_string()
}

/// Very light suffix stripping so `algorithm`/`algorithms` and
/// `cluster`/`clusters` compare equal.
fn stem(token: &str) -> String {
    if token.len() > 4 && !token.ends_with("ss") {
        if let Some(stripped) = token.strip_suffix("ies") {
            return format!("{stripped}y");
        }
        if let Some(stripped) = token.strip_suffix("es") {
            return stripped.to_string();
        }
        if let Some(stripped) = token.strip_suffix('s') {
            return stripped.to_string();
        }
    }
    token.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::{FactId, RequirementId};
    use crate::domain::job::RequirementKind;
    use crate::domain::FactSource;

    fn fact(text: &str) -> Fact {
        Fact {
            id: FactId::generate(1),
            text: text.to_string(),
            source: FactSource::Summary,
            section: crate::domain::Section::Summary,
            tags: crate::domain::fact::tokenize(text),
        }
    }

    fn req(id: usize, text: &str) -> Requirement {
        Requirement::new(RequirementId::generate(id), text, RequirementKind::Skill)
    }

    #[test]
    fn matching_fact_scores_above_unrelated() {
        let scorer = LexicalScorer::new();
        let requirements = vec![
            req(1, "Strong proficiency in Rust and async programming"),
            req(2, "Experience with PostgreSQL"),
        ];
        let relevant = scorer
            .score(&fact("Built async services in Rust"), &requirements)
            .unwrap();
        let irrelevant = scorer
            .score(&fact("Managed a team of baristas"), &requirements)
            .unwrap();
        assert!(relevant.score > irrelevant.score);
        assert_eq!(relevant.matched, vec![RequirementId::generate(1)]);
        assert!(irrelevant.matched.is_empty());
    }

    #[test]
    fn synonyms_are_normalized() {
        let scorer = LexicalScorer::new();
        let requirements = vec![req(1, "Kubernetes")];
        let score = scorer
            .score(&fact("Operated k8s clusters"), &requirements)
            .unwrap();
        assert!(score.score > 0.0);
    }

    #[test]
    fn scoring_is_deterministic() {
        let scorer = LexicalScorer::new();
        let requirements = vec![req(1, "Rust")];
        let a = scorer.score(&fact("Rust and Go"), &requirements).unwrap();
        let b = scorer.score(&fact("Rust and Go"), &requirements).unwrap();
        assert_eq!(a, b);
    }
}
