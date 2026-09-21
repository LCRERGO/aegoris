# LaTeX output is source, not a compiled PDF

**Status**: accepted

`--format tex` (alias `latex`) emits a `.tex` artifact rendered natively from
the same `RenderContext` as the Markdown and ATS renderers. LaTeX joins `md`,
`ats`, and `pdf` as an output format; it is produced for both the Resume and
the Cover Letter, like every other format.

The document is a self-contained `article` with `[T1]{fontenc}`, `geometry`,
and `hyperref`. It does **not** shell out to `pdflatex`/`xelatex`; the user
compiles the source with whichever engine they prefer. This mirrors ADR 0005,
which rejected external processes (`pandoc`, `weasyprint`, a headless browser)
for the PDF renderer in favour of pure, deterministic Rust. LaTeX is emitted as
text for the same reason, and because a `.tex` artifact is meant to be edited
and compiled by the candidate.

`inputenc` is deliberately omitted so the same source builds under modern
pdfLaTeX (UTF-8 by default), XeLaTeX, and LuaLaTeX. Literal text is escaped
(`& % $ # _ { } ~ ^ \`) with a few typographic characters mapped to LaTeX
(`–` → `--`, `·` → `\textperiodcentered`, `→` → `\textrightarrow`); contact
fields that look like URLs become `\href` links.
