# Exact MoE Top-2 Private Structural Record V2

This increment adds a private, inert structural record to the successful
`collected-moe-top2-v1` rustc admission. It does not add a public verifier API
and does not prove source-to-Kernel-IR semantic refinement.

## Live Inputs

The producer runs inside `rustc-codegen-fe2o3` after the existing exact source,
compiler-session, trusted-definition, `FnAbi`, portable-MIR, KIR, and profile
checks. One live admission supplies:

- the authenticated source bytes and their SHA-256 identity;
- the rustc-derived `FnAbi` identity and all eight observed pair-mode argument
  records;
- the admitted portable-MIR identity and a bounded whole-module diagnostic
  summary;
- the already validated `MoeTop2KernelIrV1` and `MoeTop2ProfileV1` values.

The portable-MIR summary counts functions, roots, helpers, blocks, statements,
terminators, CFG edges, external imports, root arguments and locals,
assignments, calls, indexed places, repeated values, and observed binary
operator kinds. The summary is computed over the complete imported module.
These counts are diagnostics pinned to the exact live admission, not semantic
routing evidence.

## Canonical Fields

The validated KIR and profile are encoded into one ordered private field table.
Each actual field is serialized once with a unique field name and projection
membership bits. The full KIR, full profile, ABI, effects, and routing
identities are domain-separated hashes over selected entries from that same
table. None of those identities is copied from a precomputed KIR constant.

The final record digest frames the raw source bytes, source identity, complete
observed `FnAbi`, whole-module MIR summary, and the canonical field table. Every
checked structural input is therefore committed by the record. A readable
snapshot beside the implementation pins all observed fields and all five
derived identities. That pin is filled only from a successful live rustc
admission and is covered by exact positive, hostile, and relocation tests.

## Boundary

This record establishes that one authenticated compiler session observed the
pinned source, `FnAbi`, portable-MIR identity and diagnostics, then selected the
pinned validated KIR/profile field table. It does not establish that MIR values
and effects simulate the KIR routing state machine.

The first unproved boundary remains a mechanically checked value- and
effect-preserving simulation from the authenticated portable-MIR CFG to the
exact MoE KIR, including failure paths, loops, indexing, FP32 comparisons,
writes, and ordered routing transitions. Issue #106 remains open.

The record grants no Worker V2, LLVM, ISA, artifact, load, launch, runtime, GPU,
or hardware authority. It proves no IEEE FP32 or OCML semantics, logical-to-
machine addressing, generalized memory safety, or race freedom.
