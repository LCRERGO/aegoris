# aegoris

Curated, grounded resumes and cover letters from a **Profile** and a **Job Description**.

aegoris parses a profile and a job posting, scores and selects the profile
**Facts** that matter for that posting, then phrases them into a **Resume** and
**Cover Letter**. Every generated **Claim** is structurally linked to the
**Facts** it came from, so nothing is fabricated. See [CONTEXT.md](CONTEXT.md)
for the vocabulary.

## Why it is different

- **Grounded by construction.** A claim cannot exist without evidence; the type
  system enforces it. In `llm` mode a second semantic verifier checks that the
  cited facts actually support the claim.
- **Deterministic core.** Parsing, matching, and curation are pure functions
  with no network access and no model, which makes them fully testable.
- **Works offline.** `--mode template` produces a complete, ATS-friendly resume
  and cover letter with no API key and no network.
- **Real inputs.** Accepts canonical JSON, plain text, or a LinkedIn data-export
  `.zip`.
- **Auditable.** Each run writes a curation plan recording the selected facts,
  their scores, and the model settings.

## Build

```sh
cargo build --release
```

## Usage

```sh
# Deterministic, no network:
aegoris generate --profile fixtures/ada-profile.json --jd fixtures/backend-jd.txt \
                 --out out --no-llm --format md,ats

# With an LLM (DeepSeek by default):
export DEEPSEEK_API_KEY=...
aegoris generate --profile profile.json --jd job.txt --mode llm --format md

# Inspect inputs without generating:
aegoris parse --profile profile.json
aegoris score --profile profile.json --jd job.txt
```

### Commands

| Command | Purpose |
|---|---|
| `generate` | Produce a resume and cover letter |
| `parse` | Normalize a profile and print canonical JSON |
| `score` | Report requirement/fact relevance, no generation |

### `generate` options

| Flag | Description |
|---|---|
| `--profile <path>` | Profile JSON or plain text (`-` for stdin) |
| `--jd <path>` | Job description text (`-` for stdin) |
| `--out <dir>` | Output directory (default `out`) |
| `--format md,ats,pdf` | Output formats (default `md,ats`) |
| `--mode auto\|llm\|template` | Generation mode (default `auto`) |
| `--no-llm` | Shorthand for `--mode template` |
| `--provider <name>` | `deepseek` or `openai-compatible` |
| `--model <id>` | Model identifier |
| `--base-url <url>` | OpenAI-compatible endpoint |
| `--max-bullets <n>` | Maximum bullets per role |
| `--grounding strict\|warn` | Abort on unsupported claims, or drop them |
| `--verifier-model <id>` | Separate model for the grounding verifier |
| `--embeddings` | Enable the semantic embedding stage of matching |
| `--embeddings-model <id>` | Embeddings model (default `text-embedding-3-small`) |
| `--embeddings-base-url <url>` | OpenAI-compatible embeddings endpoint |
| `--dry-run` | Print the curation plan and exit |
| `--force` | Overwrite existing outputs |
| `-v`, `-vv`, `--quiet`, `--json-logs` | Logging |

Exit codes: `0` success, `1` general, `2` usage/config, `3` provider,
`4` grounding failure.

## Configuration

Precedence: CLI flags → environment → `aegoris.toml` → defaults.

```toml
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
model = "deepseek-flash"

[embeddings]
enabled = true
base_url = "https://api.openai.com/v1"
model = "text-embedding-3-small"
```

The API key is read from `AEGORIS_API_KEY` or `DEEPSEEK_API_KEY`. Embeddings use
`AEGORIS_EMBEDDINGS_API_KEY` if set, falling back to the main LLM key. Keys are
never written to disk or logged.

## Profile formats

`generate` and `parse` accept either:

1. **Canonical JSON** — see `fixtures/ada-profile.json`.
2. **Plain text** — a `# Name` line, optional `key: value` contact lines, and
   `## Section` headings with `### Title @ Organization (start - end)` roles and
   `-` bullets. See the `parse_profile_text` docs.
3. **LinkedIn data export** — a `.zip` from LinkedIn's "Get a copy of your
   data" (or the unpacked CSVs), parsed from `Profile.csv`, `Positions.csv`,
   `Education.csv`, `Skills.csv`, `Certifications.csv`, and `Projects.csv`.

## How it works

```
Profile + JD
   │
   ├─ parse ──────────────► Profile, JobDescription (Requirements)
   ├─ match ──────────────► Relevance Score per Fact (lexical → embeddings → judge)
   ├─ curate ─────────────► Curation Plan (selected + ordered Facts)
   ├─ phrase ─────────────► Claims (template or LLM)
   ├─ ground ─────────────► structural + semantic verification
   └─ render ─────────────► Resume / Cover Letter (md, ats, pdf*)
```

\* PDF rendering is compiled behind the `pdf` feature (on by default in the
CLI); see [ADR 0005](docs/adr/0005-typst-for-pdf-rendering.md).

## Architecture

```
crates/aegoris-core/    domain, parse, matching, curate, grounding, phrase  (no I/O)
crates/aegoris-llm/     LanguageModel trait, OpenAI-compatible adapter, fakes
crates/aegoris-render/  Markdown, ATS, and Typst PDF renderers (`pdf` feature)
crates/aegoris-cli/     clap CLI, config, pipeline orchestration
prompts/v1/             versioned system prompts
docs/adr/               architecture decisions
```

Dependencies point inward: `cli → render/llm → core`. The core never touches
the network or the filesystem.

## Testing

```sh
cargo test --workspace
```

The deterministic core is covered by unit tests, the LLM pipeline runs
end-to-end against `FakeLanguageModel`, and the CLI is exercised by integration
tests in `crates/aegoris-cli/tests/cli.rs`. No test requires a network
connection or an API key.

## License

MIT.
