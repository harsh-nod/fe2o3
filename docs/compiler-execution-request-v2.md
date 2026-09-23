# Native Compiler Execution Request V2

## Status

The nominal SubjectV2 binding, challenge and request extend the
[native trust inputs](compiler-execution-trust-inputs-v2.md). Public V2 owners are
move-only and metered; the shared private codecs also implement the frozen V1
challenge/request contracts. V1 retains its public types, cloning, framing,
hashes, getters, nested SubjectV1 decoder and independent subject rehash.

These records establish content agreement, not protected execution, freshness,
rollback authority, signing, compiler admission or GPU load/launch authority.
No service or producer is activated. Signed receipts, publication/ACK/carriage,
durable state, sealed capabilities and the production consumer chain still need
native integration. This increment closes no #272 milestone and qualifies no
tutorial kernel, simulator execution, protected proof or GPU run.

## Exact Wire

Integers are little endian. Both record families use a 24-byte header: eight-byte
magic, `u16` version, zero `u16` flags, `u64` total length, zero `u32` reserved.
Public adapters select the family explicitly, never by length or policy generation.

The challenge is exactly 200 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 24 | `F2O3CEC2`, version 2, length 200 |
| 24 | 32 | PolicyV2 identity |
| 56 | 32 | SubjectV2 identity, not a raw wire SHA-256 |
| 88 | 8 | Exact subject length 690 |
| 96 | 32 | Nonzero nonce |
| 128 | 8 | Nonzero rollback sequence |
| 136 | 32 | Prior rollback anchor |
| 168 | 32 | Challenge identity |

Sequence 1 requires a zero prior anchor; later sequences require a nonzero one.
Construction borrows an admitted PolicyV2 and SubjectV2. Standalone decoding
validates bindings but cannot establish that the supplied policy, subject,
nonce or rollback position came from a protected issuer.

The request is exactly 946 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 24 | `F2O3CEQ2`, version 2, length 946 |
| 24 | 200 | Complete ChallengeV2 |
| 224 | 690 | Complete SubjectV2 |
| 914 | 32 | Request identity |

Identities hash `domain || u64_le(prefix_length) || prefix`, with domains
`FE2O3/COMPILER-EXECUTION-CHALLENGE/V2\0` and
`FE2O3/COMPILER-EXECUTION-REQUEST/V2\0`. There is no domain-length prefix.
Every nested decoder is family-specific. Resealing the outer request cannot
admit a V1 challenge or subject. Request construction compares the challenge
binding against the immutable, already-admitted SubjectV2 identity; it does not
clone or reconstruct a legacy subject.

Decode diagnostic order is outer framing, challenge, subject, nonzero request
footer, subject agreement, then outer identity/canonical equality. Challenge
decoding checks its subject binding before its footer, and its footer before
policy/nonce/rollback validation. Shared extraction preserves V1's precedence.

## Resources And Ownership

Keep complete input owners prepaid, including enclosing backing allocations and
metadata. An exact wire has a minimum floor of 200/946 bytes. Wrong lengths reject
without traversal. Minimum-floor checks do not prove complete caller accounting.

Challenge construction/decode return full additional retained storage. Request
construction consumes a challenge and subject: preserve both input reservations
and reserve only the returned `additional_storage()`. On consuming failure the
owners drop but their original reservations remain for caller retirement. The
request's `retained_storage()` includes the complete decoded owners, a separate
canonical request array, and its resource descriptor.

Request decoding returns the FULL request charge because the caller supplies
borrowed bytes, not transferable decoded-owner reservations. The outer fixed
frame remains paid while exactly one real SubjectV2 decoder runs on the same
ledger. Its returned child storage is reserved before binding checks/assembly.
No unlimited child budget or unmetered Subject decoder exists. The nested subject
retains its existing 256 MiB storage-limit ceiling.

`REQUEST_WORK_V2` covers the fixed outer operation (30,280 logical units);
`REQUEST_DECODE_WORK_V2` adds the actual nested subject charge (26,184), totaling
56,464 on a valid decode. Exported constant names carry the
`COMPILER_EXECUTION_ATTESTATION_` prefix. The decode peak is the outer
`REQUEST_STORAGE_V2` plus `INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2` above the
caller floor. These are logical admission quotas, not machine instructions,
allocator behavior, generated stack or RSS measurements.

The private scope preserves work, peak and denial history and restores entry
storage on success, error and unwind. It rejects callback results if the outer
frame was partially retired, rejects underflow below caller inputs, and never
releases storage from a substituted ledger. Internal decoders must keep the frame
paid throughout execution; exit checks are not a general monitor for arbitrary
callbacks, and no public callback is exposed.

## Validation And Integration

Tests freeze independent V1/V2 golden identities and wire digests, mutate every
byte, reseal cross-family and subject-axis substitutions, check nested/footer
precedence, exercise rollback positions, exact/one-short construction/decode
budgets, nested refusal, inherited storage, ceiling enforcement, and compile-fail
mixed/cloned owners. Scope tests cover nested errors, unwind, sticky denial
history, partial frame retirement, input underflow and ledger replacement.

The next dependency is native signed receipt issuance/verification followed by
publication, acknowledgment and carriage. Broker admission must independently
reconstruct this SubjectV2 from the actual V4 transaction; accepting this client
request alone is insufficient. Durable format/service migration, Worker replay,
host currentness, both Cargo restart paths and coordinated provisioning must
precede producer activation. M0-M7 and 47/47 remain open.
