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

## Direct-root scalar correspondence and memory effects

For the six source-admitted u32 markers, the existing ranked source resolver now
has a bounded, once-built exact callable index. A plain local call result must
have one definition, no escaped/borrowed address, the exact source caller and
u32 ABI, and a non-unwinding normal return edge that dominates its use. Operand
values are resolved at the call, before later mutations. Ordinary reaching
assignments retain precedence; helpers, ambiguous definitions and exhausted
depth/work budgets fail closed.

The corresponding lowerer relation joins each retained source call to its exact
terminator span, instruction, four source references, operands, types and sole
`NoMemory` option. Move has its input value; add/sub and bitwise instructions use
the existing u32 wrapping value grammar. This is an analysis relation: the
executable KIR stays `InlineAssembly`. Immutable checked-owner replay still
rejects substituting a distinct SSA input even if its value happens to be equal.
Checked arithmetic inside an operand is not changed to wrapping arithmetic.

Separately, formal memory extraction recognizes this same closed six-u32,
NoMemory-only profile after mandatory module verification and complete type-table
validation. It closes only the instruction's unknown-memory-effect obligation.
Surrounding loads/stores, bounds, initializedness, races and synchronization keep
their existing checks. No affine pointer-index proof, scalar-value proof, source
authentication or compiler/GPU ordering guarantee follows from this rule.
Unknown operations, i32/SGPR carriers, extra options and declared effects do not
gain this formal-memory coverage.

The [source projection tests](../rustc-codegen-fe2o3/src/production_ranked_projection_v1/gfx942_inline_value_projection_v30_tests.rs),
[lowerer checked-owner tests](../fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/gfx942_inline_scalar_correspondence_v30_tests.rs)
and [formal memory tests](../fe2o3-kernel-ir/tests/formal_memory_gfx942_inline_u32_v30.rs)
exercise these bounded contracts. Their modeled checked-owner positives are not
an actual-source reference-bound proof bridge.

## Actual source-ranked and production target observation

With the compiler extractor already built using the configured toolchain, run:

```sh
node scripts/assembly-source-ranked-smoke.mjs NEW_OUTPUT_DIRECTORY
```

The unchanged seven-occurrence source fixture has passed both fresh rustc
callbacks: mandatory source-ranked checks and the normal production target
route. Its explicit CFG bounds checks lead to plain stores, so that route selects
production KIR V8 with zero `GuardedStore` operations and seven optimizer passes.
The resulting LLVM text preserves all seven assembly calls, all six templates
and the exact VGPR constraints. It does not force V11 or take the diagnostic
Bundle V6 route below.

The `assembly-source-ranked-r3` receipt records both exit statuses as zero and
4,550 LLVM-text bytes, SHA-256
`6e48acd13b400b85206b7fcd456a9fc2c21bf481c47613a88c4749f6d34e7e9a`.
It reports compiler-closure attestation unavailable and semantic/KIR identities
not exposed by this diagnostic. Its ranked memory report alone does not show a
typed stored-value expression. The separate opt-in
`actual_source_seven_call_expression` test now captures that exact expression
from the real rustc callback using a test-only view of the existing projection.
It checks the nested u32 wrapping arithmetic/bitwise expression, unchanged source
and mandatory checks without adding a reference binding or changing the ranked
graph. Neither these observations nor the model tests above establish an actual
reference-bound functional proof, final machine
encoding, protected artifact, load/launch authority or hardware behavior.

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

This separate V6/KIR V11 path is pre-ranked LLVM-text observation, **not final machine-code inspection**. It does not
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

Ordinary Rust workgroup scratch is separately observable through the existing
V5 exporter and CPU debugger. The [LDS reproduction commands](../../docs/assembly-authoring-first-slice.md#reproduce-stopped-lds-observations-from-ordinary-source)
use `resource-query-lds-source-export.mjs` followed by `resource-query-lds-v5-smoke.mjs`.
Their actual 64-invocation reduction capture checks bytes and access pages; it
does not add authored ISA memory instructions, physical LDS placement, allocation
lifetime evidence or hardware qualification to this inline-assembly profile.

- Extend the closed direct-root relation and the observed production V8 target route through
  actual reference-bound proof qualification and protected finalization before claiming final
  instruction encoding, correlated machine evidence, or GPU execution for authored assembly.
- Model ordered multi-instruction regions, complete region ABI/metadata, labels, branches, and
  whole bounded assembly kernels. A sequence of independent `InlineAssembly` operations is not
  such a region; LLVM `sideeffect` is not a universal scheduling or memory barrier.
- Model fixed registers, immediates, inout/lateout operands, and explicit special-register effects.
- Add memory instructions only with address provenance, byte ranges, scopes, orderings, and Verus
  obligations that integrate with Kernel IR race and bounds analysis.
