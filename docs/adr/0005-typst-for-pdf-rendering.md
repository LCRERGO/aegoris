# Typst for PDF rendering

**Status**: accepted

The PDF renderer embeds `typst` through `typst-as-lib`. It is pure Rust,
produces high-quality typography without an external browser or process, and
generates deterministic output, which makes snapshot testing possible.

The alternatives were a low-level PDF crate (`printpdf`), Markdown → HTML →
headless Chromium, or shelling out to `pandoc`/`weasyprint`. The low-level
crates require hand-built layout; the browser and external-process routes add a
heavy runtime dependency and non-deterministic output.

This is a meaningful lock-in choice, and a future reader might wonder why a
resume tool does not simply emit HTML. Markdown remains the canonical
representation, so the PDF renderer is replaceable at the cost of one module.
PDF support is compiled behind the `pdf` feature (enabled by default in the
CLI) to keep library-only builds light.
