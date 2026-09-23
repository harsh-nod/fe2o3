use super::super::{DEFERRED, EMPTY, QUARANTINED, RESERVED};
use super::{
    Account, Budget, CAPACITY, DeferredReaperV1, Failure, ProtectedIssuerCleanupReportV2 as Report,
    ReaperMode, Resource, SHUTDOWN_WORK, Service,
};
use crate::process_cleanup::ChildCleanupV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::process::Pid;
use std::sync::Barrier;
use std::sync::atomic::Ordering;

const MIN_TURN_WORK: usize = Service::TURN_WORK + Service::CELL_WORK;

fn assert_work_refusal(error: Failure, actual: usize, limit: usize) {
    assert!(
        matches!(error, Failure::Resource(Resource::Work(error))
            if error.actual() == actual && error.limit() == limit),
        "unexpected refusal: {error:?}"
    );
}

fn assert_empty(reaper: &DeferredReaperV1) {
    for cell in &reaper.cells {
        assert_eq!(cell.state.load(Ordering::Acquire), EMPTY);
        assert!(cell.child.lock().unwrap().is_none());
    }
    assert!(reaper.thread_started.get().is_none());
}

fn assert_uninitialized(reaper: &DeferredReaperV1) {
    assert!(matches!(
        *reaper.mode.lock().unwrap(),
        ReaperMode::Uninitialized
    ));
    assert_empty(reaper);
}

fn synthetic_pid(index: usize) -> Pid {
    Pid::from_raw(1000 + i32::try_from(index).unwrap()).unwrap()
}

fn synthetic_child(index: usize) -> ChildCleanupV1 {
    // No process, descriptor, or spawn lease exists; missing-pidfd cleanup performs no I/O.
    ChildCleanupV1::new(None, synthetic_pid(index), None)
}

fn assert_record(reaper: &DeferredReaperV1, index: usize, state: u8) {
    let cell = &reaper.cells[index];
    assert_eq!(cell.state.load(Ordering::Acquire), state);
    let record = cell.child.lock().unwrap();
    let child = record.as_ref().expect("synthetic custody was discarded");
    assert_eq!(child.pid(), synthetic_pid(index));
    assert!(child.pidfd().is_none());
    assert!(!child.retains_spawn_lease());
    assert_eq!(child.last_errno(), None);
}

#[test]
fn exact_admission_prepays_shutdown_even_with_no_work_remaining() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut service = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .unwrap();
    let expected = Report {
        work: Service::ADMISSION_WORK,
        work_limit: Service::ADMISSION_WORK,
        failed_work: None,
        storage: Service::STORAGE,
        peak_storage: Service::STORAGE,
        admission_open: false,
        next_slot: 0,
    };
    assert_eq!(service.report().unwrap(), expected);
    assert_eq!(service.report().unwrap(), expected);
    assert_empty(&REAPER);

    let mut returned = service.shutdown().unwrap();
    assert_eq!(returned.work(), Service::ADMISSION_WORK);
    assert_eq!(returned.storage(), 0);
    assert_eq!(returned.peak_storage(), Service::STORAGE);
    assert_eq!(returned.storage_limit(), Service::STORAGE);
    assert_eq!(returned.failed_storage(), None);
    assert_eq!(returned.work_limit(), Service::ADMISSION_WORK);
    assert_eq!(returned.failed_work(), None);
    let error = returned
        .with_budget(|budget| budget.charge_work(1))
        .unwrap_err();
    assert_work_refusal(
        error.into(),
        Service::ADMISSION_WORK + 1,
        Service::ADMISSION_WORK,
    );
    assert!(matches!(*REAPER.mode.lock().unwrap(), ReaperMode::Closed));
    assert_eq!(service.report(), Err(Failure::State));
    drop(service);
    assert_empty(&REAPER);
}

#[test]
fn admission_work_one_short_returns_the_prefix_without_selecting_a_mode() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let prefix = 7;
    let limit = prefix + Service::ADMISSION_WORK - 1;
    let mut work = Work::new(limit);
    work.charge_work(prefix).unwrap();
    assert_eq!(
        work.charge_work(limit).unwrap_err().actual(),
        prefix + limit
    );
    let (error, mut returned) = Service::admit_at(&REAPER, Account::new(work, Service::STORAGE))
        .err()
        .expect("one-short admission work was accepted")
        .into_parts();

    assert_work_refusal(error, limit + 1, limit);
    assert_eq!(returned.work(), prefix);
    assert_eq!((returned.storage(), returned.peak_storage()), (0, 0));
    assert_eq!(returned.storage_limit(), Service::STORAGE);
    assert_eq!(returned.failed_storage(), None);
    assert_eq!(returned.work_limit(), limit);
    assert_eq!(returned.failed_work(), Some(prefix + limit));
    assert_uninitialized(&REAPER);

    returned
        .with_budget(|budget| budget.charge_work(limit - prefix))
        .unwrap();
    let error = returned
        .with_budget(|budget| budget.charge_work(1))
        .unwrap_err();
    assert_work_refusal(error.into(), limit + 1, limit);
    assert_eq!(returned.work(), limit);
    assert_eq!(returned.failed_work(), Some(prefix + limit));
}

