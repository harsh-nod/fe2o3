use super::*;

const LIMIT: usize = 10_000_000;
const OTHER_ERROR: &str = "postflight test preserves a non-STOP error";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Stop,
    Other,
    Extra,
    EmissionFloor,
    PlanFloor,
    OwnedUnderflow,
    OwnedBelowPlan,
    StickyWork,
    StickyStorage,
    ForeignQuery,
    ForeignAbort,
    MovedSlot,
    Success,
    MissingClaim,
    SuccessFloor,
    RestoredFloor,
    Construction,
}

fn successful_emission(case: Case) -> bool {
    matches!(
        case,
        Case::Success
            | Case::MissingClaim
            | Case::SuccessFloor
            | Case::RestoredFloor
            | Case::StickyWork
            | Case::StickyStorage
            | Case::ForeignQuery
            | Case::ForeignAbort
    )
}

thread_local! {
    static CASE: std::cell::Cell<Case> = const { std::cell::Cell::new(Case::Stop) };
    static BEFORE: std::cell::Cell<Option<(usize, usize, usize)>> = const { std::cell::Cell::new(None) };
    static BEFORE_ABORT: std::cell::Cell<Option<(usize, usize, usize)>> = const { std::cell::Cell::new(None) };
    static ABORT_ENTERED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AFTER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FIRST: std::cell::Cell<Option<ArgumentResourceV1>> = const { std::cell::Cell::new(None) };
    static PREFIX: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, false)) };
    static PREFIX_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct PostflightGuard {
    observer: Option<SourceReferencePostflightObserverV29>,
    abort_observer: Option<fn(&SourceReferenceEmissionV29<'_, '_>, &ArgumentBudgetV1<'_>)>,
    expected_storage: Option<usize>,
    storage_checked: bool,
}
impl PostflightGuard {
    fn install(case: Case) -> Self {
        CASE.set(case);
        BEFORE.set(None);
        BEFORE_ABORT.set(None);
        ABORT_ENTERED.set(false);
        AFTER.set(false);
        FIRST.set(None);
        Self {
            observer: SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29.replace(Some(postflight)),
            abort_observer: SOURCE_REFERENCE_ABORT_OBSERVER_V29.replace(Some(before_abort)),
            expected_storage: ROOT_ERROR_EXPECTED_STORAGE_V29.replace(None),
            storage_checked: ROOT_ERROR_STORAGE_CHECKED_V29.replace(false),
        }
    }
}
impl Drop for PostflightGuard {
    fn drop(&mut self) {
        SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29.set(self.observer);
        SOURCE_REFERENCE_ABORT_OBSERVER_V29.set(self.abort_observer);
        ROOT_ERROR_EXPECTED_STORAGE_V29.set(self.expected_storage);
        ROOT_ERROR_STORAGE_CHECKED_V29.set(self.storage_checked);
    }
}

fn accounting<T>(result: &Result<T, ProductionSemanticKirErrorV1>) {
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}

fn exhaust(budget: &mut ArgumentBudgetV1<'_>) {
    budget.charge_work(LIMIT - budget.work()).unwrap();
    assert_eq!(budget.work(), LIMIT);
}

fn call_instance_reservation(instances: &ExecutionInstancesV29<'_>) -> usize {
    // Measure only the independent instance owner on a separate ledger, with
    // the production callback Result layout. No root/C1/C2 reservation is paid.
    type Product = (
        PendingScopedRootEmissionV29,
        PrivateArrayPayloadV1,
        OwnedScopedSourceSlotsV29,
        TerminalFailureOriginsV18,
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(29).unwrap();
    let mut reserved = None;
    let result: Result<Product, ProductionSemanticKirErrorV1> =
        production_call_instances_v1::with_production_call_instances_v1(
            instances.owner(),
            instances.instance(instances.root()).unwrap().function(),
            &mut budget,
            |original, budget| {
                assert_eq!(original.instances().len(), instances.instances().len());
                reserved = Some(budget.storage() - 29);
                Err(unsupported(0, None, None, STOP))
            },
        );
    assert!(is_stopped(&result));
    assert_eq!(budget.storage(), 29);
    let reserved = reserved.expect("the independent instance scope must reach its callback");
    assert!(reserved > 0);
    reserved
}

fn error_observer(
    _source: &ExecutionLifecycleSourceV29<'_>,
    _instances: &ExecutionInstancesV29<'_>,
    _emitted: &mut [Option<LoweredFunctionResultV1>],
    _receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if matches!(CASE.get(), Case::Stop | Case::Other) {
        exhaust(budget);
    }
    Err(unsupported(
        0,
        None,
        None,
        if CASE.get() == Case::Other {
            OTHER_ERROR
        } else {
            STOP
        },
    ))
}

fn before_abort(references: &SourceReferenceEmissionV29<'_, '_>, budget: &ArgumentBudgetV1<'_>) {
    assert!(!ABORT_ENTERED.replace(true));
    let (work, _, owned) = BEFORE.get().expect("genuine root owner was observed");
    assert_eq!(references.owned, owned);
    if !successful_emission(CASE.get()) {
        assert_eq!(budget.work(), work, "failed assembly must go directly to abort");
    }
    // Successful assembly still requires candidate validation after injection.
    // Measure the non-traversing abort separately from that checked work.
    assert!(BEFORE_ABORT.replace(Some((budget.work(), budget.storage(), owned))).is_none());
}

fn postflight(
    references: Option<&mut SourceReferenceEmissionV29<'_, '_>>,
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    success: bool,
) {
    let case = CASE.get();
    let Some(references) = references else {
        assert!(!success);
        let (work, storage, owned) = BEFORE_ABORT.get().expect("actual abort boundary was observed");
        assert_eq!(
            budget.work(),
            work,
            "error cleanup must not debit traversal work"
        );
        let lost = matches!(
            case,
            Case::EmissionFloor
                | Case::PlanFloor
                | Case::SuccessFloor
                | Case::RestoredFloor
                | Case::OwnedUnderflow
                | Case::OwnedBelowPlan
                | Case::ForeignAbort
                | Case::StickyWork
        );
        assert_eq!(
            budget.storage(),
            if lost { storage } else { storage - owned }
        );
        AFTER.set(true);
        return;
    };
    assert_eq!(success, successful_emission(case));
    assert!(!references.plan.loans.is_empty());
    assert!(references.floor > references.plan.retained_floor);
    assert!(references.plan.retains_custody(instances, budget));
    match case {
        Case::Stop | Case::Other => assert_eq!(budget.work(), LIMIT),
        Case::Extra => budget.reserve_storage(37).unwrap(),
        Case::EmissionFloor | Case::PlanFloor | Case::SuccessFloor | Case::RestoredFloor => {
            let target = if case == Case::PlanFloor {
                references.plan.retained_floor - 1
            } else {
                references.floor - 1
            };
            let removed = budget.storage() - target;
            budget.release_storage(removed).unwrap();
            if case == Case::RestoredFloor {
                accounting(&references.check(budget));
                budget.reserve_storage(removed).unwrap();
            }
        }
        Case::OwnedUnderflow => references.owned = references.floor + 1,
        Case::OwnedBelowPlan => {
            references.owned = references.floor - references.plan.retained_floor + 1
        }
        Case::StickyWork | Case::StickyStorage => {
            let error = if case == Case::StickyWork {
                exhaust(budget);
                references.check(budget).unwrap_err()
            } else {
                budget
                    .source_reference_reserve_v29(references.plan, LIMIT)
                    .unwrap_err()
            };
            let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = error else {
                panic!("expected an actual resource refusal");
            };
            FIRST.set(Some(first));
            assert_eq!(references.plan.failure.get(), Some(first));
            if case == Case::StickyWork {
                budget
                    .release_storage(budget.storage() - references.plan.retained_floor + 1)
                    .unwrap();
            }
        }
        Case::ForeignQuery => {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
            foreign.reserve_storage(references.floor + 29).unwrap();
            let floor = foreign.storage();
            accounting(&references.check(&mut foreign));
            assert_eq!((foreign.work(), foreign.storage()), (0, floor));
            assert_eq!(
                references.plan.failure.get(),
                Some(ArgumentResourceV1::Accounting)
            );
        }
        Case::ForeignAbort => {
            // Consume the original root-owned emission on a local foreign meter.
            // A checked replacement lets the real root error cleanup still run.
            let replacement = SourceReferenceEmissionV29::new(references.plan, budget).unwrap();
            let original = std::mem::replace(references, replacement);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
            foreign.reserve_storage(original.floor + 29).unwrap();
            let floor = foreign.storage();
            let accepted = (budget.work(), budget.storage());
            accounting(&original.abort_scope(instances, &mut foreign));
            assert_eq!((foreign.work(), foreign.storage()), (0, floor));
            assert_eq!((budget.work(), budget.storage()), accepted);
            assert_eq!(
                references.plan.failure.get(),
                Some(ArgumentResourceV1::Accounting)
            );
        }
        Case::MovedSlot => moved_slot_abort(instances),
        Case::Success => {
            assert!(references.claimed.iter().all(|claim| claim.get()));
        }
        Case::MissingClaim => {
            assert!(references.claimed[0].replace(false));
        }
        Case::Construction => construction_boundaries(instances),
    }
    BEFORE.set(Some((budget.work(), budget.storage(), references.owned)));
    if matches!(
        case,
        Case::EmissionFloor
            | Case::PlanFloor
            | Case::OwnedUnderflow
            | Case::OwnedBelowPlan
            | Case::SuccessFloor
            | Case::RestoredFloor
            | Case::ForeignAbort
            | Case::StickyWork
    ) {
        let instances = call_instance_reservation(instances);
        ROOT_ERROR_EXPECTED_STORAGE_V29.set(Some(
            budget.storage().checked_sub(instances).unwrap(),
        ));
    }
}

fn run_case(case: Case) -> Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1> {
    let _postflight = PostflightGuard::install(case);
    // Query failures must precede any selected callback error. Inject them on
    // the success path; a later cleanup failure cannot displace an earlier STOP.
    let output = if successful_emission(case) {
        run_lifecycle(
            false,
            Fault::Orchestrated {
                groups: 2,
                limits: ProductionSemanticKirLimitsV1::default(),
                fixture: ScopedFixture::RepeatedReferences,
            },
            LIMIT,
            LIMIT,
        )
        .0
    } else {
        run(
            false,
            ScopedFixture::RepeatedReferences,
            error_observer,
            LIMIT,
            LIMIT,
        )
        .0
    };
    assert!(
        BEFORE.get().is_some(),
        "{case:?} did not reach the actual root owner"
    );
    if !successful_emission(case) || ABORT_ENTERED.get() {
        assert!(AFTER.get(), "{case:?} did not reach consuming cleanup");
    }
    assert!(ROOT_ERROR_EXPECTED_STORAGE_V29.get().is_none());
    if output.is_err() {
        assert!(
            ROOT_ERROR_STORAGE_CHECKED_V29.get(),
            "{case:?} did not finish its exact outer storage assertion"
        );
    }
    output
}

#[test]
fn repeated_scalar_reference_fixture_reaches_root_postflight_without_faults() {
    run_lifecycle(
        false,
        Fault::Orchestrated {
            groups: 2,
            limits: ProductionSemanticKirLimitsV1::default(),
            fixture: ScopedFixture::RepeatedReferences,
        },
        LIMIT,
        LIMIT,
    )
    .0
    .expect("real scalar-reference fixture must complete root emission before fault injection");
}

#[test]
fn root_error_postflight_preserves_exact_work_stop_and_nonstop_errors() {
    assert!(is_stopped(&run_case(Case::Stop)));
    assert!(matches!(
        run_case(Case::Other),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: OTHER_ERROR,
            ..
        })
    ));
}

