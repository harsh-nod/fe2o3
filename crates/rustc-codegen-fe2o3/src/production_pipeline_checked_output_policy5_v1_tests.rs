//! Genuine semantic source and normal ranked projection, not collected Rust,
//! authenticated proof execution or a protected publication qualification.
use super::erased_native_handoff_tests::erased_typed_roots;
use super::*;
use crate::production_pipeline::{
    checked_output_policy5_v1::prepare_checked_output_artifacts_v1,
    erased_checked_output_policy5_v1::prepare_erased_checked_output_artifacts_v1,
    native_checked_output_handoff_v1::{NativeOutputHandoffErrorV1, check_output_inputs_v1},
};
use crate::production_ranked_projection_v1::{
    with_backend_checked_output_policy5_owned_v1, with_backend_erased_output_policy5_owned_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};

fn native(owner: &Graph, profile: ProductionAmdTargetProfileV1) -> String {
    let text = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner),
        ProductionAmdTargetProfileV1::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner),
    }.unwrap();
    dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&text).unwrap()
}
fn instruction_count(text: &str, instruction: &str) -> usize {
    text.lines()
        .filter(|line| line.contains(instruction))
        .count()
}

#[test]
fn policy5_direct_native_consumer_keeps_original_roots_and_complete_actual_output() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        with_backend_checked_output_policy5_owned_v1(profile, |owner, budget| {
            let roots = typed_roots_for_source(owner.source_semantic_kir());
            let output = *owner.output().canonical().identity();
            let floor = budget.storage();
            let (artifacts, receipt) =
                prepare_checked_output_artifacts_v1(owner, profile, &roots, None, budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                *artifacts.admitted().output().canonical().identity(),
                output
            );
            assert_eq!(
                artifacts
                    .admitted()
                    .checked_output()
                    .execution()
                    .policy_version(),
                5
            );
            assert_eq!(
                artifacts
                    .admitted()
                    .checked_output()
                    .intermediate_policy4()
                    .execution()
                    .policy_version(),
                4
            );
            assert_eq!(artifacts.descriptor_source().table().kernels().len(), 2);
            assert!(
                artifacts
                    .descriptor_source()
                    .table()
                    .producer()
                    .version()
                    .as_str()
                    .contains("policy5")
            );
            check_output_inputs_v1(artifacts.native_worker_output_v1(), profile, &roots, budget)
                .unwrap();
            assert!(!artifacts.grants_artifact_or_launch_authority());
            drop(artifacts);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn policy5_real_projector_and_native_text_observe_exact_nonidentity_s_o() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for expected in [false, true] {
            for count in [1, 2] {
                with_backend_erased_output_policy5_owned_v1(
                    expected,
                    count,
                    profile,
                    |owner, ranked, budget| {
                        let typed = erased_typed_roots(owner.erased_source());
                        let original = *owner.original_source().executable().canonical().identity();
                        let erased = *owner.erased().canonical().identity();
                        assert_ne!(original, erased);
                        assert_eq!(owner.checked_output().load_forwarding_rows().len(), count);
                        // Both test-only text buffers coexist during layout binding.
                        budget
                            .reserve_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES * 2)
                            .unwrap();
                        let before = native(
                            owner.checked_output().intermediate_policy4().owner(),
                            profile,
                        );
                        budget
                            .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
                            .unwrap();
                        assert_eq!(
                            instruction_count(&before, " = load i32, ptr addrspace(5) "),
                            count * 2
                        );
                        let floor = budget.storage();
                        let (artifacts, receipt) = prepare_erased_checked_output_artifacts_v1(
                            owner, profile, &typed, None, budget,
                        )
                        .unwrap();
                        assert_eq!(budget.storage(), floor);
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        let after = artifacts.llvm_ir();
                        assert_eq!(
                            instruction_count(after, " = load i32, ptr addrspace(5) "),
                            count
                        );
                        assert_eq!(
                            instruction_count(after, " = or i32 "),
                            instruction_count(&before, " = or i32 ") + count
                        );
                        assert_eq!(
                            instruction_count(after, "store i32"),
                            instruction_count(&before, "store i32")
                        );
                        assert_eq!(
                            *artifacts
                                .admitted()
                                .original_source()
                                .executable()
                                .canonical()
                                .identity(),
                            original
                        );
                        assert_eq!(
                            *artifacts.admitted().erased().canonical().identity(),
                            erased
                        );
                        assert_eq!(artifacts.workgroup_sizes().len(), count);
                        let table = artifacts.descriptor_source().table();
                        assert_eq!(table.kernels().len(), count);
                        assert_eq!(
                            table.producer().version().as_str(),
                            match profile {
                                ProductionAmdTargetProfileV1::Gfx942 => {
                                    "production-policy5-checked-gfx942-cov6-v1"
                                }
                                ProductionAmdTargetProfileV1::Gfx950 => {
                                    "production-policy5-checked-gfx950-cov6-v1"
                                }
                            }
                        );
                        assert!(!artifacts.grants_artifact_or_launch_authority());
                        check_output_inputs_v1(
                            artifacts.native_worker_output_v1(),
                            profile,
                            &typed,
                            budget,
                        )
                        .unwrap();
                        let live = budget.storage();
                        let proof = crate::production_native_source_lineage_v1::try_prepare_erased_native_source_lineage_v1(artifacts.admitted().erased_source(), ranked, budget);
                        assert!(matches!(proof, Err(crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1::MissingSignedRankedReceipt { .. })));
                        assert_eq!(budget.storage(), live);
                        drop(artifacts);
                        budget.release_storage(receipt.retained_storage()).unwrap();
                        drop(before);
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
fn policy5_worker_join_rejects_a_different_genuine_handoff_at_actual_o_replay() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_erased_output_policy5_owned_v1(true, 1, profile, |first, _, parent| {
        let typed = erased_typed_roots(first.erased_source());
        let (first, first_receipt) =
            prepare_erased_checked_output_artifacts_v1(first, profile, &typed, None, parent)
                .unwrap();
        parent
            .reserve_storage(first_receipt.retained_storage())
            .unwrap();
        let first_floor = parent.storage();
        with_backend_erased_output_policy5_owned_v1(
            true,
            2,
            profile,
            |second, _, second_budget| {
                let second_typed = erased_typed_roots(second.erased_source());
                let (second, second_receipt) = prepare_erased_checked_output_artifacts_v1(
                    second,
                    profile,
                    &second_typed,
                    None,
                    second_budget,
                )
                .unwrap();
                second_budget
                    .reserve_storage(second_receipt.retained_storage())
                    .unwrap();
                let mut work = Work::new(1_000_000_000);
                let mut budget = Budget::new(&mut work, 1024 * 1024 * 1024);
                let floor = first_floor + second_budget.storage();
                budget.reserve_storage(floor).unwrap();
                let good = first.native_worker_output_v1();
                check_output_inputs_v1(good, profile, &typed, &mut budget).unwrap();
                let mut changed = good;
                changed.prepared = second.native_worker_output_v1().prepared;
                assert!(matches!(
                    check_output_inputs_v1(changed, profile, &typed, &mut budget),
                    Err(NativeOutputHandoffErrorV1::Native(_))
                ));
                assert_eq!(budget.storage(), floor);
                check_output_inputs_v1(good, profile, &typed, &mut budget).unwrap();
                drop(second);
                second_budget
                    .release_storage(second_receipt.retained_storage())
                    .unwrap();
            },
        );
        drop(first);
        parent
            .release_storage(first_receipt.retained_storage())
            .unwrap();
    });
}

#[test]
fn policy5_artifact_boundary_rejects_exact_typed_root_reordering() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_erased_output_policy5_owned_v1(true, 2, profile, |owner, _, budget| {
        let mut roots = erased_typed_roots(owner.erased_source());
        roots.swap(0, 1);
        let floor = budget.storage();
        assert!(
            prepare_erased_checked_output_artifacts_v1(owner, profile, &roots, None, budget)
                .is_err()
        );
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn policy5_artifact_work_storage_and_descriptor_overlap_are_paid_at_exact_limits() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let run = |work_limit, storage_limit| {
        let mut result = None;
        with_backend_erased_output_policy5_owned_v1(true, 1, profile, |owner, _, parent| {
            let typed = erased_typed_roots(owner.erased_source());
            let floor = parent.storage();
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let output = prepare_erased_checked_output_artifacts_v1(
                owner,
                profile,
                &typed,
                None,
                &mut budget,
            );
            assert_eq!(budget.storage(), floor);
            let retained = output
                .as_ref()
                .ok()
                .map(|(_, receipt)| receipt.retained_storage());
            result = Some((
                output.is_ok(),
                budget.work(),
                budget.peak_storage(),
                floor,
                retained,
            ));
        });
        result.unwrap()
    };
    let (ok, work, peak, floor, retained) = run(1_000_000_000, 1024 * 1024 * 1024);
    assert!(ok);
    assert!(peak > floor + retained.unwrap());
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}
