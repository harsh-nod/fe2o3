# Protected Compiler-Execution Issuer Durable State V2

Status: implemented local issuer state, exact subject/current-publication join,
publication-bound ACK journal, cross-journal Worker rollback commit, and bounded
service transport over an admitted connection; production deployment and Worker
verification authority remain open.

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

The crate-private `ProtectedCompilerExecutionOccurrenceV1::observe_current` is
the only production constructor for an occurrence. The durable issuer invokes
it only with its own retained service admission; callers cannot select or
substitute an occurrence. The constructor first retains and fully
revalidates one exact remote rustc process observation. It derives the producer
and managed build attempt from the sealed invocation, recovers only the
production-slot V3 publication through the retained artifact-directory
description, acquires its move-only currentness lease, and reconstructs the
canonical subject under the publication lock. The subject's attempt, invocation
digest, and complete compiler closure must match the remote observation. The
occurrence retains both custody values and repeats the complete join whenever
the issuer prepares or issues. Issuer validation returns a private guard that
keeps the cooperative publication lock held through request comparison,
signing, and durable journal commit. It exposes neither a descriptor nor
signing, publication, load, or launch authority.

The journal also signs the live occurrence identity in every active stage.
`Prepared -> Issued` requires exact identity equality in addition to the
canonical subject, so restart cannot complete an earlier challenge with a
subject-equivalent replacement process. `Ready` requires the field to be zero.
The bounded transport must carry the same occurrence/session binding without
accepting a caller-provided substitute.

## Canonical Record

The journal is one fixed 2,788-byte record. Integers are little-endian and all
reserved or absent fields are zero.

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | magic `F2O3CEJ2` |
| 8 | 2 | version `2` |
| 10 | 2 | reserved |
| 12 | 8 | total byte length `2788` |
| 20 | 4 | reserved |
| 24 | 1 | stage: `Ready=1`, `Prepared=2`, `Issued=3` |
| 25 | 7 | stage padding |
| 32 | 32 | caller-pinned issuer-policy identity |
| 64 | 8 | nonzero sequence |
| 72 | 32 | prior rollback anchor |
| 104 | 32 | last acknowledged receipt identity |
| 136 | 288 | complete last publication ACK claim or zero |
| 424 | 32 | exact live compiler-occurrence identity or zero |
| 456 | 690 | canonical compiler-execution subject or zero |
| 1146 | 200 | canonical issuer challenge or zero |
| 1346 | 946 | canonical attestation request or zero |
| 2292 | 400 | canonical signed receipt or zero |
| 2692 | 64 | Ed25519 journal signature |
| 2756 | 32 | domain-separated journal identity |

The signature covers bytes `0..2692` under
`FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-SIGNATURE/V2`. The terminal identity
covers bytes `0..2756` under
`FE2O3/COMPILER-EXECUTION-ISSUER-DURABLE-IDENTITY/V2`. Both digests include the
domain, an explicit input length, and the complete input bytes.

Decoding verifies the journal signature before interpreting semantic fields.
It then strictly decodes every nested protocol record, rechecks the policy,
subject, challenge, request, receipt, sequence, and rollback relationships, and
requires byte-for-byte canonical encoding.

## State Machine

Only these transitions are legal:

1. `Ready(N, A) -> Prepared(N, A, occurrence, subject, challenge)`
2. `Prepared(N, A, occurrence, subject, challenge) -> Issued(N, A, occurrence, subject, challenge, request, receipt)`
3. `Issued(N, A, receipt) -> Ready(N+1, receipt.next_anchor, complete_ack)`

