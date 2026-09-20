# XGMI Currentness Diagnostics

The opt-in `fe2o3-kfd/hardware-diagnostic` feature adds owner-free host
intervals for a full directional XGMI pair check. This is a lower-level
measurement API, not a relaxation of currentness or new execution authority.

## API

- `Gfx942NativeXgmiSdmaQueueV1::begin_batch_currentness_diagnostic_v1` returns
  the existing batch owner and its opening pair observation.
- `Gfx942NativeXgmiSdmaBatchV1::finish_currentness_diagnostic_v1` consumes the
  same owner and returns the closing pair observation on success.
- `finish_terminal_currentness_diagnostic_v1` performs the closing check and
  quarantines the queue and both sessions, including on successful validation.

The original begin, finish, terminal finish, and batch `Drop` remain available.
There is no diagnostic wrapper owner or alternate cleanup path. Ordinary and
diagnostic halves can be mixed; each returned observation describes only its
own check. An upper-level capture must require both halves and bind them to the
same successful operation after settlement and explicit teardown. It must not
abandon a live batch or change a workload result because timing is unavailable.

This change does not yet add that runtime recorder integration, a benchmark
flag, or a native performance result. Existing diagnostic V1 schemas are
unchanged.

## Intervals

`Gfx942XgmiPairCurrentnessDiagnosticsV1` reports source and peer prechecks,
fresh discovery, route admission plus both retained-snapshot comparisons,
source and peer postchecks, and total timed host interval. The pair total
starts after device pre-latching and ends before those latches are cleared;
outer binding, session-guard, and queue-owner checks are not part of it.

Its nested `Gfx942TopologyDiscoveryDiagnosticsV1` splits discovery into:

1. Complete topology-tree traversal, including all GPUs and IO/P2P links.
2. Initial boot, kernel-release, and amdgpu-module observations.
3. Root validation/canonicalization and all render-node, PCI, unique-ID, and
   partition correlation.
4. Closing generation, boot, release, and module observations.

`Option<u64>` represents nanoseconds; missing, repeated, inverted, or
unrepresentable intervals invalidate diagnostics. `is_complete` checks all
fields, checked sums, and nested containment. The nested phases are already
included in discovery time and must not be added again to the outer total.
Structs are ordinary forgeable data, not authenticated evidence or authority.

## Compatibility And Tests

Both modes instantiate the same generic control flow. Disabled timing is a
zero-sized type with no clock reads. Enabled timing adds no filesystem reads,
observation reordering, topology caching, or heap allocation of its own.
Errors and panics retain their original causes; diagnostic finalization occurs
before device poison latches are cleared. Session guards and batch quarantine
semantics remain unchanged.

CPU tests compare both modes' full 32-callback pair sequence, every callback
error and panic, original boxed panic identities, endpoint/snapshot/route
rejections, prior poison, and latch order. A final-diagnostic panic test checks
that devices and sessions stay terminal. Complete host fixtures cover one,
two, and three GPUs, both link sets, optional module fields, 17 static failure
cases, four closing-identity changes, fresh reads, and panics at every traced
I/O boundary. The trace records selected Rust I/O calls and bounds, not kernel
syscall counts or an atomic driver snapshot.

The [CPU qualification packet](evidence/dev-xgmi-currentness-diagnostic-cpu-2026-09-20/PROTOCOL.md)
binds these tests to source bytes, command receipts, and exact test rosters.
This is regression evidence, not machine-code refinement, Linux/driver
atomicity, HIP/HSA parity, or a speedup claim.

The previous aggregate measurement attributed about 98.8% of host time to
opening and closing currentness. These finer intervals are intended to locate
that cost; their native measurement and runtime capture remain the next step.
