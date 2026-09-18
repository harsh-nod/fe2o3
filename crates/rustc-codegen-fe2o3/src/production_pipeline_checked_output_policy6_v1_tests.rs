//! Genuine projector compatibility and actual-I artifact joins. These no-op
//! fixtures do not establish ordinary Rust mutation or protected qualification.
use super::erased_native_handoff_tests::erased_typed_roots;
use super::*;
use crate::production_pipeline::{
    checked_output_policy6_v1::prepare_checked_output_artifacts_v1,
    erased_checked_output_policy6_v1::prepare_erased_checked_output_artifacts_v1,
    native_checked_output_handoff_v1::{NativeOutputHandoffErrorV1, check_output_inputs_v1},
};
use crate::production_ranked_projection_v1::{
    with_backend_checked_output_policy6_owned_v1, with_backend_erased_output_policy6_owned_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};

fn independent_native(owner: &Graph, profile: ProductionAmdTargetProfileV1) -> String {
    let native = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner),
        ProductionAmdTargetProfileV1::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner),
    }.unwrap();
    dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&native).unwrap()
}

fn bind_expected_descriptor(
    native: String,
    output: &Graph,
    source: &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
    budget: &mut Budget<'_>,
) -> crate::kernel_ir_codegen::InertCompilerModuleTextV1 {
    let floor = budget.storage();
    // The existing reservation owns the expected text. Prepay transient binding
    // before its in-place growth; the descriptor itself is borrowed, not copied.
    budget
        .reserve_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
        .unwrap();
    let expected = crate::kernel_ir_codegen::retain_production_compiler_module_text_v1(
        output.module(),
        native,
    )
    .unwrap();
    let expected =
        crate::kernel_ir_codegen::bind_compiler_descriptor_source_v1(expected, source).unwrap();
    assert!(expected.llvm_ir().len() <= dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES);
    assert!(expected.llvm_ir().contains(".fe2o3.kd.v1"));
    budget
        .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    expected
}

#[test]
fn policy6_direct_native_consumer_reemits_actual_i_and_retains_original_source() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        with_backend_checked_output_policy6_owned_v1(profile, |owner, budget| {
            let roots = typed_roots_for_source(owner.source_semantic_kir());
            let original = *owner
                .source_semantic_kir()
                .canonical_kernel_ir_identity()
                .digest();
            let output = *owner.output().canonical().identity().digest();
            budget
                .reserve_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES * 2)
                .unwrap();
            let expected = independent_native(owner.output(), profile);
            budget
                .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
                .unwrap();
            let floor = budget.storage();
            let (artifacts, storage) =
                prepare_checked_output_artifacts_v1(owner, profile, &roots, None, budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(
                artifacts
                    .admitted()
                    .output()
                    .canonical()
                    .identity()
                    .digest(),
                &output
            );
            assert_eq!(
                artifacts
                    .admitted()
                    .source_semantic_kir()
                    .canonical_kernel_ir_identity()
                    .digest(),
                &original
            );
            let checked = artifacts.admitted().checked_output();
            assert_eq!(checked.execution().policy_version(), 6);
            assert_eq!(
                checked.intermediate_policy5().execution().policy_version(),
                5
            );
            assert_eq!(checked.continuation().report().passes().len(), 2);
            let expected = bind_expected_descriptor(
                expected,
                artifacts.admitted().output(),
                artifacts.descriptor_source(),
                budget,
            );
            assert_eq!(artifacts.llvm_ir(), expected.llvm_ir());
            assert_eq!(artifacts.descriptor_source().table().kernels().len(), 2);
            assert!(
                artifacts
                    .descriptor_source()
                    .table()
                    .producer()
                    .version()
                    .as_str()
                    .contains("policy6")
            );
            check_output_inputs_v1(artifacts.native_worker_output_v1(), profile, &roots, budget)
                .unwrap();
            assert!(!artifacts.grants_artifact_or_launch_authority());
            drop(artifacts);
            budget.release_storage(storage.retained_storage()).unwrap();
            drop(expected);
            budget
                .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
                .unwrap();
        });
    }
}

