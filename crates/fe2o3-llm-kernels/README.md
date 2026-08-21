# fe2o3 LLM kernels

This crate owns bounded compiler-backed LLM kernel profiles. The first lane is
the Qwen3 BF16-input, FP32-accumulation, FP32-output GEMM/GEMV profile in
`src/gemm.rs`.

The RMSNorm lane in `src/rmsnorm.rs` adds a closed catalog of 132 profiles:
two model roles, eleven Ferric buckets, all five pure graph RMSNorm operations,
and one explicitly separate hidden-width residual-fused operation. Input,
post-attention, final, and residual-fused profiles normalize rows of width 4096
for the target or 1024 for the draft. Query profiles normalize each of 32
target or 16 draft query heads independently at width 128; key profiles
normalize each of eight heads at width 128. Thus query and key row counts are
the active token rows multiplied by their exact head counts.

RMSNorm uses one mode-tagged ABI. Pure graph profiles require both the residual
and fused-output pointers and element counts to be exactly zero. Those optional
pointer parameters have no `nonnull` contract, and typed control flow prevents
loads or stores through them in pure mode. The auxiliary fused mode instead
requires exact nonzero, disjoint BF16 spans and writes both the BF16 fused value
and BF16 normalized output. Both modes declare FP32 square accumulation, mean,
epsilon addition, square root, reciprocal, normalization, and weight multiply.
The declaration is not numerical evidence.

The RMSNorm graph uses typed Handoff V2 directly because the existing Pliron
AMDGPU route rejects scalar BF16. It serializes canonical LLVM into a sealed
Worker V2 request and implements strict post-worker metadata, descriptor,
resource, transcript, and COV6 loader checks. This is an explicit compiler
boundary, not a claim that RMSNorm traverses Pliron. The checked-in
qualification does not execute Worker V2 and establishes no HSACO existence,
LLVM-to-machine refinement, numerical correctness, hardware behavior, or
performance result.

The generic ABI carries behavior, rows, and width, but not the six-operation
tag or a profile identity. It rejects fused mode at head width 128, unsupported
widths, zero rows, and incompatible spans. It does not constrain rows to the
finite catalog and cannot distinguish Input, PostAttention, or Final RMSNorm
when their pure hidden geometry is identical. Exact operation and bucket
selection are retained only by the host checked binding and still need an
identity join to the Ferric plan and protected runner.

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

Consequently these foundations do not close their full M1 kernel obligations.
Closure still requires an
authenticated rustc frontend owner, verifier property evidence and final join,
execution of the GEMM requests and RMSNorm request with the pinned Worker V2
tool, independent artifact
approval, protected KFD allocations and kernarg packing, AQL publication and
completion custody, differential numerical tests, and exact-device hardware and
performance evidence. The post-worker APIs are implemented but were not invoked
to make the source-only qualification recorded with this change.
