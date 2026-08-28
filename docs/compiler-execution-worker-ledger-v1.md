# Protected Compiler-Execution Worker Ledger V1, Durable Record V2

## Status

This document fixes the implemented descriptor-relative Worker rollback ledger
for protected compiler-execution receipts. It is one component of the existing
Worker V3 pipeline, not an alternate compiler or runtime route. Bounded service
transport and an exact-current carriage verification operation over an admitted
connection are implemented. The ledger also implements the local crash-safe
external-anchor journal for one publication. The supervisor carries a retained
external-anchor endpoint and service pidfd through the static launch, the issuer
independently re-admits them, and the production Publish operation drives the
bounded authenticated exchange before Worker commit and ACK. Production
distinct-UID deployment, independently operated monotonic service integration,
externally anchored VerifyCurrent evidence, production verifier authority, and
the Cargo-to-KFD run remain open.

The ledger consumes the canonical
[receipt publication V1](compiler-execution-receipt-publication-v1.md) sidecar.
It retains the complete request because restart must re-run signature, subject,
challenge, request, policy, sequence, and rollback verification. A digest-only
record is not accepted.

## Canonical Record

The ledger has no synthetic genesis record. Absence of both managed V2 names and
both legacy V1 names means `next_sequence=1` and a zero current rollback anchor.
The managed names are `compiler-execution-worker-v2.state` and
`compiler-execution-worker-v2.redo`. Every committed state is one fixed
2,218-byte record:

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | magic `F2O3CEW2` |
| 8 | 2 | version `2` |
| 10 | 2 | zero flags |
| 12 | 8 | total byte length `2218` |
| 20 | 4 | zero reserved bytes |
| 24 | 32 | caller-pinned issuer-policy identity |
| 56 | 8 | nonzero receipt and rollback sequence |
| 64 | 32 | prior rollback anchor; zero only at sequence one |
| 96 | 32 | nonzero resulting current rollback anchor |
| 128 | 946 | complete canonical attestation request |
| 1074 | 584 | complete canonical receipt sidecar |
| 1658 | 528 | complete canonical signed proposed-position external-anchor receipt |
| 2186 | 32 | domain-separated Worker-record identity |

The terminal identity covers bytes `0..2186` under
`FE2O3/COMPILER-EXECUTION-WORKER-LEDGER-RECORD/V2`. It establishes exact byte
identity, not independent signature authority. Decoding also verifies the
nested Ed25519 receipt against the complete request, pinned policy, recorded
prior anchor, sequence, subject, challenge, and resulting anchor, then requires
the external receipt to be a valid proposed Advance under the separately pinned
anchor key for the exact policy, request, sidecar, transaction digest, and
sequence. Sequence one requires the zero external prior head, while every later
sequence requires a nonzero prior head. The decoder finally requires
byte-for-byte canonical re-encoding.

## Commit And Reacquisition

Only an exact next rollback transition may replace the current record. The
implementation uses the retained-directory synced-temp, durable-redo,
rename-to-canonical, and directory-sync protocol. A commit error poisons the
live ledger; only restart recovery may resolve the result.

After a successful commit, the Worker reopens the canonical name relative to
the retained directory, strictly decodes it, and compares every byte and its
terminal identity with the record it attempted to commit. Only that exact
reacquisition can form `ReacquiredWorkerReceiptRecordV2`, and only that
move-only private witness can construct the committed-publication capability
consumed by the issuer. Raw sidecar, ACK, digest, or record bytes cannot invoke
the issuer transition.

An exact retry of the latest request and sidecar reacquires the same Published
anchor journal, including its exact receipt, and performs no write. It reacquires
the same canonical record and reproduces the same ACK. A stale
receipt, sequence gap, wrong prior anchor, request substitution, same-receipt
sidecar substitution, external-receipt substitution, policy change, or
non-successor redo fails closed.

## External-Anchor Journal