#[test]
fn root_error_postflight_refunds_only_destroyed_emission_backing() {
    assert!(is_stopped(&run_case(Case::Extra)));
}

#[test]
fn root_error_postflight_does_not_refund_lost_emission_or_plan_floor() {
    for case in [
        Case::EmissionFloor,
        Case::PlanFloor,
        Case::OwnedUnderflow,
        Case::OwnedBelowPlan,
    ] {
        assert!(
            is_stopped(&run_case(case)),
            "{case:?} preserves the earlier callback error"
        );
    }
}

#[test]
fn root_error_postflight_preserves_first_work_and_storage_failures() {
    for case in [Case::StickyWork, Case::StickyStorage] {
        let output = run_case(case);
        let first = FIRST.get().unwrap();
        assert!(matches!(
            output,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first
        ));
        assert!(matches!(
            (case, first),
            (Case::StickyWork, ArgumentResourceV1::Work(_))
                | (Case::StickyStorage, ArgumentResourceV1::Storage(_))
        ));
    }
}

#[test]
fn root_error_postflight_rejects_foreign_query_and_consuming_abort_without_debit() {
    for case in [Case::ForeignQuery, Case::ForeignAbort] {
        accounting(&run_case(case));
    }
}

fn moved_slot_abort(instances: &ExecutionInstancesV29<'_>) {
    let mut first_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut second_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut first_work, LIMIT);
    let mut other = ArgumentBudgetV1::new(&mut second_work, LIMIT);
    budget.reserve_storage(29).unwrap();
    let mut retained = 0;
    let result = with_source_reference_plan_v29(instances, &mut budget, |plan, budget| {
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        retained = emission.owned;
        let full = budget.storage();
        other.reserve_storage(full)?;
        let original = (budget.work(), budget.storage());
        std::mem::swap(budget, &mut other);
        // Same original ledger and sufficient storage, but a different concrete
        // ArgumentBudget slot. No part of either reservation may be refunded.
        accounting(&emission.abort_scope(instances, &mut other));
        assert_eq!((other.work(), other.storage()), original);
        assert_eq!((budget.work(), budget.storage()), (0, full));
        std::mem::swap(budget, &mut other);
        other.release_storage(full)?;
        Err::<(), _>(unsupported(0, None, None, STOP))
    });
    accounting(&result);
    assert!(retained > 0);
    let result_headers = 2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
    assert_eq!(
        (budget.storage(), other.storage()),
        (29 + retained + result_headers, 0)
    );
    // Only this test owns the abandoned backing and both concrete budgets.
    drop(result);
    budget.release_storage(retained + result_headers).unwrap();
    assert_eq!(budget.storage(), 29);
}

