# Producer-Bound Read Reservations V1

Status: concrete Rust model candidate. Not yet wired into RuntimeContext, Worker
transport or a native backend. Conditional custody and logical lifecycle proofs
are registered in the Verus campaign; Rust/native refinement remains open.
Pending journaled producer-to-consumer requests still return `ContextReserved`.

## Ownership Model

`ContextProducerReadJournalV1` owns the existing stable-reader journal and a
separate bounded reservation arena. It exposes immutable inspection, but neither
mutable extraction nor `DerefMut`. Every allocation-mutating entry point remains
inside the wrapper. New writes, allocation retirement and Unknown-writer disposal
require the combined reader count to be zero. Producer settlement intentionally
changes protected allocation state while preserving the reservation lifecycle.

A reservation binds a complete allocation reference, device, extent, range,
producer writer reference, consumer identity, attempt epoch `E`, prior lineage
`L`, and fresh reservation incarnation. Admission requires `L < E`, the exact
pending producer, and a same-context submission consumer with a later local ID.
The caller must independently authenticate the event-to-producer relationship
and the backend's success-gated execution contract; model values are not native
authority or initialized-data evidence.

Reservations remain in their arena until exact consumer quiescence. They never
migrate into ordinary stable-version reads. Their status follows the protected
allocation without a settlement-time scan or allocation:

| Allocation observation | Reservation status |
| --- | --- |
| Exact pending producer, epoch `E`, lineage `L` | Pending |
| Exact Unknown producer, epoch `E`, lineage `L` | Unknown |
| No pending writer, epoch `E`, lineage `E` | Success |
| No pending writer, epoch `E`, lineage `L` | NoEffect |
| Any other identity or version | Invalid |

Only Pending admits a new producer-bound reservation. Success and NoEffect may
retire the producer's writer slot, so resolved reservations do not require that
old slot to remain occupied. Continuous allocation exclusion protects their
epoch/lineage relationship even if the slot is reused for another writer.
Release accepts every legitimate lifecycle status but authenticates the exact
consumer reference and inert quiescence premise. Release never implies that the
consumer received successful input.

## Bounds And Cost

Both arenas have `reads` preallocated slots but share a total limit of `reads`
active records. Thus the active-read budget is unchanged; allocated metadata is
larger than the original single-arena journal. Counts include both kinds of read
for writer admission, allocation retirement and Unknown-writer disposal.

Construction uses O(allocations + writers + reads) storage. Canonical acquisition
and release take O(k) for k requests and are unchanged-on-error, including caller
output. Lookup is O(1); producer settlement preserves the existing journal's
member-roster complexity and does not scan consumers. Ordinary stable-reader
header rejection precedence is preserved before shared-budget checks.

## Verification And Integration Gates

CPU tests cover all producer outcomes, arbitrary release/settlement ordering for
fan-out sizes one through four, independent producers in one roster, canonical
atomic admission/release, shared capacity, stable-reader coexistence, writer and
allocation slot reuse, incarnation exhaustion, unchanged storage identity, and
whole-roster mutation/disposal exclusion when a later member is protected.
These are executable tests, not a formal refinement or hardware qualification.
The [signed-source CPU qualification](evidence/dev-producer-read-model-cpu-2026-09-21/README.md)
records the GNU/musl model, doctest and runtime regression results.

`context_producer_read_invariant_v1.rs` adds 25 obligations to the 156 inherited
reader/journal obligations. It specifies exact arena partitions, live reference
identity/incarnation, per-allocation counts, and the shared active-read ceiling.
It proves logical construction, a two-consumer nonempty witness, and conditional
preservation under Success/NoEffect settlement and Pending-to-Unknown marking
(including idempotent Unknown). Settlement frames stable readers and the outer
reservation storage. Resolved status depends on the protected allocation, not
retention of the old writer/member slots. A retained reservation contributes a
positive combined reader count; concrete mutation rejection is not yet refined.

The dedicated pinned checker runs the whole importing crate before and after
12 invariant-sensitivity mutations, requiring each negative to fail exactly the
named postcondition. These mutations test the invariant, not implementations of
acquisition, release, or settlement. The checker authenticates the recursive
source/tool closure and binds solver inputs to captured pinned bytes.
The [signed-source custody qualification](evidence/dev-producer-read-custody-verus-2026-09-21/README.md)
records the accepted dedicated campaign and the full registered proof run,
including all 691 standalone negative files and portable evidence validation.

