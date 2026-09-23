# Native Compiler Execution Subject V2

## Status

`fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2` binds the exact
native V4 compiler handoff and its durable occurrence. It is an additive,
resource-metered content codec, not a protected execution receipt. The normal
compiler, issuer, Cargo, Worker, runtime and generated-host path has not switched
to this version. This library increment completes no #272 milestone and promotes
no tutorial kernel to production or hardware coverage.

The existing [subject V1](compiler-execution-subject-v1.md) remains unchanged
at its public and wire boundaries. Both versions use one private fixed codec;
V2 does not accept, decode or fall back to a V1 subject or V3 handoff.

## Exact Binding

The subject retains these coordinates, not the large payloads themselves:

1. Durable build attempt, nominal V4 slot and transaction identity.
2. Cached digest of the complete canonical rustc invocation V3. This is distinct
   from the build attempt's source/configuration invocation identity.
3. All six compiler-closure pins, transition protocol and closure identity.
4. Seven ordered identity/length pairs: inventory receipt, preflight receipt,
   complete V4 capsule, final module commitment, module handoff V2, pair binding
   V4, and outer handoff V4.

The capsule binding includes its mandatory source/refined-forwarding carrier,
not merely `capsule.base()`. Two capsules with identical base bytes but different
carriers therefore produce different subjects. No second executable graph or
editable verification program is introduced.

`from_publication` checks the receipt against the immutable decoded handoff.
`from_consumed` accepts the raw consumed V4 transport. For a concrete verifier
owner, use `from_publication(consumed.receipt(), owner.handoff(), budget)` while
retaining its prepaid storage. Arbitrary user-defined `AsRef` callbacks are not
part of this fixed-cost codec.

`from_replay_evidence` binds caller-supplied occurrence coordinates without
authenticating that such an execution occurred. Standalone `decode` checks
framing and internal identities, not agreement between supplied bindings and a
real payload. A later protected consumer must independently reconstruct the
subject from the exact handoff and compare it before admitting an attestation.

## Wire Contract

All integers are little endian. The complete wire is exactly 690 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | Magic `F2O3CES2` |
| 8 | 2 | Version `2` |
| 10 | 2 | Zero flags |
| 12 | 8 | Total length `690` |
| 20 | 4 | Zero reserved word |
| 24 | 56 | Build attempt |
| 80 | 8 | V4 slot plus seven zero reserved bytes |
| 88 | 32 | V4 transaction identity |
| 120 | 32 | Canonical rustc invocation V3 digest |
| 152 | 226 | Complete compiler closure |
| 378 | 280 | Seven ordered identity/length bindings |
| 658 | 32 | Terminal subject identity |

The subject identity is SHA-256 over
`FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V2\0`, `u64_le(658)` and the exact prefix.
The closure encoding and identity rules are unchanged. Decoding rejects mixed
versions, unknown slots/flags, nonzero reservations, zero generation, zero
required identities/lengths, invalid closures, stale hashes and noncanonical
bytes. DIRECT session/invocation coordinates retain the existing attempt rules.
Shared framing errors reuse the V1 diagnostic type; this is not V1 admission.

## Resource Contract

Every public construction, decode and identity check uses the caller's existing
work/storage ledger, with the existing V4 256 MiB storage ceiling. Construction
uses cached immutable identities; it never reserializes the invocation or
rehashes the capsule or module. Construction and decoding allocate no heap
storage. The subject is move-only; another instance requires metered admission.

Keep the complete borrowed input owner prepaid, including enclosing backing
capacity, prefix/suffix and spare capacity, plus decoded metadata. The codec
checks a minimum floor; that check does not prove that the caller accounted for
every other retained object. Exact-size wire input has a 690-byte floor; other
lengths fail after fixed prepayment without traversing the input.

The exported `INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2` prepays fixed codec work.
`INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2` covers peak logical additional
storage for retained results, staging, hashes and control/error scratch. These
are logical policy bounds, not machine instruction, allocator or RSS bounds.
Construction returns `(subject, storage)` with admitted but unreserved retained
storage: reserve `storage.retained_storage()` before retaining or using it.
Scoped cleanup restores the entry storage floor without resetting accepted
work, peak storage, first denials or ledger identity.

## Verification And Remaining Work

Tests freeze independent V1/V2 golden identities, mutate every wire byte, reject
independently resealed malformed records, preserve V1 error precedence, and
check cross-version nominal separation. Resource tests cover exact and one-short
budgets, inherited denial history, full backing capacity, input release, and
zero allocations even with a large invocation. Publication, recovery, raw
consumption and the concrete verifier-mapped owner reconstruct the same subject.
The verifier fixtures cover both source routes and gfx942/gfx950 with public
test keys; they establish content agreement, not protected origin.

Required next integration is an explicit V2 receipt-transport and protected
policy/attestation/carriage family, broker occurrence reconstruction, producer
and Cargo admission before consumption, and matching Worker/runtime/host
consumers. Semantic replay and canonical subject equality cannot replace issuer
authentication, machine refinement or target-matched end-to-end qualification.

See [production convergence](production-pipeline-convergence-v1.md) and
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
