# gfx942 Floating-Point Lowering V1

`lower_kernel_to_gfx942_llvm_ir` and
`lower_compiler_module_to_gfx942_llvm_ir` are the only APIs that admit
`FloatOperation`. The baseline G1 APIs continue to reject `Float16`,
`BFloat16`, and every explicit floating-point operation.

The gfx942 profile fixes these LLVM function attributes:

- `target-cpu=gfx942`
- `denormal-fp-math-f32=ieee,ieee`
- unsafe, approximate, finite-only, no-NaN, no-infinity, and no-signed-zero
  modes disabled

`F16` and `Bf16` remain integer-backed `i16` values in memory and SSA. Their
widening and round-to-nearest-even narrowing helpers are emitted as explicit
integer bit algorithms. Widened arithmetic uses one LLVM constrained `f32`
operation between those helpers. `Bf16x2` is an `i32`; its FMA lowering
extracts both lanes, performs exactly two constrained `f32` FMAs, narrows each
result with the BF16 RNE helper, and repacks the lanes.

`sqrt`, FMA, floor, ceil, truncation, and round-to-even use LLVM constrained
intrinsics. Transcendentals use their exact `__ocml_*_f32` ABI names. OCML
symbols remain unresolved in the emitted module. A later authenticated
direct-LLVM link plan must bind the expected device-library bitcode identity;
this dialect does not search for, load, or authenticate device libraries.

The lowering produces inert LLVM text. It grants no compiler provenance,
proof, link, code-object, load, launch, or execution authority. The gfx942
compile probes validate LLVM acceptance and metadata only; they do not claim
runtime output validation.
