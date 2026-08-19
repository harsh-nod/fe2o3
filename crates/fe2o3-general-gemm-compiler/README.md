# fe2o3 general GEMM compiler

This crate binds one checked general tiled GEMM plan, its runtime ABI values,
the complete structured KIR, the selected schedule, and the compiler request.
It consumes the existing compiler-local proof admission before constructing a
real, owner-bound Pliron projection.

The current LLVM Handoff V2 schema cannot represent the required workgroup
BF16 arrays, the wave64 BF16 MFMA fragment/intrinsic, or the loop-carried FP32
fragment accumulator. The route therefore stops with a typed AMDGCN lowering
blocker and returns a transactional compiler rejection. It cannot construct an
LLVM handoff, compiler-worker request, executable candidate, artifact,
publication, load, or launch authority. There is no legacy, COMGR, or shell
compiler fallback.

The reference and A-v4 schedules share the same semantic algorithm and Pliron
projection code. Their domain-separated schedule and aggregate compilation
identities differ, so proof, machine qualification, and artifact evidence
cannot transfer between them.
