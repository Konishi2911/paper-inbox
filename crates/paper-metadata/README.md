# Metadata acquisition adapter

Crossref DOI and arXiv Atom lookup implementing paper-core's MetadataResolver port. The desktop uses resolve_blocking on its dedicated worker. The async port adapter offloads the same blocking transport to a dedicated thread and awaits a one-shot result without blocking its caller.

Production destinations are fixed HTTPS endpoints. Deterministic HTTP/fixture tests run offline; tests/live_contract.rs is an explicit opt-in external smoke test. Full mapping, transport limits, identity and provenance rules are in ../../docs/metadata-lookup-and-keys.md.
