# fe2o3-runtime

`fe2o3-runtime` is the sole public composition boundary for fe2o3 runtime
backends. `RuntimeContextV1` owns one backend instance and multiplexes its
devices, streams, allocations, modules, typed kernels, events, asynchronous
submissions, and peer copies through context-local stable handles. Backend
capabilities are explicit, so unsupported operations reject before native
mutation rather than being inferred from the build host.

`RuntimeBackendV1` is the backend SPI. It carries only numeric sealed handles,
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

Observe each submission to `Succeeded` or `Failed`, release events retaining
that submission, consume `release_submission`, destroy streams, and call
`RuntimeContextV1::shutdown`. Cleanup failures retain their handles for retry.
Direct KFD backend drop may abort when live or ambiguous native custody remains,
so explicit shutdown is required for predictable teardown. The deprecated HSA
qualification adapter retains the same conservative cleanup rule.

The current KFD adapter admits one gfx942 device and serializes logical streams
over one native queue. Same-shape launches retain native host-visible storage,
code, kernarg, and dispatch state across completion generations. Logical host
images use shared immutable snapshots; exact repeated full writes reuse their
validated digest and update attached coherent storage before launch. GPU-written
extents stay native-authoritative until `read_allocation` or a later launch
requires the host image, and facade reads can copy directly into the caller's
destination. Device-local input is still host-staged, materialized per launch,
and read-only. KFD module validation is cached at load and kernel metadata is
cached at resolution. Logical KFD allocations are capped at 256 MiB each and 1
GiB per backend context; budget and allocator exhaustion return `Capacity`
before native publication. The deprecated HSA qualification adapter admits one
correlated gfx942 or gfx950 device and host-visible memory when its explicit
legacy features are enabled. Neither adapter currently advertises peer copy,
multi-device operation, atomics, or collectives; atomics and collectives have no
general V1 facade operation. This is not HIP/HSA parity. See
[`docs/runtime-community-architecture-v1.md`](../../docs/runtime-community-architecture-v1.md).

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
