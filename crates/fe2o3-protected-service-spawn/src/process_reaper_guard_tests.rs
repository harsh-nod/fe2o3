use super::*;
use crate::process_cleanup::ChildCleanupV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{FlockOperation, MemfdFlags, flock, memfd_create};
use rustix::io::Errno;
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const MIN_TURN: usize = Service::TURN_WORK + Service::CELL_WORK;
const LIMIT: usize = (1 << 20)
    + 2 * (Service::TURN_WORK + CAPACITY * Service::CELL_WORK)
    + Service::RESERVATION_WORK
    + Service::STORAGE;
const MARKER: &str = "FE2O3_PRIVATE_CLEANUP_GUARD_TEST";

struct GuardChild(Option<Child>);
impl Drop for GuardChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn isolated_guard_custody_schedules() {
    if std::env::var_os(MARKER).is_some() {
        guard_moves_between_original_accounts_and_only_empty_shutdown_releases_it();
        late_error_unwind_and_post_exec_quarantine_preserve_guard_until_terminal_retirement();
        guard_cannot_be_replaced_or_installed_over_an_occupied_slot();
        guard_refusals_close_input_and_preserve_request_and_service_denial_history();
        bounded_cleanup_exhaustion_and_failed_recovery_keep_guard_charged();
        guard_clone_exact_accounts_and_independent_retention();
        guard_clone_short_quotas_precede_duplication();
        guard_clone_requires_existing_guard_and_active_controller();
        guard_clone_checks_persistent_storage();
        guard_clone_allows_occupied_draining_pool_and_recovery();
        guard_clone_io_error_and_unwind_preserve_custody_and_history();
        guard_clone_overflow_precedes_duplication();
        println!("\n{MARKER}:complete");
        return;
    }
    // A concurrent harness fork may temporarily inherit any CLOEXEC flock alias.
    // All guards are created after re-exec in this single-test, non-forking process.
    let mut child = GuardChild(Some(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_reaper::native::guard_tests::isolated_guard_custody_schedules",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env(MARKER, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.0.as_mut().unwrap().try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "guard custody subprocess timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.0.take().unwrap().wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line == format!("{MARKER}:complete")),
        "all custody schedules must complete, not a zero-test filter"
    );
}

// These are real independent open descriptions and kernel flock exclusion,
// not protected deployment admission or evidence of a successful child wait.
fn locked_guard() -> (File, File) {
    let guard = File::from(memfd_create(c"cleanup-guard-test", MemfdFlags::CLOEXEC).unwrap());
    let observer = File::open(format!("/proc/self/fd/{}", guard.as_raw_fd())).unwrap();
    flock(&guard, FlockOperation::NonBlockingLockShared).unwrap();
    (guard, observer)
}

fn assert_locked(observer: &File) {
    assert_eq!(
        flock(observer, FlockOperation::NonBlockingLockExclusive),
        Err(Errno::AGAIN)
    );
}

fn assert_released(observer: &File) {
    flock(observer, FlockOperation::NonBlockingLockExclusive).unwrap();
    flock(observer, FlockOperation::Unlock).unwrap();
}

fn pool(limit: usize) -> (&'static DeferredReaperV1, Service) {
    let reaper = Box::leak(Box::new(DeferredReaperV1::new()));
    let service =
        Service::admit_at(reaper, Account::new(Work::new(limit), Service::STORAGE)).unwrap();
    (reaper, service)
}

fn guard_moves_between_original_accounts_and_only_empty_shutdown_releases_it() {
    let (reaper, mut service) = pool(LIMIT);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, Service::GUARD_FILE_STORAGE);
    b.reserve_storage(Service::GUARD_FILE_STORAGE).unwrap();
    let (guard, observer) = locked_guard();
    service.retain_deployment_guard(guard, &mut b).unwrap();
    assert_eq!(b.work(), Service::GUARD_WORK);
    assert_eq!(b.storage(), Service::GUARD_FILE_STORAGE);
    b.release_storage(Service::GUARD_FILE_STORAGE).unwrap();
    let report = service.report().unwrap();
    assert_eq!(report.work, Service::ADMISSION_WORK + Service::GUARD_WORK);
    assert_eq!(report.storage, Service::STORAGE);
    drop(service);
    assert_locked(&observer);
    let mut service = Service::recover_at(reaper).unwrap();
    assert_eq!(
        service.report().unwrap().work,
        report.work + Service::RECOVERY_WORK
    );
    assert_eq!(service.shutdown().unwrap().storage(), 0);
    assert_released(&observer);
}

