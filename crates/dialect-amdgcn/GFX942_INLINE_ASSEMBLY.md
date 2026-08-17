# Authenticated gfx942 Inline Assembly V1

This profile lowers `OperationKind::InlineAssembly` only after Kernel IR verification and only
through `lower_kernel_to_gfx942_llvm_ir` or `lower_compiler_module_to_gfx942_llvm_ir`. It grants
no source authority: the four source identities must be derived from frontend records that the
compiler has already authenticated against the monomorphized function and MIR statement.

The operation requires the explicit target capability
`fe2o3.amdgpu/authenticated-inline-assembly.gfx942.v1`. Mnemonic text is matched against a closed
table and replaced with the table's static LLVM template. It is never copied through as arbitrary
LLVM assembly.

## Admitted instructions

- `v_mov_b32` with one `Vgpr32` input and one `Vgpr32` output
- `s_mov_b32` with one `Sgpr32` input and one `Sgpr32` output
- `v_add_u32`, `v_sub_u32`, `v_and_b32`, `v_or_b32`, and `v_xor_b32` with two `Vgpr32` inputs and
  one `Vgpr32` output

All values must have the same Kernel IR `i32` or `u32` type. Each operation is exactly
`NoMemory` and effect-free. `Pure` controls whether LLVM receives `sideeffect`; `NoStack` and
`PreservesFlags` remain authenticated source facts. Every output is a normal SSA result.

## Fail-closed boundary

The preflight rejects unknown instructions, memory or atomic effects, barriers, control flow,
convergent instructions, immediates, inout operands, constraint mismatches, type mismatches,
missing capability declarations, and non-gfx942 targets before returning any LLVM text. Scalar
ALU instructions such as `s_add_u32` are excluded because they modify SCC, which V1 does not model.

The generated module uses the `amdgcn-amd-amdhsa` triple and fixes `target-cpu` to `gfx942`. The
configured test `rocm_compiles_links_and_inspects_gfx942_inline_assembly` is ignored with
`requires ROCm LLVM tools with gfx942 support`; it uses `FE2O3_LLC`, `FE2O3_LLD`,
`FE2O3_LLVM_READELF`, and `FE2O3_LLVM_OBJDUMP` as a test-only code-object probe. It is not the
production finalizer and grants no compiler- or machine-correctness evidence. Production-directed
finalization instead uses pinned upstream LLVM target-machine APIs and the in-process LLD library
API, without COMGR or command-line `clang`, `llc`, or `ld.lld`.

## Remaining work

- Convert authenticated rustc MIR assembly statements into this semantic operation without
  weakening statement, function, contract, or frontend-unit identity binding.
- Model fixed registers, immediates, inout/lateout operands, and explicit special-register effects.
- Add memory instructions only with address provenance, byte ranges, scopes, orderings, and Verus
  obligations that integrate with Kernel IR race and bounds analysis.
- Load and execute the generated HSACO through the reviewed HSA adapter.
