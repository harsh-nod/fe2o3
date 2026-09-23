// Child of genuine semantic-MIR prefix fixtures. No rustc/native claim.
use super::*;
#[path = "production_checked_output_local_order_fixture_v1_tests.rs"]
mod fixture;
use crate::{
    ProductionOwnedSourceLocalOrderContinuationV1 as LocalOwner,
    ProductionSourceLocalOrderErrorV1 as LocalError,
};
use fe2o3_kernel_opt::U32LocalOrderPreferenceV1 as Preference;
use fixture::{local_order_source, local_prefix};

#[test]
fn source_local_order_both_actual_tails_replay_prefix_anchors_traps_and_fresh_reports() {
    let mut outputs = Vec::new();
    for preference in [Preference::SourceOrder, Preference::ReverseReady] {
        let (prefix, floor, mut request) = local_prefix(false, false, 64);
        request.preference = preference;
        let original_i = prefix.output().canonical().canonical_bytes().to_vec();
        let original_source = prefix.source_semantic_kir().module().clone();
        let old_required = prefix.retained_input_storage_floor_v1().unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let (owner, receipt) = prefix
            .continue_source_local_order_v1(request, &mut budget)
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(
            owner.prefix().source_semantic_kir().module(),
            &original_source
        );
        assert_eq!(
            owner.prefix().output().canonical().canonical_bytes(),
            original_i
        );
        assert_eq!(owner.request(), request);
        assert_eq!(
            owner.additional_retained_storage_v1(),
            receipt.retained_storage()
        );
        assert_eq!(
            owner.retained_input_storage_floor_v1().unwrap(),
            old_required + receipt.retained_storage()
        );
        assert!(!owner.grants_artifact_or_launch_authority());
        assert!(!owner.continuation().grants_authority());
        assert!(std::ptr::eq(owner.output(), owner.continuation().output()));
        if preference == Preference::SourceOrder {
            assert_eq!(owner.output().canonical().canonical_bytes(), original_i);
        } else {
            assert_ne!(owner.output().canonical().canonical_bytes(), original_i);
        }
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let retained = budget.storage();
        let before = budget.work();
        owner.verify_equivalence(&mut budget).unwrap();
        assert!(budget.work() > before);
        assert_eq!(budget.storage(), retained);
        assert!(budget.work_ledger_identity_v1() == ledger);
        let fresh = derive_checked_output_guarded_obligations_v1(
            owner.output(),
            ProductionSemanticKirLimitsV1::default().max_operations,
        )
        .unwrap();
        assert_eq!(owner.kernels(), fresh.as_ref());
        outputs.push(owner.output().canonical().canonical_bytes().to_vec());
    }
    assert_ne!(outputs[0], outputs[1]);
}

fn run_local(work_limit: usize, storage_limit: usize) -> (bool, usize, usize, usize) {
    let (prefix, floor, mut request) = local_prefix(false, false, 64);
    request.preference = Preference::ReverseReady;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = prefix.continue_source_local_order_v1(request, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (result.is_ok(), budget.work(), budget.peak_storage(), floor)
}

#[test]
fn source_local_order_exact_constructor_budgets_preserve_all_incoming_storage() {
    let (ok, work, peak, floor) = run_local(WORK, STORAGE);
    assert!(ok && work > 0 && peak > floor);
    assert!(run_local(work, peak).0);
    assert!(!run_local(work - 1, peak).0);
    assert!(!run_local(work, peak - 1).0);
}

#[test]
fn source_local_order_stale_noncontiguous_reordered_and_duplicate_selection_refuse() {
    for case in 0..4 {
        let (prefix, floor, mut request) = local_prefix(false, false, 64);
        match case {
            0 => {
                let other = local_order_source(true, false, 64);
                request.expected_source = *other.executable().canonical().identity();
            }
            1 => request.operations[1].operation += 1,
            2 => request.operations.swap(0, 1),
            _ => request.operations[1] = request.operations[0],
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            prefix.continue_source_local_order_v1(request, &mut budget),
            Err(LocalError::Admission(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn source_local_order_lost_prefix_operator_duplicate_formal_and_wrong_launch_refuse() {
    for (dead, duplicate, launch) in [(true, false, 64), (false, true, 64), (false, false, 128)] {
        let (prefix, floor, request) = local_prefix(dead, duplicate, launch);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            prefix.continue_source_local_order_v1(request, &mut budget),
            Err(LocalError::Admission(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn source_local_order_replay_requires_added_receipt_and_exact_bounded_work() {
    fn owner() -> (LocalOwner, usize, usize) {
        let (prefix, floor, request) = local_prefix(false, false, 64);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (owner, receipt) = prefix
            .continue_source_local_order_v1(request, &mut budget)
            .unwrap();
        (owner, floor, receipt.retained_storage())
    }
    let (owned, _, _) = owner();
    let required = owned.retained_input_storage_floor_v1().unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(required - 1).unwrap();
    assert!(matches!(
        owned.verify_equivalence(&mut budget),
        Err(LocalError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(budget.storage(), required - 1);
    let (owned, floor, added) = owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor + added).unwrap();
    owned.verify_equivalence(&mut budget).unwrap();
    let used = budget.work();
    let peak = budget.peak_storage();
    for (limit, storage, ok) in [
        (used, peak, true),
        (used - 1, peak, false),
        (used, peak - 1, false),
    ] {
        let (owned, floor, added) = owner();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
        budget.reserve_storage(floor + added).unwrap();
        assert_eq!(owned.verify_equivalence(&mut budget).is_ok(), ok);
        assert_eq!(budget.storage(), floor + added);
    }
}