#[test]
fn policy6_erased_native_consumer_keeps_n_e_prefix_and_i_distinct() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for expected in [false, true] {
            for roots in [1, 2] {
                with_backend_erased_output_policy6_owned_v1(
                    expected,
                    roots,
                    profile,
                    |owner, ranked, budget| {
                        let typed = erased_typed_roots(owner.erased_source());
                        let original = *owner
                            .original_source()
                            .executable()
                            .canonical()
                            .identity()
                            .digest();
                        let erased = *owner.erased().canonical().identity().digest();
                        assert_ne!(original, erased);
                        assert_eq!(
                            owner
                                .checked_output()
                                .intermediate_policy5()
                                .load_forwarding_rows()
                                .len(),
                            roots
                        );
                        budget
                            .reserve_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES * 2)
                            .unwrap();
                        let expected = independent_native(owner.output(), profile);
                        budget
                            .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
                            .unwrap();
                        let floor = budget.storage();
                        let (artifacts, storage) = prepare_erased_checked_output_artifacts_v1(
                            owner, profile, &typed, None, budget,
                        )
                        .unwrap();
                        assert_eq!(budget.storage(), floor);
                        budget.reserve_storage(storage.retained_storage()).unwrap();
                        let expected = bind_expected_descriptor(
                            expected,
                            artifacts.admitted().output(),
                            artifacts.descriptor_source(),
                            budget,
                        );
                        assert_eq!(artifacts.llvm_ir(), expected.llvm_ir());
                        assert_eq!(
                            artifacts
                                .admitted()
                                .original_source()
                                .executable()
                                .canonical()
                                .identity()
                                .digest(),
                            &original
                        );
                        assert_eq!(
                            artifacts
                                .admitted()
                                .erased()
                                .canonical()
                                .identity()
                                .digest(),
                            &erased
                        );
                        assert_eq!(artifacts.workgroup_sizes().len(), roots);
                        assert_eq!(artifacts.descriptor_source().table().kernels().len(), roots);
                        assert_eq!(
                            artifacts
                                .descriptor_source()
                                .table()
                                .producer()
                                .version()
                                .as_str(),
                            match profile {
                                ProductionAmdTargetProfileV1::Gfx942 =>
                                    "production-policy6-checked-gfx942-cov6-v1",
                                ProductionAmdTargetProfileV1::Gfx950 =>
                                    "production-policy6-checked-gfx950-cov6-v1",
                            }
                        );
                        check_output_inputs_v1(
                            artifacts.native_worker_output_v1(),
                            profile,
                            &typed,
                            budget,
                        )
                        .unwrap();
                        let live = budget.storage();
                        let proof = crate::production_native_source_lineage_v1::try_prepare_erased_native_source_lineage_v1(
                        artifacts.admitted().erased_source(), ranked, budget);
                        assert!(matches!(proof, Err(crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1::MissingSignedRankedReceipt { .. })));
                        assert_eq!(budget.storage(), live);
                        assert!(!artifacts.grants_artifact_or_launch_authority());
                        drop(artifacts);
                        budget.release_storage(storage.retained_storage()).unwrap();
                        drop(expected);
                        budget
                            .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
                            .unwrap();
                    },
                );
            }
        }
    }
}

#[test]
fn policy6_actual_i_join_refuses_foreign_native_handoff() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_erased_output_policy6_owned_v1(true, 1, profile, |owner, _, budget| {
        let typed = erased_typed_roots(owner.erased_source());
        let (first, storage) =
            prepare_erased_checked_output_artifacts_v1(owner, profile, &typed, None, budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        with_backend_erased_output_policy6_owned_v1(true, 2, profile, |owner, _, other_budget| {
            let other_typed = erased_typed_roots(owner.erased_source());
            let (second, second_storage) = prepare_erased_checked_output_artifacts_v1(
                owner,
                profile,
                &other_typed,
                None,
                other_budget,
            )
            .unwrap();
            other_budget
                .reserve_storage(second_storage.retained_storage())
                .unwrap();
            let mut work = Work::new(1_000_000_000);
            let mut trial = Budget::new(&mut work, 1024 * 1024 * 1024);
            trial
                .reserve_storage(floor + other_budget.storage())
                .unwrap();
            let good = first.native_worker_output_v1();
            check_output_inputs_v1(good, profile, &typed, &mut trial).unwrap();
            let mut foreign = good;
            foreign.prepared = second.native_worker_output_v1().prepared;
            assert!(matches!(
                check_output_inputs_v1(foreign, profile, &typed, &mut trial),
                Err(NativeOutputHandoffErrorV1::Native(_))
            ));
            assert_eq!(trial.storage(), floor + other_budget.storage());
            check_output_inputs_v1(good, profile, &typed, &mut trial).unwrap();
            drop(second);
            other_budget
                .release_storage(second_storage.retained_storage())
                .unwrap();
        });
        drop(first);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn policy6_artifact_preparation_exact_and_one_short_work_and_storage() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let run = |work_limit, storage_limit| {
        let mut measured = None;
        with_backend_checked_output_policy6_owned_v1(profile, |owner, parent| {
            let roots = typed_roots_for_source(owner.source_semantic_kir());
            let floor = parent.storage();
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result =
                prepare_checked_output_artifacts_v1(owner, profile, &roots, None, &mut budget);
            assert_eq!(budget.storage(), floor);
            measured = Some((result.is_ok(), budget.work(), budget.peak_storage()));
            drop(result);
        });
        measured.unwrap()
    };
    let (success, work, peak) = run(1_000_000_000, 1024 * 1024 * 1024);
    assert!(success);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}
