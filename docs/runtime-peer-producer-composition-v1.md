# Peer Producers For Typed Compute

`launch_producer_aware_v1` accepts pending directed peer-copy events, or ordinary
scalar peer-copy events after Context has reconciled those copies as successful
and quiescent. It requires a version journal and a backend implementing the
explicit `RuntimeProducerAwareLaunchBackendV1` contract. Context admission does
not supply missing backend execution support.

## Current Support

Context accepts completed ordinary and directed scalar peer copies on the
consumer's destination device. It authenticates the retained copy identity,
stream, backend submission and original destination; aliases naming one
producer twice reject. Directed roots preserve their original graph depth after
settlement. An ordinary completed copy is a terminal leaf. A producer's source
allocation and earlier ancestors may already have been released; its retained
historical root remains the authority for this terminal dependency.

Context also accepts pending directed peers and mixed native/peer producer
rosters. Every original Read alias must fit the exact peer destination interval
and captured allocation record; the whole-allocation journal lease does not
authorize reads outside that interval. Pending writable aliases reject. Journal
reservations bind the exact producer writer, including its generation through
slot reuse. Directed depth includes settled history and remains bounded by 256.
Ordinary pending peers still reject.

Physical consumer success is retained separately from logical success. The
shared bounded planner reconciles exact parents first; callbacks and writer
publication follow canonical Context order while backend request order remains
unchanged. A later parent failure/cancellation does not invalidate structural
custody. Discarded parent results propagate Unknown, not Success; contradictory
Pending/Failed observations after physical consumer success seal custody.

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

The child compute record now carries a private external-input gate before any
opportunistic publication. Completed peer producers enter through a successful
gate. Waiting gates retain the genuine pending compute record, allocation/module
retains, completion reservation and stream position while observing native
dependencies. Failed inputs still wait for both external and native stream
prefixes before unpublished settlement. Successful gates continue through the
existing backing and native-order checks; they do not supply initialized bytes.
Terminal observation or unwind reinstalls the pending owner and seals execution.

The scalar resolve/action and ownership/access bodies are shared with
`compute_peer_gate_v1.rs`.
Their proof boundary is exact gate identity, monotonic resolution, separate
input-success/order conditions, sticky failure and predecessor-access expiry.
Access is revoked after peer success plus completed external ordering, even if
native failure is subsequently known. Allowed access excludes native publication.
Concrete custody maps, router ancestry, permit construction, scratch ownership,
cancellation disposal and native publication refinement are outside
that proof. Scripted tests exercise retained ownership and three-binding
persistent continuation; an ordered-successor control reaches the native
publication attempt but does not execute a native queue.

Private directed-copy origins now capture the exact producer, child, endpoint,
leg and original interval. The bounded direct-parent permit roster is attached
to the real pending child compute record and searched by exact key. A copy can
access an endpoint only when every conflicting compute owner individually
authorizes it and remains unpublished behind its gate. Public host operations
and copies never receive this origin. Ordinary copies keep their existing path.

Copy authority is interval-limited; reconciliation authority separately covers
the captured native dirty extent. Each accepted DMA retains its exact stream,
scratch and chunk arguments, rechecked before publication along with clean
native endpoints. Host access validates mapped or explicitly synthetic backing.
Reconciliation retains scratch size/backing, checks exclusive clean scratch
before every native read, and preserves exact descriptor/generation authority.
Common leaf entry authenticates the retained origin for progress and disposal.
Consumer cancellation removes its access exception without cancelling live
producer DMA or releasing that DMA's custody.

CPU tests exercise genuine accepted child consumers with multipart directed
DMA, both host reconciliation endpoints, synthetic host/device storage, public
access exclusion, consumer cancellation with live DMA and corruption before
effects. Publication-corruption controls invoke the publication boundary
directly with an already accepted record; they are not a hardware retry result.
These tests do not admit pending producers through the public router API or
execute the consumer kernel.

## Not Yet Supported

