# fe2o3-kir-sim

`fe2o3-kir-sim` is a bounded, deterministic CPU execution engine for an
explicit subset of verified canonical Kernel IR V7. Admission consumes a
`VerifiedCanonicalKernelIrV7`; raw in-memory modules and older wire formats are
not execution inputs.

The `fe2o3-kir-sim-capabilities` binary emits the complete V1 semantic
ownership matrix as stable JSON. It covers every top-level KIR operation and
terminator for each simulator-facing profile, plus every scalar
unary/binary/compare/cast type combination. Rows name either the exact
simulator owner or the typed preflight rejection; the document explicitly
grants no hardware or performance authority.

Ordinary admitted Rust can obtain these exact V7 bytes from a strict
`VerifiedSimulationBundleV1` produced by the authority-free
[`fe2o3-export-sim`](../../docs/simulation-bundle-v1.md) transaction. Bundle
export does not execute or authorize a kernel, and its extraction-only compiler
binding does not authenticate compiler execution.

Admission relies on that consumed owner's private immutable bytes and identity:
the owner cannot be constructed without exact V7 canonical decoding and full
semantic verification. The simulator therefore does not rerun the semantic
verifier. It independently enforces `max_canonical_bytes`, canonical-decodes and
re-encodes the consumed bytes, and then accounts the resulting decode-phase
peak. `max_resident_bytes` is a successful-admission acceptance limit evaluated
after that decode; it is not a pre-decode allocator cap.

The execution profile supports integer, boolean, and F16/BF16/F32/F64 scalar
operations, structured control flow, internal helper calls, private
allocations, global buffer arguments, ordinary and guarded scalar loads, static scalar
workgroup-memory declarations, convergent workgroup barriers, and one-, two-,
or three-dimensional launch hierarchy intrinsics. A false guarded load
evaluates only its predicate and fallback; it does not validate the pointer,
record a memory access, or emit a memory-read event. Workgroups and local slots
are created in canonical Z/Y/X
lexicographic order. Invocations in one workgroup then advance cooperatively and
yield at barriers; a phase releases only after every live in-grid participant
arrives at the same site with identical barrier semantics. Padded local slots
are included in admission and execution accounting but never become barrier
participants. The target profile enforces its legal workgroup volume before
scheduling begins.

The V7 core wave profile executes `LaneId`, `Ballot`, `Any`, `All`, and
integer `ShuffleIndex` with an explicit Wave32 or Wave64 contract. Logical lane
identity uses X-fast local-linear invocation order. Every participating lane
must reach the same wave operation and semantics; divergence, mismatch,
out-of-tile shuffle sources, and a final partial logical wave are exact typed
failures. These are logical collective semantics, not ISA emulation or a claim
about a hardware `EXEC` mask.

Each static `WorkgroupMemory` operation denotes one zeroed-but-uninitialized
allocation site per workgroup. The allocation is shared by that workgroup and
released before the next workgroup starts. Loads and stores reuse the ordinary
typed-pointer address-space, access, alignment, bounds, initialization, and
provenance checks. A lane can read its own initialized write immediately; a
different lane can read those bytes only after a compatible workgroup barrier
publishes them. Uninitialized access and cross-lane use before publication are
distinct typed failures. Workgroup allocations, bytes, publication ownership,
all cooperative machines, and their frame storage are included in preflight
resource accounting.

Every result identifies this schedule and carries bounded, byte-level
cross-invocation global-memory conflict and race assessments. The race
assessment classifies observed conflicts as unordered races or as exactly
ordered by integer atomic serialization or a compatible same-workgroup global
acquire-release barrier. Record exhaustion is machine-readable and takes
precedence over a clean result. Release/acquire atomic and fence metadata is
preserved, but their reads-from and synchronizes-with edges into ordinary
accesses are not resolved; an ordinary conflict in a run containing such an
operation is therefore typed incomplete instead of being called a race. Even a
complete clean observation covers only that deterministic CPU order; it is not
a proof of race freedom or a model of GPU scheduling.

Each tracked global byte retains bounded current and displaced read/write
frontiers. Ordered reads and atomic accesses therefore cannot erase an older
writer before a later conflicting access is classified. If more representatives
must be evicted, a later access that is not known to serialize with the lost
history makes both conflict and race evidence explicitly incomplete;
it can never produce a false clean result. Once a race is observed for that
byte, later frontier replacement cannot weaken its classification or increment
the unique racing-byte count again.

The ordinary `simulate` paths retain the canonical cooperative order. Opt-in
`simulate_scheduled` and `simulate_debugged_scheduled_with_sink` calls can
record either that order or a deterministic seeded order. A schedule decision
selects one currently runnable invocation by exact logical workgroup and local
coordinates for one cooperative barrier phase. Seeded ordering uses a frozen
SplitMix64 permutation within each phase; workgroups remain in canonical Z/Y/X
order. It is a CPU semantic exploration order, not an approximation of GPU
wave, workgroup, or compute-unit scheduling.

