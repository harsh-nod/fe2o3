# Persistent Returned-Data Cleanup V1

Status: reviewed handoff, not implemented. This follows
[detached persistent-control cleanup](runtime-detached-persistent-control-cleanup-v1.md).
The read-only swarm review on 2026-09-14 adds no accepted test, proof or hardware
result for these persistent returned-data bridges.

## Boundary And Contract

Replace only DispatchResourceOwnerV1::release_persistent_data_before_publication,
release_persistent_data_after_recycle and shared release_persistent_data in
crates/fe2o3-kfd/src/queue_dispatch_binding.rs. Use the existing complete owner
in queue_dispatch_binding/control_release.rs, its borrowed control adapter and
narrow test helpers; add a separate persistent test module.

Generation rejection precedes cardinality rejection; both return empty data.
After successful validation, every control-cleanup Err returns the entire
original ordered data set. Success returns the same set and admitted generation.
Before-publication permits generation zero; after-recycle does not. Do not add
an Attached-state requirement absent from today's methods.

## Ownership Design

1. Add explicit persistent-before-publication and persistent-after-recycle
   modes; construct the complete root before generation validation.
2. Root initially empty Vec<Gfx942FixedDispatchDataV1> output storage and explicit
   readiness. Reserve capacity after generation/cardinality checks, before any
   disposal. Do not reserve ordinary returned-lease storage for these modes.
3. Reuse the one-shot kernarg-first, forward-code borrowed control driver.
   Preserve active custody until explicit Complete.
4. Persistent-only extraction may transfer data after cleanup success or Err
   when preflight completed and cleanup returned normally. Use
   dispatch_data_from_authority_v1 and preallocated
   storage, preserving initialization without reviving content descriptors.
   Never allocate after disposal or add a second cleanup loop.
5. Preserve tuple signatures. Early rejection retains the whole root and returns
   empty data. Later Err transfers all data and retains controls/metadata.
   Panic retains untransferred ownership before resuming the original payload.
6. Ordinary returning wrappers/extraction positively admit only their original
   two modes. Unit extraction is detached-only. Wrong wrappers reject before
   cleanup or transfer. Track readiness separately from output-vector emptiness.

Returned-on-error data is no longer owned by the retained control root. Preserve
this explicit split in tests and documentation, including valid zero-data input.
Retain original premises and metadata after transfer; borrow premises while
converting data. Do not reuse the existing interrupted-root assertion unchanged,
since it requires that all original data remain rooted.

Follow-up review freezes explicit persistent output states:
Unprepared -> Prepared(generation) -> Returnable(generation) -> Taken.
Prepared follows successful generation/cardinality/capacity preflight.
Returnable follows a normal return from the common cleanup driver, including
Err; a panic leaves Prepared and extraction must reject without mutation.
Taken latches before extraction, independently of data length and the existing
control-complete flag. A normal cleanup Err can return data without claiming
that controls completed. Later N4-L may retain Returnable across retake, but
outer settlement still decides whether output escapes. No second cleanup loop
is needed for this distinction.

## Decisive Tests

Use the accepted scripted lower adapter, not live cancellation injection, which
can bypass the real bridge. Cover generation precedence, cardinality, capacity,
zero data, cancelled history, exhausted next generation and exact five-variant
mixed data output on success/error. Check identities, order, initialization and
absence of revived descriptors. Cross every control position with native
error/panic, currentness, partial unmap, projection/actual commit rejection and
incomplete callback. Inspect exact active state, suffix, metadata, charges and
completed effects; data itself receives no disposal. Error transfers once,
panic retains, retry/wrong extraction never re-enters. Capacity must predate
disposal and survive success/error extraction. Use genuine single- and
three-binding persistent preparation fixtures. Keep R115/R117 regressions.

Use fourteen table-driven behavioral functions and one supplemental routing
guard: exact mixed-data success; generation-error precedence; cardinality/capacity
precedence; zero-data readiness; generation history/exhaustion; native error/panic
custody split; all eighteen currentness boundaries; partial unmap; projection and
actual commit failure; incomplete callbacks; wrong/repeated extraction; real
preparation; pre-disposal output capacity; and panic-root extraction rejection.
Exercise wrappers for error/panic output splits, not only the borrowed driver.

Reuse pristine_dispatch_fixture_v1(8), lower error/currentness/unmap/commit
injection and native/control-record snapshots. Advance genuine prepared owners
using their retained queue identities, without detaching data. Add explicit
persistent-mode cases to fixture generation transitions: the old fixture only
recycles Mode::AfterRecycle and would otherwise mask later failure assertions.

Freeze mutation anchors only after implementation. Independently test mode
selection, swallowed generation errors, preflight ordering/guards, constructor
data retention, length-based readiness, premature Returnable, missing normal-Err
eligibility, missing Taken, late/wrong output allocation, reversed/truncated or
misinitialized output, error/panic retention and each wrong-wrapper guard.
The decisive panic negative promotes Prepared to Returnable before cleanup and
must fail the panic-root extraction-rejection assertion. Reuse shared control
order/active-custody/retry/incomplete mutations against persistent assertions,
recording repeated source maps separately from distinct mutations.

## Subsequent Production Joins

Persistent N4-L must root custody outside the live model loan through retake.
Both existing callers are before-publication cancellation; no current caller
uses the true after-recycle branch. Retake error still dominates while returning
exact data; retake panic must retain the sole output owner.

Ordinary N4-L also needs outer custody for detach_recycled_fixed_dispatch_inner
and retained-control release. with_live_queue_memory_model can otherwise discard
successful output on a closing failure. Data disposal needs typed custody for
release_fixed_dispatch_data, mixed owner release and detached-data release
before committing a reusable hole. N4-Q must retain outputs across later signal
disposal, callbacks and backend extraction, and retain taken auxiliary lanes.

This packet feeds N5 DATA-ADOPT, but N5 still needs its applicable live/data/queue
routes. It does not need every unrelated teardown mode first. No new native,
formal, memory-bound or performance acceptance follows from this handoff.
