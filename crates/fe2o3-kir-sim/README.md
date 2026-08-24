# fe2o3-kir-sim

`fe2o3-kir-sim` is a bounded, deterministic CPU execution engine for an
explicit subset of verified canonical Kernel IR V6. Admission consumes a
`VerifiedCanonicalKernelIrV6`; raw in-memory modules and older wire formats are
not execution inputs.

Admission relies on that consumed owner's private immutable bytes and identity:
the owner cannot be constructed without exact V6 canonical decoding and full
semantic verification. The simulator therefore does not rerun the semantic
verifier. It independently enforces `max_canonical_bytes`, canonical-decodes and
re-encodes the consumed bytes, and then accounts the resulting decode-phase
peak. `max_resident_bytes` is a successful-admission acceptance limit evaluated
after that decode; it is not a pre-decode allocator cap.

The first execution profile supports integer and boolean scalar operations,
structured control flow, internal helper calls, private allocations, global
buffer arguments, static scalar workgroup-memory declarations, convergent
workgroup barriers, and one-, two-, or three-dimensional launch hierarchy
intrinsics. Workgroups and local slots are created in canonical Z/Y/X
lexicographic order. Invocations in one workgroup then advance cooperatively and
yield at barriers; a phase releases only after every live in-grid participant
arrives at the same site with identical barrier semantics. Padded local slots
are included in admission and execution accounting but never become barrier
participants. The target profile enforces its legal workgroup volume before
scheduling begins.

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

Every result identifies this schedule and carries a bounded, byte-level
cross-invocation global-memory conflict assessment. A conflict or an incomplete
assessment is machine-readable. Even a clean observation is not a proof of race
freedom or a model of GPU scheduling.

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
Floating-point operations, external calls, generic barriers, atomics, fences,
dynamic or non-scalar workgroup memory, wave operations, matrix operations,
memory intrinsics, and inline assembly are rejected by this profile. Workgroup
barriers do not simulate physical waves, atomics, cache behavior, timing, or
performance.

Callers consume an exact V6 owner with `AdmittedSimulationModuleV1::admit`, then
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
