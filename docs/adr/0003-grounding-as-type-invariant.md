# Grounding is a type invariant, not a prompt request

A `Claim` has private fields and can only be constructed with at least one
`Evidence` entry pointing at a `Fact`. Unevidenced model output fails to
deserialize, so it can never reach a rendered artifact.

This is backed by two layers: the structural invariant above, and an optional
semantic verifier that checks whether the cited facts actually entail the
claim. The default policy is strict: unsupported claims abort the run rather
than being silently dropped.

The alternative was to ask the model not to hallucinate and trust the result.
For a resume, a fabricated metric is worse than a failed run, so the guarantee
was made mechanical. The trade-off is that the verifier costs an extra model
call in `llm` mode; in `template` mode it is a provable no-op.
