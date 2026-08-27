# fe2o3-runtime

`fe2o3-runtime` is the sole safe composition boundary for the pure-Rust
`gfx942:xnack-` runtime. Its first implemented layer joins the bounded AMDHSA
COV6 loader, selected descriptor and resource facts, complete implicit kernarg
initialization, and the address-sealed KFD request.

The safe production API deliberately stops before execution authority. It
returns a structurally complete request whose pure-Rust KFD entry point remains
`unsafe`; only an admitting Worker V3 decision may close that boundary.
Successful parsing, materialization, request construction, or diagnostic
execution is not verifier or launch authority.

The opt-in `gfx942-lds-diagnostic` executes one SHA-pinned, loader-inspected LDS
reduction through the private KFD mechanics. On the qualifying MI300X it
completed with result `2080` and preserved all canaries. That measured lane
guards CPU/GPU identity mapping, complete static-plus-dynamic AQL group-segment
allocation, packet publication, completion, and teardown. It deliberately
bypasses the absent Worker V3 authority under an explicit `unsafe` call and is
therefore neither a safe application path nor parity evidence.

The production dependency closure contains no HIP, ROCr/HSA runtime, COMGR,
`libdrm`, native shim, or runtime dynamic loader.
