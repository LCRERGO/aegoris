//! Renderers: turn grounded claims plus a curation plan into artifacts.
//!
//! Markdown is the canonical representation; the ATS renderer emits plain text
//! for applicant-tracking systems, and LaTeX emits compilable `.tex` source.
//! PDF is feature-gated behind `pdf`.

mod ats;
mod error;
mod latex;
mod markdown;

#[cfg(feature = "pdf")]
mod pdf;

use aegoris_core::domain::artifact::{Artifact, ArtifactKind, OutputFormat};
use aegoris_core::domain::fact::FactStore;
use aegoris_core::domain::job::JobDescription;
use aegoris_core::domain::profile::Profile;
use aegoris_core::domain::{Claim, CurationPlan};

pub use error::{RenderError, RenderResult};

/// Everything needed to render one artifact.
pub struct RenderContext<'a> {
    pub profile: &'a Profile,
    pub jd: &'a JobDescription,
    pub plan: &'a CurationPlan,
    pub claims: &'a [Claim],
    pub store: &'a FactStore,
}

/// Render a single artifact in the requested format.
pub fn render(
    context: &RenderContext<'_>,
    kind: ArtifactKind,
    format: OutputFormat,
) -> RenderResult<Artifact> {
    let content = match format {
        OutputFormat::Markdown => match kind {
            ArtifactKind::Resume => markdown::resume(context).into_bytes(),
            ArtifactKind::CoverLetter => markdown::cover_letter(context).into_bytes(),
        },
        OutputFormat::Ats => match kind {
            ArtifactKind::Resume => ats::resume(context).into_bytes(),
            ArtifactKind::CoverLetter => ats::cover_letter(context).into_bytes(),
        },
        OutputFormat::Latex => match kind {
            ArtifactKind::Resume => latex::resume(context).into_bytes(),
            ArtifactKind::CoverLetter => latex::cover_letter(context).into_bytes(),
        },
        OutputFormat::Pdf => {
            #[cfg(feature = "pdf")]
            {
                pdf::render_pdf(context, kind)?
            }
            #[cfg(not(feature = "pdf"))]
            {
                return Err(RenderError::UnsupportedFormat(
                    "PDF support requires building with the `pdf` feature".to_string(),
                ));
            }
        }
    };

    Ok(Artifact::new(kind, format, content))
}

