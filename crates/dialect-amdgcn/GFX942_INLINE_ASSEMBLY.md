# Authenticated gfx942 Inline Assembly V1

This profile lowers `OperationKind::InlineAssembly` only after Kernel IR verification through
the gfx942 kernel or complete-module LLVM lowering entry points, including their exact
`gfx942:xnack-` variants. The backend grants no source authority: it checks the four source
identity fields structurally. Authenticating those references against the monomorphized function
and source occurrence remains the compiler frontend owner's responsibility.

The operation requires the explicit target capability
`fe2o3.amdgpu/authenticated-inline-assembly.gfx942.v1`. Mnemonic text is matched against a closed
table and replaced with the table's static LLVM template. It is never copied through as arbitrary
LLVM assembly.

## Admitted instructions

- `v_mov_b32` with one `Vgpr32` input and one `Vgpr32` output
- `s_mov_b32` with one `Sgpr32` input and one `Sgpr32` output
- `v_add_u32`, `v_sub_u32`, `v_and_b32`, `v_or_b32`, and `v_xor_b32` with two `Vgpr32` inputs and
  one `Vgpr32` output

All values must have the same Kernel IR `i32` or `u32` type. Each backend operation requires
`NoMemory` and has no declared effects. The backend also accepts `Pure`, `NoStack`, and
`PreservesFlags`; `Pure` controls whether LLVM receives `sideeffect`. Their presence is not
itself source authentication. Every output is a normal SSA result.

## Authored-source subset and simulation

The typed `fe2o3_device::amdgpu_asm!` expression frontend admits the six vector instructions
above, with `u32` inputs/results and exactly the `NoMemory` option. For example:

```rust
let sum = fe2o3_device::amdgpu_asm!(v_add_u32(left, right));
let masked = fe2o3_device::amdgpu_asm!(v_and_b32(sum, mask));
```

The normal compiler path authenticates the device marker/provider, records the actual rustc
preflight/expansion and frontend contract binding, carries each occurrence in Semantic MIR V30,
and lowers it to `InlineAssembly`, not `Binary`. Occurrence references bind the frontend unit,
enclosing function, contract, and statement. V30 excludes Execution roles/operations; historical
semantic schemas cannot consume the new carrier. These references do not claim a raw source-byte
hash or a new toolchain attestation.

Bundle V6 retains the canonical KIR V11 operations and source references. The simulator executes
the six vector integer operations with wrapping 32-bit arithmetic and exposes their SSA results.
It does not execute `s_mov_b32`. Complete-type-table validation permits this exact six-`u32`,
`NoMemory`-only subset in retained helpers' effect summaries; generic opaque assembly remains
incomplete. Source-helper readmission and independent CPU/canary checks are exercised by
[`assembly-source-roundtrip-smoke.mjs`](../../scripts/assembly-source-roundtrip-smoke.mjs).

## Observation-only LLVM inspection

An admitted pre-ranked V6 bundle can be inspected through the existing complete-module lowerer:

```sh
cargo run --locked -p fe2o3-amdgcn-model --example inspect_bundle_v6_llvm -- kernel.fe2sim > kernel-observation.ll
```

The example accepts one bounded regular-file input and only `gfx942:xnack-`. It performs existing
V6 admission and KIR V11 decoding, applies the existing `bind_production_target_v1` transform,
then calls
`lower_compiler_module_to_gfx942_xnack_minus_llvm_ir` without selecting a kernel-only projection.
The LLVM text retains internal helper definitions and calls. Comment headers identify the exact
bundle, original KIR, separate target-bound KIR, semantic MIR section, preflight receipt reference,
emitted LLVM text, and original assembly source references. They explicitly grant no compiler,
source, proof, artifact, load, or launch authority. Target binding is not semantic refinement.
Malformed bundles, target-binding errors, and unsupported lowering fail before stdout is written.

[`assembly-source-llvm-inspection-smoke.mjs`](../../scripts/assembly-source-llvm-inspection-smoke.mjs)
consumes the actual source smoke and generated-helper roundtrip captures. It checks all six
instruction templates and VGPR constraints, retained helper calls, changed LLVM text after the
explicit OR-to-AND source edit, exact identity references, and corrupted-bundle refusals. Run it
after building the example:

```sh
node scripts/assembly-source-llvm-inspection-smoke.mjs SOURCE_SMOKE_DIRECTORY SOURCE_ROUNDTRIP_DIRECTORY NEW_OUTPUT_DIRECTORY
```

This is pre-ranked LLVM-text observation, **not final machine-code inspection**. It does not
invoke `llc`, link an object, construct a production handoff, or publish an HSACO. The protected
pinned LLVM/LLD worker requires a consumed compiler-owned V3 handoff and its exact publication
receipt/closure; a diagnostic bundle or this example's text cannot supply that authority.

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

## Remaining boundaries

- Complete the production ranked/refinement and protected finalization path before claiming
  final instruction encoding, correlated machine evidence, or GPU execution for authored assembly.
- Model ordered multi-instruction regions, complete region ABI/metadata, labels, branches, and
  whole bounded assembly kernels. A sequence of independent `InlineAssembly` operations is not
  such a region; LLVM `sideeffect` is not a universal scheduling or memory barrier.
- Model fixed registers, immediates, inout/lateout operands, and explicit special-register effects.
- Add memory instructions only with address provenance, byte ranges, scopes, orderings, and Verus
  obligations that integrate with Kernel IR race and bounds analysis.
