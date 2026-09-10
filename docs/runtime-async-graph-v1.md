# Runtime-Bound Graph Execution: R63

This is a local A2 implementation tranche for #182, not completion of that issue
or HIP/HSA parity. It consumes the existing `CompletionGraphV1`; it does not
introduce a compiler plan or supply the missing #134/#214 admission receipts.

## Contract

- Obtain process-local context/stream identities from the owning runtime context.
  They are descriptive, generation-scoped labels, not cross-process identities
  or executable authority.
- Construct `RuntimeGraphRequestV1` with an exact, one-to-one live stream binding.
  Bind each Future node once to an ordinary typed launch or same-device copy.
  Atomic/collective/peer operations are not graph operations in this profile.
- Launch argument getters run once during binding. The returned bytes and binding
  list are owned thereafter. This is not an atomic snapshot of application state.
  The original argument object is not retained; context allocations and the typed
  kernel are the runtime resource references.
- Admission runs the ordinary context's exact geometry, kernel, module, device,
  allocation, range and pointer-patch validators before committing a reservation.
  It also rejects unordered overlapping declared memory effects involving a writer.
  Declared effects still require independent backend/kernel admission.
- One queued or active graph per engine, one reserved graph per context. Admission
  requires no retained submissions/events, queued operation registry entries,
  event observers or registered progress streams. Allocations/modules/streams
  must already exist. Failure admits no native work.
- The reservation excludes ordinary context mutation, submissions, polling,
  release and cleanup, even for resources not named by the graph. Read-only
  metadata queries remain available. There is no public reservation bypass.
- Dependencies are completion-and-retirement gated. EventRecord/EventWait nodes
  are host joins, not fabricated native events. Independent ready streams can
  issue concurrently; native backend custody rules may reject otherwise valid
  fine-grained overlap.
- Each native token is polled and retired exactly once on success. Rejected polls
  and releases retain that token and retry observation/retirement, never issue.
  Successors become eligible only after successful native retirement.
- Failed/cancelled predecessors suppress successors. Already-issued independent
  work still drains before a terminal graph report. A terminal report can contain
  failed nodes: inspect every completion state, not only the outer Result/errors.
- Cancellation is an asynchronously observed request to close unissued nodes,
  checked before each issue boundary. It never withdraws issued work. Future Drop
  abandons observation only. Caller executor timeouts do not revoke custody.
- Unknown publication, backend panic and terminal backend failures seal/retain the
  context. They return no terminal completion report. Immediate engine shutdown
  with active/retiring tokens quarantines rather than drains. Cancel and await
  the graph before normal shutdown when complete retirement is required.

## Bounds And Scheduling

The profile caps graphs at 256 nodes/streams, 65,536 aggregate explicit argument
bytes and 1,024 declared effects. One shared engine slot bounds queued/active
graph snapshots. Arbitrary user encoder allocations and ordinary command
captures remain outside this retained-snapshot bound.

Readiness notifications are queued at most once per node. Cancellation can make
them stale; the driver rechecks state. The driver maintains a round-robin active
token queue and a separate full-stream flush roster. Issue/poll/release work uses
the configured per-tick operation budget; flushes use the flush budget.
Cancellation/failure propagation may visit the entire bounded graph.

Admission computes ancestry bitsets from the existing graph edges and sorts
effects by allocation/offset. Its overlap scan is worst-case quadratic in the
bounded effect count. It does not rebuild a second dependency state machine.
Failure origin at a join follows the actual observation sequence, not a
schedule-independent canonical cause.

## Verification Boundary

The R63 Verus file has eight abstract guard obligations and eight targeted
negative mutations: clean/exclusive acquisition, exact issue token, closed
issuance, empty/closed/exact release and terminal retention. Production uses the
three matching pure Rust predicates, exhaustively tested over finite inputs.
The authenticated runner/source/solver/negative-inventory gates are unchanged
apart from the added roster and refreshed pins.

These are predicate proofs, not a proof of the graph executor, its ledger,
threading, allocation failure, termination, GPU semantics or kernel effects.
Rust-to-Verus correspondence and private-token enforcement are reviewed; runtime
integration is tested; backend publication/quiescence truth is contracted.

CPU regressions cover mixed copy/compute diamonds, both branch completion orders,
frozen stateful arguments, resource exclusion, cancellation, dropped observation,
poll/release retries, failed sibling draining, terminal retention, owner panic/
shutdown, wake-time successor admission and repeated execution. Completion tests
cover unique/stale readiness notifications.

The R63 MI300X runner uses the inherited signed-source, offline static-musl,
full-symbol/dependency, topology, queue-census and owned-stage cleanup gates.
Its five-copy, four-stream diamond checks the exact 12-node graph report, five
successful native observations, all input/output/padding bytes, empty submission
registries and complete native shutdown. It is correctness qualification, not
evidence of achieved overlap, compiler admission, two-host execution or speedup.

The [R63 evidence report](evidence/mi300x-r63-async-graph-2026-09-09/README.md)
records two accepted MI300X runs from signed source, the complete CPU/proof
results, independent capture verification and the final shared-host cleanup check.

[R64](runtime-async-admission-v1.md) adds a private-construction descriptive
execution identity to terminal reports. Repeated identical structural graphs
receive distinct context-local generations; these are not data versions or
distributed epochs.

[R65](runtime-async-drain-versions-v1.md) adds bounded exact segment lineage,
optional graph-local producer expectations and input/output version records.
It also offers cooperative `begin_drain` without cancelling accepted graphs;
its result and owner cleanup remain separate. This does not add cross-run
version authority, compiler admission or measured compute/copy overlap.

## Still Required For #182

Compiler-authenticated admitted plans, cross-run versions/graph epochs/residency,
multi-device group placement and quiescence, authenticated two-host transport,
distributed collectives, failure/recovery campaigns,
end-to-end budgets and the precommitted scaling/performance gates remain open.
R64 supplies named-executor integration tests, not progress/fairness proofs.
