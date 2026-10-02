# Directed Context Peer Copies

Status: the [CPU-qualified Context contract](evidence/dev-directed-context-peer-cpu-2026-09-24/README.md)
above the [directed scalar backend SPI](runtime-directed-scalar-peer-v1.md) now
has [bounded integrated native qualification](evidence/dev-multigpu-directed-peer-2026-10-02/README.md)
for prequeued three-GPU chains and fanout, now extended with
[dependent D2H qualification](evidence/dev-multigpu-directed-readback-2026-10-02/README.md).
The [late-admission checkpoint](evidence/dev-multigpu-late-admission-2026-10-02/README.md)
also qualifies directed peers and compute consumers admitted after an exact
native producer has published and still retains its physical owners.
Formal refinement, broader native coverage and performance evidence remain
separate boundaries.

## Public Contract

`RuntimeContextV1::directed_peer_copy_v1` requires an explicit
`RuntimeDirectedScalarPeerCopyBackendV1` implementation and returns the distinct
`RuntimeDirectedScalarPeerCopyV1` submission marker. Legacy peer copies, ordered
batches, generated launches and Worker protocols do not acquire this contract.
The separate native XGMI owner and integrated multi-device KFD router implement
this SPI. The latter retains staging by default; its native-peer opt-in can use
admitted XGMI routes for initialized, equal full-buffer directed copies.

The entire device/stream/region route and original event-to-producer roster are
bound before backend entry. Dependencies must belong to this same directed
profile. Duplicate producers, including two events for one producer, reject.
Context retains every producer independently of the public event lifetime.
Dependency depth is at most 256; each operation has at most 256 dependencies.

With the optional version journal, an input can reserve an earlier pending
directed producer's output. Admission requires the exact pending writer,
allocation/member, event/submission, device, allocation extent, attempt epoch,
lineage and source range. The producer's actual destination range must contain
the consumer's source range; a whole-allocation journal writer does not prove
that every byte was produced. A failed or unknown producer is an ordinary
reservation refusal, not permission to read a successful output.

The one-source producer reservation shares the stable-reader budget. Its complete
original binding is rooted before acquisition and backend entry. Resolution
does not revisit the old writer slot: a producer can settle and its writer slot
can be reused while downstream reservations still retain their own result.
This is custody and version bookkeeping, not a new initializedness theorem.

## Dependent Readback

`RuntimeAsyncCopyBackendV1::supports_pending_directed_peer_readback_v1` is a
separate, default-false opt-in for `copy_async` from a pending directed peer's
DeviceLocal output to HostVisible memory on the destination device. Ordinary
peer-readback support alone does not enable it. The readback remains a distinct
same-device submission, not another directed peer.

Journal admission requires the exact current producer, covered source range,
device, writer epoch and lineage. Public producer events can be released after
admission; independent result retains survive until reconciliation. Cancellation
releases only the consumer's custody. A failed or unknown producer cannot become
successful host output, and ambiguous native failure still seals the router.

The integrated KFD router admits these readbacks behind selected native directed
copies before or after native publication. Admission retains router metadata and
staging, without reserving child SDMA ahead of the peer. Explicit readback
progress drives retained directed ancestry and, when necessary, one authenticated
already-started native resource blocker. A resource sibling's result is not a
success dependency. Child DMA starts only after the required owners are restored.
This readback opt-in does not widen application-kernel authority.

## Late Native Admission

An eligible full-buffer directed successor can retain native transport after an
exact directed peer extracts its endpoints. The router authenticates the
in-flight owner, endpoint, pair reservations and directed provenance before
preparing an empty successor root. It never borrows or clones the parent's
physical allocations. Existing dependency and shared-source ordering remain
required; ordinary profiles, partial ranges and absent routes keep their
existing fallback behavior.

Producer-aware compute can likewise enter metadata-only deferred custody when
its exact published directed producer owns the destination child. Immutable
stream/depth/profile/region identities and independent result retains survive
public event release. The earlier prepublication native-permit path remains
unchanged. Compute enters the child only after required producers succeed,
restore their owners and pass the normal child capacity/authority checks.
An authenticated already-started native resource sibling can be progressed
without making its result a success dependency.

`retained_compute_xgmi_copies_v1` reports stored native Published/Ready roots
with retained ownership, without sampling a fence or advancing execution.
The new witnesses observe a sole first peer's retained publication before
admitting its successor. This is neither current GPU activity nor overlap,
completion, allocation-access authority or arbitrary mixed-graph admission.

## Observation And Progress

Native completion and Context logical completion are distinct. The backend may
finish a consumer before Context has observed any of its producers. The original
terminal backend fact is retained, while public status remains Pending until
the required logical reconciliation is complete.

Submission poll/wait, event poll/wait, stream synchronization, public drain and
owner async-drain observation use the same planner. Each submission observation
either observes the requested operation, polls one exact retained producer, or makes only local
progress. It never observes a consumer and then polls a producer in that same
observation. Stream synchronization performs one such observation for each
snapshotted pending member under its existing shared deadline, so the aggregate
call can perform multiple backend actions. Subsequent observations do not keep
polling an already-successful native consumer. Explicit producers can be on another stream and their public events
can already be released.

The planner uses a fixed 256-identity path and at most 513 traversal/cursor steps
per pass, with at most three passes per submission observation. Cursors persist
across calls; a shared predecessor settles once. Each node's complete roster and
input roots are validated once per contiguous visit, not once per cursor
increment. That local cache never crosses a node change, settlement or backend
boundary. Touched-roster validation is bounded by the dependency limits, not
constant time. Each submission observation performs at most one backend action.
These count bounds do not prove wall-clock latency or fairness.

`progress_directed_peer_copy_v1` selects either one exact directed backend
progress quantum or one ordinary producer poll. It adds no wait, sleep, thread
or generic stream flush. The additive
[directed async adapter](runtime-directed-async-peer-v1.md) now consumes this
quantum without an implicit operation flush. Ordinary operations and explicit
observer/progress registrations retain their independent scheduling contracts;
the action bound is not global to a stream or engine tick.

Consumer Success requires all exact Context predecessors to be Succeeded and
the exact producer reservation, when present, to report Success. A native
consumer Success with a retained producer Pending, Failed or rejected observation
is a contract contradiction: Context seals and retains resources. Discarded
producer results and Unknown reservations conservatively produce
QuiescentWithoutResult, never invented Success.

Failure, confirmed quiescence and definite cancellation settle the consumer
without waiting on unrelated pending predecessors. Its input reservations release
before its own writer settles, then dependency retains discharge and callbacks
publish exactly once. A cancellation request after retained native Success
returns TooLate locally, without another backend action. Terminal failures and
panics preserve uncertain custody.

## Remaining Qualification

- Broader native directed async/owner integration beyond the recorded witnesses.
- Native partial-failure disposition and isolation, and broader mixed-graph
  admission beyond the qualified directed/compute profiles. Late witnesses
  qualify both orders of three-GPU chains/fanout and two-GPU compute pipelines,
  with full output and exact cleanup, not arbitrary graphs or kernel authority.
- Source-bound executable refinement for the changed Context transitions;
  historical model proofs retain their original source and do not prove this
  integration automatically.
- Aggregate byte accounting, matched HIP/HSA measurements and workload-scoped
  acceptance against precommitted thresholds.

This candidate does not close A1/A2, issue #182 or HIP/HSA parity.
