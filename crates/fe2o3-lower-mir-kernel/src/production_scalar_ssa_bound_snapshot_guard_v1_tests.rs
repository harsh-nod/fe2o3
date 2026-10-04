use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 29;
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

enum Fault {
    Error,
    Panic,
    Payload(Arc<AtomicUsize>),
    Undercut,
}
std::thread_local! {
    static FAULT: RefCell<Option<Fault>> = const { RefCell::new(None) };
    static DERIVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
pub(super) fn derived() {
    DERIVED.with(|count| count.set(count.get() + 1));
}
pub(super) fn after_reservation(budget: &mut Budget<'_>) -> std::result::Result<(), Resource> {
    let fault = FAULT.with(|slot| slot.take());
    match fault {
        None => Ok(()),
        Some(Fault::Error) => Err(Resource::Allocation),
        Some(Fault::Panic) => panic!("actual new-family guard derivation reservation"),
        Some(Fault::Undercut) => {
            budget.release_storage(budget.storage())?;
            Err(Resource::Accounting)
        }
        Some(Fault::Payload(count)) => {
            struct Payload(Arc<AtomicUsize>);
            impl Drop for Payload {
                fn drop(&mut self) {
                    self.0.fetch_add(1, Ordering::SeqCst);
                    panic!("guard derivation payload destructor");
                }
            }
            std::panic::panic_any(Payload(count));
        }
    }
}

fn materialize(seed: u8) -> (ProductionScalarSsaEmissionOwnerV1, SnapshotReport) {
    let (ssa, launch, _) = super::super::super::tests::bound_snapshot_tests::snapshot_source(seed);
    let source = fe2o3_mir_model::analyze_semantic_u32_induction_bound_snapshots_v1(
        ssa.source_semantic(),
        ROOT,
    )
    .unwrap();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
        ssa,
        launch,
        Default::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    (owner, source)
}
fn inherited(owner: &ProductionScalarSsaEmissionOwnerV1, report: &SnapshotReport) -> usize {
    owner.retained_analysis_storage_v1() + report.retained_storage()
}
fn run<'s>(
    owner: &'s ProductionScalarSsaEmissionOwnerV1,
    source: &SnapshotReport,
    budget: &mut Budget<'_>,
) -> Result<(ProductionU32GuardReportV1<'s>, ProductionU32GuardStorageV1)> {
    owner.analyze_u32_bound_snapshot_guard_consistency_v1(
        &[ProductionU32BoundSnapshotGuardRequestV1::new(
            ROOT, source, 0,
        )],
        Default::default(),
        budget,
    )
}
fn joined<'s>(row: &ProductionU32GuardRowV1<'s>) -> ProductionU32GuardConsistencyFactV1<'s> {
    match row.outcome() {
        ProductionU32GuardConsistencyV1::Joined(fact) => fact,
        ProductionU32GuardConsistencyV1::Unavailable(reason) => {
            panic!("exact guard join: {reason:?}")
        }
    }
}

