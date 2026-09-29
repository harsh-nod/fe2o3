use super::super::super::{DEFERRED, RESERVED};
use super::super::{Account, ProtectedServiceCleanupReservationV2 as Reservation};
use super::*;
use crate::process_cleanup::ChildCleanupV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, atomic::AtomicUsize},
};

const LIMIT: usize = 100_000_000
    + 2 * (Service::TURN_WORK + super::super::CAPACITY * Service::CELL_WORK)
    + Service::RESERVATION_WORK
    + Service::STORAGE;
const INPUT: usize = 256;

thread_local! {
    static PENDING_OBSERVER: std::cell::Cell<Option<fn(&ReapCellV1)>> = const {
        std::cell::Cell::new(None)
    };
}

pub(super) fn observe_before_terminal_pending(cell: &ReapCellV1) {
    if let Some(observe) = PENDING_OBSERVER.with(|observer| observer.take()) {
        observe(cell);
    }
}

struct Witness(Arc<AtomicUsize>);
impl Drop for Witness {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn pool(work: usize, storage: usize) -> Service {
    Service::admit_at(
        Box::leak(Box::new(DeferredReaperV1::new())),
        Account::new(Work::new(work), storage),
    )
    .unwrap()
}
fn dropped() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}
fn terminal() -> ChildCleanupV1 {
    let mut child = ChildCleanupV1::new(None, rustix::process::Pid::from_raw(1000).unwrap(), None);
    // Descriptor/process-free fixture only, never production reap evidence.
    child.terminal_reaped();
    child
}

#[test]
fn exact_funding_retains_payload_until_last_view_and_slot_retire() {
    let bytes = Retained::<Witness>::payload_storage(INPUT).unwrap();
    let request = Retained::<Witness>::storage_for(INPUT).unwrap();
    let work = Service::retained_launch_work::<Witness>(INPUT).unwrap();
    let scratch = Service::retained_launch_scratch::<Witness>(INPUT).unwrap();
    for slot_first in [false, true] {
        let mut c = pool(
            Service::ADMISSION_WORK + work + Service::TURN_WORK + Service::CELL_WORK,
            Service::STORAGE + bytes,
        );
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, INPUT + scratch);
        b.reserve_storage(INPUT).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let d = dropped();
        let (slot, view, charge) = c
            .reserve_retaining(Witness(d.clone()), INPUT, &mut b)
            .unwrap();
        assert_eq!(charge.additional_storage(), request - INPUT);
        assert_eq!(
            (b.work(), b.storage(), b.peak_storage()),
            (work, INPUT, INPUT + scratch)
        );
        assert!(ledger == b.work_ledger_identity_v1());
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(view.retained_storage(), request);
        assert_eq!(c.report().unwrap().storage, Service::STORAGE + bytes);
        assert_eq!(c.reaper.cells[0].state.load(Ordering::Acquire), RESERVED);
        assert!(matches!(c.shutdown(), Err(Failure::Busy)));
        if slot_first {
            drop(slot);
            assert_eq!(c.report().unwrap().storage, Service::STORAGE);
            assert_eq!(d.load(Ordering::SeqCst), 0);
            drop(view);
        } else {
            drop(view);
            assert_eq!(d.load(Ordering::SeqCst), 0);
            drop(slot);
        }
        assert_eq!(d.load(Ordering::SeqCst), 1);
        assert_eq!(c.report().unwrap().storage, Service::STORAGE);
        b.release_storage(request).unwrap();
    }
}

#[test]
fn request_short_work_floor_and_scratch_refuse_before_service_mutation() {
    let work = Service::retained_launch_work::<Witness>(INPUT).unwrap();
    let scratch = Service::retained_launch_scratch::<Witness>(INPUT).unwrap();
    for cause in 0..3 {
        let mut c = pool(LIMIT, LIMIT);
        let before = c.report().unwrap();
        let mut w = Work::new(if cause == 0 { work - 1 } else { work });
        let mut b = Budget::new(&mut w, INPUT + scratch - usize::from(cause == 2));
        let floor = INPUT - usize::from(cause == 1);
        b.reserve_storage(floor).unwrap();
        let d = dropped();
        let result = c.reserve_retaining(Witness(d.clone()), INPUT, &mut b);
        assert!(matches!(result, Err(Failure::Resource(_))));
        assert_eq!(b.storage(), floor);
        assert_eq!(c.report().unwrap(), before);
        assert_eq!(d.load(Ordering::SeqCst), 1);
        assert!(
            c.reaper
                .cells
                .iter()
                .all(|cell| cell.state.load(Ordering::Acquire) == EMPTY)
        );
    }
}

