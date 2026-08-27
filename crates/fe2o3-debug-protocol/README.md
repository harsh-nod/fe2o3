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
