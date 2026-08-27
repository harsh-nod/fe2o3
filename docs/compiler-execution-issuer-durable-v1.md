# Protected Compiler-Execution Issuer Durable State V1

Status: implemented local issuer state; supervised producer and Worker V3 join remain open.

This contract consumes
[`ProtectedCompilerExecutionIssuerAdmissionV1`](compiler-execution-issuer-admission-v1.md)
and the canonical
[`compiler-execution attestation V1`](compiler-execution-attestation-v1.md)
records. It adds one singleton, signed, crash-safe state machine without creating
compiler, publication, load, or launch authority.

## Ownership Boundary

The admitted issuer retains all of the following for its complete lifetime:

- the hardened process and measured static executable;
- the immutable service-owned Ed25519 signing key;
- the descriptor-only service-owned `0700` durability root;
- a separately opened `FD_CLOEXEC` directory description holding a nonblocking
  exclusive `flock`; and
- exactly one decoded canonical journal record.

The lock description is not a duplicate of the retained root's open-file
description. Its device and inode must match the retained root before the lock
is accepted. A second live issuer for the same root therefore fails before it
can inspect or mutate state.

No public API constructs a
`ProtectedCompilerExecutionOccurrenceV1`. The future supervised compiler
adapter must independently reconstruct the exact canonical subject and create
that move-only token inside the authority-service crate. Challenge preparation
and receipt issuance are unreachable without such a token.

## Canonical Record

The journal is one fixed 2,468-byte record. Integers are little-endian and all
reserved or absent fields are zero.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | magic `F2O3CEJ1` |
| 8 | 2 | version `1` |
| 10 | 2 | reserved |
| 12 | 8 | total byte length `2468` |
| 20 | 4 | reserved |
| 24 | 1 | stage: `Ready=1`, `Prepared=2`, `Issued=3` |
| 25 | 7 | stage padding |
| 32 | 32 | caller-pinned issuer-policy identity |
| 64 | 8 | nonzero sequence |
| 72 | 32 | prior rollback anchor |
| 104 | 32 | last acknowledged receipt identity |
| 136 | 690 | canonical compiler-execution subject or zero |
| 826 | 200 | canonical issuer challenge or zero |
| 1026 | 946 | canonical attestation request or zero |
| 1972 | 400 | canonical signed receipt or zero |
| 2372 | 64 | Ed25519 journal signature |
| 2436 | 32 | domain-separated journal identity |

The signature covers bytes `0..2372` under
`FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-SIGNATURE/V1`. The terminal identity
covers bytes `0..2436` under
`FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-IDENTITY/V1`. Both digests include the
domain, an explicit input length, and the complete input bytes.

Decoding verifies the journal signature before interpreting semantic fields.
It then strictly decodes every nested protocol record, rechecks the policy,
subject, challenge, request, receipt, sequence, and rollback relationships, and
requires byte-for-byte canonical encoding.

## State Machine

Only these transitions are legal:

1. `Ready(N, A) -> Prepared(N, A, subject, challenge)`
2. `Prepared(N, A, subject, challenge) -> Issued(N, A, subject, challenge, request, receipt)`
3. `Issued(N, A, receipt) -> Ready(N+1, receipt.next_anchor)`

Genesis is `Ready(1, zero)`. Later ready states require nonzero prior and
last-receipt anchors. Prepared and issued records retain the same sequence,
anchor, subject, and challenge. Acknowledgment advances exactly once; repeating
the last receipt identity returns `AlreadyAcknowledged` without another write.
Sequence overflow fails closed.

Issuer continuity is checked before signing, immediately after the signed next
state is formed, and after durable commit. The challenge or receipt wrapper is
returned only after the complete corresponding record is durable. A failed
commit poisons the live ledger, so only restart recovery may determine whether
the prior or successor state became durable.

## Durable Commit And Recovery

Every transition uses the retained-directory protocol:

1. create a private temporary file relative to the retained root;
2. write and sync the complete record;
3. rename it without replacement to the redo name and sync the directory;
4. compare the canonical and redo bytes observed during the operation;
5. rename redo to canonical and sync the directory; and
6. revalidate the retained directory identity and security metadata.

Recovery accepts only one of these shapes:

- no canonical and no redo: create signed genesis;
- canonical only: decode it, then reestablish its durability with a complete
  rename-and-sync cycle;
- redo only: accept only exact signed genesis and promote it; or
- canonical plus redo: decode both and promote only an immediate legal
  successor.

Any malformed, unsigned, wrong-policy, noncanonical, stale/non-successor redo,
symlink, hard link, wrong owner, wrong mode, wrong size, or changed directory
identity fails closed. A recovered prepared state re-emits the exact challenge.
A recovered issued state re-emits the exact receipt.

## Security Limit

The journal provides integrity, state-machine ordering, crash consistency, and
single-live-writer exclusion. It does not provide local anti-rollback against
an actor that can replace the entire service-owned directory with an older
valid signed snapshot. Worker V3 must maintain its own protected current
rollback anchor, verify the receipt against that anchor, durably advance it,
and only then acknowledge the issuer receipt. This independent join is required
before `CompilerExecutionProvenance` can close.

## Qualification

The package suite checks all three stage encodings, truncation and extension,
wrong key and policy, request/subject/occurrence substitutions, singleton
exclusion, exact challenge and receipt re-emission, idempotent acknowledgment,
and mutation of every one of the 2,468 issued-record bytes. Deterministic fault
injection exercises both sides of all seven retained-directory boundaries for
genesis and each of the three legal transitions. Recovery must produce only
the exact prior record or exact legal successor.

This evidence qualifies the local durable issuer mechanism only. The protected
supervision adapter, bounded `SOCK_SEQPACKET` service protocol, Worker V3 wire
carriage, verifier rollback ledger, and MI300X Cargo-to-KFD run remain required.