`context_producer_read_lifecycle_v1.rs` adds 74 obligations to the 181 inherited
obligations. Executable logical status, lookup, capacity and ordered preflight
functions match their exact decisions, including error precedence. Acquire and
release commit loops implement exact contents relations; rejection frames the
whole logical state and caller output. Separate wrappers prove arena, count,
incarnation and shared-budget preservation. Stable-reader wrapper operations
preserve the producer arena and shared limit. Combined-count arithmetic is
bounded, and the ordered unread guard rejects a retained reservation.

Nonempty executable witnesses cover late-item acquire/release rejection,
two-item round trips, release-capacity rejection, slot reuse and stale references,
and release in all four legitimate producer statuses. Settlement witnesses
instantiate the conditional journal relations; they do not refine production
settlement preflight. Release leaves the journal unchanged in every status.
The dedicated checker runs whole-crate positives before and after 19 executable
body mutations, with strict single-postcondition diagnostics. The exact custody
module header is authenticated separately; arbitrary imports remain forbidden.
The [signed-source lifecycle qualification](evidence/dev-producer-read-lifecycle-verus-2026-09-21/README.md)
records both whole-crate positive brackets, all 19 executable controls, and the
complete registered proof run with all 691 standalone negative files.

The linked historical archives retain their original source and obligation counts.
`context_producer_journal_issuance_v1.rs` composes the custody
predicate with exact `reserved_count` and successful-registration history. Its
logical constructor/register/Reserved-abort wrappers preserve the combined
invariant, reader storage and unrelated Pending/Unknown chains. A constructor-based
reuse witness covers rejection and stale identity; a separate directly initialized
mixed-state fixture covers all four reservation statuses during register/abort in
a retired producer slot. That fixture is not production reachability. The
[signed-source issuance qualification](evidence/dev-producer-journal-issuance-verus-2026-09-21/README.md)
records the 201 whole-crate obligations (181 inherited and 20 new), all 13
executable mutation controls, and the complete registered runner, including the
updated inherited proofs and all 691 standalone negative files.

The [signed-source enrollment qualification](evidence/dev-enrollment-execution-verus-2026-09-21/README.md)
connects complete logical batch admission, sorting/search, rollback and commit to
producer custody and issuance preservation: two 263/0 positives and 21 exact
262/1 controls. Its constructor/enroll/register witness reaches Reserved, not
Pending. Production still uses the standard-library sorting/search operations.

These are conditional logical-execution proofs, not whole-wrapper verification.
Complete retained-chain coverage is preserved by logical enrollment, but its
reachability through begin-write and the remaining base-journal transitions
still needs to be established.
Physical Vec storage,
fallible allocation, settlement scratch/preflight execution, panic/unwind behavior,
and production Rust correspondence remain separate. Release bounds use an observed
capacity argument, not a proved binding to Rust `Vec::capacity()`. The acquisition
induction advances a hypothetical incarnation prefix; the concrete logical loop
updates its watermark once at the end. It does not establish invariants at every
intermediate machine state or atomicity under panic/unwind.

Before enabling runtime admission, the remaining work is:

1. Finish base-custody reachability/issuance composition across begin-write and
   settlement. Establish production Rust correspondence for enrollment and the
   proved logical admission/release and unread guards.
   Bind physical capacity and fallible allocation to those relations, and refine
   settlement preflight before treating the logical lifecycle as full-wrapper
   refinement.
2. Retain an exact Context event-to-producer writer/member binding and a distinct
   producer-reader root, with preallocated capacity before backend entry.
   Include it in Context cleanup, generated-operation exclusion and usage.
3. Retain producer result custody independently of public event lifetime.
   Reconcile the producer through its own backend completion before publishing
   consumer success. Do not infer producer success from a consumer result.
4. Add an audited, default-false backend contract for success-gated dependencies
   and retained producer results. Generic event support and Worker negotiation
   do not imply this guarantee.
5. Drive bounded producer-first dependency progress from consumer-only owner
   registration, with shared deadlines, deduplicated work and cancellation/error
   custody. Do not require an undocumented separate producer registration.
6. Qualify the journal-enabled pending dataflow on native XGMI, including event
   release, producer-first and consumer-first observation, failures and cleanup;
   then run matched HIP/HSA comparisons before claiming performance gains.

This is immediate runtime/backend admission with producer-bound custody, not
host-deferred submission, journal bypass, or a claim of GPU-side dependency
packets. Wider typed-launch inputs and Worker transport remain separate
integration work under the full parity objective.