fn late_error_unwind_and_post_exec_quarantine_preserve_guard_until_terminal_retirement() {
    for unwind in [false, true] {
        let (reaper, mut service) = pool(LIMIT);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Service::GUARD_FILE_STORAGE).unwrap();
        let (guard, observer) = locked_guard();
        service.retain_deployment_guard(guard, &mut b).unwrap();
        b.release_storage(Service::GUARD_FILE_STORAGE).unwrap();
        let slot = service.reserve_launch(&mut b).unwrap().into_slot();
        let mut child =
            ChildCleanupV1::new(None, rustix::process::Pid::from_raw(1000).unwrap(), None);
        child.release_spawn_after_exec();
        slot.defer(child);
        let caught = catch_unwind(AssertUnwindSafe(|| {
            // A late request refusal cannot draw from or reset cleanup funding.
            assert!(b.charge_work(LIMIT).is_err());
            if unwind {
                panic!("after native dispatch");
            }
        }));
        assert_eq!(caught.is_err(), unwind);
        drop(service);
        assert_locked(&observer);
        let mut service = Service::recover_at(reaper).unwrap();
        service.pump(CAPACITY).unwrap();
        assert_eq!(
            reaper.cells[0].state.load(Ordering::Acquire),
            super::super::QUARANTINED
        );
        assert!(matches!(service.shutdown(), Err(Failure::Busy)));
        assert_eq!(service.report().unwrap().storage, Service::STORAGE);
        assert_locked(&observer);
        // Inject the trusted terminal event for this synthetic, descriptor-free
        // record. No signal, real child, or successful OS wait is claimed here.
        reaper.cells[0]
            .child
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .child_mut()
            .terminal_reaped();
        reaper.cells[0]
            .state
            .store(super::super::DEFERRED, Ordering::Release);
        service.pump(CAPACITY).unwrap();
        assert_locked(&observer);
        assert_eq!(service.shutdown().unwrap().storage(), 0);
        assert_released(&observer);
    }
}

fn guard_cannot_be_replaced_or_installed_over_an_occupied_slot() {
    for occupied in [false, true] {
        let (_, mut service) = pool(LIMIT);
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Service::GUARD_FILE_STORAGE).unwrap();
        let (first, first_observer) = locked_guard();
        let slot = if occupied {
            drop(first);
            Some(service.reserve_launch(&mut b).unwrap())
        } else {
            service.retain_deployment_guard(first, &mut b).unwrap();
            None
        };
        let (next, next_observer) = locked_guard();
        assert_eq!(
            service.retain_deployment_guard(next, &mut b),
            Err(Failure::State)
        );
        assert_released(&next_observer);
        if !occupied {
            assert_locked(&first_observer);
        }
        drop(slot);
        service.shutdown().unwrap();
        assert_released(&first_observer);
    }
}

fn guard_refusals_close_input_and_preserve_request_and_service_denial_history() {
    for cause in 0..3 {
        let service_limit = if cause == 2 {
            Service::ADMISSION_WORK + Service::GUARD_WORK - 1
        } else {
            LIMIT
        };
        let (_, mut service) = pool(service_limit);
        let request_limit = if cause == 0 {
            Service::GUARD_WORK - 1
        } else {
            LIMIT
        };
        let mut work = Work::new(request_limit);
        let mut b = Budget::new(&mut work, LIMIT);
        let floor = Service::GUARD_FILE_STORAGE - usize::from(cause == 1);
        b.reserve_storage(floor).unwrap();
        assert!(b.reserve_storage(LIMIT).is_err());
        let prior = b.failed_storage();
        let (guard, observer) = locked_guard();
        assert!(matches!(
            service.retain_deployment_guard(guard, &mut b),
            Err(Failure::Resource(_))
        ));
        assert_released(&observer);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.failed_storage(), prior);
        if cause == 0 {
            assert_eq!(b.failed_work(), Some(Service::GUARD_WORK));
        }
        if cause == 2 {
            assert_eq!(
                service.report().unwrap().failed_work,
                Some(service_limit + 1)
            );
        } else {
            assert_eq!(service.report().unwrap().work, Service::ADMISSION_WORK);
        }
        service.shutdown().unwrap();
    }
}

