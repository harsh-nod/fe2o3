// Genuine constructed UnitLocal source/N/E with real P8; no native authority.
use super::*;
use crate::ProductionPrivateCellPromotionContinuationErrorV1 as PromotionError;

#[path = "production_checked_output_loop_preheaders_erased_v1_tests.rs"]
mod loop_preheader_tests;

#[test]
fn private_cell_unit_local_checked_metadata_scope_preserves_reports_resources_and_borrowed_cleanup()
{
    for roots in [1, 2] {
        let (prefix, inherited) = prefix8(roots);
        let sibling = vec![0xa5_u8; 127];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget
            .reserve_storage(inherited + sibling.capacity())
            .unwrap();
        let (owner, added) = prefix
            .continue_private_cell_promotion_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(added.retained_storage()).unwrap();
        owner.exercise_checked_promoted_sites_scope_v1(budget.storage(), &sibling);
        owner.verify_equivalence(&mut budget).unwrap();
    }
}

fn prefix8(
    roots: usize,
) -> (
    crate::ProductionOwnedUnitLocalCommutativeContinuationV1,
    usize,
) {
    let (prefix, inherited) = prefix7(roots, true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (prefix, added) = prefix
        .continue_commutative_bitwise_cse_v1(&mut budget)
        .unwrap();
    (prefix, inherited + added.retained_storage())
}

fn external_effects(module: &Module) -> Vec<fe2o3_kernel_ir::Operation> {
    module
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
        .filter(|op| match op.kind {
            OperationKind::Load { access, .. } | OperationKind::Store { access, .. } => {
                access.address_space != fe2o3_kernel_ir::AddressSpace::Private
            }
            OperationKind::Call { .. } => true,
            _ => false,
        })
        .cloned()
        .collect()
}

#[test]
fn private_cell_unit_local_keeps_actual_n_e_p8_and_all_external_effects_for_each_root() {
    let pointer = |owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12| {
        owner.canonical().canonical_bytes().as_ptr()
    };
    for roots in [1, 2] {
        let (prefix, inherited) = prefix8(roots);
        let original = pointer(prefix.prefix().prefix().original_source().executable());
        let erased = pointer(prefix.prefix().prefix().erased());
        let actual_k = pointer(prefix.output());
        let effects = external_effects(prefix.output().module());
        assert!(!effects.is_empty());
        let minimum = prefix.retained_input_storage_floor_v1().unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        let floor = inherited + 71;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let (mut owner, added) = prefix
            .continue_private_cell_promotion_v1(&mut budget)
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            owner.additional_retained_storage_v1(),
            added.retained_storage()
        );
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert_eq!(
            owner.retained_input_storage_floor_v1().unwrap(),
            minimum + added.retained_storage()
        );
        assert_eq!(
            original,
            pointer(
                owner
                    .prefix()
                    .prefix()
                    .prefix()
                    .original_source()
                    .executable()
            )
        );
        assert_eq!(erased, pointer(owner.prefix().prefix().prefix().erased()));
        assert_eq!(actual_k, pointer(owner.prefix().output()));
        assert_ne!(
            owner
                .prefix()
                .prefix()
                .prefix()
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes(),
            owner
                .prefix()
                .prefix()
                .prefix()
                .erased()
                .canonical()
                .canonical_bytes()
        );
        assert!(owner.continuation().selected_allocations().len() >= roots * 2);
        assert_ne!(
            owner.output().canonical().canonical_bytes(),
            owner.prefix().output().canonical().canonical_bytes()
        );
        assert_eq!(external_effects(owner.output().module()), effects);
        assert_eq!(owner.kernels().len(), roots);
        owner.verify_equivalence(&mut budget).unwrap();
        if roots == 2 {
            owner.exercise_private_cell_report_order_v1(&mut budget);
        }
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(!owner.grants_artifact_or_launch_authority());
        drop(owner);
        budget.release_storage(added.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn private_cell_unit_local_refuses_a_bad_prefix_before_executing_promotion() {
    let (prefix, inherited) = prefix8(2);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    prefix.exercise_private_cell_prefix_refusal_v1(&mut budget);
    assert_eq!(budget.storage(), inherited);
}

#[test]
fn private_cell_unit_local_exact_short_and_unreserved_result_limits_abort_without_fallback() {
    let run = |work_limit, storage_limit| {
        let (prefix, inherited) = prefix8(1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        let floor = inherited + 83;
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
    let (prefix, inherited) = prefix8(1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    let (owner, added) = prefix
        .continue_private_cell_promotion_v1(&mut budget)
        .unwrap();
    let minimum = owner.retained_input_storage_floor_v1().unwrap();
    let mut short_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut short = ArgumentBudgetV1::new(&mut short_work, STORAGE);
    short.reserve_storage(minimum - 1).unwrap();
    assert!(matches!(
        owner.verify_equivalence(&mut short),
        Err(PromotionError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!((short.storage(), short.work()), (minimum - 1, 0));
    budget.reserve_storage(added.retained_storage()).unwrap();
    owner.verify_equivalence(&mut budget).unwrap();
    let floor = budget.storage();
    let replay = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = owner.verify_equivalence(&mut budget);
        assert_eq!(budget.storage(), floor);
        (result.is_ok(), budget.work(), budget.peak_storage())
    };
    let (success, work, peak) = replay(WORK, STORAGE);
    assert!(success && replay(work, peak).0);
    assert!(!replay(work - 1, peak).0);
    assert!(!replay(work, peak - 1).0);
}
