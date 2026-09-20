//! Constructed source custody through actual J -> K and native K, not signed Rust.
use super::erased_native_handoff_tests::erased_typed_roots;
use super::*;
use crate::production_pipeline::{
    checked_output_policy8_v1::{
        PreparedPolicy8ArtifactsV1, prepare_direct_policy8_artifacts_v1,
        prepare_erased_policy8_artifacts_v1,
    },
    native_checked_output_handoff_v1::{NativeOutputHandoffErrorV1, check_output_inputs_v1},
};
use crate::production_ranked_projection_v1::{
    with_backend_policy8_direct_prefix_v1, with_backend_policy8_erased_prefix_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

fn check_artifacts(
    artifacts: &PreparedPolicy8ArtifactsV1,
    duplicate: bool,
    profile: ProductionAmdTargetProfileV1,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) {
    assert_eq!(
        artifacts.test_deletion_count_v1(),
        if duplicate { 4 } else { 0 }
    );
    assert_eq!(
        artifacts.test_input_v1().canonical().canonical_bytes()
            != artifacts.output().canonical().canonical_bytes(),
        duplicate
    );
    assert_eq!(artifacts.policy_version(), 8);
    assert_eq!(artifacts.prefix_execution().policy_version(), 7);
    let source = artifacts.descriptor_source();
    assert_eq!(
        source.table().producer().version().as_str(),
        match profile {
            ProductionAmdTargetProfileV1::Gfx942 => "production-policy8-checked-gfx942-cov6-v1",
            ProductionAmdTargetProfileV1::Gfx950 => "production-policy8-checked-gfx950-cov6-v1",
        }
    );
    assert_eq!(source.table().kernels().len(), 2);
    artifacts
        .verify_equivalence(profile, typed, budget)
        .unwrap();
    crate::production_pipeline::checked_output_policy8_v1::semantic::tests::exercise(
        artifacts, budget,
    );
    crate::production_pipeline::checked_output_policy8_v1::native::tests::exercise_producer_limits_v1(
        artifacts.native_worker_output_v1().prepared, profile,
    );
    budget
        .reserve_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES * 3)
        .unwrap();
    {
        let llvm = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(artifacts.output()),
        ProductionAmdTargetProfileV1::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(artifacts.output()),
    }.unwrap();
        let llvm = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&llvm).unwrap();
        let expected = crate::kernel_ir_codegen::retain_production_compiler_module_text_v1(
            artifacts.output().module(),
            llvm,
        )
        .unwrap();
        let expected =
            crate::kernel_ir_codegen::bind_compiler_descriptor_source_v1(expected, source).unwrap();
        assert_eq!(artifacts.llvm_ir(), expected.llvm_ir());
    }
    budget
        .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES * 3)
        .unwrap();
    assert!(!artifacts.grants_artifact_or_launch_authority());
}

