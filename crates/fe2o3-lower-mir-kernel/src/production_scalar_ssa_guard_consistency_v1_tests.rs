use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

use super::super::super::tests::fixture;
#[path = "production_scalar_ssa_guard_native_v1_tests.rs"]
mod native_tests;

pub(super) const WORK: usize = 1_000_000_000;
pub(super) const STORAGE: usize = 512 * 1024 * 1024;
pub(super) const FLOOR: usize = 29;
pub(super) const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

pub(super) fn materialize(snapshot: bool) -> (ProductionScalarSsaEmissionOwnerV1, SourceReport) {
    let (ssa, launch, report) = fixture::source_report(30, snapshot);
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
    (owner, report)
}

fn joined(
    value: ProductionU32GuardConsistencyV1<'_>,
) -> Result<ProductionU32GuardConsistencyFactV1<'_>> {
    match value {
        ProductionU32GuardConsistencyV1::Joined(fact) => Ok(fact),
        ProductionU32GuardConsistencyV1::Unavailable(_) => {
            Err(Error::Mismatch("genuine guard fixture must join"))
        }
    }
}

fn run<'s>(
    owner: &'s ProductionScalarSsaEmissionOwnerV1,
    source: &SourceReport,
    budget: &mut Budget<'_>,
) -> Result<(ProductionU32GuardReportV1<'s>, ProductionU32GuardStorageV1)> {
    owner.analyze_u32_guard_consistency_v1(
        &[ProductionU32GuardRequestV1::new(ROOT, source, 0)],
        Default::default(),
        budget,
    )
}