fn bounded_cleanup_exhaustion_and_failed_recovery_keep_guard_charged() {
    let limit =
        Service::ADMISSION_WORK + Service::GUARD_WORK + Service::TURN_WORK + Service::CELL_WORK;
    let (reaper, mut service) = pool(limit);
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(Service::GUARD_FILE_STORAGE).unwrap();
    let (guard, observer) = locked_guard();
    service.retain_deployment_guard(guard, &mut b).unwrap();
    let slot = service.reserve_launch(&mut b).unwrap();
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    assert!(matches!(service.pump(CAPACITY), Err(Failure::Resource(_))));
    // The larger domain allowance still funds one smaller turn after refusal.
    service.pump(1).unwrap();
    let before = service.report().unwrap();
    assert_eq!(before.storage, Service::STORAGE);
    drop(service);
    assert!(matches!(
        Service::recover_at(reaper),
        Err(Failure::Resource(_))
    ));
    assert_locked(&observer);
    let mode = reaper.mode.lock().unwrap();
    let ReaperMode::Native(native) = &*mode else {
        panic!("custody lost");
    };
    assert_eq!(native.ledger.storage(), Service::STORAGE);
    assert_eq!(native.ledger.failed_work(), before.failed_work);
    drop(mode);
    drop(slot);
    assert_locked(&observer);
    // This process-lifetime test pool deliberately stays charged and closed to
    // admission, like production exhaustion. The test process releases its memfd.
}

fn installed_pool(limit: usize) -> (&'static DeferredReaperV1, Service, File) {
    let (reaper, mut service) = pool(limit);
    let (guard, observer) = locked_guard();
    let mut w = Work::new(Service::GUARD_WORK);
    let mut b = Budget::new(&mut w, Service::GUARD_FILE_STORAGE);
    b.reserve_storage(Service::GUARD_FILE_STORAGE).unwrap();
    service.retain_deployment_guard(guard, &mut b).unwrap();
    b.release_storage(Service::GUARD_FILE_STORAGE).unwrap();
    (reaper, service, observer)
}

fn references(observer: &File) -> usize {
    // The independent open description pins this unique inode without sharing
    // its flock. No other test runs or forks inside this helper process.
    let stat = rustix::fs::fstat(observer).unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter(
            |entry| match std::fs::metadata(entry.as_ref().unwrap().path()) {
                Ok(m) => (m.dev(), m.ino()) == (stat.st_dev, stat.st_ino),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => panic!("inspect guard references: {e}"),
            },
        )
        .count()
}