The same retained directory owns
`compiler-execution-worker-anchor-v1.state` and its private redo name. Each is
the canonical 2,682-byte journal record defined by the compiler-execution
protocol. The ledger permits exactly these durable transitions:

1. no journal to a genesis `PreparedAnchor`;
2. `PreparedAnchor` to either `AnchorCommitted` for an exact signed proposed
   observation or `Aborted` for an exact signed prior observation;
3. `AnchorCommitted` to `Published` only after the complete matching Worker
   record has been committed and reacquired; and
4. `Published` to the next transaction's `PreparedAnchor`, or `Aborted` to a
   distinct replacement transaction at the same position.

The ledger generates a nonzero nonce with the kernel RNG and durably commits the
complete transaction and exact Advance challenge before returning the challenge.
An exact preparation retry reacquires and returns those same bytes without a
write. It verifies an observation under the distinct anchor key pinned in the
issuer policy and durably commits the resulting full transition receipt before
the Worker record can advance. Exact observation and publication retries are
idempotent; a changed observation, policy, transaction, stage, challenge, key,
or Worker-record identity fails closed.

The V2 Worker record atomically embeds the complete receipt copied from the
durably reacquired `AnchorCommitted` journal. Recovery requires exact receipt
equality whenever the journal and current record describe the same committed
transaction. This is stronger than comparing transaction coordinates: a second
validly signed challenge for the same transaction cannot be substituted. Once a
record is Published, its embedded receipt remains its currentness evidence while
the rolling journal advances to a successor `PreparedAnchor`, `Aborted`, or
replacement preparation. Every such successor must use the embedded receipt's
proposed head as its exact external prior head.

Worker-record commit and journal publication are intentionally separate durable
writes. Recovery accepts `AnchorCommitted` with either no matching Worker record
or the exact matching record, representing the only crash window between those
writes. `Published` requires that exact record and identity. An ACK is formed
only after the `Published` journal has itself been committed and reacquired.

These operations remain internal, but the application-facing Publish operation
now composes them with the transport described below. A fresh transaction cannot
create a Worker record until the issuer has durably prepared its challenge,
received and verified an exact signed proposed-position observation, and durably
recorded that receipt. Restart from `PreparedAnchor` re-exchanges the same
challenge; restart from `AnchorCommitted` completes locally; exact replay from
`Published` returns the same record without network traffic; and a signed prior
position leaves the transaction `Aborted` with no Worker ACK.

## External-Anchor Endpoint And Exchange

`ProtectedExternalAnchorServiceAdmissionV1` retains one supervisor-provisioned
unnamed connected Unix `SOCK_SEQPACKET` endpoint and one pidfd for its exact
peer. Admission requires `FD_CLOEXEC`, nonblocking read-write status, exact
`SO_PEERCRED` UID/GID agreement with the separately pinned service identity,
exact pidfd agreement with the peer PID, live process state, distinct endpoint
and pidfd objects, and a service UID different from the protected issuer UID.
Every operation repeats descriptor identity, socket shape, status flags,
credentials, pidfd target, process start time, and liveness checks.

`ProtectedCompilerExecutionExternalAnchorV1` binds that move-only admission to
one exact `PinnedAnchorKeyV1`. It permits one mutable in-flight exchange, uses a
fixed 30-second monotonic deadline, watches both endpoint and pidfd, sends only
the canonical 184-byte challenge, accepts no ancillary data, requires one exact
288-byte observation packet, and verifies its key identity, signature, nonce,
phase, transaction, sequence, and prior/proposed hash-chain position before
returning a canonical transition receipt. A valid exact response already queued
after local process recovery is consumed before retransmission. A second queued
response, timeout, process death, endpoint closure, partial send, truncation,
wrong key, malformed packet, or substituted challenge fails closed and poisons
the live transport. Retrying after any exchange failure requires process restart
and fresh admission of the still-pinned endpoint and pidfd.

