//! Component custody tests only; no fabricated facts can enter the real context.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::panic::{AssertUnwindSafe, catch_unwind};

const FLOOR: usize = 37;
const LIMIT: usize = 1_000_000;

#[test]
fn resource_borrow_retains_accepted_credits_and_the_same_ledger() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let state = ReservationState::new(&budget, owned).unwrap();
    let values = with_resource_borrow(&mut budget, &mut owned, state, |resources| {
        assert!(resources.original_ledger_v1() == Some((state.slot, state.ledger)));
        resources.filled(7, 11_u64)
    })
    .unwrap();
    assert_eq!(values, vec![11; 7]);
    assert_eq!(owned, 7 * size_of::<u64>());
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(values);
    // No automatic refund: only this physical owner can now release its credit.
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn captured_vector_outlives_the_resource_borrow_without_a_premature_refund() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let state = ReservationState::new(&budget, owned).unwrap();
    let mut pending = None;
    with_resource_borrow(&mut budget, &mut owned, state, |resources| {
        pending = Some(resources.filled(5, 19_u32)?);
        Ok(())
    })
    .unwrap();
    assert_eq!(pending.as_ref().unwrap(), &vec![19; 5]);
    assert_eq!(budget.storage(), FLOOR + owned);
    assert!(owned > 0);
    drop(pending);
    budget.release_storage(owned).unwrap();
}

#[test]
fn accepted_storage_survives_callback_error_and_panic_until_outer_drop() {
    for panic in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let state = ReservationState::new(&budget, owned).unwrap();
        let mut pending = None;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_resource_borrow::<()>(&mut budget, &mut owned, state, |resources| {
                pending = Some(resources.filled(9, 23_u64)?);
                if panic {
                    panic!("recipe resource component panic");
                }
                Err(Error::Incomplete("recipe resource component refusal"))
            })
        }));
        match outcome {
            Err(payload) => {
                assert!(panic);
                drop(payload);
            }
            Ok(result) => {
                assert!(!panic);
                assert!(matches!(
                    result,
                    Err(Error::Incomplete("recipe resource component refusal"))
                ));
            }
        }
        state.check(&budget, owned).unwrap();
        assert_eq!(budget.storage(), FLOOR + owned);
        assert_eq!(pending.as_ref().unwrap().len(), 9);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn swallowed_work_or_storage_denials_stay_sticky_and_refuse() {
    for storage_denial in [false, true] {
        let mut work = Work::new(if storage_denial { LIMIT } else { 0 });
        let mut budget = Budget::new(&mut work, if storage_denial { FLOOR } else { LIMIT });
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let state = ReservationState::new(&budget, owned).unwrap();
        let result = with_resource_borrow(&mut budget, &mut owned, state, |resources| {
            if storage_denial {
                assert!(resources.reserve_storage(1).is_err());
            } else {
                assert!(resources.work(1).is_err());
            }
            Ok(())
        });
        assert!(matches!(
            result,
            Err(Error::CanonicalAssertions(
                super::super::CanonicalAssertionErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(budget.failed_storage().is_some(), storage_denial);
        assert_eq!(budget.failed_work().is_some(), !storage_denial);
        assert_eq!(owned, 0);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn propagated_resource_errors_are_superseded_by_sticky_accounting_postflight() {
    for storage_denial in [false, true] {
        let mut work = Work::new(if storage_denial { LIMIT } else { 0 });
        let mut budget = Budget::new(&mut work, if storage_denial { FLOOR } else { LIMIT });
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let state = ReservationState::new(&budget, owned).unwrap();
        let result = with_resource_borrow(&mut budget, &mut owned, state, |resources| {
            // Deliberately propagate Err instead of swallowing it into Ok.
            if storage_denial {
                resources.reserve_storage(1)?;
            } else {
                resources.work(1)?;
            }
            Ok(())
        });
        assert!(matches!(
            result,
            Err(Error::CanonicalAssertions(
                super::super::CanonicalAssertionErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(budget.failed_storage().is_some(), storage_denial);
        assert_eq!(budget.failed_work().is_some(), !storage_denial);
        assert_eq!(owned, 0);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn custody_refuses_undercut_counter_regression_and_equal_slot_foreign_ledger() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 7;
    let state = ReservationState::new(&budget, owned).unwrap();
    assert!(state.check(&budget, owned - 1).is_err());
    with_resource_borrow(&mut budget, &mut owned, state, |resources| {
        resources.reserve_storage(13)
    })
    .unwrap();
    budget.release_storage(1).unwrap();
    assert!(state.check(&budget, owned).is_err());

    let mut foreign_work = Work::new(LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(FLOOR + 13).unwrap();
    assert!(state.check(&foreign, owned).is_err());
    let original = std::mem::replace(&mut budget, foreign);
    assert!(state.check(&budget, owned).is_err());
    drop(original);
}

#[test]
fn callback_surplus_is_not_added_to_or_refunded_by_the_owned_counter() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let state = ReservationState::new(&budget, owned).unwrap();
    with_resource_borrow(&mut budget, &mut owned, state, |resources| {
        resources.reserve_storage(17)
    })
    .unwrap();
    // Model a separate canonical query/caller reservation on the same ledger.
    budget.reserve_storage(23).unwrap();
    state.check(&budget, owned).unwrap();
    assert_eq!(owned, 17);
    assert_eq!(budget.storage(), FLOOR + 17 + 23);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR + 23);
}

#[test]
fn frame_accounts_for_large_callback_and_return_and_owned_prefix_must_fit() {
    let small = frame::<(), ()>().unwrap();
    let large = frame::<[u8; 8192], [u8; 8192]>().unwrap();
    assert_eq!(
        large - small,
        2 * 8192 + 2 * (size_of::<Result<[u8; 8192]>>() - size_of::<Result<()>>())
    );
    assert!(large > small + 3 * 8192);
    let mut work = Work::new(LIMIT);
    let budget = Budget::new(&mut work, LIMIT);
    assert!(ReservationState::new(&budget, 1).is_err());
}

#[test]
fn storage_and_work_boundaries_are_exact_for_the_shared_resource_adapter() {
    for under in [false, true] {
        let mut work = Work::new(3);
        let mut budget = Budget::new(&mut work, FLOOR + 24 - usize::from(under));
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let state = ReservationState::new(&budget, owned).unwrap();
        let result = with_resource_borrow(&mut budget, &mut owned, state, |resources| {
            resources.filled(3, 7_u64)
        });
        assert_eq!(result.is_ok(), !under);
        assert_eq!(budget.work(), 3);
        assert_eq!(budget.failed_storage().is_some(), under);
        if let Ok(values) = result {
            assert_eq!(owned, 24);
            drop(values);
            budget.release_storage(owned).unwrap();
        } else {
            assert_eq!(owned, 0);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}
