//! PDF rendering via an embedded Typst engine.
//!
//! Markdown remains the canonical representation; this module translates the
//! same model into Typst markup and compiles it to PDF. It is only built with
//! the `pdf` feature so the default build stays light.

use aegoris_core::domain::artifact::ArtifactKind;
use aegoris_core::domain::{Claim, Section};
use typst_as_lib::typst_kit_options::TypstKitFontOptions;
use typst_as_lib::TypstEngine;
use typst_layout::PagedDocument;

use crate::{RenderContext, RenderError, RenderResult};

pub(crate) fn render_pdf(context: &RenderContext<'_>, kind: ArtifactKind) -> RenderResult<Vec<u8>> {
    let source = match kind {
        ArtifactKind::Resume => resume_source(context),
        ArtifactKind::CoverLetter => cover_letter_source(context),
    };
    compile(&source)
}

fn compile(source: &str) -> RenderResult<Vec<u8>> {
    let engine = TypstEngine::builder()
        .main_file(source)
        .search_fonts_with(TypstKitFontOptions::default())
        .build();

    let warned = engine.compile::<PagedDocument>();
    let document = warned
        .output
        .map_err(|error| RenderError::Failed(format!("typst compilation failed: {error}")))?;

    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|errors| RenderError::Failed(format!("pdf generation failed: {errors:?}")))
}

fn resume_source(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = String::new();

    out.push_str("#set page(paper: \"a4\", margin: (x: 1.6cm, y: 1.4cm))\n");
    out.push_str("#set text(size: 10pt)\n");
    out.push_str("#set par(leading: 0.6em)\n\n");

    out.push_str("#align(center)[\n");
    out.push_str(&format!(
        "  #text(size: 19pt, weight: \"bold\")[{}] \\\n",
        esc(&profile.name)
    ));
    if let Some(headline) = &profile.headline {
        out.push_str(&format!("  {} \\\n", esc(headline)));
    }
    if let Some(contact) = contact_line(context, " · ") {
        out.push_str(&format!("  #text(size: 9pt)[{}]\n", esc(&contact)));
    }
    out.push_str("]\n\n");

    write_section_heading(&mut out, Section::Summary);
    for claim in claims_in(context, Section::Summary) {
        out.push_str(&format!("{}\n\n", esc(claim.text())));
    }

    if !context.plan.role_order.is_empty() {
        write_section_heading(&mut out, Section::Experience);
        for role_id in &context.plan.role_order {
            let Some(role) = profile.roles.iter().find(|r| &r.id == role_id) else {
                continue;
            };
            let dates = date_range(role.start.as_deref(), role.end.as_deref(), role.current);
            out.push_str(&format!("*{}*", esc(&role.title)));
            if !role.organization.is_empty() {
                out.push_str(&format!(" — {}", esc(&role.organization)));
            }
            if !dates.is_empty() {
                out.push_str(&format!(" \\\n_{}_\n", esc(&dates)));
            } else {
                out.push_str(" \\\n");
            }
            for claim in context
                .claims
                .iter()
                .filter(|c| c.section() == Section::Experience && c.role_id() == Some(role_id))
                .take(context.plan.max_bullets_per_role)
            {
                out.push_str(&format!("- {}\n", esc(claim.text().trim_end_matches('.'))));
            }
            out.push('\n');
        }
    }

    write_facts_list(&mut out, context, Section::Projects, "Projects");
    write_facts_inline(&mut out, context, Section::Skills, "Skills");
    write_facts_list(&mut out, context, Section::Education, "Education");
    write_facts_list(&mut out, context, Section::Certifications, "Certifications");

    out
}

fn cover_letter_source(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = String::new();

    out.push_str("#set page(paper: \"a4\", margin: (x: 2.2cm, y: 2cm))\n");
    out.push_str("#set text(size: 11pt)\n");
    out.push_str("#set par(leading: 0.7em)\n\n");

    out.push_str(&format!(
        "#text(size: 15pt, weight: \"bold\")[{}]\n\n",
        esc(&profile.name)
    ));
    if let Some(contact) = contact_line(context, " · ") {
        out.push_str(&format!("#text(size: 9pt)[{}]\n\n", esc(&contact)));
    }

    let target = match (&context.jd.title, &context.jd.organization) {
        (Some(title), Some(org)) => format!(" for the {title} role at {org}"),
        (Some(title), None) => format!(" for the {title} role"),
        (None, Some(org)) => format!(" at {org}"),
        (None, None) => String::new(),
    };

    out.push_str("Dear Hiring Manager,\n\n");
    out.push_str(&format!(
        "I am writing to apply{}. My background includes the following, most relevant to your needs:\n\n",
        esc(&target)
    ));
    for claim in context.claims {
        out.push_str(&format!("{}\n\n", esc(claim.text())));
    }
    out.push_str(
        "I would welcome the opportunity to discuss how my experience fits your team.\n\n",
    );
    out.push_str(&format!("Sincerely, \\\n{}\n", esc(&profile.name)));

    out
}

fn write_section_heading(out: &mut String, section: Section) {
    out.push_str(&format!("= {}\n", section.title()));
}

fn write_facts_list(out: &mut String, context: &RenderContext<'_>, section: Section, title: &str) {
    let facts = facts_in(context, section);
    if facts.is_empty() {
        return;
    }
    out.push_str(&format!("= {title}\n"));
    for fact in facts {
        out.push_str(&format!("- {}\n", esc(fact.trim_end_matches('.'))));
    }
    out.push('\n');
}

fn write_facts_inline(
    out: &mut String,
    context: &RenderContext<'_>,
    section: Section,
    title: &str,
) {
    let facts = facts_in(context, section);
    if facts.is_empty() {
        return;
    }
    out.push_str(&format!("= {title}\n"));
    out.push_str(&esc(&facts.join(", ")));
    out.push_str("\n\n");
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

fn contact_line(context: &RenderContext<'_>, separator: &str) -> Option<String> {
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
    (!parts.is_empty()).then(|| parts.join(separator))
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

/// Escape Typst markup control characters in literal text.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '#' | '*' | '_' | '`' | '$' | '@' | '<' | '>' | '[' | ']' | '~' | '='
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_typst_control_characters() {
        assert_eq!(esc("R&D #1"), "R&D \\#1");
        assert_eq!(esc("a_b*c"), "a\\_b\\*c");
        assert_eq!(esc("[x]"), "\\[x\\]");
    }
}
