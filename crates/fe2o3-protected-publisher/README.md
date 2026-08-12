# fe2o3-protected-publisher

Opt-in, loopback-only reference service for GitHub OIDC authorization,
durable replay control, and client-compatible protected publisher receipts.
It uses descriptor-validated configuration, a configuration-derived service
identity, bounded request/JWKS admission, and a capacity-limited SQLite replay
ledger with permanent tombstones.

This crate is inert by default and is not a production deployment or parity
claim. See `docs/protected-publisher-service-v1.md` for the V1 schema, bounds,
threat model, conformance commands, limitations, and deployment checklist.