#[test]
fn admission_storage_one_short_returns_charged_work_and_storage_denial() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let prefix = 5;
    let mut work = Work::new(prefix + Service::ADMISSION_WORK);
    work.charge_work(prefix).unwrap();
    let (error, returned) = Service::admit_at(&REAPER, Account::new(work, Service::STORAGE - 1))
        .err()
        .expect("one-short admission storage was accepted")
        .into_parts();

    assert!(matches!(error, Failure::Resource(Resource::Storage(error))
        if error.actual() == Service::STORAGE && error.limit() == Service::STORAGE - 1));
    assert_eq!(returned.work(), prefix + Service::ADMISSION_WORK);
    assert_eq!((returned.storage(), returned.peak_storage()), (0, 0));
    assert_eq!(returned.failed_storage(), Some(Service::STORAGE));
    assert_eq!(returned.storage_limit(), Service::STORAGE - 1);
    assert_eq!(returned.work_limit(), prefix + Service::ADMISSION_WORK);
    assert_eq!(returned.failed_work(), None);
    assert_uninitialized(&REAPER);

    let mut service = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .unwrap();
    assert_eq!(service.shutdown().unwrap().storage(), 0);
    assert_empty(&REAPER);
}

#[test]
fn admission_rejects_live_storage_without_erasing_its_history() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut account = Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE);
    account.with_budget(|budget| {
        budget.reserve_storage(3).unwrap();
        budget.release_storage(2).unwrap();
        assert!(matches!(
            budget.reserve_storage(Service::STORAGE),
            Err(Resource::Storage(_))
        ));
    });
    let (error, returned) = Service::admit_at(&REAPER, account)
        .err()
        .expect("admission accepted live caller storage")
        .into_parts();
    assert_eq!(error, Failure::State);
    assert_eq!(returned.work(), Service::ADMISSION_WORK);
    assert_eq!((returned.storage(), returned.peak_storage()), (1, 3));
    assert_eq!(returned.failed_storage(), Some(Service::STORAGE + 1));
    assert_uninitialized(&REAPER);
}

#[test]
fn legacy_mode_excludes_native_admission_and_recovery_without_starting_a_thread() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    assert!(matches!(Service::recover_at(&REAPER), Err(Failure::State)));
    assert_uninitialized(&REAPER);
    *REAPER.mode.lock().unwrap() = ReaperMode::Legacy;

    let (error, returned) = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .err()
    .expect("native admission replaced legacy mode")
    .into_parts();
    assert_eq!(error, Failure::State);
    assert_eq!(returned.work(), Service::ADMISSION_WORK);
    assert_eq!((returned.storage(), returned.peak_storage()), (0, 0));
    assert_eq!(returned.failed_storage(), None);
    assert!(matches!(Service::recover_at(&REAPER), Err(Failure::State)));
    assert!(matches!(*REAPER.mode.lock().unwrap(), ReaperMode::Legacy));
    assert_empty(&REAPER);
}

#[test]
fn native_controller_is_unique_and_shutdown_permanently_excludes_both_modes() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut service = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .unwrap();
    let admitted = service.report().unwrap();
    assert!(matches!(Service::recover_at(&REAPER), Err(Failure::State)));
    assert!(matches!(
        REAPER.reserve(),
        Err(crate::ProtectedIssuerLaunchErrorV1::InvalidProcessState(_))
    ));
    let (error, returned) = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .err()
    .expect("second controller admission succeeded")
    .into_parts();
    assert_eq!(error, Failure::State);
    assert_eq!(returned.work(), Service::ADMISSION_WORK);
    assert_eq!((returned.storage(), returned.peak_storage()), (0, 0));
    assert_eq!(service.report().unwrap(), admitted);

    let returned = service.shutdown().unwrap();
    assert_eq!(
        (returned.work(), returned.storage()),
        (Service::ADMISSION_WORK, 0)
    );
    assert!(matches!(Service::recover_at(&REAPER), Err(Failure::State)));
    assert!(matches!(
        REAPER.reserve(),
        Err(crate::ProtectedIssuerLaunchErrorV1::InvalidProcessState(_))
    ));
    let (error, refused) = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .err()
    .expect("closed service was reopened")
    .into_parts();
    assert_eq!(error, Failure::State);
    assert_eq!(refused.work(), Service::ADMISSION_WORK);
    assert_eq!((refused.storage(), refused.peak_storage()), (0, 0));
    assert_eq!(service.report(), Err(Failure::State));
    assert_eq!(service.pump(1), Err(Failure::State));
    assert!(matches!(service.shutdown(), Err(Failure::State)));
    let mut request = Account::new(Work::new(Service::RESERVATION_WORK), 0);
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::State)
    ));
    assert_eq!(request.work(), Service::RESERVATION_WORK);
    drop(service);
    assert!(matches!(*REAPER.mode.lock().unwrap(), ReaperMode::Closed));
    assert_empty(&REAPER);
}

