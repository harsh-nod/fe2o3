# fe2o3-runtime

`fe2o3-runtime` is the sole public composition boundary for fe2o3 runtime
backends. `RuntimeContextV1` owns one backend instance and multiplexes its
devices, streams, allocations, modules, typed kernels, events, asynchronous
submissions, optional same-device copies, and peer copies through context-local
stable handles. Submissions and events share a typed completion state, and the
context provides exact-once completion callbacks plus aggregate stream query
and bounded synchronization. Backend capabilities are explicit, so unsupported
operations reject before native mutation rather than being inferred from the
build host.

`RuntimeBackendV1` is the base backend SPI. `RuntimeAsyncCopyBackendV1`,
`RuntimeFlushBackendV1`, and `RuntimeCancellationBackendV1` are additive
in-process copy, explicit-publication, and cancellation/drain extensions that
leave the Worker V3 wire contract unchanged.
All carry only numeric sealed handles,
address-free argument images, allocation-relative bindings, explicit event
dependencies, and monotonic deadlines. The direct-KFD and worker-backed
adapters implement that contract without exposing raw addresses or native
resource types. The deprecated HSA adapter exercises the same SPI only in
explicit qualification builds. Typed kernels pair the application signature
with the pure
`fe2o3-runtime-model` kernel identity; peer copies similarly retain a model
contract identity alongside the concrete backend submission.

The application-supplied typed signature is an identity association, not proof
that a Rust type matches a native kernarg ABI or that declared regions cover all
memory effects. Direct KFD execution requires an unsafe launch authority. The
deprecated HSA qualification adapter separately requires its caller to uphold
an unsafe artifact, ABI, and effect contract; it is not a production fallback.

`RuntimeWorkerTransportV1` is the preferred community-facing deployment for
native GPU backends. It verifies protocol compatibility with a fixed handshake,
uses bounded request and response frames, enforces response deadlines, and
terminates a worker that becomes unresponsive or violates the protocol. The
parent caps each worker-backed completion wait at the caller's monotonic
deadline and sends only a relative child duration, so no cross-process clock
epoch is assumed. An already-expired wait returns `Pending` without publishing
a request. The handshake does not authenticate or attest the worker executable,
loaded module, or host. Callers must select a trusted child and provide any
required artifact authority, sandbox, or operating-system isolation. Native
KFD code, or deprecated HSA qualification code, may preserve its fail-closed
abort policy inside that child; the
application receives terminal backend loss without being terminated itself.

The parent uses `RuntimeWorkerBackendV1<RuntimeBinaryCodecV1>` and the child
calls `serve_runtime_backend_worker_v1` with its concrete backend. The repository
provides the transport, canonical codec, and server loop, but does not yet ship
a standalone KFD worker executable. Shut down the context first, then
shut down the returned worker backend so the transport can send its empty-frame
termination and reap the child.

Observe each submission to a conclusive `RuntimeCompletionStatusV1`: `Succeeded`,
a typed backend-code or cancellation failure, or `QuiescentWithoutResult` when
native references are gone without an execution result. `query_event`,
`poll_event`, and `wait_event` alias the source submission's central status.
`on_completion` callbacks are removed before invocation and run exactly once on
the first conclusive transition across submission/event observation, cancel,
drain, cleanup, or release; callback panics are contained and counted. Terminal
backend ambiguity is not completion and does not discharge callbacks.
`query_stream` reports aggregate retained-submission counts, and
`synchronize_stream` waits under one shared deadline before returning the same
observation. A rejected or quiescent backend wait is returned only after later
pending submissions have received their one wait; terminal ambiguity stops
immediately. Release events retaining a submission, consume
`release_submission`, destroy streams, and call `RuntimeContextV1::shutdown`.
Cleanup failures retain their handles for retry.
Direct KFD backend drop may abort when live or ambiguous native custody remains,
so explicit shutdown is required for predictable teardown. The deprecated HSA
qualification adapter retains the same conservative cleanup rule.

The current single-device KFD adapter admits one gfx942 device and serializes
logical compute streams over one AQL queue. Live logical allocations retain
native host or HBM SDMA storage, while bounded host images remain the current
compute authority. Same-device `copy_async` uses the native directional SDMA
queues, splits logical ranges larger than one linear packet into sequential
packets, and retains explicit event dependencies until publication. Cancellation
before publication quiesces the submission; cancellation after a doorbell is
explicitly `TooLate`. One compute dispatch and SDMA work may overlap when every
referenced allocation is disjoint. An overlapping copy may remain unpublished
behind an explicit event for the active compute dispatch; a compute launch that
overlaps pending SDMA is rejected, as is a compute dependency on a pending copy.
Same-device concurrent compute remains unsupported. Persistent SDMA allocations
and transient staging use the queue-owned best-fit memory pool; device-local
buffers are initialized before publication and scrubbed before recycle, and
explicit shutdown trims the pool. Persistent SDMA buffers are not yet shared
with fixed-dispatch compute storage, so device-local compute input remains
materialized per launch and read-only.