fn assert_guard_clone(file: &File, observer: &File) {
    assert!(file.as_raw_fd() >= 256);
    assert_eq!(
        rustix::io::fcntl_getfd(file).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
    let a = rustix::fs::fstat(file).unwrap();
    let b = rustix::fs::fstat(observer).unwrap();
    assert_eq!(
        (a.st_dev, a.st_ino, a.st_mode),
        (b.st_dev, b.st_ino, b.st_mode)
    );
    assert_locked(observer);
}

fn guard_clone_exact_accounts_and_independent_retention() {
    let prefix = MIN_TURN.saturating_sub(Service::GUARD_CLONE_WORK);
    let service_limit =
        Service::ADMISSION_WORK + Service::GUARD_WORK + prefix + Service::GUARD_CLONE_WORK;
    let (reaper, mut service, observer) = installed_pool(service_limit);
    {
        let mut mode = reaper.mode.lock().unwrap();
        let ReaperMode::Native(native) = &mut *mode else {
            panic!("native pool")
        };
        native.charge(prefix).unwrap();
    }
    let mut w = Work::new(17 + Service::GUARD_CLONE_WORK);
    let peak = 37 + Service::GUARD_CLONE_SCRATCH + Service::GUARD_FILE_STORAGE;
    let mut b = Budget::new(&mut w, peak);
    b.charge_work(17).unwrap();
    b.reserve_storage(37).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (file, charge) = service.try_clone_deployment_guard(&mut b).unwrap();
    assert_eq!(charge.additional_storage(), Service::GUARD_FILE_STORAGE);
    assert_eq!(b.storage(), 37);
    assert_eq!(b.peak_storage(), peak);
    assert_eq!(b.work(), 17 + Service::GUARD_CLONE_WORK);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!((b.failed_work(), b.failed_storage()), (None, None));
    let report = service.report().unwrap();
    assert_eq!(report.work, service_limit);
    assert_eq!(report.storage, Service::STORAGE);
    assert_eq!(report.peak_storage, Service::STORAGE);
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_guard_clone(&file, &observer);
    assert_eq!(references(&observer), 3);
    // Exact remaining work may close admission, but cannot discard the guard.
    assert!(!report.admission_open);
    let account = service.shutdown().unwrap();
    assert_eq!(account.storage(), 0);
    assert_eq!(account.work(), service_limit);
    assert_eq!(references(&observer), 2);
    assert_locked(&observer);
    drop(file);
    b.release_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), 37);
    assert_eq!(references(&observer), 1);
    assert_released(&observer);
}

fn guard_clone_short_quotas_precede_duplication() {
    for cause in 0..4 {
        let prefix = if cause == 3 {
            MIN_TURN.saturating_sub(Service::GUARD_CLONE_WORK - 1)
        } else {
            0
        };
        let initial = Service::ADMISSION_WORK + Service::GUARD_WORK + prefix;
        let limit = if cause == 3 {
            initial + Service::GUARD_CLONE_WORK - 1
        } else {
            LIMIT
        };
        let (reaper, mut service, observer) = installed_pool(limit);
        {
            let mut mode = reaper.mode.lock().unwrap();
            let ReaperMode::Native(native) = &mut *mode else {
                panic!("native pool")
            };
            native.charge(prefix).unwrap();
        }
        let work = if cause == 0 {
            17 + Service::GUARD_CLONE_WORK - 1
        } else {
            LIMIT
        };
        let peak = 37 + Service::GUARD_CLONE_SCRATCH + Service::GUARD_FILE_STORAGE;
        let storage = match cause {
            1 => 37 + Service::GUARD_CLONE_SCRATCH - 1,
            2 => peak - 1,
            _ => peak,
        };
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, storage);
        b.charge_work(17).unwrap();
        b.reserve_storage(37).unwrap();
        let mut calls = 0;
        let result = service.try_clone_deployment_guard_with(&mut b, |_| {
            calls += 1;
            Err(Errno::INTR)
        });
        assert!(matches!(result, Err(Failure::Resource(_))));
        assert_eq!(calls, 0);
        assert_eq!(b.storage(), 37);
        assert_eq!(
            b.work(),
            if cause == 0 {
                17
            } else {
                17 + Service::GUARD_CLONE_WORK
            }
        );
        assert_eq!(
            b.peak_storage(),
            match cause {
                0 | 1 => 37,
                2 => 37 + Service::GUARD_CLONE_SCRATCH,
                _ => peak,
            }
        );
        assert_eq!(
            b.failed_work(),
            (cause == 0).then_some(17 + Service::GUARD_CLONE_WORK)
        );
        assert_eq!(
            b.failed_storage(),
            match cause {
                1 => Some(37 + Service::GUARD_CLONE_SCRATCH),
                2 => Some(peak),
                _ => None,
            }
        );
        let report = service.report().unwrap();
        assert_eq!(report.work, initial);
        assert_eq!(report.storage, Service::STORAGE);
        assert_eq!(
            report.failed_work,
            (cause == 3).then_some(initial + Service::GUARD_CLONE_WORK)
        );
        assert_eq!(references(&observer), 2);
        assert_locked(&observer);
        service.shutdown().unwrap();
        assert_released(&observer);
    }
}

