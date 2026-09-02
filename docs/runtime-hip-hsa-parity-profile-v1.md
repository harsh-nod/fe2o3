# Runtime HIP/HSA Parity Profile V1

Status: implementation candidate; no parity claim is made by this document.

## Scope

This profile defines the bounded runtime behavior that fe2o3 must qualify before
using the phrase "HIP/HSA runtime parity". It is behavioral parity for the
fe2o3 execution surface, not symbol-for-symbol compatibility with every HIP or
ROCr API.

The profile includes:

- authenticated, typed, asynchronous kernel launch;
- multiple independent streams and more than one in-flight dispatch per GPU;
- events, cross-stream dependencies, bounded waits, cancellation, and drain;
- persistent device-local and host-visible allocations with explicit lifetime,
  access, coherence, and reuse rules;
- asynchronous same-device H2D, D2H, and D2D copies using admitted SDMA paths;
- asynchronous peer copies using admitted native XGMI routes;
- local multi-device execution, topology-current routing, and failure isolation;
- the closed atomic, barrier, Wave64 reduction, and scan language admitted by
  the compiler and Worker V3 authority;
- deterministic cleanup, terminal-ambiguity containment, reset reporting,
  profiling timestamps, and capability discovery; and
- correctness and performance comparison against pinned HIP and HSA baselines.

HIP graphs, RTC, image and texture objects, graphics interop, managed memory,
virtual-memory remapping, IPC, and multi-host transport are outside V1. A
backend must report these as unsupported; it must not emulate them silently or
advertise their capability.

## Required Gates

### G1: API and ownership

All public handles are context-generation bound. A live submission or event
retains every stream, module, allocation, mapping, queue, and completion object
that native work may still reference. Rejected operations do not mutate native
state. Quiescent failures permit retry or release. Ambiguous failures seal the
owning backend and retain possible native references.

### G2: asynchronous execution

Two independent streams on one device can each retain an in-flight compute
dispatch. Copy and compute can overlap when the admitted device exposes the
required queues. Dependencies are explicit events; submission is nonblocking;
poll never blocks; wait observes a monotonic deadline. Progress storage is
bounded by declared queue, submission, dependency, and staging limits.

### G3: memory and copies

Runtime allocations are persistent native allocations, not allocation-wide
host byte vectors materialized for each launch. H2D, D2H, D2D, and peer copies
use the admitted native copy engine. Range, overlap, mapping-currentness,
topology-currentness, direction, engine, and completion identities are checked
before publication. Async free is represented by dependency plus release after
quiescence; reuse cannot occur before the prior generation is released.

### G4: multi-device

Each device owns independent queue and allocation state. Native XGMI routes are
selected from the current directional topology and support every admitted local
GPU pair rather than a hard-coded pair. Cross-device event use is accepted only
by an operation whose contract explicitly spans both devices. Reset, partition,
or topology changes invalidate stale routes without freeing ambiguous work.

### G5: atomics and collectives

The source operation, memory order, memory scope, address space, width, return
value, fences, and machine instruction sequence are part of one authenticated
execution identity. Collective admission additionally binds wave size,
participant mask, convergence point, LDS extent, barriers, and result layout.
System-scope and XGMI-visible claims require native litmus evidence; structural
instruction matching alone is insufficient.

### G6: executable refinement

The Rust implementation has executable transition tests against the pure model.
Verus proves the corresponding bounded safety properties and pinned negative
mutations demonstrate that each critical premise is necessary. Compiler/ISA,
KFD firmware, and hardware semantics remain named external contracts unless a
separate authenticated refinement closes them.

### G7: qualification

The complete supported kernel corpus passes differential correctness testing
on each admitted target. Copy and dispatch benchmarks use the same allocation
kind, transfer direction, byte range, warmup, queue depth, synchronization
boundary, and sample count for fe2o3, HIP, and HSA. Reports include p50, p95,
throughput, failures, GPU topology, clocks, utilization, thermal state, ROCm and
kernel versions, exact commit, and raw samples. A busy shared GPU invalidates a
performance result rather than producing a parity claim.

## Claim Rule

V1 parity may be claimed only when every in-scope gate has current functional,
negative, proof, hardware, and performance evidence for the exact commit. The
release notes must publish the numerical latency and bandwidth thresholds used
for the claim. Source presence, unit tests, a successful code-object build, or a
single favorable benchmark cannot substitute for the complete gate.
