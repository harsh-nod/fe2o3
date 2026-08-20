# fe2o3 general GEMM compiler

This crate binds one checked general tiled GEMM plan, its runtime ABI values,
the complete structured KIR, the selected schedule, and the compiler request.
It consumes the existing compiler-local proof admission before constructing a
real, owner-bound Pliron projection.

The structural gfx942 route now represents the required workgroup BF16 arrays,
wave64 BF16 MFMA fragment/intrinsic, loop-carried FP32 accumulators, and both
closed schedules. It lowers them into a live owner-bound Pliron LLVM graph,
freshly exports that graph into Handoff V2, serializes it, admits an exact
LLVM/LLD build-policy request at the worker boundary, and retains the concrete
graph, Worker, finalized-byte, and inspection owners through post-link
observation. The late graph, worker, and finalizer axes are freshly derived
from those owners. Build-policy admission is not worker measurement
authentication, and these axes grant no authority.

The production selector remains fail-closed until one rustc-owned authority
join consumes the #174 owner-carrying authenticated Rust MIR-to-KIR receipt,
live graph serialization, worker request/response, post-link ISA observation,
and verifier identity chain. The current same-session MIR graph owner is still
authenticated through the ordinary-scalar frontend contract and does not yet
satisfy that General GEMM receipt. Until that join is installed, it returns a
transactional compiler rejection and grants no candidate, artifact,
publication, load, or launch authority. There is no legacy, COMGR, or shell
compiler fallback.

The reference and A-v4 schedules share the same semantic algorithm and Pliron
projection code. Their domain-separated schedule and aggregate compilation
identities differ, so proof, machine qualification, and artifact evidence
cannot transfer between them.
