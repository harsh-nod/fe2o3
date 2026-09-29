use super::*;
use crate::{
    ProtectedServiceCleanupErrorV2 as Failure, ProtectedServiceCleanupServiceV2 as Service,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    fs::File,
    os::fd::AsRawFd,
    panic::{AssertUnwindSafe, catch_unwind},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

type Owner = RootOwnedProtectedServiceChildV2;
const CLEANUP_TURNS: usize = 512;
const LIMIT: usize = 1_000_000
    + CLEANUP_TURNS * (Service::TURN_WORK + 64 * Service::CELL_WORK)
    + 65 * Service::RESERVATION_WORK
    + Service::STORAGE
    + Owner::OPERATION_SCRATCH;
const MARKER: &str = "FE2O3_PRIVATE_NATIVE_ROOT_CHILD_TEST";
fn pool() -> Service {
    crate::process_reaper::isolated_cleanup(Account::new(Work::new(LIMIT), Service::STORAGE))
}
fn synthetic(service: &mut Service, b: &mut Budget<'_>) -> Owner {
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let pid = Pid::from_raw(1000).unwrap();
    // Inert descriptor/lease-free records exercise ownership, never real child evidence.
    Owner {
        custody: Custody(Some((Child::new(None, pid, None), slot))),
        pid,
        disposition: Poll::Pending,
    }
}

#[test]
fn missing_pidfd_cancel_and_unwind_retain_slots_without_claiming_reap() {
    for unwind in [false, true] {
        let mut service = pool();
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let child = synthetic(&mut service, &mut b);
        b.reserve_storage(Owner::STORAGE).unwrap();
        let original = service.report().unwrap();
        if unwind {
            let result = catch_unwind(AssertUnwindSafe(move || {
                let _child = child;
                panic!("private native child unwind fixture");
            }));
            assert!(result.is_err());
        } else {
            let mut child = child;
            assert_eq!(child.cancel(), Poll::Quarantined);
            for _ in 0..10 {
                assert_eq!(child.cancel(), Poll::Quarantined);
            }
            assert!(matches!(child.is_live(&mut b), Err(Error::State(_))));
            drop(child);
        }
        assert_eq!(service.report().unwrap(), original);
        service.pump(64).unwrap();
        let mut remaining = Vec::new();
        for _ in 0..63 {
            remaining.push(service.reserve_launch(&mut b).unwrap());
        }
        assert!(matches!(
            service.reserve_launch(&mut b),
            Err(Failure::Capacity)
        ));
        drop(remaining);
        assert!(matches!(service.shutdown(), Err(Failure::Busy)));
        assert_eq!(b.storage(), Owner::STORAGE);
    }
}

#[test]
fn child_operations_refuse_short_funding_before_touching_custody() {
    for operation in 0..3 {
        for limit in 0..3 {
            let mut service = pool();
            let mut initial = Work::new(LIMIT);
            let mut initial = Budget::new(&mut initial, LIMIT);
            let mut child = synthetic(&mut service, &mut initial);
            let work = if limit == 0 {
                Owner::OPERATION_WORK - 1
            } else {
                LIMIT
            };
            let storage = if limit == 2 {
                Owner::STORAGE + Owner::OPERATION_SCRATCH - 1
            } else {
                LIMIT
            };
            let floor = Owner::STORAGE - usize::from(limit == 1);
            let mut w = Work::new(work);
            let mut b = Budget::new(&mut w, storage);
            b.reserve_storage(floor).unwrap();
            let result = match operation {
                0 => child.is_live(&mut b).map(|_| ()),
                1 => child.try_clone_pidfd(&mut b).map(|_| ()),
                // No real descriptor, child or artifact lease exists in this fixture.
                _ => unsafe { child.confirm_exec(&mut b) },
            };
            let e = result.unwrap_err();
            match limit {
                0 => assert!(matches!(e, Error::Resource(Resource::Work(_)))),
                1 => assert!(matches!(e, Error::Resource(Resource::Accounting))),
                _ => assert!(matches!(e, Error::Resource(Resource::Storage(_)))),
            }
            assert!(child.custody.0.is_some());
            assert_eq!(b.storage(), floor);
            assert_eq!(child.cancel(), Poll::Quarantined);
        }
    }
}

#[test]
fn terminal_record_retires_exactly_one_slot_and_cancel_is_idempotent() {
    let mut service = pool();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let mut child = synthetic(&mut service, &mut b);
    child.custody.0.as_mut().unwrap().0.terminal_reaped();
    assert_eq!(child.cancel(), Poll::Reaped);
    assert_eq!(child.cancel(), Poll::Reaped);
    drop(child);
    let mut slots = Vec::new();
    for _ in 0..64 {
        slots.push(service.reserve_launch(&mut b).unwrap());
    }
    assert!(matches!(
        service.reserve_launch(&mut b),
        Err(Failure::Capacity)
    ));
    drop(slots);
    service.shutdown().unwrap();
}

#[test]
fn namespace_setup_refusal_keeps_original_custody_and_deferred_state() {
    let mut service = pool();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut child = synthetic(&mut service, &mut budget);
    let report = service.report().unwrap();
    let original_work = budget.work();
    assert!(matches!(child.configure_namespace(), Err(Error::State(_))));
    assert!(matches!(child.revalidate_namespace(), Err(Error::State(_))));
    assert!(child.custody.0.is_some());
    assert_eq!(service.report().unwrap(), report);
    assert_eq!(budget.work(), original_work);
    assert_eq!(child.cancel(), Poll::Quarantined);
    assert!(child.custody.0.is_none());
    assert!(matches!(child.configure_namespace(), Err(Error::State(_))));
    assert!(matches!(child.revalidate_namespace(), Err(Error::State(_))));
    assert_eq!(child.cancel(), Poll::Quarantined);
    assert_eq!(service.report().unwrap(), report);
    assert!(matches!(service.shutdown(), Err(Failure::Busy)));
}

#[test]
fn namespace_fd_storage_and_close_only_retirement_are_prepaid() {
    use crate::native_user_namespace::NativeUserNamespaceV1 as Namespace;

    assert_eq!(
        Owner::STORAGE,
        size_of::<(Owner, Storage)>() + NativeCgroupDomainV1::STORAGE
            - size_of::<NativeCgroupDomainV1>()
            + Namespace::STORAGE
            - size_of::<Namespace>()
            + NativeCgroupDomainV1::STEP_SCRATCH
    );
    assert_eq!(
        Namespace::STORAGE - size_of::<Namespace>(),
        13 * size_of::<usize>()
    );
    assert_eq!(
        Service::CELL_WORK,
        16 + NativeCgroupDomainV1::STEP_WORK + 13 * (1024 + 64)
    );
}

#[test]
fn exact_pidfd_exec_confirmation_cancellation_and_drop_in_subprocess() {
    subprocess("lifecycle");
}

#[test]
fn atomic_clone_and_guard_adoption_in_subprocess() {
    subprocess("clone");
}

#[test]
fn namespace_drop_and_pool_retain_until_aggregate_completion_in_subprocess() {
    subprocess("namespace");
}

fn subprocess(mode: &str) {
    let completion = tempfile::NamedTempFile::new().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_spawn::child::tests::native_child_subprocess",
            "--nocapture",
        ])
        .env_clear()
        .env(MARKER, mode)
        .env("FE2O3_NATIVE_CHILD_COMPLETION", completion.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native child custody subprocess timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(completion.path()).unwrap(), b"complete");
}