Pending peer-copy-to-compute admission remains rejected by the KFD router. Its
child compute ledger retains bound allocations immediately. Private direct-root
copy access is implemented, but bounded transitive/FIFO ancestry construction,
router-driven gate resolution and full lifecycle integration remain required
before opening pending admission. The ordinary
multi-device router uses host-staged cooperative peer copies, not native XGMI.
The separate native-XGMI copy backend has not acquired compute support. This
change adds no Worker protocol, atomic/collective authority, generated-launch
authority or compatibility runtime.

The shared completion-planner body is unchanged. Its finite projection now has
explicit directed-peer and launch dependency profiles and a witnessed exact
mixed-parent observation outcome; it does not
establish the complete Context adapters. CPU validation is not a formal-refinement,
native-correctness or performance result.

## Pending Composition Work

1. Build native pending peer-to-compute admission on the now-implemented
   [directed router profile](runtime-directed-cooperative-peer-v1.md) and resumable
   native-dirty preparation described below. Context now admits directed parents
   with their existing success-gated state and one graph-wide depth bound. Ordinary pending
   peers need an explicit compatible completion contract before admission, not
   a relaxed flag check.
2. Preserve Context's original-Read range checks in native admission. The
   completed CPU contract checks exact intervals, mixed leases, all completion
   ingresses, failure, cancellation, corruption, slot reuse and the depth limit.
3. Reuse the child compute ledger's module, kernarg, allocation, stream and
   cancellation ownership. The implemented private direct-root access bypasses
   consumer custody only for individually authorized unpublished owners;
   the cooperative copy's public write path remains blocked. Extend this to a bounded authenticated
   predecessor roster for each bound allocation, including transitive source
   readers and FIFO predecessors; allocation overlap or an ID alone is not proof
   of ancestry. Keep destination-write range authority separate from native
   reconciliation authority for an existing dirty extent.
4. Integrate the implemented child gate across the router's pending-peer
   lifecycle. Immediate, observed, ordered-successor, deadline and flush child
   publication paths now check it. Preserve transitive stream ordering, producer fan-out, cross-stream
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
`native_dirty` means separately materialized compute data must be reconciled.
The native recycled read-into API accepts HostVisible authority only. Materialized
launch admission rejects writable DeviceLocal bindings; persistent DeviceLocal
compute instead dirties its SDMA shadow. Earlier descriptions of this work as
DeviceLocal native-dirty asynchronous upload were incorrect.

Cooperative HostVisible reconciliation now captures the exact logical/native
lane, returned generation, descriptor and dirty extent in a child-owned root.
Each progress step reads and writes at most one private scratch window. Mapped
HostVisible backing receives only that exact range, without GPU submit/wait.
Lane/allocation pins survive across chunks; conflicting admitted compute remains
Pending, not Failed. Only a fully reconciled extent is removed and counted down.
Cancellation preserves an incomplete extent for retry. Changed generation or
descriptor, mapped-write failure and unwind seal the router with authority and
scratch retained.

HostVisible destination writes also update only their exact persistent range,
without detaching compute caches or refreshing the whole CPU shadow. Hash/write
provenance is invalidated; ordinary launch preparation must refresh the shadow
and overwrite stale native data before reuse. Active recipe reuse is excluded
by allocation custody. The old synchronous reconciliation helper now also
uploads exact extents, preventing stale bytes outside them from overwriting
previously reconciled data; it preserves the incoming shadow-dirty flag.

Scratch is reserved for every native endpoint at admission, including a clean
HostVisible allocation that a retained producer can dirty before Read starts.
The CPU fixture exercises generation/descriptor mismatch, multi-chunk writes,
cancellation followed by retry, exact destination sentinels, later mapped-write
fault/panic and compute Pending guards. It does not manufacture DeviceLocal
recycled readback authority. Native execution and positive composed-account
qualification remain required.

This removes the identified cooperative mapped-host reconciliation fallback.
The subsequent [directed profile](runtime-directed-cooperative-peer-v1.md)
adds the router SPI and retains exact ordered provenance through settlement.
Allocation, copy-on-write and driver calls do not establish a hard latency
bound. Pending peer-to-compute
admission, mixed-kind formal refinement and matched hardware measurements
remain open.
