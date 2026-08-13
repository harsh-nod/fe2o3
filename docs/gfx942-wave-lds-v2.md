# gfx942 Wave/LDS V2

This document records the exact-target authority repair layered on the bounded
[gfx942 wave/LDS V1](gfx942-wave-lds-v1.md) slice. The row status remains
Partial. V2 does not add collective operations, change the LLVM templates, or
claim authenticated Rust-source-to-HSACO execution.

## Candidate identity

- Rejected V1 base and V2 parent:
  `a2375e8d2cd34dc9197f7c541c83e062fc8baf69`.
- Exact-target implementation commit:
  `29544cbe08a525e8b78190e3e7f1dd069b5d6f8f`.
- V2 implementation and wire-test commit:
  `c17e847c2017369c1e52d3dfb847001765273cdc`.
- V2 implementation and wire-test tree:
  `d233871061f1b932d10c33ac3c18e96cb75bead9`.
- Sole admitted target ID: exactly `gfx942:xnack-`.

The final documentation and candidate-freeze commits are descendants of the
implementation identified above.

## Exact target authority

The compiler parses both the requested and expected targets as `AmdTargetId`
values and requires all of the following before collective or LDS authority is
created:

- full parsed equality with `gfx942:xnack-`;
- explicit `XNACK` disabled, not omitted;
- no explicit `SRAM-ECC` state;
- canonical parsed spelling equal to `gfx942:xnack-`; and
- original input spelling equal to `gfx942:xnack-`.

This rejects omitted XNACK, XNACK enabled, either explicit SRAM-ECC state,
additional or unknown features, duplicate or conflicting features,
noncanonical spelling, and every other processor. Parsing fails closed before
the private collective target token exists.

Successful lowering binds the extension capability
`fe2o3.amdgpu.target/gfx942:xnack-` into verified Kernel IR. All four Kernel IR
wire versions round-trip that exact capability. The baseline dialect rejects
it; only the strict gfx942 lowerer accepts it.

Worker V2 compares every target-binding capability in the module, functions,
and kernels with the complete canonical `DeviceTargetV1` in the compiler FFI
envelope. The codegen helper receives that complete target value and cannot
reduce the check to processor-only equality. The handoff retains the exact
`gfx942:xnack-` target. Unknown, conflicting, omitted-XNACK, XNACK-enabled,
SRAM-ECC-qualified, and other-processor bindings fail before publication.

## Validation

The genuine Rust source matrix admits only `gfx942:xnack-`. It rejects:

```text
gfx942
gfx942:xnack+
gfx942:sramecc+:xnack-
gfx942:sramecc-:xnack-
gfx942:xnack-:sramecc+
gfx942:xnack-:xnack-
gfx942:xnack-:xnack+
gfx942:future+
gfx941
gfx950
gfx1100
```

The exact source case reaches verified wave/LDS Kernel IR. Focused tests also
cover parser equality and canonicalization, Kernel IR wire persistence,
dialect target-bound admission, Worker V2 envelope matching, and handoff target
identity. The existing LLVM checks still require one ballot, six shuffles, one
1,024-byte aligned LDS allocation, 18 barriers, fixed workgroup metadata, and
wave64 target features.

The V1 direct LLVM/LLD hardware route remains independent of the genuine Rust
source path. It uses no COMGR linking and inspects `gfx942:xnack-` assembly,
`EF_AMDGPU_FEATURE_XNACK_OFF_V4`, and 1,024 bytes of fixed group storage before
the 256-lane MI300X oracle runs.

The Verus source and theorem statements are unchanged. The required proof lane
still comprises six positive harnesses and 26 expected-rejection fixtures.

## Explicit boundary

The repaired authority chain is:

```text
exact gfx942:xnack- source target
  -> private collective/LDS target token       established
  -> verified Kernel IR exact target binding  established
  -> canonical Kernel IR wire persistence     established
  -> Worker V2 exact envelope/handoff check   established in the partial path
  -> complete source finalizer envelope       missing for this profile
  -> finalized/admitted source HSACO          missing for this profile
  -> source-proven GPU execution              therefore missing
```

The independent verified-Kernel-IR-to-LLVM/LLD-to-MI300X lane does not fill the
missing source joins. Compiler correctness, Verus-to-compiler refinement, and
machine-code refinement also remain unproved. These limits keep all affected
dashboard rows Partial.