#[test]
fn consuming_abort_rejects_a_moved_original_ledger_without_refunding_it() {
    assert!(is_stopped(&run_case(Case::MovedSlot)));
}

#[test]
fn root_success_postflight_still_checks_owner_and_finishes_all_claims() {
    assert!(run_case(Case::Success).is_ok());
    assert!(matches!(
        run_case(Case::MissingClaim),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference borrow was not emitted",
            ..
        })
    ));
    accounting(&run_case(Case::SuccessFloor));
}

#[test]
fn root_postflight_custody_loss_stays_denied_after_counter_restore() {
    accounting(&run_case(Case::RestoredFloor));
}

fn construction_boundaries(instances: &ExecutionInstancesV29<'_>) {
    use std::mem::size_of;
    // Independent fixed model: existing owner query 5, prepaid abort query 5,
    // then the unchanged Self-header owner query 5. No collection query yet.
    let headers = size_of::<SourceReferenceEmissionV29<'_, '_>>()
        + 2 * size_of::<Result<SourceReferenceEmissionV29<'_, '_>, ProductionSemanticKirErrorV1>>()
        + 2 * size_of::<Result<(), ProductionSemanticKirErrorV1>>();
    for (allowance, capacity, expected_work, storage_failure) in [
        (9, headers, 5, false),
        (10, headers, 10, false),
        (15, headers, 15, false),
        (15, headers - 1, 15, true),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(29).unwrap();
        let mut reached = false;
        let output = with_source_reference_plan_v29(instances, &mut budget, |plan, budget| {
            with_canonical_call_scratch_v1(budget, |budget| {
                reached = true;
                let filler = budget.storage_limit() - budget.storage() - capacity;
                budget.reserve_storage(filler)?;
                let before = budget.storage();
                budget.charge_work(LIMIT - budget.work() - allowance)?;
                let start = budget.work();
                let error = SourceReferenceEmissionV29::new(plan, budget).err().unwrap();
                assert_resource(&error, !storage_failure);
                assert_eq!(budget.work() - start, expected_work);
                assert_eq!(
                    plan.failure.get(),
                    match error {
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) =>
                            Some(error),
                        _ => unreachable!(),
                    }
                );
                let reserved = if allowance < 15 {
                    0
                } else if storage_failure {
                    headers - 2 * size_of::<Result<(), ProductionSemanticKirErrorV1>>()
                } else {
                    headers
                };
                assert_eq!(budget.storage(), before + reserved);
                if allowance == 15 && !storage_failure {
                    assert_eq!(budget.peak_storage(), LIMIT);
                }
                Err::<(), _>(error)
            })
        });
        assert!(reached);
        assert_resource(output.as_ref().err().unwrap(), !storage_failure);
        assert_eq!(budget.storage(), 29);
    }
}

