# Exact top-2 MoE routing Phase A

This standalone crate contains ordinary attributed Rust `#[kernel]` source for
one fixed `T8/E4/K2/C4` deterministic router, plus host-side oracle and
proof-facing executable contracts.

The source is real but is not yet compiler-authorized. In particular, the
bounded staging and exclusive-scan structure has no exact authenticated
MIR-to-Kernel-IR profile. This crate therefore claims no artifact, launch,
hardware result, protected evidence, or machine-checked Verus/source refinement
proof. Later vertical-slice phases must fail closed until those authorities are
implemented.