#[test]
fn service_short_work_or_payload_storage_refuses_before_allocation_and_keeps_history() {
    let bytes = Retained::<Witness>::payload_storage(INPUT).unwrap();
    let work = Service::retained_launch_work::<Witness>(INPUT).unwrap();
    for short_work in [false, true] {
        let mut c = pool(
            if short_work {
                Service::ADMISSION_WORK + work - 1
            } else {
                LIMIT
            },
            if short_work {
                LIMIT
            } else {
                Service::STORAGE + bytes - 1
            },
        );
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(INPUT).unwrap();
        let d = dropped();
        assert!(matches!(
            c.reserve_retaining(Witness(d.clone()), INPUT, &mut b),
            Err(Failure::Resource(_))
        ));
        assert_eq!(d.load(Ordering::SeqCst), 1);
        assert_eq!(c.report().unwrap().storage, Service::STORAGE);
        assert!(!c.report().unwrap().admission_open);
        assert!(
            c.reaper
                .cells
                .iter()
                .all(|cell| cell.state.load(Ordering::Acquire) == EMPTY)
        );
        let mut mode = c.reaper.mode.lock().unwrap();
        let native = c.native(&mut mode).unwrap();
        if short_work {
            assert_eq!(
                native.ledger.failed_work(),
                Some(Service::ADMISSION_WORK + work)
            );
        } else {
            assert_eq!(
                native.ledger.failed_storage(),
                Some(Service::STORAGE + bytes)
            );
        }
    }
}

#[test]
fn deferred_quarantine_and_controller_loss_preserve_full_dependencies() {
    let mut c = pool(LIMIT, LIMIT);
    let reaper = c.reaper;
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(INPUT).unwrap();
    let d = dropped();
    let (slot, view, _) = c
        .reserve_retaining(Witness(d.clone()), INPUT, &mut b)
        .unwrap();
    let bytes = Retained::<Witness>::payload_storage(INPUT).unwrap();
    let child = ChildCleanupV1::new(None, rustix::process::Pid::from_raw(1000).unwrap(), None);
    slot.into_slot().defer(child);
    drop(view);
    assert_eq!(reaper.cells[0].state.load(Ordering::Acquire), DEFERRED);
    assert_eq!(d.load(Ordering::SeqCst), 0);
    drop(c);
    let mut c = Service::recover_at(reaper).unwrap();
    c.pump(1).unwrap();
    assert_eq!(reaper.cells[0].state.load(Ordering::Acquire), QUARANTINED);
    assert_eq!(c.report().unwrap().storage, Service::STORAGE + bytes);
    assert_eq!(d.load(Ordering::SeqCst), 0);
    assert!(matches!(c.shutdown(), Err(Failure::Busy)));
    // Explicit fixture teardown only: no process ever existed and the marker is
    // already terminal. Production cannot unquarantine or assert reaping safely.
    reaper.cells[0]
        .child
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .child_mut()
        .terminal_reaped();
    reaper.cells[0].state.store(DEFERRED, Ordering::Release);
    c.pump(64).unwrap();
    assert_eq!(d.load(Ordering::SeqCst), 1);
    assert_eq!(c.shutdown().unwrap().storage(), 0);
}

