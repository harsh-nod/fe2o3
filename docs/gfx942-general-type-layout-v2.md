# gfx942 General Type/Layout V2 Boundary

This slice is an inert compiler-side foundation for CUDA-Oxide parity rows 02,
03, 08, 09, 10, and S04. It does not mark any row Complete and is not protected
evidence.

## Separate Domains

- `RustcTypeLayoutObservationV2` records exact layout facts from the active
  rustc target. Its identity binds that target's LLVM triple, data layout,
  pointer width, canonical semantic graph, and exact layout sidecar.
- A host observation remains a host observation. It is described as observed
  on gfx942 only when the active rustc target itself exactly matches the
  canonical gfx942 triple, data layout, and pointer width.
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
zero-sized aggregate fields, padding, cycles, arithmetic overflow, target or
projection substitution, and resource-limit violations.

## Bounds And Exactness

- Root, normalized, and nested rustc type names are rendered through bounded
  writers before the general layout extractor runs.
- Node, field, variant, path, total text, observation work, sidecar record,
  sidecar byte, projection work, and projection byte limits are explicit.
- Principal adapter-owned variable-size buffers use fallible reservation.
  Sorting and projection traversal are charged before committing their results.
- Untrusted graph input must decode canonically under dialect MIR budgets and
  exactly equal a fresh rustc observation.
- Tests reject max-plus-one work, overflow, mutated observation bytes, sidecar
  substitution, canonical-target substitution, and projection substitution.
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
