# fe2o3 LLM kernel contracts

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

This slice does not bypass issue #174. It does not produce attributed source
or MIR custody, Kernel IR, compiler descriptors, objects, HSACO, load or launch
capabilities, GPU observations, performance evidence, machine numerical
refinement, or BF16 output narrowing. Its grid fields are inert planner
arithmetic. No qualification receipt is produced.

The tracked slice inventory is `Cargo.toml`, `README.md`, `src/lib.rs`,
`src/gemm.rs`, `tests/gemm_differential.rs`, and `tests/gemm_hostile.rs`. The
shared planner/reference/proof inventory remains owned by
`examples/tiled_gemm_v1`.

Run focused gates with the repository-pinned Rust toolchain:

```sh
cargo fmt --all -- --check
cargo clippy -p fe2o3-llm-kernels --all-targets --locked -- -D warnings
cargo test -p fe2o3-llm-kernels --locked
cargo test -p fe2o3-llm-kernels --release --locked
cargo doc -p fe2o3-llm-kernels --no-deps --locked
VERUS=/absolute/path/to/verus examples/tiled_gemm_v1/run-verus.sh
```
