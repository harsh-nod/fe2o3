# Gfx950 Independent-Rank Engineering Rounds V1

This additive API is explicit unauthenticated-machine-code engineering, not
protected runtime or serving authority. Existing independent dispatch, serial
peer dispatch and same-rank sequence profiles keep their contracts. One
single-threaded disposable process still owns the entire group. There is no
thread sharing, second KFD client, exported allocation or symmetric-memory API.

## API And Integration

```rust,ignore
pub unsafe fn dispatch_round_unchecked(
    &mut self,
    commands: Vec<Gfx950EngineeringPeerDispatchV1<'_>>,
) -> Result<Vec<u64>, String>;
```

The command type is the existing borrowed kernel token, owned kernarg bytes,
grid, workgroup, owned pointer records and timeout. Kernel tokens identify ranks;
the caller cannot supply a substituted raw rank, allocation handle or pointer.
The result has exactly one observed completion time per command, in input order.
These are host submission-to-observed-completion nanoseconds, including later
rank submissions and polling delay, not hardware kernel durations. Full exit
validation is outside these per-command intervals. Measure model timing in the
controller and do not add overlapping command durations as a workload duration.

A client must explicitly name a new concurrent-round profile. The suggested
Ferric label is `device-peer-concurrent-round-v1`, not `device-peer-serial-v4`.
Use one bounded frame containing 1..world rank-tagged dispatches, and one ordered
all-completed response or terminal failure. No existing core worker wire command
is changed. Rank PID reports still identify one shared child, not eight distinct
processes. The client must reject mixed profile capabilities, duplicate ranks,
interleaved host/lifecycle operations and reordered or incomplete responses.

Rounds can contain a subset of ranks. Model dependencies remain explicit
barriers: independent projections may share one round, followed by a completed
barrier and then a reduction round. A producer and its peer reader may not be in
the same round. This API does not reorder reductions, add atomics, change BF16
rounding or permit kernels to synchronize with another command's progress.

## Admission And Ownership

World size is exactly two or eight. Each round has at least one command and at
most one command per retained rank. Per-command kernarg and pointer limits are
unchanged: 65536 bytes and 256 fixups, respectively. Positive timeout sums must
fit 600000 ms. Each dispatched command gets its own deadline starting before
kernarg initialization/publication. Previously submitted deadlines are checked
before further publication, and a round rejects expired work before polling;
a late observed completion cannot retroactively authorize more work.

The ordinary serial entry still observes completion before testing the timeout
of an incomplete poll, as it did before publication/polling were factored. The
new round entry is deliberately stricter: it checks the deadline before polling
because other ranks' submission work can delay that first observation. Existing
serial callers are not silently assigned the round's deadline semantics.

The entire finite set is validated before any packet publication: kernel/group
incarnation, live owner buffer and mapped peer-read authority, exact argument
ABI/access/alignment and bounds, within-command aliasing, geometry, and capacity
in each affected queue. Cross-command overlapping ranges reject if either
access may write. Read/read sharing, zero-length views and disjoint ranges are
permitted. Every token is still checked against its retained owner; a raw range
comparison is not an alternative to ownership validation.

An exclusive group borrow retains all code objects, buffers, peer mappings,
contexts and queue resources through the round. Each rank has private retained
kernarg and signal storage, and only one dispatch may be outstanding on a rank.
No mapping, host copy, free, kernel load, rollover or unrelated GPU access can
interleave. Kernels must honor their complete declared access regions; binding
validation does not provide hardware memory protection or prove machine code.

## Currentness And Completion

The unchanged full all-context currentness and idle checks run at entry and
exit. They retain exact topology/aperture identity, directional coherent XGMI
route/hive observations, descriptor/process identity, XNACK, DRM/reset state and
real queue state. An opt-in operational configuration applies only inside the
round; without that configuration, these inner checks remain full as before.

Before each publication, every context undergoes its configured currentness
check and queue-exception observation. The publishing context additionally
performs its normal immediate prepublication check. All commands are submitted
before any completion poll or wait. Hardware overlap is permitted, not promised:
fast kernels may finish before later queues are published.

In-flight queues cannot truthfully pass an idle check. Instead, each pending
queue is polled without blocking the others, using the unchanged acquire signal,
write/read frontier and exception checks. Queue identity/epoch and pending state
must match. Read retirement is never fabricated from a completed signal.
Completed queues undergo normal idle validation. Unfinished rounds also check
every participant's currentness and exception state on the first incomplete
pass and at approximately 100 ms intervals; ordinary per-queue incomplete-wait
checks remain. The full group exit fence requires all queues idle again.

Any rejection, timeout, currentness/reset failure, native error or exit-fence
failure poisons the entire group. A failed publication may already be visible
to hardware. Neither it nor previously published work is canceled, retried or
freed speculatively. No partial-success result is returned. Later operations,
including close, reject on the poisoned owner; Drop deliberately retains native
contexts until the disposable process exits. Successful close continues to
unmap every peer before freeing its owning allocation.

## Qualification And Predicted Cost

Host fault tests exercise full-set prevalidation, actual submit-before-poll
control flow, out-of-order completion, each partial-publication boundary,
pre/post currentness faults, timeout/range/roster rejection and terminal group
poison. These tests do not prove native overlap, peer visibility or speedup.
Require separate TP2/TP8 GPU-producer/peer-reader probes, unchanged numerical
references, native close/idle checks and matched unprofiled model measurements.

Predicted counts are not measured latency. With operational checks enabled,
one full N-rank round has two full group fences (2N full currentness checks),
instead of N separate calls with two group fences each (2N squared). Additional
all-participant operational checks still precede each publication, and incomplete
waits add checks. Setup allocation, map, model upload, host access, kernel load
and teardown retain their old boundaries and are not improved by this API.
