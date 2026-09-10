# Local Drain, Reply Admission And Versions: R65

This advances A1/A2 in [#182](https://github.com/harsh-nod/fe2o3/issues/182).
It does not complete either milestone, grant executable authority, establish
HIP/HSA parity, or qualify compute/copy overlap.

## Cooperative Drain

`RuntimeAsyncProgressHandleV1::begin_drain(max_ticks)` closes admission across
all cloned handles and returns an executor-neutral future. Closing admission
and accepting a command share a mutex, establishing one finite accepted prefix.
The single lifecycle slot is independent of command-channel and reply capacity.
Rejected payload destructors, callbacks, wakers and blocking owner joins run
outside that mutex. Subsequent calls cannot admit commands; their local shape
or capacity checks can fail before they reach the closed admission gate.

The owner continues accepted commands, active graphs, standalone operations and
event waiters. Persistent stream-progress registrations do not prevent drain.
It also snapshots and observes Context submissions created by generic commands,
including unregistered predecessors needed by tracked dependent operations.
Native polling uses the ordinary Context completion transition, not a second
completion implementation. Already-terminal records need no additional poll;
an empty native roster needs no flush.

`Quiescent` requires an exhausted command queue, no active graph, no operation
or event waiter, and no retained pending submission. Failed completion and
`QuiescentWithoutResult` are quiescent, not successful results. The report's
counts describe retained Context submission records; already-retired graph
submissions and rejected launches are absent. The first retained failure is
selected deterministically by submission identity.

The report is not a native cleanup receipt. Await it, then join the owner with
`shutdown` and inspect logical cleanup and explicit native shutdown separately.
The owner exits automatically after successful drain. A transferable engine
can instead return its Context under its existing thread-safety contract.

The budget is 1 through 1,000,000 cooperative ticks, not a wall-clock deadline.
Per-tick native polling/flushing has independent configured budgets, additional
to normal operation, graph and observer lanes. Initial snapshots scan retained
records and sort identities; final counts scan records. These costs and backend
calls, callbacks and wakers are not constant-time or preemptible.

Budget exhaustion seals and retains unresolved custody. Dropping the drain
future does not withdraw drain or release native resources. Stop APIs still
interrupt drain; interruption, terminal Context and adapter panic resolve as
`EngineStopped`, never a fabricated quiescent report. Unresponsive adapters
still require an external process deadline. Drain never grants retry permission.

## Reply Count Budget

`with_reply_capacity` configures 1 through 65,536 async reply cells per engine,
defaulting to 16,384. Commands, standalone operations and graphs share this
budget across cloned handles. The private non-cloneable permit follows the
shared producer/future cell and returns credit only when the last owner drops
it, after retained result and waker disposal. Abandonment does not refund a
still-queued producer. Retaining a completed future also retains its credit.
Admission returns `ReplyCapacity` without publishing work or reserving a graph
slot. `reply_cells_in_use` reports the count, not native release authority.

The one drain reply is separately bounded. Synchronous commands, event waiters
and stream-progress registrations retain their independent existing bounds.
This count budget and [R64's payload budget](runtime-async-admission-v1.md) do
not bound arbitrary callback/result bytes, caller allocation, allocator
overhead, executable residency, native pools or cumulative quarantine.

## Graph-Local Versions

Every admitted graph derives a historical storage lineage before reservation.
All validated effect endpoints partition each allocation into exact covered
byte segments; gaps are excluded. Same-node equal and overlapping aliases are
coalesced per segment. Read inputs identify the preceding planned producer;
writes retain their destination storage predecessor, which is distinct from
a copy's source input. Write effects do not prove full overwrite or initialization.

`expect_input_version(node, exact_read_region, source)` optionally requires
`InitialAtAdmission` or `ProducedBy(node)` for every segment of an exact declared
read region. A read spanning mixed producers cannot claim one uniform producer.
Omitted expectations still derive and check complete local lineage. Malformed,
unknown or non-predecessor producer expectations reject before reservation or
backend entry. Producer node IDs are interpreted within this graph only.

Before issue, the ledger validates every input and output without mutation.
Only then does it invalidate each output's prior availability and install the
exact pending writer. Successful backend observation and native retirement
precede version commit; commit precedes successor readiness. Complete-set
validation prevents a later mismatch from partially beginning or committing a
node. Failed writers remain unavailable even after definite submission rejection;
this conservatism is not evidence that the old bytes physically changed.

Terminal reports contain opaque versions bound to the existing private execution
occurrence identity, exact segment, and producer. Initial versions remain
`AvailableAtAdmission`; produced records are `Committed`, `Failed` or
`NotProduced`. No successful terminal report can contain a pending writer or
`InFlight` version. Input rows say whether availability was checked at issue,
not whether a kernel actually read those bytes. `current_at_terminal` means
runtime-established availability at the final reserved boundary, not freshness
at a later observation time. Reports are not executable admission tokens.

No previous report can authorize a later run. Ordinary Context writes are not
a persistent cross-run mutation ledger; every run starts a new graph-local
lineage. Compiler-authenticated effects and semantic-to-machine refinement are
still external requirements, not inferred from typed arguments or copy canaries.

Existing graph bounds remain: one queued/active graph, 256 nodes/streams,
1,024 effects and 64 KiB explicit arguments. R65 additionally caps coalesced
node/segment references at 16,384 and version records at 18,432; excess alias
partitioning now rejects before reservation. Report storage is preallocated.
Preparation is bounded by effect/segment and reference/expectation products and
ordered-map operations;
begin/commit are linear in that node's coalesced references, with no allocation.

## Verification Scope

Production uses three shared pure version predicates. Eight authenticated Verus
obligations cover exact available inputs, planned output state, absent pending
writer, exact predecessor, in-flight commit state, exact pending owner and
invalidated current pointer. Eight targeted negative mutations must fail.
Exhaustive phase/index boundary tests compare the Rust guards with their stated
conditions. Reply counts reuse the R64 checked accounting helpers.

These are guard and arithmetic proofs with reviewed Rust correspondence, not
proofs of the whole ledger, partitioning, mutex/CAS linearization, channels,
thread scheduling, drain termination, native completion or generated machine
code. Scripted tests separately exercise transactionality, admission races,
reentrancy, real producer/future lifetime, partial overlaps, failure/cancellation,
raw predecessors, active graphs, terminal/panic paths and conservative retention.

The R65 hardware profile is two repeated copy-only diamonds in one Context,
exact version/input manifests, full readback including padding, then idle drain
and explicit cleanup. It does not qualify drain with outstanding GPU work,
mixed-duration kernels, high native depth, physical overlap or performance.

## Remaining A1/A2 Acceptance

A1 still needs admitted general generated kernels, mixed-duration/out-of-order
and thousands-in-flight native qualification, complete native memory/signal/
kernarg budgeting, and active-work drain/failure qualification. A2 still needs
compiler-admitted repeated kernel/copy graphs, cross-run versions, pool and
executable residency admission, and measured compute/copy overlap. Disjoint
persistent compute/SDMA coexistence requires reciprocal exact-storage checks in
the lower KFD layer and runtime; it is not solely blocked by compiler admission.
Neither milestone should be closed on this local copy-only evidence.

The [A1/A2 swarm plan](runtime-a1-a2-swarm-plan.md) assigns the remaining work,
dependency waves, code ownership and independent implementation/proof/hardware
acceptance gates. It is a planning artifact, not additional implementation.

The [R65 evidence report](evidence/mi300x-r65-drain-versions-2026-09-10/README.md)
retains final CPU/proof results, signed-source hardware records and independent
capture, ELF, queue-census and shared-host cleanup audits.
