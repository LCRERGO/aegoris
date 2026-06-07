use std::path::PathBuf;

use aegoris_core::domain::OutputFormat;
use aegoris_core::grounding::GroundingPolicy;
use aegoris_core::CoreError;
use aegoris_llm::{DEFAULT_BASE_URL, DEFAULT_MODEL};
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Use the LLM when an API key is available, otherwise fall back to templates.
    Auto,
    Llm,
    Template,
}

pub const DEFAULT_EMBEDDINGS_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_EMBEDDINGS_MODEL: &str = "text-embedding-3-small";

#[derive(Debug, Clone)]
pub struct EmbeddingsConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub mode: Mode,
    pub grounding: GroundingPolicy,
    pub verifier_model: Option<String>,
    pub max_bullets: usize,
    pub out_dir: PathBuf,
    pub formats: Vec<OutputFormat>,
    pub force: bool,
    pub dry_run: bool,
    pub embeddings: Option<EmbeddingsConfig>,
}

impl Settings {
    /// Resolve the effective mode, erroring if `llm` was demanded without a key.
    pub fn effective_mode(&self) -> Result<Mode, CoreError> {
        match self.mode {
            Mode::Llm if self.api_key.is_none() => Err(CoreError::Config(
                "--mode llm requires an API key (set AEGORIS_API_KEY or DEEPSEEK_API_KEY)"
                    .to_string(),
            )),
            Mode::Auto if self.api_key.is_some() => Ok(Mode::Llm),
            Mode::Auto => Ok(Mode::Template),
            other => Ok(other),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileConfig {
    #[serde(default)]
    pub llm: Option<FileLlm>,
    #[serde(default)]
    pub embeddings: Option<FileEmbeddings>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileEmbeddings {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileLlm {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Load `aegoris.toml` from the current directory, then the XDG config dir.
pub fn load_file_config() -> Result<Option<FileConfig>> {
    for path in candidate_config_paths() {
        if path.is_file() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let config: FileConfig =
                toml::from_str(&contents).with_context(|| format!("parsing {}", path.display()))?;
            return Ok(Some(config));
        }
    }
    Ok(None)
}

fn candidate_config_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("aegoris.toml")];
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(config_home).join("aegoris/aegoris.toml"));
    } else if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".config/aegoris/aegoris.toml"));
    }
    paths
}

pub fn api_key_from_env() -> Option<String> {
    std::env::var("AEGORIS_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("DEEPSEEK_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

pub fn resolve_provider(cli: Option<String>, file: Option<&FileConfig>) -> String {
    cli.or_else(|| std::env::var("AEGORIS_PROVIDER").ok())
        .or_else(|| {
            file.and_then(|f| f.llm.as_ref())
                .and_then(|l| l.provider.clone())
        })
        .unwrap_or_else(|| "deepseek".to_string())
}

pub fn resolve_base_url(cli: Option<String>, file: Option<&FileConfig>) -> String {
    cli.or_else(|| std::env::var("AEGORIS_BASE_URL").ok())
        .or_else(|| {
            file.and_then(|f| f.llm.as_ref())
                .and_then(|l| l.base_url.clone())
        })
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

pub fn resolve_model(cli: Option<String>, file: Option<&FileConfig>) -> String {
    cli.or_else(|| std::env::var("AEGORIS_MODEL").ok())
        .or_else(|| {
            file.and_then(|f| f.llm.as_ref())
                .and_then(|l| l.model.clone())
        })
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

pub fn embeddings_api_key_from_env() -> Option<String> {
    std::env::var("AEGORIS_EMBEDDINGS_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// Resolve the optional embeddings configuration.
///
/// Embeddings are enabled by `--embeddings` or `[embeddings] enabled = true`.
/// The API key falls back to the main LLM key so an OpenAI-compatible
/// embeddings endpoint can share credentials.
pub fn resolve_embeddings(
    enabled_flag: bool,
    cli_base_url: Option<String>,
    cli_model: Option<String>,
    file: Option<&FileConfig>,
    llm_api_key: Option<&str>,
) -> Result<Option<EmbeddingsConfig>, CoreError> {
    let file_embeddings = file.and_then(|f| f.embeddings.as_ref());
    let enabled = enabled_flag || file_embeddings.and_then(|e| e.enabled).unwrap_or(false);
    if !enabled {
        return Ok(None);
    }

    let base_url = cli_base_url
        .or_else(|| std::env::var("AEGORIS_EMBEDDINGS_BASE_URL").ok())
        .or_else(|| file_embeddings.and_then(|e| e.base_url.clone()))
        .unwrap_or_else(|| DEFAULT_EMBEDDINGS_BASE_URL.to_string());
    let model = cli_model
        .or_else(|| std::env::var("AEGORIS_EMBEDDINGS_MODEL").ok())
        .or_else(|| file_embeddings.and_then(|e| e.model.clone()))
        .unwrap_or_else(|| DEFAULT_EMBEDDINGS_MODEL.to_string());
    let api_key = embeddings_api_key_from_env()
        .or_else(|| llm_api_key.map(str::to_string))
        .ok_or_else(|| {
            CoreError::Config(
                "--embeddings requires an API key (set AEGORIS_EMBEDDINGS_API_KEY or AEGORIS_API_KEY)"
                    .to_string(),
            )
        })?;

    Ok(Some(EmbeddingsConfig {
        base_url,
        model,
        api_key,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(mode: Mode, api_key: Option<&str>) -> Settings {
        Settings {
            provider: "deepseek".to_string(),
            base_url: DEFAULT_BASE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            api_key: api_key.map(str::to_string),
            mode,
            grounding: GroundingPolicy::Strict,
            verifier_model: None,
            max_bullets: 4,
            out_dir: PathBuf::from("out"),
            formats: vec![OutputFormat::Markdown],
            force: false,
            dry_run: false,
            embeddings: None,
        }
    }

    #[test]
    fn auto_mode_prefers_llm_when_key_present() {
        assert_eq!(
            settings(Mode::Auto, Some("k")).effective_mode().unwrap(),
            Mode::Llm
        );
        assert_eq!(
            settings(Mode::Auto, None).effective_mode().unwrap(),
            Mode::Template
        );
    }

    #[test]
    fn explicit_llm_without_key_is_an_error() {
        assert!(settings(Mode::Llm, None).effective_mode().is_err());
    }

    #[test]
    fn provider_precedence_prefers_cli() {
        let file = FileConfig {
            llm: Some(FileLlm {
                provider: Some("file".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            resolve_provider(Some("cli".to_string()), Some(&file)),
            "cli"
        );
        assert_eq!(resolve_provider(None, Some(&file)), "file");
        assert_eq!(resolve_provider(None, None), "deepseek");
    }
}
