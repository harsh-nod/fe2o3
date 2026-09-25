# Producer-Aware Typed Launches

Development toward A1/A2 in #182. This profile is not protected Worker V3
execution, a new formal-refinement result, aggregate-memory acceptance or
HIP/HSA parity. It extends the existing ordinary launch lifecycle rather than
creating a second executor or changing Worker transport.

## Contract

`RuntimeProducerAwareLaunchBackendV1` is an explicit, ordinary-only backend
contract. `BackendProducerAwareLaunchV1` carries each original event together
with its exact producer submission. The backend authenticates both identities,
rejects aliases naming the same producer twice, retains dependencies separately
from public events, and publishes work only after every explicit producer has
succeeded. Stream ordering, timeout, cancellation and quiescence without a
result do not establish producer success. Existing atomic and collective launch
contracts are unchanged and cannot be selected through this request.

`RuntimeContextV1::launch_producer_aware_v1` requires an allocation version
journal and same-device producers admitted through this profile. Frozen async
requests use `enqueue_producer_launch`, `enqueue_producer_launch_tracked`, or
`enqueue_producer_launch_with_event`; the existing owner loop, snapshot and reply
budgets, cancellation controls and observer futures remain responsible for
progress. Enqueue does not itself admit or publish device work.

The original typed bindings, module identity and canonical dependency roster
remain Context-owned until conclusive quiescence. Dependency ordinals preserve
the caller's backend order even though logical reconciliation uses canonical
submission order. A read-only consumer has this ownership record without an
artificial output writer. Public event release and observer loss do not release
the retained producer submissions or their input leases.

## Input Custody

Pure reads retain whole-allocation leases. This conservatively prevents another
writer or disposal from overlapping the consumer, but does not describe the
kernel's actual read footprint. Every original pending Read binding must be
covered by the union of its named producer's original writable intervals.
Adjacent intervals may cover one read; a gap, uncovered alias, Read-only
producer binding, or unrelated event cannot supply coverage. Canonicalizing
allocation custody must not discard these original footprints.

One consumer may hold stable reads, multiple pending-producer reads and output
writers simultaneously. Both reader classes share one capacity budget. All
requests, canonical rosters and combined capacity are checked before any lease
or dependency retain is acquired. All fallible roster allocations precede
submission identity consumption. Writable aliases with pending predecessors
remain rejected; this is not ordered-writer or cross-run reuse authority.

The consumer root is installed before journal mutation and backend entry.
Both complete reader rosters are validated before either is released. A
resolved producer reservation is queried by its retained identity, not
readmitted against a writer slot that may have been reused. Rejected initial
submission releases inputs/dependencies and settles output writers as NoEffect;
conclusively quiescent failure releases inputs but retains Unknown output
state. Terminal failure, panic, malformed handles and ownership contradictions
retain uncertain resources and seal the Context.

### Atomic Mixed Acquisition

`ContextProducerReadJournalV1::acquire_mixed_reads` performs both complete
preflights before committing either reader class. It validates the Submission
consumer, both output shapes and checked combined headroom even when one side
is empty. An empty side does not consume or validate its unused arena's next
incarnation. A returned error leaves the complete journal and both output
arrays unchanged; success commits the exact stable and producer rosters using
the existing allocation-free helpers. This adds no post-preflight rescan.

The Context adapter installs both original input roots before calling that
operation. It fills both preallocated reference vectors before publishing
either marker and enters the backend only after both are complete. Writerless
and empty-input consumers remain supported. A panic after acquisition retains
the roots and journal leases and seals the Context; it does not roll back the
transaction. Output-writer Begin, dependency retention and these two input
classes are not one formally proved all-or-nothing transaction.

The shared-body Verus candidate specifies error preservation, the two exact
commit relations and combined-budget preservation under storage-domain
preconditions. Its root includes inherited paired proofs but does not yet add
independent logical mixed-acquisition correspondence, constructor-origin mixed
traces, Context-map authentication, panic recovery or completion reconciliation.
These remain distinct obligations, even if the candidate's solver run passes.

The [mixed-acquisition CPU packet](evidence/dev-mixed-input-acquisition-cpu-2026-09-24/README.md)
passes full GNU/musl runtime and model suites and 73 doctests. Its whole-root
Verus development run reports 1,244 verified obligations and zero errors; the
formal scope and authentication limits above still apply. The earlier two-case
MI300X producer packet qualifies previous signed source, not this change.

## Completion

Typed launches and directed peer copies use the same bounded reconciliation
walker. A retained physical consumer success is not yet public logical
success: each producer must reconcile first and each pending input must resolve
to Success. Producer callbacks consequently precede consumer-success callbacks.
The walker preserves its cursor across yields, bounds dependency depth to 256,
and makes at most one backend observation per step. Retained physical success
makes cancellation TooLate even while logical reconciliation remains pending.

