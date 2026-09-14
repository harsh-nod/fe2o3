# Returning Control Cleanup Custody V1

Status: R115 locally accepted at the lower returning-control CPU/test boundary,
with [285 retained artifacts](evidence/local-r115-returning-control-cleanup-2026-09-14/README.md)
and two passing independent archive reviews. The preceding accepted runtime
checkpoint is R114 `9eaf19141e8af6ade490feba3062c8b49d9b38ca`; the publication
parent is `0265025b96f25a7cb79c97b576bb25cb38b15df5`.

## Boundary

R115 accepts the returning-control subset of N4-R2: the lower consuming
`release_non_data_after_recycle` and
`release_non_data_for_returning_destroy` paths in `queue_dispatch_binding.rs`.
Both use the new private `queue_dispatch_binding/control_release.rs` root and
the existing [R114 control cleanup adapter](runtime-pristine-control-cleanup-v1.md).
Existing public signatures and generation semantics are preserved.

This packet does not qualify persistent returned-data bridges, ordinary data
disposal, live model-loan/retake composition, queue teardown, native GPU
execution, authenticated proofs or performance. In particular, tests invoking
scripted native leaves are not evidence of successful Linux/KFD execution.

## Ownership And Ordering

`ReturningControlCleanupCustodyV1::new` moves the complete original dispatch
owner before any validation or fallible operation. It retains kernarg, a forward
code iterator, code identities, packets, data authorities and their premises,
the original generation owner and persistent-control state. Construction does
not allocate or duplicate resource authority.

Cleanup is one-shot. It first validates the selected generation mode, then
data/premise cardinality, then fallibly reserves all returned-data capacity.
No disposal callback precedes these checks. Overflow or insufficient capacity
returns `HostAllocationCapacity` without entering native cleanup.

Kernarg is cleaned first, then code in original forward order. Each actual token
is installed in the active `ControlCleanupCustodyV1` before the borrowed lower
callback. Only confirmed Complete clears that slot. A callback returning `Ok`
without Complete rejects; errors and panics preserve the exact mapped,
unmapped or native-disposed state and the untouched forward suffix.

After all controls complete, data and premises move in their original order
into the preallocated output. Complete stays inside the root until explicit
`take_completed`; extraction is one-shot and performs no later fallible work.
The iterator and preallocation keep orchestration linear in controls plus data,
with no repeated front removal or per-control allocation. This observation is
not an authenticated complexity proof or a measured speedup.

`release_returning_with_v1` retains the complete root before returning an error
or resuming the original panic payload. Production uses the existing permanent
retention policy; tests observe that same wrapper through an injected sink.
No new terminal slot or independent memory engine is introduced. A repeated
cleanup attempt cannot re-enter native disposal or extract failed output.

## Compatibility

After-recycle requires actual recycled completion history. Returning destruction
also accepts never-published or cancelled-only history and returns generation
zero. Out-of-order recycling returns the maximum recycled generation. Cancelled
reservations and exhausted next-generation counters do not themselves prohibit
cleanup. Generation rejection still precedes malformed retained cardinality.

Every returned authority retains its corresponding premise and initialized
state. Existing `into_data()` deliberately discards stale pre-dispatch content
descriptors; those descriptors do not become current device-content authority.
Output extraction preserves the reserved vector allocation.

The lower R114 adapter still owns unmap/release model transitions, separate
revision preflights, malformed-prefix precedence, currentness checks, native
record settlement and retained charges. Final-currentness failure after
disposal is distinct from release-model-commit rejection: both retain a disposal
receipt, but only the latter has already settled the native record and VA charge.

## Validation Contract

Fifteen new tests comprise fourteen behavioral tests and one supplemental source
routing guard. The routing guard is not a compiled behavioral mutation target.
The main fixture has three distinct controls and five data variants. A genuine
preparation fixture adds populated code/packet metadata and writable ranges.

Require exact constructor snapshots, first/middle/last destructive errors and
panics, all eighteen currentness points in both modes, partial/malformed unmap
outcomes, injected projection failures and actual certificate commit rejection.
Compare native identities/order, model commits, record settlement, storage,
charges, typed custody, untouched suffix, full premises and output cardinality.
An explicit no-entry callback tests retry rejection independently of the lower
adapter's own one-shot guard.

Generation tests drive reserve, publish, completion and recycle transitions.
Capacity tests cover both overflow and insufficient reserved space. Wrapper
tests require retention before exposing errors or original panic payloads.
Preserve accepted pristine, lower-cleanup, transport and preparation regressions.

The closed, independently reviewed campaign passes GNU/musl with 2,727 tests and
five ignored each, preserving all 49 executable identities/order and prior test
multisets plus exactly fifteen new tests. Forty-eight targets are libtest
harnesses; the unchanged harnessless CSV benchmark is accounted separately.
The full runs are pre-freeze prerequisites, followed by fifteen source gates
and ten auxiliary checks. All seventeen source gates therefore pass.

Frozen/restored returning, pristine, lower-cleanup and transport suites pass
15/37/7/9. Sixteen compiled negatives reject at their exact behavioral
assertions; all 5,685 source identities are restored after each. Nine runner,
30 freeze and 87 qualification-contract tests pass. The closed collector and
both independent archive reviews pass. Preliminary failures and helper history
remain retained. An earlier GNU pass belongs to its original boot and is not a
current-boot prerequisite; no cross-boot monotonic ordering is claimed.

Source restoration and child/process-group closure are independent acceptance
conditions. This evidence qualifies the named local scripted boundary only,
not the excluded persistent, live, native, formal or performance paths.

## Following Work

The [detached persistent-control handoff](runtime-detached-persistent-control-cleanup-v1.md)
is the next narrow packet. Its valid state has
no data authorities but retained premises matching binding count, so this
packet's equal-cardinality preflight cannot be reused unchanged. Persistent
returned-data bridges separately preserve their public data-on-error behavior.

N4-L must keep the root outside the actual live model loan and retain Complete
through retake and metadata commit. N4-QP returning destruction must also retain
returned data through later completion-signal disposal and callbacks. Ordinary
mixed-owner release additionally depends on accepted typed data cleanup. None
of those compositions follows from the lower consuming wrapper alone.