struct Nested {
    reaper: &'static DeferredReaperV1,
    reservation: Mutex<Option<Reservation>>,
    releases: Arc<AtomicUsize>,
}
impl Drop for Nested {
    fn drop(&mut self) {
        // Avoid a hanging test if either lock is mistakenly held during Drop.
        assert!(self.reaper.mode.try_lock().is_ok());
        assert!(
            self.reaper
                .cells
                .iter()
                .all(|c| c.child.try_lock().is_ok() && c.retained.try_lock().is_ok())
        );
        assert!(
            self.reaper
                .cells
                .iter()
                .any(|c| c.state.load(Ordering::Acquire) == RETIRING)
        );
        let slot = self
            .reservation
            .get_mut()
            .unwrap()
            .take()
            .unwrap()
            .into_slot();
        slot.defer(terminal());
        self.releases.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn terminal_retirement_allows_nested_cleanup_in_same_pool_without_early_reuse() {
    let mut c = pool(LIMIT, LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(2 * INPUT).unwrap();
    let nested_d = dropped();
    let (nested_slot, nested_view, _) = c
        .reserve_retaining(Witness(nested_d.clone()), INPUT, &mut b)
        .unwrap();
    drop(nested_view);
    let d = dropped();
    let nested = Nested {
        reaper: c.reaper,
        reservation: Mutex::new(Some(nested_slot)),
        releases: d.clone(),
    };
    let (outer_slot, outer_view, _) = c.reserve_retaining(nested, INPUT, &mut b).unwrap();
    drop(outer_view);
    outer_slot.into_slot().defer(terminal());
    c.pump(2).unwrap();
    assert_eq!(d.load(Ordering::SeqCst), 1);
    assert_eq!(nested_d.load(Ordering::SeqCst), 0);
    assert_eq!(c.reaper.cells[1].state.load(Ordering::Acquire), EMPTY);
    assert_eq!(c.reaper.cells[0].state.load(Ordering::Acquire), DEFERRED);
    assert_eq!(
        c.report().unwrap().storage,
        Service::STORAGE + Retained::<Witness>::payload_storage(INPUT).unwrap()
    );
    c.pump(64).unwrap();
    assert_eq!(nested_d.load(Ordering::SeqCst), 1);
    assert_eq!(c.shutdown().unwrap().storage(), 0);
}

#[test]
fn preclone_unwind_rolls_back_retained_storage_without_refunding_work() {
    let mut c = pool(LIMIT, LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(INPUT).unwrap();
    let d = dropped();
    let before = c.report().unwrap().work;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let (_slot, _view, _) = c
                .reserve_retaining(Witness(d.clone()), INPUT, &mut b)
                .unwrap();
            panic!("preclone dependency unwind");
        }))
        .is_err()
    );
    assert_eq!(d.load(Ordering::SeqCst), 1);
    assert_eq!(c.report().unwrap().storage, Service::STORAGE);
    assert!(c.report().unwrap().work > before);
    assert_eq!(b.storage(), INPUT);
}

