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
in-process copy, explicit-progress, and cancellation/drain extensions that
leave the frozen Runtime Worker V1 wire contract unchanged. Negotiated Runtime
Worker V4 carries one exact extension profile: execution-capability discovery,
flush, same-device asynchronous copy, cancellation, and deadline-bounded drain.
Runtime Worker V5 is additive and carries typed atomic and collective semantic
submission while retaining every V4 operation unchanged.
This transport versioning is separate from compiler/proof Worker V3.
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

`RuntimeWorkerTransportV1` is the subprocess owner shared by all transport
versions. Native GPU backends requiring the V4 operational SPIs should use
`RuntimeWorkerBackendV4<RuntimeBinaryCodecV4>` with a child served by
`serve_runtime_backend_worker_v4`; V4 requires its exact handshake before any
request. Its canonical server requires the backend to implement flush,
same-device async copy, and cancellation/drain. Typed semantic process
transport uses `RuntimeWorkerBackendV5<RuntimeBinaryCodecV5>` with a child
served by `serve_runtime_backend_worker_v5`; its server additionally requires
atomic and collective submission SPIs. Direct and multi-device KFD owners
satisfy that V5 type bound with contract-carrying semantic execution. The
copy-only native-XGMI owner retains V5 hosting through explicit pre-custody
rejections from both semantic SPIs; it does not advertise either execution
capability.
Execution capabilities are
cached only for the latest successfully enumerated device roster and otherwise
fail closed. The frozen `RuntimeWorkerBackendV1<RuntimeBinaryCodecV1>` remains
for backends that explicitly opt into the immediate-progress marker and rejects
newer peers. Every version verifies protocol compatibility with an exact
handshake, uses bounded request and response frames, enforces response
deadlines, and terminates a worker that becomes unresponsive or violates the
protocol. Adding `semantic-launch-v1` introduced V5 rather than mutating V4:
Runtime Worker V1 and V4 bytes and codec source contracts remain unchanged, as
does backend behavior for valid canonical requests. V4 rejects V5, and V5
rejects V4 as a downgrade rather than accepting an ambiguous capability set.
The parent caps each worker-backed completion wait at the caller's monotonic
deadline and sends only a relative child duration, so no cross-process clock
epoch is assumed. An already-expired wait returns `Pending` without publishing
a request. The handshake does not authenticate or attest the worker executable,
loaded module, or host. Callers must select a trusted child and provide any
required artifact authority, sandbox, or operating-system isolation. Native
KFD code, or deprecated HSA qualification code, may preserve its fail-closed
abort policy inside that child; the
application receives terminal backend loss without being terminated itself.

The repository provides both bounded canonical codecs and server loops, but does
not yet ship a standalone KFD worker executable. Shut down the context first,
then shut down the returned worker backend so the transport can send its
empty-frame termination and reap the child. V4 flush and ordinary extension
calls may synchronously block up to the configured request timeout; drain obeys
the caller's earlier deadline. Timeout, malformed terminal response, or terminal
backend failure seals and reaps the worker.

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

`RuntimeAsyncEngineV1` adds executor-neutral background completion observation
for any `Send` backend context. One thread-affine engine owner retains the
context; cloneable handles can cross threads to enqueue bounded context calls or
obtain standard `Future` values for exact event identities. Commands and event
polls have independent per-tick budgets, events are visited in stable cyclic
identity order, executor-waker and context-command panics are contained, and
worker-thread reentry is rejected. Dropping a future abandons only observation
and never cancels or releases runtime work. Consuming shutdown wakes retained
futures as stopped and returns the context. This engine observes completion; it
does not publish deferred backend work, replace explicit `flush_stream`, or
provide native asynchronous execution by itself.

The current single-device KFD adapter admits one gfx942 device and at most
65,536 logical streams, multiplexed over exactly two persistent native compute
lanes. Logical stream creation does not lease a lane. Accepted compute launches
own their kernarg, bindings, dependencies, and retained resources in bounded
per-stream FIFOs until the lowest available lane can publish the FIFO head.
At most one dispatch occupies each lane, and concurrent native work must use
disjoint allocations. This is bounded two-lane concurrency, not arbitrary
same-device compute concurrency.

