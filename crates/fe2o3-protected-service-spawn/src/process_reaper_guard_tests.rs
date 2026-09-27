use super::*;
use crate::process_cleanup::ChildCleanupV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{FlockOperation, MemfdFlags, flock, memfd_create};
use rustix::io::Errno;
use std::os::fd::AsRawFd;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: usize = 1 << 20;
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
        "all five custody schedules must complete, not a zero-test filter"
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
