# Native Compiler Execution Trust Inputs V2

## Status

`CompilerExecutionIssuerPolicyV2` and `CompilerExecutionClientProfileV2` are
nominal, move-only, resource-metered public configuration records for the native
SubjectV2 family. They share private fixed codecs with V1; existing V1 APIs,
wire bytes, hash transcripts and diagnostic precedence remain unchanged.

These are trust inputs, not attestations or trusted provisioning. They do not
prove protected compiler execution and grant no signing, compiler, publication,
load or launch authority. Native sealed capabilities and the fixed profile-path
reader are documented in the [capability contract](compiler-execution-capabilities-v2.md).
The supervisor, serving entrypoint and producer still use V1. These inputs complete no #272 milestone
and adds no tutorial kernel or protected hardware qualification.

## Wire Contracts

All integer fields are little endian. Family selection is explicit, never by
length or policy generation. V1 and V2 records have equal respective lengths.

The issuer policy is exactly 216 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | Magic `F2O3CEP2` |
| 8 | 2 | Version `2` |
| 10 | 2 | Zero flags |
| 12 | 8 | Total length `216` |
| 20 | 4 | Zero reserved word |
| 24 | 8 | Nonzero policy generation |
| 32 | 40 | Executable digest and nonzero length |
| 72 | 40 | Runtime digest and nonzero length |
| 112 | 32 | Issuer Ed25519 public key |
| 144 | 32 | Distinct external-anchor Ed25519 public key |
| 176 | 2 | Required subject version `2` |
| 178 | 6 | Zero reserved bytes |
| 184 | 32 | Terminal identity |

Both keys must parse and be non-weak. The identity is SHA-256 over
`FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V2\0`, `u64_le(184)` and the exact prefix.
The generation is deployment configuration, not authenticated rollback state.

The client profile is exactly 280 bytes:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | Magic `F2O3CEP2` |
| 8 | 2 | Version `2` |
| 10 | 2 | Zero flags |
| 12 | 4 | Total length `280` |
| 16 | 8 | Supervisor UID and GID |
| 24 | 8 | External-anchor service UID and GID |
| 32 | 216 | Complete policy V2 |
| 248 | 32 | Terminal identity |

UIDs/GIDs reject zero and `u32::MAX`. Credential relationships beyond this
record remain deployment obligations. Unlike the policy transcript, the profile
transcript includes its domain length: SHA-256 over `u64_le(domain.len())`,
`FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V2\0`, `u64_le(248)` and the exact prefix.
Nested policy errors precede outer identity errors. Rehashing an outer V2 frame
cannot admit a nested V1 policy. Neither decoder upgrades or falls back.

## Ownership And Resources

Every public constructor, decoder and identity check takes the existing KernelIR
work/storage ledger. The exported `*_WORK_V2` and `*_STORAGE_V2` constants prepay
fixed work and additional peak logical storage. Work includes named public-key
validation quotas, byte traversal, hashing and comparison; storage includes
retained owners, duplicate staging records, hash and scalar/crypto scratch.
These are logical admission units, not measured machine instructions, generated
stack bounds, allocator/RSS limits, or cryptographic performance claims.

Keep complete borrowed inputs prepaid, including enclosing allocation capacity
and other live metadata. Exact wires have minimum floors of 216/280 bytes;
wrong lengths reject after fixed prepayment without traversing the input. Floor
checks alone do not prove complete caller accounting. The fixed callback cannot
access or replace the ledger. A profile decode prepays the whole nested policy
operation in its aggregate quota; it never creates an unlimited child budget.

Operations restore entry storage on success, error and unwind while preserving
accepted work, peak and first-denial observations. Successful construction
returns `(owner, storage)`. Reserve `storage.additional_storage()` before keeping
the result. The owner's `retained_storage()` is its complete eventual release:

- Policy construction and borrowed-wire decode return their full retained charge.
- `Profile::new` consumes a prepaid policy and returns only the additional charge.
- `Profile::decode` borrows bytes and returns the full profile charge, including
  its newly decoded policy. No nested reservation transfers from the caller.
- If consuming construction fails, its policy is dropped but the inherited
  reservation remains at entry; the caller retires that reservation.

Immutable accessors only borrow data or copy small identities. There is no public
`Clone` on either large owner; obtaining another instance requires admission.

## Validation And Next Integration

The protocol tests independently construct both wire transcripts, mutate every
V2 byte, reject mixed/resealed families and weak/equal keys, freeze diagnostic
precedence, and exercise exact/one-short work/storage, inherited reservations,
cumulative work, unwind cleanup and compile-fail nominal/clone boundaries.
Existing V1 attestation/publication and sealed-capability tests remain required.

The [SubjectV2 challenge/request](compiler-execution-request-v2.md) retains
the complete native subject through a same-ledger nested decode. Native signed
receipts, publication, acknowledgment and carriage are covered by the
[publication contract](compiler-execution-publication-v2.md). The native issuer
launch-input reader checks independently admitted policy and manifest images,
but does not activate service handlers. Remaining integration includes native
program/key custody, broker reconstruction, durable-state and service packets,
Worker/runtime/finalizer/host admission, and coherent provisioning. Consumers
must land before activating Cargo and the producer. Existing V1 state must never
be treated as absent V2 state or silently retried after V2 rejection. The complete
M1 fresh-build and both restart paths, followed by the 47-kernel matrix, remain
unqualified.