#[test]
fn policy8_direct_genuine_source_mutation_and_noop_reemit_exact_k_both_profiles() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for duplicate in [false, true] {
            with_backend_policy8_direct_prefix_v1(profile, duplicate, |owner, ranked, budget| {
                let mut typed = typed_roots_for_source(owner.source_semantic_kir());
                for descriptor in &mut typed {
                    let logical_name = {
                        let mut roots = ranked
                            .roots()
                            .iter()
                            .filter(|r| r.export_symbol() == descriptor.entry_symbol().as_bytes());
                        let root = roots.next().unwrap();
                        assert!(roots.next().is_none());
                        assert_eq!(descriptor.kernel_binding_bytes(), *root.kernel_binding());
                        root.logical_name().to_owned()
                    };
                    descriptor.logical_name = logical_name;
                }
                let original = *owner
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .unwrap()
                    .canonical()
                    .identity();
                let floor = budget.storage();
                let (artifacts, storage) =
                    prepare_direct_policy8_artifacts_v1(owner, profile, &typed, None, budget)
                        .unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_eq!(
                    artifacts.retained_storage_floor_v1(),
                    floor + storage.retained_storage()
                );
                assert_eq!(*artifacts.original().canonical().identity(), original);
                check_artifacts(&artifacts, duplicate, profile, &typed, budget);
                crate::production_pipeline::checked_output_policy8_v1::native::tests::exercise_final_k_components(
                    &artifacts, &ranked, &typed, budget,
                );
                drop(artifacts);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn policy8_unit_local_genuine_source_mutation_and_noop_keep_original_n_both_profiles() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for duplicate in [false, true] {
            with_backend_policy8_erased_prefix_v1(profile, duplicate, |owner, ranked, budget| {
                let mut typed = erased_typed_roots(owner.erased_source());
                for descriptor in &mut typed {
                    let logical_name = {
                        let mut roots = ranked
                            .roots()
                            .iter()
                            .filter(|r| r.export_symbol() == descriptor.entry_symbol().as_bytes());
                        let logical_name = roots.next().unwrap().logical_name().to_owned();
                        assert!(roots.next().is_none());
                        logical_name
                    };
                    descriptor.logical_name = logical_name;
                }
                let original = *owner.original_source().executable().canonical().identity();
                assert_ne!(owner.erased().canonical().identity(), &original);
                let floor = budget.storage();
                let (artifacts, storage) =
                    prepare_erased_policy8_artifacts_v1(owner, profile, &typed, None, budget)
                        .unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_eq!(
                    artifacts.retained_storage_floor_v1(),
                    floor + storage.retained_storage()
                );
                assert_eq!(*artifacts.original().canonical().identity(), original);
                check_artifacts(&artifacts, duplicate, profile, &typed, budget);
                crate::production_pipeline::checked_output_policy8_v1::native::tests::exercise_final_k_components(
                    &artifacts, &ranked, &typed, budget,
                );
                drop(artifacts);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn policy8_actual_k_refuses_historical_j_native_substitution_and_wrong_target() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_policy8_direct_prefix_v1(profile, true, |owner, _, budget| {
        let typed = typed_roots_for_source(owner.source_semantic_kir());
        let (mutated, storage) =
            prepare_direct_policy8_artifacts_v1(owner, profile, &typed, None, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        with_backend_policy8_direct_prefix_v1(profile, true, |owner, _, other| {
            let other_typed = typed_roots_for_source(owner.source_semantic_kir());
            let (historical, receipt) = crate::production_pipeline::checked_output_policy7_v1::prepare_direct_policy7_artifacts_v1(owner, profile, &other_typed, None, other).unwrap();
            other.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                historical.output().canonical().canonical_bytes(),
                mutated.test_input_v1().canonical().canonical_bytes()
            );
            assert_ne!(
                historical.output().canonical().canonical_bytes(),
                mutated.output().canonical().canonical_bytes()
            );
            let mut work = Work::new(1_000_000_000);
            let mut trial = Budget::new(&mut work, 1024 * 1024 * 1024);
            let floor = budget.storage() + other.storage();
            trial.reserve_storage(floor).unwrap();
            let good = mutated.native_worker_output_v1();
            check_output_inputs_v1(good, profile, &typed, &mut trial).unwrap();
            let mut bad = good;
            bad.prepared = historical.native_worker_output_v1().prepared;
            assert!(matches!(
                check_output_inputs_v1(bad, profile, &typed, &mut trial),
                Err(NativeOutputHandoffErrorV1::Native(_))
            ));
            assert!(
                check_output_inputs_v1(
                    good,
                    ProductionAmdTargetProfileV1::Gfx950,
                    &typed,
                    &mut trial
                )
                .is_err()
            );
            assert_eq!(trial.storage(), floor);
            drop(historical);
            other.release_storage(receipt.retained_storage()).unwrap();
        });
        drop(mutated);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

#[test]
fn policy8_artifact_exact_one_short_work_and_storage_preserve_transferred_floor() {
    for erased in [false, true] {
        let run = |work_limit, storage_limit| {
            let mut observed = None;
            let mut check =
                |result: Result<(PreparedPolicy8ArtifactsV1, _), _>, budget: &Budget<'_>, floor| {
                    observed = Some((result.is_ok(), budget.work(), budget.peak_storage()));
                    drop(result);
                    assert_eq!(budget.storage(), floor);
                };
            let profile = ProductionAmdTargetProfileV1::Gfx942;
            if erased {
                with_backend_policy8_erased_prefix_v1(profile, true, |owner, _, parent| {
                    let typed = erased_typed_roots(owner.erased_source());
                    let floor = parent.storage();
                    let mut work = Work::new(work_limit);
                    let mut budget = Budget::new(&mut work, storage_limit);
                    budget.reserve_storage(floor).unwrap();
                    let result = prepare_erased_policy8_artifacts_v1(
                        owner,
                        profile,
                        &typed,
                        None,
                        &mut budget,
                    );
                    check(result, &budget, floor);
                });
            } else {
                with_backend_policy8_direct_prefix_v1(profile, true, |owner, _, parent| {
                    let typed = typed_roots_for_source(owner.source_semantic_kir());
                    let floor = parent.storage();
                    let mut work = Work::new(work_limit);
                    let mut budget = Budget::new(&mut work, storage_limit);
                    budget.reserve_storage(floor).unwrap();
                    let result = prepare_direct_policy8_artifacts_v1(
                        owner,
                        profile,
                        &typed,
                        None,
                        &mut budget,
                    );
                    check(result, &budget, floor);
                });
            }
            observed.unwrap()
        };
        let (success, work, peak) = run(1_000_000_000, 1024 * 1024 * 1024);
        assert!(success);
        assert!(run(work, peak).0);
        assert!(!run(work - 1, peak).0);
        assert!(!run(work, peak - 1).0);
    }
}

#[test]
fn policy8_noop_equal_j_k_still_refuses_policy7_producer() {
    use crate::production_pipeline::ProductionPipelineError;
    use crate::production_pipeline::checked_output_policy8_v1::{
        CheckedOutputPolicy8StageErrorV1, native::tests::check_fixed_producer_v1,
    };
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_policy8_direct_prefix_v1(profile, false, |owner, _, budget| {
        let typed = typed_roots_for_source(owner.source_semantic_kir());
        let (current, receipt) =
            prepare_direct_policy8_artifacts_v1(owner, profile, &typed, None, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(current.test_deletion_count_v1(), 0);
        with_backend_policy8_direct_prefix_v1(profile, false, |owner, _, other| {
            let (historical, old_receipt) = crate::production_pipeline::checked_output_policy7_v1::prepare_direct_policy7_artifacts_v1(owner, profile, &typed, None, other).unwrap();
            other
                .reserve_storage(old_receipt.retained_storage())
                .unwrap();
            assert_eq!(
                current.output().canonical().canonical_bytes(),
                historical.output().canonical().canonical_bytes()
            );
            let floor = budget.storage();
            check_fixed_producer_v1(current.native_worker_output_v1().prepared, profile, budget)
                .unwrap();
            assert!(matches!(
                check_fixed_producer_v1(
                    historical.native_worker_output_v1().prepared,
                    profile,
                    budget
                ),
                Err(ProductionPipelineError::CheckedOutputPolicy8Stage(
                    CheckedOutputPolicy8StageErrorV1::Execution(
                        "exact fixed Policy8 producer identity"
                    )
                ))
            ));
            assert_eq!(budget.storage(), floor);
            // The exact same guard is reached by unsigned and signed/final/input replay.
            drop(historical);
            other
                .release_storage(old_receipt.retained_storage())
                .unwrap();
        });
        drop(current);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}
