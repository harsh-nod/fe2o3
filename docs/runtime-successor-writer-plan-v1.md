# Successor Writer Integration Plan

Open implementation plan, not a supported API, accepted proof, or parity claim.
Native full-output wait eligibility is only a prerequisite. Context still rejects
a writable destination with a pending predecessor, including an exact event.

## Ownership Boundary

The executable model now has `ContextQueuedWriterJournalV1`, a bounded queued-writer
owner around `ContextProducerReadJournalV1`. It deliberately exposes neither mutable
nor immutable dereferencing: inherited availability queries could bypass queue
custody. The Context adapter still uses the previous owner. Do not replace or bypass the inner
single-pending-writer invariant. Existing core proofs continue to describe the
inner journal; the new composition needs its own proof and concrete integration
qualification. An immutable projection must not expose an availability API that
silently ignores outer reservations. No mutable inner extraction is permitted.

Each queued submission retains its own registered writer, complete canonical
destination roster, exact predecessor references and authenticated dependency
roots. Allocation indexes identify queue head/tail; stale identities, cycles,
foreign devices, wrong extents and inconsistent indexes fail before effects.
Queue storage has an explicit checked bound and fallible preallocation before
backend submission. Settlement must use indexed retained members/edges, not scan
every writer or allocation. Dropping a descriptive reference never releases it.

The native N3 path has one full-allocation Write destination. The overall goal
also requires multi-destination writers, mixed ready/busy destinations, partial
writes, ReadWrite and downstream reads. A first full-overwrite implementation
must not be reported as those broader semantics. A Context writer must not be
split into separately settled immediate and queued subsets.

## Transitions

1. Authenticate the entire destination/dependency roster and reserve capacity
   before changing any ownership. A pending destination names its exact current
   writer or queue tail, not merely some reachable ancestor. Explicit-success
   Context dependencies remain distinct from native failure-neutral ordering.
2. Keep the predecessor as the inner pending writer and the successor registered
   but not begun. The outer reservation blocks reuse even if activation is delayed.
   Derive the successor's attempt epoch and prior lineage at activation, after
   predecessor settlement; never guess future lineage at enqueue time.
3. Activate a successor only after every required predecessor has actually
   reconciled successfully and every exact destination is eligible. Unknown,
   no-effect and discarded-result outcomes are not activation authority. Existing
   producer-read reservations cannot stand in for exclusive writer reservations.
4. Prevalidate settlement and promotion identities before the first effect where
   possible. If predecessor settlement commits and promotion then fails, retain
   the successor and quarantine. Describe this as a committed prefix, not rollback.
5. Cancellation or backend Rejected must release only the cancelled successor's
   complete reservation. Prevalidate the whole roster before any removal. A
   cancelled middle writer must never make a descendant inherit its predecessor:
   descendants remain success-dependent on the cancelled identity. The model
   unlinks only the cancelled member and retains its immutable predecessor identity
   in each descendant, with a Failed prerequisite. Later ancestor success cannot
   rewrite this prerequisite. Concrete backend cancellation still needs qualification.
6. Terminal backend results, unknown ownership and unwind retain all affected
   allocation credits, writer nodes and dependency roots. Cleanup/drop and the
   Context-wide quarantine path must account for queued as well as active writers.

## Model Checkpoint

The fixed member arena reserves complete destination rosters, including idle
members of a multi-parent join. Each unactivated successor consumes epoch
headroom at admission. Activation uses actual reconciled epoch/lineage values,
not predicted versions. Public proposed-key admission is borrowed and consumes
no ID; registration and Begin recheck their respective state before commitment.

The wrapper calls shared inner preflights before added queue exclusion checks,
preserving ordinary-path error precedence. It has no heap allocation after
construction; roster operations are O(k), independent of total queue depth.
Model tests exercise arena conservation, fixed storage addresses/capacities,
three-deep success, cancellation permutations, multi-parent/idle joins, readers,
stale identities, epoch exhaustion and fail-stop Unknown custody.

This is not Context support or a proof of the new owner. In particular:

- Context must authenticate each exact latest writer and explicit success event,
  retain the queued roster, and audit all availability probes before enabling it.
- Queued Unknown is fail-stop and cannot be refunded by a later NoEffect value.
  Closed-group model disposal is implemented; its Context/native integration
  remains open. It destroys allocations rather than exposing uncertain content.
