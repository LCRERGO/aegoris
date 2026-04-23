use aegoris_core::domain::Section;

use crate::RenderContext;

pub(crate) fn resume(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = String::new();

    out.push_str(&format!("{}\n", profile.name));
    if let Some(headline) = &profile.headline {
        out.push_str(&format!("{headline}\n"));
    }
    if let Some(contact) = contact_line(context) {
        out.push_str(&format!("{contact}\n"));
    }
    out.push('\n');

    write_facts_section(&mut out, "SUMMARY", context, Section::Summary, false);
    write_experience(&mut out, context);
    write_facts_section(&mut out, "PROJECTS", context, Section::Projects, true);
    write_facts_section(&mut out, "SKILLS", context, Section::Skills, false);
    write_facts_section(&mut out, "EDUCATION", context, Section::Education, true);
    write_facts_section(
        &mut out,
        "CERTIFICATIONS",
        context,
        Section::Certifications,
        true,
    );

    out.trim_end().to_string() + "\n"
}

pub(crate) fn cover_letter(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = String::new();
    out.push_str(&format!("{}\n", profile.name));
    if let Some(contact) = contact_line(context) {
        out.push_str(&format!("{contact}\n"));
    }
    out.push_str("\nDear Hiring Manager,\n\n");
    for claim in context.claims {
        out.push_str(&format!("{}\n\n", claim.text().trim()));
    }
    out.push_str(&format!("Sincerely,\n{}\n", profile.name));
    out
}

fn write_experience(out: &mut String, context: &RenderContext<'_>) {
    if context.plan.role_order.is_empty() {
        return;
    }
    out.push_str("EXPERIENCE\n");
    for role_id in &context.plan.role_order {
        let Some(role) = context.profile.roles.iter().find(|r| &r.id == role_id) else {
            continue;
        };
        let dates = date_range(role.start.as_deref(), role.end.as_deref(), role.current);
        out.push_str(&format!(
            "{} — {}{}\n",
            role.title,
            role.organization,
            if dates.is_empty() {
                String::new()
            } else {
                format!(" ({dates})")
            }
        ));
        for claim in context
            .claims
            .iter()
            .filter(|c| c.section() == Section::Experience && c.role_id() == Some(role_id))
            .take(context.plan.max_bullets_per_role)
        {
            out.push_str(&format!(
                "- {}\n",
                claim.text().trim().trim_end_matches('.')
            ));
        }
        out.push('\n');
    }
}

fn write_facts_section(
    out: &mut String,
    title: &str,
    context: &RenderContext<'_>,
    section: Section,
    bullets: bool,
) {
    let facts: Vec<String> = context
        .plan
        .selected_facts(context.store)
        .into_iter()
        .filter(|fact| fact.section == section)
        .map(|fact| fact.text.clone())
        .collect();
    if facts.is_empty() {
        return;
    }
    out.push_str(title);
    out.push('\n');
    if bullets {
        for fact in facts {
            out.push_str(&format!("- {}\n", fact.trim().trim_end_matches('.')));
        }
    } else {
        out.push_str(&facts.join(", "));
        out.push('\n');
    }
    out.push('\n');
}

fn contact_line(context: &RenderContext<'_>) -> Option<String> {
    let contact = &context.profile.contact;
    let mut parts = Vec::new();
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
    if let Some(location) = &context.profile.location {
        if !location.trim().is_empty() {
            parts.push(location.trim().to_string());
        }
    }
    (!parts.is_empty()).then(|| parts.join(" | "))
}

fn date_range(start: Option<&str>, end: Option<&str>, current: bool) -> String {
    match (start, end, current) {
        (Some(start), _, true) => format!("{start} - Present"),
        (Some(start), Some(end), _) => format!("{start} - {end}"),
        (Some(start), None, _) => start.to_string(),
        (None, Some(end), _) => end.to_string(),
        _ => String::new(),
    }
}
