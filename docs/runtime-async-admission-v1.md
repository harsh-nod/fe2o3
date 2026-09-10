# Standalone Payload Admission And Executors: R64

This advances local A1/A2 work for #182. It does not complete the distributed
execution plane, admit compiler plans, establish data versions, or demonstrate
HIP/HSA performance parity.

## Frozen Launch Requests

`RuntimeAsyncLaunchRequestV1::new` takes the stream, typed kernel, borrowed
arguments, geometry and dependencies. It evaluates the two argument getters
once on the calling thread and owns compact boxed byte, binding and dependency
slices. Encoder panic remains on that thread; no owner command has been admitted.
There is no atomic snapshot of independently mutable application state.

The constructor rejects oversized kernargs, excess bindings, more than 256
dependencies and duplicate dependency identities. It does not grant execution
authority. `enqueue_launch` and `enqueue_launch_tracked` consume this request;
the owner uses the same context launch validators, current resource identities,
alias/geometry/pointer checks and backend admission as ordinary launches.
Failed enqueue consumes the request, consistently with existing operation APIs.

Legacy `launch`/`launch_tracked` retain owner-thread argument evaluation and
existing panic semantics. All six standalone launch/copy/peer variants now
preflight dependency length/duplicates before enqueue and retain only the
compact slice, not the caller's spare vector capacity. Malformed lists return
`InvalidSnapshot` synchronously; resource identity validation remains owner-side.

## Payload Budget

`RuntimeAsyncEngineConfigV1::with_snapshot_byte_capacity` sets a shared per-engine
payload limit: 16 MiB by default, with a 1 GiB hard maximum. Frozen launches charge
the lengths of encoded bytes, bindings and dependencies. Legacy launches and
copies charge only their compact dependency slices. All cloned handles share
the same checked atomic counter; excess admission returns `SnapshotCapacity`.
`snapshot_bytes_in_use` is descriptive telemetry, never native resource authority.

A non-cloneable permit follows the exact payload through the command channel and
operation registry. The payload drops before the permit returns its exact charge.
Future abandonment, timeout and a cancellation request do not release that
charge while the owner still retains the payload. Channel/registry rejection,
pre-submission cancellation disposal, validation failure, completed submit, panic
and shutdown disposal refund it only when that payload actually drops.

This is not an end-to-end process or GPU memory bound. It excludes:

- caller-owned requests and allocations inside user argument encoders;
- legacy argument objects and arbitrary context callbacks/results;
- allocator overhead, fixed operation records and shared kernel metadata;
- the separate fixed-bound graph snapshot profile;
- native/context submission resources and allocation/executable pools;
- caller-retained completed replies and cumulative quarantine.

Owner validation temporarily copies the frozen bytes/bindings and constructs
backend descriptors. That scratch is bounded by one request's existing context
limits and is additional to the retained-payload counter. Returning a payload
permit after submit does not retire a submission, release a device resource or
grant retry permission. Full end-to-end byte admission remains open.

## Graph Occurrences

Every admitted graph terminal report now carries a
`RuntimeGraphExecutionIdentityV1`: validated completion context identity,
structural graph identity and the fresh private reservation generation. Identical
graphs in the same context therefore have distinct increasing execution
generations. Other context operations may leave gaps. Rejections and indeterminate
outcomes produce no terminal graph report.

The fields have no public constructor and the value cannot be converted to a
reservation token. It identifies one local occurrence, not bound argument bytes,
a data version, a device-reset generation, a compiler plan or distributed epoch.
Exact producer versions still need graph-local derivation checks and, for
cross-run consumption, context-wide mutation/invalidation tracking.

The subsequent [R65 contract](runtime-async-drain-versions-v1.md) supplies the
graph-local derivation/checks and a separate retained-reply count budget.
Cross-run mutation authority and full byte/native-resource budgets remain open.

## Verification Boundaries

The eight R64 abstract arithmetic obligations cover exact reservation, capacity
and overflow rejection, exact release, underflow rejection, round-trip,
boundedness and zero-charge behavior. Production CAS updates call the matching
Rust helpers. Tests compare their machine-width results with wide-integer
arithmetic. Eight targeted mutations must fail the authenticated proof gate;
the overflow mutation models wrapping addition on valid machine-domain inputs.

These are arithmetic proofs with reviewed Rust correspondence, not proofs of
atomic linearization, lease ownership, threads, allocator behavior, termination,
GPU completion or the entire executor. Concurrency and actual payload lifetime
are separately tested; backend completion truth remains contracted.

Executor regressions use dev-only Tokio 1.47.1 (current-thread and multi-thread)
and futures-executor 0.3.34 LocalPool. They pass real executor wakers to initially
pending operations, exercise Send observation with a !Send owner backend,
LocalPool non-Send observation, task abortion and real timer recovery of the same
queued operation. Watchdog expiry cannot count as a successful owner wakeup.
No executor library becomes a production runtime dependency.

## Remaining Acceptance

#182 still requires compiler-authenticated plans and semantic-to-machine
refinement from #134/#214; cross-run versions, residency and measured overlap;
end-to-end budgets and complete native drain qualification; integrated multi-GPU
placement/shards/replicas/group quiescence; authenticated two-host control/data
transport and collectives; authorized reset/partition/failure campaigns; and
precommitted all-GPU/two-host performance gates. Named-executor tests validate
integration, not executor fairness or production hardware throughput.

The [R64 evidence report](evidence/mi300x-r64-admission-2026-09-09/README.md)
retains the CPU/executor and authenticated proof results, two guarded MI300X
copy-DAG regressions, the rejected offline-cache attempt and independent
capture/cleanup audits. Its hardware scope excludes compute and nonzero frozen
launch payload budgets.
