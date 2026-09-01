# fe2o3 debug protocol

`fe2o3-debug-protocol` is the inert, bounded JSONL contract shared by a future
standalone fe2o3 debugger, the deterministic KIR simulator debugger, and an
explicitly authorized direct-KFD debugger backend. It performs no compilation,
simulation, dispatch, device discovery, attachment, or KFD operation.

## Framing and state

Every request and response is one compact JSON object followed by exactly one
LF byte. Embedded CR/LF bytes, unterminated input, oversized lines, invalid
UTF-8/JSON, duplicate fields, unknown fields, explicit `null` optionals, and
unknown enum tags fail closed. The default request limit is 1 MiB. Responses
default to 2 MiB and can be configured only up to 16 MiB. Encoding uses a
bounded writer rather than constructing an unbounded JSON string.

All requests carry a nonzero `request_id` and an `expected_revision`. A backend
must apply a state-changing request atomically only when the revision matches,
then return the new revision. Protocol rejection and capability unavailability
must leave the session unchanged. The response schema separates:

- `ok`: the protocol operation was performed;
- `unavailable`: the requested semantic capability is not available from this
  backend, capture, or authority state; and
- `error`: framing, protocol, session, backend, or publication rejected the
  request.

A simulated kernel fault is an `ok` control operation whose session is
terminal with a failed execution outcome. It is not misclassified as a
protocol error.

## Operations

The closed V1 operation set is capability discovery, state inspection,
breakpoint and watchpoint management, continue, pause, forward/reverse step,
seek/replay, hierarchy inspection, source resolution, captured-stack
inspection, value inspection, allocation-relative memory reads, event queries,
bounded trace export, and termination.

Stepping has explicit event, operation, frame-aware over/out, memory, barrier,
lane, wave, workgroup, and source granularities. A backend must report source stepping unavailable
without an authenticated semantic map. Simulator wave scopes are labeled
`logical_visualization`; only a real hardware collector may emit
`hardware_observed`. Hardware replay is a new dispatch and must not be exposed
as reverse execution of the original dispatch.

Resolved source locations carry a closed provenance class. `caller_bound`
means the map and expected subject were supplied through a low-level external
admission boundary; it is not compiler authenticity.
`compiler_bundle_bound` is reserved for exact map bytes whose digest and
subject were verified by the compiler-owned bundle decode transaction. It is
content association, not protected compiler-execution authentication.
Absent, optimized-out, and many-to-one mappings remain typed unavailable
states. Stack frames are captured backend facts with one-based identities and
typed value availability, never name-derived or UI-synthesized frames.

Source-variable inspection is an additive, separately versioned
`fe2o3-debug-source-variable-request-v2`/response exchange. It does not add a
variant to the closed V1 request, response, operation, or value-availability
enums. Exact identity selection is stable across shadowing; bounded name
selection uses retained lexical scope depth and reports ambiguity instead of
guessing. `all` results are page bounded. The response reuses V1 typed scalar,
bit-vector, redaction, unavailable, and allocation-relative pointer values and
adds ambiguity only inside its V2 availability enum.

The CPU backend admits this query only with an exact Source Map V2 envelope.
A V1 map returns `source_map_v2_required`; a valid V2 map with no variable
records returns `variables_not_captured`. Map and bundle identities establish
exact content association, not compiler-execution authentication. Hardware V2
does not admit this simulator query.

Semantic diagnosis is another additive, read-only exchange under
`fe2o3-debug-diagnosis-request-v2` and its matching response schema. It pages
only typed dynamic findings retained by the deterministic CPU simulator:
out-of-bounds memory regions and workgroup-barrier divergence or mismatch.
Every dispatch, workgroup, work-item, logical wave, lane, KIR site, memory
region, barrier participant, and phase fact is individually labeled
`declared`, `observed`, `inferred` with a closed derivation, or `unavailable`
with a closed reason. In this schema `observed` always means a CPU semantic
simulation observation. The response validator rejects KFD hardware sessions,
hardware-observation substitution, invalid hierarchy joins, in-bounds ranges
claimed as out of bounds, and incompatible finding/detail pairs. It also joins
the session configuration to the diagnosis, re-derives the domain-separated
request/KIR dispatch-input identity, checks bundle/map/subject and source-site
bindings, checks allocation/ABI view bounds, and requires barrier phase,
semantics, LDS epoch, participant, and mismatch fields to agree.