Logical KFD allocations are capped at 256 MiB each and 1 GiB per backend
context; budget and allocator exhaustion return `Capacity` before native
publication. `KfdMultiDeviceRuntimeBackendV1` admits every selected physical
device before any queue exists and routes independent child backends. A live
same-device copy uses that child's native SDMA path. Peer copy in this generic
router remains bounded host staging: each `poll` performs at most one 64 KiB
child range request, pending staging is capped at 1 GiB, and fairness requires
the caller to poll or wait. It is not background DMA or native XGMI.

`KfdNativeXgmiRuntimeBackendV1` is the separate exact two-device, copy-only
native peer backend. It admits both gfx942 devices and both directional topology
routes before acquiring either VM, allocates PUBLIC device-local VRAM, maps both
allocations to the canonical GPU-ID pair, and selects the source device's
topology-admitted XGMI SDMA engine. A successful full mapping is retained for
reuse across copies until host access or allocation release requires an explicit
unmap; indeterminate mapping or completion remains quarantined. The destination
device owns the public stream. Every copy is limited to one admitted linear
packet and one copy per logical stream may be pending. Ready copies in one
direction are selected by a deterministic FIFO readiness queue and published in one
native reservation and doorbell store, capped at 63 so the 64-slot ring retains
one empty slot. Polling or waiting for work beyond the current batch advances a
published predecessor, providing bounded caller-driven fairness without
claiming background progress. After enqueueing a complete ready batch of at most
63 copies, `flush_stream` explicitly publishes it before returning, allowing
host work after the flush to overlap DMA. An oversized ready set rejects before
publication; poll or wait remains the bounded fallback. Flush does not progress
dependencies, observe completion, or create a background thread. It exposes no
compute, same-device copy, memory
pool, profiling, atomics, or collectives. Capability detail is
available through `RuntimeContextV1::execution_capabilities` so applications can
distinguish this native peer path from the router's host-staged peer copy.
The XGMI benchmark reports host-access `remap-per-round` and
`persistent-hot` rows separately; only the latter avoids host access between a
priming batch and timed repetitions and therefore measures mapping reuse. Its
reported `outstanding_depth` is queued work on one ordered SDMA engine, not an
engine-concurrency claim.

Applications must explicitly shut down either multi-device KFD owner after
releasing submissions, events, allocations, and streams. Ambiguous native
failure retains custody and latches terminal state. The HSA adapter admits one
correlated gfx942 or gfx950 device and host-visible memory. Per-device
concurrent KFD compute remains unsupported, and native peer copy is available
only through the separate copy-only XGMI owner. There is no unified native
multi-device compute owner. A separate
authority-free gfx942 model checks the reviewed integer-atomic and collective
semantic declarations against exact runtime resources. The facade's
`launch_atomic` and `launch_collective` wrappers match typed operation, scope,
success ordering, geometry, and collective membership contracts.
Compare-exchange also binds a required failure order and weak mode. Failure
ordering cannot be `Release` or `AcquireRelease` and cannot be stronger than
success; non-CAS operations require no failure order and `weak = false`. A
collective grid must contain at least one complete workgroup in every dimension
and divide exactly by that workgroup shape; partial tail workgroups reject
before submission.
Atomic launches retain base geometry validation and may use a partial final
workgroup. The wrappers require both stable and execution-detail backend
capabilities and then
use the ordinary admitted typed kernel launch path. They do not synthesize
native operations or grant artifact, Worker V3, or machine-semantic authority.
Current KFD backends advertise both
atomic and collective capability layers as false, so these calls reject before
native submission. There is still no authenticated code-object refinement.
This is not HIP/HSA parity. See
[`docs/runtime-community-architecture-v1.md`](../../docs/runtime-community-architecture-v1.md).

The additive R11 executable model covers the shared completion/event state,
exact-once callback discharge, atomic and compare-exchange order/weak contract
matching, collective geometry/membership gating, and persistent-batch mapping
custody. Its pinned Verus obligations prove the corresponding abstract model properties only; they
are not a Rust-to-Verus refinement or native execution proof.

The direct-KFD backend also exposes an opt-in bounded profiler. It records
address-free logical resource lifecycle, host staging read/write ranges, native
queue creation/teardown, successful AQL publication, completion, and
host-monotonic phase durations for every admitted kernel shape. Range-only host
content observation is the low-overhead default; callers can explicitly request
content identities when the additional hashing cost is appropriate. It reports
rocprof correlation, per-dispatch device timestamps, copy-engine events,
counters, PC samples, decoded ATT, and authenticated source/IR/ISA attribution
as unavailable rather than inferring them. The low-level KFD queue can sample
GPU, CPU, and system counters under currentness checks, but this is a
clock-domain calibration input only: it does not identify a dispatch
publication, start, or completion boundary. Collection begins before context
construction and finishes only after logical cleanup and native shutdown. See
[`docs/kfd-native-profiler-v1.md`](../../docs/kfd-native-profiler-v1.md).

