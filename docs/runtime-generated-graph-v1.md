# Generated Graph Execution

C6 development above [C5 typed completion](runtime-generated-typed-completion-v1.md).
Accepted milestones remain R125 Native CPU/test, R118B C1-C3 and R116/V3.
This is not A1/A2, #182, protected native, formal or HIP/HSA parity acceptance.

## Admission And Ownership

`RuntimeGeneratedGraphRequestV1` wraps the existing bounded ordinary graph and
fresh, move-only reserved tickets. Ordinary launch/copy actions remain unchanged.
Binding rejection returns the incoming ticket without replacing any prior node.
`take_reserved` recovers a ticket for explicit discard or corrected binding.
Dropping a ticket alone never disposes its parked runtime owner.

`try_submit_generated_graph_v1` uses the existing single queued/active graph slot.
Reentrancy, reply pressure, graph capacity, queue pressure and closed admission
return the original request. Queued discard and ordinary owner-thread rejection
also return all original tickets. Every generated ticket is checked against its
exact parked registry entry, Context generation, completion support and preflight
before committing a Context graph reservation. A failed Context reservation does
not consume the request. Execution errors after commitment do not promise ticket
return or native quiescence; existing shutdown retains ambiguous custody.

The graph never owns the non-Send native carrier. That carrier, original reply
producer and native/Context custody remain in the existing operation registry.
The graph owns each activated node's one original C5 completion observer. No
second reply, generated scheduler, waiter or completion consumer is introduced.
Its bounded active roster alternates issue and observation as before. Terminal
suffix cleanup scans the bounded node roster only on failure or new cancellation.

## Dependency Boundary

Dependencies release only after the original C4 settlement, decoder and result
gate commit produce a successful C5 receipt. Physical completion alone does not
release a dependent. Engine/readback errors fail that node, propagate dependency
failure and exact-discard the unactivated suffix while independent branches can
finish. Mixed ordinary launch, generated invocation and copy nodes use the same
executor and Context reservation.

`RuntimeGeneratedGraphReportV1` adds node receipts and generated errors alongside
the unchanged ordinary report type. A receipt can be used with the caller's
original `GeneratedRuntimeChargedResultV1::take_completed_v1`; the graph does not
return an already-consumed completion future. Receipts retain only gate metadata,
not native authority, replay permission or a second result allocation.

Graph edges order already-reserved invocations. Their input DATA is frozen; an
edge does not rebind a successor to a predecessor's output. Generated storage is
not represented as ordinary allocation-version lineage. Automatic dataflow
rebinding, cross-run reuse and heterogeneous output bundles remain separate work.

## Reservation, Cancellation And Drain

The exact graph token travels through owner-local activation preflight and is
stored in the unpublished hold. New holds require the exact open reservation;
retained validation/retirement remains possible after issue closure. Public
ordinary access is not relaxed. Issue and completion validate the hold before
native/currentness callbacks. The token accessor remains Context-private.

Graph release additionally requires no unpublished holds, generated attempts,
shell markers, completion callbacks or logical/native submission/event indexes.
These are additional Rust guards beyond the existing R63 release model, not a
claim that the old model proves these new predicates.

Cancellation exact-discards only unactivated graph owners before recording
cancellation. It cannot withdraw activated work or touch unrelated parked owners.
Dropped graph observers do not cancel execution. Internal activation remains
available to accepted graph successors after ordinary drain closes public
admission. Live completion-capable direct invocations likewise retain their
Adopting/Adopted prefix during drain. Legacy no-completion adoption cleanup and
actual Stop retain their prior disposal boundary.

Drain snapshots exclude exact generated submissions because the generated
registry already progresses and retires them. They still contribute to pending
counts; exclusion cannot manufacture quiescence. Drain requires an inactive graph
and zero active drivers. Budget exhaustion retains and reports both, never success.
Stop first cancels unactivated graph owners. It then consumes already-published
generated completion cells without issuing, polling native work, settling or
decoding. If this removes the last active entry, the existing release checks
can close the graph normally. Any still-pending generated or ordinary action
conservatively retains the reserved Context and drivers. A callback already
running when Stop is queued finishes before the owner dequeues Stop; Stop is
not callback preemption.
Terminal/stopped progress entry points cannot reenter execution hooks.

## Evidence And Remaining Work

The [development archive](evidence/dev-c6-generated-graph-2026-09-17/README.md)
records qualification separately from exploratory diagnostics. Tests cover exact
ticket/domain identity, dependency timing through physical completion, decoder
error/panic, unrelated parked custody, transitive cancellation, post-admission
activation failure with an independent branch, mixed ordinary actions, exact
Context access/settlement, actual owner-thread drain, dropped observation and
budget exhaustion. Public doctests check API/Send linkage and move-only types.

Frozen-source GNU and scoped musl qualification each pass 934 runtime and 271
host tests, with seventeen and four ignored respectively. All 61 doctests,
strict Clippy, formatting, no-default-feature checks, unsafe-source policy and
transcript-parser calibration pass. Scoped musl disables optional legacy HIP
linkage; it does not qualify an unrestricted musl/HIP build. The archive binds
all 23 changed source files and both targets' exact executables and test rosters.

Fixtures exercise shared production control flow with scripted backends and
inert domain metadata. Constructed Context settlement is not native settlement.
The subsequent [owned-Stop correction](evidence/dev-c6-owned-stop-2026-09-17/README.md)
adds deterministic owner-loop phase coverage and avoids retaining an otherwise
settled Context when Stop precedes graph observation of the original ready cell.
Its frozen-source GNU and scoped musl each pass 942 runtime and 271 host tests,
with seventeen and four ignored respectively; all quality gates and 61 doctests
pass. The prior 23-file C6 archive remains unchanged.
Protected Worker/carrier/native graph composition and its failure campaign,
native Stop/failure qualification, formal Rust/model correspondence,
production journals, aggregate residency bounds and matched HIP/HSA benchmarks
remain open. This packet does not run a GPU workload or measure performance.