A successful recording returns an opaque `SimulationScheduleRecordV1`. The
record and every result expose a SHA-256 transcript identity plus exact decision,
workgroup, and barrier-release coverage. Replay binds the record to the exact
canonical KIR identity, selected kernel, launch, target layout, arguments,
shared buffers, event policy, and resource limits. It validates every decision
against the currently runnable local identities and rejects context drift,
missing or trailing decisions, duplicate or unavailable locals, phase drift,
coverage drift, and transcript corruption. Decision retention has an explicit
caller bound, a fixed hard cap, fallible reservation, and resident-byte
admission. Unrecorded canonical execution retains no decision vector and never
fails a legacy run because of the recording bound.
Successful records compact the execution-time reservation to the exact realized
decision count before the record is returned.

`explore_seeded_schedules` sweeps a bounded contiguous, wrapping seed interval
and retains at most one exact replayable schedule for each race, no-race, and
incomplete class, plus the first typed dynamic failure. Requests have fixed
hard caps on attempted schedules, decisions per schedule, and decisions
retained across witnesses. Results state whether the requested seed budget was
consumed and whether witness retention was exhausted. Consuming that caller
budget does not exhaust or characterize the possible schedule space and cannot
prove absence of another behavior.
The inline exploration result and all retained schedule, assessment, and first
failure payloads are charged together with every later scheduled run under
`max_resident_bytes`; decision retention is charged by actual compact capacity.

`PersistedSimulationScheduleDocumentV1` is the canonical, bounded JSON custody
form for that same record. It adds exact raw-KIR versus simulation-bundle route,
bundle subject when present, request byte identity, target profile, and every
simulation limit. Strict decode rejects unknown, duplicate, null, trailing,
noncanonical, oversized, structurally invalid, and integrity-corrupt input.
Decoding grants no authority: callers compare the retained binding to already
admitted inputs, then ordinary replay still performs every context,
runnable-decision, coverage, and transcript check.

Before any mutable execution state is created, preflight visits the complete
call graph reachable from the selected kernel, checks target-specific constants,
SSA frame size, and statically known acyclic call depth. Recursive internal
helpers remain supported and are dynamically bounded by the call-depth and step
limits. Both preflight traversal and execution use fallibly reserved explicit
stacks, so admitted call depth never becomes native Rust recursion. Unsupported
diagnostics retain a deterministic prefix of at most 4,096 scan-order
occurrences and at most 1 MiB of owned identifier bytes, and separately report
the exact total, so hostile identifiers cannot amplify diagnostics into
unbounded storage.
The regression suite runs the maximum admitted call chain and recursive limit on
an unchanged 128 KiB native thread stack. Large ordinary-operation and
non-return-terminator evaluators remain separate leaf calls from the cooperative
scheduler; exact compiler stack-frame sizes are diagnostic measurements, not a
stable ABI.
Verified V7 integer atomics are supported for every represented fixed width,
operation, legal ordering, and legal scope over global and workgroup memory.
Each atomic executes as one indivisible mutation in the selected deterministic
CPU schedule; add and subtract use fixed-width wrapping semantics, signed
minimum/maximum compare two's-complement values, and compare-exchange reports
and commits its exact success or failure outcome. Workgroup atomic writes are
immediately visible to later atomic operations in the same simulated workgroup.
Fences are explicit scoped order points in the sequential interpreter and do
not synchronize which invocation executes next. Atomic and fence events retain
their KIR semantics and allocation-relative provenance, and atomic debug
records distinguish read, committed-write, and read-modify-write observations.

Scalar floating-point constants, loads/stores, negation, add/subtract/multiply/
divide/remainder, comparison/select, float widening/narrowing, and fixed-width
integer conversions use pinned `rustc_apfloat` software IEEE semantics. Binary
arithmetic and format conversion use round-to-nearest, ties-to-even; float to
integer conversion uses round-toward-zero and fails with a typed range error for
NaN, infinity, or an unrepresentable result. Values retain exact bits, including
NaN payloads and signed zero. The canonical FloatOperation conversions, widened
F16/BF16 arithmetic, F32 fused multiply-add and integral-rounding functions,
and packed BF16x2 fused multiply-add use the same software evaluator. Operations
are never implemented with host `f32`/`f64` arithmetic and are never implicitly
contracted.

Float atomics, generic-address-space atomics, external calls, generic barriers,
dynamic or non-scalar workgroup memory, V9 F32 wave reductions and broadcasts,
matrix operations, gfx950 LDS transpose operations, memory
intrinsics, and inline assembly remain typed unsupported states. F32 square root
and the canonical sin/cos/exp/exp2/log/log2/log10 functions each retain a
distinct typed unsupported state because the pinned software evaluator does not
provide their declared exact semantics. The sequential
CPU mutation and fence order model does not simulate physical waves, caches,
GPU floating-point modes, GPU timing, GPU performance, or prove memory-model
race freedom.

