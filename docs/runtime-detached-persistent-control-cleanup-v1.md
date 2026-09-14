# Detached Persistent Control Cleanup V1

Status: reviewed implementation handoff, not implemented or qualified. This is
the next lower cleanup packet above [locally accepted R115 returning-control cleanup](runtime-returning-control-cleanup-v1.md).
R115's lower returning paths do not qualify this detached-persistent path.

## Boundary

Convert only `DispatchResourceOwnerV1::release_detached_persistent_control_v1`
in `crates/fe2o3-kfd/src/queue_dispatch_binding.rs`, using the existing
`queue_dispatch_binding/control_release.rs` full-owner storage and R114 borrowed
control cleanup adapter. Preserve the existing public and borrowed validation
interfaces. Primary owns shared edits and integration.

The input's data was already detached and returned to its separate owner. This
method returns `Result<(), Gfx942DispatchBindingErrorV1>` and has no returned-data
on-error API. The separate `release_persistent_data*` bridges, ordinary data
disposal, live model-loan transport and queue teardown remain later packets.

## Ownership And Validation

Move the complete original owner into custody before any validation. Retain
kernarg, the forward code iterator, code identities, packets, all data premises,
generation state and the exact persistent-control identity. A malformed input's
unexpected data must also remain owned; constructing custody cannot discard it.

Use an explicit detached-persistent mode carrying `expected_generation` and a
unit-result wrapper. Share the borrowed kernarg/forward-code driver without
forcing detached success through returned-data conversion or extraction.
Detached cleanup needs no return-vector reservation or new per-control storage.

Preserve the existing error precedence:

1. Reject repeated execution, then latch the one-shot attempt.
2. Evaluate `generation.returned_generation()`. Poisoned state returns Poisoned;
   live, never-published, cancelled-only or unrecycled state returns ResourcePhase.
3. Apply the existing detached-state helper: require DataDetached, zero data
   authorities, premise count equal to retained binding count, and recycled
   generation equal to `expected_generation`. Every mismatch is ResourcePhase,
   including an unexpected generation; do not replace it with a stale-generation
   error.
4. Dispose kernarg first, then code in original forward order. Install each
   token in the active lower custody before calling the borrowed adapter.
5. Mark Complete only after every control is confirmed Complete. Preserve
   premises, persistent identity and generation until the root is released.

Cancelled reservations after recycled work and an exhausted next-generation
counter do not themselves prohibit cleanup. The validator does not recompute
semantic, queue or storage identity or validate arbitrary private identity
corruption. Preserve those fields exactly rather than claiming new admission
checks.

Errors, panics and an incomplete successful callback retain the complete root
before exposing the original result. Retry cannot re-enter cleanup. Confirmed
native disposal followed by model/currentness failure retains the disposal
receipt and must not reconstruct releasable authority. The detached root cannot
yield a returning-data output.

## Required Tests

- Construct one- and three-binding Attached owners, drive real reserve,
  publication, completion and recycle transitions, then use the actual detach
  operation. Keep detached data and any fixture-only surplus owners separately
  rooted throughout cleanup.
- Use model-aware lower cleanup fixtures for native progress, records, charges
  and certificates. Add genuine preparation fixtures for populated code, packet
  and premise metadata, with matching queue identities in completion records.
- Check exact success order, untouched detached data, retained metadata and
  storage, and absence of output allocation or conversion.
- Combine generation errors with malformed persistent state/cardinality to
  establish precedence. Test Ordinary/Attached state, nonempty data,
  missing/excess premises and unexpected generation with zero cleanup entry.
- Inject errors and panics at every control position, currentness boundary,
  native unmap/release prefix and model projection. Include malformed unmap
  progress and actual certificate-commit rejection.
- Inspect mapped/unmapped/disposed active custody, untouched suffix and exact
  charges. Distinguish final-currentness failure from failure after native
  record settlement but before model commit.
- Use an independent no-entry retry callback, incomplete-callback rejection and
  wrapper sinks that observe retention before exact errors or original panic
  payloads. Successful cleanup must not invoke the retention sink.
- Qualify decisive compiled negatives, unchanged R114/R115 regressions, exact
  source restoration and independently reviewed evidence.

## Later Integration

Keep Complete in borrowed custody so N4-L can retain it through actual model
retake and ledger commit. This lower packet does not qualify that composition,
data disposal, queue/event/signal destruction, native GPU execution, formal
refinement, aggregate retained-memory bounds or performance.