#[test]
fn actual_rhs_statement_and_entry_remain_distinct_through_fresh_guard_report() {
    let (owner, source) = materialize(30);
    let certificate = source.certificates()[0];
    assert_ne!(certificate.guard_bound(), certificate.bound());
    let site = certificate.bound_snapshot().unwrap();
    let expected = owner
        .emission
        .capture
        .expected
        .iter()
        .find(|row| {
            row.site
                == SourceSite::Event(Site::Statement {
                    block: SsaBlockIdV1::new(site.block().block().index()),
                    statement: site.statement(),
                })
                && row.variable == SsaVariableIdV1::new(certificate.guard_bound().local().index())
        })
        .unwrap();
    assert_ne!(expected.site, SourceSite::Entry);
    let old = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
        owner.original().semantic_ssa().source_semantic(),
        ROOT,
    )
    .unwrap();
    assert!(old.certificates().is_empty());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + inherited(&owner, &source);
    budget.reserve_storage(floor).unwrap();
    DERIVED.with(|count| count.set(0));
    let (report, receipt) = run(&owner, &source, &mut budget).unwrap();
    assert_eq!(DERIVED.with(|count| count.get()), 1);
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(std::ptr::eq(report.owner(), &owner));
    assert_eq!(report.rows().len(), 1);
    let fact = joined(&report.rows()[0]);
    assert!(std::ptr::eq(fact.source(), owner.original()));
    assert!(matches!(fact.bound(), Definition::FunctionArgument { .. }));
    assert!(matches!(fact.condition(), Definition::Result { .. }));
    assert_eq!(fact.then_edge().successor, 0);
    assert_eq!(fact.else_edge().successor, 1);
    assert!(!report.authorizes_compiler_transform());
    assert!(!fact.authorizes_compiler_transform());
    assert_eq!(report.storage(), receipt);
    drop(report);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(owner);
    drop(source);
    budget.release_storage(floor - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn foreign_report_wrong_root_ordinal_and_duplicate_requests_preserve_floor() {
    let (owner, source) = materialize(30);
    let (foreign_owner, foreign) = materialize(31);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + inherited(&owner, &source) + inherited(&foreign_owner, &foreign);
    budget.reserve_storage(floor).unwrap();
    for (request, expected) in [
        (
            ProductionU32BoundSnapshotGuardRequestV1::new(ROOT, &foreign, 0),
            "guard report actual source/root identity",
        ),
        (
            ProductionU32BoundSnapshotGuardRequestV1::new(
                SemanticFunctionIdV1::from_index(99),
                &source,
                0,
            ),
            "guard report actual source/root identity",
        ),
        (
            ProductionU32BoundSnapshotGuardRequestV1::new(ROOT, &source, 1),
            "guard report certificate ordinal",
        ),
    ] {
        assert!(
            matches!(owner.analyze_u32_bound_snapshot_guard_consistency_v1(
            &[request], Default::default(), &mut budget), Err(Error::Mismatch(actual)) if actual == expected)
        );
        assert_eq!(budget.storage(), floor);
    }
    let request = ProductionU32BoundSnapshotGuardRequestV1::new(ROOT, &source, 0);
    DERIVED.with(|count| count.set(0));
    assert!(matches!(
        owner.analyze_u32_bound_snapshot_guard_consistency_v1(
            &[request, request],
            Default::default(),
            &mut budget
        ),
        Err(Error::Mismatch("duplicate guard request"))
    ));
    assert_eq!(DERIVED.with(|count| count.get()), 1);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn persistent_report_floor_is_checked_before_scratch_can_mask_one_missing_byte() {
    let (owner, source) = materialize(30);
    let required = inherited(&owner, &source);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(required - 1).unwrap();
    DERIVED.with(|count| count.set(0));
    assert!(matches!(
        run(&owner, &source, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), required - 1);
    assert_eq!(DERIVED.with(|count| count.get()), 0);
    assert!(budget.peak_storage() > required);
    budget.reserve_storage(1).unwrap();
    let (report, _) = run(&owner, &source, &mut budget).unwrap();
    joined(&report.rows()[0]);
    drop(report);
    assert_eq!(budget.storage(), required);
}

#[test]
fn complete_new_derivation_and_guard_have_exact_independent_work_and_storage_limits() {
    let (owner, source) = materialize(30);
    let floor = FLOOR + inherited(&owner, &source);
    let execute = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = run(&owner, &source, &mut budget).map(|(report, receipt)| {
            let fact = joined(&report.rows()[0]);
            (
                fact.bound(),
                fact.condition(),
                fact.then_edge(),
                fact.else_edge(),
                receipt,
            )
        });
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, used, peak) = execute(WORK, STORAGE);
    let signature = result.unwrap();
    assert_eq!(execute(used, peak).0.unwrap(), signature);
    assert_eq!(execute(WORK, STORAGE).0.unwrap(), signature);
    let work_error = execute(used - 1, peak).0.unwrap_err();
    let storage_error = execute(used, peak - 1).0.unwrap_err();
    let resource = |error: Error| match error {
        Error::Resource(error)
        | Error::Loops(CanonicalKirLoopErrorV1::Resource(error))
        | Error::Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error)) => {
            error
        }
        other => panic!("expected exact resource domain: {other:?}"),
    };
    assert!(matches!(resource(work_error), Resource::Work(_)));
    assert!(matches!(resource(storage_error), Resource::Storage(_)));
}

