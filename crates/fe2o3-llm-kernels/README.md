# fe2o3 LLM kernels

This crate owns bounded compiler-backed LLM kernel profiles. The first lane is
the Qwen3 BF16-input, FP32-accumulation, FP32-output GEMM/GEMV profile in
`src/gemm.rs`.

The GEMM lane enumerates the exact target/draft model geometry and all eleven
Ferric M1 bucket selections. It lowers the existing runtime-parameterized
general-GEMM semantic graph through Pliron, typed Handoff V2, canonical LLVM,
and two sealed Worker V2 schedules. Post-worker inspection retains both source
owners, checks the exact ABI, resources, machine-code profiles, descriptor and
compilation-binding sections, and applies the strict `gfx942:xnack-` COV6
loader envelope.

This is an inert compiler and inspection boundary. Caller identities are
untrusted labels, observed output digests are not independently approved
deployment pins, and no type here grants publication, allocation, load,
launch, completion, hardware-correctness, numerical-refinement, or performance
authority. The compiler graph does not prove LLVM or machine semantics. Qwen3
token embedding remains a gather, not a GEMM, and is not covered by this lane.

The ABI is deliberately BF16 A and B with an FP32 C accumulator/output. Alpha
is FP32 one. Attention-output and down projections select beta FP32 one so a
caller can provide the residual as initial C; all other profiles select beta
zero. Those are checked graph and ABI choices, not a claim that the generated
machine computes Qwen3 numerically. Any BF16 inter-layer output conversion or
fused epilogue required by the physical runner remains outside this slice.

Consequently this crate does not close M1 K1. Closure still requires an
authenticated rustc frontend owner, verifier property evidence and final join,
execution of both requests with the pinned Worker V2 tool, independent artifact
approval, protected KFD allocations and kernarg packing, AQL publication and
completion custody, differential numerical tests, and exact-device hardware and
performance evidence. The post-worker APIs are implemented but were not invoked
to make the source-only qualification recorded with this change.
