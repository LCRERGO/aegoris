# One OpenAI-compatible LLM adapter

All model access goes through a single `LanguageModel` trait and one
OpenAI-compatible HTTP adapter, with a configurable `base_url` and `model`.

One adapter covers OpenAI, OpenRouter, Ollama, LM Studio, and DeepSeek (the
default: `https://api.deepseek.com`, model `deepseek-flash`). Native JSON mode is
used opportunistically when the backend advertises support, falling back to
prompt-for-JSON plus a bounded repair loop otherwise.

The alternative was a bespoke adapter per provider. That multiplies the
surface area for no benefit, since every target provider speaks the same
chat-completions dialect. The trade-off is that provider-specific features
beyond chat completions (for example, embeddings) are out of scope for this
adapter.
