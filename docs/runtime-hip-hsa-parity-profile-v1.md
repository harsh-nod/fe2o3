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

## Current R12 Status

R12 retains the backend-neutral typed completion state, exact-once callbacks,
aggregate stream query, and bounded synchronization introduced in R11. It also
retains typed atomic and collective wrappers that match operation, scope,
success ordering, optional failure ordering, weak mode, geometry, and
collective membership before submitting an ordinary admitted typed kernel.
Compare-exchange requires a failure order without release semantics that is no
stronger than success; non-CAS operations require no failure order and
`weak = false`. Collective grids must contain only complete workgroups: every
grid dimension is at least and exactly divisible by its workgroup dimension.
Atomic launch admission retains base geometry validation and permits a partial
final workgroup. These are facade semantics, not a native-operation or
authority claim: current KFD backends advertise both stable and
execution-detail atomic/collective capabilities as false, so the wrappers
reject before KFD submission.

The direct single-device KFD backend now assigns the first two live logical
streams to two persistent native compute queue lanes. Each lane has distinct
ring, doorbell, completion, exception-event, CWSR, dispatch, and recyclable
queue-local custody under one shared VM session. Public lane handles bind the
exact session occurrence, ordinal, and generation; destroyed slots advance
generation before reuse, and stale or cross-session handles reject. Auxiliary
destruction preflights quiescence before taking custody and follows the same
event, payload, runtime, and resource teardown order as the primary queue. The
lane callback surface exposes only admitted fixed-dispatch and observation
operations, not session-global SDMA or lifecycle control.

At most one dispatch per lane may be in flight, and the two lanes may publish
concurrently only when their allocation sets are disjoint. A third live stream
is rejected with capacity before native queue creation; after an owning stream
is quiescent and destroyed, its logical lane can be reassigned. A pending
compute event dependency rejects as busy before publication, so the caller must
poll or wait for the dependency and retry the launch. R12 does not provide
native queue-side dependency scheduling, arbitrary stream counts, multiple
queued dispatches per stream, or overlapping-memory compute concurrency. The
`concurrent_compute` capability therefore means this exact two-lane profile,
not general HIP/HSA stream parity. When the exact native profile is available,
the direct backend also reports only its implemented native async-copy,
compute-copy-overlap, memory-pool, and cancellation surfaces; native peer copy
remains a separate backend, and atomic and collective execution remain false.

The exact two-device XGMI copy-only backend retains successful peer mappings
until host access or allocation release and publishes directional copies from a
deterministic FIFO readiness queue in batches of at most 63 with caller-driven
fairness. Ready selection is O(batch), bounded by 63, and focused in-flight
selection is O(log batch), independent of the total active set. It remains
separate from the single-device compute owner; there is no unified native
multi-device compute backend.

The additive in-process `flush_stream` extension publishes one complete ready
XGMI directional batch before returning, so later host work can overlap that DMA
without waiting for the first poll. A ready set larger than the 63-ticket ring
admission rejects before publication. Poll and wait remain the fallback and the
only completion-progress operations; there is no background progress thread and
Worker V3 carries no flush request. The XGMI benchmark labels queued work as
`outstanding_depth`; the native route uses one ordered SDMA engine per direction
and does not claim that depth as engine concurrency.

Worker V3 now has a public, move-only application execution binding on the
generated KFD invocation path. The production transition retains the exact
authenticated executable and current-publication token and binds compiler and
proof evidence, target and code object, generated argument packing, kernel and
dispatch identities, launch geometry, and one checked device. Its fields and
constructor remain private, currentness is revalidated, and the synthetic test
verifier path retains qualification custody that cannot execute as production.
This is the application-side binding and release transition, not the missing
semantic or machine-refinement verifier. A reviewed production producer for the
required semantic-to-machine proof and protected verifier decision remains
absent, so ordinary compiler-produced applications still cannot use this path
to manufacture production execution authority.

The low-level KFD clock-correlation observation is one currentness-bracketed
GPU/CPU/system counter sample for calibration. It does not mark dispatch
publication, start, or completion and is not a per-dispatch device timestamp.
The current Rust device-language addition remains a bounded
volatile-load/store bridge rather than broad Rust support. The R12 executable
model and Verus development cover bounded abstract multi-queue custody,
generation, dependency, cancellation, drain, and currentness properties only;
the model's larger configurable queue bound is not a runtime capability. These
proofs are not a refinement proof of the Rust KFD implementation and make no
Rust-to-Verus, compiler-to-ISA, firmware, or hardware refinement claim. The
runtime therefore remains below this parity profile.

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
Submission and event observation share one typed completion state. Completion
callbacks discharge exactly once on the first conclusive transition, and
aggregate stream synchronization applies one shared deadline without obscuring
failed or quiescent-without-result submissions. Nonterminal wait errors do not
prevent later pending submissions from receiving their one bounded wait;
terminal ambiguity stops immediately.

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
Compare-exchange separately binds success order, failure order, and weak mode;
failure order has no release semantics and is no stronger than success, while
non-CAS operations admit neither a failure order nor weak mode.
System-scope and XGMI-visible claims require native litmus evidence; structural
instruction matching alone is insufficient.
Every grid dimension must contain at least one whole workgroup and divide
exactly by its workgroup dimension; partial tail workgroups are not admitted.
This complete-tiling restriction applies to collectives, not atomic launches.

Typed facade contracts and ordinary typed-kernel submission are necessary but
not sufficient for this gate. A backend must advertise both the stable and
execution-detail capability, and the resulting execution must still carry the
authenticated identity and native evidence required above.

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
Clock-correlation calibration samples must be reported separately from actual
per-dispatch device timestamps and cannot substitute for them.

## Claim Rule

V1 parity may be claimed only when every in-scope gate has current functional,
negative, proof, hardware, and performance evidence for the exact commit. The
release notes must publish the numerical latency and bandwidth thresholds used
for the claim. Source presence, unit tests, a successful code-object build, or a
single favorable benchmark cannot substitute for the complete gate.
