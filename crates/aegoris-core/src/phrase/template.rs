use crate::domain::artifact::ArtifactKind;
use crate::domain::evidence::Evidence;
use crate::domain::fact::FactSource;
use crate::domain::Claim;
use crate::error::CoreResult;
use crate::phrase::{PhraseContext, Phraser};

/// Deterministic phrasing: each claim is a selected fact, lightly normalized.
///
/// Because claims are mechanically derived from their evidence, the semantic
/// grounding verifier is a provable no-op in this mode.
#[derive(Debug, Clone, Default)]
pub struct TemplatePhraser;

impl TemplatePhraser {
    pub fn new() -> Self {
        Self
    }
}

impl Phraser for TemplatePhraser {
    fn phrase(&self, context: &PhraseContext<'_>) -> CoreResult<Vec<Claim>> {
        let limit = match context.kind {
            ArtifactKind::Resume => usize::MAX,
            ArtifactKind::CoverLetter => 4,
        };

        let mut claims = Vec::new();
        for fact in context.plan.selected_facts(context.store) {
            if claims.len() >= limit {
                break;
            }
            let role_id = match &fact.source {
                FactSource::Role { role_id, .. } => Some(role_id.clone()),
                _ => None,
            };
            claims.push(Claim::new(
                normalize(&fact.text),
                vec![Evidence::new(fact.id.clone())],
                fact.section,
                role_id,
            )?);
        }
        Ok(claims)
    }
}

fn normalize(text: &str) -> String {
    let trimmed = text.trim().trim_end_matches('.').trim();
    let mut chars = trimmed.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curate::{curate, CurationConfig};
    use crate::domain::fact::FactStore;
    use crate::grounding::{ClaimVerifier, StructuralVerifier};
    use crate::matching::LexicalScorer;
    use crate::parse::{parse_job_description, parse_profile_json};

    fn setup() -> (
        crate::domain::Profile,
        crate::domain::JobDescription,
        FactStore,
    ) {
        let profile = parse_profile_json(
            r#"{"name":"Ada","summary":"Pioneering programmer",
                "roles":[{"id":"r_1","title":"Engineer","organization":"Babbage",
                "bullets":["wrote the first algorithm"]}]}"#,
        )
        .unwrap();
        let jd = parse_job_description("Requirements:\n- Algorithms");
        let store = FactStore::from_profile(&profile);
        (profile, jd, store)
    }

    #[test]
    fn template_claims_are_grounded() {
        let (profile, jd, store) = setup();
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let ctx = PhraseContext {
            kind: ArtifactKind::Resume,
            profile: &profile,
            jd: &jd,
            plan: &plan,
            store: &store,
        };
        let claims = TemplatePhraser::new().phrase(&ctx).unwrap();
        assert!(!claims.is_empty());
        let verifications = StructuralVerifier.verify(&claims, &store).unwrap();
        assert!(verifications.iter().all(|v| v.supported));
    }

    #[test]
    fn normalization_capitalizes_and_trims() {
        assert_eq!(normalize("wrote it."), "Wrote it");
        assert_eq!(normalize("  already Cap  "), "Already Cap");
    }
}
