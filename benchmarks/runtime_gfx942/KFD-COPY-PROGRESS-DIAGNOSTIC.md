# KFD Directional Copy Progress Diagnostic

The optional modes of `gfx942-runtime-directional-window-benchmark` investigate
the [remaining directional copy gap](../../docs/evidence/dev-hsa-pool-engine-mi300x-2026-09-18/README.md).
They retain the public RuntimeContext copy path and add host timestamps plus
public wait/flush call counts. They are development diagnostics, not parity
acceptance records or new formal/native milestone qualification.

The original four-argument path, copy timer, progress loop, validation, teardown
and single `fe2o3.async-copy-benchmark.v1` output row remain uninstrumented.
Diagnostic modes use a separate implementation and distinct
`fe2o3.kfd-directional-progress-diagnostic.v1` schema. Compare the two diagnostic
policies to each other, not their instrumentation overhead to the legacy row.

## Invocation

Build the actual example in an owned directory with the pinned toolchain:

```sh
CARGO_TARGET_DIR="$owned/target" cargo build --locked --release \
  -p fe2o3-runtime --example gfx942-runtime-directional-window-benchmark
"$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" \
  "$unique_id" 268435456 3 10 diagnostic-slice50us
"$owned/target/release/examples/gfx942-runtime-directional-window-benchmark" \
  "$unique_id" 268435456 3 10 diagnostic-window-deadline
```

Unknown modes, extra arguments, zero/oversized copy lengths, zero samples,
overflow and more than 10,000 total rounds reject before opening KFD. The
diagnostic reserves its bounded round storage and two host buffers fallibly
before native setup. Run only after a fresh shared-host device identity,
utilization, VRAM and mapped-PID check, with an external deadline and postflight
check. Neither the executable nor low utilization alone reserves a GPU.

## Policies And Timers

`diagnostic-slice50us` passes `min(outer remaining duration, 50 microseconds)` to
each public Context wait. `diagnostic-window-deadline` passes the whole remaining
duration. Both initially flush and flush again after every `Pending` result.
At a completed native window frontier, the backend retires that window and
returns `Pending` with retained Ready custody; the next explicit flush publishes
the continuation. It does not wait out the full remaining duration before
returning that frontier. This is not a native liveness guarantee.

Both modes use the same three timestamps:

- `submit_ns`: before `copy_async` through its return, including admission and
  the backend's immediate first publication.
- `progress_ns`: that return through observed success, including initial and
  continuation flushes, waits, currentness checks and retirement work.
- `total_ns`: the complete host interval, exactly `submit_ns + progress_ns`.

Submission release, host preparation/copy/hash, readback, full-buffer validation
and teardown are outside these timers. Timing does not separate physical DMA,
OS scheduling, facade instructions or native-adapter overhead. Full-remaining
mode stays in a lower wait epoch long enough to reach adaptive yield/sleep;
sliced mode repeatedly re-enters the active-spin/currentness/ticket path. A
measured difference is the combined policy/re-entry effect.

`wait_slice_ns=50000` is a cap for sliced waits, not a claim that every wait gets
exactly that budget. `wait_slice_ns=0` in window-deadline mode means **not
applicable**, never `wait(Duration::ZERO)`. Context internally reconstructs a
deadline from the passed duration; call overhead can extend it beyond the outer
Instant. `*_wait_calls` and `*_flush_calls` count public calls, not native
completion observations, physical doorbells, or DMA packets. KFD engine indices
1/0 must not be identified with HSA engine masks 1/2.

## Validation And Output

Each round uses a changing uniform upload pattern and poisoned download buffer.
It retains no round until both directional submissions are conclusively
completed/released and every returned byte matches. All warmups and samples
are reported with explicit index and phase; statistical aggregation belongs to
the external analysis, not an implicit pool of process samples.

Only successful release of all three allocations, stream destruction, Context
shutdown and native backend shutdown creates the completed-run value that can
emit config, round and completion records. Error paths cannot emit successful
measurements. A failed native operation may invoke the runtime's existing
abort/quarantine behavior rather than graceful cleanup; the external process
deadline and postflight check remain required. Writer failures after teardown
return failure, including a final flush failure after apparently complete output.
Accept only the exact expected record roster **and** zero process exit **and**
successful postflight. Setup failures, truncated output and unsupported policy
results must never be silently replaced by another mode.

## CPU Qualification

```sh
cargo test --locked -p fe2o3-runtime \
  --example gfx942-runtime-directional-window-benchmark
```

The deterministic tests exercise legacy opt-in isolation, independent budget
oracles, frontier flushes, repeated Pending, deadlines, errors, checked counters,
timestamp decomposition, direction/round-distinct output and writer failures.
The existing scripted native-backend frontier test supplies a separate check
that a future wait deadline returns Pending without publishing the next window.
Neither test family proves Linux timing, physical DMA behavior or performance.

## Native Attempt

The [2026-09-18 MI300X attempt](../../docs/evidence/dev-kfd-copy-progress-mi300x-2026-09-18/README.md)
completed eight of sixteen planned processes, then stopped before launching
block three when the shared-host occupancy guard failed. Its completed prefix
is retained only as individual observations; the complete-campaign checker
rejects it. The owned remote build directory was removed. No production wait
default or parity milestone changed on the basis of that interrupted experiment.