#[test]
fn controller_drop_and_recovery_preserve_the_original_cumulative_account() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let prefix = 11;
    let turn = Service::TURN_WORK + Service::CELL_WORK;
    let limit = prefix + Service::ADMISSION_WORK + turn + 2 * Service::RECOVERY_WORK;
    let peak = Service::STORAGE + 13;
    let mut work = Work::new(limit);
    work.charge_work(prefix).unwrap();
    assert_eq!(
        work.charge_work(limit).unwrap_err().actual(),
        prefix + limit
    );
    let mut account = Account::new(work, peak);
    account.with_budget(|budget| {
        budget.reserve_storage(peak).unwrap();
        budget.release_storage(peak).unwrap();
        assert!(matches!(
            budget.reserve_storage(peak + 1),
            Err(Resource::Storage(_))
        ));
    });
    let mut service = Service::admit_at(&REAPER, account).unwrap();
    let mut request = Account::new(Work::new(Service::RESERVATION_WORK), 0);
    let reservation = request
        .with_budget(|budget| service.reserve_launch(budget))
        .unwrap();
    let mut expected = service.pump(1).unwrap();
    assert_eq!(expected.work, prefix + Service::ADMISSION_WORK + turn);
    assert_eq!(expected.work_limit, limit);
    assert_eq!(expected.failed_work, Some(prefix + limit));
    assert_eq!(expected.storage, Service::STORAGE);
    assert!(expected.admission_open);
    assert_eq!(expected.peak_storage, peak);
    assert_eq!(expected.next_slot, 1);

    for recovery in 1..=2 {
        drop(service);
        {
            let mode = REAPER.mode.lock().unwrap();
            let ReaperMode::Native(native) = &*mode else {
                panic!("native account was discarded")
            };
            assert!(!native.leased);
            assert_eq!(native.report(), expected);
            assert_eq!(native.ledger.storage_limit(), peak);
            assert_eq!(native.ledger.failed_storage(), Some(peak + 1));
        }
        assert_eq!(REAPER.cells[0].state.load(Ordering::Acquire), RESERVED);
        assert!(REAPER.thread_started.get().is_none());
        service = Service::recover_at(&REAPER).unwrap();
        expected.work += Service::RECOVERY_WORK;
        expected.admission_open = recovery == 1;
        assert_eq!(service.report().unwrap(), expected);
        assert!(matches!(Service::recover_at(&REAPER), Err(Failure::State)));
        assert_eq!(service.report().unwrap(), expected);
    }

    drop(reservation);
    let mut returned = service.shutdown().unwrap();
    assert_eq!(returned.work(), limit);
    assert_eq!(returned.work_limit(), limit);
    assert_eq!(returned.failed_work(), Some(prefix + limit));
    assert_eq!((returned.storage(), returned.peak_storage()), (0, peak));
    assert_eq!(returned.storage_limit(), peak);
    assert_eq!(returned.failed_storage(), Some(peak + 1));
    let error = returned
        .with_budget(|budget| budget.charge_work(1))
        .unwrap_err();
    assert_work_refusal(error.into(), limit + 1, limit);
    assert_eq!(returned.failed_work(), Some(prefix + limit));
    assert_empty(&REAPER);
}

