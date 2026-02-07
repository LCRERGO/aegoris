use serde::{Deserialize, Serialize};

use crate::domain::fact::FactStore;
use crate::domain::ids::{FactId, RequirementId, RoleId};
use crate::domain::job::Requirement;
use crate::domain::profile::Profile;

/// A fact paired with its computed relevance to the target job description.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredFact {
    pub fact_id: FactId,
    pub score: f32,
    #[serde(default)]
    pub matched_requirements: Vec<RequirementId>,
}

/// The deterministic decision layer: which facts are used, and in what order.
///
/// A plan is fully reproducible from its inputs and contains no generated
/// language. Phrasing happens after curation, over the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurationPlan {
    pub requirements: Vec<Requirement>,
    pub scored: Vec<ScoredFact>,
    /// Selected fact IDs in relevance order.
    pub selected_fact_ids: Vec<FactId>,
    /// Roles in presentation order (reverse-chronological).
    pub role_order: Vec<RoleId>,
    pub max_bullets_per_role: usize,
}

impl CurationPlan {
    pub fn selected_facts<'a>(&self, store: &'a FactStore) -> Vec<&'a crate::domain::fact::Fact> {
        self.selected_fact_ids
            .iter()
            .filter_map(|id| store.get(id))
            .collect()
    }

    pub fn score_of(&self, fact_id: &FactId) -> Option<f32> {
        self.scored
            .iter()
            .find(|s| &s.fact_id == fact_id)
            .map(|s| s.score)
    }

    /// Group selected facts by the role they belong to, in `role_order`.
    pub fn facts_for_role<'a>(
        &self,
        store: &'a FactStore,
        role_id: &RoleId,
    ) -> Vec<&'a crate::domain::fact::Fact> {
        self.selected_facts(store)
            .into_iter()
            .filter(|fact| {
                matches!(
                    &fact.source,
                    crate::domain::fact::FactSource::Role { role_id: r, .. } if r == role_id
                )
            })
            .collect()
    }

    /// Roles in reverse-chronological order, as derived from the profile.
    pub fn role_order_for(profile: &Profile) -> Vec<RoleId> {
        let mut roles: Vec<&crate::domain::profile::Role> = profile.roles.iter().collect();
        roles.sort_by(|a, b| {
            let a_current = a.current;
            let b_current = b.current;
            b_current
                .cmp(&a_current)
                .then_with(|| sort_key(&b.end).cmp(&sort_key(&a.end)))
                .then_with(|| sort_key(&b.start).cmp(&sort_key(&a.start)))
        });
        roles.into_iter().map(|r| r.id.clone()).collect()
    }
}

fn sort_key(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}
