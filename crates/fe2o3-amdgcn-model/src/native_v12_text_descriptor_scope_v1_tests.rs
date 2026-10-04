//! Private fault injection only; no caller-controlled production callback.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    panic::panic_any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
    },
};

const FLOOR: usize = 29;
const PRIOR: usize = 7;

// Exact predecessor scope, used only for nominal, non-hostile transcript parity.
fn legacy_scoped<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let floor = budget.storage();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(E::Panicked)
        }
    };
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    if let Err(error) = restored {
        drop(result);
        return Err(error.into());
    }
    result
}

#[test]
fn nominal_scope_matches_exact_predecessor_work_storage_errors_and_transfer() {
    for exit in 0..3 {
        for work_limit in [PRIOR, PRIOR + 1, PRIOR + 20] {
            for storage_limit in [FLOOR, FLOOR + 128] {
                let run = |legacy| {
                    let mut work = Work::new(work_limit);
                    let transcript = {
                        let mut budget = Budget::new(&mut work, storage_limit);
                        budget.reserve_storage(FLOOR).unwrap();
                        budget.charge_work(PRIOR).unwrap();
                        let body = |budget: &mut Budget<'_>| {
                            let (text, bytes) =
                                engine_text(16, budget, || Ok(String::from("native")))?;
                            exact_output_bytes(text.as_bytes(), b"native", budget)?;
                            drop(text);
                            budget.release_storage(bytes)?;
                            match exit {
                                0 => Ok(17),
                                1 => Err(E::Invalid("nominal native refusal")),
                                _ => panic!("nominal native scope panic"),
                            }
                        };
                        let result = if legacy {
                            legacy_scoped(&mut budget, body)
                        } else {
                            scoped(&mut budget, body)
                        };
                        assert_eq!(budget.storage(), FLOOR);
                        (
                            result.map_err(|error| format!("{error:?}")),
                            budget.work(),
                            budget.peak_storage(),
                            budget.failed_storage(),
                        )
                    };
                    (transcript, work.failed_work())
                };
                assert_eq!(run(false), run(true));
            }
        }
    }
}

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, AtomicOrdering::SeqCst);
    }
}

#[test]
fn scope_local_owners_drop_on_success_error_and_unwind_before_return() {
    for exit in 0..3 {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(PRIOR).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let dropped = Arc::new(AtomicBool::new(false));
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(3)?;
            let _owner = Dropped(dropped.clone());
            match exit {
                0 => Ok(19),
                1 => Err(E::Invalid("retained exact error")),
                _ => panic!("private scope unwind"),
            }
        });
        match exit {
            0 => assert_eq!(result.unwrap(), 19),
            1 => assert!(matches!(result, Err(E::Invalid("retained exact error")))),
            _ => assert!(matches!(result, Err(E::Panicked))),
        }
        assert!(dropped.load(AtomicOrdering::SeqCst));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + 13);
        assert_eq!(budget.work(), PRIOR + 3);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

struct HostilePayload;
impl Drop for HostilePayload {
    fn drop(&mut self) {
        panic!("private panic payload destructor");
    }
}

#[test]
fn hostile_payload_unwind_observes_already_restored_same_ledger_floor() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(FLOOR).unwrap();
    budget.charge_work(PRIOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), E> = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(3)?;
            let _owner = Dropped(dropped.clone());
            panic_any(HostilePayload)
        });
    }));
    assert!(result.is_err());
    assert!(dropped.load(AtomicOrdering::SeqCst));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + 13);
    assert_eq!(budget.work(), PRIOR + 3);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn all_foreign_ledger_exits_refuse_without_refunding_either_meter() {
    for exit in 0..3 {
        let mut original_work = Work::new(100);
        let mut foreign_work = Work::new(100);
        let mut original = Budget::new(&mut original_work, 100);
        let mut foreign = Budget::new(&mut foreign_work, 100);
        original.reserve_storage(FLOOR).unwrap();
        original.charge_work(PRIOR).unwrap();
        foreign.reserve_storage(61).unwrap();
        foreign.charge_work(11).unwrap();
        let original_ledger = original.work_ledger_identity_v1();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        let result = scoped(&mut original, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(3)?;
            std::mem::swap(budget, &mut foreign);
            match exit {
                0 => Ok(()),
                1 => Err(E::Invalid("foreign error")),
                _ => panic!("foreign panic"),
            }
        });
        assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
        assert!(original.work_ledger_identity_v1() == foreign_ledger);
        assert!(foreign.work_ledger_identity_v1() == original_ledger);
        assert_eq!((original.storage(), original.work()), (61, 11));
        assert_eq!((foreign.storage(), foreign.work()), (FLOOR + 13, PRIOR + 3));
        // Only the test that still owns both meters can close their reservations.
        foreign.release_storage(13).unwrap();
        std::mem::swap(&mut original, &mut foreign);
        original.release_storage(FLOOR).unwrap();
        foreign.release_storage(61).unwrap();
    }
}

struct RejectedResult(Arc<AtomicBool>);
impl Drop for RejectedResult {
    fn drop(&mut self) {
        self.0.store(true, AtomicOrdering::SeqCst);
        panic!("private rejected result destructor");
    }
}

#[test]
fn undercut_refuses_and_contains_rejected_result_drop_panic() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(FLOOR).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        budget.charge_work(3)?;
        Ok(RejectedResult(dropped.clone()))
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert!(dropped.load(AtomicOrdering::SeqCst));
    assert_eq!(budget.storage(), FLOOR - 1);
    assert_eq!(budget.work(), 3);
}

#[test]
fn foreign_ledger_rejected_result_drops_without_foreign_refund() {
    let mut original_work = Work::new(100);
    let mut foreign_work = Work::new(100);
    let mut original = Budget::new(&mut original_work, 100);
    let mut foreign = Budget::new(&mut foreign_work, 100);
    original.reserve_storage(FLOOR).unwrap();
    foreign.reserve_storage(61).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = scoped(&mut original, |budget| {
        std::mem::swap(budget, &mut foreign);
        Ok(RejectedResult(dropped.clone()))
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert!(dropped.load(AtomicOrdering::SeqCst));
    assert_eq!(original.storage(), 61);
    assert_eq!(foreign.storage(), FLOOR);
    std::mem::swap(&mut original, &mut foreign);
    original.release_storage(FLOOR).unwrap();
    foreign.release_storage(61).unwrap();
}

#[test]
fn foreign_hostile_payload_cannot_refund_a_matching_numeric_floor() {
    let mut original_work = Work::new(100);
    let mut foreign_work = Work::new(100);
    let mut original = Budget::new(&mut original_work, 100);
    let mut foreign = Budget::new(&mut foreign_work, 100);
    original.reserve_storage(FLOOR).unwrap();
    foreign.reserve_storage(FLOOR + 13).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), E> = scoped(&mut original, |budget| {
            std::mem::swap(budget, &mut foreign);
            panic_any(HostilePayload)
        });
    }));
    assert!(result.is_err());
    assert_eq!(original.storage(), FLOOR + 13);
    assert_eq!(foreign.storage(), FLOOR);
    std::mem::swap(&mut original, &mut foreign);
    original.release_storage(FLOOR).unwrap();
    foreign.release_storage(FLOOR + 13).unwrap();
}