// A dedicated subprocess may select the REAL global service without influencing
// parallel tests. Its guard drains real children through funded finite turns.
struct Drain(Service);
impl Drop for Drain {
    fn drop(&mut self) {
        for _ in 0..CLEANUP_TURNS {
            if self.0.shutdown().is_ok() {
                return;
            }
            self.0.pump(64).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        if !std::thread::panicking() {
            panic!("native child cleanup did not drain");
        }
    }
}

#[test]
fn native_child_subprocess() {
    if std::env::var_os(MARKER).is_none() {
        return;
    }
    if std::env::var_os(MARKER).as_deref() == Some(std::ffi::OsStr::new("namespace")) {
        Child::check_namespace_drop_retention_fixture();
        Service::check_namespace_pool_retention_fixture();
        std::fs::write(
            std::env::var_os("FE2O3_NATIVE_CHILD_COMPLETION").unwrap(),
            b"complete",
        )
        .unwrap();
        return;
    }
    let mut service =
        Drain(Service::admit(Account::new(Work::new(LIMIT), Service::STORAGE + 1024)).unwrap());
    let guard = File::from(
        rustix::fs::memfd_create(c"native-child-guard", rustix::fs::MemfdFlags::CLOEXEC).unwrap(),
    );
    let observer = File::open(format!("/proc/self/fd/{}", guard.as_raw_fd())).unwrap();
    rustix::fs::flock(&guard, rustix::fs::FlockOperation::NonBlockingLockShared).unwrap();
    let mut startup_work = Work::new(LIMIT);
    let mut startup = Budget::new(&mut startup_work, Service::GUARD_FILE_STORAGE);
    startup
        .reserve_storage(Service::GUARD_FILE_STORAGE)
        .unwrap();
    service
        .0
        .retain_deployment_guard(guard, &mut startup)
        .unwrap();
    startup
        .release_storage(Service::GUARD_FILE_STORAGE)
        .unwrap();
    if std::env::var_os(MARKER).as_deref() == Some(std::ffi::OsStr::new("clone")) {
        clone_probe(&mut service);
    } else {
        lifecycle_probe(&mut service, &observer);
    }
    assert_guard_locked(&observer);
    drop(service);
    rustix::fs::flock(
        &observer,
        rustix::fs::FlockOperation::NonBlockingLockExclusive,
    )
    .unwrap();
    std::fs::write(
        std::env::var_os("FE2O3_NATIVE_CHILD_COMPLETION").unwrap(),
        b"complete",
    )
    .unwrap();
}

fn assert_guard_locked(observer: &File) {
    assert_eq!(
        rustix::fs::flock(
            observer,
            rustix::fs::FlockOperation::NonBlockingLockExclusive
        ),
        Err(Errno::AGAIN)
    );
}

fn lifecycle_probe(service: &mut Drain, observer: &File) {
    for (confirm, unwind) in [(false, false), (true, false), (true, true)] {
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        let slot = service.0.reserve_launch(&mut b).unwrap().into_slot();
        let lease =
            fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1().unwrap();
        let mut process = Command::new("/bin/sleep")
            .arg("2")
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let pid = Pid::from_raw(process.id() as i32).unwrap();
        let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
            Ok(fd) => fd,
            Err(e) => {
                let _ = process.kill();
                let _ = process.wait();
                panic!("pidfd: {e}");
            }
        };
        // Test-only adoption of our direct Command child, not an atomic-clone or
        // credential-transition claim. No other owner performs consuming waits.
        let mut child = Owner::new(pid, Some(pidfd), lease, slot);
        drop(process);
        child.check_pidfd().unwrap();
        b.reserve_storage(Owner::STORAGE).unwrap();
        assert!(child.is_live(&mut b).unwrap());
        let (witness, charge) = child.try_clone_pidfd(&mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(child.record().unwrap().retains_spawn_lease());
        if confirm {
            // Command::spawn returned only after successful exec/CLOEXEC completion.
            unsafe {
                child.confirm_exec(&mut b).unwrap();
            }
            assert!(!child.record().unwrap().retains_spawn_lease());
            unsafe {
                child.confirm_exec(&mut b).unwrap();
            }
        }
        if !confirm {
            let disposition = child.cancel();
            assert!(matches!(disposition, Poll::Pending | Poll::Reaped));
            assert_eq!(child.cancel(), disposition);
        }
        assert_guard_locked(observer);
        if confirm {
            assert!(b.charge_work(LIMIT).is_err());
            assert_guard_locked(observer);
        }
        if unwind {
            assert!(matches!(service.0.shutdown(), Err(Failure::Busy)));
            assert!(
                catch_unwind(AssertUnwindSafe(move || {
                    let _child = child;
                    panic!("native post-exec unwind with deployment guard");
                }))
                .is_err()
            );
        } else {
            drop(child);
        }
        assert_guard_locked(observer);
        b.release_storage(Owner::STORAGE).unwrap();
        await_reaped(service, &witness);
        assert_guard_locked(observer);
        drop(witness);
        b.release_storage(charge.additional_storage()).unwrap();
        assert_eq!(b.storage(), 0);
    }
}

fn clone_probe(service: &mut Drain) {
    use super::super::{
        Binding, Credentials, StagedProtectedServiceExecV2 as Stage, clone_guarded,
    };
    use std::fs::File;
    // This cannot become an admitted service: no gate token is sent and /dev/null
    // is not executable. Rootless profile setup may fail. It tests real clone3,
    // immediate pidfd custody and finite disposal, not a protected deployment.
    let executable = File::open("/dev/null").unwrap();
    let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let bindings = [Binding::new(read.as_fd(), 3).unwrap()];
    let mut w = Work::new(4 * LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(4096).unwrap();
    let (stage, charge) = unsafe {
        Stage::stage(
            &executable,
            &bindings,
            write.as_fd(),
            read.as_fd(),
            write.as_fd(),
            4096,
            &mut b,
        )
    }
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let credentials = Credentials::new(65534, 65534).unwrap();
    for failure in 0..3 {
        let held = File::from(
            rustix::fs::memfd_create(
                c"retained-clone-dependency",
                rustix::fs::MemfdFlags::CLOEXEC,
            )
            .unwrap(),
        );
        rustix::fs::flock(&held, rustix::fs::FlockOperation::NonBlockingLockShared).unwrap();
        let observer = File::open(format!("/proc/self/fd/{}", held.as_raw_fd())).unwrap();
        b.reserve_storage(Service::GUARD_FILE_STORAGE).unwrap();
        let (mut child, resources) = b
            .with_prepaid_scope::<_, Error>(
                stage.retained_storage() + Service::GUARD_FILE_STORAGE,
                0,
                stage.spawn_work(63).unwrap() - Service::RESERVATION_WORK,
                Stage::SPAWN_SCRATCH,
                |b| {
                    let (slot, resources, charge) =
                        service
                            .0
                            .reserve_retaining(held, Service::GUARD_FILE_STORAGE, b)?;
                    b.reserve_storage(charge.additional_storage())?;
                    let lease =
                        fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1()
                            .map_err(Error::SpawnLease)?;
                    clone_guarded(&stage.inner, credentials, 63, lease, slot.into_slot())
                        .map(|child| (child, resources))
                },
            )
            .unwrap();
        let resources_charge = resources.retained_storage();
        b.reserve_storage(resources_charge - Service::GUARD_FILE_STORAGE)
            .unwrap();
        drop(resources);
        b.release_storage(resources_charge).unwrap();
        assert_guard_locked(&observer);
        assert!(service.0.report().unwrap().storage > Service::STORAGE);
        b.reserve_storage(Owner::STORAGE).unwrap();
        assert!(child.record().unwrap().retains_spawn_lease());
        let (witness, witness_charge) = child.try_clone_pidfd(&mut b).unwrap();
        b.reserve_storage(witness_charge.additional_storage())
            .unwrap();
        match failure {
            0 => {
                let disposition = child.cancel();
                assert!(matches!(disposition, Poll::Reaped | Poll::Pending));
                drop(child);
            }
            1 => {
                // A failed post-adoption descriptor check must retain finite Drop.
                rustix::io::fcntl_setfd(child.pidfd().unwrap(), FdFlags::empty()).unwrap();
                assert!(matches!(child.check_pidfd(), Err(Error::State(_))));
                drop(child);
            }
            _ => assert!(
                catch_unwind(AssertUnwindSafe(move || {
                    let _child = child;
                    panic!("private post-clone unwind fixture");
                }))
                .is_err()
            ),
        }
        b.release_storage(Owner::STORAGE).unwrap();
        await_reaped(service, &witness);
        rustix::fs::flock(
            &observer,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .unwrap();
        assert_eq!(service.0.report().unwrap().storage, Service::STORAGE);
        drop(witness);
        b.release_storage(witness_charge.additional_storage())
            .unwrap();
    }
    drop(stage);
    b.release_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), 4096);
}

fn await_reaped(service: &mut Drain, witness: &OwnedFd) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        service.0.pump(64).unwrap();
        match rustix::process::waitid(
            WaitId::PidFd(witness.as_fd()),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        ) {
            Err(Errno::CHILD) => break,
            Ok(_) => {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => panic!("observe fixture reap: {e}"),
        }
    }
}