#[test]
fn genuine_guard_and_snapshot_join_actual_n_and_keep_request_report_external() {
    for snapshot in [false, true] {
        let (owner, source) = materialize(snapshot);
        assert_eq!(
            source.certificates()[0]
                .guard_induction_snapshot()
                .is_some(),
            snapshot
        );
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + owner.retained_analysis_storage_v1();
        budget.reserve_storage(floor).unwrap();
        let (report, receipt) = run(&owner, &source, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(std::ptr::eq(report.owner(), &owner));
        assert_eq!(report.rows().len(), 1);
        assert_eq!(receipt, report.storage());
        assert_eq!(
            receipt.retained_storage(),
            size_of::<ProductionU32GuardReportV1<'_>>()
                + report.rows.capacity() * size_of::<ProductionU32GuardRowV1<'_>>()
        );
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        // Request/report storage is not retained: every copied claim is rechecked.
        drop(source);
        let row = &report.rows()[0];
        assert_eq!(
            (row.root(), row.function(), row.certificate_ordinal()),
            (ROOT, ROOT, 0)
        );
        let fact = joined(row.outcome()).unwrap();
        assert!(std::ptr::eq(fact.source(), owner.original()));
        assert!(matches!(fact.bound(), Definition::FunctionArgument { .. }));
        assert!(matches!(fact.condition(), Definition::Result { .. }));
        assert_eq!(fact.recurrence().scalar(), ScalarType::U32);
        assert_eq!(fact.recurrence().step_bits(), 1);
        assert!(fact.recurrence().overflow().is_some());
        assert_eq!(fact.then_edge().successor, 0);
        assert_eq!(fact.else_edge().successor, 1);
        assert_eq!(fact.then_edge().source, fact.else_edge().source);
        assert_ne!(fact.body(), fact.exit());
        assert!(!fact.authorizes_compiler_transform());
        assert!(!report.authorizes_compiler_transform());
        drop(report);
        budget.release_storage(receipt.retained_storage()).unwrap();
        let retained = owner.retained_analysis_storage_v1();
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn empty_subset_is_not_complete_coverage_and_duplicate_requests_reject() {
    let (owner, source) = materialize(false);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    let (empty, receipt) = owner
        .analyze_u32_guard_consistency_v1(&[], Default::default(), &mut budget)
        .unwrap();
    assert!(empty.rows().is_empty());
    assert!(!empty.authorizes_compiler_transform());
    assert_eq!(
        receipt.retained_storage(),
        size_of::<ProductionU32GuardReportV1<'_>>()
    );
    drop(empty);
    assert_eq!(budget.storage(), floor);
    let request = ProductionU32GuardRequestV1::new(ROOT, &source, 0);
    assert!(matches!(
        owner.analyze_u32_guard_consistency_v1(
            &[request, request],
            Default::default(),
            &mut budget
        ),
        Err(Error::Mismatch("duplicate guard request"))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn genuine_multiple_root_subset_preserves_requested_order_and_never_implies_coverage() {
    let (ssa, launch, reports) = fixture::source_reports(&[(30, false), (31, false)]);
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
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    let first =
        ProductionU32GuardRequestV1::new(SemanticFunctionIdV1::from_index(0), &reports[0], 0);
    let second =
        ProductionU32GuardRequestV1::new(SemanticFunctionIdV1::from_index(1), &reports[1], 0);
    let (both, receipt) = owner
        .analyze_u32_guard_consistency_v1(&[second, first], Default::default(), &mut budget)
        .unwrap();
    assert_eq!(both.rows().len(), 2);
    assert_eq!(both.rows()[0].root().index(), 1);
    assert_eq!(both.rows()[1].root().index(), 0);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let second_fact = joined(both.rows()[0].outcome()).unwrap();
    let first_fact = joined(both.rows()[1].outcome()).unwrap();
    assert_ne!(first_fact.body().function, second_fact.body().function);
    assert_eq!(first_fact.function(), first.root);
    assert_eq!(second_fact.function(), second.root);
    let (subset, subset_receipt) = owner
        .analyze_u32_guard_consistency_v1(&[second], Default::default(), &mut budget)
        .unwrap();
    assert_eq!(subset.rows().len(), 1);
    assert_eq!(subset.rows()[0].root(), second.root);
    assert_eq!(
        joined(subset.rows()[0].outcome()).unwrap().condition(),
        second_fact.condition()
    );
    assert!(!subset.authorizes_compiler_transform());
    assert_eq!(subset.storage(), subset_receipt);
    drop(subset);
    drop(both);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn foreign_report_wrong_root_and_bad_ordinal_refuse_exactly_before_c_query() {
    let (owner, source) = materialize(false);
    let (_, _, foreign) = fixture::source_report(31, false);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    for (request, expected) in [
        (
            ProductionU32GuardRequestV1::new(ROOT, &foreign, 0),
            "guard report actual source/root identity",
        ),
        (
            ProductionU32GuardRequestV1::new(SemanticFunctionIdV1::from_index(99), &source, 0),
            "guard report actual source/root identity",
        ),
        (
            ProductionU32GuardRequestV1::new(ROOT, &source, 1),
            "guard report certificate ordinal",
        ),
    ] {
        assert!(matches!(owner.analyze_u32_guard_consistency_v1(
            &[request], Default::default(), &mut budget), Err(Error::Mismatch(actual)) if actual == expected));
        assert_eq!(budget.storage(), floor);
    }
}

fn resource(error: &Error) -> Option<Resource> {
    match error {
        Error::Resource(value)
        | Error::Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(value))
        | Error::Loops(CanonicalKirLoopErrorV1::Resource(value)) => Some(*value),
        _ => None,
    }
}

#[test]
fn guard_query_exact_work_and_peak_storage_are_independent_and_deterministic() {
    let (owner, source) = materialize(false);
    let execute = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let floor = FLOOR + owner.retained_analysis_storage_v1();
        budget.reserve_storage(floor).unwrap();
        let result = run(&owner, &source, &mut budget).and_then(|(report, receipt)| {
            let fact = joined(report.rows()[0].outcome())?;
            let signature = (
                fact.condition(),
                fact.bound(),
                fact.then_edge(),
                fact.else_edge(),
                receipt,
            );
            drop(report);
            Ok(signature)
        });
        assert_eq!(budget.storage(), floor);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, used, peak) = execute(WORK, STORAGE);
    let expected = result.unwrap();
    assert_eq!(execute(used, peak).0.unwrap(), expected);
    assert_eq!(execute(WORK, STORAGE).0.unwrap(), expected);
    assert!(matches!(
        resource(&execute(used - 1, peak).0.err().unwrap()),
        Some(Resource::Work(_))
    ));
    assert!(matches!(
        resource(&execute(used, peak - 1).0.err().unwrap()),
        Some(Resource::Storage(_))
    ));
}

#[test]
fn missing_c_receipt_refuses_without_work_and_request_limit_precedes_allocation() {
    let (owner, source) = materialize(false);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(matches!(
        run(&owner, &source, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!((budget.work(), budget.storage()), (0, FLOOR));
    budget
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        owner.analyze_u32_guard_consistency_v1(
            &[ProductionU32GuardRequestV1::new(ROOT, &source, 0)],
            CanonicalKirLoopLimitsV1 {
                rows: 0,
                ..Default::default()
            },
            &mut budget
        ),
        Err(Error::Limit)
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, floor, floor)
    );
}

#[test]
fn inherited_occurrence_receipt_must_be_reserved_before_any_new_scratch() {
    let (mut ssa, launch, source) = fixture::source_report(30, false);
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, STORAGE);
    let occurrences = ssa
        .try_capture_occurrences_with_budget_v1(&mut setup)
        .unwrap();
    setup
        .reserve_storage(occurrences.retained_storage())
        .unwrap();
    let owner = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
        ssa,
        launch,
        Default::default(),
        &mut setup,
    )
    .unwrap();
    assert!(owner.captured_occurrences.is_none());
    assert_eq!(setup.storage(), occurrences.retained_storage());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    assert!(matches!(
        run(&owner, &source, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.peak_storage(), owner.retained_analysis_storage_v1());
    budget
        .reserve_storage(occurrences.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (report, _) = run(&owner, &source, &mut budget).unwrap();
    joined(report.rows()[0].outcome()).unwrap();
    drop(report);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn complete_c_replay_still_rejects_a_stale_capture_before_publishing_guard_rows() {
    let (mut owner, source) = materialize(false);
    owner.emission.capture.definitions[0].values[0] = ValueId(u32::MAX);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        run(&owner, &source, &mut budget),
        Err(Error::Mismatch(_))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn new_report_rows_do_not_bypass_actual_cfg_control_dominance_or_owner() {
    let (owner, source) = materialize(false);
    let (foreign, _) = materialize(false);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    let (report, receipt) = run(&owner, &source, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let fact = joined(report.rows()[0].outcome()).unwrap();
    with_canonical_kir_control_flow_v1(
        owner.original().executable(),
        fact.body().function,
        Default::default(),
        &mut budget,
        |cfg, budget| -> Result<()> {
            check_control(&owner, fact, cfg, budget)?;
            let mut wrong = fact;
            wrong.body = fact.exit;
            assert!(matches!(
                check_control(&owner, wrong, cfg, budget),
                Err(Error::Mismatch("actual N guard control dominance"))
            ));
            assert!(matches!(
                check_control(&foreign, fact, cfg, budget),
                Err(Error::Mismatch("actual N guard control dominance"))
            ));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor + receipt.retained_storage());
    drop(report);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}
