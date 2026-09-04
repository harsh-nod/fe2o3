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

## Current R14 Status

R14 retains the backend-neutral typed completion state, exact-once callbacks,
aggregate stream query, and bounded synchronization introduced in R11. It also
retains typed atomic and collective wrappers that match operation, scope,
success ordering, optional failure ordering, weak mode, geometry, and
collective membership before submitting an ordinary admitted typed kernel.
Compare-exchange requires a failure order without release semantics that is no
stronger than success; non-CAS operations require no failure order and
`weak = false`. Collective grids must contain only complete workgroups: every
grid dimension is at least and exactly divisible by its workgroup dimension.
Atomic launch admission retains base geometry validation and permits a partial
final workgroup. Additive backend SPIs preserve each exact contract. Ordinary
and qualification KFD constructors advertise both capability layers as false.
Separate constructors accept an unsafe semantic authority and advertise only
its enumerated non-System atomic and workgroup-collective profiles; the contract
is retained through scheduler custody, recycled-dispatch identity, and final
invocation authorization. No concrete production authority, semantic Worker
transport, native litmus evidence, or formal compiler/hardware refinement is
shipped.

The direct single-device KFD backend now admits at most 65,536 logical streams
and multiplexes their compute work over exactly two persistent native compute
queue lanes. Logical stream creation does not consume a native lane. Each lane has distinct
ring, doorbell, completion, exception-event, CWSR, dispatch, and recyclable
queue-local custody under one shared VM session. Public lane handles bind the
exact session occurrence, ordinal, and generation; destroyed slots advance
generation before reuse, and stale or cross-session handles reject. Auxiliary
destruction preflights quiescence before taking custody and follows the same
event, payload, runtime, and resource teardown order as the primary queue. The
lane callback surface exposes only admitted fixed-dispatch and observation
operations, not session-global SDMA or lifecycle control.

At most one dispatch per lane may be in flight. Accepted compute work retains
owned kernarg, binding, dependency, module, allocation, submission, and stream
custody in a bounded per-stream FIFO until it can lease the lowest available
lane. Compute and copy operations on one logical stream gain an implicit tail
dependency; cross-stream overlapping allocation use still requires an explicit
event dependency. Publication requires a FIFO head whose dependencies
succeeded, an available lane, and allocation disjointness from active native
work. Dependency count and transitive unpublished depth are capped at 256.
Prepublication cancellation removes owned work and restores the prior stream
tail; published work remains too late to cancel.

Poll remains observation-only and never performs deferred compute preparation.
Submit may publish immediately ready work. `wait` is likewise observation-only
and does not publish deferred work. The additive in-process `flush_stream`
operation may drive a dependency-ready FIFO head through potentially blocking
dirty-buffer reconciliation and native publication. Frozen Runtime Worker V1
has no flush request and is not a conforming KFD deployment; negotiated Runtime
Worker V4 exposes execution-capability discovery, flush, same-device async copy,
cancellation, and deadline-bounded drain under one exact handshake. Its
capability cache is bound to the latest successfully enumerated roster and
otherwise fails closed. Runtime transport versioning is separate from
compiler/proof Worker V3. Additive Runtime Worker V5 retains the V4 operations
and transports exact typed atomic/collective contracts; this does not supply a
production semantic authority or native proof. There is no background
native-publication scheduler, queue-side dependency packet,
or more than one in-flight dispatch per native lane. Consequently the new
logical-stream surface removes the third-stream capacity failure but is not
general HIP/HSA stream scheduling parity. The
`concurrent_compute` capability still means exact two-lane, disjoint-allocation
execution. Native peer copy remains a separate backend. Ordinary KFD atomic and
collective capabilities remain false; the unsafe semantic-authority SPI does
not supply the missing production authority.

An optional executor-neutral observer owns a runtime context on one
thread-affine engine and provides cloneable cross-thread handles plus standard
event futures. Command/waiter capacity and command/poll counts per tick are
bounded, polling follows a stable cyclic event order, and context-command and
executor-waker panics are contained. Worker-thread reentry rejects, future drop
abandons only observation, and consuming shutdown returns the context while
waking pending futures as stopped. This is completion observation only: it does
not publish deferred native work or replace explicit `flush_stream`.

The exact two-device XGMI copy-only backend retains successful peer mappings
until host access or allocation release and publishes directional copies from a
deterministic FIFO readiness queue in batches of at most 63 with caller-driven
fairness. Ready selection is O(batch), bounded by 63, and focused in-flight
selection is O(log batch), independent of the total active set. It remains
separate from the single-device compute owner; there is no unified native
multi-device compute backend.

