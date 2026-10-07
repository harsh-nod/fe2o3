use super::super::{DEFERRED, EMPTY, QUARANTINED, RESERVED, RETIRING, TERMINAL_PENDING};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::fd::AsRawFd;

#[test]
fn quiescent_phase_retains_original_guard_ledger_and_prepaid_shutdown() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut original = Account::new(Work::new(usize::MAX), Service::STORAGE);
    original.with_budget(|b| b.charge_work(19)).unwrap();
    let mut service = Service::admit_at(&REAPER, original).unwrap();
    let guard = File::open("/dev/null").unwrap();
    let descriptor = guard.as_raw_fd();
    // This private close-only fixture tests retention, not lifecycle admission.
    let ledger = match &mut *REAPER.mode.lock().unwrap() {
        ReaperMode::Native(native) => {
            native.deployment_guard = Some(guard);
            native.ledger.with_budget(|b| b.work_ledger_identity_v1())
        }
        _ => unreachable!(),
    };
    let before = service.report().unwrap();
    assert_eq!(before.work, 19 + Service::ADMISSION_WORK);
    for checkpoint in 1..=2 {
        service.checkpoint_quiescent_phase().unwrap();
        let after = service.report().unwrap();
        assert_eq!(
            after.work,
            before.work + checkpoint * Service::quiescent_phase_work()
        );
        assert_eq!(after.storage, before.storage);
        assert_eq!(after.peak_storage, before.peak_storage);
        assert_eq!(after.failed_work, before.failed_work);
        assert!(after.admission_open);
        assert!(service.shutdown_paid);
        match &mut *REAPER.mode.lock().unwrap() {
            ReaperMode::Native(native) => {
                assert_eq!(
                    native.deployment_guard.as_ref().unwrap().as_raw_fd(),
                    descriptor
                );
                assert!(native.ledger.with_budget(|b| b.work_ledger_identity_v1()) == ledger);
            }
            _ => panic!("checkpoint changed the original native mode"),
        }
    }
    let mut request = Account::new(Work::new(usize::MAX), 0);
    let reservation = request.with_budget(|b| service.reserve_launch(b)).unwrap();
    assert_eq!(service.checkpoint_quiescent_phase(), Err(Failure::Busy));
    drop(reservation);
    service.checkpoint_quiescent_phase().unwrap();
    let final_report = service.report().unwrap();
    let returned = service.shutdown().unwrap();
    // A borrowed Work identity is not persistent across moving its owner.
    assert_eq!(returned.work(), final_report.work);
    assert_eq!(returned.work_limit(), final_report.work_limit);
    assert_eq!(returned.failed_work(), final_report.failed_work);
    assert_eq!(returned.peak_storage(), final_report.peak_storage);
    assert_eq!(returned.storage(), 0);
    assert!(matches!(*REAPER.mode.lock().unwrap(), ReaperMode::Closed));
    assert_eq!(service.checkpoint_quiescent_phase(), Err(Failure::State));
    assert!(Service::admit_at(&REAPER, returned).is_err());
}

#[test]
fn quiescent_phase_of_another_live_pool_cannot_retire_original_reservation() {
    static ORIGINAL: DeferredReaperV1 = DeferredReaperV1::new();
    static OTHER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut original = Service::admit_at(
        &ORIGINAL,
        Account::new(Work::new(usize::MAX), Service::STORAGE),
    )
    .unwrap();
    let mut other = Service::admit_at(
        &OTHER,
        Account::new(Work::new(usize::MAX), Service::STORAGE),
    )
    .unwrap();
    let mut request = Account::new(Work::new(usize::MAX), 0);
    let reservation = request.with_budget(|b| original.reserve_launch(b)).unwrap();
    assert_eq!(original.checkpoint_quiescent_phase(), Err(Failure::Busy));
    let before = original.report().unwrap();
    other.checkpoint_quiescent_phase().unwrap();
    assert_eq!(original.report().unwrap(), before);
    assert_eq!(original.checkpoint_quiescent_phase(), Err(Failure::Busy));
    drop(reservation);
    original.checkpoint_quiescent_phase().unwrap();
    original.shutdown().unwrap();
    other.shutdown().unwrap();
}

#[test]
fn quiescent_phase_refuses_every_nonterminal_cell_without_discarding_it() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut service = Service::admit_at(
        &REAPER,
        Account::new(Work::new(usize::MAX), Service::STORAGE),
    )
    .unwrap();
    for state in [RESERVED, DEFERRED, QUARANTINED, RETIRING, TERMINAL_PENDING] {
        REAPER.cells[CAPACITY - 1]
            .state
            .store(state, Ordering::Release);
        let before = service.report().unwrap();
        assert_eq!(service.checkpoint_quiescent_phase(), Err(Failure::Busy));
        assert_eq!(
            REAPER.cells[CAPACITY - 1].state.load(Ordering::Acquire),
            state
        );
        let after = service.report().unwrap();
        assert_eq!(after.work, before.work + Service::quiescent_phase_work());
        assert_eq!(after.storage, before.storage);
        assert!(after.admission_open);
    }
    // Only inert fixture state was installed; no child or native custody existed.
    REAPER.cells[CAPACITY - 1]
        .state
        .store(EMPTY, Ordering::Release);
    service.shutdown().unwrap();
}

#[test]
fn quiescent_phase_refuses_orphaned_retained_storage_and_stops_admission() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut service = Service::admit_at(
        &REAPER,
        Account::new(Work::new(usize::MAX), Service::STORAGE + 1),
    )
    .unwrap();
    match &mut *REAPER.mode.lock().unwrap() {
        ReaperMode::Native(native) => {
            native.ledger.with_budget(|b| b.reserve_storage(1)).unwrap();
            native.retained_storage = 1;
        }
        _ => unreachable!(),
    }
    assert_eq!(
        service.checkpoint_quiescent_phase(),
        Err(Resource::Accounting.into())
    );
    assert!(!service.report().unwrap().admission_open);
    match &mut *REAPER.mode.lock().unwrap() {
        ReaperMode::Native(native) => {
            assert_eq!(native.retained_storage, 1);
            native.retire_storage(1).unwrap();
        }
        _ => unreachable!(),
    }
    service.shutdown().unwrap();
}

#[test]
fn quiescent_phase_work_refusal_preserves_denial_and_final_shutdown_credit() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK + Service::quiescent_phase_work() - 1;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    assert!(matches!(
        service.checkpoint_quiescent_phase(),
        Err(Failure::Resource(Resource::Work(_)))
    ));
    let report = service.report().unwrap();
    assert_eq!(report.work, Service::ADMISSION_WORK);
    assert_eq!(report.failed_work, Some(limit + 1));
    assert_eq!(report.storage, Service::STORAGE);
    assert!(!report.admission_open);
    let returned = service.shutdown().unwrap();
    assert_eq!(returned.work(), report.work);
    assert_eq!(returned.failed_work(), report.failed_work);
}

#[test]
fn quiescent_phase_cannot_reopen_a_shutdown_attempt() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut service = Service::admit_at(
        &REAPER,
        Account::new(Work::new(usize::MAX), Service::STORAGE),
    )
    .unwrap();
    REAPER.cells[0].state.store(RESERVED, Ordering::Release);
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    REAPER.cells[0].state.store(EMPTY, Ordering::Release);
    let before = service.report().unwrap();
    assert_eq!(service.checkpoint_quiescent_phase(), Err(Failure::State));
    assert_eq!(service.report().unwrap(), before);
    service.shutdown().unwrap();
}