Feature `hardware-qualification` exposes a repository-owned, SHA-pinned gfx942
vecadd fixture and an exact KFD qualification constructor. That constructor
retains a private gate which accepts only the fixture's fixed artifact, typed
ABI, metadata-declared read/read/write effects, deterministic allocation
contents, and launch geometry. It does not implement or weaken
`KfdRuntimeLaunchAuthorityV1`, so it cannot satisfy the production Worker V3
transition. Deprecated HSA and HIP qualification oracles exercise the same
HSACO and inputs through one clean-checkout MI300X runner; see
[`benchmarks/runtime_gfx942/README.md`](../../benchmarks/runtime_gfx942/README.md).

The bounded direct `gfx942:xnack-` layer remains available for the protected
Worker V3 path. It joins the bounded AMDHSA COV6 loader, selected descriptor and
resource facts, complete implicit kernarg initialization, and the
address-sealed KFD request.

The safe production API now has one consuming execution transition, but it is
unreachable without an implementation of the unsafe
`WorkerV3Gfx942ExecutionAuthorityV1` boundary. The transition independently
matches the exact finalized object and length, selected kernel, complete
address-free invocation contract, and checked KFD GPU unique ID. That invocation
identity binds the KFD mechanics manifest, materialized image, descriptor,
kernarg, buffer bytes and declared effects, pointer fixups, geometry, resource
sizes, and timeout. It revalidates retained authority before dispatch and after
confirmed completion, preserves each buffer's declared effect in the result,
and rejects any changed read-only buffer after teardown. Because every
low-level error after native mutation is terminal, the safe transition aborts
instead of returning into application code.

`execute_authorized_gfx942_runtime_debug_target_dispatch_v1` is the cooperative
debug-target form of that same generic prepared-kernel transition. It does not
create a second verifier or launch implementation. Before native mutation it
derives bounded code-object, kernel, dispatch, geometry, and allocation
declarations from the exact prepared request already checked against Worker V3
authority. Telemetry failure at that point returns without entering queue
mechanics. Once native work begins, either a KFD failure or telemetry failure
aborts rather than exposing ambiguous state. Completed buffer cardinality,
length, access, and read-only contents are rechecked before a safe result is
returned.

The current KFD dispatch primitive has no lifecycle callback, so successful
`submitted` and `completed` declarations are emitted retrospectively after the
native transaction returns. They remain target declarations. Queue and runtime
lifecycle facts shown by the debugger come independently from KFD observations;
neither source upgrades the code object to observed execution in V3.

The separate V2 native telemetry composition emits a declaration before queue
preparation and one target-side KFD publication observation immediately after
the release-header/doorbell publication point. The low-level transition returns
a linear terminal handle after confirmed completion and teardown. Only this
safe runtime boundary can emit `Completed`, after completed-buffer validation
and post-completion Worker V3 currentness both succeed. Either rejection emits
`Failed`; an inability to send that terminal aborts. V2 does not mutate the V1
wire or lifecycle and grants no queue or packet authority.

`fe2o3-host` now has one private implementation of the unsafe authority trait.
It is constructible only by consuming an authenticated Worker V3 executable,
compiler-generated host-memory arguments, retained current-publication custody,
runtime preparation, and one checked KFD device into a move-only invocation.
The joined scalar-GEMM lane passes on MI300X with an explicitly synthetic test
verifier. No reviewed production verifier exists yet. Successful parsing,
materialization, request construction, synthetic verification, or descriptive
digest equality is not production verifier authority.

The Rust device-language surface includes an authenticated bounded scalar
volatile-load/store bridge with explicit bounds and access checks. It is not
broad Rust/device-language support; general `std`, allocation, unwind, dynamic
dispatch, arbitrary inline assembly, and external calls remain outside the
admitted device subset.

The opt-in `gfx942-lds-diagnostic` executes one SHA-pinned, loader-inspected LDS
reduction through this same transition using an explicitly unsafe diagnostic
authority implementation. On the qualifying MI300X it completed with result
`2080` and preserved all canaries. That measured lane guards CPU/GPU identity
mapping, complete static-plus-dynamic AQL group-segment allocation, packet
publication, completion, and teardown. Its manually asserted authority bypasses
the absent production Worker V3 verifier and is therefore neither a safe
application path nor parity evidence.

The direct-KFD/Worker V3 production dependency closure contains no
HIP, ROCr/HSA runtime, COMGR, `libdrm`, native shim, or runtime dynamic loader.
The authenticated machine-structure receipt remains outside this host-runtime
crate; `fe2o3-runtime-machine-adapter` owns the integration join to
`fe2o3-kernel-analysis` and delegates only to this crate's existing authorized
dispatch transition.