#[test]
fn recovery_work_one_short_retains_unleased_custody_and_cannot_reset_the_ledger() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK + Service::RECOVERY_WORK - 1;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let mut request = Account::new(Work::new(Service::RESERVATION_WORK), 0);
    let mut reservation = request
        .with_budget(|budget| service.reserve_launch(budget))
        .unwrap();
    reservation.slot.take().unwrap().defer(synthetic_child(0));
    let before = service.report().unwrap();
    drop(service);

    for _ in 0..2 {
        let error = Service::recover_at(&REAPER)
            .err()
            .expect("one-short recovery succeeded");
        assert_work_refusal(error, limit + 1, limit);
        let mode = REAPER.mode.lock().unwrap();
        let ReaperMode::Native(native) = &*mode else {
            panic!("recovery discarded custody")
        };
        assert!(!native.leased);
        assert_eq!(
            native.report(),
            Report {
                admission_open: false,
                failed_work: Some(limit + 1),
                ..before
            }
        );
    }
    let (error, returned) = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .err()
    .expect("fresh account replaced retained native custody")
    .into_parts();
    assert_eq!(error, Failure::State);
    assert_eq!(returned.storage(), 0);
    assert!(matches!(
        REAPER.reserve(),
        Err(crate::ProtectedIssuerLaunchErrorV1::InvalidProcessState(_))
    ));
    assert_record(&REAPER, 0, DEFERRED);
    assert!(REAPER.thread_started.get().is_none());
}

#[test]
fn busy_and_refused_shutdown_keep_reservations_and_quarantine_fully_charged() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let turn = Service::TURN_WORK + 2 * Service::CELL_WORK;
    let limit = Service::ADMISSION_WORK + turn + SHUTDOWN_WORK;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let mut request = Account::new(Work::new(3 * Service::RESERVATION_WORK), 0);
    let reserved = request
        .with_budget(|budget| service.reserve_launch(budget))
        .unwrap();
    let mut deferred = request
        .with_budget(|budget| service.reserve_launch(budget))
        .unwrap();
    let admitted = service.report().unwrap();
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    let draining = Report {
        admission_open: false,
        ..admitted
    };
    assert_eq!(service.report().unwrap(), draining);
    assert_eq!(REAPER.cells[0].state.load(Ordering::Acquire), RESERVED);
    assert_eq!(REAPER.cells[1].state.load(Ordering::Acquire), RESERVED);
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::AdmissionStopped)
    ));
    assert_eq!(request.work(), 3 * Service::RESERVATION_WORK);
    assert_eq!(service.report().unwrap(), draining);

    deferred.slot.take().unwrap().defer(synthetic_child(1));
    drop(reserved);
    let pumped = service.pump(2).unwrap();
    assert_record(&REAPER, 1, QUARANTINED);
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    let busy = Report {
        work: limit,
        ..pumped
    };
    assert_eq!(service.report().unwrap(), busy);
    let error = service
        .shutdown()
        .err()
        .expect("unfunded shutdown succeeded");
    assert_work_refusal(error, limit + SHUTDOWN_WORK, limit);
    let retained = Report {
        failed_work: Some(limit + SHUTDOWN_WORK),
        ..busy
    };
    assert_eq!(service.report().unwrap(), retained);
    assert_eq!(
        (retained.storage, retained.peak_storage),
        (Service::STORAGE, Service::STORAGE)
    );
    drop(service);
    {
        let mode = REAPER.mode.lock().unwrap();
        let ReaperMode::Native(native) = &*mode else {
            panic!("busy shutdown closed the pool")
        };
        assert!(!native.leased);
        assert_eq!(native.report(), retained);
    }
    assert_record(&REAPER, 1, QUARANTINED);
    assert_eq!(REAPER.cells[0].state.load(Ordering::Acquire), EMPTY);
    assert!(REAPER.thread_started.get().is_none());
}

#[test]
fn request_work_one_short_refuses_before_reserving_a_slot() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut service = Service::admit_at(
        &REAPER,
        Account::new(
            Work::new(Service::ADMISSION_WORK + MIN_TURN_WORK),
            Service::STORAGE,
        ),
    )
    .unwrap();
    let before = service.report().unwrap();
    assert!(before.admission_open);
    let prefix = 3;
    let limit = prefix + Service::RESERVATION_WORK - 1;
    let mut work = Work::new(limit);
    work.charge_work(prefix).unwrap();
    {
        let mut budget = Budget::new(&mut work, 0);
        let error = service
            .reserve_launch(&mut budget)
            .err()
            .expect("one-short request was admitted");
        assert_work_refusal(error, limit + 1, limit);
        assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
    }
    assert_eq!(work.work(), prefix);
    assert_eq!(work.failed_work(), Some(limit + 1));
    assert_eq!(service.report().unwrap(), before);
    assert_empty(&REAPER);
    assert_eq!(service.shutdown().unwrap().storage(), 0);
}

