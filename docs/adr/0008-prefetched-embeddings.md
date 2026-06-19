# Embeddings are prefetched, then matched synchronously

The semantic stage of relevance matching is backed by an OpenAI-compatible
`/embeddings` endpoint (`OpenAiCompatEmbedder` in `aegoris-llm`). The CLI
prefetches a vector for every fact and requirement once, then hands the
deterministic core a synchronous `CachedEmbedder` lookup table.

This preserves the core's "no I/O, no async" invariant: the embedding model is
an adapter at the edge, and the matcher stays a pure function over vectors.

The alternative was to make the `Embedder` trait async, which would have pushed
async into every matcher and broken offline testing of the deterministic path.
The trade-off is one upfront batch request in `--embeddings` mode; DeepSeek has
no embeddings endpoint, so this stage is opt-in and defaults to an
OpenAI-compatible URL.
