# fe2o3 LLM kernel contracts

This crate contains bounded operator contracts and CPU reference models for
the Ferric M1 Qwen3 envelope. It currently exposes two independent foundations.

## GEMM and GEMV

`src/gemm.rs` is the exact host/model foundation for Ferric M1 requirement
`m1.r06`. It binds the target Qwen3-8B and draft Qwen3-0.6B linear shapes to
Ferric B3 graph source commit
`e078ca3f37aeddab43b04e568831b1c7a1471204`, tree
`11d048144b76548d5e3c79f15d09934206903fa3`, and exact `graph.rs` blob and
SHA-256 recorded in the module.

The adapter accepts eight B3 operators across the eleven exact prefill,
decode, and speculative buckets. It selects GEMV only for flattened `M=1` and
GEMM for every `M>1` selection. Checked Qwen `[N,K]` BF16 weight transposition,
contiguous layouts, BF16 operands, increasing-K FP32 accumulation, exact
alpha/beta epilogues, guarded/zero-filled tails, effects, alias premises, and
resources are delegated to or reconstructed from the existing
`fe2o3-tiled-gemm-v1` plan/reference and its twelve-property taxonomy.

The GEMM/GEMV slice does not bypass issue #174. It does not produce attributed source
or MIR custody, Kernel IR, compiler descriptors, objects, HSACO, load or launch
capabilities, GPU observations, performance evidence, machine numerical
refinement, or BF16 output narrowing. Its grid fields are inert planner
arithmetic. No qualification receipt is produced.

The tracked GEMM/GEMV inventory is `Cargo.toml`, `README.md`, `src/lib.rs`,
`src/gemm.rs`, `tests/gemm_differential.rs`, and `tests/gemm_hostile.rs`. The
shared planner/reference/proof inventory remains owned by
`examples/tiled_gemm_v1`.

## RoPE and paged KV write

`src/rope_kv.rs` provides exact split-half Qwen3 RoPE plus an exclusive,
generation-bound paged-KV append model for target and draft caches.

The RoPE/KV slice is intentionally below the compiler and runtime authority boundary:

- it contains no attributed device source and does not bypass the open
  same-session MIR custody work in issue #174;
- it constructs no Kernel IR, LLVM IR, object, or HSACO artifact;
- it exposes no publication, artifact, load, dispatch, launch, or hardware
  authority;
- its `f64` CPU trigonometry is differential scaffolding, not IEEE-754,
  BF16/FP32, OCML, LLVM, ISA, or machine refinement;
- its page model proves and tests conditional mapping and exclusive-write
  properties, not Ferric KV commit/rollback/retirement refinement.

The admitted profile is finite: target Qwen3-8B and draft Qwen3-0.6B geometry,
absolute positions below 8192, batch buckets `1/4/16/32`, active-token buckets
`1/2/3/4/5/8/9/16/17/128/512/2048/8192`, context buckets
`128/1024/4096/8192`, and page sizes `16/64/256`.

The family, candidate schema, and schedule identities are SHA-256 of these
canonical UTF-8 strings, respectively:

```text
fe2o3.qwen3.rope_paged_kv.foundation.gfx942.v1
fe2o3.qwen3.rope_paged_kv.candidate.schema.v1
fe2o3.qwen3.rope_paged_kv.schedule.wave64.split_half.exclusive_pages.v1
```

The complete tracked RoPE/KV inventory is `Cargo.toml`, `README.md`,
`src/lib.rs`, `src/rope_kv.rs`, `tests/cpu_differential.rs`,
`tests/hostile.rs`, `verus/rope_kv_v1.rs`, its three adjacent pin files, and
`run-verus.sh`.
The Rust validator requires independently expected candidate, generation, and
owner identities. No receipt or qualification claim is produced here.

## Validation

Run the ordinary gates with the repository-pinned Rust toolchain:

```sh
cargo fmt --all -- --check
cargo clippy -p fe2o3-llm-kernels --all-targets --locked -- -D warnings
cargo test -p fe2o3-llm-kernels --locked
cargo test -p fe2o3-llm-kernels --release --locked
cargo doc -p fe2o3-llm-kernels --no-deps --locked
```

Run the retained structural proofs with the pinned Verus release:

```sh
VERUS=/absolute/path/to/verus examples/tiled_gemm_v1/run-verus.sh
VERUS=/absolute/path/to/verus ./crates/fe2o3-llm-kernels/run-verus.sh
```