#[test]
fn exact_request_work_reserves_and_drop_rolls_back_only_capacity() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let mut service = Service::admit_at(
        &REAPER,
        Account::new(
            Work::new(Service::ADMISSION_WORK + MIN_TURN_WORK),
            Service::STORAGE,
        ),
    )
    .unwrap();
    let before = service.report().unwrap();
    assert_eq!(before.work, Service::ADMISSION_WORK);
    assert_eq!(before.work_limit - before.work, MIN_TURN_WORK);
    assert_eq!(before.failed_work, None);
    assert!(before.admission_open);
    let mut work = Work::new(Service::RESERVATION_WORK);
    let reservation = {
        let mut budget = Budget::new(&mut work, 0);
        let reservation = service.reserve_launch(&mut budget).unwrap();
        assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        assert_eq!(budget.failed_storage(), None);
        reservation
    };
    assert_eq!(work.work(), Service::RESERVATION_WORK);
    assert_eq!(work.failed_work(), None);
    assert!(std::ptr::eq(
        reservation.slot.as_ref().unwrap().cell,
        &REAPER.cells[0]
    ));
    assert_eq!(REAPER.cells[0].state.load(Ordering::Acquire), RESERVED);
    assert!(REAPER.cells[0].child.lock().unwrap().is_none());
    assert_eq!(service.report().unwrap(), before);

    drop(reservation);
    assert_eq!(work.work(), Service::RESERVATION_WORK);
    assert_eq!(service.report().unwrap(), before);
    assert_empty(&REAPER);
    let mut retry_work = Work::new(Service::RESERVATION_WORK);
    let retry = service
        .reserve_launch(&mut Budget::new(&mut retry_work, 0))
        .unwrap();
    assert!(std::ptr::eq(
        retry.slot.as_ref().unwrap().cell,
        &REAPER.cells[0]
    ));
    drop(retry);
    assert_empty(&REAPER);
    assert_eq!(service.shutdown().unwrap().storage(), 0);
}

#[test]
fn pump_rejects_invalid_bounds_without_charging_and_accepts_the_full_capacity() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    assert_eq!(CAPACITY, 64);
    let limit = Service::ADMISSION_WORK + Service::TURN_WORK + CAPACITY * Service::CELL_WORK;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let before = service.report().unwrap();
    for visits in [0, CAPACITY + 1, usize::MAX] {
        assert_eq!(service.pump(visits), Err(Failure::InvalidTurn));
        assert_eq!(service.report().unwrap(), before);
        assert_empty(&REAPER);
    }
    assert_eq!(
        service.pump(CAPACITY).unwrap(),
        Report {
            work: limit,
            admission_open: false,
            ..before
        }
    );
    let returned = service.shutdown().unwrap();
    assert_eq!((returned.work(), returned.storage()), (limit, 0));
    assert_eq!(returned.peak_storage(), Service::STORAGE);
    assert_empty(&REAPER);
}

#[test]
fn upfront_pump_refusal_preserves_custody_and_allows_a_smaller_turn_after_admission_stops() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK + Service::TURN_WORK + 2 * Service::CELL_WORK - 1;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let mut request = Account::new(Work::new(3 * Service::RESERVATION_WORK), 0);
    for index in 0..2 {
        let mut reservation = request
            .with_budget(|budget| service.reserve_launch(budget))
            .unwrap();
        let mut child = synthetic_child(index);
        if index == 0 {
            // This record has no actual child; its terminal state needs no wait syscall.
            child.terminal_reaped();
        }
        reservation.slot.take().unwrap().defer(child);
    }
    let before = service.report().unwrap();
    assert_work_refusal(service.pump(2).unwrap_err(), limit + 1, limit);
    let stopped = Report {
        admission_open: false,
        failed_work: Some(limit + 1),
        ..before
    };
    assert_eq!(service.report().unwrap(), stopped);
    assert_record(&REAPER, 0, DEFERRED);
    assert_record(&REAPER, 1, DEFERRED);
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::AdmissionStopped)
    ));
    assert_eq!(request.work(), 3 * Service::RESERVATION_WORK);
    assert_eq!(service.report().unwrap(), stopped);

    let partial = Report {
        work: Service::ADMISSION_WORK + Service::TURN_WORK + Service::CELL_WORK,
        next_slot: 1,
        ..stopped
    };
    assert_eq!(service.pump(1).unwrap(), partial);
    assert_eq!(REAPER.cells[0].state.load(Ordering::Acquire), EMPTY);
    assert!(REAPER.cells[0].child.lock().unwrap().is_none());
    assert_record(&REAPER, 1, DEFERRED);
    assert_work_refusal(
        service.pump(1).unwrap_err(),
        partial.work + Service::TURN_WORK + Service::CELL_WORK,
        limit,
    );
    assert_eq!(service.report().unwrap(), partial);
    assert_record(&REAPER, 1, DEFERRED);
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    assert_eq!(service.report().unwrap(), partial);
    assert!(REAPER.thread_started.get().is_none());
}

