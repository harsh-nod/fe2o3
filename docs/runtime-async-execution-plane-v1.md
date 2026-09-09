# Async Execution Plane: R61

This is a local integration slice of [issue #182](https://github.com/harsh-nod/fe2o3/issues/182),
not completion of its A0-A7 roadmap. It introduces no network transport, graph
compiler, compatibility backend, or new executable authority.

## Ownership

`RuntimeAsyncOwnedEngineV1::spawn_with_progress` transfers a `Send` factory,
not a backend. The owner thread constructs the complete `RuntimeContextV1`,
executes commands, advances operations, cleans logical resources, and calls
`RuntimeOwnedShutdownBackendV1` for explicit native teardown. Direct KFD,
multi-device KFD, and the separate copy-only native-XGMI backend implement that
extension using their existing `shutdown_native_v1` methods. No unsafe `Send`
implementation, new KFD queue, signal, mapping, or loader is introduced.

The factory owns initialization cleanup and panic safety until it returns its
complete context. After that transfer, command/adapter panics seal the context.
Incomplete logical cleanup, failed native teardown, or an unwinding progress
adapter causes retention of the whole context until process exit. The owner
cannot transfer a thread-affine context back to the caller. An isolated process
is required when this terminal retention must be reclaimable without ending the
application. Shutdown is a stop followed by one cleanup pass, not a drain or
proof that pending work completed. Potentially blocking backend methods retain
their existing contracts; an external process deadline is still needed against
an unresponsive adapter.

## Operation Progress

`enqueue_with_context` returns immediately after bounded enqueue. Its standard
future receives the command result; it does not imply GPU completion. Discarded
commands resolve their reply as `EngineStopped`. Generic callbacks are local
API calls, not serializable distributed requests, and must not block progress.

The progress handle adds `launch`, `copy_async`, and `peer_copy`. Their futures
cover submission, background observation, and progress. Each operation reserves
a registry entry before invoking the existing context admission path. A
consumed submission closure cannot run again. Rejected completion observations
retain that same submission, a saturating rejection count, and at most one last
error; they retry observation, never dispatch. Quiescent errors, successful or
failed completion, and terminal backend loss remain distinct results.

Dropping an operation future neither removes its registry entry nor withdraws
its stream progress. This differs intentionally from the older paired
`event_future_with_progress`, whose Drop withdraws its observer-owned progress
registration. The existing event APIs retain their contracts unchanged.

Submission and native resources remain context-owned after a result. Explicit
release or inspected shutdown is still necessary. A stopped future, timeout,
host-side panic, or absent result never authorizes native resource release or
automatic dispatch retry. Pending operations are not pinned against generic
context commands before backend admission; an intervening release therefore
rejects at normal admission. This is not yet graph-wide resource reservation.

## Bounds And Scheduling

- The existing command channel bounds queued commands. The operation registry
  independently admits at most `waiter_capacity` entries (maximum 65,536).
- Each operation tick advances at most `polls_per_tick` entries and flushes at
  most `flushes_per_tick` distinct streams. These budgets are additional to the
  existing event-poll and observer-stream-flush lanes, not global totals.
- A persistent refcounted stream roster has no more entries than the operation
  registry. Its cyclic flush cursor is independent of cyclic operation polling,
  including when the poll budget visits only part of the roster.
- Admission/retirement is `O(log S)` for `S` live streams. Each tick is bounded
  by its operation/flush budgets, apart from the contracted adapter calls.
- Record capacities do not bound arbitrary closure/argument/result bytes,
  caller-retained completed futures, all native pools, or cumulative process
  quarantine. Those end-to-end budgets remain #182 work.

## Evidence Boundaries

| Property | Status and boundary |
| --- | --- |
| Release requires normal loop return, complete context cleanup, and attempted successful native shutdown | Eight-property R61 abstract Verus file includes this rule; actual observations are contracted adapter results. |
| Reply resolution and registry admission predicates | Shared pure Rust helpers are used by production; exhaustive boundary tests check them. Whole-engine executable refinement is not established. |
| Future Drop, stopped observers, and resource retention | Abstract observation projection preserves custody and cannot manufacture completion; scripted thread/operation tests validate actual behavior. |
| One owner thread, bounded records, out-of-order observation, independent flush fairness | Scripted CPU validation, including 2,048 in-flight operations. Not 2,048 hardware queue slots or a throughput result. |
| Native copy behavior | The default-feature canary and guarded two-run runner exercise one selected gfx942 GPU, one stream, H2D/D2H, full input/output/padding bytes, abandoned observation, and explicit cleanup. Hardware evidence is recorded separately. |
| Verus/Rust/solver/source identity | The existing authenticated runner, source auditor, expected-negative inventory, closure checker, and SHA256 pins are retained. R61 adds eight obligations and eight negative mutations; it does not prove Linux, KFD, firmware, GPU, threads, channels, or executors. |
| Generated-kernel production authority | Unchanged external dependency on #137/generated-host/Worker V3 admission. Neither typed metadata nor a factory grants authority. |

## Remaining Acceptance

#182 must remain open until its full acceptance matrix is met:

- A0: distributed threat model, stable identities, bounded protocols and budgets.
- A1: general generated-kernel production authority, operation-addressable
  prepublication cancellation, timeout policy, end-to-end byte budgets, multiple
  executor integration, mixed-duration/high-depth hardware qualification.
- A2: admitted graph driver, exclusive resource reservation or pins, exact data
  versions, graph epochs, joins, residency and repeated overlap qualification.
- A3: integrated group placement/sharding/replicas, all-admitted-GPU qualification,
  partial failure and group quiescence. Existing child backends alone are not it.
- A4-A5: authenticated two-host control, membership/coordinator epochs, exact
  artifacts, receipts, leases, host-staged data transport, versions, broadcast,
  reduce-scatter, all-gather and all-reduce. All remain unsupported here.
- A6: every-boundary distributed failure/drain/recovery campaigns, including
  unknown publication without replay and retained-resource accounting.
- A7: precommitted eight-GPU/two-host performance gates, scaling/collective
  bandwidth/tail/CPU/memory results and complete production dependency audits.

One shared MI300X host does not satisfy the two-host gate. A busy GPU must not
be included in an all-device campaign without an agreed availability window.
Two local processes, scripted peers, and abstract proofs cannot substitute for
authenticated two-host GPU execution.
