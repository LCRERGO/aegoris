# Hybrid relevance matching behind a trait

Relevance is computed by a `RelevanceScorer` trait with three additive stages:
deterministic lexical matching (always on), optional embeddings, and an
optional LLM judge for the ambiguous tail. Each stage is opt-in; the scorer
degrades gracefully to pure lexical matching.

The lexical stage is the default because it needs no network and is trivially
testable. Embeddings and the judge are traits so they can be swapped or faked
without touching the core.

The alternative was to make LLM scoring the primary path. We rejected it: it
would make curation non-deterministic, costly, and impossible to test offline,
for a step where keyword and synonym overlap already performs well.
