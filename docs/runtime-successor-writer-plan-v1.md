# Successor Writer Integration Plan

Implementation and qualification plan, not accepted formal proof or a parity
claim. The journal-enabled producer-aware Context profile now admits queued
full-overwrite destinations behind exact explicit latest-writer events. Native
backend eligibility and hardware qualification remain separate boundaries.

## Ownership Boundary

The executable model now has `ContextQueuedWriterJournalV1`, a bounded queued-writer
owner around `ContextProducerReadJournalV1`. It deliberately exposes neither mutable
nor immutable dereferencing: inherited availability queries could bypass queue
custody. Context uses this owner with bounded queued admission. The original
journal constructor supplies one member slot per allocation; the new
`open_with_version_journal_members_v1` constructor permits an explicit larger
bound. Every retained (writer, destination) pair consumes a member, including
shared destinations. Exhaustion refuses before ID issuance/backend entry. Do not
replace or bypass the inner single-pending-writer invariant. Existing core proofs
continue to describe the inner journal; the new composition needs its own proof and concrete integration
qualification. An immutable projection must not expose an availability API that
silently ignores outer reservations. No mutable inner extraction is permitted.

Each queued submission retains its own registered writer, complete canonical
destination roster, exact predecessor references and authenticated dependency
roots. Allocation indexes identify queue head/tail; stale identities, cycles,
foreign devices, wrong extents and inconsistent indexes fail before effects.
Queue storage has an explicit checked bound and fallible preallocation before
backend submission. Settlement must use indexed retained members/edges, not scan
every writer or allocation. Dropping a descriptive reference never releases it.

The native N3 path has one full-allocation Write destination. Context supports
whole multi-destination writers, multi-parent joins and mixed idle/busy
destinations without splitting settlement. The overall goal also requires partial
writes and ReadWrite. Downstream pure reads of the exact queued output are now
implemented at the model/Context boundary described below. Full-overwrite support
must not be reported as broader write semantics or as multi-output native support.

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
construction; writer admission and activation are O(k), independent of total queue
depth. Settlement additionally visits the writer's attached queued reads.
Model tests exercise arena conservation, fixed storage addresses/capacities,
three-deep success, cancellation permutations, multi-parent/idle joins, readers,
stale identities, epoch exhaustion and fail-stop Unknown custody.

The owner model alone is not proof of the Context composition. In particular:

- Context authenticates each exact latest Ordinary producer-launch writer and
  explicit success event, retains the complete queued roster and blocks outside
  access through queue-aware availability preflights. Generated, synchronous and
  peer-copy predecessors do not grant queued-output admission.
- Queued Unknown is fail-stop and cannot be refunded by a later NoEffect value.
  Closed-group model disposal and Context shared receipt bookkeeping are
  implemented. Context tests now cover genuinely overlapping queued groups as
  well as disjoint multi-root groups. Native qualification remains separate.
  Disposal destroys allocations rather than exposing uncertain content.
- Failed/quiescent backend results alone are not NoEffect authority. Descendant
  failure after cancellation cannot be used to silently release queued custody.
- Queued-output reads have a CPU-tested model/Context implementation, scripted
  native-owner integration and frozen async-driver coverage. Native hardware,
  partial writes and ReadWrite integration remain open.
- The unchanged inner journal proofs do not establish outer queue invariants.

### Queued Output Reads

`ContextQueuedProducerReadV1` binds a range, allocation identity/device/extent,
and exact queued writer. It carries no guessed epoch or lineage. New admission
requires the latest Waiting/Ready writer and an explicit event authenticated by
the producer-aware Context profile. Original pure-Read aliases must be covered
by that producer's writable ranges. Generic launches and directed-peer consumers
do not gain this admission path.

The fixed queued-read arena shares the existing R budget with stable and active
producer leases. Acquisition validates all three families before committing any;
it validates each distinct queued producer's full destination roster once using
preallocated sorted scratch. Its model cost is O(S + A + Q log Q + sum of distinct
producer roster sizes). This is not a linear-complexity claim for Context's
complete binding and producer authentication.

Already-admitted ancestors may activate and settle while the deferred reader
remains. New writers, retirement and Unknown-group disposal cannot ignore it.
Producer Success records actual settled versions; NoEffect and Unknown resolve
only that exact producer's reads, never reparenting them to an ancestor. Resolved
read records survive writer-slot reuse and stay live until consumer quiescence.

Context retains separate active/queued typed reference arrays and independent
first-reference/count markers. Both complete rosters are validated before release.
The active release followed by queued release is a committed-prefix operation,
not an atomic transaction: an error or unwind between them retains the original
Context root, seals Context and quarantines remaining custody. The old shared
completion proof does not prove this internal composition.

