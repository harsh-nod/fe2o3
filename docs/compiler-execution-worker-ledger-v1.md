# Protected Compiler-Execution Worker Ledger V1

## Status

This document fixes the implemented descriptor-relative Worker rollback ledger
for protected compiler-execution receipts. It is one component of the existing
Worker V3 pipeline, not an alternate compiler or runtime route. Bounded service
transport and an exact-current carriage verification operation over an admitted
connection are implemented. Production distinct-UID deployment, the external
monotonic anchor, production verifier authority, and the Cargo-to-KFD run remain
open.

The ledger consumes the canonical
[receipt publication V1](compiler-execution-receipt-publication-v1.md) sidecar.
It retains the complete request because restart must re-run signature, subject,
challenge, request, policy, sequence, and rollback verification. A digest-only
record is not accepted.

## Canonical Record

The ledger has no synthetic genesis record. Absence of both managed names means
`next_sequence=1` and a zero current rollback anchor. Every committed state is
one fixed 1,690-byte record:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `F2O3CEW1` |
| 8 | 2 | version `1` |
| 10 | 2 | zero flags |
| 12 | 8 | total byte length `1690` |
| 20 | 4 | zero reserved bytes |
| 24 | 32 | caller-pinned issuer-policy identity |
| 56 | 8 | nonzero receipt and rollback sequence |
| 64 | 32 | prior rollback anchor; zero only at sequence one |
| 96 | 32 | nonzero resulting current rollback anchor |
| 128 | 946 | complete canonical attestation request |
| 1074 | 584 | complete canonical receipt sidecar |
| 1658 | 32 | domain-separated Worker-record identity |

The terminal identity covers bytes `0..1658` under
`FE2O3/COMPILER-EXECUTION-WORKER-LEDGER-RECORD/V1`. It establishes exact byte
identity, not independent signature authority. Decoding also verifies the
nested Ed25519 receipt against the complete request, pinned policy, recorded
prior anchor, sequence, subject, challenge, and resulting anchor, then requires
byte-for-byte canonical re-encoding.

## Commit And Reacquisition

Only an exact next rollback transition may replace the current record. The
implementation uses the retained-directory synced-temp, durable-redo,
rename-to-canonical, and directory-sync protocol. A commit error poisons the
live ledger; only restart recovery may resolve the result.

After a successful commit, the Worker reopens the canonical name relative to
the retained directory, strictly decodes it, and compares every byte and its
terminal identity with the record it attempted to commit. Only that exact
reacquisition can form `ReacquiredWorkerReceiptRecordV1`, and only that
move-only private witness can construct the committed-publication capability
consumed by the issuer. Raw sidecar, ACK, digest, or record bytes cannot invoke
the issuer transition.

An exact retry of the latest request and sidecar performs no write. It
reacquires the same canonical record and reproduces the same ACK. A stale
receipt, sequence gap, wrong prior anchor, request substitution, same-receipt
sidecar substitution, policy change, or non-successor redo fails closed.

## Exact-Current Verification

The protected service may supply one complete expected receipt carriage to the
ledger. The ledger first requires exact protected-policy equality, then reopens
and strictly decodes the canonical Worker record, reconstructs its complete
carriage, and compares both the typed value and every canonical byte with the
expected carriage. A stale, substituted, or merely subject-equivalent carriage
fails closed.

On success, the ledger derives separate domain-separated policy-verification
and Worker-ledger-verification identities. The former binds the protected policy
bytes, exact subject, complete carriage, and reacquired record identity. The
latter binds the complete 1,690-byte reacquired record, complete carriage, and
policy-verification identity. A canonical 352-byte result carries those
identities together with every journal and rollback coordinate. This result is
descriptive wire evidence, not a move-only authority capability.

## Recovery

Recovery accepts only:

- no canonical and no redo: empty sequence-one state;
- canonical only: strict decode followed by the full durability-reestablishment
  rename cycle and exact reacquisition;
- redo only: sequence one with zero prior anchor, promoted and reacquired; or
- canonical plus redo: one strictly decoded immediate successor, promoted and
  reacquired.

No implicit migration, truncation repair, reset, or selection of a later
sequence occurs.

## Cross-Journal Invariant

Issuer admission recovers both journals under the issuer's singleton root lock.
Exactly three positions are legal:

1. the Worker is at sequence `N-1` and the issuer is in `Ready(N)`,
   `Prepared(N)`, or `Issued(N)`;
2. the Worker has committed the exact sequence-`N` request and sidecar while
   the issuer remains in `Issued(N)`; or
3. the Worker has sequence `N` and the issuer is in `Ready(N+1)` with the exact
   ACK naming that Worker record.

Genesis is the empty Worker ledger plus issuer sequence one. Every non-genesis
prior-position join compares the complete ACK. The post-Worker/pre-issuer crash
position compares the complete request and sidecar. Any other sequence, anchor,
record identity, request, publication, or ACK relationship fails admission.

The sole public composition operation first validates this join and exact
issuer input, commits and reacquires the Worker record, creates the private
capability, advances the issuer journal, and validates the resulting join.
After a crash, replay follows the same operation and reaches the exact same
state without a second Worker transition.

## Trust Limit

The retained root is owner-only, descriptor-relative, identity-checked, and
held by the dedicated protected service. The Worker record identity is not a
second signature and same-host storage is not an external monotonic anchor. An
actor able to replace the complete service-owned directory with an older
mutually consistent issuer/Worker snapshot can roll both journals back. A
production deployment must bind the combined position to the reviewed external
anti-rollback service or an equivalent monotonic facility.

The current-record verification identities are deterministic hashes of the
protected comparison inputs. They neither authenticate an arbitrary client
connection nor add freshness beyond the reacquired local state. The caller must
authenticate the supervisor-provisioned service endpoint and join the result to
the external monotonic anchor.

This ledger proves durable publication of an authenticated receipt. It does not
by itself prove Worker V3 load-envelope custody, Verus correctness,
source-to-machine refinement, or load/launch authority. Those facts must be
joined by the production `WorkerV3VerifierV1` implementation before KFD
execution.

## Qualification

Tests cover exact layout and round trip, post-commit byte reacquisition,
write-free idempotent replay, a two-step rollback chain, stale and substituted
inputs, wrong policy, truncation and extension, mutation of every one of the
1,690 record bytes, valid non-successor redo, and both sides of all seven
durable boundaries for first and successor commits. Cross-journal tests cover
all three legal crash positions and reject gaps, substituted publications, and
unrelated ACK records. Exact-current tests cover successful complete carriage
reacquisition, canonical result round trip, distinct nonzero policy/ledger
verification identities, and stale-carriage rejection after a successor commit.