Ordinary poll/wait, event observation, stream synchronization, drain, explicit
release and cleanup use the existing centralized settlement paths. Exact
quiescence can release consumer input custody without declaring that its inputs
or outputs succeeded. Protected generated completion remains a separate domain.

## Backend And Evidence Boundaries

Single-device KFD uses its existing pending-compute retention, explicit-success
gates and dispatch materialization. Multi-device KFD translates both halves of
each identity pair into the same child; cross-child and cooperative-copy events
are rejected. This adds no copy-only XGMI backend implementation, fallback
runtime, capability-bit authority or Worker wire variant.

CPU tests exercise Context custody, failure atomicity, exact backend identity
validation and async owner integration. They do not prove native publication or
GPU memory ordering. Fresh hardware qualification of the new path and explicit
production/model refinement remain required, as do broader copy/compute
producer composition, generated graphs, resource bounds and matched performance.

The [CPU qualification packet](evidence/dev-producer-aware-launch-cpu-2026-09-24/README.md)
passes all seventeen stages, including 27 new focused tests and the complete
1,392-test runtime roster on each GNU/musl target. The twenty hardware-only
runtime ignores remain; the packet records its collector failure and subsequent
non-deleting evidence recovery separately from the successful test campaign.

The initial CPU packet does not qualify active-producer admission. Its R57
three-binding persistent DeviceLocal backend requires ready backing and rejects
a consumer after the producer moves shared bindings into `ComputeInFlight`.
The subsequent [active-producer extension](evidence/dev-active-producer-cpu-2026-09-24/README.md)
adds the distinct deferred eligibility below. Both queued and native published
producer cases still require separate hardware witnesses, without weakening
artifact authority or materializing a consumer early.

Existing journal acquire/release correspondence applies to its individual
batch operations. It does not prove the new mixed Context transaction, original
binding coverage, shared reconciliation walker, or async request carriage.
Those composition proofs remain required; passing CPU tests cannot supply them.

## Deferred Active Inputs

The existing authenticated dependency collector carries a private admission mode
to the shared compute preflight. Ordinary event-only launches retain their ready
input requirement. Only exact producer-aware requests may defer readiness for
the two Read bindings of the existing full-extent, distinct-allocation,
initialized DeviceLocal R/R/W path; the destination must remain ready.

A deferred allocation must name its actual active producer in the explicit
success dependency roster. The producer must be on the same device with the
existing three-binding prepared or published execution, matching allocation
rosters, and exact retained compute owners. These facts establish permission to
wait, not ready backing, publication authority or producer success. No separate
pending registry or synthetic ready capability is constructed.

After explicit dependencies and stream ordering settle, the backend recomputes
the original ready admission before staging. A missing restored input settles
the unpublished child as failed. Persistent candidates cannot take the ordinary
early-successor path or fall back to user-data materialization. The existing
artifact gate still authorizes the final snapshot before owner extraction.
Unknown parent outcomes retain both parent and child custody and seal the
backend; cancellation of an unpublished child does not release parent owners.

## Next Qualification

The existing `qualification_gfx942_r57_n3_v1` gate independently authorizes
`A+B->C`, then `C+B->D`. Reuse its exact artifact, ABI, expected bytes and
authority-call observation rather than broadening the plain vecadd gate, whose
input-content policy does not authorize this chain. A version-journal witness
must enqueue both producer-aware launches before observing either, release the
public producer event after consumer admission, observe the consumer first,
check producer-first logical settlement and all four complete buffers, then
inspect logical cleanup and native teardown. Retaining the final completed H2D
submission on the producer stream keeps the first persistent launch queued;
that is a queued-producer witness only. A separate case must enqueue the
consumer after native producer publication to qualify active-producer support.

The formal follow-up must connect three production boundaries to the existing
producer acquire/release, stable-reader and journal-issuance correspondence:

- Admission: original writable-range coverage, canonical mixed-input partition,
  shared capacity, writerless consumer identity, and the entire writer/read
  transaction's rejection or retained-terminal outcome.
- Reconciliation: earlier-ID dependencies, exact retained physical outcomes,
  cursor bounds, event-independent retention and producer-first logical success.
  A bounded observation step is not a proof of eventual hardware progress.
- Async carriage: frozen bytes/bindings/dependencies enter that same transition;
  observer abandonment and cancellation cannot discharge live custody.

Constructor-origin traces should include two pending inputs and one stable
input, writable and writerless consumers, public-event release, Unknown
propagation and complete reservation release. Missing-last-input, writable-alias
mispartition, split-capacity, early-success and premature-terminal-release
mutations must fail independently. Existing individual journal proofs are not
a substitute for these new composition obligations.
