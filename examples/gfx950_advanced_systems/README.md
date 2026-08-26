# gfx950 advanced systems kernels

The ordinary attributed Rust kernels in [`src/kernel.rs`](src/kernel.rs) are
the fe2o3 source for these tutorials. [`src/reference.rs`](src/reference.rs)
contains independent safe CPU references, and `cargo test --offline` checks
their bounded numerical and transactional contracts. The HIP program remains
a separate compiler, ISA, and MI350 hardware-validation companion.

The current rustc backend cannot lower these Rust sources to gfx950 HSACO: its
production profile is still exact `gfx942:xnack-`, and gfx950 low-precision
semantic importing and Kernel IR lowering are pending. The source therefore
claims source and CPU-reference evidence only; the HIP results below do not
silently become fe2o3 artifact evidence.

The fixed-shape suite covers several systems patterns on AMD CDNA 4 (`gfx950`).
It is deliberately small enough to have independent, deterministic CPU oracles;
it is not a framework, a distributed runtime, or a production collective
implementation.

The executable covers:

- A 16-token, 128-input, 16-output fused FP4/FP8 MoE. It computes router
  logits, stable top-2 selection (lower expert ID wins exact ties), compact
  per-expert dispatch metadata, five expert tiles with a gfx950 mixed
  FP4/FP8 `16x16x128` scaled MFMA, SiLU, routed weighting, and a shared-expert
  contribution.
- Two logical rank-local expert partitions. The compact fixture copies its full
  five-expert weight tensor to each device, but each rank kernel reads only its
  assigned two-expert partition. With two visible gfx950 devices, the suite
  executes one rank per device and uses peer copies when HIP exposes
  bidirectional peer access, including a peer return and GPU combine on device
  zero. Otherwise it uses a deterministic, host-staged transport simulation.
  Rank results are reduced in fixed order and checked against independently
  decoded CPU expert GEMMs. This validates the bounded execution path, not an
  expert-parallel communication library.
- Eight four-token speculative/MTP candidates with deterministic prefix
  acceptance. Recurrent/KV-style state is committed only after full acceptance;
  all other candidates roll back to the byte-identical base state.
- A GPU-resident, 16-slot Qwen-style 3-gram hash table gather. Lookup scans in
  linear-probe order, verifies the complete key, selects higher priority on
  duplicate keys, and then the lower slot on a tie. Host-offload overlap,
  eviction, and table construction are outside this kernel's scope.
- A 4x4 Muon update from two sharded gradient contributions. GPU kernels stage
  each rank, while the host stages a deterministic rank-order reduction before
  a GPU Frobenius norm and five Newton-Schulz/polar iterations. This is
  explicitly not a GPU collective or a distributed optimizer runtime.

Every input is deterministic. The executable compares outputs, routing
metadata, accepted prefixes, transaction states, hash-table results, norms,
and optimizer updates to separately implemented CPU references. Runtime
execution rejects devices whose GCN architecture is not exactly `gfx950`.

Run the complete compiler, target-rejection, ISA, and numerical suite on MI350:

```bash
./build_and_test.sh
```

Run only compilation and symbol-scoped ISA validation on another compiler host:

```bash
./build_and_test.sh --compile-only
```

`check_isa.sh` requires all seven kernel symbols and checks that the fused expert
tile contains `v_mfma_f32_16x16x128_f8f6f4` with the FP4 format selector
`cbsz:4`. The build also requires compilation for `gfx942` to fail, preventing
this gfx950-only example from silently targeting an older architecture.

## Validation evidence

On 2026-08-26, `./build_and_test.sh` passed through SSH host `mi350`
(`smci350-rck-g03-b19-03`) with ROCm 7.2.1, HIP 7.2.53211, AMD Clang 22,
and eight visible AMD Instinct MI350X devices. The peer-access path selected
`mode=two-device-peer`; Muon selected its explicit
`mode=two-device-host-staged` reduction. The retained run reported zero error
for fused MoE, routing weights, speculative state, staged Muon shards, and the
Muon norm. Its largest expert-partition error was `4.76837e-07`, and its Muon
update error was `4.65661e-09`. The N-gram test reported four hits, four misses,
and deterministic duplicate-key winner `4242`.

The tested HIP source SHA-256 was
`c29a6bc2de55563abddfb50f43aaccf6077ef0b4706fbfb314266ecaa48054c5`.
The retained gfx950 HSACO SHA-256 was
`5ccc37902f9b549ac405f1096ad6df8ea58eba5dd6a08c765f5ea3148eb47d16`.
Symbol-scoped disassembly found exactly one
`v_mfma_f32_16x16x128_f8f6f4` in the fused MoE kernel, with `cbsz:4`.
