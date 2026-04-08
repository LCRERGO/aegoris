# Template mode as a first-class fallback

Phrasing is selected by `--mode`: `llm` for model-written text, `template` for
deterministic phrasing derived mechanically from facts. `auto` (the default)
uses the LLM when an API key is available and templates otherwise.

Template mode shares the entire front half of the pipeline — parse, match,
curate, ground — and swaps only requirement extraction and phrasing. This
makes the whole product usable with no network and no key, gives the strongest
golden-test surface, and provides a fallback when a provider is down.

The alternative was to require an LLM. That would make the tool unusable
offline and would leave the deterministic half of the system untestable
end-to-end.
