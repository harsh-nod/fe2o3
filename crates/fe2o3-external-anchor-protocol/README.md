# fe2o3 external anchor protocol

`fe2o3-external-anchor-protocol` is a bounded, inert protocol foundation for a
future broker-owned anti-rollback hash chain.

It provides:

- fixed-width canonical V1 challenge and signed-observation messages;
- a domain-separated hash-chain head over the exact expected sequence, prior
  head, transaction digest, and anchor-key identity;
- strict Ed25519 verification against an exact caller-supplied public-key value;
- distinct advance and recovery phases; and
- move-only `Prepared -> Pending -> Commit | Abort` transitions.

A commit decision exists only after a valid signature reports the exact proposed
sequence and head. A valid signature reporting the exact prior sequence and head
produces an abort decision. Sequence gaps, regressions, overflow, stale challenge
fields, phase confusion, noncanonical encodings, and trailing bytes are rejected.

## Authority boundary

`AUTHORITY=none`.

The crate does **not**:

- establish public-key provenance;
- generate, remember across crashes, or prove freshness of nonces;
- authenticate or implement a transport;
- persist either local state or an external anchor;
- implement a monotonic service or hardware counter;
- make preparation, anchoring, or publication atomic;
- publish, link, load, or launch an artifact; or
- integrate with the protected broker service.

The caller must supply a cryptographically fresh 256-bit nonce for every
challenge. A pending transition is single use and rejects a signed response from
another nonce or phase. Deliberate nonce reuse after process loss is outside this
in-memory foundation and requires a durable external freshness mechanism.

V1 admits only the exact prior or exact proposed anchor position. An observation
of an unrelated or later position fails closed; reconciliation across multiple
concurrent writers, skipped local records, or key rotation is not modeled.

`recover_from_local_state` checks internal hash-chain consistency only. It does
not attest where the supplied fields came from. The commit and abort values are
signed protocol observations, not publication authority.
