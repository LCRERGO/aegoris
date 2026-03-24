//! Deterministic core for aegoris.
//!
//! This crate contains the domain model, parsing, matching, curation,
//! grounding, and template phrasing. It performs **no I/O and no async work**,
//! which keeps its tests pure, fast, and fully deterministic. Network-backed
//! concerns (LLM providers) live in `aegoris-llm`; rendering lives in
//! `aegoris-render`.

pub mod curate;
pub mod domain;
pub mod error;
pub mod grounding;
pub mod matching;
pub mod parse;
pub mod phrase;

pub use error::{CoreError, CoreResult};
