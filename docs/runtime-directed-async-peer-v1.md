# Directed Async Peer Copies

Status: [CPU-qualified development candidate](evidence/dev-directed-async-peer-cpu-2026-09-24/README.md).
GNU and musl each pass 1,325 runtime tests with twenty hardware-only ignores;
46 doctests, formatting and strict all-target Clippy pass on unchanged inputs.

This additive owner-engine adapter consumes the
[directed Context contract](runtime-directed-context-peer-v1.md). It does not
change the backend SPI, Worker wire formats, legacy peer copies or generated
operation admission. Development qualification is recorded separately from
native, formal-refinement and performance acceptance.

## Public API

For a backend implementing `RuntimeDirectedScalarPeerCopyBackendV1`,
`RuntimeAsyncProgressHandleV1` exposes `directed_peer_copy` and
`directed_peer_copy_tracked`. Both take the exact stream, source/destination
regions and dependency events and return the existing operation future carrying
`RuntimeDirectedScalarPeerCopyV1`. The tracked variant retains ordinary local
pre-submission cancellation and recoverable timeout observation.

Dependency snapshots use the existing bounded, compacted payload charge and
reply/command/operation admission. Live Context identities, ranges, producer
events and journal reservations are checked on the owner, not at enqueue time.
Distinct events naming the same producer therefore reject at Context admission.

The original methods expose the submission only with its final observation.
The additive [early-event API](runtime-async-operation-events-v1.md),
`directed_peer_copy_with_event`, returns an independently budgeted exact event
receipt before final observation. Callers can compose pending directed chains
and diamonds through those receipts. It reuses this operation driver and adds
a separate event-recording advance, not a second scheduler. General admitted
multi-device graph composition remains a separate A2/A3 boundary.

## Progress

One private compile-time policy selects the observation call and the operation's
automatic flush identity. The common driver still owns submission-once,
classification, rejection counters, replies, cancellation and Stop behavior.
Ordinary operations continue to poll and register their stream for flushing.
Directed operations call `progress_directed_peer_copy_v1` and register no
automatic flush identity, including before their first advance.

Submission and observation occur on separate advances. A directed observation
performs at most one backend action, or only local reconciliation, under
the existing bounded dependency traversal. Native consumer success is not
logical success until its exact producers and input reservations reconcile.

This is not a global per-stream or per-tick one-action guarantee. Other ordinary
operations on the same stream, event observers, explicit stream registrations,
graph work and drain have their own scheduling contracts and budgets. An
ordinary operation or explicit registration may independently flush that same
stream later in the tick. Multiple directed drivers may each advance once.

## Custody And Errors

Factories and drivers own inert submit closures before entry; accepted native
custody and producer reservations remain in Context. Dropping an observer does
not cancel work. Tracked cancellation wins only before submission starts. Stop
resolves observation without granting completion, release or replay authority.
The adapter introduces no requirement that the backend itself be Send.

A live-Context rejected observation remains retryable and records its diagnostic.
A producer contradiction can seal Context while preserving a rejected backend
diagnostic. The shared driver finishes that original error, rather than hiding
it behind a later EngineStopped reply. The registry's existing terminal check
still retains the driver and Context custody until the owner cleanup policy
permits disposal or retains the owner for process teardown.

## Remaining Gates

The [exact native owner diamond](evidence/dev-xgmi-directed-owner-native-mi300x-2026-09-24/README.md)
now validates pending inputs, physical copies, complete buffers and ordinary
cleanup. Broader native graph shapes, fault cleanup, executable Rust/model
refinement, aggregate memory accounting and matched HIP/HSA measurements remain
separate. CPU fixtures are not native execution or proof evidence. This adapter
alone does not close A1/A2 or issue #182.

### Native Witness

The [native witness](runtime-directed-owner-witness-v1.md) is a sibling
of `gfx942-runtime-xgmi-segments-owner-smoke.rs`. Preserve
that historical example and parser: they deliberately qualify refusal of
pending dataflow, not this newly admitted profile. The separate
`xgmi_directed_owner_campaign.py` / `xgmi_directed_owner_native.py` controller
reuses strict, separately recorded two-endpoint observations and owned cleanup.

The witness uses five 64-KiB device-local allocations on two admitted
GPUs and four directed copies, each with a distinct destination-owned stream.
A root copy feeds two reverse-direction branches; the final forward copy reads
one branch and depends on both. This is a dependency diamond, not a two-source
data merge. Every copy transfers a 32-KiB subrange at independently selected
offsets, leaving distinct prefix/suffix canaries in each destination.

Create the first three pending submissions and exact events through an owner
Context command. With one command and one operation advance per tick, use an
owner gate to enqueue the tracked async join followed by event release before
the join's first progress. Do not add separate stream-progress registrations.
Require all four exact Context results to succeed, no rejected observations,
zero remaining snapshot charge, and full readback of all five allocations:
327,680 checked bytes including 131,072 payload bytes, 131,072 destination
guard bytes and the unchanged 65,536-byte source.

Use one absolute workload deadline. Always inspect owned shutdown, including
when the workload oracle fails; print success only after complete cleanup and
native disposition. A fresh strict parser must bind the exact selected device
IDs and every result field, with hostile mutations. Run only from signed clean
source and fresh source/ELF/tool identities. Point-in-time host admission is not
an exclusive reservation; even a passing graph is not fault, formal-refinement,
performance, general concurrency or HIP/HSA parity evidence.
