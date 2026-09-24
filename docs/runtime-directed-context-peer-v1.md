# Directed Context Peer Copies

Status: [CPU-qualified development candidate](evidence/dev-directed-context-peer-cpu-2026-09-24/README.md)
above the [directed scalar backend SPI](runtime-directed-scalar-peer-v1.md).
Native, formal-refinement and performance evidence remain separate boundaries.

## Public Contract

`RuntimeContextV1::directed_peer_copy_v1` requires an explicit
`RuntimeDirectedScalarPeerCopyBackendV1` implementation and returns the distinct
`RuntimeDirectedScalarPeerCopyV1` submission marker. Legacy peer copies, ordered
batches, generated launches and Worker protocols do not acquire this contract.
Only the native XGMI backend implements the production SPI.

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
or generic stream flush. Dedicated async-engine observer registration is a
separate integration: ordinary async observation still follows its existing
poll/flush scheduling contract and is not advertised as this one-action driver.

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

- Dedicated bounded async-engine registration and caller-owner integration.
- Native pending-input chains, diamonds, full output/canary checks, cross-stream
  progress, fault disposition and exact cleanup on admitted GPU pairs.
- Source-bound executable refinement for the changed Context transitions;
  historical model proofs retain their original source and do not prove this
  integration automatically.
- Aggregate byte accounting, matched HIP/HSA measurements and workload-scoped
  acceptance against precommitted thresholds.

This candidate does not close A1/A2, issue #182 or HIP/HSA parity.
