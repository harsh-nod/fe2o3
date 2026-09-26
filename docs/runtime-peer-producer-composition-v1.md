# Peer Producers For Typed Compute

`launch_producer_aware_v1` accepts scalar peer-copy events after Context has
reconciled those copies as successful and quiescent. It still requires a version
journal and the explicit `RuntimeProducerAwareLaunchBackendV1` contract.
Physical completion without logical settlement is insufficient.

## Current Support

Context accepts completed ordinary and directed scalar peer copies on the
consumer's destination device. It authenticates the retained copy identity,
stream, backend submission and original destination; aliases naming one
producer twice reject. Directed roots preserve their original graph depth after
settlement. An ordinary completed copy is a terminal leaf. A producer's source
allocation and earlier ancestors may already have been released; its retained
historical root remains the authority for this terminal dependency.

`KfdMultiDeviceRuntimeBackendV1` supports completed cooperative copies as exact
typed-launch producers. It requires an exact event/submission pair and checks
the actual copy destination, not only the event's device label. Native parents
continue to use child-local dependencies. Completed cooperative parents remain
router-owned; no native event or GPU-completion record is fabricated for them.

The router reserves producer custody before child submission. Dropping the
public event cannot release that custody. Rejected/quiescent admission failures
refund it under the existing child submission contract; terminal failure and
unwind retain it. Poll, wait, drain, cancellation, flush and explicit release
retire it only after the applicable exact consumer quiescence. A quiescent error
alone is not used to infer that an unrelated pending consumer has completed.

Retention counts support fan-out and checked overflow. Storage is bounded by
the existing submission and dependency limits. Native-only dependency rosters
do not allocate a peer-producer vector. Flush scans the retained peer-consumer
roster; no runtime performance improvement is claimed for this checkpoint.

## Not Yet Supported

Pending peer-copy-to-compute admission remains rejected. The ordinary
multi-device router uses host-staged cooperative peer copies, not native XGMI.
The separate native-XGMI copy backend has not acquired compute support. This
change adds no Worker protocol, atomic/collective authority, generated-launch
authority or compatibility runtime.

The shared completion-planner body is unchanged. Its existing finite projection
requires same-kind edges and does not establish coverage of these newly admitted
mixed-kind graphs or the complete Context adapters. CPU validation is not a
formal-refinement, native-correctness or performance result.

## Pending Composition Work

1. Add an explicit directed-copy contract to the multi-device router, which
   currently exposes only ordinary cooperative copies. Finish resumable
   native-dirty preparation on top of the authoritative-backing child SDMA leaves
   described below. Then admit directed peer parents with their existing
   success-gated state, preserving one graph-wide depth bound. Ordinary pending
   peers need an explicit compatible completion contract before admission, not
   a relaxed flag check.
2. Check each original Read binding against the exact peer destination interval
   and captured allocation record. Whole-allocation journal leases are not proof
   that a smaller producer wrote every consumer input.
3. Reuse the child compute ledger's module, kernarg, allocation, stream and
   cancellation ownership. A private producer bridge is insufficient by itself:
   retaining the consumer's allocation currently blocks the cooperative copy's
   public write path. Any internal copy-access exception must authenticate that
   all conflicting consumers are unpublished and blocked on that exact producer.
   Public host access must remain rejected.
4. Gate immediate, observed, ordered-successor, deadline and flush publication
   paths. Preserve transitive stream ordering, producer fan-out, cross-stream
   flush and drain progress, initialization authority and final publication-time
   checks. Failure, cancellation and unknown results must not publish compute.
   Poll/wait must not silently acquire cooperative-copy progress semantics.
5. Extend and requalify the mixed-kind finite projection, terminal-leaf model and
   production adapters. Then qualify actual copy/compute execution, native XGMI
   composition and matched HIP/HSA workloads separately.

## Nonblocking Transport Prerequisite

Cooperative Read/Write phases now use the child's asynchronous copy ledger when
DeviceLocal DMA backing is authoritative. Each phase retains a private
HostVisible scratch allocation, logical stream and exact child submission.
Scratch is at most 64 KiB and is reused across chunks. All source chunks are
captured before any destination chunk is written. Public poll/wait remain
observational; flush/drain drive publication, observation, readback and disposal.
A clean private Ready head uses exact child progress, not a blocking stream flush.
One flush can now return while a DeviceLocal leaf is Pending; callers must use
drain or further explicit flushes to advance subsequent chunks and phases.

The router reserves one scratch window in addition to its full-copy Vec budget.
Composed request mode charges the exact selected child account before backing
effects. Scratch handles never enter the router's public handle tables. Ordered
successors remain admissible while a predecessor's private DMA is live; every
conflicting native custody owner must match that exact allowed predecessor.
Published DMA cannot be cancelled or have its allocations released.

Recoverable scratch disposal failure conclusively fails the copy and the selected
dependent path. Only quiescent private cleanup custody and its charge remain;
submission release retries disposal, never resumes destination writes. Terminal
failure or unwind retains all potentially live custody and seals the router.
This is development implementation, not native or formal qualification of the
new cooperative adapter or positive native composed-account execution.

Two dirty states have different authority. `sdma_shadow_dirty` means persistent
DMA backing is authoritative and its CPU shadow may be stale. It no longer
blocks initial async H2D/D2H publication when dependencies have already succeeded.
The existing H2D compute-ready promotion still rejects dirty source shadows.
`native_dirty` means separately materialized compute data must be reconciled;
it still prevents immediate publication. That reconciliation needs bounded
recycled-data reads and asynchronous uploads retaining exact extent/generation
authority until success. That path still falls back to public child host-transfer
APIs, which can wait synchronously for directional SDMA for up to 30 seconds.
Resumable native-dirty preparation remains open; wrapping the current router in
the directed SPI would still violate its no-wait contract. No broad directed
router support, pending peer-to-compute admission or measured latency improvement
is established here.

Resumable reconciliation must capture the recycled dispatch generation once and
pin its lane against replay, rebind and detach until settlement. The current
synchronous path fetches the lane's current generation on entry; doing that on
every resume could read a different dispatch. Reconciliation uploads need a
private, exact-root SDMA purpose authenticating scratch, destination interval,
descriptor and captured generation. Clearing dirty extents early or adding a
general `skip_dirty` flag is not an acceptable substitute. Retire each upload
before advancing its cursor, and remove only the fully reconciled extent.