#[test]
fn emission_abort_prepayment_and_result_headers_have_independent_boundaries() {
    assert!(is_stopped(&run_case(Case::Construction)));
}

fn slot_prefix_observer(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _receipt: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let (allowance, storage) = PREFIX.get();
    let floor = budget.storage();
    let scan_work = omission_work(instances);
    PREFIX_WORK.set(scan_work);
    if storage {
        let filler = budget.storage_limit() - floor - allowance;
        budget.reserve_storage(filler)?;
        let output = derive_scoped_source_slots_v29(instances, emitted, 1024, budget);
        assert_resource(output.as_ref().err().unwrap(), false);
        assert_eq!(budget.storage(), floor + filler);
        budget.release_storage(filler)?;
    } else {
        budget.charge_work(LIMIT - budget.work() - allowance)?;
        let start = budget.work();
        let output = derive_scoped_source_slots_v29(instances, emitted, 1024, budget);
        assert_resource(output.as_ref().err().unwrap(), true);
        assert!(budget.work() - start <= allowance);
        if allowance == scan_work {
            assert_eq!(budget.work() - start, scan_work);
        }
        assert_eq!(budget.storage(), floor);
    }
    Err(unsupported(0, None, None, STOP))
}

fn omission_work(instances: &ExecutionInstancesV29<'_>) -> usize {
    fn place_work(place: &SemanticPlaceV1) -> usize {
        if matches!(
            place.projections().first().map(|row| row.kind()),
            Some(SemanticProjectionKindV1::Dereference)
        ) {
            3
        } else {
            5
        }
    }
    // Independent source-derived count, not a successful-run measurement.
    let mut work = 2; // scratch entry
    for index in 0..instances.instances().len() {
        let instance = instances.instance(instances.id_at(index).unwrap()).unwrap();
        let function = instance.declaration();
        let ssa = instance.ssa();
        work += 3
            + 3
            + 2 * function.locals().len()
            + ssa.plan().promoted_variables().len()
            + ssa.retained_cross_edge_variables().len();
        for block in function.blocks() {
            work += 2 + block.statements().len();
            for statement in block.statements() {
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if !assignment.destination().projections().is_empty() {
                            work += place_work(assignment.destination());
                        }
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Borrow { place, .. }
                            | SemanticRvalueKindV1::AddressOf { place, .. } => {
                                work += place_work(place)
                            }
                            SemanticRvalueKindV1::Load(load) => work += place_work(load.source()),
                            _ => {}
                        }
                    }
                    SemanticStatementKindV1::Store(store) => {
                        work += place_work(store.destination())
                    }
                    SemanticStatementKindV1::AtomicRmw(operation) => {
                        work += place_work(operation.address());
                        if !operation.destination().projections().is_empty() {
                            work += place_work(operation.destination());
                        }
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                        work += place_work(operation.address());
                        if !operation.destination().projections().is_empty() {
                            work += place_work(operation.destination());
                        }
                    }
                    _ => {}
                }
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && !destination.place().projections().is_empty()
            {
                work += place_work(destination.place());
            }
        }
    }
    work
}

