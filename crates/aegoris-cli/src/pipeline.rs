use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use aegoris_core::curate::{curate, CurationConfig};
use aegoris_core::domain::artifact::{Artifact, ArtifactKind};
use aegoris_core::domain::fact::FactStore;
use aegoris_core::domain::job::JobDescription;
use aegoris_core::domain::profile::Profile;
use aegoris_core::domain::{Claim, CurationPlan};
use aegoris_core::grounding::{
    apply_policy, ClaimVerifier, GroundingPolicy, StructuralVerifier, Verification,
};
use aegoris_core::matching::{CachedEmbedder, HybridScorer, LexicalScorer, RelevanceScorer};
use aegoris_core::parse::{parse_job_description, parse_linkedin_export, parse_profile};
use aegoris_core::phrase::{PhraseContext, Phraser, TemplatePhraser};
use aegoris_llm::{
    AsyncEmbedder, LlmClaimVerifier, LlmPhraser, OpenAiCompatEmbedder, OpenAiCompatModel,
};
use aegoris_render::{render, RenderContext};
use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::config::{Mode, Settings};

const PROMPT_VERSION: &str = "v1";

pub fn load_profile(path: &Path) -> Result<Profile> {
    if path != Path::new("-") && is_zip(path) {
        return load_linkedin_zip(path);
    }
    let contents = read_input(path)?;
    parse_profile(&contents).with_context(|| format!("parsing profile {}", path.display()))
}

fn is_zip(path: &Path) -> bool {
    path.extension()
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

/// Unpack a LinkedIn data-export archive and normalize it.
///
/// The archive is read here (I/O); the CSV-to-`Profile` normalization lives in
/// `aegoris-core`.
fn load_linkedin_zip(path: &Path) -> Result<Profile> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("reading zip {}", path.display()))?;

    let mut files = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .with_context(|| format!("reading entry {index} of {}", path.display()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if !name.to_lowercase().ends_with(".csv") {
            continue;
        }
        let base = Path::new(&name)
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or(&name)
            .to_string();
        let mut contents = String::new();
        use std::io::Read;
        entry
            .read_to_string(&mut contents)
            .with_context(|| format!("reading {name} from {}", path.display()))?;
        files.insert(base, contents);
    }

    parse_linkedin_export(&files)
        .with_context(|| format!("parsing LinkedIn export {}", path.display()))
}

pub fn load_jd(path: &Path) -> Result<JobDescription> {
    let contents = read_input(path)?;
    Ok(parse_job_description(&contents))
}

pub fn read_input(path: &Path) -> Result<String> {
    if path == Path::new("-") {
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("reading stdin")?;
        return Ok(buffer);
    }
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

pub fn build_plan(
    profile: &Profile,
    jd: &JobDescription,
    store: &FactStore,
    max_bullets: usize,
) -> Result<CurationPlan> {
    let config = CurationConfig {
        max_bullets_per_role: max_bullets,
        ..Default::default()
    };
    let plan = curate(profile, jd, store, &LexicalScorer::new(), &config)?;
    Ok(plan)
}

/// Build the relevance scorer, prefetching embeddings when configured.
///
/// Embeddings are fetched once for every fact and requirement, then exposed to
/// the synchronous core as a [`CachedEmbedder`].
async fn build_scorer(
    settings: &Settings,
    jd: &JobDescription,
    store: &FactStore,
) -> Result<Box<dyn RelevanceScorer>> {
    let mut scorer = HybridScorer::new();
    if let Some(embeddings) = &settings.embeddings {
        let texts = collect_embedding_texts(jd, store);
        let embedder = OpenAiCompatEmbedder::new(
            embeddings.base_url.clone(),
            embeddings.api_key.clone(),
            embeddings.model.clone(),
        );
        let vectors = embedder.embed(&texts).await?;
        let dimension = vectors.first().map(Vec::len).unwrap_or(0);
        if dimension == 0 {
            bail!("embeddings provider returned no vectors");
        }
        let cached = CachedEmbedder::from_pairs(dimension, texts.into_iter().zip(vectors));
        scorer = scorer.with_embedder(Box::new(cached));
    }
    Ok(Box::new(scorer))
}

fn collect_embedding_texts(jd: &JobDescription, store: &FactStore) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut texts = Vec::new();
    for requirement in &jd.requirements {
        if seen.insert(requirement.text.clone()) {
            texts.push(requirement.text.clone());
        }
    }
    for fact in store.iter() {
        if seen.insert(fact.text.clone()) {
            texts.push(fact.text.clone());
        }
    }
    texts
}

