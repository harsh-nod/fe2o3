// Constructed genuine source through materialization and P8, not Rustc/native proof.
use super::*;
use crate::{
    ProductionOwnedCommutativeContinuationV1 as Prefix8,
    ProductionPrivateCellPromotionContinuationErrorV1 as PromotionError,
};

#[test]
fn private_cell_direct_checked_metadata_scope_preserves_reports_resources_and_borrowed_cleanup() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = prefix8(profile, true);
        let sibling = vec![0xa5_u8; 113];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited + sibling.capacity()).unwrap();
        let (owner, added) = prefix.continue_private_cell_promotion_v1(&mut budget).unwrap();
        budget.reserve_storage(added.retained_storage()).unwrap();
        owner.exercise_checked_promoted_sites_scope_v1(budget.storage(), &sibling);
        owner.verify_equivalence(&mut budget).unwrap();
    }
}

fn prefix8(profile: Profile, trap: bool) -> (Prefix8, usize) {
    let (prefix, inherited) = prefix7(profile, true, trap);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (prefix, added) = prefix.continue_commutative_bitwise_cse_v1(&mut budget).unwrap();
    (prefix, inherited + added.retained_storage())
}

fn noop_prefix8(profile: Profile) -> (Prefix8, usize) {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| vec![block(31, vec![], SemanticTerminatorKindV1::Return)],
        |_| "private_array_relation".to_owned(),
        &[],
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa, launch, ProductionSemanticKirLimitsV1::default(), &mut budget,
    ).unwrap();
    let input = fixture6_from_source(profile, source);
    let inherited = input.floor;
    let prefix = admit6(input, WORK, STORAGE).0.unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (prefix, seven) = prefix.continue_redundant_private_stores_v1(&mut budget).unwrap();
    budget.reserve_storage(seven.retained_storage()).unwrap();
    let (prefix, eight) = prefix.continue_commutative_bitwise_cse_v1(&mut budget).unwrap();
    (prefix, inherited + seven.retained_storage() + eight.retained_storage())
}

fn trap_count(module: &Module) -> usize {
    operations(module).filter(|op| matches!(&op.kind,
        OperationKind::Call { callee, arguments } if matches!(
            fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
            Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)))).count()
}

