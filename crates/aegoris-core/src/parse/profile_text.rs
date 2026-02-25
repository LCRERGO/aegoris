use crate::domain::ids::RoleId;
use crate::domain::profile::{Certification, Contact, Education, Profile, Project, Role, Skill};
use crate::error::{CoreError, CoreResult};

/// Parse the lightweight plain-text profile format.
///
/// The format is deliberately simple and forgiving:
///
/// ```text
/// # Ada Lovelace
/// Mathematician
/// email: ada@example.com
/// github: adalovelace
///
/// ## Summary
/// A sentence or two.
///
/// ## Experience
/// ### Analytical Engine @ Babbage & Co. (1842 - 1843)
/// - Wrote the first algorithm
/// - Foresaw general-purpose computing
///
/// ## Skills
/// Mathematics, Algorithms
///
/// ## Education
/// ### University of London
/// Degree: Mathematics
/// ```
pub fn parse_profile_text(input: &str) -> CoreResult<Profile> {
    let mut profile = Profile::default();
    let mut section = String::new();
    let mut current_role: Option<Role> = None;
    let mut current_education: Option<Education> = None;
    let mut saw_name = false;

    for raw_line in input.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if let Some(name) = trimmed.strip_prefix("# ") {
            profile.name = name.trim().to_string();
            saw_name = true;
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("## ") {
            flush_role(&mut profile, &mut current_role);
            flush_education(&mut profile, &mut current_education);
            section = heading.trim().to_lowercase();
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("### ") {
            let heading = heading.trim();
            match section.as_str() {
                "experience" => {
                    flush_role(&mut profile, &mut current_role);
                    current_role = Some(parse_role_heading(heading, profile.roles.len()));
                }
                "education" => {
                    flush_education(&mut profile, &mut current_education);
                    current_education = Some(Education {
                        degree: None,
                        institution: heading.to_string(),
                        field: None,
                        start: None,
                        end: None,
                    });
                }
                "projects" => {
                    profile.projects.push(Project {
                        name: heading.to_string(),
                        description: None,
                        bullets: Vec::new(),
                        skills: Vec::new(),
                        url: None,
                    });
                }
                _ => {}
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        match section.as_str() {
            "" => {
                if !saw_name {
                    profile.name = trimmed.to_string();
                    saw_name = true;
                } else if let Some((key, value)) = split_key_value(trimmed) {
                    apply_contact(&mut profile.contact, key, value);
                } else if profile.headline.is_none() {
                    profile.headline = Some(trimmed.to_string());
                }
            }
            "summary" => match &mut profile.summary {
                Some(existing) => {
                    existing.push(' ');
                    existing.push_str(trimmed);
                }
                None => profile.summary = Some(trimmed.to_string()),
            },
            "experience" => {
                if let Some(role) = current_role.as_mut() {
                    if let Some(bullet) = strip_bullet(trimmed) {
                        role.bullets.push(bullet);
                    }
                }
            }
            "projects" => {
                if let Some(project) = profile.projects.last_mut() {
                    if let Some(bullet) = strip_bullet(trimmed) {
                        project.bullets.push(bullet);
                    } else if project.description.is_none() {
                        project.description = Some(trimmed.to_string());
                    }
                }
            }
            "skills" => {
                for name in trimmed.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    profile.skills.push(Skill {
                        name: name.to_string(),
                        level: None,
                    });
                }
            }
            "education" => {
                if let Some(education) = current_education.as_mut() {
                    if let Some((key, value)) = split_key_value(trimmed) {
                        match key.to_lowercase().as_str() {
                            "degree" => education.degree = Some(value.to_string()),
                            "field" => education.field = Some(value.to_string()),
                            "start" => education.start = Some(value.to_string()),
                            "end" => education.end = Some(value.to_string()),
                            _ => {}
                        }
                    }
                }
            }
            "certifications" => {
                let (name, issuer) = match trimmed.split_once(" (") {
                    Some((name, rest)) => (
                        name.trim().to_string(),
                        rest.trim_end_matches(')').to_string(),
                    ),
                    None => (trimmed.to_string(), String::new()),
                };
                profile.certifications.push(Certification {
                    name,
                    issuer: (!issuer.is_empty()).then_some(issuer),
                    year: None,
                });
            }
            _ => {}
        }
    }

    flush_role(&mut profile, &mut current_role);
    flush_education(&mut profile, &mut current_education);

    if profile.name.trim().is_empty() {
        return Err(CoreError::parse(
            "profile text must begin with a `# Name` line",
        ));
    }
    Ok(profile)
}

fn flush_role(profile: &mut Profile, current: &mut Option<Role>) {
    if let Some(role) = current.take() {
        profile.roles.push(role);
    }
}

fn flush_education(profile: &mut Profile, current: &mut Option<Education>) {
    if let Some(education) = current.take() {
        profile.education.push(education);
    }
}

fn parse_role_heading(heading: &str, position: usize) -> Role {
    let (main, dates) = match heading.rsplit_once(" (") {
        Some((main, rest)) if rest.ends_with(')') => {
            (main.trim(), Some(rest.trim_end_matches(')')))
        }
        _ => (heading.trim(), None),
    };

    let (title, organization) = match main.split_once(" @ ") {
        Some((title, organization)) => (title.trim(), organization.trim()),
        None => (main, ""),
    };

    let (start, end) = match dates {
        Some(range) => match range.split_once(" - ") {
            Some((start, end)) => (Some(start.trim().to_string()), Some(end.trim().to_string())),
            None => (Some(range.trim().to_string()), None),
        },
        None => (None, None),
    };

    Role {
        id: RoleId::generate(position + 1),
        title: title.to_string(),
        organization: organization.to_string(),
        location: None,
        start,
        end,
        current: false,
        bullets: Vec::new(),
        skills: Vec::new(),
    }
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    Some((key.trim(), value.trim()))
}

fn apply_contact(contact: &mut Contact, key: &str, value: &str) {
    match key.to_lowercase().as_str() {
        "email" => contact.email = Some(value.to_string()),
        "phone" => contact.phone = Some(value.to_string()),
        "linkedin" => contact.linkedin = Some(value.to_string()),
        "github" => contact.github = Some(value.to_string()),
        "website" | "site" => contact.website = Some(value.to_string()),
        _ => {}
    }
}

fn strip_bullet(line: &str) -> Option<String> {
    for prefix in ["- ", "* ", "• "] {
        if let Some(rest) = line.strip_prefix(prefix) {
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Ada Lovelace
Mathematician
email: ada@example.com

## Summary
First programmer.

## Experience
### Analytical Engine @ Babbage & Co. (1842 - 1843)
- Wrote the first algorithm
- Foresaw general-purpose computing

## Skills
Mathematics, Algorithms

## Education
### University of London
Degree: Mathematics
";

    #[test]
    fn parses_the_documented_format() {
        let profile = parse_profile_text(SAMPLE).unwrap();
        assert_eq!(profile.name, "Ada Lovelace");
        assert_eq!(profile.headline.as_deref(), Some("Mathematician"));
        assert_eq!(profile.contact.email.as_deref(), Some("ada@example.com"));
        assert_eq!(profile.summary.as_deref(), Some("First programmer."));
        assert_eq!(profile.roles.len(), 1);
        let role = &profile.roles[0];
        assert_eq!(role.title, "Analytical Engine");
        assert_eq!(role.organization, "Babbage & Co.");
        assert_eq!(role.start.as_deref(), Some("1842"));
        assert_eq!(role.end.as_deref(), Some("1843"));
        assert_eq!(role.bullets.len(), 2);
        assert_eq!(profile.skills.len(), 2);
        assert_eq!(profile.education[0].institution, "University of London");
        assert_eq!(profile.education[0].degree.as_deref(), Some("Mathematics"));
    }

    #[test]
    fn requires_a_name() {
        let err = parse_profile_text("## Summary\nhi").unwrap_err();
        assert!(matches!(err, CoreError::Parse(_)));
    }
}
