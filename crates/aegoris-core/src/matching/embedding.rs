use std::collections::HashMap;

use crate::domain::ids::RequirementId;
use crate::domain::job::Requirement;
use crate::error::{CoreError, CoreResult};

/// Produces vector representations of text.
///
/// The default implementation is local and deterministic ([`HashingEmbedder`]);
/// a real embedding model can be dropped in behind this trait without the core
/// ever knowing.
pub trait Embedder {
    fn embed(&self, texts: &[String]) -> CoreResult<Vec<Vec<f32>>>;

    fn dimension(&self) -> usize;
}

/// Deterministic hashing embedder used for tests and as a lexical fallback.
///
/// It is not semantically meaningful, but it is stable, dependency-free, and
/// exercises the exact code path a real embedder would.
#[derive(Debug, Clone)]
pub struct HashingEmbedder {
    dim: usize,
}

impl HashingEmbedder {
    pub fn new(dim: usize) -> Self {
        Self { dim: dim.max(1) }
    }
}

impl Default for HashingEmbedder {
    fn default() -> Self {
        Self::new(256)
    }
}

impl Embedder for HashingEmbedder {
    fn embed(&self, texts: &[String]) -> CoreResult<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut vector = vec![0.0f32; self.dim];
                for token in crate::domain::fact::tokenize(text) {
                    let bucket = (fnv1a(&token) as usize) % self.dim;
                    vector[bucket] += 1.0;
                }
                l2_normalize(&mut vector);
                vector
            })
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// An embedder backed by a precomputed lookup table.
///
/// Network-backed embedders are async and live in `aegoris-llm`. The CLI
/// prefetches every fact and requirement vector once, then hands the core this
/// synchronous view, which keeps the matching pipeline deterministic and
/// offline.
#[derive(Debug, Clone, Default)]
pub struct CachedEmbedder {
    vectors: HashMap<String, Vec<f32>>,
    dim: usize,
}

impl CachedEmbedder {
    pub fn new(dim: usize) -> Self {
        Self {
            vectors: HashMap::new(),
            dim,
        }
    }

    pub fn from_pairs(dim: usize, pairs: impl IntoIterator<Item = (String, Vec<f32>)>) -> Self {
        let vectors = pairs.into_iter().collect();
        Self { vectors, dim }
    }

    pub fn insert(&mut self, text: impl Into<String>, vector: Vec<f32>) {
        self.dim = self.dim.max(vector.len());
        self.vectors.insert(text.into(), vector);
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

impl Embedder for CachedEmbedder {
    fn embed(&self, texts: &[String]) -> CoreResult<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| {
                self.vectors
                    .get(text)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; self.dim])
            })
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// Cosine similarity, returning `0.0` for zero-length or mismatched vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Similarity of one fact against each requirement, via the given embedder.
pub fn semantic_similarities(
    embedder: &dyn Embedder,
    fact_text: &str,
    requirements: &[Requirement],
) -> CoreResult<Vec<(RequirementId, f32)>> {
    if requirements.is_empty() {
        return Ok(Vec::new());
    }
    let mut texts = Vec::with_capacity(requirements.len() + 1);
    texts.push(fact_text.to_string());
    texts.extend(requirements.iter().map(|r| r.text.clone()));

    let vectors = embedder.embed(&texts)?;
    if vectors.len() != texts.len() {
        return Err(CoreError::validation(
            "embedder returned a different number of vectors than inputs",
        ));
    }
    let fact_vector = &vectors[0];
    if fact_vector.len() != embedder.dimension() {
        return Err(CoreError::validation(
            "embedder returned vectors of unexpected dimension",
        ));
    }

    Ok(requirements
        .iter()
        .zip(vectors.iter().skip(1))
        .map(|(requirement, vector)| {
            (
                requirement.id.clone(),
                cosine_similarity(fact_vector, vector),
            )
        })
        .collect())
}

fn l2_normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

fn fnv1a(input: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_is_maximally_similar() {
        let embedder = HashingEmbedder::default();
        let vectors = embedder
            .embed(&["rust async".into(), "rust async".into()])
            .unwrap();
        assert!((cosine_similarity(&vectors[0], &vectors[1]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn embedding_is_deterministic() {
        let embedder = HashingEmbedder::default();
        let a = embedder.embed(&["kubernetes clusters".into()]).unwrap();
        let b = embedder.embed(&["kubernetes clusters".into()]).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn mismatched_dimensions_yield_zero_similarity() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0]), 0.0);
    }

    #[test]
    fn cached_embedder_looks_up_and_falls_back_to_zero() {
        let embedder = CachedEmbedder::from_pairs(2, [("known".to_string(), vec![1.0, 0.0])]);
        let vectors = embedder.embed(&["known".into(), "unknown".into()]).unwrap();
        assert_eq!(vectors[0], vec![1.0, 0.0]);
        assert_eq!(vectors[1], vec![0.0, 0.0]);
        assert_eq!(embedder.len(), 1);
    }
}
