use std::path::PathBuf;

use aegoris_core::domain::OutputFormat;
use aegoris_core::grounding::GroundingPolicy;
use aegoris_core::CoreError;
use aegoris_llm::{LlmError, ProviderError};
use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use tracing_subscriber::EnvFilter;

mod config;
mod pdf;
mod pipeline;
mod scrape;

use config::{
    api_key_from_env, load_file_config, resolve_base_url, resolve_embeddings, resolve_model,
    resolve_provider, Mode, Settings,
};

#[derive(Parser)]
#[command(
    name = "aegoris",
    version,
    about = "Curated, grounded resumes and cover letters from a profile and a job description"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Increase log verbosity (-v info, -vv debug).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Only log errors.
    #[arg(long, global = true)]
    quiet: bool,

    /// Emit logs as JSON.
    #[arg(long, global = true)]
    json_logs: bool,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Command {
    /// Generate a curated resume and cover letter.
    Generate(GenerateArgs),
    /// Parse a profile and print its canonical JSON.
    Parse(ParseArgs),
    /// Report fact/requirement relevance without generating anything.
    Score(ScoreArgs),
}

#[derive(Args)]
struct GenerateArgs {
    /// Profile source: JSON, plain text, a LinkedIn export `.zip`, a PDF, a
    /// LinkedIn profile URL to scrape, or `-` for stdin.
    #[arg(long)]
    profile: PathBuf,

    /// Path to the job description text. Use `-` for stdin.
    #[arg(long)]
    jd: PathBuf,

    /// Output directory.
    #[arg(long, default_value = "out")]
    out: PathBuf,

    /// Output formats: md, ats, pdf (comma-separated).
    #[arg(long, value_delimiter = ',', default_value = "md,ats")]
    format: Vec<String>,

    /// Generation mode.
    #[arg(long, value_enum, default_value_t = ModeArg::Auto)]
    mode: ModeArg,

    /// Shorthand for `--mode template`.
    #[arg(long)]
    no_llm: bool,

    /// LLM provider (deepseek, openai-compatible).
    #[arg(long)]
    provider: Option<String>,

    /// Model identifier.
    #[arg(long)]
    model: Option<String>,

    /// OpenAI-compatible base URL.
    #[arg(long)]
    base_url: Option<String>,

    /// Maximum bullets per role.
    #[arg(long, default_value_t = 4)]
    max_bullets: usize,

    /// Grounding failure policy.
    #[arg(long, value_enum, default_value_t = GroundingArg::Strict)]
    grounding: GroundingArg,

    /// Optional separate model for the grounding verifier.
    #[arg(long)]
    verifier_model: Option<String>,

    /// Enable the semantic embedding stage of relevance matching.
    #[arg(long)]
    embeddings: bool,

    /// Embeddings model identifier.
    #[arg(long)]
    embeddings_model: Option<String>,

    /// OpenAI-compatible embeddings base URL.
    #[arg(long)]
    embeddings_base_url: Option<String>,

    /// Print the curation plan and exit without generating.
    #[arg(long)]
    dry_run: bool,

    /// Overwrite existing output files.
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct ParseArgs {
    /// Profile source: JSON, plain text, a LinkedIn export `.zip`, a PDF, or a
    /// LinkedIn profile URL to scrape.
    #[arg(long)]
    profile: PathBuf,

    #[arg(long)]
    out: Option<PathBuf>,

    /// Allow the LLM to structure a PDF the deterministic parser cannot read.
    #[arg(long)]
    llm: bool,
}

#[derive(Args)]
struct ScoreArgs {
    /// Profile source: JSON, plain text, a LinkedIn export `.zip`, a PDF, or a
    /// LinkedIn profile URL to scrape.
    #[arg(long)]
    profile: PathBuf,

    #[arg(long)]
    jd: PathBuf,

    /// Allow the LLM to structure a PDF the deterministic parser cannot read.
    #[arg(long)]
    llm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModeArg {
    Auto,
    Llm,
    Template,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum GroundingArg {
    Strict,
    Warn,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_tracing(&cli);

    let code = match run(cli).await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("error: {error:#}");
            classify_exit(&error)
        }
    };
    std::process::exit(code);
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Generate(args) => {
            let settings = build_settings(&args)?;
            let paths = pipeline::run_generate(&settings, &args.profile, &args.jd).await?;
            for path in paths {
                println!("{}", path.display());
            }
            Ok(())
        }
        Command::Parse(args) => {
            pipeline::run_parse(&args.profile, args.out.as_deref(), args.llm).await
        }
        Command::Score(args) => pipeline::run_score(&args.profile, &args.jd, args.llm).await,
    }
}

fn build_settings(args: &GenerateArgs) -> Result<Settings> {
    let file = load_file_config()?;
    let provider = resolve_provider(args.provider.clone(), file.as_ref());
    let base_url = resolve_base_url(args.base_url.clone(), file.as_ref());
    let model = resolve_model(args.model.clone(), file.as_ref());

    let formats = args
        .format
        .iter()
        .map(|value| {
            OutputFormat::parse(value).ok_or_else(|| anyhow::anyhow!("unknown format: {value}"))
        })
        .collect::<Result<Vec<_>>>()?;
    if formats.is_empty() {
        anyhow::bail!("at least one --format is required");
    }

    let mode = if args.no_llm {
        Mode::Template
    } else {
        match args.mode {
            ModeArg::Auto => Mode::Auto,
            ModeArg::Llm => Mode::Llm,
            ModeArg::Template => Mode::Template,
        }
    };

    let grounding = match args.grounding {
        GroundingArg::Strict => GroundingPolicy::Strict,
        GroundingArg::Warn => GroundingPolicy::Warn,
    };

    let api_key = api_key_from_env();
    let embeddings = resolve_embeddings(
        args.embeddings,
        args.embeddings_base_url.clone(),
        args.embeddings_model.clone(),
        file.as_ref(),
        api_key.as_deref(),
    )?;

    let settings = Settings {
        provider,
        base_url,
        model,
        api_key,
        mode,
        grounding,
        verifier_model: args.verifier_model.clone(),
        max_bullets: args.max_bullets,
        out_dir: args.out.clone(),
        formats,
        force: args.force,
        dry_run: args.dry_run,
        embeddings,
    };

    Ok(settings)
}

fn classify_exit(error: &anyhow::Error) -> i32 {
    if let Some(core) = error.downcast_ref::<CoreError>() {
        return match core {
            CoreError::Grounding(_) => 4,
            CoreError::Config(_) => 2,
            _ => 1,
        };
    }
    if let Some(LlmError::Grounding(_)) = error.downcast_ref::<LlmError>() {
        return 4;
    }
    if error.downcast_ref::<ProviderError>().is_some() || error.downcast_ref::<LlmError>().is_some()
    {
        return 3;
    }
    1
}

fn init_tracing(cli: &Cli) {
    let level = if cli.quiet {
        "error"
    } else {
        match cli.verbose {
            0 => "warn",
            1 => "info",
            _ => "debug",
        }
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    if cli.json_logs {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }
}
