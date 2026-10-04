# Native Compiler Execution Receipt V2

## Status And Authority

`CompilerExecutionAttestationReceiptV2` signs an exact
[native request](compiler-execution-request-v2.md) under a caller-pinned
[PolicyV2](compiler-execution-trust-inputs-v2.md). Both the receipt and its
verified wrapper are move-only. Shared private encoding/signature mechanics
preserve the V1 wire, getters, cloning and validation order. Public families
remain nominally distinct; neither wire length nor policy generation selects a
version.

Decoding verifies a signature under the embedded key. Consuming `verify` also
checks the supplied policy, request and current rollback input. Neither operation
proves protected key custody, compiler execution, freshness or durable anchor
advancement. The verified wrapper authenticates the pinned signing key but grants
no compiler, load or launch authority. `into_receipt` is a storage-neutral
downgrade, not an authority conversion.

This protocol increment activates no producer or service and qualifies no
simulator execution, protected proof or GPU run. M0-M7 and 47/47 remain open.

## Wire And Transcripts

The receipt is exactly 400 bytes, with little-endian integers:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 24 | `F2O3CER2`, version 2, zero flags, length 400, zero reserved |
| 24 | 32 | RequestV2 identity |
| 56 | 8 | Request length 946 |
| 64 | 32 | PolicyV2 identity |
| 96 | 32 | SubjectV2 identity |
| 128 | 8 | Subject length 690 |
| 136 | 32 | ChallengeV2 identity |
| 168 | 32 | Challenge nonce |
| 200 | 8 | Rollback sequence |
| 208 | 32 | Prior rollback anchor |
| 240 | 32 | Next rollback anchor |
| 272 | 32 | Ed25519 verifying key |
| 304 | 64 | Signature |
| 368 | 32 | Receipt identity |

Ordinary Ed25519 signs the 32-byte SHA-256 digest of
`signature_domain || u64_le(304) || wire[..304]`. This is not Ed25519ph.
The receipt identity hashes `identity_domain || u64_le(368) || wire[..368]`.
The next anchor hashes
`rollback_domain || sequence_le || prior || request_identity || subject_identity
|| subject_length_le || nonce || policy_identity`, without a length prefix.

The respective domains are
`FE2O3/COMPILER-EXECUTION-RECEIPT-SIGNATURE/V2\0`,
`FE2O3/COMPILER-EXECUTION-RECEIPT/V2\0`, and
`FE2O3/COMPILER-EXECUTION-ROLLBACK-ANCHOR/V2\0`. They are independent of V1.

Decode checks framing, request binding/length, subject binding/length, key,
rollback position/transition, signature, then terminal identity/canonical bytes.
It deliberately retains V1's acceptance of correctly signed zero policy,
challenge or nonce fields; contextual verification rejects those records.
Consuming verification authenticates the signature before constructing expected
fields. Its comparison order is policy/key, subject, sequence, challenge/nonce,
prior/current anchor, next anchor, then request binding. A substituted request
digest therefore fails the derived next-anchor comparison before the final
defensive request comparison.

## Resources And Ownership

All four operations prepay work and fixed scratch on the caller's ledger before
cryptography. No child meter or Subject decoder is introduced. Exported names
use the `COMPILER_EXECUTION_ATTESTATION_RECEIPT_` prefix:

| Operation | Work Constant | Logical Units |
|---|---|---:|
| Issue | `ISSUE_WORK_V2` | 217608 |
| Decode | `DECODE_WORK_V2` | 152072 |
| Verify | `VERIFY_WORK_V2` | 147976 |
| Identity match | `IDENTITY_WORK_V2` | 12808 |

The weights include entry 8, fixed byte allowance 12800, key validation 4096,
signing 65536 and strict verification 131072. Issue includes two key validations,
one sign and one self-verification; decode includes two key validations and one
verification; consuming verify includes one of each. Internal cryptographic work
is part of the primitive weights. These are logical admission quotas, not CPU
instruction, latency, generated-stack, allocator or RSS bounds.

`STORAGE_V2` covers fixed result/staging owners, canonical arrays, hash/public
crypto state and control scratch. Layout assertions cover owner tuples,
result/unwind wrappers and storage-neutral downgrades. Full enclosing input
owners remain the caller's responsibility; minimum-floor checks cannot prove
complete caller accounting.

Issue's minimum floor is policy retention + request retention + the full
`size_of::<SigningKey>()`, not its serialized seed/keypair size. Decode and
identity matching require at least 400 prepaid bytes for exact-length input.
Wrong-length inputs reject without traversal, after fixed admission.
Issue/decode return the full unreserved receipt charge. Reserve the returned
`additional_storage()` before retaining the owner.

Consuming verification requires receipt + policy + request retention. Preserve
the receipt's reservation and reserve only the returned wrapper delta, currently
zero; `retained_storage()` reports the full owner charge. On refusal, the consumed
receipt drops but its reservation remains for caller retirement. Scope cleanup
restores entry storage on success, error and unwind without refunding accepted
work or clearing peak/denial history.

## Validation And Integration

Tests freeze independently calculated signatures, identities and wire hashes,
including a second nonzero-prior transition. They cover all-byte mutation,
cross-family domains, strict-signature negatives, adjacent decode and contextual
error order, policy-axis and valid alternate-request substitution, both sides of
the prior-anchor comparison, exact/one-short quotas, full-key floors, inherited
storage, denial history and nominal move-only compile-fail contracts.

[Native publication/ACK/carriage](compiler-execution-publication-v2.md) is now
implemented. Next are versioned durable/service records and production consumers.
Broker reconstruction must observe the actual V4 transaction independently.
Worker replay, runtime/host joins, both Cargo restart paths and coherent
provisioning must be integrated before producer activation. A signed receipt
alone cannot substitute for any of those steps.