/// Render every requested format for a given artifact kind.
pub fn render_all(
    context: &RenderContext<'_>,
    kind: ArtifactKind,
    formats: &[OutputFormat],
) -> RenderResult<Vec<Artifact>> {
    formats
        .iter()
        .map(|format| render(context, kind, *format))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aegoris_core::curate::{curate, CurationConfig};
    use aegoris_core::matching::LexicalScorer;
    use aegoris_core::parse::{parse_job_description, parse_profile_json};
    use aegoris_core::phrase::{PhraseContext, Phraser, TemplatePhraser};

    fn fixture() -> (
        aegoris_core::domain::Profile,
        aegoris_core::domain::JobDescription,
        FactStore,
    ) {
        let profile = parse_profile_json(
            r#"{
                "name": "Ada Lovelace",
                "headline": "Pioneering Programmer",
                "location": "London",
                "contact": {"email": "ada@example.com", "github": "adalovelace"},
                "summary": "Pioneering programmer and mathematician.",
                "roles": [{"id":"r_1","title":"Engineer","organization":"Babbage & Co.",
                           "start":"1842","end":"1843",
                           "bullets":["Wrote the first algorithm for the Analytical Engine"]}],
                "skills": [{"name":"Mathematics"},{"name":"Algorithms"}],
                "education": [{"institution":"University of London","degree":"Mathematics"}]
            }"#,
        )
        .unwrap();
        let jd = parse_job_description("Requirements:\n- Algorithms\n- Mathematics");
        let store = FactStore::from_profile(&profile);
        (profile, jd, store)
    }

    #[test]
    fn resume_markdown_contains_expected_sections() {
        let (profile, jd, store) = fixture();
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let claims = TemplatePhraser::new()
            .phrase(&PhraseContext {
                kind: ArtifactKind::Resume,
                profile: &profile,
                jd: &jd,
                plan: &plan,
                store: &store,
            })
            .unwrap();
        let context = RenderContext {
            profile: &profile,
            jd: &jd,
            plan: &plan,
            claims: &claims,
            store: &store,
        };
        let artifact = render(&context, ArtifactKind::Resume, OutputFormat::Markdown).unwrap();
        let text = artifact.as_text().unwrap();
        assert!(text.starts_with("# Ada Lovelace"));
        assert!(text.contains("## Summary"));
        assert!(text.contains("## Experience"));
        assert!(text.contains("## Skills"));
        assert!(text.contains("## Education"));
        assert!(text.contains("Wrote the first algorithm"));
    }

    #[test]
    fn ats_resume_is_plain_text() {
        let (profile, jd, store) = fixture();
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let claims = TemplatePhraser::new()
            .phrase(&PhraseContext {
                kind: ArtifactKind::Resume,
                profile: &profile,
                jd: &jd,
                plan: &plan,
                store: &store,
            })
            .unwrap();
        let context = RenderContext {
            profile: &profile,
            jd: &jd,
            plan: &plan,
            claims: &claims,
            store: &store,
        };
        let artifact = render(&context, ArtifactKind::Resume, OutputFormat::Ats).unwrap();
        let text = artifact.as_text().unwrap();
        assert!(!text.contains("##"));
        assert!(text.contains("EXPERIENCE"));
    }

    #[test]
    fn latex_resume_is_a_compilable_document() {
        let (profile, jd, store) = fixture();
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let claims = TemplatePhraser::new()
            .phrase(&PhraseContext {
                kind: ArtifactKind::Resume,
                profile: &profile,
                jd: &jd,
                plan: &plan,
                store: &store,
            })
            .unwrap();
        let context = RenderContext {
            profile: &profile,
            jd: &jd,
            plan: &plan,
            claims: &claims,
            store: &store,
        };
        let artifact = render(&context, ArtifactKind::Resume, OutputFormat::Latex).unwrap();
        let text = artifact.as_text().unwrap();
        assert!(text.starts_with("\\documentclass{article}"));
        assert!(text.contains("\\section*{Summary}"));
        assert!(text.contains("\\section*{Experience}"));
        assert!(text.contains("\\section*{Skills}"));
        assert!(text.contains("\\section*{Education}"));
        assert!(text.contains("Wrote the first algorithm"));
        assert!(text.contains("\\href{mailto:ada@example.com}{ada@example.com}"));
        assert!(text.trim_end().ends_with("\\end{document}"));
    }

    #[test]
    fn latex_cover_letter_is_a_compilable_document() {
        let (profile, jd, store) = fixture();
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let claims = TemplatePhraser::new()
            .phrase(&PhraseContext {
                kind: ArtifactKind::CoverLetter,
                profile: &profile,
                jd: &jd,
                plan: &plan,
                store: &store,
            })
            .unwrap();
        let context = RenderContext {
            profile: &profile,
            jd: &jd,
            plan: &plan,
            claims: &claims,
            store: &store,
        };
        let artifact = render(&context, ArtifactKind::CoverLetter, OutputFormat::Latex).unwrap();
        let text = artifact.as_text().unwrap();
        assert!(text.starts_with("\\documentclass{article}"));
        assert!(text.contains("Dear Hiring Manager,"));
        assert!(text.contains("Sincerely,"));
        assert!(text.trim_end().ends_with("\\end{document}"));
    }

    #[cfg(not(feature = "pdf"))]
    #[test]
    fn pdf_is_explicitly_unsupported_without_feature() {
        let (profile, jd, store) = fixture();
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let context = RenderContext {
            profile: &profile,
            jd: &jd,
            plan: &plan,
            claims: &[],
            store: &store,
        };
        let err = render(&context, ArtifactKind::Resume, OutputFormat::Pdf).unwrap_err();
        assert!(matches!(err, RenderError::UnsupportedFormat(_)));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pdf_renderer_produces_a_pdf() {
        let (profile, jd, store) = fixture();
        let plan = curate(
            &profile,
            &jd,
            &store,
            &LexicalScorer::new(),
            &CurationConfig::default(),
        )
        .unwrap();
        let claims = TemplatePhraser::new()
            .phrase(&PhraseContext {
                kind: ArtifactKind::Resume,
                profile: &profile,
                jd: &jd,
                plan: &plan,
                store: &store,
            })
            .unwrap();
        let context = RenderContext {
            profile: &profile,
            jd: &jd,
            plan: &plan,
            claims: &claims,
            store: &store,
        };
        let artifact = render(&context, ArtifactKind::Resume, OutputFormat::Pdf).unwrap();
        assert!(artifact.content.starts_with(b"%PDF"));
    }
}
