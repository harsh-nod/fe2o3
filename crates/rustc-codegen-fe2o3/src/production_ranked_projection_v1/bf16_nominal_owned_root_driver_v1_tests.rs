//! Boundary controls for the private driver. These are accounting/state tests,
//! not substitute source-owner or ordinary-dispatch qualifications.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn accounting(result: Result<()>) {
    assert!(matches!(
        result,
        Err(Error::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Accounting)
        ))
    ));
}

#[test]
fn nominal_owned_driver_frame_exact_and_one_short_are_sticky() {
    let storage = frame_bytes().unwrap();
    let work_needed = storage.checked_mul(4).unwrap();
    for mode in 0..3 {
        let work_limit = 11 + work_needed - usize::from(mode == 1);
        let storage_limit = 7 + storage - usize::from(mode == 2);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(7).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut owned = 0;
        let result = prepay_frame(&mut budget, &mut owned);
        match mode {
            0 => {
                result.unwrap();
                assert_eq!(owned, storage);
                assert_eq!(budget.work(), 11 + work_needed);
                assert_eq!(budget.storage(), 7 + storage);
                assert_eq!(budget.failed_work(), None);
                assert_eq!(budget.failed_storage(), None);
            }
            1 => {
                assert!(matches!(result, Err(Error::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Work(error))
                )) if error.actual() == 11 + work_needed && error.limit() == work_limit));
                assert_eq!(owned, 0);
                assert_eq!(budget.storage(), 7);
                assert_eq!(budget.work(), 11);
                assert_eq!(budget.failed_work(), Some(11 + work_needed));
            }
            2 => {
                assert!(matches!(result, Err(Error::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Storage(error))
                )) if error.actual() == 7 + storage && error.limit() == storage_limit));
                assert_eq!(owned, 0);
                assert_eq!(budget.storage(), 7);
                assert_eq!(budget.work(), 11 + work_needed);
                assert_eq!(budget.failed_storage(), Some(7 + storage));
            }
            _ => unreachable!(),
        }
        assert!(budget.work_ledger_identity_v1() == identity);
        if mode != 0 {
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            assert!(prepay_frame(&mut budget, &mut owned).is_err());
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                before
            );
            assert!(budget.check_prior_denials_v1().is_err());
        }
    }
}

#[test]
fn nominal_owned_driver_postflight_detects_foreign_slot_ledger_and_lost_credit() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    budget.charge_work(11).unwrap();
    let state = ReservationState::new(&budget, 0).unwrap();
    budget.reserve_storage(9).unwrap();
    budget.charge_work(13).unwrap();
    account_shape(state, &budget, 9).unwrap();
    accounting(account_shape(state, &budget, 10));
    let mut other_work = Work::new(100);
    let mut other = Budget::new(&mut other_work, 100);
    other.reserve_storage(16).unwrap();
    other.charge_work(24).unwrap();
    accounting(account_shape(state, &other, 9));
    let mut changed = state;
    changed.slot ^= 1;
    accounting(account_shape(changed, &budget, 9));
    changed = state;
    changed.ledger = other.work_ledger_identity_v1();
    accounting(account_shape(changed, &budget, 9));
    changed = state;
    changed.work = 25;
    accounting(account_shape(changed, &budget, 9));
    changed = state;
    changed.peak = 17;
    accounting(account_shape(changed, &budget, 9));
    changed = state;
    changed.owned = 10;
    accounting(account_shape(changed, &budget, 9));
    assert_eq!(budget.storage(), 16);
    assert_eq!(budget.work(), 24);
}

#[test]
fn nominal_owned_driver_only_exact_inventory_receipt_may_be_released() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    let state = ReservationState::new(&budget, 0).unwrap();
    // Symbolic inventory payload and independently accepted callback payload.
    budget.reserve_storage(13).unwrap();
    let mut owned = 0;
    PreparationResourcesV1::new(&mut budget, &mut owned)
        .reserve_storage(17)
        .unwrap();
    account_shape(state, &budget, owned).unwrap();
    budget.release_storage(13).unwrap();
    account_shape(state, &budget, owned).unwrap();
    assert_eq!(budget.storage(), 7 + 17);
    assert_eq!(owned, 17);
    budget.release_storage(1).unwrap();
    accounting(account_shape(state, &budget, owned));
}

#[test]
fn nominal_owned_driver_preserves_exact_callback_error_and_postflight_priority() {
    assert!(restore_query_error(Ok(()), None).is_ok());
    assert!(matches!(
        restore_query_error(
            Err(QueryError::Unavailable(SAVED_ERROR)),
            Some(Error::Incomplete("actual callback"))
        ),
        Err(Error::Incomplete("actual callback"))
    ));
    assert!(matches!(
        restore_query_error(
            Err(QueryError::CallbackPanicked),
            Some(Error::Incomplete("actual callback"))
        ),
        Err(Error::CanonicalAssertions(
            CanonicalAssertionErrorV1::NominalCall(QueryError::CallbackPanicked)
        ))
    ));
    assert!(matches!(
        restore_query_error(
            Err(QueryError::Resource(Resource::Accounting)),
            Some(Error::Incomplete("actual callback"))
        ),
        Err(Error::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Accounting)
        ))
    ));
    accounting(restore_query_error(
        Ok(()),
        Some(Error::Incomplete("stale saved result")),
    ));
    assert!(restore_query_error(Err(QueryError::Unavailable(SAVED_ERROR)), None).is_err());
}

#[test]
fn nominal_owned_root_cannot_promote_prefix_flags_without_actual_verified_consumer() {
    for mutation in 0..4 {
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, 1000);
        let mut owned = 0;
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut pending = PendingActualRootPrefixIndicesV1::new();
        pending.ledger = resources.original_ledger_v1();
        pending.started = mutation != 0;
        pending.completed = mutation != 1;
        pending.frame_credits = usize::from(mutation != 2);
        // All flags present (mutation 3) still reach an EMPTY actual stream,
        // which cannot be replaced by prefix completion or an observation.
        assert!(pending.into_verified_parts(&mut resources).is_err());
        assert_eq!(owned, 0);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.failed_work(), None);
        assert_eq!(budget.failed_storage(), None);
    }
}
