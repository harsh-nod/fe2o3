# fe2o3 external anchor protocol

`fe2o3-external-anchor-protocol` is a bounded, inert protocol foundation for a
future broker-owned anti-rollback hash chain.

It provides:

- fixed-width canonical V1 challenge and signed-observation messages;
- a 528-byte canonical receipt binding one exact challenge to its strictly
  verified signed observation and explicit prior/proposed position;
- a domain-separated hash-chain head over the exact expected sequence, prior
  head, transaction digest, and anchor-key identity;
- strict Ed25519 verification against an exact caller-supplied public-key value;
- distinct advance and recovery phases; and
- move-only `Prepared -> Pending -> Commit | Abort` transitions.

A commit decision exists only after a valid signature reports the exact proposed
sequence and head. A valid signature reporting the exact prior sequence and head
produces an abort decision. Sequence gaps, regressions, overflow, stale challenge
fields, phase confusion, noncanonical encodings, and trailing bytes are rejected.
The receipt is safe to persist and replay through a caller-owned journal, but it
remains inert: the caller-pinned key does not establish key provenance, external
service deployment, or monotonic storage authority.

## Canonical transaction digest

`derive_transaction_digest_v1` gives callers one bounded, language-independent
way to turn an already-canonical transaction identity into the 32-byte field used
by the protocol. Its SHA-256 preimage is the exact concatenation:

```text
ASCII("FE2O3/EXTERNAL-MONOTONIC-ANCHOR/TRANSACTION-DIGEST/V1\0")
|| u16_le(1)
|| u32_le(canonical_identity.len())
|| canonical_identity
```

The identity must contain 1 through 4096 bytes. The crate deliberately does not
define the caller's identity schema. The caller must use a versioned canonical
encoding of stable byte digests, nonces, and sequence values and must exclude
paths, raw file descriptors, pointers, and other process-local values. This
derivation does not establish the identity's provenance or publication authority.

## V1 interoperability vectors

The frozen, language-neutral vectors are in
[`tests/vectors/external_anchor_v1.txt`](tests/vectors/external_anchor_v1.txt).
They contain exact lowercase hex for both advance and recovery challenges and
the prior and proposed signed observations for each challenge. They also freeze
the transaction digest, Ed25519 seed and public key, key identity, proposed
hash-chain head, signing messages, signatures, and complete observation wires.
The seed is public test data and must never be used as a production key.

Non-Rust implementations can reproduce the vectors with raw byte concatenation;
there is no serde, JSON, locale, path, or text formatting in a protocol preimage.
All integers are little endian. SHA-256 inputs are:

```text
key_identity = SHA256(
  ASCII("FE2O3/EXTERNAL-MONOTONIC-ANCHOR/KEY-ID/V1\0")
  || ed25519_public_key
)

proposed_head = SHA256(
  ASCII("FE2O3/EXTERNAL-MONOTONIC-ANCHOR/HASH-CHAIN-HEAD/V1\0")
  || u16_le(1)
  || u64_le(expected_sequence)
  || prior_head
  || transaction_digest
  || key_identity
)
```

The 184-byte challenge layout is:

| Offset | Length | Field |
| ---: | ---: | --- |
| 0 | 8 | `F2ARBA1\0` |
| 8 | 2 | version `u16_le(1)` |
| 10 | 1 | kind: advance `1`, recover `2` |
| 11 | 5 | zero reserved bytes |
| 16 | 32 | caller nonce |
| 48 | 8 | expected sequence, `u64_le` |
| 56 | 32 | prior head |
| 88 | 32 | transaction digest |
| 120 | 32 | proposed head |
| 152 | 32 | anchor-key identity |

The first 184 bytes of an unsigned observation use `F2ARBO1\0`; byte 11 is the
position (`1` prior, `2` proposed), while bytes 12 through 15 remain zero.
The observed sequence occupies bytes 184 through 191 and the observed head bytes
192 through 223. The external anchor signs this exact message:

```text
ASCII("FE2O3/EXTERNAL-MONOTONIC-ANCHOR/OBSERVATION/V1\0")
|| unsigned_observation[0..224]
```

The 64-byte strict Ed25519 signature is appended at bytes 224 through 287. The
integration test reconstructs all vectors through the public API and rejects a
one-byte mutation in every semantic field. The transaction-digest API adds no
fields and does not change the existing V1 challenge or observation wire bytes.

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