CPU tests cover byte observations, exact-event rejection/acceptance, all seven
completion ingresses, public-event release, cancellation, Unknown/terminal
outcomes, three-family release and its injected partial failure, corruption and
writer-slot reuse. Model tests also check shared capacity, fixed storage, stale
identities, attachment-corruption rejection and affine indexed-access scaling.
No new formal proof, native hardware qualification or matched performance claim
follows from these tests.

### Queued Read Integration

Journal-enabled Context tests exercise the KFD backend with five scripted
persistent owners through A -> B -> C: A writes x, B overwrites x, and C reads
the exact B output. Same-stream and cross-stream cases release public events
after admission, complete native observations before Context reconciliation,
and compare x's owner identity through all three active binding rosters. They
inject distinguishable bytes into the scripted owners and inspect C's actual
input storage. They do not execute a kernel or synchronize native memory.

Cancellation coverage includes a cancelled middle writer without reader
reparenting, cancelled consumers that retain their ancestors, and same-stream
interior cancellation that remains TooLate until its downstream reader is
cancelled. The tests check complete native retain/reservation release, zero
journal records after allocation disposal, exact scripted owner recycling and
zero allocation-credit usage. Cleanup deliberately disables native SDMA sync;
this is not hardware teardown or native readback qualification.

Frozen async-command tests cover ordinary, tracked and event-producing drivers,
observer loss, preissue cancellation and producer-first reconciliation. A real
non-Send owner thread checks admission, progress, reply-cell retirement before
shutdown and owner-thread-only backend calls. Its MockBackend supplies consistent
terminal facts; it does not establish native scheduling or byte execution.

Scripted native terminal/unwind integration now covers both an active ancestor
with its two descendants still indexed and an active consumer after physical-only
ancestor completion. Same-stream and cross-stream subprocesses check exact owner
identities, pending rosters, dependency/module/event retains, completion reservations,
allocation-custody rosters and lane leases. Context retains six reads, three writer
roots, the pending event/callback and all five quarantined allocation charges.
Repeated public ingress and cleanup cannot release that custody. Dropping the
unrepaired Context aborts with core dumps disabled; no test disarms the backend.

These cases inject a restoration-slot mismatch or a panic immediately before the
active dispatch is moved. Consumer polling reaches the ancestor fault through an
exclusive blocker, before the pending consumer is removed from its index.
Additional same-stream flush cases reach the producer with the middle pending
submission temporarily outside its index. The latter exposed a lost pending
recipe/roster on unwind while FIFO and retain accounting remained live.

The six previously unguarded dependency observations now retain that exact
pending owner outside the caught poll, reinsert it and seal the backend before
resuming the original panic. Normal return/error classification is unchanged;
no launch or roster is cloned by the guard. Weak recipe references and original
roster addresses ensure the test snapshots cannot keep the lost recipe alive or
accept replacement storage. Backend adapter tests separately cover explicit and
ordered observation, ordered progress and failed-explicit dependency handling
behind a still-active FIFO predecessor. Direct observer cases exercise the
adapter after the SPI's normal ownership move, not its earlier blocker pre-poll.

These tests do not cover every ownership-moving panic point, the polled owner's
own unwind safety, native GPU execution or formal unwind safety. In particular,
the conflicting-SDMA-copy guard has regression-suite coverage but no dedicated
injected-panic case in this checkpoint.
The separately authorized three-phase hardware profile and full shared-body
queued-owner proofs remain open. The existing two-launch R57 authority and
inner-journal proofs do not cover those gates.

The shared queued-read resolution body now has a dedicated
[conditional proof boundary](runtime-queued-read-resolution-proof-v1.md).
Complete producer membership, selected-storage representation and authentic
settled outcomes remain premises. It does not close full outer-owner invariants,
reader release/refunds or the active-then-queued Context release composition.

Backend release now has additional CPU custody qualification for
[SDMA cancellation and unfinished stream ordering](evidence/dev-sdma-cancellation-stream-ordering-2026-09-27/README.md)
and [unpublished compute settlement](evidence/dev-pending-compute-settlement-2026-09-27/README.md).
These retain indexed descriptors through checked local release, preserve exact
recipes on settlement failure, and distinguish malformed FIFO membership from
valid interior cancellation refusal. They do not establish the complete Context
composition, arbitrary index-corruption recovery or GPU ordering.
[Prepared compute cancellation](evidence/dev-prepared-compute-cancellation-2026-09-27/README.md)
now retains the outer descriptor through native cancellation and restoration,
with a distinct three-binding Prepared boundary and CPU prefix-fault tests.
Direct native outcome-mapping qualification, allocator-failure injection and
executable formal correspondence remain open; the scripted ownership tests do
not close those gates or the complete Context composition.

