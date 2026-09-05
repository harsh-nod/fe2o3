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
execution. R20 also lacks H2H, D2D, batched local publication, shared persistent
compute/XGMI storage, direct-backend background progress, hardware correctness,
and matched performance evidence. It therefore does not satisfy this parity
profile.

## Current R21 Status

R21 adds a private runtime-layer scripted driver for the facade's complete
move-only directional SDMA transition boundary. Sixteen concrete tests cover
retryable and teardown custody, dependency and observation states, exact
direction/length completion metadata, retirement-failure handling, partial H2D
and D2H progress, cleanup, scrub, and abort-on-live-owner behavior. Driver
identity is checked before FIFO consumption, and mixed native/scripted custody
fails closed. This testing found and fixed a D2H classification error:
retryable continuation after an earlier host mutation now becomes quiescent
without result rather than retryable.

The independent R21 model has 17 focused tests and distinguishes host-dirty
from device-dirty progress. Its Verus artifact adds 37 obligations and 15
rejected mutations for pinned totals of 373 and 209. There is still no theorem
connecting that model to the Rust facade or native KFD behavior, and the seam
does not perform native fault injection. R21 therefore improves failure-atomic
facade evidence but does not satisfy this parity profile. H2H, D2D, batched
local publication, unified persistent compute/XGMI storage, hardware
correctness, and matched performance evidence remain open.

## Current R22 Status

R22 replaces the facade's packet-at-a-time local H2D and D2H publication with
bounded directional windows. One window owns one aggregate allocation lease and
one host/device pair, contains one through 63 canonical contiguous SDMA packets,
and becomes visible through one write-pointer publication and one doorbell. The
lower KFD owner authenticates the complete ordered ticket roster and retains the
whole window across pending and timeout observations; it does not retire packet
prefixes. Exact completion restores the owner pair, settles one frontier, and
requires exact frontier retirement before the facade publishes a continuation.
A 256 MiB transfer is therefore 65 packets published as two windows of 63 and
two packets, with an exact 2,048-byte tail.

The runtime seam validates canonical request order before publication and then
validates KFD-authenticated direction, host and device offsets, byte length, and
packet count at pending, failure, and completion boundaries. Recoverable
rejection before the first publication restores retryable custody;
a recoverable rejection after an earlier completed window becomes quiescent
without result. Ambiguous or substituted published custody remains terminal.
Synchronous staging remains packet-bounded but uses the same one-request window
path.

The independent R22 executable model has 18 focused tests. Its abstract Verus
artifact adds 41 obligations and 19 rejected mutations for pinned totals of 414
and 228. The proof models one publication and doorbell transition per window,
but it is not a theorem about the executable Rust, CPU atomics, mapped-memory
ordering, firmware consumption, DMA, or hardware completion. Native-neutral KFD
and facade tests cover window planning, ring wrap, exclusive occupancy, exact
rosters, custody, and failure classification. No MI300X result has yet qualified
this exact commit. R22 closes batched local publication only; H2H, D2D, unified
persistent compute/XGMI storage, background native progress, hardware
correctness, and matched performance evidence remain open. It therefore does
not satisfy this parity profile.

## Current R23 Status

R23 adds same-device D2D to the persistent local SDMA path. Each window moves
one source-read lease and one destination-write lease from two distinct
device-local allocation owners onto the fixed H2D child of the exact R19/R22
directional queue pair. Admission binds one device, VM, parent queue
occurrence, child queue occurrence, allocation generation, and two distinct
storage identities. Identical allocations, overlapping mapped GPU ranges,
noncanonical packet rosters, and windows outside one through 63 packets reject
before native publication.

One accepted window performs one release write-pointer publication and one
doorbell. Poll is observation-only, and pending or bounded-timeout outcomes
retain both allocation owners and both published leases. Completion must
authenticate the exact aggregate offsets, byte length, packet count, and lower
ticket record before either owner can advance. The two completed leases settle
as a pair, and exact paired frontier retirement is required before both native
allocations return to the runtime. A mismatch after possible publication
quarantines both owners and poisons the session; no source or destination
prefix is independently released.

