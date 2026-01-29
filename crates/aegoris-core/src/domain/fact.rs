use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::ids::{FactId, RoleId};
use crate::domain::profile::Profile;
use crate::domain::Section;

/// An atomic, profile-sourced, groundable statement.
///
/// Facts are the only legitimate source of content for generated claims. They
/// are derived deterministically from a `Profile`, which is what makes
/// grounding mechanically checkable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fact {
    pub id: FactId,
    pub text: String,
    pub source: FactSource,
    pub section: Section,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Where a fact came from in the source profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FactSource {
    Summary,
    Role {
        role_id: RoleId,
        bullet_index: usize,
    },
    RoleSkill {
        role_id: RoleId,
        skill: String,
    },
    Project {
        project_index: usize,
        bullet_index: Option<usize>,
    },
    Skill {
        index: usize,
    },
    Education {
        index: usize,
    },
    Certification {
        index: usize,
    },
}

/// An indexed collection of every fact derivable from a profile.
#[derive(Debug, Clone, Default)]
pub struct FactStore {
    facts: Vec<Fact>,
    index: HashMap<FactId, usize>,
}

impl FactStore {
    /// Derive the full fact set from a profile.
    ///
    /// Ordering is fixed (summary, roles, projects, skills, education,
    /// certifications) so that fact IDs are stable across runs.
    pub fn from_profile(profile: &Profile) -> Self {
        let mut facts = Vec::new();
        let mut seen_skills: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Some(summary) = profile.summary.as_deref().map(str::trim) {
            if !summary.is_empty() {
                facts.push(make_fact(
                    facts.len() + 1,
                    summary.to_string(),
                    FactSource::Summary,
                    Section::Summary,
                ));
            }
        }

        for role in &profile.roles {
            for (bullet_index, bullet) in role.bullets.iter().enumerate() {
                let text = bullet.trim();
                if text.is_empty() {
                    continue;
                }
                facts.push(make_fact(
                    facts.len() + 1,
                    text.to_string(),
                    FactSource::Role {
                        role_id: role.id.clone(),
                        bullet_index,
                    },
                    Section::Experience,
                ));
            }
            for skill in &role.skills {
                let text = skill.trim();
                if text.is_empty() || !seen_skills.insert(text.to_lowercase()) {
                    continue;
                }
                facts.push(make_fact(
                    facts.len() + 1,
                    text.to_string(),
                    FactSource::RoleSkill {
                        role_id: role.id.clone(),
                        skill: text.to_string(),
                    },
                    Section::Skills,
                ));
            }
        }

        for (project_index, project) in profile.projects.iter().enumerate() {
            if let Some(description) = project.description.as_deref().map(str::trim) {
                if !description.is_empty() {
                    facts.push(make_fact(
                        facts.len() + 1,
                        description.to_string(),
                        FactSource::Project {
                            project_index,
                            bullet_index: None,
                        },
                        Section::Projects,
                    ));
                }
            }
            for (bullet_index, bullet) in project.bullets.iter().enumerate() {
                let text = bullet.trim();
                if text.is_empty() {
                    continue;
                }
                facts.push(make_fact(
                    facts.len() + 1,
                    text.to_string(),
                    FactSource::Project {
                        project_index,
                        bullet_index: Some(bullet_index),
                    },
                    Section::Projects,
                ));
            }
        }

        for (index, skill) in profile.skills.iter().enumerate() {
            let text = skill.name.trim();
            if text.is_empty() || !seen_skills.insert(text.to_lowercase()) {
                continue;
            }
            facts.push(make_fact(
                facts.len() + 1,
                text.to_string(),
                FactSource::Skill { index },
                Section::Skills,
            ));
        }

        for (index, education) in profile.education.iter().enumerate() {
            let mut parts = Vec::new();
            if let Some(degree) = &education.degree {
                parts.push(degree.clone());
            }
            if let Some(field) = &education.field {
                parts.push(format!("in {field}"));
            }
            parts.push(format!("at {}", education.institution));
            facts.push(make_fact(
                facts.len() + 1,
                parts.join(" "),
                FactSource::Education { index },
                Section::Education,
            ));
        }

        for (index, certification) in profile.certifications.iter().enumerate() {
            let mut text = certification.name.clone();
            if let Some(issuer) = &certification.issuer {
                text.push_str(&format!(" ({issuer})"));
            }
            facts.push(make_fact(
                facts.len() + 1,
                text,
                FactSource::Certification { index },
                Section::Certifications,
            ));
        }

        Self::new(facts)
    }

    pub fn new(facts: Vec<Fact>) -> Self {
        let index = facts
            .iter()
            .enumerate()
            .map(|(i, fact)| (fact.id.clone(), i))
            .collect();
        Self { facts, index }
    }

    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    pub fn get(&self, id: &FactId) -> Option<&Fact> {
        self.index.get(id).map(|&i| &self.facts[i])
    }

    pub fn contains(&self, id: &FactId) -> bool {
        self.index.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Fact> {
        self.facts.iter()
    }
}

fn make_fact(id: usize, text: String, source: FactSource, section: Section) -> Fact {
    Fact {
        id: FactId::generate(id),
        tags: tokenize(&text),
        text,
        source,
        section,
    }
}

/// Lowercased alphanumeric tokens, used for lexical matching.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}