fn guard_clone_requires_existing_guard_and_active_controller() {
    let (_, mut service) = pool(LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let mut calls = 0;
    assert!(matches!(
        service.try_clone_deployment_guard_with(&mut b, |_| {
            calls += 1;
            Err(Errno::INTR)
        }),
        Err(Failure::State)
    ));
    assert_eq!(calls, 0);
    assert_eq!(b.storage(), 0);
    assert_eq!(
        service.report().unwrap().work,
        Service::ADMISSION_WORK + Service::GUARD_CLONE_WORK
    );
    let account = service.shutdown().unwrap();
    assert!(matches!(
        service.try_clone_deployment_guard_with(&mut b, |_| {
            calls += 1;
            Err(Errno::INTR)
        }),
        Err(Failure::State)
    ));
    assert_eq!(calls, 0);
    assert_eq!(b.work(), 2 * Service::GUARD_CLONE_WORK);
    assert_eq!(
        account.work(),
        Service::ADMISSION_WORK + Service::GUARD_CLONE_WORK
    );
    assert_eq!(account.storage(), 0);
}

fn guard_clone_checks_persistent_storage() {
    let (reaper, mut service, observer) = installed_pool(LIMIT);
    {
        let mut mode = reaper.mode.lock().unwrap();
        let ReaperMode::Native(native) = &mut *mode else {
            panic!("native pool");
        };
        // Private corruption fixture: even a live guard cannot bypass its floor.
        native.ledger.with_budget(|b| b.release_storage(1)).unwrap();
    }
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let mut calls = 0;
    assert!(matches!(
        service.try_clone_deployment_guard_with(&mut b, |_| {
            calls += 1;
            Err(Errno::INTR)
        }),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(calls, 0);
    assert_eq!(b.storage(), 0);
    assert_eq!(references(&observer), 2);
    assert_locked(&observer);
    {
        let mut mode = reaper.mode.lock().unwrap();
        let ReaperMode::Native(native) = &mut *mode else {
            panic!("native pool");
        };
        native.ledger.with_budget(|b| b.reserve_storage(1)).unwrap();
    }
    service.shutdown().unwrap();
    assert_released(&observer);
}

fn guard_clone_allows_occupied_draining_pool_and_recovery() {
    let (reaper, mut service, observer) = installed_pool(LIMIT);
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let slot = service.reserve_launch(&mut b).unwrap();
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
    let before = service.report().unwrap();
    let (first, c) = service.try_clone_deployment_guard(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    assert_guard_clone(&first, &observer);
    drop(service);
    assert_locked(&observer);
    let mut service = Service::recover_at(reaper).unwrap();
    assert_eq!(
        service.report().unwrap().work,
        before.work + Service::GUARD_CLONE_WORK + Service::RECOVERY_WORK
    );
    assert!(!service.report().unwrap().admission_open);
    let (second, c2) = service.try_clone_deployment_guard(&mut b).unwrap();
    b.reserve_storage(c2.additional_storage()).unwrap();
    assert_guard_clone(&second, &observer);
    // Duplicates share an open description, unlike the independently reopened observer.
    rustix::fs::seek(&first, rustix::fs::SeekFrom::Start(17)).unwrap();
    assert_eq!(
        rustix::fs::seek(&second, rustix::fs::SeekFrom::Current(0)).unwrap(),
        17
    );
    drop((first, second));
    b.release_storage(c.additional_storage() + c2.additional_storage())
        .unwrap();
    assert_eq!(references(&observer), 2);
    assert_locked(&observer);
    drop(slot);
    service.shutdown().unwrap();
    assert_released(&observer);
    assert_eq!(b.storage(), 0);
}

fn guard_clone_io_error_and_unwind_preserve_custody_and_history() {
    for cause in 0..3 {
        let (reaper, mut service, observer) = installed_pool(LIMIT);
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.charge_work(17).unwrap();
        b.reserve_storage(37).unwrap();
        assert!(b.charge_work(LIMIT).is_err());
        assert!(b.reserve_storage(LIMIT).is_err());
        let history = (b.failed_work(), b.failed_storage());
        let ledger = b.work_ledger_identity_v1();
        {
            let mut mode = reaper.mode.lock().unwrap();
            let ReaperMode::Native(native) = &mut *mode else {
                panic!("native pool");
            };
            assert!(native.charge(LIMIT).is_err());
        }
        let before = service.report().unwrap();
        let mut calls = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            service.try_clone_deployment_guard_with(&mut b, |guard| {
                calls += 1;
                if cause == 2 {
                    let _file = File::from(rustix::io::fcntl_dupfd_cloexec(guard, 256).unwrap());
                    panic!("after guard duplication before returning its owner");
                }
                Err(if cause == 0 {
                    Errno::INTR
                } else {
                    Errno::MFILE
                })
            })
        }));
        if cause == 2 {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err(Failure::GuardIo(e)))
                if e == if cause == 0 { Errno::INTR } else { Errno::MFILE }));
        }
        assert_eq!(calls, 1, "EINTR must never retry the duplication");
        assert_eq!(references(&observer), 2);
        assert_locked(&observer);
        assert_eq!(b.storage(), 37);
        assert_eq!(b.work(), 17 + Service::GUARD_CLONE_WORK);
        assert_eq!(
            b.peak_storage(),
            37 + Service::GUARD_CLONE_SCRATCH + Service::GUARD_FILE_STORAGE
        );
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        assert!(ledger == b.work_ledger_identity_v1());
        let after = service.report().unwrap();
        assert_eq!(after.work, before.work + Service::GUARD_CLONE_WORK);
        assert_eq!(after.failed_work, before.failed_work);
        assert_eq!(after.storage, Service::STORAGE);
        drop(service);
        let mut service = Service::recover_at(reaper).unwrap();
        let (file, c) = service.try_clone_deployment_guard(&mut b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        assert_guard_clone(&file, &observer);
        drop(file);
        b.release_storage(c.additional_storage()).unwrap();
        assert_eq!(b.work(), 17 + 2 * Service::GUARD_CLONE_WORK);
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        assert_eq!(service.report().unwrap().failed_work, before.failed_work);
        service.shutdown().unwrap();
        assert_released(&observer);
    }
}

