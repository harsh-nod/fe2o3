# Finite Async Generated Preparation

R79 implements B3-OWNER: a finite owner-thread preparation operation and bounded,
nonexecuting custody. It composes the [R75 local-driver boundary](runtime-owner-local-operations-v1.md),
[R76 Context preparation](runtime-context-generated-preparation-v1.md) and
[R77 persistent projection](runtime-persistent-generated-projection-v1.md).
It does not implement native adoption, submission or generated result completion.

## Interface And Ownership

The [runtime bridge](../crates/fe2o3-runtime/src/async_engine/generated_operation.rs)
exposes `RuntimeAsyncPreparationV1<E>`, `RuntimeAsyncPreparedTicketV1` and
`try_discard_prepared_v1`. The specialized `try_prepare_gfx942_v1` accepts a Send
factory callback over only an immutable checked-device reference. The mutable
Context preparation helper remains private. Its result need not be Send and
never leaves the owner-local driver.

The [host adapter](../crates/fe2o3-host/src/generated_runtime_invocation.rs)
adds `prepare_generated_context_invocation_async`. It requires the actual
protected executable evidence before enqueue, then uses the existing private
charged preparation constructor and projection inside the immutable Context
scope. The complete Context-bound carrier retains original storage, private
decoder, authority and the original shared result account. Cloning that account
does not create another account or debit. No production authorizer or compiler
evidence fallback is introduced.

The preparation future returns an outer engine result and an inner preparation
result. Success is an opaque, non-Clone ticket, not a GPU submission or typed
output. The ticket carries exact Context generation and private Arc identity;
it exposes no payload, decoder, native address or launch authority.

## State Transitions

| State | Transition And Custody |
| --- | --- |
| Queued | Reserve one existing reply cell and enqueue an inert Send factory. Operation capacity and owned-shutdown capability are checked before materialization/preparation. |
| Active preparation | Install the driver, check Context identity and the existing R62 cancellation transition, then invoke preparation once. It has no flush stream. |
| Parked | Move the entire prepared driver into the preallocated parked roster before resolving the preparation reply. It is no longer polled as an active operation. |
| Discard accepted | Consume the exact ticket, dispose never-adopted storage on its owner, then report success. Destructor panic terminalizes the Context; it is not successful disposal. |
| Stopped | Stop observations without releasing prepared custody. The existing owned cleanup/native shutdown decision retains or disposes the driver with the Context. |

Preparation's R62 `SubmissionStarted` phase means callback entry, not native
publication. `ObservationFinished` means the host preparation result is ready.
Cancellation only wins before callback entry; observer deadlines and dropped
futures do not cancel retained preparation. The synchronous callback must return
for progress to resume. The parked driver is not polled repeatedly; this does
not prevent a blocking or nonterminating caller callback.

The [operation registry](../crates/fe2o3-runtime/src/async_engine/operation.rs)
counts active plus parked entries against the same operation ceiling. Both
rosters are preallocated before Context construction. Only active entries
participate in progress and graph exclusion; stream membership is removed once
on parking. `operations_remaining` in drain reports excludes never-adopted
parked preparation and is not a storage-disposal receipt.
Operation/reply counts do not bound arbitrary factory captures, payload bytes or
aggregate metadata; MEM-5 remains open.

Discard lookup is a linear scan bounded by that operation ceiling. Exact Arc
identity plus Context generation prevents foreign/stale ticket aliasing. Queue,
reply-capacity, reentrancy and foreign-Context admission rejection return the
unchanged ticket in `RuntimeAsyncPreparedDiscardFailureV1`. Once enqueued, Stop
may consume the command and resolve `EngineStopped`; its payload remains in
owner custody. Dropping a ticket alone does not discard anything, so unreachable
parked entries keep their capacity until owned shutdown.

Successful owned shutdown performs Context cleanup and native finalization
before disposing drivers. Driver disposal is now inside the existing unwind
boundary and pops entries individually: a destructor panic retains later owners
and Context rather than reporting successful release. The currently unwinding
object follows Rust destruction semantics; no claim is made that a panicking
destructor can restore its partially destroyed payload.

## Validation Boundary

The CPU tests exercise the actual factory/command/registry paths with owner-local
Rc payloads, bounded replies and fake-backend Contexts. They cover capacity,
preparation cancellation/rejection, exact ticket identity, observer loss, no-flush
parking, latest-waker replacement, graph admission coexistence, Stop races,
discard and shutdown failures, and post-park completion/destructor panic.
The complete R75 async regression suite remains a required gate.

Four compile-fail doctests cover ticket cloning/private extraction, non-Send
captured input and checked-device borrow escape. The host example is compile-only;
the host constructor/account check is source-wiring evidence. Generic payload
drop counters do not measure actual R73 charged storage through successful
protected construction. That positive integration remains unqualified.

Existing R61 capacity/reply and R62 control guards are reused. There is no new
Verus theorem or claim of whole-adapter/executor refinement. See the
[local evidence](evidence/local-r79-async-preparation-2026-09-10/README.md)
for exact source gates and their separate proof/hardware boundaries.

## Following Packets

B3-DATA-REP must provide closed complete-roster access without generic payload
extraction, cropping or another encoded-host copy. B4-RESERVE must implement a
new completion reply and complete readback ownership in the original account.
Only then can B3-DATA-ADOPT move the same rooted carrier into non-discardable
adopting custody before native effects. That custody must join lane exclusion,
graph, drain and cleanup tracking; today's parked state is not native-ready.

B3-ISSUE supplies exact publication-time authority through actual flush/retry;
B4-COMPLETE validates/readbacks/decodes the complete returned roster. B5 and B6
then expose generated launch completion and graph/drain integration. Genuine
compiler evidence, Linux qualification, executable proof composition and matched
HIP/HSA measurements remain required, not consequences of this preparation API.