The additive in-process `flush_stream` extension snapshots the complete ready
XGMI directional set and publishes it in FIFO prefixes of at most 63. It
synchronously completes every non-final prefix, then returns with the final
prefix published so later host work can overlap DMA. First-prefix allocation
failure is a prepublication rejection; recoverable failure after a completed
prefix is quiescent and preserves retryable custody. Poll and wait remain the
only completion-observation operations; neither publishes deferred work. There
is no background native-publication thread. Runtime Worker V4 provides the portable
capability, flush, async-copy, cancellation, and drain profile; Runtime Worker V5
retains it and adds semantic contract carriage. Runtime Worker V1 does not. The XGMI benchmark labels queued work as
`outstanding_depth`; the native route uses one ordered SDMA engine per direction
and does not claim that depth as engine concurrency.

Worker V3 now requires a second, move-only semantic-to-machine refinement
receipt before either the generated KFD application transition or deprecated
HSA lifecycle can gain execution custody. The inspect-only receipt has private
state and no public constructor. A sealed composing adapter now exposes a typed
unsafe boundary for a separately reviewed proof backend. Its request carries
the exact semantic MIR/KIR owners, final LLVM bytes, selected ISA range, final
HSACO bytes, compiler-currentness evidence, physical entry binding, and durable
publication occurrence. Only exact owned machine-effect and refinement-proof
artifacts returned by that backend can be sealed into the receipt. The receipt
binds one exact executable publication occurrence across the KIR, final LLVM
module, selected ISA range, machine-effect evidence, refinement-proof identity,
final artifact, compiler current-record and rollback chain, Worker challenge
and lineage, durable publication, proof-producer measurement, and transcript.
Its machine-effect contract is universal over checked invocations rather than
bound to one dispatch geometry. Admission consumes the
receipt and matches every coordinate. Deprecated HSA retains its custody with a
loaded executable across repeated checked invocations; direct KFD consumes it
into a one-shot application binding. Protected
backend provenance and matching digests are only necessary inputs and cannot
replace this receipt. The existing protected adapter and every synthetic path
produce no receipt. No concrete backend implementing the scalar gfx942 proof
obligations in issue #214 or corresponding authenticated proof artifact ships,
so production application execution remains deliberately unavailable by
default.

The native runtime now records process-local monotonic points immediately after
accepted AQL publication and after runtime completion processing. A fresh
`getrandom` recorder occurrence makes accidental aliasing between `Instant`
epochs cryptographically negligible, samples commit only with their exact
retained runtime-profile event, and only the live KFD finish path returns the
opaque runtime-authenticated custody bundle. Empty,
partial, lost, substituted, or stale captures do not advertise complete host
intervals. These are host observations, not GPU dispatch start/end timestamps.

Separately, the low-level KFD clock-correlation observation is one
currentness-bracketed GPU/CPU/system counter sample for calibration. It does not
mark dispatch publication, start, or completion. The generic Dispatch Timestamp
Capture V1 schema can structurally join producer-claimed CPU and device ticks,
but has no trusted producer adapter. The semantic query therefore continues to
report authenticated per-dispatch device timestamps, device clock domains, and
globally synchronized time as unavailable.

The current Rust device-language addition remains a bounded
volatile-load/store bridge rather than broad Rust support. The R12 executable
model and Verus development cover bounded abstract multi-queue custody,
generation, dependency, cancellation, drain, and currentness properties only;
the model's larger configurable queue bound is not a runtime capability. R13
adds a separate abstract logical-stream leasing model; it likewise is not a
Rust-to-Verus refinement of the concrete scheduler. These
proofs are not a refinement proof of the Rust KFD implementation and make no
Rust-to-Verus, compiler-to-ISA, firmware, or hardware refinement claim. The
runtime therefore remains below this parity profile. The additive R16 Worker V5
model raises the authenticated totals to 193 obligations and 121 rejected
mutations. It proves a reachable already-decoded request/response abstraction,
exact response custody classes, and an ordered exhaustive sidecar join, not a
parser, subprocess, Rust-to-Verus, or native refinement proof.

R17 supplies the first concrete thread-affine KFD owner capable of retaining
one mapped device-local allocation across a bounded ledger of classified uses.
It is still detached from compute AQL, local SDMA, native XGMI, live
currentness observation, and the runtime facade. Its independent executable
and Verus summary models add registry-incarnation, exact home-VM/queue,
directional route-metadata, range, hazard, dependency, timeout, and quarantine
checks. The route metadata is not bound to the persistent allocation mapping
and grants no XGMI publication authority.
The pinned totals are 225 obligations and 135 rejected mutations, but no
theorem connects those summaries to the KFD owner or native behavior. Real
Worker V4/V5 subprocess tests now show background flush completing deferred
ordinary and atomic events and sealing timeout/terminal/EOF failures; they are
host protocol tests and do not satisfy the native liveness gate.