The memory contract separates a pointer/slice legal view from its backing
allocation and retains every aliased ABI argument without collapsing their
ordinals or ranges. ABI-required, request-view, and backing-allocation access
capabilities are separate facts joined by the simulator's monotonic admission
rule: read-only and write-only are incomparable, while read-write supplies both
capabilities. Barrier divergence retains complete bounded expected,
arrived, waiting, and exited participant inventories. Expected local
coordinates are launch-geometry inferences; arrived/waiting/exited local
coordinates are simulator observations. The current LDS epoch is derived from
the retained barrier phase and is never labeled as an independent observation.

Every material claim cites a stable retained evidence-record identity and has
a distinct claim identity binding field, value, origin, and source record. The
canonical evidence manifest binds session/revision/configuration,
completeness, finding sequence/class, exact input evidence, and canonical
bounded terminal/transcript records. Their identities are recomputed during
admission; barrier-arrival claims cite the transcript record. This establishes
content integrity and internal consistency, not producer signing or capture
authenticity. A capture owner authenticates a response by separately deriving
`DiagnosisCaptureBindingV2` from its owned deterministic simulator result and
calling `validate_against_capture_v2`; that binding retains the exact input
manifest, full session and completeness, response request/operation/cursor,
and Bundle envelope and subject when supplied. Declared source operations
carry a bounded membership proof against the exact admitted Source Map V2
operation/span inventory, preventing substitution of another span from the
same map.

Bundle, KIR, request, ABI, source-map, and source-lineage references are exact
declared content associations. A source operation retains the source map's
`caller_bound` or `compiler_bundle_bound` provenance. Source-lineage receipts
are never property or proof evidence. Finalized artifacts and property proofs
remain typed unavailable until a separately authenticated authority can supply
those records. Raw KIR input remains supported and reports every absent bundle,
map, source, artifact, and proof axis with a closed unavailable reason.

Diagnosis is retrospective over the immutable bounded transcript and terminal
failure, independent of the interactive cursor. Filters and page cursors are
bound to the exact simulator configuration and session revision. A complete
transcript can report the observed barrier-arrival count; a truncated
transcript keeps that count typed unavailable. Logical waves and lanes are
explicitly inferred visualization partitions, never physical GPU state.

Breakpoint predicates are a closed, bounded AST of typed value operands,
constant bits, comparisons, and boolean composition. Arbitrary expression
strings are not accepted. Watchpoints name a generation-aware allocation plus
checked byte range, access class, scope, and before/after-commit timing.

## Values and addresses

Values are captured, redacted, or unavailable with a closed reason. Scalar
bits use fixed-width lowercase hexadecimal. Aggregates are represented by
bounded typed paths and leaf records so recursive user values cannot create an
unbounded protocol tree. Memory includes an exact initialization bitset and
explicit truncation.

The schema has no file descriptor, queue handle, KFD token, native pointer, GPU
virtual address, or host address field. Pointer values can only be represented
as a trace-local `(allocation ordinal, generation)` and byte offset. Opaque
identities are nonzero 32-byte values encoded as exactly 64 lowercase hex
digits; they are correlation identities, not execution authority.

## Bounds

V1 admits at most 4,096 breakpoints, 4,096 watchpoints, 4,096 page/response
items, 64 predicate nodes at depth 16, 32 aggregate path components, one
million requested step units, and 1 MiB per memory read. Text is nonempty,
control-free UTF-8 of at most 256 bytes. All byte ranges use checked arithmetic.

The backend remains responsible for bounding simulator steps, trace events,
session resident memory, checkpoints, total commands, and device/runtime
resources. Those are execution concerns and are deliberately not granted by
this inert wire crate.

