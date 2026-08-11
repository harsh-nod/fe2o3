# gfx942 General Type/Layout V2 Boundary

This slice is an inert compiler-side foundation for CUDA-Oxide parity rows 02,
03, 08, 09, 10, and S04. It does not mark any row Complete and is not protected
evidence.

## Separate Domains

- `RustcTypeLayoutObservationV2` records exact layout facts from the active
  rustc target. Its identity binds that target's LLVM triple, data layout,
  pointer width, effective target CPU, normalized effective target-feature
  configuration, canonical semantic graph, and exact layout sidecar. The
  strengthened observation, projection, and candidate hash domains are V3;
  semantic-layout evidence uses the profile-aware V2 canonical schema, while
  the public V2 compatibility APIs remain inert records.
- A host observation remains a host observation. It is described as observed
  on gfx942 only when the active rustc target itself exactly matches the
  canonical gfx942 triple, data layout, pointer width, `gfx942` CPU, wave64
  feature state, and explicit `xnack-` profile. Missing, `default`, `generic`,
  `native`, conflicting, `gfx900`, `xnack+`, and omitted-XNACK profiles fail
  closed. Feature order and repeated declarations with the same state are
  normalized; contradictory declarations are rejected.
- `CanonicalGfx942LayoutProjectionV2` is derived independently from reviewed
  gfx942 scalar, array, `repr(C)`, and restricted `repr(transparent)` rules. It
  does not inherit size, alignment, offsets, stride, or padding from a target
  label attached by a caller.
- `Gfx942LayoutCompatibilityCandidateV2` exists only after every reachable
  accepted node's host observation exactly equals the canonical projection.
  Its identity binds the complete observation identity and exact canonical
  gfx942 target/projection bytes.

Compiler digests, source digests, and generations are deliberately absent from
these identities and APIs. Caller declarations cannot establish compiler or
source provenance, freshness, eligibility, or protected evidence. A future
compiler-transaction and protected-evidence bridge must provide those facts.

## Conservative Compatibility Subset

The candidate derivation accepts only:

- all-bits-valid fixed-width integers of 8, 16, 32, or 64 bits;
- `f32` and `f64`;
- bounded arrays with exact canonical element stride and checked total size;
- padding-free `repr(C)` structs in source field order; and
- `repr(transparent)` structs restricted to exactly one nonzero-sized accepted
  field at offset zero.

It rejects `repr(Rust)`, tuples, enums and niches, unions, pointers,
references, DSTs, packed or explicitly aligned aggregates, validity-constrained
scalars such as `bool` and `char`, unreviewed widths including 128-bit scalars,
pointer-sized `usize` and `isize`, `Unit`, zero-sized aggregate fields, padding,
cycles, arithmetic overflow, target or projection substitution, and
resource-limit violations. Pointer-sized integer provenance is retained in the
rustc fact layer and rejected before it can collapse to fixed-width semantic
integer data, including through aliases, arrays, fields, and transparent
wrappers.

## Bounds And Exactness

- Root, normalized, and nested rustc type names are rendered through bounded
  writers before the general layout extractor runs.
- Observation preflight tracks unique rustc types for graph limits while also
  traversing every occurrence edge. It cumulatively reserves extraction,
  rendering, graph, sidecar, and sorting work before the recursive occurrence
  tree is allocated. Repeated-type diamonds therefore cannot hide exponential
  reconstruction behind a small deduplicated graph budget.
- Node, field, variant, path, total text, observation work, sidecar record,
  sidecar byte, projection work, and projection byte limits are explicit.
- Principal adapter-owned variable-size buffers use fallible reservation.
  Projection preflight checks current graph counts, supported kinds, clone and
  render work, canonical output bytes, sidecar text, checked arithmetic, and
  sorting before `ProjectionBuilderV2` reserves graph-sized state. Builder
  cloning and sorting remain cumulatively metered after admission.
- Untrusted graph input must decode canonically under dialect MIR budgets and
  exactly equal a fresh rustc observation.
- Tests reject exact-bound-minus-one work/storage, zero-budget construction,
  repeated-type expansion, oversized unsupported sidecars, overflow, mutated
  observation bytes, sidecar substitution, active CPU/feature substitution,
  canonical-target substitution, and projection substitution.
- MI300X tests compare the reviewed rules with independent gfx942 record-layout
  probes under ROCm LLVM 22 and Ubuntu LLVM 18. A ROCm LLVM 22 object probe also
  checks the emitted ELF and AMDGPU metadata target.

## No Authority

An observation, projection, compatibility candidate, or fixture-byte
differential grants none of the following:

- the public `DeviceCopy` trait or permission to read object bytes;
- allocation, host-to-device transfer, or device-to-host transfer;
- compiler/source provenance, freshness, or a compiler transaction;
- artifact, manifest, link, load, dispatch, occupancy, or launch authority;
- a Verus proof or signed/protected parity evidence.

The general extractor and dialect graph still use standard Rust collections in
lower layers; process-abort behavior under allocator exhaustion is not claimed
to be eliminated by this slice. All production authority bridges remain out of
scope.