pub async fn run_generate(
    settings: &Settings,
    profile_path: &Path,
    jd_path: &Path,
) -> Result<Vec<PathBuf>> {
    let profile = load_profile(profile_path)?;
    let jd = load_jd(jd_path)?;
    let store = FactStore::from_profile(&profile);
    let mode = settings.effective_mode()?;

    let scorer = build_scorer(settings, &jd, &store).await?;
    let config = CurationConfig {
        max_bullets_per_role: settings.max_bullets,
        ..Default::default()
    };
    let plan = curate(&profile, &jd, &store, scorer.as_ref(), &config)?;

    if settings.dry_run {
        print_plan(&plan)?;
        return Ok(Vec::new());
    }

    let slug = slugify(&profile.name);
    let mut written = Vec::new();

    for kind in [ArtifactKind::Resume, ArtifactKind::CoverLetter] {
        let claims = phrase_claims(settings, mode, kind, &profile, &jd, &plan, &store).await?;
        let claims = ground_claims(settings, mode, claims, &store).await?;

        let context = RenderContext {
            profile: &profile,
            jd: &jd,
            plan: &plan,
            claims: &claims,
            store: &store,
        };

        for format in &settings.formats {
            let artifact = render(&context, kind, *format)?;
            written.push(write_artifact(settings, &slug, &artifact)?);
        }
    }

    written.push(write_curation(settings, &slug, &plan, mode)?);
    Ok(written)
}

async fn phrase_claims(
    settings: &Settings,
    mode: Mode,
    kind: ArtifactKind,
    profile: &Profile,
    jd: &JobDescription,
    plan: &CurationPlan,
    store: &FactStore,
) -> Result<Vec<Claim>> {
    let context = PhraseContext {
        kind,
        profile,
        jd,
        plan,
        store,
    };
    match mode {
        Mode::Template | Mode::Auto => Ok(TemplatePhraser::new().phrase(&context)?),
        Mode::Llm => {
            let model = build_model(settings, None)?;
            let claims = LlmPhraser::new(model).phrase(&context).await?;
            Ok(claims)
        }
    }
}

async fn ground_claims(
    settings: &Settings,
    mode: Mode,
    claims: Vec<Claim>,
    store: &FactStore,
) -> Result<Vec<Claim>> {
    let structural = StructuralVerifier.verify(&claims, store)?;
    let verifications = if mode == Mode::Llm {
        let model = build_model(settings, settings.verifier_model.as_deref())?;
        let semantic = LlmClaimVerifier::new(model)
            .verify_claims(&claims, store)
            .await?;
        merge_verifications(structural, semantic)
    } else {
        structural
    };
    let kept = apply_policy(claims, &verifications, settings.grounding)?;
    Ok(kept)
}

fn merge_verifications(
    structural: Vec<Verification>,
    semantic: Vec<Verification>,
) -> Vec<Verification> {
    structural
        .into_iter()
        .enumerate()
        .map(|(index, base)| {
            let semantic = semantic.get(index);
            let supported = base.supported && semantic.map(|s| s.supported).unwrap_or(false);
            if supported {
                Verification::supported(index)
            } else {
                let detail = [base.detail, semantic.and_then(|s| s.detail.clone())]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("; ");
                Verification::unsupported(index, detail)
            }
        })
        .collect()
}

fn build_model(settings: &Settings, model_override: Option<&str>) -> Result<OpenAiCompatModel> {
    let api_key = settings
        .api_key
        .clone()
        .context("no API key configured (set AEGORIS_API_KEY or DEEPSEEK_API_KEY)")?;
    let model = model_override.unwrap_or(&settings.model);
    Ok(OpenAiCompatModel::new(settings.base_url.clone(), api_key, model).with_json_mode(true))
}

