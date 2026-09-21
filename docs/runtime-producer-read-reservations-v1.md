# Producer-Bound Read Reservations V1

Status: concrete Rust model candidate. Not yet wired into RuntimeContext, Worker
transport or a native backend, and not covered by the registered Verus campaign.
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

Before enabling runtime admission, the remaining work is:

1. Prove outer arena partition, unique references, combined count/capacity,
   atomic admission/release, settlement preservation and inner stable-reader
   framing; add authenticated negative controls. The old allocation-frame
   lemma is not a proof of this new wrapper's concrete settlement composition.
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