Callers consume an exact V7 owner with `AdmittedSimulationModuleV1::admit`, then
provide an explicit target, resource limits, launch shape, and typed scalar or
byte-addressed buffer arguments in `SimulationRequestV1`. Index scalars,
buffers, and views are bound to the 32- or 64-bit layout used to construct them;
fixed-width values remain portable. Ordinary `Buffer` arguments receive
distinct allocations. `BufferView` arguments can instead name bounded,
overlapping views of a `SharedBufferV1`, and copied-back shared allocations are
exposed separately on the result.

`simulate` returns independent copied-back arguments and backings; it never
mutates the borrowed request. For debugger integration, `simulate_with_sink`
honors the request's event policy, while `simulate_observed_with_sink` explicitly
enables delivery without cloning or mutating the request. Bounded in-process
events include exact invocation and operation lifecycle boundaries, allocation
lifecycles, block entry, selected branch target, memory, call and return sites,
per-lane barrier arrivals, and one workgroup barrier release with the exact live
participant count.
Event sites and call targets carry canonical module-function ordinals, so
hot-path observation neither clones nor repeatedly compares owned function
identifiers; adapters resolve each ordinal against the admitted module.
Nested call and return ordering lets an adapter assign exact recursive frame and
operation-occurrence identities. A sink reports failure through a typed result;
rejection must be atomic for that event. A bounded sink can instead retain its
last event and return `SimulationEventSinkControlV1::Stop`, which permanently
disables later callbacks and event accounting without changing execution.
If the current event does not fit, it can instead return `DropAndStop`; that
event is excluded from `events_emitted` and all later delivery remains stopped.
Lifecycle begins reserve their matching end capacity, including private
allocation releases on normal return and failure unwind. If a sink rejects a
failure-closing event, the primary dynamic failure retains the first bounded
secondary observation failure rather than discarding either fact.
Every fallible store preparation step occurs before its memory-write event, and
the accepted event is followed only by an infallible byte commit. These events
are an adapter, not a stable serialization contract; storage retained by a sink
is owned and budgeted by the sink. Sink-retained event copies and a sink's own
error-detail allocation are external to `max_resident_bytes`; production
adapters must impose their own byte bound before accepting or returning them.

Successful admission caches one decoded module. Its resident charge walks the
full module, including unreachable functions, nested types, identifier and
container spare capacity, operation operands, switch cases, and capability
storage. The measured admission peak also charges the input and exact canonical
re-encoding that coexist with the decoded graph. If that post-decode peak exceeds
`max_resident_bytes`, admission rejects it, but canonical owner construction and
the rejected decode/re-encode may already have transiently exceeded that
setting. Those operations are instead bounded by `max_canonical_bytes` and the
frozen KIR wire/count/depth caps.

Preflight accounts for scheduled slots and both phase-specific resident peaks.
Its own peak includes caller inputs, full-module lookup tables, borrowed SSA type
indices, reachability worklists, and iterative call-graph/SCC scratch. The
execution peak includes the admitted module, request and plan identities,
measured nested execution indices, simulated bytes and packed initialization
maps, copied-back outputs, persistent SSA/frame scratch, allocation metadata,
and conflict records. `SimulationPlanV1::resident_bytes` exposes the larger of
those checked peaks. Standard-container payload capacity is counted against the
pinned toolchain; allocator metadata and page rounding are outside this stable
contract. Allocation paths whose size depends on admitted input reserve
capacity fallibly before mutation.
Cross-invocation conflicts aggregate all accesses to a byte within one
invocation and count each conflicting byte once, while retaining the exact first
conflicting access pair.

Simulation results are observations only. They do not establish source-to-KIR
refinement, proof discharge, GPU equivalence, artifact authority, load
authority, launch authority, timing, or performance.

`simulate_debugged_with_sink` is an opt-in observation path for deterministic
CPU debugging. It emits bounded before/after-operation snapshots derived from
the live interpreter frames, plus successful ordinary and atomic memory
observations, committed writes, and fence order points with typed values and
semantics. Atomic watchpoints stop once per indivisible atomic operation;
read/write watchpoints also match the corresponding atomic access. Snapshot
collections are either complete or carry an explicit
unavailable reason; they are never silently partial. A debug sink can stop its
own delivery without stopping or changing simulation. Debug records are
separate from the stable `SimulationEventV1` adapter and Semantic Trace V1.
Each debug record also names the semantic schedule and zero-based runnable
decision that produced it. This is ordering provenance only; neither a record,
replay, seeded variation, nor conflict-free observation establishes GPU
scheduling, timing, performance, performance prediction, or race freedom.
