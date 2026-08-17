# Exact top-2 MoE routing Phase A

This standalone crate contains ordinary attributed Rust `#[kernel]` source for
one fixed `T8/E4/K2/C4` deterministic router, plus host-side oracle and
proof-facing executable contracts.

The bounded compiler profile authenticates the exact attributed source, kernel
root and `FnAbi`, reviewed provider-terminal manifest, and complete reachable
portable-MIR closure. It checks a private same-session structural
source/ABI/MIR/KIR record, explicitly not semantic refinement, selects the
closed deterministic routing Kernel IR profile, and publishes an inert COV6
Worker V2 handoff containing one kernel and five private helpers.

A configured finalizer test is ignored with the exact prerequisite
`requires the compiler-produced module and measured direct LLVM/LLD worker`.
It uses the pinned upstream LLVM target-machine and in-process LLD worker to
produce reproducible opaque raw and finalized identities, without granting
publication, load, launch, or hardware authority. The hardware gate remains
ignored with `requires the production static wrapper, exact measured pins,
protected linear receipt injection, and MI300X` and deliberately fails closed
before HSA load until that wrapper can deliver the linear receipt in-process.

## Machine-checked mathematical model

`verus/moe_top2_v1.rs` is a real pinned Verus proof for the exact fixed
`T8/E4/K2/C4` routing profile. It uses mathematical integer scores and verifies
28 obligations covering profile/source/model identity admission, expert range
and distinctness, deterministic lower-expert-ID tie order, requested and
capacity-clamped admitted counts, exclusive-scan offsets and total bounds,
stable-prefix acceptance and dropping, accepted-slot bounds and uniqueness,
permutation/inverse round trips, and sentinel tails. Nine independently pinned
mutations must each fail its named postcondition.

Run the authenticated proof on a host with the pinned Verus release:

```sh
VERUS=/absolute/path/to/pinned/verus examples/moe_top2_v1/run-verus.sh
```

The runner authenticates the Verus executable and complete release closure,
the exact kernel bytes, the exact proof bytes, the fixed profile and model
schema identities, and every expected-negative source before verification.
Substitution at any of those boundaries fails closed.

This proof evidence is not an IEEE-754 `f32` refinement, a refinement of
`src/kernel.rs`, a compiler or machine-code refinement, a GPU memory-safety or
data-race proof, or a GPU execution result. The compiler and finalizer evidence
above does not establish those joins either.

## Bounded memory/effect proof

`verus/moe_top2_memory_v1.rs` independently models the fixed logical memory
effects of the same `T8/E4/K2/C4` source profile. Its pinned Verus run verifies
16 obligations over the exact eight-buffer ABI: `logits: f32[32]` is read-only,
while `top2_experts: u32[16]`, `requested_counts: u32[4]`,
`admitted_counts: u32[4]`, `expert_offsets: u32[5]`, `route_slots: u32[16]`,
`permutation: u32[16]`, and `inverse: u32[16]` are bounded outputs.

The obligations cover exact extents, logical address bounds, pairwise region
disjointness, lane-zero write ownership, absence of duplicate logical write
owners, stable routing-phase order, and bounded expert, route, slot,
permutation, and inverse values including the drop sentinel. Eight independently
pinned memory mutations must fail their named postconditions.

Run this second proof with the same exact pinned Verus closure:

```sh
VERUS=/absolute/path/to/pinned/verus examples/moe_top2_v1/run-memory-verus.sh
```

This is a finite logical-source proof. Its expected-evidence descriptor is
copyable and inert and authenticates nothing. It does not mint or join an
`AuthenticatedVerusExecutionReceiptV2`, prove source/compiler/KIR/LLVM/ISA or
logical-address refinement, grant artifact authority, establish generalized
machine memory safety or GPU race freedom, or report GPU execution.
