//! LaTeX renderer.
//!
//! Emits a self-contained `.tex` document built on the core `article` class
//! plus `geometry` and `hyperref`. It deliberately does not shell out to a TeX
//! toolchain: the user compiles the source with whichever engine they prefer.
//! See `docs/adr/0011-latex-output.md`.

use aegoris_core::domain::profile::Profile;
use aegoris_core::domain::{Claim, Section};

use crate::RenderContext;

pub(crate) fn resume(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = preamble();

    out.push_str("\\begin{center}\n");
    out.push_str("{\\LARGE\\textbf{");
    out.push_str(&esc(&profile.name));
    out.push_str("}}\\par\\medskip\n");
    if let Some(headline) = &profile.headline {
        out.push_str(&format!("{}\\par\\smallskip\n", esc(headline)));
    }
    if let Some(contact) = contact_line(profile) {
        out.push_str(&format!("{contact}\\par\n"));
    }
    out.push_str("\\end{center}\n\n");

    let summary: Vec<&Claim> = claims_in(context, Section::Summary);
    if !summary.is_empty() {
        out.push_str("\\section*{Summary}\n");
        for claim in summary {
            out.push_str(&format!("{}\n\n", esc(claim.text())));
        }
    }

    if !context.plan.role_order.is_empty() {
        out.push_str("\\section*{Experience}\n");
        for role_id in &context.plan.role_order {
            let Some(role) = profile.roles.iter().find(|r| &r.id == role_id) else {
                continue;
            };
            let heading = if role.organization.is_empty() {
                esc(&role.title)
            } else {
                format!("{} --- {}", esc(&role.title), esc(&role.organization))
            };
            out.push_str(&format!("\\subsection*{{{heading}}}\n"));
            let dates = date_range(role.start.as_deref(), role.end.as_deref(), role.current);
            if !dates.is_empty() {
                out.push_str(&format!("\\textit{{{}}}\n\n", esc(&dates)));
            }
            let bullets: Vec<String> = context
                .claims
                .iter()
                .filter(|claim| {
                    claim.section() == Section::Experience && claim.role_id() == Some(role_id)
                })
                .take(context.plan.max_bullets_per_role)
                .map(|claim| esc(&bullet(claim.text())))
                .collect();
            out.push_str(&itemize(&bullets));
        }
    }

    write_facts_list(&mut out, context, Section::Projects, "Projects");
    write_facts_inline(&mut out, context, Section::Skills, "Skills");
    write_facts_list(&mut out, context, Section::Education, "Education");
    write_facts_list(&mut out, context, Section::Certifications, "Certifications");

    out.push_str("\\end{document}\n");
    out
}

pub(crate) fn cover_letter(context: &RenderContext<'_>) -> String {
    let profile = context.profile;
    let mut out = preamble();

    out.push_str("{\\LARGE\\textbf{");
    out.push_str(&esc(&profile.name));
    out.push_str("}}\n\n");
    if let Some(contact) = contact_line(profile) {
        out.push_str(&format!("{contact}\n\n"));
    }

    let target = match (&context.jd.title, &context.jd.organization) {
        (Some(title), Some(org)) => format!(" for the {} role at {}", esc(title), esc(org)),
        (Some(title), None) => format!(" for the {} role", esc(title)),
        (None, Some(org)) => format!(" at {}", esc(org)),
        (None, None) => String::new(),
    };

    out.push_str("Dear Hiring Manager,\n\n");
    out.push_str(&format!(
        "I am writing to apply{target}. My background includes the following, most relevant to your needs:\n\n"
    ));

    for claim in context.claims {
        out.push_str(&format!("{}\n\n", esc(claim.text())));
    }

    out.push_str(
        "I would welcome the opportunity to discuss how my experience fits your team.\n\n",
    );
    out.push_str(&format!("Sincerely,\\\\\n{}\n", esc(&profile.name)));

    out.push_str("\\end{document}\n");
    out
}

fn preamble() -> String {
    let mut out = String::new();
    out.push_str("\\documentclass{article}\n");
    out.push_str("\\usepackage[T1]{fontenc}\n");
    out.push_str("\\usepackage[margin=1in]{geometry}\n");
    out.push_str("\\usepackage[hidelinks]{hyperref}\n");
    out.push_str("\\begin{document}\n\n");
    out
}

