# WGIndex File Manifest

Paths are relative to the repository root. This manifest concerns only the
scoped Workgroup index batch; earlier frozen tasks and other workers' files
are excluded.

## New Files

The production/test module hooks for these children are mounted. Patch files
are the archived apply_patch-format batch that was applied; do not reapply them.

- `crates/fe2o3-amdgcn-model/src/lowering/v13/workgroup_memory_index_v2.rs`
- `crates/fe2o3-amdgcn-model/src/lowering/v13/workgroup_memory_index_v2/tests.rs`
- `crates/fe2o3-kernel-ir/src/execution_capability_v1/workgroup_memory_index_v2_tests.rs`
- `crates/fe2o3-kernel-ir/src/execution_capability_v1/workgroup_memory_index_v2_tests/fixture.rs`
- `crates/fe2o3-kir-sim/src/execution_capability_v13/workgroup_memory_index_v2.rs`
- `crates/fe2o3-kir-sim/src/execution_capability_v13/workgroup_memory_index_v2/tests.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/borrowed_workgroup_01/workgroup_index.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/workgroup_index_transport_01.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/workgroup_index_transport_01/tests.rs`
- `crates/fe2o3-mir-model/src/semantic_mir_v1/workgroup_memory_index_decode_tests.rs`
- `crates/fe2o3-mir-model/src/semantic_mir_v1/workgroup_memory_index_v1.rs`
- `crates/fe2o3-verifier/src/final_kir_output_equivalence_v1/workgroup_memory_index_v2.rs`
- `crates/fe2o3-verifier/src/final_kir_output_equivalence_v1/workgroup_memory_index_v2/tests.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/FILES.md`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/INTEGRATION.md`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/compiler_tests.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/compiler_tests/full_import.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/compiler_tests/full_import/lowering.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/compiler_tests/harness.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/compiler_tests/registered_source.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/fixture.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/01-mir.patch`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/02-kir.patch`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/03-importer.patch`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/04-lowering.patch`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/05-consumers.patch`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/06-ssa-transport.patch`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/07-kir-catalog.patch`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/index_witness_mapping_v1/patches/review_batch.rs`

## Mounted Shared-File Changes

WG hooks from the archived patches are mounted in the following files. Other
preexisting and concurrent changes in these shared dirty files were preserved.

- `crates/fe2o3-amdgcn-model/src/lowering/operational_translation_v1.rs`
- `crates/fe2o3-amdgcn-model/src/lowering/v13.rs`
- `crates/fe2o3-amdgcn-model/src/production_target_capabilities_v1.rs`
- `crates/fe2o3-kernel-analysis/src/uniformity.rs`
- `crates/fe2o3-kernel-ir/src/execution_capability_v1.rs`
- `crates/fe2o3-kernel-ir/src/verify.rs`
- `crates/fe2o3-kernel-ir/tests/execution_capability_catalog_v13.rs`
- `crates/fe2o3-kernel-ir/tests/execution_capability_catalog_v13/extended.rs`
- `crates/fe2o3-kir-sim/src/execution_capability_v13.rs`
- `crates/fe2o3-kir-sim/src/model.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/borrowed_workgroup_01.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/borrowed_workgroup_01/transport.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/semantic_execution_capability_01.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/semantic_ssa_enum_values_01.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/semantic_ssa_intrinsics_01.rs`
- `crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/semantic_ssa_transport_01.rs`
- `crates/fe2o3-mir-model/src/semantic_mir_v1.rs`
- `crates/fe2o3-mir-model/src/semantic_mir_v1/canonical_decode.rs`
- `crates/fe2o3-verifier/src/final_kir_output_equivalence_v1.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/global_bf16_matrix_v1.rs`
- `crates/rustc-codegen-fe2o3/src/collector/production_importer_v1/typed_global_carriage_v1_tests/mod.rs`

## Review Helper

`patches/review_batch.rs` is a standalone read-only exact-context applicator.
It applies all drafts in memory and checks candidates via pinned rustfmt. It
does not type-check, run tests, mount hooks, or write those shared files.