Compute and same-device copy operations on one logical stream gain an implicit
tail dependency. Cross-stream overlapping allocation use requires an explicit
event dependency, and dependency count and transitive unpublished depth are
capped at 256. Prepublication cancellation removes owned work and restores the
prior stream tail; cancellation after a doorbell is explicitly `TooLate`.
`poll` observes state without preparing deferred compute. Submit may publish
immediately ready work. `wait` is likewise observation-only and does not publish
deferred work. The additive in-process `flush_stream` operation may drive a
dependency-ready FIFO head through potentially blocking dirty-buffer
reconciliation and native publication. There is no native queue-side dependency
packet. The optional async engine remains observation-only under its original
`spawn` constructor. Its additive `spawn_with_progress` mode can register a
bounded set of streams and call `flush_stream` for dependency-ready pending
heads on the owner thread, using an independently bounded cyclic flush budget.
Registration is move-only and dropping it stops future flush attempts without
cancelling, releasing, or finally flushing work. Retryable rejected/quiescent
failures remain registered and observable; terminal ambiguity seals the engine.
This opt-in host scheduler is cooperative progress, not proof of native
liveness, fairness, or hardware execution. Runtime Worker V1 has no flush
request; negotiated Runtime Worker V4 and V5 expose the same bounded progress
operation. Direct KFD owners remain thread-affine and cannot use the cross-thread
engine; the Send-capable Worker V4/V5 adapters can.

Live child-process regression tests exercise this progress path through exact
V4 ordinary and V5 atomic wire requests: an event must first report pending,
then the background engine emits the canonical flush request, and only then may
the child report completion. Separate cases verify response-deadline,
decoded-terminal, and EOF sealing. These tests validate host transport and
runtime state propagation only; they are not native KFD or liveness evidence.

Same-device `copy_async` uses the native directional SDMA queues and splits
logical ranges larger than one linear packet into sequential packets. Live
logical allocations retain native host or HBM SDMA storage, while bounded host
images remain the current compute authority. Persistent SDMA allocations and
transient staging use the queue-owned best-fit memory pool; device-local buffers
are initialized before publication and scrubbed before recycle, and explicit
shutdown trims the pool. Persistent SDMA buffers are not yet shared with
fixed-dispatch compute storage, so device-local compute input remains
materialized per launch and read-only.

Logical KFD allocations are capped at 256 MiB each and 1 GiB per backend
context; budget and allocator exhaustion return `Capacity` before native
publication. `KfdMultiDeviceRuntimeBackendV1` admits every selected physical
device before any queue exists and routes independent child backends. A live
same-device copy uses that child's native SDMA path. Peer copy in this generic
router remains bounded host staging: poll and wait are observation-only, while
`flush_stream` drives retained child range operations in 64 KiB chunks to a
conclusive state. Pending staging is capped at 1 GiB. Flush is potentially
blocking cooperative host progress, not background DMA or native XGMI.

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
one empty slot. Polling or waiting for work beyond the current batch may observe
a published predecessor, providing bounded caller-driven completion observation
without claiming background progress. `flush_stream` snapshots the
dependency-ready directional set at entry and publishes it in FIFO prefixes of
at most 63. When more than one prefix is needed, flush synchronously drains each
earlier prefix before publishing the next; the final prefix remains outstanding
so host work after the flush can overlap DMA. A first-prefix allocation or
admission failure rejects before native mutation. A recoverable later-prefix
failure is quiescent because every earlier published prefix has completed, and
the remaining ready custody can be retried. Poll and wait observe
already-published completion and terminal dependencies but never publish
deferred copies. Flush creates no background thread. It exposes no
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
correlated gfx942 or gfx950 device and host-visible memory. Direct KFD compute
concurrency is limited to two disjoint-allocation lanes per device, and native
peer copy is available only through the separate copy-only XGMI owner. There is
no unified native multi-device compute owner. A separate
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
capabilities and an additive contract-preserving backend SPI. Direct and
multi-device KFD carry that exact contract through pending custody,
recycled-dispatch identity, and final invocation authorization. Ordinary and
qualification constructors still advertise both capability layers as false.
Separate semantic-authority constructors advertise only non-System atomic and
workgroup-collective profiles enumerated by an unsafe authority; unlisted
profiles and invalid contracts reject before scheduler custody. The authority
must independently bind authenticated source-to-machine and native semantic
evidence. A later final invocation-authority denial occurs during preparation;
the accepted unpublished submission is settled as failed and its scheduler
custody is released before native publication. Authority panics are contained
as fail-closed denials under the same prepublication settlement. The runtime
does not provide a concrete production authority, synthesize instructions, or
claim formal compiler, firmware, or hardware refinement. Worker V5 preserves a
validated semantic contract across the process boundary, but does not create
native semantic support or authority when the hosted backend lacks it. This is
not HIP/HSA parity. See
[`docs/runtime-community-architecture-v1.md`](../../docs/runtime-community-architecture-v1.md).

