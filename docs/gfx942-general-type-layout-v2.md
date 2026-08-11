# gfx942 General Type/Layout V2 Boundary

This slice covers CUDA-Oxide parity rows 02, 03, 08, 09, 10, and S04 as a
compiler-side foundation. It does not mark those rows Complete.

## Implemented

- `dialect-mir` deliberately exports the bounded semantic type graph V2.
- `rustc-codegen-fe2o3` captures exact pinned-rustc layout facts for fully
  monomorphized sized values and root pointers to scalar slices, `str`, and
  trait-object DSTs.
- Captures bind the active rustc target, the exact gfx942 target profile, a
  compiler/source revision and generation, canonical graph bytes, normalized
  representation flags, field memory order, padding, array stride, enum
  encoding, discriminants, payload offsets, and scalar validity.
- Untrusted canonical bytes are decoded under explicit budgets and accepted
  only when they exactly equal a fresh rustc recapture.
- Structural DeviceCopy layout eligibility is separate and conservative. It
  accepts fixed-width all-bits-valid scalars, arrays, and padding-free
  `repr(C)`/`repr(transparent)` structs. It rejects stale or mismatched target
  identities, `repr(Rust)` aggregates, tuples, packed or padded structs,
  enums, validity-constrained scalars, references, raw pointers, DSTs, and
  unions without a future explicit all-fields-safe proof.

## Not Implemented

- The capture and eligibility token are not consumed by artifact manifests,
  allocation/transfer APIs, device linking, loading, or launch admission.
- Eligibility does not prove that the public `DeviceCopy` trait is implemented.
- Pointer provenance, address-space capabilities, and pointer-niche enums are
  intentionally rejected rather than normalized into integer bits.
- Nested pointers to DSTs and non-scalar slice elements are not in this V2
  capture profile.
- No production-safe dispatch, verifier proof, or signed parity evidence is
  claimed by this slice.
