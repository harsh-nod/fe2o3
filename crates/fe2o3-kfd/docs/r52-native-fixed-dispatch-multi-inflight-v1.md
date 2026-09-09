# R52 native fixed-dispatch multi-inflight v1

## Boundary

The Linux gfx942 KFD queue can retain up to 64 accepted epochs for one
immutable fixed-dispatch recipe on one physical compute lane. This is a host
pipelining bound. Every packet in the recipe uses `WaitForPrior`, so the slice
does not claim concurrent kernel execution. Ring occupancy and the shared 8192
signal completion arena can reduce the number accepted in practice.

The recipe seals its exact queue identity, code roster and content, ABI,
kernarg mapping and layout, ordered packet templates, and data authorities and
access premises. The outer queue session separately authenticates a public lane
handle before selecting that queue. A process-global nonzero recipe occurrence
distinguishes successive owners. The owner allocates one fixed 64-slot epoch
table and does not grow it during submission.

## Epoch lifecycle

Each slot follows:

`Vacant -> Reserved -> Published -> Completed -> Vacant`

Reservation burns a nonzero slot generation and dispatch generation. The
public token authenticates the session and lane through the sealed queue key,
plus the recipe occurrence, slot index, slot generation, dispatch generation,
exact completion batch occurrence, and native packet interval. Publication
accepts only the dispatch roster committed at reservation. The completion
occurrence is a SHA-256 commitment over the exact batch identity, queue, signal
mapping, ordered signal slots and generations, dispatch roster, packet count,
and first and last packet identities. Collision resistance is contracted; this
is not a formal collision-free proof.

Completion and recycle can be observed out of host call order. Recycle vacates
only the exact completed slot and records the monotonic maximum recycled
dispatch generation. Slot reuse burns a new slot generation, so an old token
cannot authenticate the reused slot. Validation is linear in packet count plus
fixed scans over bounded tables; this is an algorithmic bound, not a measured
performance result.

## Rollback and failure

`Independent` ordering is rejected before fixed-dispatch code or kernarg
resource allocation inside preparation and before queue packet publication.
Caller-provided data may already be allocated and mapped before this check. A
65th live epoch rejects before completion or native effects and leaves all
existing slots unchanged. Proven ring-full or completion-signal exhaustion
cancels only the exact reserved slot, returns the public input custody where
applicable, and does not rewind any burned identity. A published epoch cannot
be cancelled.

Any first-claim or later native, completion, binding, currentness, reset, or
orchestration failure is terminal. The runtime absorbs the custody it can still
identify, poisons the queue, and permanently gates new process runtime
admission. A Rust panic escaping an orchestration or lane envelope is caught,
terminalized, and resumed with its original payload. A panic inside the lower
native submission callback is instead erased there into the typed terminal
`CallbackPanic` error; no Rust payload is promised across that boundary.

## Quiescence

Readback, overwrite, insertion, removal, replacement, detach, persistent
rebind, effect promotion, ordinary release, returning destroy, and lane or
session teardown require every epoch slot to be vacant. Dependency event and
completion-signal pins remain additional independent teardown conditions. A
live `Reserved`, `Published`, or `Completed` slot therefore prevents resource
mutation and destruction.

## Evidence limits

Host tests cover 64/65 capacity, preallocation, A/B/C retained coexistence,
out-of-order observation and recycle, slot reuse and stale-token ABA attempts,
hostile queue/recipe/slot/generation/roster/completion/packet substitution,
exact no-effect rollback, panic and typed-terminal disposition, mutation and
teardown refusal, and maximum-size linear validation. Mock completion helpers
used by these tests perform no hardware operation.

The earlier R13 runtime model rejects overlapping writable retained resources
and does not represent ordered shared-recipe epochs, the 64-slot table, exact
completion commitments, or out-of-order host observation. R60 adds a separate
[ordinary-pipeline model](../../fe2o3-runtime-model/src/r60_ordinary_fixed_dispatch_pipeline.rs)
and runtime integration for that bounded surface. Neither supplies a
Rust-to-Verus refinement of R52's native implementation. General multi-recipe execution, shared-buffer
DAGs, hardware ordering and completion truth, performance improvement, and
HIP/HSA parity remain outside this tranche. R52 itself attaches no MI300X
benchmark evidence; the separately versioned
[R60 benchmark protocol](../../../benchmarks/runtime_gfx942/R60-PIPELINE.md)
tests the integrated ordinary path.
