# Context Async Submission Writer Journal V1

Status: development extension of the opt-in V6 Context journal, not completion
of V5/V6, A1/A2, #182 or HIP/HSA parity. Accepted checkpoints remain Native R125
CPU/test, Admission R118B C1-C3 and Resources R116/V3. Qualification and its limits
are recorded in [the async writer archive](evidence/dev-v6-async-writers-2026-09-17/README.md).

## Ownership

Ordinary typed launches, snapshots, atomic/collective launches and prepared graph
launches use one submission helper. Only original Context allocation IDs with
Write or ReadWrite access enter the writer roster. Same-device, prepared graph
and peer copies contribute only their original destination allocation ID.
Preparation sorts and deduplicates that roster, revalidates its live journal
records immediately before issue, and covers each complete allocation, including
partial-region writes. Read-only and empty rosters consume no writer slot.

The helper reserves all three insertion indexes before minting the original
Context submission ID. Hash-table insertion capacity is rechecked after prior
removals; constructor-time reservation alone is insufficient. The exact same
submission ID registers the model Submission writer, without a second ID or a
backend-handle-derived identity. Its allocation roster is rooted independently
of the public token and backend submission metadata before backend entry.

An installed submission also holds its exact expected writer reference. Missing,
foreign or substituted roots fail closed before publishing completion or calling
callbacks. A valid returned backend handle stays Pending. Even an invalid zero
or duplicate handle is rooted before protocol rejection seals Context.

## Settlement

| Observation | Writer disposition |
| --- | --- |
| Initial submit Rejected | NoEffect; burned attempt epoch is preserved |
| Initial submit Quiescent | Unknown, including when no handle is returned |
| Initial Terminal, panic, zero or duplicate handle | Unknown; seal Context and quarantine retained writer allocation credits |
| Poll/wait/drain Pending or Rejected | Keep Pending |
| Conclusive Succeeded | Success, before status or callback publication |
| Conclusive Failed, including a numeric cancellation code | Unknown |
| Explicit cancellation-before-publication result | NoEffect |
| TooLate or rejected cancellation | Keep Pending |
| Quiescence without a result, including successful stream destruction | Unknown |
| Flush, metadata release, callback return or future drop | No independent settlement evidence |

Repeated terminal observations do not settle twice. A later stream destroy cannot
downgrade an already successful writer. Unknown roots survive release of their
submission metadata. The subsequent [Unknown disposal extension](runtime-context-version-journal-disposal-v1.md)
retains per-member allocation-owner disposal receipts and retires the complete
ordinary Submission roster only after every original destination is disposed.
Its qualification is separate from the original async writer archive above.

Terminal failures and caught backend panics quarantine all retained ordinary
submission writers, including when the failing call concerns a different
allocation, event, stream or module. Individual secondary bookkeeping failures
must not replace the original boxed error or unwind payload. Journal-backed
ordinary operations catch mutable backend entry; immutable capability queries
retain their existing observational contract. Owned shutdown and generated
operations retain their separate enclosing failure boundaries. Non-journal panic behavior is
unchanged. All ordinary submissions now reserve facade insertion headroom before
backend entry, which can report Capacity earlier than the previous path.

The direct-KFD first-progress boundary now promotes a Rejected error to Terminal
when logical compute custody has already been accepted. It preserves the owned
detail string, changes the stable failure kind, and retains the pending launch,
dependency/module/allocation custody, stream indexes and completion reservation.
It does not change definite pre-admission rejection into terminal failure.

## Cost And Bounds

For b writable bindings and k distinct destination allocations, preparation sorts
in O(b log b), then builds the exact O(k) roster. Successful Begin/settlement
traverses that writer's members rather than every allocation. Hash indexes use
expected amortized lookup/insertion costs; rehashing is not a hard constant-time
guarantee. Terminal quarantine visits all retained writer roots and their members.
All root/member storage is acquired before backend entry; roots are count-bounded
by the configured writer capacity and their disjoint members by allocation
capacity. This is not an aggregate byte-budget or throughput qualification.

## Remaining Boundaries

SPI Rejected, completion and cancellation classifications remain explicit backend
contracts, not authenticated machine-refinement evidence. The existing V4-J1
issuance proof does not prove this production settlement integration.

Generated protected execution is deliberately not enrolled by this helper. It
requires its own effect roster, closing currentness, checked readback, native
retirement and production verifier/refinement evidence before journal settlement.
Existing generated transitions retain their distinct completion path.

Backend aliases, input/read leases, ordered overlapping writers, cross-run version
consumption, content recovery and aggregate native-residency disposal remain open.
Pending or Unknown membership currently rejects a later writer even if an event
dependency or stream order would otherwise serialize it. No public lineage or
reuse authority is exposed by this development profile. Native GPU campaigns,
new solver proofs and matched HIP/HSA performance measurements are not supplied
by its CPU qualification.
