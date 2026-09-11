# Borrowed Coherent Initialization V1

R78 implements borrowed native initialization hooks needed by GEN-2B-3's
complete-buffer adoption. Both existing ordinary runtime materializers now use
them. Full generated adoption remains open: it needs an installed owner-local
driver or explicitly nonpublishable owner, not an automatically publishable
prepared state.

## Ownership And Copying

`SharedGttMemorySessionV1::initialize_host_visible_coherent_from_slice_v1` borrows
source bytes synchronously. One private fixed-token sequencer allocates ordinary
coherent GTT, copies every requested byte and maps that exact allocation. Its
production implementation calls the existing model-aware allocation, CPU-access
and GPU-map methods. Only successful completion constructs initialized storage.
No source borrow escapes and no caller initialization assertion is accepted.
The boxed entry point delegates while retaining its source throughout the call.

Queue insertion/replacement and lane facades have corresponding borrowed methods.
Unbound/completion, capacity and exact-ordinal checks still precede native effects.
The existing model-loan/retake envelope handles initialization; detached identity
recording follows successful retake. Error/panic poisoning and uncertain native
custody policies are unchanged. These methods do not publish packets.

Both `materialize_initial_data_v1` and `materialize_rebound_data_v1` now pass
`DataSpecV1::bytes()` directly for HostVisible data. Its Arc remains owned through
the synchronous call and its selected range is unchanged. This removes the
temporary boxed payload allocation/copy previously made by `try_owned_bytes`.
DeviceLocal initialization still consumes owned bytes and the authenticated
content descriptor. The native GTT copy remains necessary; this is not zero-copy
execution or a measured copy-bandwidth improvement.

## Accounting And Failure

Configured ordinary coherent storage uses the existing R72 account: page-padded
bytes and one allocation record are charged once. Source/view/token disposal or
GPU unmapping does not refund backing. Complete confirmed free/VA disposal uses
the existing refund path. No additional encoded host buffer or duplicate R73
reservation is introduced by the HostVisible branch. Metadata, readback overlap
and aggregate accounting remain separate work.

Opening currentness failure cancels an unissued reservation. Failure after the
first native attempt retains its debit even without a completed allocation
record. Later copy/map failure retains the actual record. Error or panic returns
no initialized storage and grants no permission to retry or speculatively free it.

## Evidence Boundary

The [local record](evidence/local-r78-borrowed-coherent-2026-09-10/README.md)
separates ten new CPU tests:

- Six shared-memory cases exercise the production sequencer with real engine
  records and R72 accounting over a fake backend, plus facade source wiring.
  They cover complete non-page-aligned data, source mutation/disposal, both budget
  dimensions, native failures/panics, all seven currentness positions, partial
  mappings, retained prefixes and exact refunds.
- Two queue cases cover actual early-rejection methods on a no-engine fixture
  and wiring into the existing loan/retake/identity envelope. They do not prove
  successful Linux insertion or new loan/retake refinement.
- Two runtime cases check exact HostVisible source arguments in both branches
  and count allocations for DataSpec views versus owned copies. The count covers
  local byte views, not whole native initialization calls.

No new model theorem, Verus solver result, Linux/GPU qualification, protected
production generated launch, whole-executor refinement or HIP/HSA measurement
is claimed.

The existing R60 ordinary-pipeline qualifier exercises initial HostVisible
materialization under exact vecadd qualification authority and checks complete
outputs. Its successors intentionally reuse one buffer triplet, so it cannot
qualify rebound initialization. That needs a separate bounded two-launch cell
with distinct full HostVisible triplets in the same Context/stream/native queue,
three materializations per launch, one queue creation and complete outputs.
Device-persistent and copy-only qualifiers cannot substitute for either path.

## Next Integration

R77's [projection](runtime-persistent-generated-projection-v1.md) retains every
original buffer, including unused entries and interior offsets. Ordinary runtime
materialization still uses its bound-window snapshots; it is not a complete
generated-roster adapter. That adapter must enumerate original buffers and
immediately retain every successful native allocation, including partial prefixes
on failure or panic.

Context preparation promises no native allocation. Existing `MaterializedPrepared`
can publish through deferred flush, so neither is inert adoption custody.
Admission must supply an explicit nonpublishing owner/driver, exact registrations
and complete charged completion resources. Native then connects full-roster
adoption and retained submission-bound authority. Readback, deadlines, public
async APIs and generated graph/drain follow the
[swarm dependencies](runtime-a1-a2-next-wave.md).