The public runtime facade derives D2D from two device-local regions, reuses the
R22 packet and 63-packet window planner, restores paired custody after every
window, and marks only the destination shadow device-dirty after authenticated
completion and retirement. Continuations remain caller-driven through
`flush_stream`. Native-neutral KFD and scripted facade tests cover success,
retry, timeout, substitution, retirement failure, cleanup, and terminal
containment. The independent executable model has 24 focused tests. Its
abstract Verus artifact adds 46 obligations and 28 rejected mutations for
pinned totals of 460 obligations and 256 rejected mutations. There is no
theorem connecting those artifacts to executable Rust, CPU memory ordering,
SDMA firmware, or hardware execution.

The matched depth-one benchmark initializes and poisons two persistent device
allocations outside timing, measures public-facade enqueue/flush through
observed completion against HSA and HIP D2D operations, and then performs
untimed D2H validation of both physical source and destination contents. The
runner binds its output to the exact commit, both frozen SDMA manifests,
software/device identity, and load boundaries. No MI300X result has yet
qualified R23, so this tranche makes no correctness, performance-parity, or
speedup claim. Unified persistent compute/XGMI storage, direct-backend
background progress, production atomic/collective authority, broad Rust device
language, hardware refinement, and the remaining gates below are still open.
H2H remains unsupported but is outside the V1 copy-engine list.

## Current R24 Status

R24 closes a concrete portable-progress liveness gap in R23's multi-window copy
path for Send-capable runtime backends. `event_future_with_progress` atomically
admits one event waiter and its exact source stream into the async engine. The
engine polls events before its independently bounded, cyclic stream-flush pass,
so observing completion of the first 63-packet window can make the two-packet
continuation ready for publication in the same tick. Duplicate, capacity,
invalid-handle, and event/stream mismatch failures install neither half. A
conclusive event result stops and removes only the exact paired progress entry
before the future becomes ready, while logical resource and native custody stay
subject to the existing explicit-release rules. Drop abandons observation and
future flushes without cancellation, release, or a final progress attempt.

Direct KFD remains non-`Send` and thread-affine. R24 therefore adds a
feature-gated, copy-only Worker V5 qualification child that creates, serves, and
explicitly shuts down its KFD owner on the child's main thread. Only the
canonical empty frame reaches graceful native shutdown; malformed or truncated
input returns through the existing fail-closed child boundary. The parent can
own the Send-capable Worker adapter in the progress engine without moving a KFD
object across threads. An ignored gfx942 test submits a 256 MiB same-device D2D
copy, asserts the exact 65-packet `63 + 2` plan and 2,048-byte tail, performs no
caller `flush_stream`, and validates a boundary-sensitive absolute-offset
payload in both physical allocations after completion. The ignored test passed
on one idle MI300X device at exact R24 commit `0631c5be`; the retained record is
[`mi300x-worker-v5-portable-progress-2026-09-04.md`](evidence/mi300x-worker-v5-portable-progress-2026-09-04.md).

The independent executable R24 model has 16 focused tests. Its abstract Verus
artifact adds 34 obligations and 19 rejected mutations for pinned totals of 494
obligations and 275 rejected mutations. The model distinguishes active progress
membership from retained logical event, stream, and native custody; it covers
atomic admission, independent event/stream duplicate rejection, bounded active
capacity over bounded append-only history, independently bounded cyclic poll and
flush visits, poll-gated continuation, retryable-poll custody with observation
retirement, retryable-flush membership, terminal progress retirement,
abandonment, and stop without final progress. Executable visit-counter overflow
is preflighted before phase or cursor mutation. No theorem connects it to
executable Rust, Worker transport, KFD, firmware, or hardware. The separate
MI300X result establishes one exact native completion and data-correctness run;
it makes no parity, fairness, general liveness, bandwidth, latency, or speedup
claim. Unified persistent compute/XGMI storage, production atomic/collective
authority, broad Rust device language, hardware refinement, and the remaining
qualification gates stay open.

## Current R25 Status

R25 connects one narrow persistent local allocation to ordinary fixed compute.
An exact full-allocation H2D result over fresh or exact-size pooled storage may
be promoted into content-authenticated readiness and bound as the only global
buffer of one fixed compute packet on the primary compute lane. The KFD adapter
preserves the same mapped HBM storage and its allocation/use/dispatch generations through
publication, pending observation, exact completion, signal recycle, dispatch
detach, native restoration, and frontier retirement. The runtime preserves the
host source separately, selects lane zero only, and keeps the ready owner
pending while that lane is busy.