The additive R11 executable model covers the shared completion/event state,
exact-once callback discharge, atomic and compare-exchange order/weak contract
matching, collective geometry/membership gating, and persistent-batch mapping
custody. R12 adds 23 obligations and 13 expected-negative mutations for abstract
multi-queue custody, bringing the cumulative totals to 142 and 92. R13 adds 20
obligations and 11 mutations for the bounded logical-stream scheduler, bringing
the totals to 162 and 103. R14 adds 10 obligations and 8 mutations for bounded
event observation, bringing the totals to 172 and 111. R16 adds 21 obligations
and 10 mutations for a reachable, already-decoded Worker V5 semantic
request/response boundary, exact attempted/accepted/indeterminate custody, and
an ordered exhaustive sidecar sequence join, bringing the totals to 193 and
121. It does not parse bytes, invoke a subprocess, authenticate a worker, or
count concrete backend calls. These pinned Verus obligations prove only the
corresponding abstract models; they are not a Rust-to-Verus refinement or native
execution proof.

R17 adds 32 obligations and 14 mutations for a bounded persistent-allocation
summary, bringing the totals to 225 and 135. It covers canonical R2-shaped
admission, exact home-VM/queue and R9-shaped route predicates, reusable slot
generations, hazards, dependency summaries, timeout custody, and quarantine.
The independent executable model uses a private `Rc` registry incarnation to
reject reconstructed-registry transition-token and dependency substitution;
numeric observation identities are non-authoritative. The route predicate is
metadata only and is not bound to the persistent mapping. Neither model is a
refinement of the concrete KFD persistent owner, which remains disconnected
from native compute/SDMA/XGMI publication and completion.

R18 connects that owner to one targeted low-level KFD SDMA queue. It preserves
the queue's existing device-buffer accounting and retains exact move-only
allocation, host-buffer, range-use, and native-ticket custody through confirmed
publication, poll, bounded wait, completion, and settlement. Recoverable
prepublication failure restores the original owners; retained or later
uncertainty becomes opaque process-teardown custody. The adapter is
single-flight and is not wired into `RuntimeContextV1`, `RuntimeAsyncEngineV1`,
or Worker V4/V5. Exact quiescent-frontier retirement reclaims settled ledger
history and returns stale or substituted allocation/frontier custody unchanged;
native-neutral tests cover 66 sequential transition cycles. Its independent
executable and Verus models bring the pinned
abstract totals to 259 obligations and 159 rejected mutations, without a
Rust-to-Verus or native refinement claim. No hardware or performance evidence
is attached to this tranche.

R19 adds a versioned directional local-SDMA adapter without weakening R18. One
promoted device allocation is bound to the exact parent queue occurrence and
to distinct engine-1 H2D and engine-0 D2H child queues. It admits pooled backing
with `0 < logical <= physical <= 256 MiB`, keeps copy ranges within the logical
extent, and preserves the inherited outstanding-buffer debit. Each exact
frontier must be retired before the next use; after retirement the next use may
repeat or change direction. Promotion, demotion, submission, polling, waiting,
completion, and failure paths retain either retryable owners or explicit opaque
process-teardown custody. Active packet checks use the bounded operational
currentness fence rather than rediscovering topology through sysfs.

