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

No production implementation of the unsafe authority trait exists yet. Only a
reviewed Worker V3 verifier joined to compiler-generated invocation preparation
may implement it. Successful parsing, materialization, request construction, or
descriptive digest equality is not verifier or launch authority.

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