#[test]
fn slot_omission_scan_restores_every_denied_work_prefix_without_changing_limits() {
    PREFIX.set((0, false));
    assert!(is_stopped(
        &run(
            false,
            ScopedFixture::Plain,
            slot_prefix_observer,
            LIMIT,
            LIMIT
        )
        .0
    ));
    let work = PREFIX_WORK.get();
    assert!(work > 2);
    for allowance in 1..=work {
        PREFIX.set((allowance, false));
        assert!(is_stopped(
            &run(
                false,
                ScopedFixture::Plain,
                slot_prefix_observer,
                LIMIT,
                LIMIT
            )
            .0
        ));
    }
}

#[test]
fn slot_omission_scan_restores_denied_storage_prefixes_before_full_census() {
    fn observe(
        _source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        _receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let total: usize = (0..instances.instances().len())
            .map(|index| {
                instances
                    .instance(instances.id_at(index).unwrap())
                    .unwrap()
                    .declaration()
                    .locals()
                    .len()
            })
            .sum();
        for allowance in 0..=total {
            let filler = budget.storage_limit() - floor - allowance;
            budget.reserve_storage(filler)?;
            let before = budget.work();
            let result = derive_scoped_source_slots_v29(instances, emitted, 1024, budget);
            assert_resource(result.as_ref().err().unwrap(), false);
            if allowance == total {
                assert!(budget.work() - before > omission_work(instances));
            } else {
                assert!(budget.work() - before < omission_work(instances));
            }
            assert_eq!(budget.storage(), floor + filler);
            budget.release_storage(filler)?;
        }
        assert_eq!(budget.storage(), floor);
        Err(unsupported(0, None, None, STOP))
    }
    for fixture in [
        ScopedFixture::Plain,
        ScopedFixture::Arrays,
        ScopedFixture::RepeatedSlots,
    ] {
        assert!(is_stopped(&run(false, fixture, observe, LIMIT, LIMIT).0));
    }
}