Admission requires ordinary semantics, one device-local binding at offset zero,
equal nonzero logical and physical extents, at most one 63-packet H2D window,
page alignment, no unresolved native or SDMA shadow dirtiness, and equality
between the low-level authenticated H2D digest and the runtime shadow digest.
The selected path cannot enter generic materialization. It serializes against
all published SDMA work and all active compute lanes on the same queue; no
overlap is admitted by this bounded bridge. Address-free launch
performance reports `PersistentDeviceReused` with zero user-data
materializations. Metadata-derived read and read/write effects require the
authenticated initialization premise; any write invalidates the host shadow
after exact completion. Retryable prepublication failure restores the ready
owner. A no-effect full-ring publication remains pending in prepared custody
for explicit progress, while prepublication cancellation withdraws it and
restores the exact H2D-ready allocation without emitting false publication or
completion profile events. Foreign-queue rejection returns the exact receipt,
and later ambiguity retains opaque native custody for process teardown.

The independent executable R25 model has 17 focused tests. Its abstract Verus
artifact adds 38 obligations and 18 rejected mutations for pinned totals of 532
obligations and 293 rejected mutations. The proof requires exact full extent,
derived authorization, initialized reads, no selected-path fallback, pending
and retryable custody, completion-coordinate authentication, absorbing
quarantine, exact restoration, and exact frontier retirement. It does not prove
the Rust implementation, metadata truth, KFD/firmware behavior, hardware
execution, liveness, or performance.

This is not G3 completion or HIP/HSA parity. Partial and padded pooled ranges,
multiple bindings, auxiliary lanes, persistent XGMI sharing, unified multi-device
compute, production atomic/collective authority, broad Rust device language,
and matched hardware qualification remain open. No R25 performance ratio or
orders-of-magnitude claim exists without a retained matched measurement.

## Current R26-R35 Status

R26 adds the matched, counterbalanced direct-KFD/raw-HSA/HIP qualification
harness at exact commit `8953f757c6771823e5132708f45a43c32f459081` for one
1 MiB in-place `u32` transform on one idle MI300X. R27 retains and replays one
exact persistent dispatch control instead of reconstructing it for each launch
(`f0ec1c3acc57c1bb86f8b33da651bc1f3f543113`, with the independent model at
`e659d148c46221163aa36258d4279237c8d51e25`). R28 scopes retained-control hot
replay to operational currentness observations while keeping full audits at
lifecycle boundaries (`1fb39f8301d1859ab136058255bc58d41eed66e8`, model
`c9dc306aa2071656a0e1011438955502a8d5ef46`). R29 narrows each active retained
queue observation to opener PID, reset-event readiness, and a dedicated
VRAM-loss-counter comparison at
`7970da879291292bcf02fd99a07b5f3c9a3b6427`; full identity, topology, aperture,
and process-incarnation audits remain lifecycle operations.

R30 binds a full-write host-content certificate before H2D and consumes it only
after exact completed-H2D custody and promotion currentness checks
(`dc1887e9999135e450e53e99a8d3d99bf933c689`, model
`9e78aafa27e3c737f6a6e491a5c0fa6bbe190ff1`). In the bounded R26 V4 workload,
the retained [R30 evidence](evidence/mi300x-r30-authenticated-h2d-2026-09-05.md)
reports a 60.92% median unadjusted E2E reduction against the exact R29 baseline,
but the R30 KFD path remained about 3.30x-3.34x slower than HIP E2E. R31 routes
copies no larger than one gfx942 linear packet through the existing scalar
owner path (`2f95b4619a6ca95cd37159821429d9db196d5550`, model
`ce54ae7b06d51b5c2cd5844103def858ec93d6b7`). Its retained
[R31 evidence](evidence/mi300x-r31-single-packet-2026-09-05.md) found no
meaningful speedup and did not support one-packet vector construction as the
dominant bottleneck in that run.

R32 fuses directional preparation and publication under one owner/memory loan,
uses one shared currentness observation before an immediate no-fail handoff,
and retains the final post-publication close at exact production commit
`9f715189b8f35d4adb58be303900f937d88389ad`. The retained
[R32 evidence](evidence/mi300x-r32-currentness-handoff-2026-09-05.md) measures
the same R26 V4 1 MiB workload and reports a 9.32% median slot-matched
unadjusted E2E reduction against R31. KFD still measured approximately 3.01x
slower than HIP E2E in every slot. These are `n=3` descriptive cross-revision
effects for one workload, not a generic parity, application-speedup, or
orders-of-magnitude result.

