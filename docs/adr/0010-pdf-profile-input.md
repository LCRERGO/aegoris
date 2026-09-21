# PDF as a Profile input, parsed deterministically

**Status**: accepted

A PDF can be passed as `--profile`. The CLI extracts positioned text with the
pure-Rust `pdf-extract` crate (custom `OutputDev`) and hands a
`PositionedLayout` to a pure normalizer in `aegoris-core`, which produces a
`Profile`. It joins canonical JSON, plain text, the LinkedIn `.zip` export, and
a LinkedIn URL as profile inputs.

The target is the LinkedIn "Save to PDF" résumé (Apache FOP, two-column,
localized section headers). Because the columns interleave under flat text
extraction, the normalizer splits them by x-position and maps headers through
an en + pt-BR dictionary. Detection is by `%PDF` magic bytes; PDF metadata is a
hint, not a gate — success is defined by producing a valid `Profile`.

An optional LLM structuring pass exists but is **opt-in only** (`llm` mode): the
deterministic parser runs first and, if it fails, the input errors unless the
LLM was explicitly requested. This keeps ingestion deterministic by default and
confines non-determinism to a deliberate choice. OCR is deliberately not
implemented: the target PDFs always carry a text layer, and OCR would add a
system-level dependency for no benefit on that target. The `Languages` section
is dropped on import; modeling languages is a separate decision.