[Prepared retry publication](evidence/dev-prepared-compute-publication-2026-09-28/README.md)
now retains the indexed descriptor across consuming publication, exact retry,
terminal return and unwind, installing Published before profiling. Native and
scripted paths share the receipt transition.
[Initial publication](evidence/dev-initial-compute-publication-2026-09-28/README.md)
now indexes Armed immediately after successful binding, before queue profiling
and first submit, and gives returned errors the same one-time pending-owner
handoff as unwind. CPU fault tests preserve a trailing queued successor and
exact shared dependency retains. Native bind failure recovery, coupled GPU
qualification and executable refinement remain open; this does not close the
Context composition.

[Single bind rejection](evidence/dev-single-bind-recovery-2026-09-28/README.md)
now carries the exact original target/source/submission and empty input box
through binding, restoring retryable inputs without allocating a replacement.
Counted CPU round trips and public scripted refusal with queued successors cover
that adapter. Native bind execution, initialized-storage allocation-failure
injection and allocation-free completion restoration remain separate open gates.

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
as rollback. Context now stores unique receipts in a group rather than mirroring
each native disposal into every co-owner's mutable counters.

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
uses bounded member passes and binary searches plus an O(R) read-arena scan
when input leases remain. Empty read arenas are skipped from their retained counts;
ordinary hot-path admission/settlement still avoid global scans.

Context now retains an Ordinary writer's conclusive quiescence marker separately
from removable submission metadata. Successful Unknown settlement can set it;
quarantine cannot. Both initial and final Ordinary disposal checks require it,
including absent-submission paths. Ordinary disposal discovers the closed writer
component, authenticates its complete roots and preflights the model before
freezing any receipt index. Pending co-owners refuse recoverably; invalid custody
quarantines. Original native receipts and all unique credits remain until the
whole group commits. Terminal model error or unwind after native release
quarantines Context. Tests cover partial release order, metadata removal, credit
failure, outside input custody and repeated full member-capacity reuse.

Generated receipt-based validation/commit and synchronous disposal remain on the
single-writer path, now through the wrapper. Full shared-body proofs remain open.

Context now retains an exact-writer-bound group table and an allocation-to-group
ordinal index. It freezes canonical writer rosters and a deduplicated allocation
union before the first backend release. Retries use that frozen state, not removed
public handles. Frozen retained roots and quiescence markers are immutable;
public submission metadata may be released independently. Each retry checks its
exact receipt/index and leader binding; finalization revalidates the complete
group without rescanning it for every allocation release. Native success records
one unique receipt and removes only that original public/backend handle. Complete
model commitment and credit-refund progress are retained before roots/indexes are
removed. New allocations reusing old numeric backend handles remain intact.

Flat model evidence now borrows packed writer headers/members and the unique
union without allocating after native effects. Nested and flat APIs share the
same validator/committer; tests compare rejection, success and every injected
error/panic prefix. Context tests use genuine disjoint Unknown submissions and a
private multi-seed preparation helper. They cover all 24 four-allocation release
orders with and without public metadata, retries, pending refusal, index
corruption, reused backend handles, and every unique credit-refund prefix. A
missing-model-root fault checks that retained Context custody remains visible.
Those earlier tests did not qualify overlapping Context writer queues. The new
public admission tests exercise overlapping groups, cancellation without
reparenting, no-handle rejection/quiescence, terminal/panic custody, multi-parent
joins, released public events, exact version progression and capacity refusal.
They also test native Success retained before logical reconciliation, TooLate
cancellation, and contradictory native parent observations. Completion fault
controls retain Context custody before effects and after committed prefixes.

Public host reads, host writes, ordinary launches and both stable/producer input
admission respect outer reservations even after the inner head has settled.
Generated adoption checks outer reservations before native DATA retirement;
predecessor settlement remains unguarded so it can make progress. Queued-only
Unknown disposal discovers the closed component rather than treating an idle
inner slot as available. Shared-body proofs, native qualification, generated
retirement hardware qualification and broader input/write semantics remain open.

A generated-shell owner-reservation test checks that adoption retirement refuses
queued custody before native DATA retirement without poisoning Context. It uses
valid journal reservations, not ordinary native launches on generated handles:
KFD intentionally rejects those handles. Preflight journal faults and any failure
after native retirement begins retain the existing fail-stop boundary.

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
