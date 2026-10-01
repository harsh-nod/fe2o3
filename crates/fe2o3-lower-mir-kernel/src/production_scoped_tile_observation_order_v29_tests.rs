use super::*;
use ProductionScopedTileObservationOrderV29 as Order;

const ORDERS: [Order; 2] = [Order::Blocked, Order::Striped];

#[test]
fn selected_orders_preserve_callback_outcomes_and_original_account_custody() {
    for order in ORDERS {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            assert!(work.charge_work(SCHEDULE_LIMIT + 1).is_err());
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
            assert!(budget.reserve_storage(SCHEDULE_LIMIT).is_err());
            let denied = (budget.failed_work(), budget.failed_storage());
            let mut donor = fixture(&mut budget);
            let source = *donor.as_ref().unwrap().source_semantic_sha256();
            let pending = *donor.as_ref().unwrap().pending_identity();
            let ledger = budget.work_ledger_identity_v1();
            let mut visits = 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                Pending::with_scalar_candidate_observation_in_order_v29(
                    &mut donor,
                    &mut budget,
                    order,
                    |view, budget| {
                        visits += 1;
                        assert_eq!(view.source_semantic_sha256(), &source);
                        assert_eq!(view.pending_identity(), &pending);
                        assert!(!view.grants_execution_authority());
                        assert!(budget.work_ledger_identity_v1() == ledger);
                        budget.reserve_storage(11).unwrap();
                        match mode {
                            0 => Ok(()),
                            1 => Err(73_u32),
                            _ => std::panic::panic_any(91_u32),
                        }
                    },
                )
            }));
            match mode {
                0 => assert!(result.unwrap().is_ok()),
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(ObservationError::Callback(73))
                )),
                _ => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 91),
            }
            assert_eq!(visits, 1);
            assert!(donor.is_none());
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.storage(), SCHEDULE_FLOOR + 11);
            assert_eq!((budget.failed_work(), budget.failed_storage()), denied);
            budget.release_storage(11).unwrap();
            assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        }
    }
}

#[test]
fn selected_orders_refuse_foreign_or_underfunded_entry_before_take() {
    for order in ORDERS {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let mut donor = fixture(&mut budget);
        let identity = *donor.as_ref().unwrap().pending_identity();
        let adopted = donor.as_ref().unwrap().adopted_storage();
        let floor = budget.storage();
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, SCHEDULE_LIMIT);
        foreign.reserve_storage(floor).unwrap();
        let error = Pending::with_scalar_candidate_observation_in_order_v29(
            &mut donor,
            &mut foreign,
            order,
            |_, _| panic!("foreign entry reached the observation"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ObservationError::<()>::Resource(ArgumentResourceV1::Accounting)
        ));
        assert_eq!((foreign.work(), foreign.storage()), (0, floor));
        assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
        budget.release_storage(SCHEDULE_FLOOR + 1).unwrap();
        let before = (budget.work(), budget.storage());
        let error = Pending::with_scalar_candidate_observation_in_order_v29(
            &mut donor,
            &mut budget,
            order,
            |_, _| panic!("underfunded entry reached the observation"),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ObservationError::<()>::Resource(ArgumentResourceV1::Accounting)
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
        budget.reserve_storage(1).unwrap();
        drop(donor);
        budget.release_storage(adopted).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

fn callback_frame<F>(_: &F) -> usize {
    scoped_tile_observation_frame_v29::<(), F>().unwrap()
}

#[test]
fn selected_orders_prepay_the_complete_wrapper_before_taking_the_donor() {
    for order in ORDERS {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let mut donor = fixture(&mut budget);
        let adopted = donor.as_ref().unwrap().adopted_storage();
        let identity = *donor.as_ref().unwrap().pending_identity();
        let callback = |_: ProductionScopedTileCandidateViewV29<'_>,
                        _: &mut ArgumentBudgetV1<'_>| {
            panic!("one-short wrapper reached the observation");
            #[allow(unreachable_code)]
            Ok::<(), ()>(())
        };
        let frame = callback_frame(&callback);
        budget
            .reserve_storage(SCHEDULE_LIMIT - budget.storage() - (frame - 1))
            .unwrap();
        let before = (budget.work(), budget.storage());
        let error = Pending::with_scalar_candidate_observation_in_order_v29(
            &mut donor,
            &mut budget,
            order,
            callback,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ObservationError::Resource(ArgumentResourceV1::Storage(_))
        ));
        assert_eq!(budget.work(), before.0 + frame);
        assert_eq!(budget.storage(), before.1);
        assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
        drop(donor);
        budget.release_storage(adopted).unwrap();
        assert_eq!(budget.storage(), before.1 - adopted);
    }
}

