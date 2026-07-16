use serde_json::Value;

use crate::domain::ids::RoleId;
use crate::domain::profile::{Education, Profile, Role, Skill};
use crate::error::{CoreError, CoreResult};

/// Parse a public LinkedIn profile page into a `Profile`.
///
/// LinkedIn serves public profiles with a `schema.org/Person` JSON-LD block
/// and OpenGraph meta tags. This parser reads those structured fragments; it
/// does not attempt to execute the page's JavaScript. Fetching the HTML is I/O
/// and lives in the CLI, so this stays pure and testable.
///
/// The structured data is intentionally limited (name, headline, summary,
/// location, current company, education, skills). Full role history is only
/// available from the official data export.
pub fn parse_linkedin_html(html: &str) -> CoreResult<Profile> {
    if let Some(person) = extract_person(html) {
        let profile = profile_from_person(&person);
        if !profile.name.is_empty() {
            return Ok(profile);
        }
    }
    profile_from_meta(html)
}

fn extract_person(html: &str) -> Option<Value> {
    for block in ld_json_blocks(html) {
        let Ok(value) = serde_json::from_str::<Value>(block) else {
            continue;
        };
        if let Some(person) = find_person(&value) {
            return Some(person.clone());
        }
    }
    None
}

fn find_person(value: &Value) -> Option<&Value> {
    match value {
        Value::Array(items) => items.iter().find_map(find_person),
        Value::Object(map) => {
            let is_person = map
                .get("@type")
                .and_then(Value::as_str)
                .map(|kind| kind.eq_ignore_ascii_case("Person"))
                .unwrap_or(false);
            if is_person {
                return Some(value);
            }
            // Some pages nest the Person under @graph.
            map.get("@graph").and_then(find_person)
        }
        _ => None,
    }
}

/// Extract the bodies of `<script type="application/ld+json">` blocks.
fn ld_json_blocks(html: &str) -> Vec<&str> {
    const MARKER: &str = "application/ld+json";
    let mut blocks = Vec::new();
    let mut cursor = 0;
    while let Some(found) = html[cursor..].find(MARKER) {
        let marker_end = cursor + found + MARKER.len();
        // Find the end of the opening tag, then the matching closing tag.
        let Some(open_end) = html[marker_end..].find('>') else {
            break;
        };
        let content_start = marker_end + open_end + 1;
        let Some(close) = html[content_start..].find("</script>") else {
            break;
        };
        blocks.push(html[content_start..content_start + close].trim());
        cursor = content_start + close + "</script>".len();
    }
    blocks
}

fn profile_from_person(person: &Value) -> Profile {
    let name = string_at(person, &["name"]).unwrap_or_default();
    let headline = string_at(person, &["jobTitle"]);
    let summary = string_at(person, &["description"]);
    let location = location_of(person);

    let mut profile = Profile {
        name,
        headline,
        summary,
        location,
        ..Default::default()
    };

    let employers = names_at(person, "worksFor");
    if !employers.is_empty() {
        profile.roles.push(Role {
            id: RoleId::generate(1),
            title: string_at(person, &["jobTitle"]).unwrap_or_default(),
            organization: employers.join(", "),
            location: None,
            start: None,
            end: None,
            current: true,
            bullets: Vec::new(),
            skills: Vec::new(),
        });
    }

    for institution in names_at(person, "alumniOf") {
        profile.education.push(Education {
            degree: None,
            institution,
            field: None,
            start: None,
            end: None,
        });
    }

    for skill in names_at(person, "knowsAbout") {
        profile.skills.push(Skill {
            name: skill,
            level: None,
        });
    }

    profile
}

fn profile_from_meta(html: &str) -> CoreResult<Profile> {
    let title = meta_content(html, "og:title")
        .or_else(|| meta_content(html, "twitter:title"))
        .unwrap_or_default();
    let description =
        meta_content(html, "og:description").or_else(|| meta_content(html, "description"));

    let cleaned = title
        .split(['|', '–', '—'])
        .next()
        .unwrap_or(&title)
        .trim()
        .trim_end_matches(" on LinkedIn")
        .trim()
        .to_string();

    let (name, headline) = match cleaned.split_once(" - ") {
        Some((name, headline)) => (name.trim().to_string(), Some(headline.trim().to_string())),
        None => (cleaned, None),
    };

    if name.is_empty() {
        return Err(CoreError::parse(
            "could not find structured profile data in the page (LinkedIn may have served a login wall)",
        ));
    }

    Ok(Profile {
        name,
        headline,
        summary: description,
        ..Default::default()
    })
}

