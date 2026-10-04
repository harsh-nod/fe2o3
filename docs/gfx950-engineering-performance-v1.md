# Gfx950 Engineering Performance Policy V1

This opt-in policy applies only to the isolated, unauthenticated-machine-code
engineering worker. It is not protected runtime, loader, dispatch, or serving
authority, and does not change the public gfx950 checked-observation contract.

Immediately after `Ready`, before allocating any user buffer or loading a
kernel, a controller may issue `ConfigurePerformance` with three independent
booleans: `cache_kernel_admission`, `operational_currentness`, and `profile`.
The worker acknowledges `PerformanceConfigured`. Configuration is accepted
once only, including when all three options are false. Previously allocated
then freed resources do not restore eligibility. Existing controllers that do
not configure retain full checks and per-dispatch object validation.

## Immutable Admission Cache

The worker retains an owned clone of the selected, admitted kernel metadata
alongside the immutable object bytes, materialized code allocation, resource
binding, and descriptor offset. Cache mode avoids repeating HSACO parsing and
kernel selection. Every dispatch still checks current buffer ownership,
pointer ranges, alignment, argument access and aliasing, caller-zero implicit
arguments, grid/workgroup geometry, descriptor and kernarg addresses, queue
frontiers, signal state, and exception state. No load or mutable buffer result
is reused as evidence for another argument binding.

There is no kernel replacement or unload API, and monotonically increasing
resource identifiers are never recycled. Close destroys all cached state. Any
validation, native operation, timeout, reset, or framing failure remains
terminal: the worker reports a fatal error and retains uncertain resources
until process exit. Configuration cannot recover a poisoned owner.

## Currentness Boundaries

Full topology, aperture, sysfs, UAPI, descriptor, process, reset-stream, XNACK,
and DRM identity checks remain mandatory at initialization, configuration,
allocation/mapping, free/unmapping, kernel loading, and queue teardown.
Opt-in ordinary mapped-memory and dispatch operations use a bounded retained
queue fence, analogous to the existing gfx942 operational fence:

- The original process and process incarnation must match, before consuming
  the retained reset FIFO; process incarnation is rechecked at exit.
- KFD and render descriptors must still match their retained identities.
- KFD UAPI and XNACK mode must still match.
- Full DRM identity including the VRAM-loss observation must still match.
- The prospective reset stream is checked before and after observations.
- Any error poisons the same device token used by the full check.

Topology and aperture equality become lifecycle observations in this opt-in
policy, not per-dispatch observations. The policy does not prove an all-reset
generation, prevent unreported resets or observation ABA, or authenticate the
driver/hardware. It does not enable gfx950 XGMI publication authority.

## Measurements

With `profile` enabled, `PerformanceSnapshot` returns checked cumulative
`PerformanceCountersV1` counters. It is valid only at an idle worker boundary.
The snapshot includes its own idle currentness check but excludes its own
command service count/time. Counts begin at configuration, excluding worker
startup. Counter overflow is terminal rather than wrapping.

Timers are worker wall-clock nanoseconds, not GPU timestamps. `command_ns`
excludes header/payload reading and response writing. Read/write totals include
their validation fences. Dispatch preparation includes its initial fence and
argument validation; publication includes the publication fence and doorbell;
wait includes polling, sleeps, and periodic fences. Final idle validation is
included in command time but not those three dispatch subphases. Currentness
timers overlap their parent phases and must not be added to them. Kernel
admission timing covers object validation and binding at load or dispatch.

Take snapshots outside measured model intervals, and compare separate runs
with each policy option toggled independently. No performance improvement is
claimed until the same artifacts and workload pass numerical checks and
unprofiled repeat measurements. Profiling itself adds observation overhead.

## Queue Rollover

`RolloverQueue { expected_epoch, expected_completed_packets }` is a separately
explicit engineering command, independent of performance configuration. Epoch
starts at zero. The expected packet frontier is the count since the previous
rollover, not the lifetime dispatch count. Exact epoch and completed frontier
must match before any native lifecycle transition; incomplete work, drift,
exceptions, or exhausted epochs fail closed.

The worker validates full currentness and idle counters, destroys the old
queue, destroys its exception event, disables the old runtime lease, releases
its doorbell, and releases all six old private queue allocations. Only then
does it reset its software frontier and allocate/create a new ring, control,
signal, kernarg, EOP, CWSR, exception event, runtime lease, and doorbell. It
validates the new queue and full currentness before acknowledging
`QueueRolledOver { retired_packets, queue_epoch }`.

User allocation and kernel identifiers, mappings, bytes, and immutable
admissions remain owned and unchanged. No read pointer is fabricated from a
completion signal. The existing 131072-unretired-packet bound is unchanged;
controllers must request rollover before a dispatch would exhaust that ring.
Any uncertain destroy, unmap, disable, allocation, create, or validation result
is terminal and must end the process; it is never retried on the same owner.

## Checked Dispatch Sequences

`DispatchSequence { dispatches }` contains 1 to 16 `SequenceDispatchV1` entries.
Each entry has the ordinary kernel identifier, kernarg byte count, geometry,
pointer fixups, and timeout; binary payload concatenates exactly those kernarg
regions. Normal kernarg/fixup bounds apply, total payload is at most 4 MiB, and
the sum of per-dispatch timeouts is at most 600000 ms. Header bounds are
unchanged. Allocation, free, load, write, and rollover cannot occur inside a
sequence.

The worker checks full currentness and idle state, verifies enough retained
ring capacity for the whole sequence, and prepares every binding before
publishing the first packet. It executes packets sequentially using the same
completion checks and per-dispatch timeout as the ordinary path. Signal and
kernarg storage are never overwritten while a dispatch may still be live.
Full currentness and idle state are checked again before success. This removes
parent/worker round trips, not GPU completion waits; it is not async dispatch.

Success returns `DispatchSequenceCompleted { elapsed_ns }`, one completed
timing per requested dispatch in order. Failure returns one terminal
`DispatchSequenceFailed` with completed timings, `completed_dispatches`,
`attempted_dispatches`, message, and `fatal: true`. Preparation failures have
zero attempts. An attempted but unacknowledged dispatch is uncertain, not a
completed dispatch. Post-sequence fence failure may report every dispatch
complete but still be fatal. The process retains all resources and exits; no
second error frame or subsequent command is emitted/accepted after this
failure response. Sequence framing errors use the ordinary fatal error frame.
