//! Synthetic accounting/completion predicates only; never fake source owners,
//! inventories, C1/C3 occurrences or positive admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 37;

#[test]
fn completion_requires_the_real_driver_summary_and_unique_final_capture() {
    let good = NominalCapabilityRunV1 {
        query_visits: [1, 1, 1],
        authenticated_visits: [1, 1, 1],
        reached_blocks: [3, 3, 3],
        array_destination_has_origin: false,
    };
    assert_eq!(require_completed_run(good, [1, 1, 1], true), Ok(()));
    for pass in 0..3 {
        let mut bad = good;
        bad.query_visits[pass] = 0;
        assert!(require_completed_run(bad, [1, 1, 1], true).is_err());
        let mut seen = [1, 1, 1];
        seen[pass] = 0;
        assert!(require_completed_run(good, seen, true).is_err());
    }
    let mut bad = good;
    bad.array_destination_has_origin = true;
    assert!(require_completed_run(bad, [1, 1, 1], true).is_err());
    assert!(require_completed_run(good, [1, 1, 1], false).is_err());
    for final_count in [0, 2] {
        let mut bad = good;
        bad.authenticated_visits[2] = final_count;
        assert!(require_completed_run(bad, bad.authenticated_visits, true).is_err());
    }
}

#[test]
fn exact_scope_work_and_storage_boundaries_preserve_prefix() {
    for mode in 0..3 {
        let entered = Cell::new(false);
        let body = |budget: &mut Budget<'_>, protected: Checkpoint| {
            protected.require_completed(budget)?;
            entered.set(true);
            Ok(())
        };
        let storage = scope_storage::<()>(size_of_val(&body)).unwrap();
        let mut work = Work::new(7 + SCOPE_WORK - usize::from(mode == 1));
        let mut budget = Budget::new(&mut work, FLOOR + storage - usize::from(mode == 2));
        budget.charge_work(7).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_owned_scope(&mut budget, body);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), FLOOR);
        match mode {
            0 => {
                assert_eq!(result, Ok(()));
                assert!(entered.get());
                assert_eq!(budget.work(), 7 + SCOPE_WORK);
                assert_eq!(budget.peak_storage(), FLOOR + storage);
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert!(!entered.get());
                assert_eq!(budget.failed_work(), Some(7 + SCOPE_WORK));
            }
            _ => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert!(!entered.get());
                assert!(budget.failed_storage().is_some());
            }
        }
    }
}

#[test]
fn candidate_scope_success_error_and_panic_retain_callback_surplus() {
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let original = budget.work_ledger_identity_v1();
        let result = with_owned_scope(&mut budget, |budget, protected| -> Result<()> {
            protected.require_completed(budget)?;
            budget.reserve_storage(23)?;
            budget.charge_work(17)?;
            match mode {
                0 => Ok(()),
                1 => Err(Error::Unavailable("synthetic callback refusal")),
                _ => panic!("synthetic final-candidate panic"),
            }
        });
        match mode {
            0 => assert_eq!(result, Ok(())),
            1 => assert_eq!(
                result,
                Err(Error::Unavailable("synthetic callback refusal"))
            ),
            _ => assert_eq!(result, Err(Error::CallbackPanicked)),
        }
        assert!(budget.work_ledger_identity_v1() == original);
        assert_eq!(budget.storage(), FLOOR + 23);
        assert_eq!(budget.work(), SCOPE_WORK + 17);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
    }
}