#[test]
fn selected_orders_never_refund_a_stolen_callback_floor() {
    for order in ORDERS {
        for panic in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
            let mut donor = fixture(&mut budget);
            let mut remaining = 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                Pending::with_scalar_candidate_observation_in_order_v29(
                    &mut donor,
                    &mut budget,
                    order,
                    |_, budget| {
                        budget.release_storage(1).unwrap();
                        remaining = budget.storage();
                        if panic {
                            std::panic::panic_any(19_u32);
                        }
                        Ok::<(), ()>(())
                    },
                )
            }));
            if panic {
                assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 19);
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ObservationError::Resource(ArgumentResourceV1::Accounting))
                ));
            }
            assert!(donor.is_none());
            assert_eq!(budget.storage(), remaining);
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    source: [u8; 32],
    pending: [u8; 32],
    scalar: [u8; 32],
    schedule: [u8; 32],
    work: usize,
    peak: usize,
}

fn snapshot(order: Option<Order>) -> Snapshot {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut donor = fixture(&mut budget);
    assert!(
        donor
            .as_ref()
            .unwrap()
            .pending_module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::Execution(
                    fe2o3_kernel_ir::ExecutionOperationV15::MaskedTileLoadU32 {
                        lanes: 64,
                        elements: 2,
                        ..
                    }
                )
            )),
        "the inert source fixture must distinguish distributions, not E=1"
    );
    let ledger = budget.work_ledger_identity_v1();
    let paid = size_of::<Option<Snapshot>>();
    let mut result = None;
    let observe = |view: ProductionScopedTileCandidateViewV29<'_>,
                   budget: &mut ArgumentBudgetV1<'_>| {
        assert!(result.is_none());
        assert!(!view.grants_execution_authority());
        assert!(budget.work_ledger_identity_v1() == ledger);
        budget.reserve_storage(paid).unwrap();
        result = Some(Snapshot {
            source: *view.source_semantic_sha256(),
            pending: *view.pending_identity().digest(),
            scalar: *view.canonical().identity().digest(),
            schedule: *view.schedule_identity(),
            work: 0,
            peak: 0,
        });
        Ok::<(), ()>(())
    };
    match order {
        None => Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, observe),
        Some(order) => Pending::with_scalar_candidate_observation_in_order_v29(
            &mut donor,
            &mut budget,
            order,
            observe,
        ),
    }
    .unwrap();
    assert!(donor.is_none());
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR + paid);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    let mut result = result.unwrap();
    result.work = budget.work();
    result.peak = budget.peak_storage();
    result
}

#[test]
fn blocked_default_and_explicit_selection_have_identical_identity_and_accounting() {
    assert_eq!(snapshot(None), snapshot(Some(Order::Blocked)));
}

#[test]
fn distinct_orders_bind_distinct_candidates_to_the_same_e2_source() {
    let blocked = snapshot(Some(Order::Blocked));
    let striped = snapshot(Some(Order::Striped));
    assert_eq!(blocked.source, striped.source);
    assert_eq!(blocked.pending, striped.pending);
    assert_ne!(blocked.schedule, striped.schedule);
    assert_ne!(blocked.scalar, striped.scalar);
    assert_eq!(blocked, snapshot(Some(Order::Blocked)));
    assert_eq!(striped, snapshot(Some(Order::Striped)));
}