#[test]
fn slot_omission_scan_rejects_missing_or_foreign_ledger_before_any_debit() {
    fn observe(
        _source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        _receipt: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut foreign = ArgumentBudgetV1::new(&mut work, 29);
        foreign.reserve_storage(29)?;
        accounting(&derive_scoped_source_slots_v29(
            instances,
            emitted,
            1024,
            &mut foreign,
        ));
        assert_eq!((foreign.work(), foreign.storage()), (0, 29));
        let root = instances.root().index();
        let saved = emitted[root].as_mut().unwrap().lifecycle_events.take();
        let before = (budget.work(), budget.storage());
        assert!(derive_scoped_source_slots_v29(instances, emitted, 1024, budget).is_err());
        assert_eq!((budget.work(), budget.storage()), before);
        emitted[root].as_mut().unwrap().lifecycle_events = saved;
        Err(unsupported(0, None, None, STOP))
    }
    assert!(is_stopped(
        &run(false, ScopedFixture::Plain, observe, LIMIT, LIMIT).0
    ));
}

#[test]
fn omission_candidate_scratch_restores_caller_floor_on_error_and_panic() {
    fn observe(
        _source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        _emitted: &mut [Option<LoweredFunctionResultV1>],
        _receipt: &OwnedScopedSourceSlotsV29,
        _budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let row = instances.instance(instances.root()).unwrap();
        for panic in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
            budget.reserve_storage(29)?;
            let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_canonical_call_scratch_v1(&mut budget, |budget| {
                    let rows = scoped_slot_candidates_v29(row.declaration(), row.ssa(), budget)?;
                    assert_eq!(rows.len(), row.declaration().locals().len());
                    assert!(budget.storage() > 29);
                    if panic {
                        panic!("injected omission scratch panic");
                    }
                    Err::<(), _>(scoped_slot_error_v29())
                })
            }));
            assert_eq!(output.is_err(), panic);
            if let Ok(result) = output {
                assert!(result.is_err());
            }
            assert_eq!(budget.storage(), 29);
            assert!(budget.work() > 2);
        }
        Err(unsupported(0, None, None, STOP))
    }
    assert!(is_stopped(
        &run(false, ScopedFixture::Plain, observe, LIMIT, LIMIT).0
    ));
}