#[test]
fn private_cell_direct_removes_real_cells_and_preserves_source_p8_and_traps_both_profiles() {
    use fe2o3_kernel_analysis::CanonicalKirPrivateCellOriginKindV1 as Origin;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for trap in [false, true] {
            let (prefix, inherited) = prefix8(profile, trap);
            let p8_bytes = prefix.output().canonical().canonical_bytes().as_ptr();
            let n_bytes = prefix.prefix().prefix().source_semantic_kir()
                .pre_ranked_executable().unwrap().canonical().canonical_bytes().as_ptr();
            let before_counts = private_counts(prefix.output().module());
            let before_traps = trap_count(prefix.output().module());
            assert!(before_counts.0 > 0 && before_counts.1 > 0);
            assert_eq!(before_traps > 0, trap);
            let minimum = prefix.retained_input_storage_floor_v1().unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            let floor = inherited + 137;
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let (owner, added) = prefix.continue_private_cell_promotion_v1(&mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(added.retained_storage(), owner.additional_retained_storage_v1());
            budget.reserve_storage(added.retained_storage()).unwrap();
            assert_eq!(owner.retained_input_storage_floor_v1().unwrap(), minimum + added.retained_storage());
            assert_eq!(p8_bytes, owner.prefix().output().canonical().canonical_bytes().as_ptr());
            assert_eq!(n_bytes, owner.prefix().prefix().prefix().source_semantic_kir()
                .pre_ranked_executable().unwrap().canonical().canonical_bytes().as_ptr());
            assert!(!owner.continuation().selected_allocations().is_empty());
            let after_counts = private_counts(owner.output().module());
            assert!(after_counts.0 < before_counts.0 && after_counts.1 < before_counts.1);
            assert_eq!(trap_count(owner.output().module()), before_traps);
            assert_eq!(owner.kernels().len(), owner.output().module().kernels.len());
            assert!(!owner.grants_artifact_or_launch_authority());
            let (before, storage) = fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(
                owner.prefix().output(), &mut budget,
            ).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (after, storage) = fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(
                owner.output(), &mut budget,
            ).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(owner.continuation().origins().len(), after.operations().len());
            for (origin, output) in owner.continuation().origins().iter().zip(after.operations()) {
                assert_eq!(origin.output, output.coordinate);
                let input = before.operations().iter().find(|op| op.coordinate == origin.input).unwrap();
                match origin.kind {
                    Origin::Retained => assert_eq!(input.operation, output.operation),
                    Origin::LoadCopy { stored_value, .. } => {
                        assert!(matches!(input.operation.kind, OperationKind::Load { .. }));
                        assert_eq!(input.operation.results, output.operation.results);
                        assert!(matches!(output.operation.kind, OperationKind::Binary {
                            op: fe2o3_kernel_ir::BinaryOp::BitOr, lhs, rhs,
                        } if lhs == stored_value && rhs == stored_value));
                    }
                }
            }
            owner.verify_equivalence(&mut budget).unwrap();
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn private_cell_direct_noop_still_owns_fresh_output_and_complete_origins_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let (prefix, inherited) = noop_prefix8(profile);
        assert_eq!(private_counts(prefix.output().module()), (0, 0, 0));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(inherited).unwrap();
        let (owner, added) = prefix.continue_private_cell_promotion_v1(&mut budget).unwrap();
        assert_eq!(budget.storage(), inherited);
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert!(owner.continuation().selected_allocations().is_empty());
        assert_eq!(owner.output().canonical().canonical_bytes(), owner.prefix().output().canonical().canonical_bytes());
        assert!(!std::ptr::eq(owner.output(), owner.prefix().output()));
        assert_eq!(owner.continuation().origins().len(), operations(owner.output().module()).count());
        owner.verify_equivalence(&mut budget).unwrap();
    }
}

#[test]
fn private_cell_load_copy_keeps_load_site_not_store_site_and_rejects_incomplete_or_foreign_maps() {
    let source = source(None, 3);
    assert!(private_counts(source.executable().module()).2 > 0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = source.retained_analysis_storage_v1() + 113;
    budget.reserve_storage(floor).unwrap();
    crate::ProductionOwnedPrivateCellPromotionContinuationV1::exercise_private_cell_load_source_transport_v1(
        source.executable(), &mut budget,
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn private_cell_direct_exact_one_short_and_zero_limits_cover_construction_and_replay() {
    let run = |work_limit, storage_limit| {
        let (prefix, inherited) = prefix8(Profile::Gfx942, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        let floor = inherited + 97;
        budget.reserve_storage(floor).unwrap();
        let result = prefix.continue_private_cell_promotion_v1(&mut budget);
        let success = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        (success, budget.work(), budget.peak_storage())
    };
    let (success, work, peak) = run(WORK, STORAGE);
    assert!(success && run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    assert!(!run(0, peak).0);
    let (prefix, inherited) = prefix8(Profile::Gfx942, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (owner, added) = prefix.continue_private_cell_promotion_v1(&mut budget).unwrap();
    let floor = inherited + added.retained_storage() + 43;
    let replay = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = owner.verify_equivalence(&mut budget);
        assert_eq!(budget.storage(), floor);
        (result.is_ok(), budget.work(), budget.peak_storage())
    };
    let (success, work, peak) = replay(WORK, STORAGE);
    assert!(success && replay(work, peak).0);
    assert!(!replay(work - 1, peak).0);
    assert!(!replay(work, peak - 1).0);
    assert!(!replay(0, peak).0);
}

#[test]
fn private_cell_direct_underpaid_prefix_fails_before_work_and_foreign_tail_cannot_replace_input() {
    let (prefix, _) = prefix8(Profile::Gfx942, false);
    let minimum = prefix.retained_input_storage_floor_v1().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(prefix.continue_private_cell_promotion_v1(&mut budget),
        Err(PromotionError::Resource(AssertOriginResourceV1::Accounting))));
    assert_eq!((budget.storage(), budget.work()), (minimum - 1, 0));

    let (prefix, inherited) = prefix8(Profile::Gfx942, false);
    let (foreign, foreign_floor) = noop_prefix8(Profile::Gfx942);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited + foreign_floor).unwrap();
    let (mut owner, added) = prefix.continue_private_cell_promotion_v1(&mut budget).unwrap();
    budget.reserve_storage(added.retained_storage()).unwrap();
    owner.exercise_private_cell_foreign_tail_refusal_v1(&foreign, &mut budget);
}