#[test]
fn snapshot_statement_cannot_be_relabelled_as_entry_or_substitute_a_native_value() {
    for mode in 0..2 {
        let (mut owner, source) = materialize(30);
        let variable = SsaVariableIdV1::new(source.certificates()[0].guard_bound().local().index());
        let ordinal = owner
            .emission
            .capture
            .expected
            .iter()
            .position(|row| {
                row.variable == variable
                    && matches!(row.site, SourceSite::Event(Site::Statement { .. }))
            })
            .unwrap();
        if mode == 0 {
            owner.emission.capture.expected[ordinal].site = SourceSite::Entry;
        } else {
            let row = owner
                .emission
                .capture
                .definitions
                .iter_mut()
                .find(|row| row.expected == ordinal)
                .unwrap();
            row.values[0] = ValueId(u32::MAX);
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + inherited(&owner, &source);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            run(&owner, &source, &mut budget),
            Err(Error::Mismatch(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn actual_derivation_reservation_failure_and_panic_drop_setup_before_floor_restore() {
    let (owner, source) = materialize(30);
    for panic in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + inherited(&owner, &source);
        budget.reserve_storage(floor).unwrap();
        FAULT.with(|slot| {
            assert!(
                slot.replace(Some(if panic { Fault::Panic } else { Fault::Error }))
                    .is_none()
            )
        });
        let result = run(&owner, &source, &mut budget);
        if panic {
            assert!(matches!(result, Err(Error::Panicked)));
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Allocation))));
        }
        assert!(FAULT.with(|slot| slot.borrow().is_none()));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > 0);
        assert!(budget.peak_storage() > floor);
    }
}

#[test]
fn hostile_derivation_payload_drops_only_after_restoring_valid_floor() {
    let (owner, source) = materialize(30);
    let count = Arc::new(AtomicUsize::new(0));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + inherited(&owner, &source);
    budget.reserve_storage(floor).unwrap();
    FAULT.with(|slot| assert!(slot.replace(Some(Fault::Payload(count.clone()))).is_none()));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = run(&owner, &source, &mut budget);
        }))
        .is_err()
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > 0);
}

