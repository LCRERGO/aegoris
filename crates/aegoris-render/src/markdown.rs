use aegoris_core::domain::profile::Profile;
use aegoris_core::domain::{Claim, Section};

use crate::RenderContext;

pub(crate) fn resume(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", profile.name));
    if let Some(headline) = &profile.headline {
        out.push_str(&format!("{headline}\n\n"));
    }
    if let Some(contact) = contact_line(profile) {
        out.push_str(&format!("{contact}\n\n"));
    }

    let summary: Vec<&Claim> = claims_in(context, Section::Summary);
    if !summary.is_empty() {
        out.push_str("## Summary\n\n");
        for claim in summary {
            out.push_str(&format!("{}\n", sentence(claim.text())));
        }
        out.push('\n');
    }

    if !context.plan.role_order.is_empty() {
        out.push_str("## Experience\n\n");
        for role_id in &context.plan.role_order {
            let Some(role) = profile.roles.iter().find(|r| &r.id == role_id) else {
                continue;
            };
            let heading = if role.organization.is_empty() {
                role.title.clone()
            } else {
                format!("{} — {}", role.title, role.organization)
            };
            let dates = date_range(role.start.as_deref(), role.end.as_deref(), role.current);
            if dates.is_empty() {
                out.push_str(&format!("### {heading}\n\n"));
            } else {
                out.push_str(&format!("### {heading} ({dates})\n\n"));
            }
            let role_claims: Vec<&Claim> = context
                .claims
                .iter()
                .filter(|claim| {
                    claim.section() == Section::Experience && claim.role_id() == Some(role_id)
                })
                .take(context.plan.max_bullets_per_role)
                .collect();
            for claim in role_claims {
                out.push_str(&format!("- {}\n", bullet(claim.text())));
            }
            out.push('\n');
        }
    }

    let projects = facts_in(context, Section::Projects);
    if !projects.is_empty() {
        out.push_str("## Projects\n\n");
        for text in projects {
            out.push_str(&format!("- {}\n", bullet(&text)));
        }
        out.push('\n');
    }

    let skills = facts_in(context, Section::Skills);
    if !skills.is_empty() {
        out.push_str("## Skills\n\n");
        out.push_str(&skills.join(", "));
        out.push_str("\n\n");
    }

    let education = facts_in(context, Section::Education);
    if !education.is_empty() {
        out.push_str("## Education\n\n");
        for text in education {
            out.push_str(&format!("- {}\n", bullet(&text)));
        }
        out.push('\n');
    }

    let certifications = facts_in(context, Section::Certifications);
    if !certifications.is_empty() {
        out.push_str("## Certifications\n\n");
        for text in certifications {
            out.push_str(&format!("- {}\n", bullet(&text)));
        }
        out.push('\n');
    }

    out.trim_end().to_string() + "\n"
}

pub(crate) fn cover_letter(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", profile.name));
    if let Some(contact) = contact_line(profile) {
        out.push_str(&format!("{contact}\n\n"));
    }

    let target = match (&context.jd.title, &context.jd.organization) {
        (Some(title), Some(org)) => format!(" for the {title} role at {org}"),
        (Some(title), None) => format!(" for the {title} role"),
        (None, Some(org)) => format!(" at {org}"),
        (None, None) => String::new(),
    };

    out.push_str("Dear Hiring Manager,\n\n");
    out.push_str(&format!(
        "I am writing to apply{target}. My background includes the following, most relevant to your needs:\n\n"
    ));

    for claim in context.claims {
        out.push_str(&format!("{}\n\n", sentence(claim.text())));
    }

    out.push_str(&format!(
        "I would welcome the opportunity to discuss how my experience fits your team.\n\nSincerely,\n{}\n",
        profile.name
    ));

    out
}

fn claims_in<'a>(context: &'a RenderContext<'_>, section: Section) -> Vec<&'a Claim> {
    context
        .claims
        .iter()
        .filter(|claim| claim.section() == section)
        .collect()
}

fn facts_in(context: &RenderContext<'_>, section: Section) -> Vec<String> {
    context
        .plan
        .selected_facts(context.store)
        .into_iter()
        .filter(|fact| fact.section == section)
        .map(|fact| fact.text.clone())
        .collect()
}

fn contact_line(profile: &Profile) -> Option<String> {
    let contact = &profile.contact;
    let mut parts: Vec<String> = Vec::new();
    for value in [
        contact.email.as_deref(),
        contact.phone.as_deref(),
        contact.linkedin.as_deref(),
        contact.github.as_deref(),
        contact.website.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !value.trim().is_empty() {
            parts.push(value.trim().to_string());
        }
    }
    if let Some(location) = &profile.location {
        if !location.trim().is_empty() {
            parts.push(location.trim().to_string());
        }
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn date_range(start: Option<&str>, end: Option<&str>, current: bool) -> String {
    match (start, end, current) {
        (Some(start), _, true) => format!("{start} – Present"),
        (Some(start), Some(end), _) => format!("{start} – {end}"),
        (Some(start), None, _) => start.to_string(),
        (None, Some(end), _) => end.to_string(),
        _ => String::new(),
    }
}

fn bullet(text: &str) -> String {
    text.trim().trim_end_matches('.').to_string()
}

fn sentence(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.ends_with(['.', '!', '?']) {
        trimmed.to_string()
    } else {
        format!("{trimmed}.")
    }
}