#[test]
fn rotating_turns_reach_all_64_cells_and_quarantine_never_frees_capacity() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    assert_eq!(CAPACITY, 64);
    let turns = [CAPACITY - 1, 2, CAPACITY, CAPACITY];
    let limit = Service::ADMISSION_WORK
        + MIN_TURN_WORK
        + turns
            .iter()
            .map(|visits| Service::TURN_WORK + visits * Service::CELL_WORK)
            .sum::<usize>();
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let mut request = Account::new(
        Work::new((CAPACITY + 1 + turns.len()) * Service::RESERVATION_WORK),
        0,
    );
    for index in 0..CAPACITY {
        let mut reservation = request
            .with_budget(|budget| service.reserve_launch(budget))
            .unwrap();
        reservation
            .slot
            .take()
            .unwrap()
            .defer(synthetic_child(index));
        assert_record(&REAPER, index, DEFERRED);
    }
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::Capacity)
    ));
    let mut expected = service.report().unwrap();
    for (turn, visits) in turns.into_iter().enumerate() {
        expected.work += Service::TURN_WORK + visits * Service::CELL_WORK;
        expected.next_slot = (expected.next_slot + visits) % CAPACITY;
        assert_eq!(service.pump(visits).unwrap(), expected);
        for index in 0..CAPACITY {
            let state = if turn == 0 && index == CAPACITY - 1 {
                DEFERRED
            } else {
                QUARANTINED
            };
            assert_record(&REAPER, index, state);
        }
        assert!(matches!(
            request.with_budget(|budget| service.reserve_launch(budget)),
            Err(Failure::Capacity)
        ));
        assert_eq!(service.report().unwrap(), expected);
        assert_eq!(
            request.work(),
            (CAPACITY + turn + 2) * Service::RESERVATION_WORK
        );
        assert_eq!((request.storage(), request.peak_storage()), (0, 0));
    }
    assert_eq!(expected.work, limit - MIN_TURN_WORK);
    assert_eq!(expected.work_limit, limit);
    assert_eq!(expected.failed_work, None);
    assert_eq!(expected.next_slot, 1);
    assert_eq!(
        (expected.storage, expected.peak_storage),
        (Service::STORAGE, Service::STORAGE)
    );
    assert!(expected.admission_open);
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    expected.admission_open = false;
    assert_eq!(service.report().unwrap(), expected);
    assert!(REAPER.thread_started.get().is_none());
}

#[test]
fn pumping_a_preterminal_synthetic_record_reclaims_exactly_its_full_pool_slot() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK
        + Service::TURN_WORK
        + CAPACITY * Service::CELL_WORK
        + MIN_TURN_WORK;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let mut request = Account::new(Work::new((CAPACITY + 2) * Service::RESERVATION_WORK), 0);
    let mut reservations: Vec<_> = (0..CAPACITY)
        .map(|_| {
            request
                .with_budget(|budget| service.reserve_launch(budget))
                .unwrap()
        })
        .collect();
    let terminal = CAPACITY / 2;
    let mut child = synthetic_child(terminal);
    // Only a synthetic record is marked terminal here; there is no process to reap.
    child.terminal_reaped();
    reservations[terminal].slot.take().unwrap().defer(child);
    assert_record(&REAPER, terminal, DEFERRED);
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::Capacity)
    ));
    let before = service.report().unwrap();
    let pumped = Report {
        work: limit - MIN_TURN_WORK,
        ..before
    };
    assert_eq!(service.pump(CAPACITY).unwrap(), pumped);
    assert_eq!(pumped.work_limit - pumped.work, MIN_TURN_WORK);
    assert_eq!(pumped.failed_work, None);
    assert!(pumped.admission_open);
    for (index, cell) in REAPER.cells.iter().enumerate() {
        assert_eq!(
            cell.state.load(Ordering::Acquire),
            if index == terminal { EMPTY } else { RESERVED }
        );
        assert!(cell.child.lock().unwrap().is_none());
    }
    let replacement = request
        .with_budget(|budget| service.reserve_launch(budget))
        .unwrap();
    assert!(std::ptr::eq(
        replacement.slot.as_ref().unwrap().cell,
        &REAPER.cells[terminal]
    ));
    assert!(
        REAPER
            .cells
            .iter()
            .all(|cell| cell.state.load(Ordering::Acquire) == RESERVED)
    );
    assert_eq!(request.work(), (CAPACITY + 2) * Service::RESERVATION_WORK);
    assert_eq!(service.report().unwrap(), pumped);
    drop(reservations);
    assert_eq!(
        REAPER.cells[terminal].state.load(Ordering::Acquire),
        RESERVED
    );
    drop(replacement);
    assert_empty(&REAPER);
    let returned = service.shutdown().unwrap();
    assert_eq!(
        (returned.work(), returned.storage()),
        (limit - MIN_TURN_WORK, 0)
    );
    assert_eq!(returned.work_limit(), limit);
    assert_eq!(returned.peak_storage(), Service::STORAGE);
}