#[test]
fn invalid_retained_floor_and_overflow_never_reserve_a_slot() {
    let mut c = pool(LIMIT, LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(INPUT).unwrap();
    let before = c.report().unwrap();
    for n in [0, usize::MAX, usize::MAX / 32] {
        let d = dropped();
        assert!(matches!(
            c.reserve_retaining(Witness(d.clone()), n, &mut b),
            Err(Failure::Resource(_))
        ));
        assert_eq!(d.load(Ordering::SeqCst), 1);
    }
    assert_eq!(c.report().unwrap(), before);
}

#[test]
fn installed_guard_clones_with_dynamic_payload_storage() {
    let mut c = pool(LIMIT, LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(INPUT + Service::GUARD_FILE_STORAGE)
        .unwrap();
    c.retain_deployment_guard(std::fs::File::open("/dev/null").unwrap(), &mut b)
        .unwrap();
    b.release_storage(Service::GUARD_FILE_STORAGE).unwrap();
    let (slot, view, _) = c
        .reserve_retaining(Witness(dropped()), INPUT, &mut b)
        .unwrap();
    let before = c.report().unwrap().storage;
    let (guard, charge) = c.try_clone_deployment_guard(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    drop(guard);
    b.release_storage(charge.additional_storage()).unwrap();
    assert_eq!(c.report().unwrap().storage, before);
    drop(view);
    drop(slot);
    c.shutdown().unwrap();
}

#[test]
fn terminal_accounting_mismatch_quarantines_slot_and_stops_admission() {
    let mut c = pool(LIMIT, LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(INPUT).unwrap();
    let (slot, view, _) = c
        .reserve_retaining(Witness(dropped()), INPUT, &mut b)
        .unwrap();
    drop(view);
    slot.into_slot().defer(terminal());
    let mut mode = c.reaper.mode.lock().unwrap();
    let native = c.native(&mut mode).unwrap();
    native.ledger.with_budget(|b| b.release_storage(1)).unwrap();
    drop(mode);
    c.pump(1).unwrap();
    assert_eq!(c.reaper.cells[0].state.load(Ordering::Acquire), QUARANTINED);
    assert!(!c.report().unwrap().admission_open);
    assert!(matches!(c.shutdown(), Err(Failure::Busy)));
}

#[test]
fn violated_destructor_contract_never_publishes_empty_or_holds_pool_locks() {
    struct BadDrop;
    impl Drop for BadDrop {
        fn drop(&mut self) {
            panic!("deliberately violated destructor contract");
        }
    }
    let mut c = pool(LIMIT, LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(INPUT).unwrap();
    // Private fault injection; a trusted production caller must never supply this.
    let (slot, view, _) = c.reserve_retaining(BadDrop, INPUT, &mut b).unwrap();
    drop(view);
    slot.into_slot().defer(terminal());
    let before = c.report().unwrap().storage;
    assert!(catch_unwind(AssertUnwindSafe(|| c.pump(1))).is_err());
    assert_eq!(c.reaper.cells[0].state.load(Ordering::Acquire), QUARANTINED);
    assert_eq!(c.report().unwrap().storage, before);
    assert!(!c.reaper.mode.is_poisoned());
    assert!(!c.reaper.cells[0].child.is_poisoned());
    assert!(!c.reaper.cells[0].retained.is_poisoned());
    assert!(matches!(c.shutdown(), Err(Failure::Busy)));
}

struct LateWitness {
    _drop: Witness,
    ready: Arc<std::sync::atomic::AtomicBool>,
    calls: Arc<AtomicUsize>,
}

// SAFETY: descriptor-free, bounded atomic-only fixture; complete partial custody
// is retained on deferral and its independent preparation needs no storage.
#[allow(unsafe_code)]
unsafe impl crate::cleanup_bridge::LateRetainedPayloadV2 for LateWitness {
    const RETIRE_WORK: usize = 64;
    const RETIRE_SCRATCH: usize = 0;
    type Prepared = ();
    fn try_prepare_retirement(&self) -> Option<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.ready.load(Ordering::SeqCst).then_some(())
    }
    fn retire(self, _: ()) {
        drop(self);
    }
}

#[test]
fn terminal_pending_publication_has_only_the_slot_backing_alias() {
    fn observe(cell: &ReapCellV1) {
        assert_eq!(cell.state.load(Ordering::Acquire), RETIRING);
        assert!(cell.child.lock().unwrap().is_none());
        assert_eq!(
            cell.late
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .strong_count_for_test(),
            1
        );
        assert!(cell.retained_storage.load(Ordering::Acquire) > 0);
    }
    for unfunded in [false, true] {
        let mut c = pool(LIMIT, LIMIT);
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let slot = c.reserve_launch(&mut b).unwrap().into_slot();
        let cell = slot.cell;
        let (holder, _) = c.reserve_late::<LateWitness>(&slot, INPUT, &mut b).unwrap();
        let drops = dropped();
        let calls = dropped();
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(unfunded));
        holder.prepare_attachment().unwrap().commit(LateWitness {
            _drop: Witness(drops.clone()),
            ready: ready.clone(),
            calls: calls.clone(),
        });
        drop(holder);
        let before = c.report().unwrap();
        if unfunded {
            let mut mode = c.reaper.mode.lock().unwrap();
            c.native(&mut mode)
                .unwrap()
                .charge(before.work_limit - before.work)
                .unwrap();
        }
        PENDING_OBSERVER.with(|observer| assert!(observer.replace(Some(observe)).is_none()));
        // No process/domain was created. Observe inside complete(), immediately
        // before the release-store, not after its local temporaries have dropped.
        slot.complete();
        PENDING_OBSERVER.with(|observer| {
            assert!(
                observer.take().is_none(),
                "publication hook was not reached"
            )
        });
        assert_eq!(cell.state.load(Ordering::Acquire), TERMINAL_PENDING);
        assert_eq!(c.report().unwrap().storage, before.storage);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert_eq!(calls.load(Ordering::SeqCst), usize::from(!unfunded));
        if !unfunded {
            ready.store(true, Ordering::SeqCst);
            c.pump(1).unwrap();
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            assert_eq!(c.report().unwrap().storage, Service::STORAGE);
        }
        // Work exhaustion keeps the original fixture slot and its full charge.
    }
}
