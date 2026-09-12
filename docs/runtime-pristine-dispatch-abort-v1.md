# Pristine Dispatch Abort V1

R82 adds a distinct ordinary fixed-dispatch transition for a recipe that has
never reserved an epoch. It is the native prerequisite for reusable generated
adoption, not generated adoption or publication itself.

## Ownership

`ComputeAqlQueueSessionV1::abort_unpublished_fixed_dispatch_v1` and the narrow
lane facade return the complete ordered `Vec<Gfx942FixedDispatchDataV1>`.
They do not return `Gfx942DetachedFixedDispatchV1` or a completion generation.
A private move-only continuation preserves the recipe's exact next dispatch
generation. Rebinding consumes it and creates a fresh recipe occurrence.
Initial, rebound and near-exhaustion counters are not converted to synthetic
recycled predecessors.

Admission requires an unpoisoned ordinary owner, no queue-bound epoch history,
no recycled generation, and every epoch slot exactly vacant with zero slot
generation. Cancelled reservations therefore reject even though their phases
are vacant again. Zero and exhausted next generations reject. Complete
data/premise cardinality, bounded control/data rosters, exact retained layouts
and initialized-content extents are checked before movement or native effects.
Persistent attachments, detached ledgers and unreleasable completion custody
also reject. The completion arena's historical generation need not reset.

The implementation is isolated in
[native pristine custody](../crates/fe2o3-kfd/src/queue_dispatch_binding/pristine_abort.rs)
and [live orchestration](../crates/fe2o3-kfd/src/queue_live/pristine_abort.rs).
The existing recycled constructors and persistent cancellation's legacy zero
state are unchanged.

## Disposal And Failure

Return-vector and identity capacity are reserved before native effects.
Complete data, its original initialization variants and content descriptors,
and the continuation are placed in retained abort custody before control
disposal. Kernarg is released first, followed by code objects in reverse order.
Each control authority is consumed at most once. No data backing is disposed,
copied or refunded by this operation.

Only successful control disposal followed by successful closing model retake
installs the detached ledger and makes the data available to the caller.
Cleanup errors and panics retain the complete data owner; closing failures
also retain it even when all controls have already been disposed. These
post-effect outcomes poison the queue and process gate, expose no continuation
and admit no cleanup retry. The original cleanup panic wins over a second
closing panic. A rejected opening loan restores an unconsumed attached owner;
the loan boundary's own terminal classification is not weakened.

Continuation and terminal abort custody move with primary/auxiliary lane
selection, including unwinding. Detached mutation/release retains the existing
complete identity and ordinal checks. Recycled detach, retained persistent
control release, persistent binding, recycled-labelled returning destroy and
SDMA demotion cannot reinterpret unpublished provenance. Ordinary queue/lane
destruction is admitted only after all detached data has been disposed.

## Rebinding

`bind_fixed_dispatch` accepts exactly one provenance: the existing detached
generation or the new unpublished continuation. Exact complete data identity
and cardinality must still match. This consuming API does not return input
data on rejection. Existing preflight rejection classifications are unchanged;
terminal poisoning invalidates any continuation still in the session.

R105's [shared pristine rebind custody](runtime-pristine-rebind-custody-v1.md)
roots the original inputs, preparation and admitted continuation before the
model loan. Opening rejection retains the continuation unconsumed; preparation
entry consumes it exactly once. Partial and completed preparation remain rooted
through closing retake and borrowed live-memory validation. Checked extraction
and nonallocating commit alone install the dispatch and clear its detached
ledger. Failed Complete cannot be installed after a suppressed error.

Entered pristine errors, including opening errors, preserve the existing
session/process-terminal policy. No failure returns retry authority. Abort and
control-disposal behavior are unchanged; abort's retry-safe rejected opening is
not the same policy as admitted rebind's opening failure. This does not make
the materializer a recoverable multi-buffer constructor.

## R82 Acceptance Boundary

Focused CPU coverage exercises actual epoch reserve/cancel/publish/complete/
recycle transitions; every slot's history; exact generation preservation;
all five storage variants and descriptor preservation; malformed premises;
control-only disposal; operation errors/panics for kernarg and both code
objects; partial/malformed GPU unmaps; all eighteen cleanup currentness
boundaries; and no-retry behavior with exact disposed-prefix observations.

The private memory fixture wraps actual fake-native records with test-only
dispatch facts and initialization premises. It checks mapped Host bytes and
retained Host/Device identities and N1/N2 debits. The Device descriptor fixture
does not authenticate real initialized Device bytes. This is not successful
Linux authority issuance, GPU execution or numerical correctness evidence.

Production-used orchestration/settlement helpers are tested with scripted
loan, closing-retake and validation outcomes. These exercise complete abort
escrow, process-gate requests, original-panic preservation, primary/auxiliary
state restoration, consuming rebind rejection and prepared-owner retention.
They do not execute the actual Linux model-loan/retake envelope or a complete
native rebind constructor.

R82 adds no authenticated Verus theorem or native executable refinement.
Existing recycle, epoch and resource-cost proofs must not be relabelled as
proof of this transition. Signed same-queue Linux abort/rebind/disposal,
primary/auxiliary-lane qualification and adapter proof remain open, as do
generated adoption, publication, completion and matched HIP/HSA performance.
