# fe2o3-runtime

`fe2o3-runtime` is the sole safe composition boundary for the pure-Rust
`gfx942:xnack-` runtime. Its first implemented layer joins the bounded AMDHSA
COV6 loader, selected descriptor and resource facts, complete implicit kernarg
initialization, and the address-sealed KFD request.

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

The production dependency closure contains no HIP, ROCr/HSA runtime, COMGR,
`libdrm`, native shim, or runtime dynamic loader.
