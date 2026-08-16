# Exact host-scheduled MoE expert compute V1

This standalone crate starts the bounded expert-compute slice for the public
`T8/E4/K2/C4` router. It consumes the router's route IDs, stable permutation,
inverse map, expert offsets, capacity, and drop sentinel. The routing ABI does
not produce gating weights, so this slice makes them an explicit finite,
nonnegative input in token-major/rank-minor route-ID order.

The host compacts accepted token activations into four zero-padded `16x16`
BF16 tiles, schedules four independent exact `16x16x16` BF16/F32 GEMMs, packs
active expert rows back into routing-slot order, and combines each token's two
route rows in rank order. Dropped routes contribute zero without
renormalization.

`src/pipeline.rs` is an executable host schedule for that exact plan.
`src/oracle.rs` is independent: it evaluates accepted routes directly in
route-ID order instead of replaying the compaction and per-tile loops. Tests
compare every active and padded expert output, every inverse-permuted compact
row, every route-order weight contribution, and every final token output. They
cover empty experts, capacity drops, lower-expert ties, balanced and patterned
data, input immutability, and adjacent canaries.

`verus/moe_expert_memory_v1.rs` is a fixed logical-source model for exact
index bounds, inverse-slot admission, zero-padding separation, disjoint expert,
compact, and combined write owners, and host phase order. The pinned runner
verifies 15 obligations and requires six named mutations to fail at their
postconditions. Its expected evidence is copyable and inert and cannot mint or
join an authenticated receipt. It proves no Rust-source, MIR, Kernel-IR,
LLVM/ISA, logical-address, artifact, machine-memory, generalized race-freedom,
numerical, or GPU-execution result.

Run it with the pinned release closure:

```sh
VERUS=/absolute/path/to/pinned/verus examples/moe_expert_v1/run-verus.sh
```

`src/kernel.rs` contains ordinary attributed Rust `#[kernel]` definitions for
the expert GEMM and deterministic combine. It contains no `macro_rules!`
kernel facade.

## Integrated bounded bridge

The repository's [bounded MoE V1 checkpoint](../../docs/bounded-moe-v1.md) adds
support around this crate without turning these expert kernels into an
executable GPU path:

- the router compiler profile produces a private, inert same-session structural
  record from rustc-loaded source, the complete checked `FnAbi` identity and
  bounded projection, full imported-MIR diagnostics, and the canonical
  KIR/profile table; it is not a MIR-to-KIR refinement proof;
- a separate exact `E4/C4/routes16/width16/tile256` compact-plan model verifies
  19 Verus obligations, rejects seven negative mutations, and passes all 625
  valid count vectors; it is not bound to this host implementation, runtime
  copies, or machine addresses; and
- a host-observed bridge checks the internal relation among caller-supplied
  top-2 IDs, counts, offsets, slots, permutation, and inverse, then uploads and
  retains offsets plus inverse together. Its `gfx942` test reads those uploaded
  arrays back but does not execute or authenticate the router.

The host adapter manually pins the exact eight-region expert ABI and checks
their typed lengths, access, context, target, and non-aliasing. It derives an
inert compact-copy plan from the retained offsets. Expert preparation then
terminates at `deny_moe_expert_execution_v1`; no copy plan, kernel load, or
dispatch can begin through the safe API.

The typed V2 follow-on binds exact request/batch identity, dispatch completion
and readback order, route-weight policy, packed activations, model weight
artifact identity, lifecycle context/stream, typed regions, and fixed ABI
mechanics through private move-only stages. Its checked upload retains packed
activations, offsets, inverse, and route weights together; its generated adapter
requires the matching weight binding and checks all eight region extents,
access roles, contexts, alignments, and alias pairs.

Those V2 mechanics do not create an executable path. There is no production
issuer for completion/readback provenance or the expert-weight binding, making
safe upload and preparation constructively unreachable. V2 grants no artifact,
copy, load, or dispatch authority and proves no routing/expert semantics,
source-to-machine refinement, generalized memory safety or race freedom, or
numerical correctness. The V1 `gfx942` offsets/inverse upload-readback test is
not V2 evidence; no V2 GPU observation or parity promotion is claimed.

Run the focused non-hardware checks from the repository root:

```sh
python3 scripts/test-bounded-moe-docs.py
cargo test --locked --manifest-path examples/moe_expert_v1/Cargo.toml
cargo test --locked -p fe2o3-verifier --test moe_expert_compact_plan_v1
VERUS=/absolute/path/to/pinned/verus \
  ./scripts/test-moe-expert-compact-plan-verus.sh
cargo test --locked -p fe2o3-host --lib moe_routing_expert_bridge_v1::tests
cargo test --locked -p fe2o3-host --lib moe_routing_expert_bridge_v2::tests
cargo test --locked -p fe2o3-host --lib generated_moe_expert_v2::tests
cargo test --locked -p fe2o3-host --features hardware-test-hooks \
  --test generated_moe_expert_v2_ui
```
