# fe2o3-bf16-gemm-v1

This crate owns one exact direct-global `gfx942:xnack-` GEMM compiler profile.
One wave64 workgroup computes a row-major `16x16x16` product from BF16 bit
carriers `A[16,16]` and `B[16,16]` into FP32 `C[16,16]`. Each lane loads four
contiguous A elements and four strided B elements, invokes exactly one
`llvm.amdgcn.mfma.f32.16x16x16bf16.1k` with a zero FP32 accumulator, and stores
four disjoint FP32 outputs. The closed explicit ABI is three 64-bit global
pointers followed by the 256-byte COV6 hidden block.

The four source/stage identities are caller-supplied inert provenance labels.
They must be nonzero and pairwise distinct, but neither those checks nor their
digests authenticate an upstream source, compiler, or producer.

The numerical launch profile is fixed to one `[64,1,1]` workgroup and an AQL
total-workitem grid of `[64,1,1]`. The reviewed HSA adapter instead accepts
block counts `[1,1,1]` and multiplies them by that workgroup; the public shape
checks this expansion explicitly. Its inert
buffer validator requires nonzero addresses, exact A/B/C byte spans, respective
8/2/4-byte alignment, and pairwise-disjoint half-open ranges. These are checked
integer observations, not KFD mappings or leases, and cannot grant load or
launch authority.

The typed graph is admitted against the pinned upstream LLVM/LLD 22.1.8 policy,
serialized canonically, placed in the compiler-FFI handoff, and rechecked while
constructing a sealed Worker V2 request. Post-worker admission consumes the
prepared graph, measured worker, and reproducible-first-build evidence, then
rechecks the complete request/response/module/plan/provenance lineage and the
exact kernel ABI/resource profile before applying the strict COV6 loader
envelope.

This slice supplies an actual BF16/FP32 compiler input path. It does not prove
source-to-LLVM or LLVM-to-machine refinement, MFMA numerical semantics,
memory safety, or hardware execution. It does not independently approve the
newly observed HSACO digest and grants no publication, KFD allocation, kernarg
packing, AQL submission, completion, load, or launch authority. The ignored
HSA harness must run on the exact device/toolchain before any hardware claim.
This fixed one-tile slice is not the parameterized GEMM/GEMV K1 closure and
does not close M1 by itself.
