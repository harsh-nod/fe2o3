//! Descriptor-free pool fixtures only; no fabricated production reaping evidence.
use super::super::super::{CAPACITY, DEFERRED, EMPTY, QUARANTINED, TERMINAL_PENDING};
use super::super::{Account, Service};
use super::*;
use crate::process_cleanup::ChildCleanupV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};

const LIMIT: usize = 1_000_000_000;
const INPUT: usize = 1024;

struct Witness {
    ready: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Witness {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
// SAFETY: inert atomic-only callbacks, complete INPUT declaration, no native owner.
unsafe impl Payload for Witness {
    const RETIRE_WORK: usize = 64;
    const RETIRE_SCRATCH: usize = 128;
    type Prepared = ();
    fn try_prepare_retirement(&self) -> Option<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.ready.load(Ordering::SeqCst).then_some(())
    }
    fn retire(self, _: ()) {
        drop(self);
    }
}

fn pool(work: usize, storage: usize) -> Service {
    crate::process_reaper::isolated_cleanup(Account::new(Work::new(work), storage))
}
fn terminal() -> ChildCleanupV1 {
    let mut child = ChildCleanupV1::new(None, rustix::process::Pid::from_raw(1000).unwrap(), None);
    child.terminal_reaped();
    child
}
fn witness(ready: bool) -> Witness {
    Witness {
        ready: Arc::new(AtomicBool::new(ready)),
        calls: Arc::new(AtomicUsize::new(0)),
        drops: Arc::new(AtomicUsize::new(0)),
    }
}

#[test]
fn exact_late_funding_and_one_short_denials_preserve_slot_and_ledger_history() {
    let q = Holder::<Witness>::quota(INPUT).unwrap();
    for short in 0..4 {
        let mut service = pool(
            if short == 2 {
                Service::ADMISSION_WORK + q.work() - 1
            } else {
                LIMIT
            },
            Service::STORAGE + q.persistent_storage() - usize::from(short == 3),
        );
        let mut work = Work::new(Service::RESERVATION_WORK + q.work() - usize::from(short == 1));
        let mut b = Budget::new(&mut work, q.scratch());
        let slot = service.reserve_launch(&mut b).unwrap().into_slot();
        let before = service.report().unwrap();
        let result = service.reserve_late::<Witness>(&slot, INPUT, &mut b);
        assert_eq!(b.storage(), 0);
        if short == 0 {
            let (owner, storage) = result.unwrap();
            assert_eq!(storage.additional_storage(), q.retained_storage());
            assert_eq!(owner.retained_storage(), q.retained_storage());
            assert_eq!(b.work(), Service::RESERVATION_WORK + q.work());
            assert_eq!(b.peak_storage(), q.scratch());
            assert_eq!(
                service.report().unwrap().storage,
                before.storage + q.persistent_storage()
            );
            drop(owner);
        } else {
            assert!(matches!(result, Err(Failure::Resource(_))));
            assert!(slot.cell.late.lock().unwrap().is_none());
            assert_eq!(service.report().unwrap().storage, before.storage);
            if short == 1 {
                assert!(b.failed_work().is_some());
            }
            if short >= 2 {
                let mut mode = service.reaper.mode.lock().unwrap();
                let native = service.native(&mut mode).unwrap();
                assert!(!native.admission_open);
                assert!(if short == 2 {
                    native.ledger.failed_work().is_some()
                } else {
                    native.ledger.failed_storage().is_some()
                });
            }
        }
        slot.complete();
        assert_eq!(service.report().unwrap().storage, Service::STORAGE);
    }
}

#[test]
fn terminal_late_readiness_defers_without_another_wait_or_stalling_other_slots() {
    let mut service = pool(LIMIT, LIMIT);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let first = service.reserve_launch(&mut b).unwrap().into_slot();
    let second = service.reserve_launch(&mut b).unwrap().into_slot();
    let cell = first.cell;
    let other = second.cell;
    let (owner, _) = service
        .reserve_late::<Witness>(&first, INPUT, &mut b)
        .unwrap();
    let (other_owner, _) = service
        .reserve_late::<Witness>(&second, INPUT, &mut b)
        .unwrap();
    let value = witness(false);
    let ready = value.ready.clone();
    let calls = value.calls.clone();
    let drops = value.drops.clone();
    owner.prepare_attachment().unwrap().commit(value);
    let value = witness(true);
    let other_drops = value.drops.clone();
    other_owner.prepare_attachment().unwrap().commit(value);
    first.defer(terminal());
    second.defer(terminal());
    service.pump(2).unwrap();
    assert_eq!(cell.state.load(Ordering::Acquire), TERMINAL_PENDING);
    assert!(cell.child.lock().unwrap().is_none());
    assert!(cell.late.lock().unwrap().is_some());
    assert_eq!(other.state.load(Ordering::Acquire), EMPTY);
    assert_eq!(other_drops.load(Ordering::SeqCst), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let floor = service.report().unwrap().storage;
    service.pump(CAPACITY).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(service.report().unwrap().storage, floor);
    assert!(cell.child.lock().unwrap().is_none());
    ready.store(true, Ordering::SeqCst);
    service.pump(CAPACITY).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(cell.state.load(Ordering::Acquire), EMPTY);
    service.pump(CAPACITY).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(service.report().unwrap().storage, Service::STORAGE);
}

#[test]
fn terminal_defer_retains_original_immutable_launch_dependencies_separately() {
    let mut service = pool(LIMIT, LIMIT);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(INPUT).unwrap();
    let original = witness(true);
    let original_drops = original.drops.clone();
    let (reservation, launch_view, _) = service.reserve_retaining(original, INPUT, &mut b).unwrap();
    let slot = reservation.into_slot();
    let cell = slot.cell;
    let (holder, _) = service
        .reserve_late::<Witness>(&slot, INPUT, &mut b)
        .unwrap();
    let value = witness(false);
    let ready = value.ready.clone();
    let late_drops = value.drops.clone();
    holder.prepare_attachment().unwrap().commit(value);
    drop(launch_view);
    drop(holder);
    let floor = service.report().unwrap().storage;
    slot.defer(terminal());
    service.pump(1).unwrap();
    assert_eq!(cell.state.load(Ordering::Acquire), TERMINAL_PENDING);
    assert!(cell.retained.lock().unwrap().is_some());
    assert_eq!(service.report().unwrap().storage, floor);
    assert_eq!(original_drops.load(Ordering::SeqCst), 0);
    assert_eq!(late_drops.load(Ordering::SeqCst), 0);
    ready.store(true, Ordering::SeqCst);
    service.pump(CAPACITY).unwrap();
    assert_eq!(original_drops.load(Ordering::SeqCst), 1);
    assert_eq!(late_drops.load(Ordering::SeqCst), 1);
    assert_eq!(service.report().unwrap().storage, Service::STORAGE);
}

#[test]
fn unconfirmed_child_and_controller_loss_retain_late_payload() {
    let mut service = pool(LIMIT, LIMIT);
    let reaper = service.reaper;
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let slot = service.reserve_launch(&mut b).unwrap().into_slot();
    let cell = slot.cell;
    let (owner, _) = service
        .reserve_late::<Witness>(&slot, INPUT, &mut b)
        .unwrap();
    let value = witness(true);
    let drops = value.drops.clone();
    let calls = value.calls.clone();
    owner.prepare_attachment().unwrap().commit(value);
    slot.defer(ChildCleanupV1::new(
        None,
        rustix::process::Pid::from_raw(1000).unwrap(),
        None,
    ));
    drop(owner);
    drop(service);
    let mut service = Service::recover_at(reaper).unwrap();
    service.pump(1).unwrap();
    assert_eq!(cell.state.load(Ordering::Acquire), QUARANTINED);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    // Teardown only: this fixture never created a process or domain.
    *cell.child.lock().unwrap() = Some(crate::process_cleanup::CleanupRecordV1::Child(terminal()));
    cell.state.store(DEFERRED, Ordering::Release);
    service.pump(CAPACITY).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn exhausted_retirement_funding_preserves_actual_payload_without_polling() {
    let q = Holder::<Witness>::quota(INPUT).unwrap();
    let mut service = pool(
        Service::ADMISSION_WORK
            + q.work()
            + Service::TURN_WORK
            + Service::CELL_WORK
            + q.retirement_work(),
        LIMIT,
    );
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let slot = service.reserve_launch(&mut b).unwrap().into_slot();
    let cell = slot.cell;
    let (owner, _) = service
        .reserve_late::<Witness>(&slot, INPUT, &mut b)
        .unwrap();
    let value = witness(false);
    let calls = value.calls.clone();
    let drops = value.drops.clone();
    owner.prepare_attachment().unwrap().commit(value);
    slot.defer(terminal());
    service.pump(1).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let before = service.report().unwrap();
    assert!(matches!(service.pump(1), Err(Failure::Resource(_))));
    assert_eq!(service.report().unwrap().storage, before.storage);
    assert_eq!(cell.state.load(Ordering::Acquire), TERMINAL_PENDING);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