#[test]
fn dropping_an_empty_exhausted_service_retains_the_pool_when_recovery_is_unfunded() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let service = Service::admit_at(
        &REAPER,
        Account::new(Work::new(Service::ADMISSION_WORK), Service::STORAGE),
    )
    .unwrap();
    let before = service.report().unwrap();
    drop(service);
    assert_empty(&REAPER);
    let denied = Service::ADMISSION_WORK + Service::RECOVERY_WORK;
    for _ in 0..2 {
        let error = Service::recover_at(&REAPER)
            .err()
            .expect("exhausted recovery succeeded");
        assert_work_refusal(error, denied, Service::ADMISSION_WORK);
        let mode = REAPER.mode.lock().unwrap();
        let ReaperMode::Native(native) = &*mode else {
            panic!("empty pool lost its charged account")
        };
        assert!(!native.leased);
        assert_eq!(
            native.report(),
            Report {
                admission_open: false,
                failed_work: Some(denied),
                ..before
            }
        );
    }
    assert!(matches!(
        REAPER.reserve(),
        Err(crate::ProtectedIssuerLaunchErrorV1::InvalidProcessState(_))
    ));
    assert_empty(&REAPER);
}

#[test]
fn shutdown_with_an_undercharged_storage_floor_refuses_to_discard_the_account() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK + SHUTDOWN_WORK;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let mut request = Account::new(Work::new(Service::RESERVATION_WORK), 0);
    let reserved = request
        .with_budget(|budget| service.reserve_launch(budget))
        .unwrap();
    {
        let mut mode = REAPER.mode.lock().unwrap();
        let ReaperMode::Native(native) = &mut *mode else {
            panic!("missing native account")
        };
        // Inject an internal accounting defect; public callers cannot alter this ledger.
        native
            .ledger
            .with_budget(|budget| budget.release_storage(1))
            .unwrap();
    }
    let before = service.report().unwrap();
    assert_eq!(before.storage, Service::STORAGE - 1);
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    assert_eq!(REAPER.cells[0].state.load(Ordering::Acquire), RESERVED);
    drop(reserved);
    assert_empty(&REAPER);
    assert!(matches!(
        service.shutdown(),
        Err(Failure::Resource(Resource::Accounting))
    ));
    let retained = Report {
        work: limit,
        admission_open: false,
        ..before
    };
    assert_eq!(service.report().unwrap(), retained);
    assert_eq!(retained.peak_storage, Service::STORAGE);
    {
        let mode = REAPER.mode.lock().unwrap();
        let ReaperMode::Native(native) = &*mode else {
            panic!("accounting refusal closed the pool")
        };
        assert!(native.leased);
        assert_eq!(native.report(), retained);
    }
    drop(service);
    {
        let mode = REAPER.mode.lock().unwrap();
        let ReaperMode::Native(native) = &*mode else {
            panic!("controller Drop discarded the account")
        };
        assert!(!native.leased);
        assert_eq!(native.report(), retained);
    }
    assert_empty(&REAPER);
}

#[test]
fn admission_with_one_short_minimum_turn_stops_requests_without_a_work_denial() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let prefix = 5;
    let limit = prefix + Service::ADMISSION_WORK + MIN_TURN_WORK - 1;
    let mut work = Work::new(limit);
    work.charge_work(prefix).unwrap();
    let mut service = Service::admit_at(&REAPER, Account::new(work, Service::STORAGE)).unwrap();
    let expected = Report {
        work: prefix + Service::ADMISSION_WORK,
        work_limit: limit,
        failed_work: None,
        storage: Service::STORAGE,
        peak_storage: Service::STORAGE,
        admission_open: false,
        next_slot: 0,
    };
    assert_eq!(service.report().unwrap(), expected);
    let mut request = Account::new(Work::new(Service::RESERVATION_WORK), 0);
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::AdmissionStopped)
    ));
    assert_eq!(request.work(), Service::RESERVATION_WORK);
    assert_eq!(request.failed_work(), None);
    assert_eq!(service.report().unwrap(), expected);
    assert_empty(&REAPER);
    let returned = service.shutdown().unwrap();
    assert_eq!(returned.work(), prefix + Service::ADMISSION_WORK);
    assert_eq!(returned.work_limit(), limit);
    assert_eq!(returned.failed_work(), None);
    assert_eq!(
        (returned.storage(), returned.peak_storage()),
        (0, Service::STORAGE)
    );
}

