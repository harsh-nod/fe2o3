use super::*;
use ProductionPendingScopedSourceOwnerV29 as Pending;
use ProductionScopedTileObservationErrorV29 as ObservationError;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn fixture(budget: &mut ArgumentBudgetV1<'_>) -> Option<Pending> {
    let (owner, capture) = pending_source(SourceCase::Repeated, false, None, budget);
    assert_eq!(capture, 0);
    Some(owner)
}

#[test]
fn observation_retains_source_and_only_refunds_its_owners_on_all_callback_outcomes() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        assert!(work.charge_work(SCHEDULE_LIMIT + 9).is_err());
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
            Pending::with_scalar_candidate_observation_v29(
                &mut donor,
                &mut budget,
                |view, budget| {
                    visits += 1;
                    assert_eq!(view.source_semantic_sha256(), &source);
                    assert_eq!(view.pending_identity(), &pending);
                    assert_ne!(view.canonical().identity().digest(), pending.digest());
                    assert_ne!(view.schedule_identity(), &[0; 32]);
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
        assert_eq!(budget.storage(), SCHEDULE_FLOOR + 11);
        assert_eq!((budget.failed_work(), budget.failed_storage()), denied);
    }
}

#[test]
fn observation_refuses_foreign_and_underfunded_entry_without_taking_donor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut donor = fixture(&mut budget);
    let identity = *donor.as_ref().unwrap().pending_identity();
    let floor = budget.storage();
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, SCHEDULE_LIMIT);
    foreign.reserve_storage(floor).unwrap();
    let error = Pending::with_scalar_candidate_observation_v29(&mut donor, &mut foreign, |_, _| {
        panic!("foreign entry called callback");
        #[allow(unreachable_code)]
        Ok::<(), ()>(())
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ObservationError::Resource(ArgumentResourceV1::Accounting)
    ));
    assert_eq!(foreign.work(), 0);
    assert_eq!(foreign.storage(), floor);
    assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
    let paid = donor.as_ref().unwrap().adopted_storage();
    budget.release_storage(SCHEDULE_FLOOR + 1).unwrap();
    let before = budget.work();
    let error = Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |_, _| {
        panic!("underfunded entry called callback");
        #[allow(unreachable_code)]
        Ok::<(), ()>(())
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ObservationError::Resource(ArgumentResourceV1::Accounting)
    ));
    assert!(donor.is_some());
    assert_eq!(budget.work(), before);
    budget.reserve_storage(1).unwrap();
    drop(donor);
    budget.release_storage(paid).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn observation_refuses_callback_floor_theft_without_refund() {
    for panic in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let mut donor = fixture(&mut budget);
        let mut remaining = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |_, budget| {
                budget.release_storage(1).unwrap();
                remaining = budget.storage();
                if panic {
                    std::panic::panic_any(19_u32);
                }
                Ok::<(), ()>(())
            })
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

#[test]
fn observation_handled_resource_probes_preserve_denial_history() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut donor = fixture(&mut budget);
    let mut denied = (None, None);
    Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |view, budget| {
        assert!(!view.grants_execution_authority());
        assert!(budget.charge_work(SCHEDULE_LIMIT).is_err());
        assert!(budget.reserve_storage(SCHEDULE_LIMIT).is_err());
        denied = (budget.failed_work(), budget.failed_storage());
        Ok::<(), ()>(())
    })
    .unwrap();
    assert!(donor.is_none());
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    assert_eq!((budget.failed_work(), budget.failed_storage()), denied);
    assert!(denied.0.is_some() && denied.1.is_some());
}

#[test]
fn observation_preparation_refusal_consumes_owner_without_callback() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let (pending, _) = pending_source(SourceCase::Empty, false, None, &mut budget);
    let mut donor = Some(pending);
    let error = Pending::with_scalar_candidate_observation_v29(&mut donor, &mut budget, |_, _| {
        panic!("empty tile source reached callback");
        #[allow(unreachable_code)]
        Ok::<(), ()>(())
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ObservationError::Unavailable {
            phase: "preparation",
            reason: "no tile occurrences"
        }
    ));
    assert!(donor.is_none());
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
}

#[path = "production_scoped_tile_observation_limits_v29_tests.rs"]
mod limits;

#[path = "production_scoped_tile_observation_fault_v29_tests.rs"]
mod faults;

#[path = "production_scoped_tile_observation_order_v29_tests.rs"]
mod orders;
