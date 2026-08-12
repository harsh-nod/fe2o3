# fe2o3-protected-publisher

Opt-in, loopback-only reference service for GitHub OIDC authorization and
client-compatible protected publisher receipts. Issuance uses a bounded
single worker and an fd-bound, checksummed, hash-chained append-only ledger.
A CSPRNG client request key is durably bound to the canonical request digest
and stable authorization projection, so a fresh valid token can recover an
already committed receipt after the original JWT expires.

Enrollment accepts a token only through a non-regular inherited descriptor,
never a token pathname, argv value, or environment value. Unknown JWKS keys
share singleflight refresh, a bounded negative cache, and issuer-wide refresh
backoff. Configuration, enrollment, key, and ledger authority is
descriptor-checked through retained owner-only directories.

This crate is inert by default. It is not deployed, production-ready, or an
acceptance/parity claim. See `docs/protected-publisher-service-v1.md` for the
wire contract, bounds, ledger format, recovery behavior, limitations, tests,
and external controls still required.
