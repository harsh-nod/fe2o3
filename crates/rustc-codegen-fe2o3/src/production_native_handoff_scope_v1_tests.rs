//! Shared native scope accounting only; no signed-source owner is constructed.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Mark(Arc<AtomicUsize>);
impl Drop for Mark {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct PanickingPayload(Arc<AtomicUsize>);
impl Drop for PanickingPayload {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("native scope payload destructor");
    }
}

#[test]
fn native_scope_success_transfers_result_and_preserves_incoming_floor_and_work() {
    let mut work = Work::new(24);
    let mut budget = Budget::new(&mut work, 40);
    budget.reserve_storage(23).unwrap();
    budget.charge_work(11).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let drops = Arc::new(AtomicUsize::new(0));
    let result = scoped(&mut budget, |budget| {
        budget.reserve_storage(17)?;
        budget.charge_work(13)?;
        Ok(Mark(Arc::clone(&drops)))
    })
    .unwrap();
    assert_eq!(budget.storage(), 23);
    assert_eq!(budget.peak_storage(), 40);
    assert_eq!(budget.work(), 24);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(result);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn native_scope_exact_and_one_short_limits_keep_accepted_work_and_floor() {
    for (work_limit, storage_limit, work_short, storage_short) in [
        (24, 40, false, false),
        (23, 40, true, false),
        (24, 39, false, true),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        budget.charge_work(11).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(17)?;
            budget.charge_work(13)?;
            Ok(())
        });
        match result {
            Ok(()) => assert!(!work_short && !storage_short),
            Err(E::Resource(Resource::Work(_))) => assert!(work_short),
            Err(E::Resource(Resource::Storage(_))) => assert!(storage_short),
            other => panic!("unexpected native scope result: {other:?}"),
        }
        assert_eq!(budget.storage(), 23);
        assert_eq!(budget.peak_storage(), if storage_short { 23 } else { 40 });
        assert_eq!(
            budget.work(),
            if work_short || storage_short { 11 } else { 24 }
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn native_scope_refusal_and_ordinary_panic_drop_inputs_without_resetting_work() {
    for panics in [false, true] {
        let mut work = Work::new(24);
        let mut budget = Budget::new(&mut work, 40);
        budget.reserve_storage(23).unwrap();
        budget.charge_work(11).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let drops = Arc::new(AtomicUsize::new(0));
        let result: R<()> = scoped(&mut budget, |budget| {
            let _input = Mark(Arc::clone(&drops));
            budget.reserve_storage(17)?;
            budget.charge_work(13)?;
            if panics {
                panic!("native scope ordinary panic");
            }
            Err(E::Mismatch("native scope refusal"))
        });
        assert!(if panics {
            matches!(result, Err(E::Panicked))
        } else {
            matches!(result, Err(E::Mismatch("native scope refusal")))
        });
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), 23);
        assert_eq!(budget.peak_storage(), 40);
        assert_eq!(budget.work(), 24);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn native_scope_panicking_payload_destructor_runs_after_floor_cleanup() {
    let mut work = Work::new(24);
    let mut budget = Budget::new(&mut work, 40);
    budget.reserve_storage(23).unwrap();
    budget.charge_work(11).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let drops = Arc::new(AtomicUsize::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        scoped::<()>(&mut budget, |budget| {
            budget.reserve_storage(17)?;
            budget.charge_work(13)?;
            std::panic::panic_any(PanickingPayload(Arc::clone(&drops)))
        })
    }));
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), 23);
    assert_eq!(budget.peak_storage(), 40);
    assert_eq!(budget.work(), 24);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn native_scope_foreign_ledger_is_untouched_for_success_refusal_and_panic() {
    for exit in 0..3 {
        let mut original_work = Work::new(24);
        let mut foreign_work = Work::new(31);
        let mut budget = Budget::new(&mut original_work, 100);
        let mut foreign = Budget::new(&mut foreign_work, 100);
        budget.reserve_storage(23).unwrap();
        budget.charge_work(11).unwrap();
        foreign.reserve_storage(53).unwrap();
        foreign.charge_work(7).unwrap();
        let original_ledger = budget.work_ledger_identity_v1();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        let drops = Arc::new(AtomicUsize::new(0));
        let result = scoped(&mut budget, |budget| {
            let input = Mark(Arc::clone(&drops));
            budget.reserve_storage(17)?;
            budget.charge_work(13)?;
            std::mem::swap(budget, &mut foreign);
            match exit {
                0 => Ok(input),
                1 => Err(E::Mismatch("foreign refusal")),
                _ => panic!("foreign ordinary panic"),
            }
        });
        assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), 53);
        assert_eq!(budget.peak_storage(), 53);
        assert_eq!(budget.work(), 7);
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert_eq!(foreign.storage(), 40);
        assert_eq!(foreign.peak_storage(), 40);
        assert_eq!(foreign.work(), 24);
        assert!(foreign.work_ledger_identity_v1() == original_ledger);
    }
}

#[test]
fn native_scope_foreign_ledger_is_untouched_when_payload_destructor_panics() {
    let mut original_work = Work::new(24);
    let mut foreign_work = Work::new(31);
    let mut budget = Budget::new(&mut original_work, 100);
    let mut foreign = Budget::new(&mut foreign_work, 100);
    budget.reserve_storage(23).unwrap();
    budget.charge_work(11).unwrap();
    foreign.reserve_storage(53).unwrap();
    foreign.charge_work(7).unwrap();
    let original_ledger = budget.work_ledger_identity_v1();
    let foreign_ledger = foreign.work_ledger_identity_v1();
    let drops = Arc::new(AtomicUsize::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        scoped::<()>(&mut budget, |budget| {
            budget.reserve_storage(17)?;
            budget.charge_work(13)?;
            std::mem::swap(budget, &mut foreign);
            std::panic::panic_any(PanickingPayload(Arc::clone(&drops)))
        })
    }));
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), 53);
    assert_eq!(budget.peak_storage(), 53);
    assert_eq!(budget.work(), 7);
    assert!(budget.work_ledger_identity_v1() == foreign_ledger);
    assert_eq!(foreign.storage(), 40);
    assert_eq!(foreign.peak_storage(), 40);
    assert_eq!(foreign.work(), 24);
    assert!(foreign.work_ledger_identity_v1() == original_ledger);
}

#[test]
fn native_scope_below_floor_refusal_drops_result_without_inventing_reservations() {
    let mut work = Work::new(11);
    let mut budget = Budget::new(&mut work, 23);
    budget.reserve_storage(23).unwrap();
    budget.charge_work(11).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let drops = Arc::new(AtomicUsize::new(0));
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(Mark(Arc::clone(&drops)))
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), 22);
    assert_eq!(budget.work(), 11);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