#[test]
fn completion_checkpoint_rejects_sticky_denials_and_unreleased_inner_storage() {
    for mode in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let loan_entered = Cell::new(false);
        let result = with_owned_scope(&mut budget, |budget, protected| {
            match mode {
                0 => {
                    let _ = budget.charge_work(LIMIT + 1);
                }
                1 => {
                    let _ = budget.reserve_storage(LIMIT + 1);
                }
                _ => {
                    budget.reserve_storage(1)?;
                }
            }
            protected.require_completed(budget)?;
            loan_entered.set(true);
            Ok(())
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert!(!loan_entered.get());
        assert_eq!(budget.storage(), FLOOR + usize::from(mode == 2));
        assert_eq!(budget.failed_work().is_some(), mode == 0);
        assert_eq!(budget.failed_storage().is_some(), mode == 1);
    }
}

#[test]
fn late_callback_ignored_denials_are_not_success() {
    for mode in 0..2 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_owned_scope(&mut budget, |budget, protected| {
            protected.require_completed(budget)?;
            if mode == 0 {
                let _ = budget.charge_work(LIMIT + 1);
            } else {
                let _ = budget.reserve_storage(LIMIT + 1);
            }
            Ok(())
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_work().is_some(), mode == 0);
        assert_eq!(budget.failed_storage().is_some(), mode == 1);
    }
}

#[test]
fn undercut_floor_is_not_repaired_or_exposed_as_completed() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let at = Cell::new(0);
    let loan_entered = Cell::new(false);
    let result = with_owned_scope(&mut budget, |budget, protected| {
        at.set(budget.storage());
        budget.release_storage(1)?;
        protected.require_completed(budget)?;
        loan_entered.set(true);
        Ok(())
    });
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert!(!loan_entered.get());
    assert_eq!(budget.storage(), at.get() - 1);
    assert!(budget.storage() > FLOOR);
}

#[test]
fn replacement_ledger_is_left_untouched() {
    let mut original_work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut original_work, LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(19).unwrap();
    let foreign_identity = foreign.work_ledger_identity_v1();
    let result = with_owned_scope(&mut budget, |budget, protected| {
        std::mem::swap(budget, &mut foreign);
        protected.require_completed(budget)?;
        Ok(())
    });
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert!(budget.work_ledger_identity_v1() == foreign_identity);
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.work(), 0);
}

#[test]
fn preexisting_denial_refuses_before_body() {
    for mode in 0..2 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        if mode == 0 {
            let _ = budget.charge_work(LIMIT + 1);
        } else {
            let _ = budget.reserve_storage(LIMIT + 1);
        }
        let before = (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_work(),
            budget.failed_storage(),
        );
        let entered = Cell::new(false);
        assert_eq!(
            with_owned_scope(&mut budget, |_, _| {
                entered.set(true);
                Ok(())
            }),
            Err(Resource::Accounting.into())
        );
        assert!(!entered.get());
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage()
            ),
            before
        );
    }
}

#[test]
fn panic_payload_drops_before_own_refund_and_explicit_errors_survive() {
    struct OwnedPayload(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for OwnedPayload {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let cloned = dropped.clone();
    assert_eq!(
        with_owned_scope::<(), _>(&mut budget, move |_, _| {
            std::panic::panic_any(OwnedPayload(cloned));
        }),
        Err(Error::CallbackPanicked)
    );
    assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(budget.storage(), 0);
    for error in [
        Resource::Allocation,
        Resource::Arithmetic,
        Resource::Accounting,
    ] {
        assert_eq!(
            with_owned_scope::<(), _>(&mut budget, |_, _| Err(error.into())),
            Err(error.into())
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn large_callback_and_result_are_prepaid_with_checked_arithmetic() {
    let payload = [7u8; 8192];
    let body = move |_budget: &mut Budget<'_>, _protected: Checkpoint| Ok(payload);
    let bytes = scope_storage::<[u8; 8192]>(size_of_val(&body)).unwrap();
    assert!(bytes >= 8 * 8192);
    let mut work = Work::new(SCOPE_WORK);
    let mut budget = Budget::new(&mut work, bytes);
    assert_eq!(with_owned_scope(&mut budget, body), Ok([7; 8192]));
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), bytes);
    assert_eq!(
        scope_storage::<()>(usize::MAX),
        Err(Resource::Arithmetic.into())
    );
}

#[test]
fn synthetic_wrong_slot_checkpoint_is_not_completion() {
    let mut work = Work::new(LIMIT);
    let budget = Budget::new(&mut work, LIMIT);
    let mut original = Checkpoint::take(&budget);
    original.slot ^= 1;
    assert_eq!(
        original.require_completed(&budget),
        Err(Resource::Accounting.into())
    );
}
