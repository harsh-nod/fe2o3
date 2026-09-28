use super::layout_tests::{LINK, RECURSIVE, REFERENCE, WORD, owner, work};
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, StorageLayoutLimitsV1};

const FLOOR: usize = 29;
const MAX: usize = 32 * 1024 * 1024;

fn scalar_limits() -> StorageLayoutLimitsV1 {
    StorageLayoutLimitsV1 {
        rows: 1,
        edges: 0,
        containment_depth: 1,
        object_bytes: 8,
    }
}

#[test]
fn explicit_scalar_policy_accepts_exact_bounds_and_preserves_source_owner() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let layouts =
        SourceStorageLayoutsV29::new_with_limits(&owner, &[WORD], scalar_limits(), &mut budget)
            .unwrap();
    assert_eq!(layouts.rows(&owner, &mut budget).unwrap().len(), 1);
    assert_eq!(layouts.rows(&owner, &mut budget).unwrap()[0].size, 8);
    let foreign = super::layout_tests::owner();
    assert!(layouts.rows(&foreign, &mut budget).is_err());
    // A rejected foreign-owner query does not exchange the real source binding.
    assert_eq!(layouts.rows(&owner, &mut budget).unwrap().len(), 1);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn row_depth_and_object_policy_refusals_drop_the_source_table_before_refund() {
    for (limits, reason) in [
        (
            StorageLayoutLimitsV1 {
                rows: 0,
                ..scalar_limits()
            },
            "row policy",
        ),
        (
            StorageLayoutLimitsV1 {
                containment_depth: 0,
                ..scalar_limits()
            },
            "containment policy",
        ),
        (
            StorageLayoutLimitsV1 {
                object_bytes: 7,
                ..scalar_limits()
            },
            "object-size policy",
        ),
    ] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let error = SourceStorageLayoutsV29::new_with_limits(&owner, &[WORD], limits, &mut budget)
            .err()
            .unwrap();
        assert!(format!("{error}").contains(reason), "{error}");
        assert_eq!(budget.storage(), FLOOR);
        let recovered =
            SourceStorageLayoutsV29::new_with_limits(&owner, &[WORD], scalar_limits(), &mut budget)
                .unwrap();
        recovered.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn pointer_references_count_as_edges_but_not_recursive_containment() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let limits = StorageLayoutLimitsV1 {
        rows: 2,
        edges: 1,
        containment_depth: 1,
        object_bytes: 8,
    };
    let layouts =
        SourceStorageLayoutsV29::new_with_limits(&owner, &[REFERENCE], limits, &mut budget)
            .unwrap();
    assert_eq!(layouts.rows(&owner, &mut budget).unwrap().len(), 2);
    layouts.release(&mut budget).unwrap();
    let error = SourceStorageLayoutsV29::new_with_limits(
        &owner,
        &[REFERENCE],
        StorageLayoutLimitsV1 { edges: 0, ..limits },
        &mut budget,
    )
    .err()
    .unwrap();
    assert!(format!("{error}").contains("edge policy"));
    assert_eq!(budget.storage(), FLOOR);
    let recursive = StorageLayoutLimitsV1 {
        rows: 2,
        edges: 2,
        containment_depth: 2,
        object_bytes: 8,
    };
    let layouts =
        SourceStorageLayoutsV29::new_with_limits(&owner, &[RECURSIVE], recursive, &mut budget)
            .unwrap();
    assert_eq!(layouts.rows(&owner, &mut budget).unwrap().len(), 2);
    assert!(layouts.row_for(&owner, LINK, &mut budget).is_ok());
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn empty_source_demands_accept_a_zero_policy_without_weakening_nonempty_policy() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let zero = StorageLayoutLimitsV1 {
        rows: 0,
        edges: 0,
        containment_depth: 0,
        object_bytes: 0,
    };
    let layouts = SourceStorageLayoutsV29::new_with_limits(&owner, &[], zero, &mut budget).unwrap();
    assert!(layouts.rows(&owner, &mut budget).unwrap().is_empty());
    layouts.release(&mut budget).unwrap();
    assert!(SourceStorageLayoutsV29::new_with_limits(&owner, &[WORD], zero, &mut budget).is_err());
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn checker_first_work_denial_survives_constructor_cleanup_unchanged() {
    let owner = owner();
    // Independent phase premise: the unchanged domain constructor completes
    // before the KIR checker charges its first single work unit.
    let construction_work = {
        let mut work = work();
        let mut budget = Budget::new(&mut work, MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[WORD], &mut budget).unwrap();
        let construction_work = budget.work();
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        construction_work
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(construction_work);
    let mut budget = Budget::new(&mut work, MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let error =
        SourceStorageLayoutsV29::new_with_limits(&owner, &[WORD], scalar_limits(), &mut budget)
            .err()
            .unwrap();
    assert!(matches!(
        error,
        Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))
    ));
    assert_eq!(budget.work(), construction_work);
    assert_eq!(budget.storage(), FLOOR);
    drop(budget);
    assert_eq!(work.failed_work(), Some(construction_work + 1));
}

#[test]
fn metered_policy_validation_has_exact_and_one_below_resource_boundaries() {
    let owner = owner();
    let (exact_work, peak) = {
        let mut work = work();
        let mut budget = Budget::new(&mut work, MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let layouts =
            SourceStorageLayoutsV29::new_with_limits(&owner, &[WORD], scalar_limits(), &mut budget)
                .unwrap();
        let observed = (budget.work(), budget.peak_storage());
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        observed
    };
    for (work_limit, storage_limit, kind) in [
        (exact_work, peak, None),
        (exact_work - 1, peak, Some("work")),
        (exact_work, peak - 1, Some("storage")),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        match SourceStorageLayoutsV29::new_with_limits(
            &owner,
            &[WORD],
            scalar_limits(),
            &mut budget,
        ) {
            Ok(layouts) => {
                assert_eq!(kind, None);
                assert_eq!((budget.work(), budget.peak_storage()), (exact_work, peak));
                layouts.release(&mut budget).unwrap();
            }
            Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_))) => {
                assert_eq!(kind, Some("work"))
            }
            Err(Error::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))) => {
                assert_eq!(kind, Some("storage"))
            }
            Err(error) => panic!("unexpected error: {error:?}"),
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn module_table_transfer_settles_before_live_demand_credit_is_refunded() {
    use super::super::source_storage_demands_v29::SourceStorageDemandsV29;
    use super::super::source_storage_demands_v29_tests::{captured, owner as demand_owner};
    let mut owner = demand_owner(2, true, false, false);
    let mut work = work();
    let mut budget = Budget::new(&mut work, MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = captured(&mut owner, &mut budget);
    let floor = budget.storage();
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let demand_bytes = budget.storage() - floor;
    let types = demands.types(&owner, &mut budget).unwrap();
    let layouts = SourceStorageLayoutsV29::new_with_limits(
        &owner,
        types,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    for root in 0..2 {
        assert!(
            !demands
                .root_requests(&owner, root, &mut budget)
                .unwrap()
                .0
                .is_empty()
        );
        layouts.check_owner(&owner, &mut budget).unwrap();
    }
    let mut module = Module::new("source_storage_table_transfer");
    let rows_credit = layouts
        .install_rows(&owner, &mut module, &mut budget)
        .unwrap();
    assert!(!module.storage_layouts.is_empty());
    assert_eq!(budget.storage(), floor + demand_bytes + rows_credit);
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor + rows_credit);
    drop(module);
    budget.release_storage(rows_credit).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(capture).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
