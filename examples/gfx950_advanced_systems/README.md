# gfx950 advanced systems kernels

The ordinary attributed Rust kernels in [`src/kernel.rs`](src/kernel.rs) are
the fe2o3 source for these tutorials. [`src/reference.rs`](src/reference.rs)
contains independent safe CPU references, and `cargo test --offline` checks
their bounded numerical and transactional contracts. The HIP program remains
a separate compiler, ISA, and MI350 hardware-validation companion.

Each `run-*-gfx950.sh` entry point selects exactly one kernel feature, invokes
the production fe2o3 extractor, checks the compiler-published crate binding,
links an exact gfx950:xnack- COV6 HSACO, validates its single-kernel metadata
and symbol-scoped ISA, and runs a digest-pinned numerical HSA test. This is an
explicit verification path; it does not grant protected artifact publication
authority. The HIP results below remain independent evidence.

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

Run the production Rust lowering and numerical verification on a gfx950 host:

```bash
./run-moe-route-gfx950.sh
./run-moe-expert-rank-gfx950.sh
./run-combine-expert-ranks-gfx950.sh
./run-speculative-transaction-gfx950.sh
./run-qwen-ngram-gather-gfx950.sh
./run-stage-gradient-shard-gfx950.sh
./run-muon-update-gfx950.sh
```

The expert runner requires exactly three mixed FP4/FP8
`v_mfma_f32_16x16x128_f8f6f4` instructions with `cbsz:4`: two rank-local
experts and the optional shared expert. Routing and expert exponential math
links only the reviewed ROCm 7.2.1 OCML `exp` closure shared with the
low-precision examples; Muon square root uses the gfx950 native LLVM intrinsic.
The per-kernel harness checks immutable inputs, output canaries, exact integer
and rollback state, and bounded floating-point tolerances against the CPU
references. Set `FE2O3_REPO_ROOT`, `ROCM_PATH`, `RUSTUP`, `CARGO`, or the
documented tool and target-directory environment variables when validating a
copied checkout.

## Production Rust validation evidence

On 2026-08-27, all seven production Rust wrappers passed on SSH host `mi350`
(`smci350-rck-g03-b19-03`) with ROCm 7.2.1 and eight visible MI350X devices.
Routing metadata, dispatch entries, N-gram results, and gradient shards matched
exactly. The largest observed absolute errors were `4.768371582e-7` for the
two expert-rank launches, `2.980232239e-8` for speculative state, and
`7.450580597e-9` for the Muon update; rank combine and the Muon norm had zero
error. The corresponding float tolerances are `2e-6` for routing weights,
`3e-3` for expert and rank combine, `1e-7` for committed speculative state,
and `2e-6` for Muon.

The expert-rank Rust HSACO contained exactly three
`v_mfma_f32_16x16x128_f8f6f4` instructions with FP4-A/FP8-B selector
`cbsz:4`. The per-kernel portable namespace and LLVM/HSACO SHA-256 values are
printed by each wrapper and pinned by the corresponding advanced tutorial
evidence record.

Run the independent HIP compiler, target-rejection, ISA, and numerical suite:

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
