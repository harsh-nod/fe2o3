# Exact causal FlashAttention Phase A

This standalone crate contains ordinary attributed Rust `#[kernel]` source for
one fixed causal `B1/H1/N8/D16` FP32 profile, an independent two-pass host
oracle, executable proof-facing contracts, and a real machine-checked
mathematical model.

The source is real but is not yet compiler-authorized. It has no authenticated
MIR-to-Kernel-IR profile, direct LLVM/LLD code object, typed HSA lifecycle, or
protected gfx942 execution evidence. Those later phases must fail closed until
their identities and joins exist.

## Machine-checked mathematical model

`verus/flash_attention_v1.rs` is a pinned Verus proof over exact mathematical
integers and rational outputs represented by numerator/denominator pairs. The
pinned run verifies 25 obligations covering the exact `B1/H1/N8/D16` shape,
inclusive lower-triangular causal domain, nonempty causal prefixes, running-max
update bounds, conditional online frame rescaling, exact running denominator
and numerator invariants, positive denominators, correspondence with the
modeled causal reference, tensor indexing and bounds, and total injective
one-writer ownership of all 128 outputs. Ten independently pinned mutations
must each fail its named postcondition.

Run the authenticated proof on a host with the pinned Verus release:

```sh
VERUS=/absolute/path/to/pinned/verus examples/flash_attention_v1/run-verus.sh
```

The runner authenticates the exact kernel and proof bytes, the fixed profile
and model-schema identities, every expected-negative source, the Verus
executable, and the complete 190-file release closure before accepting the
verification result.

## Exponential boundary

`exp_weight_v1` is the model's sole uninterpreted transcendental abstraction.
An admitted frame must supply positive exact weights and the pointwise and
aggregate frame-rescaling relations used by the online recurrence. Verus proves
the causal recurrence and rational correspondence conditional on those explicit
premises; it proves no exponential law.

This evidence is not an IEEE-754 `f32` or OCML numerical refinement, a
refinement of `src/kernel.rs`, a compiler/Kernel-IR/LLVM/ISA refinement, a
machine-safety proof, a GPU data-race-freedom proof, or a GPU execution result.
Each of those joins remains a separate milestone.
