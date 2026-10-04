# gfx942 scalar arithmetic and cast slice

This slice implements the bounded scalar contracts used by parity rows 24 and
25. It does not promote either row to Complete and does not establish memory,
race, provenance, dispatch, or whole-kernel safety.

## Boundaries

- `fe2o3_kernel_ir::scalar_ops_v2` is the target-neutral schema. A
  `ScalarOperationV2` carries canonical operation bytes in a reserved function
  identity and exact SSA operand/result shapes.
- The physical carrier supports signed and unsigned 8, 16, 32, 64, and 128-bit
  integers, `bool`, the physical `u32` representation of a validated `char`,
  and `f32`/`f64`.
- Pointer/address-space casts and provenance loss are rejected before a carrier
  is created. `f16`, `f128`, and implicit float policies are not admitted.
- Checked operations return `{ value, valid }`; overflowing operations return
  `{ value, overflowed }`. Invalid checked values are canonically zero.
- Non-checked division by zero traps. Signed `MIN / -1` follows the selected
  checked, wrapping, overflowing, or saturating contract.
- Shift validity is computed from the full typed RHS. No intermediate `u32`
  narrowing is permitted. Source shift operators retain their explicit
  overflow-check policy.
- Rust float equality uses ordered equality and unordered inequality. Rust
  ordering uses ordered predicates. IEEE ordered/unordered policies are
  separate. Three-way `total_cmp` is a separate explicit operation.
- Rust float-to-integer `as` uses saturating conversion, including NaN to zero.
  All i128/u128 conversions and div/rem are emitted without target runtime
  libcalls because gfx942 LLVM cannot legalize those hidden calls reliably.
- Floating division is rejected before LLVM. LLVM 18 constrained `fdiv` is not
  a reviewed gfx942 path and has crashed during backend compilation; ordinary
  `fdiv` is not substituted because that would silently weaken the strict
  floating-point policy.

## Compiler admission

`rustc_codegen_fe2o3::scalar_mir_v2` accepts only `gfx942:xnack-` and rejects a
custom LLVM pass pipeline or LLVM arguments. Raw MIR arithmetic, comparisons,
shifts, unary operations, and numeric casts are normalized separately from
authenticated checked/wrapping/overflowing/saturating intrinsics.

Raw integer MIR `Div` and `Rem` remain rejected. Their Rust operator semantics
require composition with the exact MIR assertion terminator; treating the
arithmetic node alone as `wrapping_div` would be wrong. Explicit integer
intrinsic div/rem modes are supported. Raw floating MIR division is separately
rejected because the LLVM 18 gfx942 backend path is unsupported.

## Current limitations

- The scalar admission API consumes a normalized rustc expression. The legacy
  whole-function MIR importer does not yet reconstruct every standard-library
  checked or saturating call into this API.
- The dialect entry point emits one deterministic scalar helper module. Merging
  that helper into the existing multi-kernel compiler-module path is a later
  integration step.
- The configured test `rocm_clang_compiles_every_accepted_gfx942_scalar_path`
  is ignored with `requires ROCm clang with gfx942 support`. It asks
  `FE2O3_SCALAR_CLANG` to compile the 1,544-operation helper module at O0 and
  O2. This command-line object probe is test-only, does not link or dispatch a
  kernel, and records no GPU execution or machine-correctness evidence. The
  production-directed finalizer instead uses pinned upstream LLVM
  target-machine APIs and in-process LLD.
- No Verus proof, signed production evidence, or independent Complete review is
  included here.
