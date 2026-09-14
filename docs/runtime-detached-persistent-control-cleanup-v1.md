# Detached Persistent Control Cleanup V1

Status: R117 locally accepted at the scripted lower CPU/source boundary, with
[retained evidence and two independent reviews](evidence/local-r117-detached-persistent-control-cleanup-2026-09-14/README.md).
This extends [R115 returning-control cleanup](runtime-returning-control-cleanup-v1.md)
only to the detached-persistent path described below.

## Integrated Candidate

The reviewed six-source-file candidate was integrated byte-for-byte above R116
`4a49234a03fc3759bd1d3ca330197619b4ae8ab0`. Its source map is
`c7cbc3c52eb29d3eaf70cfe9fdf964a9f5c092eb439cad26d1706874f51735be` across
5,689 source identities. Fresh formatting and all 31 focused control tests pass.
Fresh GNU and musl dependency-closure runs each pass 2,762 tests with five
ignored across 48 libtest harnesses and one unchanged harnessless benchmark.
Both children are closed, their owned process groups are empty and all source
endpoints match. Nine runner, 31 freeze and 117 qualification-contract tests also
pass. All 17 source gates and ten auxiliary checks pass. Frozen/restored
detached, returning, pristine, lower-cleanup and transport suites pass 16/15/37/7/9.
All 32 compiled negative executions reach their exact behavioral assertions,
covering 28 distinct mutations: twelve detached-specific mutations, sixteen
retained R115 mutations and four shared mutations checked against detached
oracles as well. Every negative restores all 5,689 source identities.
The closed collector and both independent reviews verify all 372 raw artifacts,
including the separate isolated history below. Source evidence establishes
endpoint equality, not continuous immutability.

## Isolated Candidate History

The local `fe2o3-detached-control` worktree above R115
`4756971168f6b4f2c33d217f5d476ccde8ea2740` now implements the explicit detached
mode and unit-result wrapper. Six source files differ, including a new detached
test module and test-only preparation constructors; no feature or public API
change is added. The existing returning driver and borrowed cleanup adapter are
shared, with no detached return allocation or conversion.

The current source map is
`60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d` across
5,686 source identities. GNU focused control tests pass 31/31: all fifteen R115
regressions plus sixteen new tests, fifteen behavioral and one routing guard.
Formatting and strict all-feature/all-target KFD Clippy pass on the same map.
The seven `detachedcandidate-*` runner records retain an initial routing-guard
failure caused by a formatter-inserted comma and a later Clippy style failure;
neither is relabeled as a passing run. Every recorded child is closed and its
owned process group is empty.

The new matrices cover separately owned detached data, generation/state
precedence, zero return storage, every control position and destructive prefix,
currentness, partial unmap, projection and actual certificate-commit rejection,
incomplete callbacks, one-shot cleanup and wrapper retention. Genuine single-
and three-binding preparation fixtures preserve populated code/packet metadata.
Lower state-premise fixtures intentionally do not supply authenticated binding
admission. Fresh/cancelled-only generation paired with detached controls is a
separately identified corruption case, not a reachable successful detach.

Those isolated runs did not qualify integration. The accepted integrated R117
checks, compiled negatives, exact restoration and independent evidence reviews
are recorded separately above. These local
scripted tests do not establish live Linux/KFD execution, live-model transport,
queue teardown, formal correspondence, total memory bounds or performance.

## Boundary

Convert only `DispatchResourceOwnerV1::release_detached_persistent_control_v1`
in `crates/fe2o3-kfd/src/queue_dispatch_binding.rs`, using the existing
`queue_dispatch_binding/control_release.rs` full-owner storage and R114 borrowed
control cleanup adapter. Preserve the existing public and borrowed validation
interfaces. Primary owns shared edits and integration.

The input's data was already detached and returned to its separate owner. This
method returns `Result<(), Gfx942DispatchBindingErrorV1>` and has no returned-data
on-error API. The separate [`release_persistent_data*` bridges](runtime-persistent-returned-data-cleanup-v1.md),
ordinary data disposal, live model-loan transport and queue teardown remain
later packets.

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