fn write_facts_list(out: &mut String, context: &RenderContext<'_>, section: Section, title: &str) {
    let facts = facts_in(context, section);
    if facts.is_empty() {
        return;
    }
    out.push_str(&format!("\\section*{{{title}}}\n"));
    let bullets: Vec<String> = facts.iter().map(|fact| esc(&bullet(fact))).collect();
    out.push_str(&itemize(&bullets));
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
    out.push_str(&format!("\\section*{{{title}}}\n"));
    out.push_str(&format!("{}\n\n", esc(&facts.join(", "))));
}

fn itemize(items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut out = String::from("\\begin{itemize}\n");
    for item in items {
        out.push_str(&format!("  \\item {item}\n"));
    }
    out.push_str("\\end{itemize}\n\n");
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

/// Render the contact fields, linking the ones that look like URLs.
fn contact_line(profile: &Profile) -> Option<String> {
    let contact = &profile.contact;
    let mut parts: Vec<String> = Vec::new();

    if let Some(email) = non_empty(contact.email.as_deref()) {
        parts.push(format!("\\href{{mailto:{email}}}{{{}}}", esc(email)));
    }
    if let Some(phone) = non_empty(contact.phone.as_deref()) {
        parts.push(esc(phone));
    }
    for value in [
        contact.linkedin.as_deref(),
        contact.github.as_deref(),
        contact.website.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let Some(value) = non_empty(Some(value)) else {
            continue;
        };
        if looks_like_url(value) {
            parts.push(format!(
                "\\href{{{}}}{{{}}}",
                url_escape(&with_scheme(value)),
                esc(value)
            ));
        } else {
            parts.push(esc(value));
        }
    }
    if let Some(location) = non_empty(profile.location.as_deref()) {
        parts.push(esc(location));
    }

    (!parts.is_empty()).then(|| parts.join(" \\textperiodcentered{} "))
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn looks_like_url(value: &str) -> bool {
    let lower = value.to_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        || (value.contains('.') && !value.chars().any(char::is_whitespace))
}

fn with_scheme(value: &str) -> String {
    let lower = value.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        value.to_string()
    } else {
        format!("https://{value}")
    }
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

/// Escape literal text for LaTeX, mapping a few typographic characters.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\textbackslash{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(ch);
            }
            '–' => out.push_str("--"),
            '—' => out.push_str("---"),
            '·' => out.push_str("\\textperiodcentered{}"),
            '→' => out.push_str("\\textrightarrow{}"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escape a URL for use inside `\href{...}`.
fn url_escape(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for ch in url.chars() {
        match ch {
            '#' | '%' | '&' | '_' | '{' | '}' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            ' ' => out.push_str("%20"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_latex_control_characters() {
        assert_eq!(esc("R&D #1"), "R\\&D \\#1");
        assert_eq!(esc("a_b{c}"), "a\\_b\\{c\\}");
        assert_eq!(esc("100% $5"), "100\\% \\$5");
    }

    #[test]
    fn maps_typographic_characters() {
        assert_eq!(esc("2018 – 2020"), "2018 -- 2020");
        assert_eq!(esc("a · b"), "a \\textperiodcentered{} b");
        assert_eq!(esc("25 → 2000"), "25 \\textrightarrow{} 2000");
    }

    #[test]
    fn escapes_url_specials() {
        assert_eq!(
            url_escape("https://x.test/a_b?c#d"),
            "https://x.test/a\\_b?c\\#d"
        );
    }

    #[test]
    fn adds_a_scheme_only_when_missing() {
        assert_eq!(
            with_scheme("www.linkedin.com/in/ada"),
            "https://www.linkedin.com/in/ada"
        );
        assert_eq!(with_scheme("https://x.test"), "https://x.test");
    }

    #[test]
    fn detects_urls() {
        assert!(looks_like_url("www.linkedin.com/in/ada"));
        assert!(looks_like_url("https://x.test"));
        assert!(!looks_like_url("adalovelace"));
        assert!(!looks_like_url("github.com/ada (Portfolio)"));
    }
}