fn write_artifact(settings: &Settings, slug: &str, artifact: &Artifact) -> Result<PathBuf> {
    std::fs::create_dir_all(&settings.out_dir)
        .with_context(|| format!("creating {}", settings.out_dir.display()))?;
    let filename = format!(
        "{slug}-{}.{}",
        artifact.kind.slug(),
        artifact.format.extension()
    );
    let path = settings.out_dir.join(filename);
    if path.exists() && !settings.force {
        bail!(
            "{} already exists; pass --force to overwrite",
            path.display()
        );
    }
    std::fs::write(&path, &artifact.content)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

#[derive(Serialize)]
struct CurationRecord<'a> {
    schema: &'static str,
    mode: &'static str,
    provider: &'a str,
    model: &'a str,
    base_url: &'a str,
    prompt_version: &'static str,
    temperature: f32,
    max_bullets_per_role: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    embeddings_model: Option<&'a str>,
    plan: &'a CurationPlan,
}

fn write_curation(
    settings: &Settings,
    slug: &str,
    plan: &CurationPlan,
    mode: Mode,
) -> Result<PathBuf> {
    std::fs::create_dir_all(&settings.out_dir)
        .with_context(|| format!("creating {}", settings.out_dir.display()))?;
    let path = settings.out_dir.join(format!("{slug}-curation.json"));
    if path.exists() && !settings.force {
        bail!(
            "{} already exists; pass --force to overwrite",
            path.display()
        );
    }
    let record = CurationRecord {
        schema: "aegoris/curation/v1",
        mode: match mode {
            Mode::Llm => "llm",
            _ => "template",
        },
        provider: if mode == Mode::Llm {
            settings.provider.as_str()
        } else {
            "none"
        },
        model: if mode == Mode::Llm {
            settings.model.as_str()
        } else {
            "template"
        },
        base_url: if mode == Mode::Llm {
            settings.base_url.as_str()
        } else {
            ""
        },
        prompt_version: if mode == Mode::Llm {
            PROMPT_VERSION
        } else {
            "n/a"
        },
        temperature: 0.0,
        max_bullets_per_role: plan.max_bullets_per_role,
        embeddings_model: settings.embeddings.as_ref().map(|e| e.model.as_str()),
        plan,
    };
    let json = serde_json::to_string_pretty(&record)?;
    std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

fn print_plan(plan: &CurationPlan) -> Result<()> {
    let json = serde_json::to_string_pretty(plan)?;
    println!("{json}");
    Ok(())
}

pub fn run_parse(profile_path: &Path, out: Option<&Path>) -> Result<()> {
    let profile = load_profile(profile_path)?;
    let json = serde_json::to_string_pretty(&profile)?;
    match out {
        Some(path) => {
            std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
        }
        None => println!("{json}"),
    }
    Ok(())
}

pub fn run_score(profile_path: &Path, jd_path: &Path) -> Result<()> {
    let profile = load_profile(profile_path)?;
    let jd = load_jd(jd_path)?;
    let store = FactStore::from_profile(&profile);
    let plan = build_plan(&profile, &jd, &store, usize::MAX)?;

    println!("Requirements: {}", jd.requirements.len());
    for requirement in &jd.requirements {
        let hits = plan
            .scored
            .iter()
            .filter(|s| s.matched_requirements.contains(&requirement.id))
            .count();
        println!(
            "  [{}] {} ({} matching fact(s))",
            requirement.id, requirement.text, hits
        );
    }

    println!("\nTop facts:");
    let mut scored = plan.scored.clone();
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for scored_fact in scored.iter().take(15) {
        if let Some(fact) = store.get(&scored_fact.fact_id) {
            println!("  {:.3}  [{}] {}", scored_fact.score, fact.id, fact.text);
        }
    }
    Ok(())
}

pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            slug.extend(ch.to_lowercase());
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "profile".to_string()
    } else {
        slug
    }
}

#[allow(dead_code)]
fn _assert_policy(_: GroundingPolicy) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_names() {
        assert_eq!(slugify("Ada Lovelace"), "ada-lovelace");
        assert_eq!(slugify("  Ada  M.  Lovelace "), "ada-m-lovelace");
        assert_eq!(slugify("!!!"), "profile");
    }

    #[test]
    fn merge_requires_both_layers_to_support() {
        let structural = vec![Verification::supported(0), Verification::supported(1)];
        let semantic = vec![
            Verification::supported(0),
            Verification::unsupported(1, "not entailed"),
        ];
        let merged = merge_verifications(structural, semantic);
        assert!(merged[0].supported);
        assert!(!merged[1].supported);
    }
}