fn location_of(person: &Value) -> Option<String> {
    let address = person.get("address")?;
    if let Some(text) = address.as_str() {
        let text = text.trim();
        return (!text.is_empty()).then(|| text.to_string());
    }
    let parts: Vec<String> = ["addressLocality", "addressRegion", "addressCountry"]
        .iter()
        .filter_map(|key| address.get(*key).and_then(Value::as_str))
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

fn string_at(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn names_at(value: &Value, key: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(entry) = value.get(key) {
        collect_names(entry, &mut names);
    }
    names
}

fn collect_names(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            let text = text.trim();
            if !text.is_empty() {
                out.push(text.to_string());
            }
        }
        Value::Object(map) => {
            if let Some(name) = map.get("name").and_then(Value::as_str) {
                let name = name.trim();
                if !name.is_empty() {
                    out.push(name.to_string());
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_names(item, out);
            }
        }
        _ => {}
    }
}

/// Read the `content` attribute of a `<meta name/property="...">` tag.
fn meta_content(html: &str, property: &str) -> Option<String> {
    let needle = format!("=\"{property}\"");
    let mut cursor = 0;
    while let Some(found) = html[cursor..].find(&needle) {
        let tag_start = html[..cursor + found].rfind('<')?;
        let tag_end = html[cursor + found..].find('>')? + cursor + found;
        let tag = &html[tag_start..=tag_end];
        if let Some(content) = attribute(tag, "content") {
            if !content.is_empty() {
                return Some(content);
            }
        }
        cursor = tag_end;
    }
    None
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let start = tag.find(&needle)? + needle.len();
    let rest = tag[start..].trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERSON_HTML: &str = r#"
<!DOCTYPE html><html><head>
<title>Ada Lovelace - Backend Engineer | LinkedIn</title>
<meta property="og:title" content="Ada Lovelace - Backend Engineer | LinkedIn" />
<meta property="og:description" content="Fallback description" />
<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "Person",
  "name": "Ada Lovelace",
  "jobTitle": "Senior Backend Engineer",
  "description": "Pioneering programmer.",
  "address": {"@type": "PostalAddress", "addressLocality": "London", "addressCountry": "GB"},
  "worksFor": {"@type": "Organization", "name": "Analytical Engines Ltd"},
  "alumniOf": [{"@type": "CollegeOrUniversity", "name": "University of London"}],
  "knowsAbout": ["Rust", "PostgreSQL", "Kubernetes"]
}
</script>
</head><body></body></html>
"#;

    const META_ONLY_HTML: &str = r#"
<!DOCTYPE html><html><head>
<meta property="og:title" content="Grace Hopper - Rear Admiral | LinkedIn" />
<meta property="og:description" content="Computer scientist and United States Navy rear admiral." />
</head><body></body></html>
"#;

    #[test]
    fn parses_person_json_ld() {
        let profile = parse_linkedin_html(PERSON_HTML).unwrap();
        assert_eq!(profile.name, "Ada Lovelace");
        assert_eq!(profile.headline.as_deref(), Some("Senior Backend Engineer"));
        assert_eq!(profile.summary.as_deref(), Some("Pioneering programmer."));
        assert_eq!(profile.location.as_deref(), Some("London, GB"));
        assert_eq!(profile.roles.len(), 1);
        assert_eq!(profile.roles[0].organization, "Analytical Engines Ltd");
        assert_eq!(profile.skills.len(), 3);
        assert_eq!(profile.education[0].institution, "University of London");
    }

    #[test]
    fn falls_back_to_open_graph_tags() {
        let profile = parse_linkedin_html(META_ONLY_HTML).unwrap();
        assert_eq!(profile.name, "Grace Hopper");
        assert_eq!(profile.headline.as_deref(), Some("Rear Admiral"));
        assert!(profile
            .summary
            .as_deref()
            .unwrap()
            .contains("Computer scientist"));
    }

    #[test]
    fn finds_person_nested_in_graph() {
        let html = r#"<script type="application/ld+json">
            {"@context":"https://schema.org","@graph":[{"@type":"WebPage"},{"@type":"Person","name":"Alan Turing"}]}
        </script>"#;
        let profile = parse_linkedin_html(html).unwrap();
        assert_eq!(profile.name, "Alan Turing");
    }

    #[test]
    fn errors_when_no_structured_data_is_present() {
        let error = parse_linkedin_html("<html><body>Sign in</body></html>").unwrap_err();
        assert!(matches!(error, CoreError::Parse(_)));
    }
}