This boundary proves retained endpoint continuity and signed response
correlation. It does not prove that the service is independently operated, that
its backend is monotonic or crash durable, that duplicate requests are
coalesced by the backend, or that its signing key has protected custody. Those
properties require the reference service/deployment profile and end-to-end
restart qualification that remain open.

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
latter binds the complete 2,218-byte reacquired record, complete carriage, and
policy-verification identity. A canonical 352-byte result carries those
identities together with every journal and rollback coordinate. This result is
descriptive wire evidence, not a move-only authority capability. Its Worker
identity transitively binds the embedded receipt, but the V1 verification wire
record does not yet carry that receipt for independent client verification; that
is the next currentness-protocol revision.

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

Presence of either `compiler-execution-worker-v1.state` or
`compiler-execution-worker-v1.redo` fails before V2 recovery or genesis. V1 does
not retain the external receipt and therefore requires an explicit offline
migration policy. A V2 Worker record without its anchor journal also fails
closed.

Anchor-journal recovery follows the same synced-temp, durable-redo,
rename-to-canonical, directory-sync, strict-decode, and exact-reacquisition
protocol. It promotes only one legal adjacent journal successor and then joins
the recovered journal to the configured policy and recovered Worker record.
An anchor journal under a substituted policy is rejected even when the Worker
ledger is still empty.

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
second signature. Its embedded receipt authenticates the exact external
observation, but the repository does not yet implement or qualify the
independently operated monotonic backend behind that endpoint. An
actor able to replace the complete service-owned directory with an older
mutually consistent issuer/Worker snapshot can roll both journals back. A
production deployment must supply and qualify the reviewed external
anti-rollback service or an equivalent monotonic facility.

The current-record verification identities are deterministic hashes of the
protected comparison inputs. The service now signs the complete record together
with a fresh caller challenge, and the client verifies that signature under the
pinned policy key. This prevents unsigned endpoint substitution and stale
response replay, but the signature does not itself prove protected key custody
or add external rollback freshness. The final verifier must join the result to
the deployed service measurement and external monotonic anchor.

This ledger proves durable publication of an authenticated receipt. It does not
by itself prove Worker V3 load-envelope custody, Verus correctness,
source-to-machine refinement, or load/launch authority. Those facts must be
joined by the production `WorkerV3VerifierV1` implementation before KFD
execution.

## Qualification

Tests cover exact layout and round trip, post-commit byte reacquisition,
write-free idempotent replay, a two-step rollback chain, stale and substituted
inputs, wrong policy, truncation and extension, mutation of every one of the
2,218 record bytes, legacy V1 rejection, invalid external genesis position,
alternate-valid-receipt substitution, retention across successor
prepare/abort/replacement, forged successor external head, valid non-successor
redo, and both sides of all seven
durable boundaries for first and successor commits. Cross-journal tests cover
all three legal crash positions and reject gaps, substituted publications, and
unrelated ACK records. Exact-current tests cover successful complete carriage
reacquisition, canonical result round trip, distinct nonzero policy/ledger
verification identities, and stale-carriage rejection after a successor commit.
Anchor-journal integration tests cover exact challenge re-emission, signed
commit and abort, observation substitution, replacement after abort, policy
substitution before a Worker record exists, restart replay, and ACK gating.
Deterministic injection covers both sides of all seven retained-record
boundaries for preparation, commit and abort observation persistence, the
post-anchor Worker-record write, and final Published-journal write. Recovery is
required to produce only the exact prior or exact legal successor at every one
of those 98 injected failures, after which retry completes exactly once.
Endpoint and transport tests additionally cover exact socket and pidfd binding,
wrong pinned credentials, same-UID production rejection, blocking or mutated
status flags, wrong pidfd target, process death, endpoint closure, timeout,
short and oversized packets, forbidden ancillary data, duplicate responses,
prequeued recovery, wrong signature/key, and nonce and phase substitution.
Issuer-composition tests cover first proposed commit, write-free published
replay, prepared and anchor-committed restart, signed-prior abort, endpoint
failure, and the absence of any Worker ACK before anchor commit.
