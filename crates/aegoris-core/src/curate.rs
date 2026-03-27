use crate::domain::curation::{CurationPlan, ScoredFact};
use crate::domain::fact::FactStore;
use crate::domain::ids::FactId;
use crate::domain::job::JobDescription;
use crate::domain::profile::Profile;
use crate::domain::Section;
use crate::error::CoreResult;
use crate::matching::RelevanceScorer;

/// Tunables for the deterministic curation step.
#[derive(Debug, Clone, PartialEq)]
pub struct CurationConfig {
    /// Maximum number of scored facts kept (education and certifications are
    /// always kept and do not count against this budget).
    pub max_facts: usize,
    pub max_bullets_per_role: usize,
    pub min_score: f32,
    pub always_include_summary: bool,
    pub always_include_education: bool,
    pub always_include_certifications: bool,
}

impl Default for CurationConfig {
    fn default() -> Self {
        Self {
            max_facts: 24,
            max_bullets_per_role: 4,
            min_score: 0.0,
            always_include_summary: true,
            always_include_education: true,
            always_include_certifications: true,
        }
    }
}

/// Score every fact, then select and order the ones that make the cut.
///
/// The result is a pure function of its inputs: same profile, job description,
/// scorer, and config always produce the same plan.
pub fn curate(
    profile: &Profile,
    jd: &JobDescription,
    store: &FactStore,
    scorer: &dyn RelevanceScorer,
    config: &CurationConfig,
) -> CoreResult<CurationPlan> {
    let mut scored = Vec::with_capacity(store.len());
    for fact in store.iter() {
        let score = scorer.score(fact, &jd.requirements)?;
        scored.push(ScoredFact {
            fact_id: fact.id.clone(),
            score: score.score,
            matched_requirements: score.matched,
        });
    }

    let mut selected: Vec<(Section, f32, FactId)> = Vec::new();
    for fact in store.iter() {
        let score = scored
            .iter()
            .find(|s| s.fact_id == fact.id)
            .map(|s| s.score)
            .unwrap_or(0.0);
        let always = (fact.section == Section::Summary && config.always_include_summary)
            || (fact.section == Section::Education && config.always_include_education)
            || (fact.section == Section::Certifications && config.always_include_certifications);
        if always || score > config.min_score {
            selected.push((fact.section, score, fact.id.clone()));
        }
    }

    // Order: section priority first, then descending score, then ID for a
    // stable tie-break.
    selected.sort_by(|a, b| {
        section_priority(a.0)
            .cmp(&section_priority(b.0))
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.2.cmp(&b.2))
    });

    // Apply the budget to scored (non-education, non-certification) facts.
    let mut budget = config.max_facts;
    let mut selected_fact_ids = Vec::new();
    for (section, _, id) in &selected {
        let always = (*section == Section::Summary && config.always_include_summary)
            || (*section == Section::Education && config.always_include_education)
            || (*section == Section::Certifications && config.always_include_certifications);
        if always {
            selected_fact_ids.push(id.clone());
        } else if budget > 0 {
            budget -= 1;
            selected_fact_ids.push(id.clone());
        }
    }

    let role_order: Vec<_> = CurationPlan::role_order_for(profile)
        .into_iter()
        .filter(|role_id| {
            store.iter().any(|fact| {
                selected_fact_ids.contains(&fact.id)
                    && matches!(
                        &fact.source,
                        crate::domain::fact::FactSource::Role { role_id: r, .. } if r == role_id
                    )
            })
        })
        .collect();

    Ok(CurationPlan {
        requirements: jd.requirements.clone(),
        scored,
        selected_fact_ids,
        role_order,
        max_bullets_per_role: config.max_bullets_per_role,
    })
}

fn section_priority(section: Section) -> u8 {
    match section {
        Section::Summary => 0,
        Section::Experience => 1,
        Section::Projects => 2,
        Section::Skills => 3,
        Section::Education => 4,
        Section::Certifications => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fact::FactStore;
    use crate::matching::LexicalScorer;
    use crate::parse::{parse_job_description, parse_profile_json};

    fn profile() -> Profile {
        parse_profile_json(
            r#"{
                "name": "Ada Lovelace",
                "summary": "Pioneering programmer",
                "roles": [
                    {"id":"r_1","title":"Engineer","organization":"Babbage","current":false,
                     "end":"1843","start":"1842",
                     "bullets":["Wrote the first algorithm for the Analytical Engine",
                                "Published extensive notes on computation"]},
                    {"id":"r_2","title":"Senior Engineer","organization":"Analytical Co","current":true,
                     "start":"1844",
                     "bullets":["Designed looping constructs for the engine"]}
                ],
                "skills": [{"name":"Rust"},{"name":"Mathematics"}],
                "education": [{"institution":"University of London","degree":"Mathematics"}]
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn plan_is_deterministic_and_roles_reverse_chronological() {
        let profile = profile();
        let store = FactStore::from_profile(&profile);
        let jd = parse_job_description("Requirements:\n- Rust\n- Algorithms\n- looping constructs");
        let scorer = LexicalScorer::new();
        let config = CurationConfig::default();
        let a = curate(&profile, &jd, &store, &scorer, &config).unwrap();
        let b = curate(&profile, &jd, &store, &scorer, &config).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.role_order[0], crate::domain::RoleId::new("r_2"));
    }

    #[test]
    fn education_is_always_included() {
        let profile = profile();
        let store = FactStore::from_profile(&profile);
        let jd = parse_job_description("Requirements:\n- COBOL");
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let education = store
            .iter()
            .find(|f| f.section == Section::Education)
            .unwrap();
        assert!(plan.selected_fact_ids.contains(&education.id));
    }
}
