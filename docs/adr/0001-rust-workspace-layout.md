# Rust workspace with an I/O-free core

aegoris is a Cargo workspace of four crates with a strictly inward dependency
direction: `aegoris-cli` → `aegoris-render` / `aegoris-llm` → `aegoris-core`.

`aegoris-core` holds the domain model, parsing, matching, curation, grounding,
and template phrasing. It performs no I/O and no async work, so its tests are
pure, fast, and fully deterministic. Network access is confined to
`aegoris-llm`, and rendering to `aegoris-render`.

The alternative was a single crate with modules. We chose a workspace because
the compile-time boundary prevents the core from accidentally acquiring a
network or filesystem dependency, which is what keeps TDD viable for a project
whose interesting behaviour would otherwise be model-dependent.
