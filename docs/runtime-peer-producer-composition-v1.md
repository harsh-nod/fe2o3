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
   currently exposes only ordinary cooperative copies. First replace its blocking
   DeviceLocal host-transfer leaves with resumable child SDMA operations as
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

The existing cooperative Read/Write phases call public child host-transfer APIs.
Those can execute synchronous directional SDMA with a 30-second wait. A wrapper
around that state machine would violate the directed SPI's no-wait contract,
even with bounded dependency selection and 64-KiB range requests.

The child asynchronous copy ledger already owns submission, observation,
retirement and failure custody. Reuse those real submissions with retained,
accounted private host staging and child streams. Outer D2H/H2D phases must
retain the exact child operation through completion and cancellation; dropping
a public event must not release either owner. Publication and observation must
remain separate bounded actions, without calling the blocking child flush path.

Two dirty states have different authority. `sdma_shadow_dirty` means persistent
DMA backing is authoritative and its CPU shadow may be stale. It no longer
blocks initial async H2D/D2H publication when dependencies have already succeeded.
The existing H2D compute-ready promotion still rejects dirty source shadows.
`native_dirty` means separately materialized compute data must be reconciled;
it still prevents immediate publication. That reconciliation needs bounded
recycled-data reads and asynchronous uploads retaining exact extent/generation
authority until success. This preparation and router integration remain open;
no broad directed router support or latency improvement is established here.