R18 connects one persistent device allocation to one targeted local SDMA queue.
It preserves exact queue-owned buffer accounting and move-only allocation, host,
range-use, and ticket custody across confirmed publication, poll, bounded wait,
completion, and settlement. Recoverable prepublication failure restores owners;
retained or later uncertainty remains opaque until process teardown. Its
independent executable model composes the R17 registry/lease state, and its
abstract Verus summary adds 34 obligations and 24 rejected mutations for pinned
totals of 259 and 159. This single-flight low-level adapter is not connected to
the public facade or async progress engine. Exact quiescent-frontier retirement
reclaims settled history and returns stale or substituted custody unchanged;
native-neutral tests cover 66 sequential transition cycles. The tranche
supplies no persistent compute, XGMI, concurrency, hardware-execution,
refinement, or performance evidence.

R19 adds the exact directional pair needed by the current KFD runtime queue
shape: distinct engine-1 H2D and engine-0 D2H children under one parent queue
occurrence. Pooled allocations retain separate logical and page-rounded
physical extents, copy ranges are logical-bound, and exact frontier retirement
is required before arbitrary repeated or mixed-direction reuse. Explicit
retryable versus process-teardown custody covers promotion, demotion,
publication, completion, and currentness failure. The active packet path uses
the bounded operational currentness fence, not full topology discovery.

The independent R19 executable model and abstract Verus proof add 46
obligations and 20 rejected mutations for pinned totals of 305 and 179. No
theorem connects them to executable Rust or native behavior. The adapter is not
connected to the public facade or async progress engine, supports neither H2H
nor D2D, and remains single-flight with a `0x003f_ffe0`-byte packet cap. It has
no hardware-execution or matched HIP/HSA performance evidence and therefore
does not satisfy this parity profile.

## Current R20 Status

R20 integrates the R19 owner into direct-KFD runtime allocation and same-device
copy state. It admits native H2D and D2H only, preserves exact host/device and
terminal custody, chunks logical ranges at `0x003f_ffe0`, and requires exact
completion, settlement, and frontier retirement before continuation. H2H and
D2D reject before facade mutation. Poll and wait observe only; explicit flush
publishes continuations. Cancellation is limited to an unpublished transfer
with zero completed bytes.

The concrete facade distinguishes conclusive failure before the first packet
from quiescent-without-result after partial device mutation. The latter releases
all native and scheduler retains before installing a pre-reserved exact
submission marker. That marker remains observable through poll, wait, drain,
events, dependencies, stream destruction, release, shutdown, and Worker V4/V5
transport. Packet-bounded staging replaces allocation-sized temporary buffers
for synchronous copy, zero, scrub, and dirty-shadow reconciliation.

The independent R20 executable model has 14 focused tests, and its abstract
Verus summary adds 31 obligations and 15 rejected mutations for pinned totals
of 336 and 194. No theorem connects the model to the Rust facade or native KFD
execution. Runtime-layer scripted failure injection for every move-only R19
custody result remains open. R20 also lacks H2H, D2D, batched local publication,
shared persistent compute/XGMI storage, direct-backend background progress,
hardware correctness, and matched performance evidence. It therefore does not
satisfy this parity profile.

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
After a backend accepts a nonblocking submission, every dependency-ready stream
head must be able to reach native publication and completion through a declared,
portable progress mechanism, without an undocumented backend-specific call. A
backend may use native queue-side dependencies, an owned background progress
mechanism, or an explicit portable flush operation. If flush is required, every
advertised transport must expose it with bounded blocking and failure semantics
defined by this profile and exercised by parity tests. Poll remains nonblocking;
wait may drive progress only within its deadline. Tests must cover more logical
streams than physical lanes and multiple queued mixed compute/copy operations
per stream.
Submission and event observation share one typed completion state. Completion
callbacks discharge exactly once on the first conclusive transition, and
aggregate stream synchronization applies one shared deadline without obscuring
failed or quiescent-without-result submissions. Nonterminal wait errors do not
prevent later pending submissions from receiving their one bounded wait;
terminal ambiguity stops immediately.

The additive runtime async progress mode is one declared portable mechanism for
Send-capable backends: a bounded registered-stream roster receives bounded,
cyclic `flush_stream` attempts on the owner thread while event observation keeps
its independent budget. Ordinary async-engine construction remains
observation-only. Direct KFD is thread-affine and therefore still requires
caller-driven flush. These host tests establish transition behavior, not native
liveness or scheduling parity.

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
