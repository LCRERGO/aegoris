use crate::domain::ids::RequirementId;
use crate::domain::job::{JobDescription, Requirement, RequirementKind};

const REQUIREMENT_SECTIONS: &[&str] = &[
    "requirement",
    "qualification",
    "skill",
    "experience",
    "must have",
    "you have",
    "you'll have",
    "you will have",
    "looking for",
    "what you bring",
    "about you",
    "responsibilit",
    "what you'll do",
    "what you will do",
    "the role",
];

/// Extract a job description from free text using deterministic rules.
///
/// This is the non-LLM requirement extractor: it recognizes headings and
/// bullet points, keeps lines from requirement-shaped sections, and classifies
/// each line by keyword. It never calls the network.
pub fn parse_job_description(input: &str) -> JobDescription {
    let mut jd = JobDescription {
        raw: input.to_string(),
        ..Default::default()
    };

    let mut in_requirement_section = true;
    let mut seen: Vec<String> = Vec::new();
    let mut requirements: Vec<Requirement> = Vec::new();
    let mut seen_content = false;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // The first non-heading line is the posting title.
        if !seen_content {
            seen_content = true;
            if heading_text(line).is_none() {
                jd.title = Some(line.to_string());
                continue;
            }
        }

        if let Some(title) = heading_text(line) {
            if jd.title.is_none() && line.starts_with('#') {
                jd.title = Some(title.to_string());
            }
            in_requirement_section = is_requirement_section(title);
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_lowercase();
            if matches!(key.as_str(), "title" | "role" | "position") && jd.title.is_none() {
                jd.title = Some(value.trim().to_string());
            }
            if matches!(key.as_str(), "company" | "organization" | "employer")
                && jd.organization.is_none()
            {
                jd.organization = Some(value.trim().to_string());
            }
        }

        let candidate = match strip_bullet(line) {
            Some(bullet) => Some(bullet),
            None if in_requirement_section => Some(line),
            None => None,
        };

        let Some(text) = candidate else { continue };
        let text = clean(text);
        if text.is_empty() || text.len() > 400 {
            continue;
        }
        let key = text.to_lowercase();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);

        let kind = classify(&text, in_requirement_section);
        let id = RequirementId::generate(requirements.len() + 1);
        requirements.push(Requirement::new(id, text, kind));
    }

    jd.requirements = requirements;
    jd
}

fn heading_text(line: &str) -> Option<&str> {
    if let Some(rest) = line.strip_prefix('#') {
        let title = rest.trim_start_matches('#').trim();
        if !title.is_empty() {
            return Some(title);
        }
    }
    let is_caps = line.len() > 2
        && line.len() < 80
        && line.chars().any(|c| c.is_alphabetic())
        && line
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(|c| c.is_uppercase());
    if is_caps {
        return Some(line);
    }
    if let Some(stripped) = line.strip_suffix(':') {
        let title = stripped.trim();
        if !title.is_empty() && title.split_whitespace().count() <= 6 {
            return Some(title);
        }
    }
    None
}

fn is_requirement_section(heading: &str) -> bool {
    let heading = heading.to_lowercase();
    REQUIREMENT_SECTIONS
        .iter()
        .any(|needle| heading.contains(needle))
}

fn strip_bullet(line: &str) -> Option<&str> {
    for prefix in ["- ", "* ", "• ", "· "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest.trim());
        }
    }
    split_ordered_bullet(line)
}

fn split_ordered_bullet(line: &str) -> Option<&str> {
    let digits_len = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits_len == 0 {
        return None;
    }
    let rest = &line[digits_len..];
    for sep in [". ", ") "] {
        if let Some(rest) = rest.strip_prefix(sep) {
            return Some(rest.trim());
        }
    }
    None
}

fn clean(text: &str) -> String {
    text.trim().trim_end_matches('.').trim().to_string()
}

fn classify(text: &str, in_requirement_section: bool) -> RequirementKind {
    let lower = text.to_lowercase();
    if lower.contains("degree")
        || lower.contains("bachelor")
        || lower.contains("master")
        || lower.contains("phd")
        || lower.contains("b.s.")
        || lower.contains("m.s.")
    {
        return RequirementKind::Education;
    }
    if lower.contains("certif") {
        return RequirementKind::Certification;
    }
    if lower.contains("year")
        || lower.contains("experience")
        || lower.contains("proven track record")
    {
        return RequirementKind::Experience;
    }
    if lower.starts_with("responsib")
        || (!in_requirement_section && lower.split_whitespace().count() > 6)
    {
        return RequirementKind::Responsibility;
    }
    RequirementKind::Skill
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Senior Rust Engineer

Requirements:
- 5+ years of experience building backend services
- Strong proficiency in Rust and async programming
- Experience with PostgreSQL
- Bachelor's degree in Computer Science or equivalent

Nice to have:
- Kubernetes
- Prior startup experience
";

    #[test]
    fn extracts_requirements_and_classifies_them() {
        let jd = parse_job_description(SAMPLE);
        assert_eq!(jd.title.as_deref(), Some("Senior Rust Engineer"));
        let texts: Vec<&str> = jd.requirements.iter().map(|r| r.text.as_str()).collect();
        assert!(!texts.contains(&"Senior Rust Engineer"));
        assert!(texts.contains(&"5+ years of experience building backend services"));
        assert!(texts.contains(&"Strong proficiency in Rust and async programming"));
        assert!(texts.contains(&"Experience with PostgreSQL"));
        assert!(texts.contains(&"Bachelor's degree in Computer Science or equivalent"));
        assert!(texts.contains(&"Kubernetes"));

        let experience = jd
            .requirements
            .iter()
            .find(|r| r.text.contains("5+ years"))
            .unwrap();
        assert_eq!(experience.kind, RequirementKind::Experience);

        let education = jd
            .requirements
            .iter()
            .find(|r| r.text.contains("Bachelor"))
            .unwrap();
        assert_eq!(education.kind, RequirementKind::Education);
    }

    #[test]
    fn ids_are_sequential_and_stable() {
        let jd = parse_job_description(SAMPLE);
        for (i, req) in jd.requirements.iter().enumerate() {
            assert_eq!(req.id.as_str(), format!("rq_{}", i + 1));
        }
    }

    #[test]
    fn deduplicates_repeated_lines() {
        let jd = parse_job_description("Skills:\n- Rust\n- Rust\n- Go");
        assert_eq!(jd.requirements.len(), 2);
    }
}
