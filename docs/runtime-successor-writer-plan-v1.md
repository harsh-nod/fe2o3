# Successor Writer Integration Plan

Open implementation plan, not a supported API, accepted proof, or parity claim.
Native full-output wait eligibility is only a prerequisite. Context still rejects
a writable destination with a pending predecessor, including an exact event.

## Ownership Boundary

Add a bounded queued-writer owner around `ContextProducerReadJournalV1`, following
the existing immutable-only owner layering. Do not replace or bypass the inner
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
   descendants remain success-dependent on the cancelled identity. Specify and
   test retained failed-node bookkeeping or a pre-effect TooLate response before
   choosing a concrete cancellation representation.
6. Terminal backend results, unknown ownership and unwind retain all affected
   allocation credits, writer nodes and dependency roots. Cleanup/drop and the
   Context-wide quarantine path must account for queued as well as active writers.

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
