# R48 native compute dependency occurrence lifecycle v1

## Boundary

The Linux gfx942 queue session owns one private compute-dependency issuer. Its
nonzero session occurrence is derived from the session's nonzero monotonically
allocated primary queue identity. The issuer burns one nonzero monotonically
increasing acceptance epoch for every source-event publication or
dependent-target attempt that reaches epoch reservation. Pure facade preflight
rejections, including active-target-capacity rejection, happen before that
reservation. Cancellation never rewinds an epoch.

The public API exposes move-only lane-affiliated event, source-batch, target,
completed-target, and failure custody. It exposes no signal address, packet ID,
completion slot, native reader lease, queue key, or epoch.

## Production states

The session owner preallocates and tracks at most 128 exact active targets by
their unique acceptance epochs. This is a dependency-owner storage-readiness
bound, not evidence that 64 in-flight epochs per physical lane are implemented.
The public facade preflights this bound before event, reader, target-resource,
or native mutation; the lower owner repeats the check before admission. Each
target follows:

`Idle -> Prepared -> NativePublished -> Published -> Completed -> Idle`

`Prepared -> Idle` is allowed only after the native publisher proves ring-full
no effect and the runtime atomically releases every reader, releases the private
target event, cancels the bound target completion, and cancels the dispatch
generation. The burned epoch remains consumed.

A source packet occurrence transitions from a private unbound reservation to a
public bound event only after the ordinary fixed batch is actually published
and the exact packet ID is bound. Target admission accepts 1 through 256
distinct events from exactly one other live lane in the same session. Every source
epoch must be strictly earlier than the target epoch. Same-queue, self, equal,
later, duplicate, stale generation, cross-session, and cross-lane substitutions
are rejected before native target publication. Preallocated hash ledgers make
duplicate and per-slot pin validation one-pass expected-linear work in the event
count; this is an algorithmic bound, not a measured performance claim.

The target publisher uses the B37 barrier chain and final dispatch implemented
by `fe2o3-aql`. Successful publication returns a stable boxed target dispatch
and an independent addressless target event. The lower owner state machine can
consume that event into a later target before the earlier target completes.
Pending polls reuse the same dispatch box. Exact dependent completion observation precedes the atomic
exactly-once release of all source reader and event pins. A returned target
event remains pinned until explicit release. Signal recycle and lane/session
teardown reject any active target, event pin, reader pin, or retained completion.
At the 128-active-target boundary, a 129th target is rejected before mutation
with its exact event custody and a later completed release makes that storage
slot admissible again.
An eager recycle of an otherwise completed source or target while a dependency
pin remains returns a typed proven-no-effect failure with the exact completed
dispatch custody. It can be retried after the dependent completion releases the
pin. Stale generation, currentness, observation, and reset failures remain
terminal and do not claim retryable custody.

## Failure and unwind

Wrong-session or invalid-lane rejection before the owning lane is selected
returns the exact move-only input custody. A proven ring-full no-effect result
returns the exact dependency roster in original order. Errors or panics at the
first native claim attempt or later are terminal: the session restores its
lane-owner placement, retains only the custody still present in its sealed
owners, poisons the local queue state, and permanently gates the process runtime.
The lower native callback boundary converts callback unwind into the typed
terminal `CallbackPanic` error. A distinct panic escaping the Rust orchestration
or lane envelope preserves and resumes its original payload after poisoning. No
terminal path is converted into a retryable result. Acceptance-epoch exhaustion
is also fail-closed and process gated.

## Evidence limits

Unit and fault-injection tests cover host state transitions, hostile identity
substitution, atomic batch rejection, no-effect rollback, callback errors and
panics, exact completion-gated release, pin-blocked recycle, and teardown
preconditions. This is not Rust-Verus or native syscall/hardware refinement.
GPU interpretation of dependency packets, ordering, completion truth, CPU/GPU
coherence, and MMIO/driver/firmware behavior remain contracted. There is no
MI300X execution, performance result, parity result, or generic speedup claim.
The multiple-active-target owner, one-source-arena production composition, and
post-dependent-completion atomic reader/event release have no R42/R45 executable
or Verus refinement.
This slice gives each source event one dependent consumer, accepts all fan-in
events for a target from exactly one source arena/lane, and exposes polling
rather than a bounded/event-backed wait. Multi-source-lane fan-in, event
fan-out, and a service-host dependency facade remain future work. R52 adds
[bounded host-retained coexistence](r52-native-fixed-dispatch-multi-inflight-v1.md)
for up to 64 epochs over one immutable `WaitForPrior` recipe on one physical
lane. It does not extend this R48 dependency graph to multiple recipes or prove
concurrent kernel execution. This remains lower host-state evidence for a
bounded chain and a single-source-arena lifecycle, not generic event-DAG parity.

Each dependency poll retains the existing pre/post currentness checks around one
signal load. Those checks include reset-fd readiness, VRAM-loss observation, and
runtime/event/shadow validation. Their syscall and metadata cost is unmeasured;
an amortized-currentness certificate or event-backed wait needs a separate proof
and MI300X benchmark tranche.