#[test]
fn undercut_derivation_floor_is_not_fabricated_or_repaired() {
    let (owner, source) = materialize(30);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + inherited(&owner, &source))
        .unwrap();
    FAULT.with(|slot| assert!(slot.replace(Some(Fault::Undercut)).is_none()));
    assert!(matches!(
        run(&owner, &source, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 0);
    assert!(budget.work() > 0);
}

#[test]
fn new_family_direct_subset_preserves_actual_order_and_legacy_guard_coordinates() {
    let (ssa, launch, legacy) =
        super::super::super::tests::fixture::source_reports(&[(30, false), (31, true)]);
    let sources = [0, 1].map(|function| {
        fe2o3_mir_model::analyze_semantic_u32_induction_bound_snapshots_v1(
            ssa.source_semantic(),
            SemanticFunctionIdV1::from_index(function),
        )
        .unwrap()
    });
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
        ssa,
        launch,
        Default::default(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(
            owner.retained_analysis_storage_v1()
                + sources
                    .iter()
                    .map(|source| source.retained_storage())
                    .sum::<usize>(),
        )
        .unwrap();
    let floor = budget.storage();
    let new_requests = [1usize, 0].map(|i| {
        ProductionU32BoundSnapshotGuardRequestV1::new(
            SemanticFunctionIdV1::from_index(i as u32),
            &sources[i],
            0,
        )
    });
    let old_requests = [1usize, 0].map(|i| {
        ProductionU32GuardRequestV1::new(SemanticFunctionIdV1::from_index(i as u32), &legacy[i], 0)
    });
    DERIVED.with(|count| count.set(0));
    let (new, receipt) = owner
        .analyze_u32_bound_snapshot_guard_consistency_v1(
            &new_requests,
            Default::default(),
            &mut budget,
        )
        .unwrap();
    assert_eq!(DERIVED.with(|count| count.get()), 2);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let before = budget.work();
    let (old, old_receipt) = owner
        .analyze_u32_guard_consistency_v1(&old_requests, Default::default(), &mut budget)
        .unwrap();
    let old_work = budget.work() - before;
    budget
        .reserve_storage(old_receipt.retained_storage())
        .unwrap();
    let signature = |row: &ProductionU32GuardRowV1<'_>| {
        let fact = joined(row);
        (
            row.root(),
            row.function(),
            fact.bound(),
            fact.condition(),
            fact.recurrence(),
            fact.then_edge(),
            fact.else_edge(),
        )
    };
    assert_eq!(new.rows()[0].function().index(), 1);
    assert_eq!(new.rows()[1].function().index(), 0);
    assert_eq!(signature(&new.rows()[0]), signature(&old.rows()[0]));
    assert_eq!(signature(&new.rows()[1]), signature(&old.rows()[1]));
    drop(old);
    budget
        .release_storage(old_receipt.retained_storage())
        .unwrap();
    let before = budget.work();
    let (repeat, repeat_receipt) = owner
        .analyze_u32_guard_consistency_v1(&old_requests, Default::default(), &mut budget)
        .unwrap();
    assert_eq!(budget.work() - before, old_work);
    budget
        .reserve_storage(repeat_receipt.retained_storage())
        .unwrap();
    assert_eq!(signature(&new.rows()[0]), signature(&repeat.rows()[0]));
    drop(repeat);
    budget
        .release_storage(repeat_receipt.retained_storage())
        .unwrap();
    drop(new);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    DERIVED.with(|count| count.set(0));
    let (subset, _) = owner
        .analyze_u32_bound_snapshot_guard_consistency_v1(
            &new_requests[..1],
            Default::default(),
            &mut budget,
        )
        .unwrap();
    assert_eq!(DERIVED.with(|count| count.get()), 1);
    assert_eq!(subset.rows().len(), 1);
    assert_eq!(subset.rows()[0].function().index(), 1);
    assert!(!subset.authorizes_compiler_transform());
    drop(subset);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn empty_subset_and_zero_row_cap_do_not_derive_unrequested_source_reports() {
    let (owner, source) = materialize(30);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + inherited(&owner, &source);
    budget.reserve_storage(floor).unwrap();
    DERIVED.with(|count| count.set(0));
    let (empty, _) = owner
        .analyze_u32_bound_snapshot_guard_consistency_v1(&[], Default::default(), &mut budget)
        .unwrap();
    assert!(empty.rows().is_empty());
    assert!(!empty.authorizes_compiler_transform());
    drop(empty);
    assert_eq!(DERIVED.with(|count| count.get()), 0);
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    assert!(matches!(
        owner.analyze_u32_bound_snapshot_guard_consistency_v1(
            &[ProductionU32BoundSnapshotGuardRequestV1::new(
                ROOT, &source, 0
            )],
            CanonicalKirLoopLimitsV1 {
                rows: 0,
                ..Default::default()
            },
            &mut budget
        ),
        Err(Error::Limit)
    ));
    assert_eq!(budget.work(), before.0 + 1);
    assert_eq!(
        (budget.storage(), budget.peak_storage()),
        (before.1, before.2)
    );
    assert_eq!(DERIVED.with(|count| count.get()), 0);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn equal_separately_allocated_reports_require_both_receipts_but_one_source_derivation() {
    let (owner, first) = materialize(30);
    let second = fe2o3_mir_model::analyze_semantic_u32_induction_bound_snapshots_v1(
        owner.original().semantic_ssa().source_semantic(),
        ROOT,
    )
    .unwrap();
    assert_eq!(first, second);
    assert!(!std::ptr::eq(&first, &second));
    let requests = [&first, &second]
        .map(|source| ProductionU32BoundSnapshotGuardRequestV1::new(ROOT, source, 0));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let first_floor = inherited(&owner, &first);
    budget.reserve_storage(first_floor).unwrap();
    DERIVED.with(|count| count.set(0));
    assert!(matches!(
        owner.analyze_u32_bound_snapshot_guard_consistency_v1(
            &requests,
            Default::default(),
            &mut budget
        ),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), first_floor);
    assert_eq!(DERIVED.with(|count| count.get()), 0);
    budget.reserve_storage(second.retained_storage()).unwrap();
    let complete_floor = budget.storage();
    assert!(matches!(
        owner.analyze_u32_bound_snapshot_guard_consistency_v1(
            &requests,
            Default::default(),
            &mut budget
        ),
        Err(Error::Mismatch("duplicate guard request"))
    ));
    assert_eq!(DERIVED.with(|count| count.get()), 1);
    assert_eq!(budget.storage(), complete_floor);
}
