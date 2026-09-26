use super::source_storage_demands_v29::{DemandV29, RootDemandRangeV29, SourceStorageDemandsV29};
use super::source_storage_demands_v29_tests::{captured, owner};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

const FLOOR: usize = 37;
const LIMIT: usize = 4_000_000;

fn resource(error: ProductionSemanticKirErrorV1) -> ArgumentResourceV1 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error,
        other => panic!("unexpected error: {other:?}"),
    }
}

fn kind(error: ProductionSemanticKirErrorV1) -> &'static str {
    match resource(error) {
        ArgumentResourceV1::Work(_) => "work",
        ArgumentResourceV1::Storage(_) => "storage",
        other => panic!("unexpected resource error: {other:?}"),
    }
}

fn capacity(length: usize) -> usize {
    if length == 0 {
        0
    } else {
        length.max(4).next_power_of_two()
    }
}

#[test]
fn retained_bytes_are_exact_vector_capacities_and_preserve_unrelated_credit() {
    let mut owner = owner(2, true, false, false);
    let mut source_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut source_budget = ArgumentBudgetV1::new(&mut source_work, LIMIT);
    let source_credit = captured(&mut owner, &mut source_budget);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let type_count = demands.types(&owner, &mut budget).unwrap().len();
    let (rows, paths) = demands.requests(&owner, &mut budget).unwrap();
    assert!(paths.is_empty());
    let exact = capacity(type_count) * std::mem::size_of::<SemanticTypeIdV1>()
        + capacity(rows.len()) * std::mem::size_of::<DemandV29>()
        + owner.source_semantic().roots().len() * std::mem::size_of::<RootDemandRangeV29>();
    assert_eq!(budget.storage(), FLOOR + exact);
    assert!(budget.peak_storage() > budget.storage());
    budget.reserve_storage(13).unwrap();
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR + 13);
    drop(owner);
    source_budget.release_storage(source_credit).unwrap();
}

#[test]
fn exact_and_one_below_work_and_peak_storage_are_deterministic() {
    let mut owner = owner(2, true, true, false);
    let mut source_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut source_budget = ArgumentBudgetV1::new(&mut source_work, LIMIT);
    let source_credit = captured(&mut owner, &mut source_budget);
    let (exact_work, peak) = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
        let observed = (budget.work(), budget.peak_storage());
        demands.discard(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        observed
    };
    assert!(exact_work > 0 && peak > FLOOR);
    for (work_limit, storage_limit, expected) in [
        (exact_work, peak, None),
        (exact_work - 1, peak, Some("work")),
        (exact_work, peak - 1, Some("storage")),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        match SourceStorageDemandsV29::collect(&owner, &mut budget) {
            Ok(demands) => {
                assert_eq!(expected, None);
                assert_eq!((budget.work(), budget.peak_storage()), (exact_work, peak));
                demands.discard(&mut budget).unwrap();
            }
            Err(error) => assert_eq!(Some(kind(error)), expected),
        }
        assert_eq!(budget.storage(), FLOOR);
    }
    drop(owner);
    source_budget.release_storage(source_credit).unwrap();
}

#[test]
fn independently_derived_first_reservation_and_zero_work_fail_without_live_scratch() {
    let mut owner = owner(1, false, false, false);
    let type_count = owner.source_semantic().types().len();
    let mut source_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut source_budget = ArgumentBudgetV1::new(&mut source_work, LIMIT);
    let source_credit = captured(&mut owner, &mut source_budget);
    for (work_limit, storage_limit, expected, consumed) in [
        (0, LIMIT, "work", 0),
        // First allocation is the source-type u8 property array; vector
        // construction charges three work units before reserving its bytes.
        (LIMIT, FLOOR + type_count - 1, "storage", 3),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let error = SourceStorageDemandsV29::collect(&owner, &mut budget)
            .err()
            .unwrap();
        assert_eq!(kind(error), expected);
        assert_eq!(
            (budget.storage(), budget.peak_storage(), budget.work()),
            (FLOOR, FLOOR, consumed)
        );
    }
    drop(owner);
    source_budget.release_storage(source_credit).unwrap();
}

#[test]
fn equal_source_owner_foreign_ledger_and_lost_credit_cannot_rebind_demands() {
    let mut owner = owner(1, false, false, false);
    let foreign_owner = super::source_storage_demands_v29_tests::owner(1, false, false, false);
    assert_eq!(
        owner.source_semantic_sha256(),
        foreign_owner.source_semantic_sha256()
    );
    let mut source_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut source_budget = ArgumentBudgetV1::new(&mut source_work, LIMIT);
    let source_credit = captured(&mut owner, &mut source_budget);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let retained = budget.storage() - FLOOR;
    let before = (budget.work(), budget.storage());
    assert_eq!(
        resource(demands.types(&foreign_owner, &mut budget).unwrap_err()),
        ArgumentResourceV1::Accounting
    );
    assert_eq!((budget.work(), budget.storage()), before);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(FLOOR + retained).unwrap();
    assert_eq!(
        resource(demands.types(&owner, &mut foreign).unwrap_err()),
        ArgumentResourceV1::Accounting
    );
    assert_eq!((foreign.work(), foreign.storage()), (0, FLOOR + retained));
    budget.release_storage(retained).unwrap();
    assert_eq!(
        resource(demands.types(&owner, &mut budget).unwrap_err()),
        ArgumentResourceV1::Accounting
    );
    budget.reserve_storage(retained).unwrap();
    assert!(!demands.types(&owner, &mut budget).unwrap().is_empty());
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    drop(owner);
    source_budget.release_storage(source_credit).unwrap();
}
