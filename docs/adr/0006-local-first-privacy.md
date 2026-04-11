# Local-first, stateless by default

aegoris reads files and writes files. It stores no candidate data, keeps no
database, and sends only the curated facts and job requirements to the
configured LLM provider when running in `llm` mode. API keys are read from the
environment or a config file and are never written or logged.

The alternative was a hosted service that retains profiles and generated
documents. Resumes are personal data, and a local CLI removes an entire class of
privacy and compliance concerns. The trade-off is that there is no
server-side history or collaboration; every run is reproducible from its
inputs plus the recorded curation plan.
