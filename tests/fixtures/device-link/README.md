# Bidirectional gfx942 device-link FFI fixture

This fixture fixes both directions of a Rust/HIP device-link boundary to exact,
unmangled C ABI names:

| Caller | Definition | Role | Physical ABI | Observable output |
| --- | --- | --- | --- | --- |
| `rust_calls_hip_kernel_v1` | `external_scale_bias_v1` in HIP | device function | `(u32,u32)->u32` | `3 * input[lane] + 5 + lane` |
| `hip_calls_rust_kernel_v1` | `rust_accumulate_v1` in Rust | device function | `(u32,u32)->u32` | `7 * input[lane] + 11 + lane` |

Arithmetic is modulo 2^32. Both kernels accept an input pointer, output pointer,
and extent in their link-test models. The fixed CPU oracle includes zero, small,
`UINT32_MAX`, and high-bit inputs and compares against literal expected arrays.

`rust-device/src/lib.rs` is the real `fe2o3-device` source fixture. It imports
`external_scale_bias_v1`, exports `rust_accumulate_v1`, and defines the Rust
kernel. `hip/bidirectional.hip` defines the imported device function and a HIP
kernel that imports the Rust function. `device_ffi.h` fixes the external ABI.

`rust-device/link-surrogate.amdgpu.ll` is a deterministic surrogate for the
Rust compiler output so the checked-in fixture can test LLVM linking before the
production compiler handoff is public. It is not compiler-derived evidence.
The positive check requires target `gfx942:sramecc+:xnack-`, code-object version
5, exact device-function and protected kernel symbols, both `.kd` symbols, a
closed linked module, LLVM verification, and AMDGPU object generation.

## Adversarial corpus

The `adversarial` directory pins one rejection reason per source:

| Source | Expected result |
| --- | --- |
| `missing_definition.hip` | linked IR retains unresolved `external_scale_bias_v1` |
| `duplicate_definition.hip` | LLVM rejects a second strong definition |
| `wrong_role_definition.hip` | matching name is an `amdgpu_kernel`, not a device function |
| `abi_mismatched_definition.hip` | matching name has `(u64,u32)->u64` ABI |
| `wrong_target_definition.hip` | valid ABI is compiled for `gfx90a`, not `gfx942` |

These cases are intentionally independent of production crate APIs. A mature
artifact validator should reject missing definitions, wrong roles, ABI changes,
and target changes before code generation; LLVM itself rejects duplicates.

## Running

Run every local check:

```text
tests/fixtures/device-link/run.sh all
```

Individual modes are `oracle`, `positive`, and `adversarial`. Tool overrides
are `FE2O3_FIXTURE_HIPCC`, `FE2O3_FIXTURE_CLANG`, `FE2O3_FIXTURE_LLVM_DIS`,
`FE2O3_FIXTURE_LLVM_LINK`, `FE2O3_FIXTURE_LLC`, `FE2O3_FIXTURE_OPT`,
`FE2O3_FIXTURE_LLVM_READELF`, and `FE2O3_FIXTURE_CXX`. The LLVM tools must be
compatible with the LLVM bitcode emitted by HIP Clang.

The runner invokes no COMGR and performs no GPU load or launch. Passing results
establish source, LLVM closure, ABI-symbol, role, target, verifier, object, and
CPU-oracle evidence only. They grant no load or launch authority.