The independent R19 executable model and Verus summary add 46 obligations and
20 expected-negative mutations, bringing the pinned abstract totals to 305 and
179. They cover the bounded directional state machine but do not prove a
Rust-to-Verus or native refinement. R19 is not wired into
`RuntimeContextV1`, `RuntimeAsyncEngineV1`, or Worker V4/V5. Its native packet
limit remains `0x003f_ffe0` bytes and its allocation ledger is single-flight,
so large transfers still require repeated completion and retirement cycles.
There is no D2D or H2H path and no hardware or comparative HIP/HSA performance
evidence in this tranche.

R20 wires the R19 directional owner into `KfdRuntimeBackendV1` without changing
the runtime or Worker wire protocols. Native allocation records distinguish
host, persistent device, demoted cleanup-only, in-flight, and opaque terminal
custody. Direct `copy_async` admits only H2D and D2H; H2H and D2D reject before
handle or retain mutation. Every packet is bounded by the R19 cap, and exact
completion, settlement, and frontier retirement restore both owners before the
next packet. Poll and wait remain observation-only. `flush_stream` owns every
continuation publication, and cancellation succeeds only before publication
and before any completed byte.

Zero-progress retryable publication settles as a conclusive failed submission.
A retryable continuation failure after partial device mutation instead releases
all native and scheduler retains and records an exact per-submission
`QuiescentWithoutResult` marker. Poll, wait, drain, event retention, stream
destruction, release, shutdown, and Worker V4/V5 transport preserve that
distinction. Synchronous upload, download, zeroing, shadow reconciliation, and
release scrub use packet-bounded transient staging rather than a second
allocation-sized buffer.

The independent R20 model has 14 focused executable tests. Its pinned Verus
summary adds 31 obligations and 15 expected-negative mutations, bringing the
authenticated totals to 336 and 194. The proof covers the abstract facade state
machine only. The direct backend remains thread-affine and cannot use
`RuntimeAsyncEngineV1` background progress directly; Send-capable Worker V4/V5
adapters can. R20 has no facade-level scripted move-only failure driver, H2H or
D2D path, batched large-copy publication, native hardware result, Rust-to-Verus
refinement theorem, or comparative HIP/HSA performance evidence.

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
Atomic and collective contracts affect the opaque dispatch-shape identity but
are not yet exposed as typed profiler fields, so profiler/query consumers cannot
independently report their operation, scope, order, or participants.

The profiler also records process-local monotonic points immediately after an
AQL publication is accepted and when runtime completion processing finishes.
These points are committed only when the exact profile event is retained and
are returned through an opaque runtime-owned custody bundle. Each recorder owns
a fresh `getrandom` occurrence bound into its clock-domain identity, making
accidental aliasing across reused caller capture scopes and process-local
`Instant` epochs cryptographically negligible. They delimit host observations,
not GPU execution: packet start/end, a device clock domain, and global clock
synchronization remain unavailable.

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

The safe production API now has one consuming execution transition and one
private implementation of the unsafe `WorkerV3Gfx942ExecutionAuthorityV1`
boundary. That implementation remains unreachable without admission of a
move-only `WorkerV3SemanticMachineRefinementReceiptV1`. The inspect-only receipt
has private state but no production constructor or producer wiring. It binds one
exact executable publication occurrence across the KIR, final LLVM, selected ISA
range, machine-effect evidence, refinement proof, final artifact,
compiler-currentness and rollback chain, Worker challenge and lineage, and
durable publication. Its machine-effect contract is universal over checked
invocations, not bound to one dispatch geometry. The deprecated HSA lifecycle
retains receipt custody with a loaded executable across repeated checked
invocations; direct KFD consumes it into a one-shot application binding.
Existing protected and synthetic adapters produce no such receipt.
The transition independently
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
an admitted semantic-to-machine refinement receipt, runtime preparation, and one
checked KFD device into a move-only invocation.
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