## Qualification V1

`fe2o3-debug-qualification-manifest-v1` is an additive inert comparison and
overhead-policy record. Its fixed component matrix covers the two fe2o3 KFD
debugger modes, ROCgdb, rocprofv3, ROCprof Compute Viewer/ATT, and
representative HIP and Mojo workflows. Every task cell is explicitly
caller-bound observed, documentation-only, or unavailable. The fixed overhead
matrix covers no-capture, counters, PC sampling, ATT, debugger-stop, and
instrumented modes.

Decoding this manifest never authenticates an observation or qualifies a
capture mode. Caller-bound observations retain exact content, version,
configuration, and evidence identities, but those identities provide content
association only. `grants_observation_authority()` and every policy
assessment's `grants_qualification_authority()` return `false`. A measured
policy comparison must name exact workload, input, artifact, environment,
device, collector content, baseline configuration, and captured configuration
identities and the canonical raw/no-capture comparator record. Only the
manifest-level evaluator is public; it revalidates the complete current
manifest immediately before re-deriving whether caller-supplied values meet
the declared policy. A separate authenticated producer is required for a
production observation claim.

An available comparator requires exactly one measured no-capture row whose
axes, configurations, raw/no-capture evidence, statistic, clock, warmups,
repetitions, and durations match the comparator record. That baseline must be
loss-free and non-truncated. Every other measured mode uses that no-capture
evidence and configuration as its baseline and must use treatment evidence
distinct from both canonical evidence records.

The manifest is capped at 256 KiB. It requires all seven component rows and all
six capture-mode rows once in canonical order, bounds all text and numeric
fields, rejects unknown/duplicate/trailing JSON, and domain-separates and
length-binds its content identity. No typed field interprets text as an
executable path, argument vector, process/device address, descriptor, queue
token, or execution action, and text grants no such authority. Unavailable
tools and unmeasured modes remain first-class records.

## Hardware V2

The separate `fe2o3-hardware-debug-request-v2` and response schemas describe
only KFD-observed hardware state. They do not extend or reinterpret the frozen
simulator V1 protocol. V2 pages redacted device/queue snapshots and exception
events, and controls queue suspend/resume plus termination. A control revision
orders mutation while a separate observation sequence orders asynchronous KFD
events. Device and queue identities are session-local generation/ordinal
pairs, so a refreshed snapshot rejects stale control identities.

Hardware V2 is bounded to 64 KiB requests, 1 MiB responses, 256 page/control
items, 4,096 retained events, one million commands, and a one-second event
wait. Errors state whether an operation had no,
committed, partial, or indeterminate effect; indeterminate backend mutation is
terminal. The schema has no PID, file descriptor, native KFD identifier,
target address, or argv field. It explicitly reports wave/lane/register/CWSR,
stack/source/KIR, stepping/replay/breakpoints/values, target memory, semantic
trace, address watch, and dispatch submission unavailable.

## Live GPU V3

`fe2o3-live-gpu-debug-request-v3` is an exact-artifact facade over the hardware
state machine. Its session binding separately retains the declared code
object, any matching target declaration, any independently observed execution
identity, canonical KIR V7 and Source Map V2 identities, and deterministic CPU
reference inputs. Every fact carries a closed `declared`, `proved`, `observed`,
`inferred`, or `unavailable` origin with bounded evidence. A declaration can
never satisfy an observed execution field.

V3 reuses Hardware V2 device, queue, event, and control results, including
committed/partial/indeterminate effects. Semantic scope, register, value,
memory, and program-site queries are separately versioned and require an exact
binding and stopped identity. Unsupported or uncaptured state is a typed
top-level result, not an empty scope. A stopped scope page is always an
`observed_subset`; omitted workgroups, waves, and lanes are never interpreted
as absent.

Requests remain bounded to 64 KiB, responses to 2 MiB, pages to 256 items,
memory reads to 1 MiB, and cooperative target telemetry to 4,096 records.
Program counters are code-object-relative, memory is allocation-relative, and
the schema has no native process, descriptor, queue, or address authority.