fn guard_clone_overflow_precedes_duplication() {
    for cause in 0..4 {
        let (reaper, mut service, observer) = installed_pool(usize::MAX);
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let prefix = match cause {
            1 => usize::MAX - Service::GUARD_CLONE_SCRATCH + 1,
            2 => usize::MAX - Service::GUARD_CLONE_SCRATCH - Service::GUARD_FILE_STORAGE + 1,
            _ => 0,
        };
        b.reserve_storage(prefix).unwrap();
        if cause == 0 {
            b.charge_work(usize::MAX - Service::GUARD_CLONE_WORK + 1)
                .unwrap();
        }
        if cause == 3 {
            let mut mode = reaper.mode.lock().unwrap();
            let ReaperMode::Native(native) = &mut *mode else {
                panic!("native pool");
            };
            native
                .charge(usize::MAX - native.ledger.work() - Service::GUARD_CLONE_WORK + 1)
                .unwrap();
        }
        let before = service.report().unwrap();
        let mut calls = 0;
        let result = service.try_clone_deployment_guard_with(&mut b, |_| {
            calls += 1;
            Err(Errno::INTR)
        });
        assert!(matches!(result, Err(Failure::Resource(_))));
        assert_eq!(calls, 0);
        assert_eq!(b.storage(), prefix);
        match cause {
            0 => assert_eq!(b.failed_work(), Some(usize::MAX)),
            1 | 2 => assert_eq!(b.failed_storage(), Some(usize::MAX)),
            _ => assert_eq!(service.report().unwrap().failed_work, Some(usize::MAX)),
        }
        assert_eq!(service.report().unwrap().work, before.work);
        assert_eq!(references(&observer), 2);
        assert_locked(&observer);
        service.shutdown().unwrap();
        assert_released(&observer);
    }
}
