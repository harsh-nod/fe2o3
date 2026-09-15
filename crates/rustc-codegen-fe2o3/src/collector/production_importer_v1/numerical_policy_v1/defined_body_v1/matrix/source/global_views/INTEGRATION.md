# Global FP4/FP8 Constructor Checkpoint 29

Mounted by Bernoulli after exporter28g hash release. No Cargo, dependency
rebuild, SSH or network. Freeze for parent core29 after the readiness message.

## Scope

Four reviewed Global row-major constructors are now traversed Defined helpers.
Source validation runs through the existing matrix pre-type-construction hook;
canonical validation runs through its post-roster and carriage hooks. No shared
importer parent changes, MIR/KIR schema changes, numerical grant or wire/tag
allocation. BF16 and queued LDS transpose paths are untouched.

The source adapter checks the actual six-argument policy receiver/Global/u8/
ReadOnly/usize signature, original five-argument checked callee, strict policy,
full subgroup width/root/epoch and both matrix/policy references. It validates
the nine-field 40-byte view and actual pointer-niche Result layout. Original
source argument order, Copy operands, return/unwind edges and checked instance
must agree. The policy receiver remains in the original ABI; no argument-free
context or policy parameter is manufactured for the checked helper.

Canonical replay uses the existing Roster converters for types, ABIs and source
identities. It authenticates the selected root, then uses the real body converter
and live local/block/call mappings to reconstruct each constructor and every
reachable Defined checked-helper body. A single default-budget body owner is
shared for the selected replay set. Canonical entry/role/order are preserved;
original rustc indexes are never substituted for canonical indexes. Result error
branches, mapping closures and aggregate construction remain ordinary MIR.

The separate logical-value model consumes one byte per FP4 value, masks its low
nibble and packs eight values per dword; upper four dwords stay zero. FP8 keeps
all eight bits and the two depth halves. Checked/clipped coordinates zero-fill
without a memory event. This model neither emits loads nor proves memory, matrix,
convergence, target or numerical authority.

## Parent Tests

- `global_fp4_fp8_constructors_retain_bodies_and_loads_stay_rejected`
- `global_views::logical_values::tests` (eight standalone integer tests)
- `policy_global_matrix_full_import_gfx942`
- `policy_global_matrix_full_import_gfx950`
- Existing `policy_matrix_full_import_gfx942/gfx950` must remain passing.

The full-import tests use the existing real-inventory/live-plan harness, transcript
equality, repo cwd and scratch out-dir. The new registered fixture has a real
Global argument and four runtime dimensions, and retains all four Result uses.
Checks include exact constructor/checked callable counts and ABIs, original-body
mutations, generic/format/role/root/layout substitutions, swapped canonical
forwarding operands and an erased checked-helper branch. Full canonical decode
and existing matrix custody tests remain enabled.

Scratch observations before mount passed on both gfx942 and gfx950: four original
body positives, 52 body mutations, eight layout positives and 20 layout
substitutions per CPU. All eight packing/extent tests passed. Those observations
are not registered full-import or production qualification. Parent core29 and
the two new full imports are still required for this mounted adapter.

## Parent Relay

Mixed28 cleared the six PolicyGfx950MatrixIssue cases to Global FP4/FP8. Several
Max and FP8 wrapping-sub cases now reach this same constructor frontier.

Flash attention is NOT a legacy root-Matrix classification case. Snapshot source
`flash_attention_general_v1/src/kernel.rs:232-236` uses a reusable phase subgroup.
Its SubgroupBrand parent is ReusableWorkgroupBrand<Root> and its epoch is
DynamicPhaseEpoch. `rust_matrix_capability_v1` currently requires a direct
KernelCapabilityBrand parent. Lagrange/Pauli need exact reusable-phase custody,
not a classifier bypass or root-only unwrapping authority. No fix mounted here.

Materialized attention's separate mixed28 failure is the legacy unbranded
MatrixMultiplyAccumulate ABI. Global loads, policy-specific MFMA consumers and
LDS transpose issuance remain subsequent explicit frontiers.