- Failed/quiescent backend results alone are not NoEffect authority. Descendant
  failure after cancellation cannot be used to silently release queued custody.
- Queued-output reads, partial writes and ReadWrite integration remain open.
- The unchanged inner journal proofs do not establish outer queue invariants.

### Unknown Group Disposal

Rejecting disposal while a co-owner exists is safe but cannot finish cleanup:
an Active Unknown A and queued Unknown B on the same allocation block each other
in either disposal order. The model disposal group closes over every selected
writer's complete roster and every co-owner of that allocation union. Require
exact identities, no omitted neighbors/readers, all roots Unknown, and separately
authenticated Context quiescence before any native release.

Retain one group with unique allocation receipts. Release each native allocation
once and keep all original writer roots/credits until the entire union is disposed.
After complete backend release, retire queued-only allocations, dispose inner
Active-Unknown rosters and abort queued Reserved identities as destruction
bookkeeping, not as NoEffect or success. Clear the outer group without predecessor
success propagation. Partial model commitment must quarantine, never masquerade
as rollback. Context's current per-writer disposal receipts assume disjoint
destinations and cannot safely implement this contract unchanged.

Tests must cover branching/multi-allocation closure, unrelated writers, pending
co-owner refusal before backend effects, exact identity/reader rejection, partial
release/retry without double release, terminal/panic retention and queued-only
Unknown groups. The executable model now checks both directions of group closure,
exact canonical rosters, destination readers and selected writers' input leases
outside the group. Neighbor inspection supports bounded component discovery.

Model destruction first retires queued-only destinations, then disposes active
Unknown writers or burns queued Reserved identities, and finally clears outer
indexes without success propagation. Outer roots remain through all inner
subtransitions. A preflight failure is Rejected without mutation; any error or
unwind after starting commitment permanently makes the owner terminal. Adapters
must retain their own full native custody through that failure. Group validation
uses bounded member passes and binary searches plus an O(R) read-arena scan;
ordinary hot-path admission/settlement still avoid global scans.

Context now retains an Ordinary writer's conclusive quiescence marker separately
from removable submission metadata. Successful Unknown settlement can set it;
quarantine cannot. Both initial and final Ordinary disposal checks require it,
including absent-submission paths. Generated receipt-based validation remains
separate. Context still lacks shared-group receipts/credits and does not call the
new model group disposer. Its integration and full shared-body proofs remain open.

## Entry-Point Audit

Every operation that can read, write, retire or dispose an allocation must respect
queued reservations, including when the inner writer has already settled:

- Ordinary submission writer preparation and begin.
- Host write admission and synchronous settlement.
- Stable reader validation/acquisition and mixed input acquisition.
- Producer-bound reader validation/acquisition, including reads from later writers.
- Generated issue, reservation and reuse paths.
- Allocation retirement, unknown disposal, and cleanup/drop.
- Inspection or version projections used as admission authority.

Release and settlement of already-held predecessor resources must remain valid;
an indiscriminate guard in `validate_live` would block necessary reconciliation.
When the queue is empty, ordinary paths must preserve existing behavior and error
precedence. Differential tests against the inner owner should establish this.

## Required Qualification

- Public Context A -> B and A -> B -> C with exact event, writer, allocation,
  device and extent binding; release public events after admission.
- Independent N3 inputs with a shared output, plus the broader destination/input
  combinations before claiming their support.
- Exact activation order, attempt epochs and content lineages. Settling A must
  not erase B's reservation or publish B's version.
- Every availability entry point rejects unauthorized reuse while a successor is
  queued; existing predecessor settlement and release remain possible.
- Predecessor Success, NoEffect, Unknown and discarded-result outcomes; successor
  Rejected, cancellation, quiescent error, terminal error and panic.
- Multi-destination cancellation is all-or-none before effects; faults after an
  unavoidable committed prefix preserve the remaining custody and quarantine.
- Promotion faults after predecessor settlement; exact pending roots/credits and
  failure semantics across fanout and cancelled intermediate writers.
- Shared production-body proofs with mutation controls for each queue guard,
  identity check, activation predicate, lineage update and refund operation.
- Native Context integration and matched hardware tests. CPU model/fixture
  evidence alone does not establish GPU ordering or HIP/HSA performance parity.
