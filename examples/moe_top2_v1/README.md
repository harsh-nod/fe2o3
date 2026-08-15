# Exact top-2 MoE routing Phase A

This standalone crate contains ordinary attributed Rust `#[kernel]` source for
one fixed `T8/E4/K2/C4` deterministic router, plus host-side oracle and
proof-facing executable contracts.

The source is real but is not yet compiler-authorized. In particular, the
bounded staging and exclusive-scan structure has no exact authenticated
MIR-to-Kernel-IR profile. This crate therefore claims no artifact, launch,
hardware result, or protected evidence. Later vertical-slice phases must fail
closed until those authorities are implemented.

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

This evidence is not an IEEE-754 `f32` refinement, a refinement of
`src/kernel.rs`, a compiler or machine-code refinement, a GPU memory-safety or
data-race proof, or a GPU execution result. Those joins require later evidence;
the integer model alone cannot establish them.