R33 adds a fused synchronous directional helper at production commit
`f25000bec19d45229a4b9ab531457d70f7977e3d`, but the R26 harness uses the
public asynchronous copy path and a separate wait. The retained
[R33 control evidence](evidence/mi300x-r33-synchronous-fusion-regression-control-2026-09-05.md)
therefore records a clean asynchronous-path regression control, not a measured
execution or performance attribution for that helper. R34 fuses asynchronous
single-copy admission, detach, lower preparation, publication, and custody
recovery at production commit
`b015b81f862220d48671e1c4809b8ce858a317e7`. Its retained
[R34 evidence](evidence/mi300x-r34-asynchronous-single-copy-fusion-2026-09-05.md)
reports a 4.71% median slotwise unadjusted E2E reduction against R33, led by
D2H, while compute regressed by about 1%. That comparison's archived Rust
version fields were later shown to describe the caller working directory rather
than the private archived build tree, so they cannot establish build-toolchain
identity.

R35 fuses the retained fixed-dispatch-control replay bind under one live queue
memory-model loan at exact production commit
`4b324bbd53e4c6e767c5c5f2f18817c133edbe03`. The matched
[R35 evidence](evidence/mi300x-r35-retained-control-replay-fusion-2026-09-05.md)
uses the corrected runner for both the exact unchanged-R34 baseline and R35.
The directly enclosed native-binding p50 improved in all three slots by
6.77%-10.19%, with a 7.03% median slotwise reduction. Median slotwise E2E
movement was only 0.68% raw, 0.78% HSA-adjusted, and 0.82% HIP-adjusted. R35
still measured 2.84x-2.88x slower than HIP E2E and 3.52x-3.55x slower than HIP
compute. One elevated clean-monitor baseline compute slot and sequential
revision ordering limit attribution. The result is descriptive for one
workload, not causal, parity, orders-of-magnitude, or workload-general evidence.

The authenticated aggregate formal runner through R35 includes the R27, R28,
and R30-R35 independent bounded models. At exact proof commit
[`91475313b441c6ab691e39fa7fe1bf2441827681`](https://github.com/harsh-nod/fe2o3/commit/91475313b441c6ab691e39fa7fe1bf2441827681),
the complete included proof set establishes 808 obligations and rejects 322
pinned expected-negative mutations; final transcript SHA-256 is
`40090d573642767f00ac742264c97ecc177e5b6c8555d40a0813912ebf8c2ad5`.
R33 contributes 45 obligations and four coupled negatives for the mathematical
synchronous fusion relation. R34 contributes 54 obligations and four negatives
for premised asynchronous fusion equivalence. R35 contributes 13 positive
obligations and four standalone negatives. Unlike R34's external-equivalence
relation, R35 proves only a premised projection of former and fused custody and
exact commit coordinates; it excludes production/public error identity,
terminal failure stage, internal authority labels, event indices, and loan and
currentness counts from that relation. The proof inputs contract currentness,
lower outcomes, identities, certificates, tickets, and loan results; there is
no Rust-to-Verus correspondence or proof of syscalls, driver, firmware,
hardware, DMA visibility, liveness, parity, or performance.

R26-R35 therefore improve one narrow persistent single-device path but do not
close the profile. Remaining blockers include ordinary public source-to-GPU
execution authority; broader native scheduling and concurrency beyond the
bounded two-lane, caller-flush path; full persistent memory, pool, and
concurrent-range behavior; unified compute plus native XGMI custody; production
atomic/collective authority and native litmus evidence; broad
Rust/device-language support; authenticated GPU execution profiling; broader
target and reset qualification; and concrete Rust/native refinement. The
normative gates below remain open.

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
observation-only. Direct in-process KFD is thread-affine and therefore still
requires caller-driven flush. A dedicated Worker V5 child can retain KFD on its
main thread while exposing a Send-capable address-free adapter to the progress
engine. The R24 ignored native test passed once on an idle gfx942 device at
commit `0631c5be`, covering its exact 63+2 completion and data result. Host
tests and that bounded hardware result do not establish general native
liveness, fairness, scheduling parity, or performance.

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