#[test]
fn successful_pump_with_exact_minimum_turn_remaining_keeps_admission_until_spent() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK + 2 * MIN_TURN_WORK;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let before = service.report().unwrap();
    assert!(before.admission_open);
    assert_eq!(before.failed_work, None);
    let funded = Report {
        work: Service::ADMISSION_WORK + MIN_TURN_WORK,
        next_slot: 1,
        ..before
    };
    assert_eq!(service.pump(1).unwrap(), funded);
    assert_eq!(funded.work_limit - funded.work, MIN_TURN_WORK);
    let mut request = Account::new(Work::new(2 * Service::RESERVATION_WORK), 0);
    let reserved = request
        .with_budget(|budget| service.reserve_launch(budget))
        .unwrap();
    let exhausted = Report {
        work: limit,
        admission_open: false,
        next_slot: 2,
        ..funded
    };
    assert_eq!(service.pump(1).unwrap(), exhausted);
    assert_eq!(REAPER.cells[0].state.load(Ordering::Acquire), RESERVED);
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::AdmissionStopped)
    ));
    assert_eq!(request.work(), 2 * Service::RESERVATION_WORK);
    assert_eq!(request.failed_work(), None);
    assert_eq!(service.report().unwrap(), exhausted);
    drop(reserved);
    assert_empty(&REAPER);
    let returned = service.shutdown().unwrap();
    assert_eq!(returned.work(), limit);
    assert_eq!(returned.failed_work(), None);
    assert_eq!(
        (returned.storage(), returned.peak_storage()),
        (0, Service::STORAGE)
    );
}

#[test]
fn successful_pump_with_one_short_minimum_turn_remaining_stops_without_a_denial() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK + 2 * MIN_TURN_WORK - 1;
    let mut service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let before = service.report().unwrap();
    assert!(before.admission_open);
    assert_eq!(before.failed_work, None);
    let stopped = Report {
        work: Service::ADMISSION_WORK + MIN_TURN_WORK,
        admission_open: false,
        next_slot: 1,
        ..before
    };
    assert_eq!(service.pump(1).unwrap(), stopped);
    assert_eq!(stopped.work_limit - stopped.work, MIN_TURN_WORK - 1);
    let mut request = Account::new(Work::new(Service::RESERVATION_WORK), 0);
    assert!(matches!(
        request.with_budget(|budget| service.reserve_launch(budget)),
        Err(Failure::AdmissionStopped)
    ));
    assert_eq!(request.work(), Service::RESERVATION_WORK);
    assert_eq!(request.failed_work(), None);
    assert_eq!(service.report().unwrap(), stopped);
    assert_empty(&REAPER);
    let returned = service.shutdown().unwrap();
    assert_eq!(returned.work(), stopped.work);
    assert_eq!(returned.work_limit(), limit);
    assert_eq!(returned.failed_work(), None);
    assert_eq!(
        (returned.storage(), returned.peak_storage()),
        (0, Service::STORAGE)
    );
}

#[test]
fn concurrent_recovery_grants_one_controller_and_charges_the_original_account_once() {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    let limit = Service::ADMISSION_WORK + Service::RECOVERY_WORK;
    let service =
        Service::admit_at(&REAPER, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    let before = service.report().unwrap();
    drop(service);
    let barrier = Barrier::new(2);
    let (first, second) = std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            let result = Service::recover_at(&REAPER);
            // Retain the result until both attempts finish, keeping the winner leased.
            barrier.wait();
            result
        });
        let second = scope.spawn(|| {
            barrier.wait();
            let result = Service::recover_at(&REAPER);
            barrier.wait();
            result
        });
        (first.join().unwrap(), second.join().unwrap())
    });
    let mut winner = match (first, second) {
        (Ok(service), Err(Failure::State)) | (Err(Failure::State), Ok(service)) => service,
        _ => {
            panic!("concurrent recovery must grant one controller and refuse the other with State")
        }
    };
    assert_eq!(
        winner.report().unwrap(),
        Report {
            work: before.work + Service::RECOVERY_WORK,
            admission_open: false,
            ..before
        }
    );
    assert_eq!(winner.report().unwrap().failed_work, None);
    assert_empty(&REAPER);
    let returned = winner.shutdown().unwrap();
    assert_eq!(returned.work(), limit);
    assert_eq!(returned.work_limit(), limit);
    assert_eq!(returned.failed_work(), None);
    assert_eq!(
        (returned.storage(), returned.peak_storage()),
        (0, Service::STORAGE)
    );
    assert!(matches!(*REAPER.mode.lock().unwrap(), ReaperMode::Closed));
    assert_empty(&REAPER);
}