Genesis is `Ready(1, zero, no_ack)`. Every later state retains the complete ACK
from the preceding sequence. Its policy, receipt, sequence, and current anchor
must match the journal's last receipt and current rollback position. Prepared
and issued records retain that ACK together with the same sequence, anchor,
subject, and challenge. An issued record reconstructs its exact 584-byte
sidecar from its own signed journal and occurrence identities. ACK must match
every sidecar field before the transition can be signed. Repeating only the
same complete ACK returns `AlreadyAcknowledged`; a same-receipt ACK naming a
different publication or Worker record rejects. Sequence overflow fails
closed.

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
A recovered issued state reconstructs and re-emits the exact receipt sidecar.
Presence of either historical V1 journal name fails before V2 genesis. V1
advanced state lacks a publication-bound ACK and therefore requires an explicit
offline migration policy rather than an implicit reset.

## Security Limit

The journal provides integrity, state-machine ordering, crash consistency, and
single-live-writer exclusion. It does not provide local anti-rollback against
an actor that can replace the entire service-owned directory with an older
valid signed snapshot. The
[Worker ledger](compiler-execution-worker-ledger-v1.md) now maintains its own
current rollback anchor, verifies the receipt against that anchor, durably
advances it, and independently reacquires the exact durable record named by the
canonical [publication ACK](compiler-execution-receipt-publication-v1.md). Raw
ACK bytes cannot construct the move-only committed-publication token accepted
by the issuer. Lossless Worker V3 custody and production verifier authority are
still required before `CompilerExecutionProvenance` can close.

## Shared Process Validation

`fe2o3-process-identity` owns the one semantic comparison used for a protected
rustc occurrence. It checks the canonical V3 descriptor against independently
supplied observations of:

- the exact ordered argument vector;
- the canonical working directory;
- the complete compile environment;
- the supported AMD target and required backend and artifact-directory paths;
- the descriptor-to-closure rustc and backend pins;
- the measured running rustc and backend bytes; and
- the closed compiler-closure and backend environment observations.

The in-process backend now calls this shared validator instead of maintaining a
second implementation. The validator accepts only inert observations and
returns only success or a typed mismatch. Backend self-observation cannot
construct an occurrence because it has neither protected-service admission nor
the independently retained current-publication lease.

The protected service now gathers the same observations from the admitted live
rustc process. It retains the exact procfs process directory; duplicates and
validates the sealed invocation, backend, and artifact-directory descriptors
through the admitted pidfd; hashes the retained rustc and backend objects; and
revalidates process continuity and every retained object. The observation is
move-only and exposes no descriptor. The occurrence constructor uses its
private artifact-directory description only to reacquire exact V3 currentness;
the resulting lease remains private and is checked under lock together with a
second full process revalidation. A production service under a distinct UID
still needs a narrowly scoped launch policy that permits these ptrace-governed
inspections. Reusing the validator prevents the backend and issuer from
assigning different meaning to the same V3 descriptor without treating backend
self-observation as protected evidence.

## Qualification

The package suite checks all three stage encodings, truncation and extension,
wrong key and policy, request/subject/occurrence substitutions, singleton
exclusion, exact challenge and receipt re-emission, idempotent acknowledgment,
and mutation of every one of the 2,788 issued and acknowledged record bytes.
It also rejects legacy V1 journal presence and same-receipt/different-Worker
ACK replay. Deterministic fault
injection exercises both sides of all seven retained-directory boundaries for
genesis and each of the three legal transitions. Recovery must produce only
the exact prior record or exact legal successor.

This evidence qualifies the local durable issuer mechanism, remote observation,
and current-publication subject join. Live tests cover exact construction,
repeat revalidation, process exit, superseded attempt, wrong producer, changed
invocation, non-compile input, and missing or malformed managed attempts.
The issuer constructs every occurrence from its own admission, and a guard
contention test proves a superseding attempt cannot advance until issuer
currentness custody is explicitly dropped. The bounded `SOCK_SEQPACKET` service
and exact replay path are implemented. Production distinct-UID launch and
inspection policy, Worker V3 load-envelope/verification carriage, external
monotonic rollback anchoring, and the MI300X Cargo-to-KFD run remain required.
