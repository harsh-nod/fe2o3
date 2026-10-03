# Multi-GPU Fill Preparation

This checkpoint fixes two prerequisites for the verified Rust fill -> peer copy
-> readback workflow. It does not complete dynamic output coverage, protected
Worker admission, source-to-machine refinement, or native multi-GPU qualification.
Broader HIP/HSA parity and performance work remains behind that critical path.

## Changes

- The source projector distinguishes a failed write-only operation from a
  trapping dereference. A failed write skips the store and reaches the same
  continuation as a successful write. Live loop arguments survive both edges.
  Checked reads and references keep their existing trapping behavior. A consumed
  write-result Boolean still gets conservative control-flow analysis, not an
  invented exact predicate.
- Runtime preparation now accepts the exact explicit-only COV6 layout already
  supported by the finalizer and KFD packet binder: no hidden records, no implicit
  offset, zero implicit size, and physical size equal to supplied explicit bytes.
  The alternate profile still requires the exact trailing 256-byte hidden block.
  No phantom suffix is allocated. Resource, geometry, pointer-fixup and dynamic
  LDS checks remain on the common path.
- A test-only context helper used by the existing multi-GPU CPU tests is no
  longer gated behind the hardware-qualification feature. The initial default
  runtime test build exposed this configuration mismatch. Production APIs and
  admission authority are unaffected.

## Qualification

Parent commit: `9eba818bab38d105e28f8891e04411e94fb782ad`.
Pinned toolchain: `nightly-2026-04-03`; four Cargo jobs, no incremental builds,
test opt-level 1, debug assertions and overflow checks enabled, serial tests.

| Check | Result |
| --- | --- |
| Runtime library, default features | 2,200 passed, 5 ignored; 191.58 seconds |
| Compiler library | 524 passed |
| Captured-fill runtime integration | 5 passed |
| Actual AMD write-only mapping extraction | 1 passed, exercising four mapping variants and an invalid-geometry rejection |
| Actual AMD V9 guarded-store LLVM/descriptor extraction | 1 passed |
| Runtime/compiler library Clippy | Passed with `--no-deps -- -D warnings` |
| Targeted formatting and whitespace checks | Passed |

The new finite-graph tests run the normal nine-pass compiler pipeline at lengths
64, 65 and 4097. Exact padded launches pass TotalView checks; underlaunches fail
ownership. These known finite test shapes do not replace dynamic production
extents with guessed sizes. Actual mapping extraction also rejects any projected
trap in the selected write-only fixture profiles.

The captured fill's physical 16-byte kernarg survives runtime preparation,
persistent packet projection, and consuming generated storage at the same three
lengths. Tests check retained object/dispatch identities, geometry, pointer fixup,
WriteOnly policy and complete initialized bytes. Truncated/oversized arguments,
missing dynamic-LDS fields, wrong workgroups and out-of-range fixups reject.
The existing 256-byte hidden profile still initializes geometry and reserved bytes.

The captured HSACO SHA256 is
`f31451aa1e21d20120fa7d7d6428c3c5efcfda9493b13e0dbf9a7047a30244c5`.
It is genuine captured compiler output, not a protected finalized Worker artifact.
The preparation checks grant no execution authority and do not establish output
values or complete dynamic coverage. No protected Verus or GPU execution was run
for this checkpoint. No files or processes were created on MI300X.

[Qualification archive](qualification.tar.xz), SHA256
`c50fea05ed51631e89f4b54908bf441fa8a5643b252cd07a26f028b0cdc49a04`,
contains the candidate code patch, exact commands and final passing test output.
Earlier development iterations are not included.

## Next Critical Path

1. Classify the actual retained guarded-identity graph and retain conditional
   coverage separately from unconditional TotalView. Bind its extent to the
   authenticated source output and actual KIR slice-length component.
2. Carry the conditional aggregate proof through compiler and Worker custody,
   without promoting per-point equality or receipt identity into total coverage.
   At invocation preparation, check actual packed length `N <= G`, with `G` the
   AQL global work-item count, not HIP block count or a maximum-grid annotation.
3. Complete the same-owner source/KIR/optimized-code/ISA relation and protected
   application provider. Inspect the actual finalized artifact; preparation of
   the captured fixture does not substitute for this evidence.
4. Run fill -> staging/upload -> native peer transfer -> complete readback in
   both directions for lengths 64, 65 and 4097 on freshly observed idle GPUs.
   Check guard bytes, exact completion, peer counters, credits and cleanup.