#[test]
fn retained_child_keeps_complete_inputs_through_exec_cancel_and_unwind() {
    use super::super::RootOwnedRetainedServiceChildV2 as RetainedChild;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Input(Arc<AtomicUsize>);
    impl Drop for Input {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    const INPUT: usize = 64;
    for terminal in [false, true] {
        for unwind in [false, true] {
            let mut c =
                crate::process_reaper::isolated_cleanup(Account::new(Work::new(LIMIT), LIMIT));
            let mut w = Work::new(LIMIT);
            let mut b = Budget::new(&mut w, LIMIT);
            b.reserve_storage(INPUT).unwrap();
            let drops = Arc::new(AtomicUsize::new(0));
            let (slot, resources, _) = c
                .reserve_retaining(Input(drops.clone()), INPUT, &mut b)
                .unwrap();
            let pid = Pid::from_raw(1000).unwrap();
            let mut record = Child::new(None, pid, None);
            if terminal {
                record.terminal_reaped();
            }
            let child = Owner {
                custody: Custody(Some((record, slot.into_slot()))),
                pid,
                disposition: Poll::Pending,
            };
            let full = RetainedChild::<Input>::storage_for(INPUT).unwrap();
            b.reserve_storage(full - INPUT).unwrap();
            let mut child = RetainedChild::new(child, resources, full);
            child
                .with_resources(&mut b, |resources, _| {
                    assert!(Arc::ptr_eq(&resources.0, &drops));
                    Ok::<_, Error>(())
                })
                .unwrap();
            assert_eq!(child.retained_storage(), full);
            let mut short_w = Work::new(LIMIT);
            let mut short = Budget::new(&mut short_w, LIMIT);
            short.reserve_storage(full - 1).unwrap();
            assert!(matches!(
                child.is_live(&mut short),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert!(matches!(
                child.try_clone_pidfd(&mut short),
                Err(Error::Resource(Resource::Accounting))
            ));
            // No process or inherited artifact alias exists in this synthetic record.
            unsafe {
                child.confirm_exec(&mut b).unwrap();
            }
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            assert!(c.report().unwrap().storage > Service::STORAGE);
            if unwind {
                assert!(
                    catch_unwind(AssertUnwindSafe(move || {
                        let _child = child;
                        panic!("retained child owner unwind");
                    }))
                    .is_err()
                );
            } else {
                let result = child.cancel();
                assert_eq!(
                    result,
                    if terminal {
                        Poll::Reaped
                    } else {
                        Poll::Quarantined
                    }
                );
                assert_eq!(child.cancel(), result);
                assert_eq!(drops.load(Ordering::SeqCst), 0);
                drop(child);
            }
            b.release_storage(full).unwrap();
            assert_eq!(drops.load(Ordering::SeqCst), usize::from(terminal));
            if terminal {
                assert_eq!(c.report().unwrap().storage, Service::STORAGE);
                assert_eq!(c.shutdown().unwrap().storage(), 0);
            } else {
                c.pump(64).unwrap();
                assert!(matches!(c.shutdown(), Err(Failure::Busy)));
                assert!(c.report().unwrap().storage > Service::STORAGE);
            }
        }
    }
}
