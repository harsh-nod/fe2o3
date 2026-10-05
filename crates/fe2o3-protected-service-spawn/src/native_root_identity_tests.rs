use super::super::{IDENTITY_ALLOCATION_STORAGE, IdentityAllocation};
use super::*;
use crate::{
    ProtectedServiceCleanupServiceV2 as Service,
    native_spawn::RootOwnedProtectedServiceChildV2 as Owner,
    process_cleanup::CleanupPollV1 as Poll,
    process_reaper::{ReapSlotV1, isolated_cleanup},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::{
    io::Errno,
    process::{Pid, Signal, WaitId, WaitIdOptions},
};
use std::{
    os::fd::{AsFd, OwnedFd},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

type View<'a, 'work> = RootTaskObservationV2<'a, 'work>;
const LIMIT: usize = 100_000_000;
const MARKER: &str = "FE2O3_NATIVE_ROOT_IDENTITY_CASE";
const COMPLETE: &str = "FE2O3_NATIVE_ROOT_IDENTITY_COMPLETE";

#[test]
fn identity_quotes_include_allocation_and_full_escaped_handle() {
    assert_eq!(IDENTITY_ALLOCATION_STORAGE, 3 * size_of::<usize>());
    assert_eq!(size_of::<IdentityAllocation>(), size_of::<usize>());
    assert_eq!(
        Owner::ROOT_TRACE_GROWTH,
        size_of::<RootTaskTraceV2<'static>>() + IDENTITY_ALLOCATION_STORAGE
    );
    assert_eq!(
        RootTaskIdentityV2::STORAGE,
        size_of::<(RootTaskIdentityV2, Storage)>() + IDENTITY_ALLOCATION_STORAGE
    );
    assert_eq!(View::IDENTITY_WORK, ENTRY + View::CONTINUITY_WORK);
    assert_eq!(
        View::IDENTITY_SCRATCH,
        RootTaskIdentityV2::STORAGE + View::CONTINUITY_SCRATCH
    );
}

#[test]
fn original_trace_identity_lifetime_and_refusals() {
    for mode in [
        "lifecycle",
        "account",
        "terminal",
        "begin-exact",
        "begin-short-allocation",
        "begin-short-work",
        "begin-short-storage",
        "retain-exact",
        "retain-short-work",
        "retain-postcheck-short-work",
        "retain-short-storage",
        "device-exact",
        "device-short-work",
        "device-short-storage",
        "device-missing-floor",
    ] {
        subprocess(mode);
    }
}

fn subprocess(mode: &str) {
    let completion = tempfile::NamedTempFile::new().unwrap();
    let mut process = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_spawn::child::trace::observation::identity_tests::root_identity_subprocess",
            "--nocapture",
        ])
        .env_clear()
        .env(MARKER, mode)
        .env(COMPLETE, completion.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while process.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = process.kill();
            let _ = process.wait();
            panic!("root identity subprocess timed out: {mode}");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let output = process.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(completion.path()).unwrap(), b"complete");
}

fn pool() -> Service {
    isolated_cleanup(Account::new(Work::new(LIMIT), LIMIT))
}

// Test-only direct-child adoption. The readiness byte excludes the initial exec
// race before real SEIZE; this grants no production clone or broker admission.
fn spawn(slot: ReapSlotV1<'static>) -> (Owner, OwnedFd, OwnedFd) {
    let lease = fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1().unwrap();
    let (read, gate) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (ready, report) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    let mut process = Command::new("/bin/sh")
        .args(["-c", "printf r; read token; exec /bin/true"])
        .env_clear()
        .stdin(Stdio::from(File::from(read)))
        .stdout(Stdio::from(File::from(report)))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = Pid::from_raw(process.id() as i32).unwrap();
    let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = process.kill();
            let _ = process.wait();
            panic!("pidfd for identity fixture: {error}");
        }
    };
    let witness = rustix::io::fcntl_dupfd_cloexec(&pidfd, 0).unwrap();
    let child = Owner::new(pid, Some(pidfd), lease, slot);
    drop(process);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut byte = [0];
    loop {
        match rustix::io::read(&ready, &mut byte) {
            Ok(1) => {
                assert_eq!(byte, [b'r']);
                break;
            }
            Err(Errno::AGAIN | Errno::INTR) => {}
            other => panic!("identity fixture readiness: {other:?}"),
        }
        assert!(
            Instant::now() < deadline,
            "identity child readiness timeout"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    (child, gate, witness)
}

fn drain(service: &mut Service, witness: &OwnedFd) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        service.pump(64).unwrap();
        match service.shutdown() {
            Ok(_) => break,
            Err(crate::ProtectedServiceCleanupErrorV2::Busy) => {}
            Err(error) => panic!("identity cleanup refused: {error}"),
        }
        assert!(Instant::now() < deadline, "identity cleanup timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(
        rustix::process::waitid(
            WaitId::PidFd(witness.as_fd()),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG
        ),
        Err(Errno::CHILD)
    ));
}

fn finish(
    mut trace: RootTaskTraceV2<'_>,
    service: &mut Service,
    witness: &OwnedFd,
    b: &mut Budget<'_>,
) {
    let retained = trace.retained_storage();
    assert_ne!(trace.cancel(), Poll::Quarantined);
    drop(trace);
    b.release_storage(retained).unwrap();
    drain(service, witness);
}

fn retain(trace: &RootTaskTraceV2<'_>, b: &mut Budget<'_>) -> RootTaskIdentityV2 {
    let entry = b.storage();
    let (identity, charge) = trace
        .with_task_observation::<_, Error>(b, |view, b| {
            let output = view.retain_identity(b)?;
            b.reserve_storage(output.1.additional_storage())?;
            Ok(output)
        })
        .unwrap();
    assert_eq!(
        b.storage(),
        entry,
        "outer view also returns unreserved output"
    );
    assert_eq!(charge.additional_storage(), identity.retained_storage());
    b.reserve_storage(charge.additional_storage()).unwrap();
    identity
}

#[test]
fn root_identity_subprocess() {
    let Ok(mode) = std::env::var(MARKER) else {
        return;
    };
    let mut service = pool();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let slot = service.reserve_launch(&mut b).unwrap().into_slot();
    let (child, gate, witness) = spawn(slot);
    if mode.starts_with("begin-") {
        begin_quote(&mode, child, &mut service, &witness, &mut b);
    } else {
        b.reserve_storage(Owner::STORAGE + Owner::ROOT_TRACE_GROWTH)
            .unwrap();
        let mut trace = child.into_root_trace(&mut b).unwrap();
        match mode.as_str() {
            "lifecycle" => {
                lifecycle(trace, &mut service, &witness, &mut b);
                std::fs::write(std::env::var_os(COMPLETE).unwrap(), b"complete").unwrap();
                return;
            }
            "account" => account_refusals(&mut trace, &mut b),
            "terminal" => terminal_refusal(&trace, &witness, &mut b),
            mode if mode.starts_with("device-") => device_quote(mode, &mut trace, &mut b),
            _ => retain_quote(&mode, &trace, &mut b),
        }
        assert_eq!(Rc::strong_count(&trace.identity), 1);
        finish(trace, &mut service, &witness, &mut b);
    }
    drop(gate);
    assert_eq!(b.storage(), 0);
    std::fs::write(std::env::var_os(COMPLETE).unwrap(), b"complete").unwrap();
}

fn device_quote(mode: &str, trace: &mut RootTaskTraceV2<'_>, b: &mut Budget<'_>) {
    trace.interrupt(b).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while trace.poll(b).unwrap().is_pending() {
        assert!(
            Instant::now() < deadline,
            "device observation interrupt timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    if mode == "device-missing-floor" {
        b.release_storage(1).unwrap();
        assert!(matches!(
            (View { trace }).require_device_open_confinement(b),
            Err(Error::Resource(Resource::Accounting))
        ));
        b.reserve_storage(1).unwrap();
        return;
    }
    let scratch = View::DEVICE_CONFINEMENT_SCRATCH;
    let pressure =
        LIMIT - trace.retained_storage() - scratch + usize::from(mode == "device-short-storage");
    b.reserve_storage(pressure).unwrap();
    let allowance = View::DEVICE_CONFINEMENT_WORK - usize::from(mode == "device-short-work");
    b.charge_work(LIMIT - b.work() - allowance).unwrap();
    let before = b.storage();
    let result = (View { trace }).require_device_open_confinement(b);
    assert_eq!(b.storage(), before);
    match (mode, result) {
        ("device-exact", Err(Error::State("compiler has no original device-confined domain"))) => {
            assert_eq!(b.work(), LIMIT);
            assert_eq!(b.peak_storage(), LIMIT);
        }
        ("device-short-work", Err(Error::Resource(Resource::Work(_)))) => {
            assert_eq!(b.failed_work(), Some(LIMIT + 1));
        }
        ("device-short-storage", Err(Error::Resource(Resource::Storage(_)))) => {
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
        }
        (_, result) => panic!("device observation {mode}: {result:?}"),
    }
    b.release_storage(pressure).unwrap();
}

fn lifecycle(
    trace: RootTaskTraceV2<'_>,
    service: &mut Service,
    witness: &OwnedFd,
    b: &mut Budget<'_>,
) {
    let first = retain(&trace, b);
    let weak = Rc::downgrade(&trace.identity);
    let mut slots = [Some(trace), None];
    let before = slots[0].as_ref().unwrap() as *const _;
    slots[1] = slots[0].take();
    let trace = slots[1].as_mut().unwrap();
    assert_ne!(before, trace as *const _);
    assert!(trace.poll(b).unwrap().is_pending());
    let second = retain(trace, b);
    assert!(first.matches(&first));
    assert!(first.matches(&second));
    assert!(second.matches(&first));
    assert_eq!(Rc::strong_count(&trace.identity), 3);
    let trace = slots[1].take().unwrap();
    finish(trace, service, witness, b);
    assert_eq!(weak.strong_count(), 2, "tokens retain only the allocation");
    assert!(
        first.matches(&second),
        "inert equality survives child reaping"
    );

    let mut other_service = pool();
    let slot = other_service.reserve_launch(b).unwrap().into_slot();
    let (child, _gate, other_witness) = spawn(slot);
    b.reserve_storage(Owner::STORAGE + Owner::ROOT_TRACE_GROWTH)
        .unwrap();
    let other_trace = child.into_root_trace(b).unwrap();
    let other = retain(&other_trace, b);
    assert!(
        !first.matches(&other),
        "equal marker values are not identity"
    );
    finish(other_trace, &mut other_service, &other_witness, b);
    let charge = first.retained_storage();
    assert_eq!(b.storage(), 3 * charge);
    drop(first);
    b.release_storage(charge).unwrap();
    assert_eq!(weak.strong_count(), 1);
    drop(second);
    b.release_storage(charge).unwrap();
    assert!(weak.upgrade().is_none());
    drop(other);
    b.release_storage(charge).unwrap();
    assert_eq!(b.storage(), 0);
}

fn begin_quote(
    mode: &str,
    child: Owner,
    service: &mut Service,
    witness: &OwnedFd,
    b: &mut Budget<'_>,
) {
    let retained = Owner::STORAGE + Owner::ROOT_TRACE_GROWTH;
    let floor = retained
        - if mode == "begin-short-allocation" {
            IDENTITY_ALLOCATION_STORAGE
        } else {
            0
        };
    b.reserve_storage(floor).unwrap();
    let pressure = if mode == "begin-short-allocation" {
        0
    } else {
        LIMIT - floor - RootTaskTraceV2::OPERATION_SCRATCH
            + usize::from(mode == "begin-short-storage")
    };
    b.reserve_storage(pressure).unwrap();
    let work = RootTaskTraceV2::OPERATION_WORK - usize::from(mode == "begin-short-work");
    b.charge_work(LIMIT - b.work() - work).unwrap();
    let result = child.into_root_trace(b);
    assert_eq!(b.storage(), floor + pressure);
    b.release_storage(pressure).unwrap();
    match (mode, result) {
        ("begin-exact", Ok(trace)) => {
            assert_eq!(b.work(), LIMIT);
            assert_eq!(b.peak_storage(), LIMIT);
            assert_eq!(Rc::strong_count(&trace.identity), 1);
            finish(trace, service, witness, b);
            return;
        }
        ("begin-short-allocation", Err(Error::Resource(Resource::Accounting))) => {}
        ("begin-short-work", Err(Error::Resource(Resource::Work(_)))) => {
            assert_eq!(b.failed_work(), Some(LIMIT + 1));
        }
        ("begin-short-storage", Err(Error::Resource(Resource::Storage(_)))) => {
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
        }
        (_, other) => panic!("unexpected begin result for {mode}: {other:?}"),
    }
    b.release_storage(floor).unwrap();
    drain(service, witness);
}

fn retain_quote(mode: &str, trace: &RootTaskTraceV2<'_>, b: &mut Budget<'_>) {
    let scratch = size_of::<View<'_, '_>>() + View::IDENTITY_SCRATCH;
    let pressure =
        LIMIT - trace.retained_storage() - scratch + usize::from(mode == "retain-short-storage");
    b.reserve_storage(pressure).unwrap();
    let work = if mode == "retain-short-work" {
        // Refuse inside extraction, before cloning, not in the view's postcheck.
        8 + View::CONTINUITY_WORK + View::IDENTITY_WORK - 1
    } else {
        View::VIEW_WORK + View::IDENTITY_WORK - usize::from(mode == "retain-postcheck-short-work")
    };
    b.charge_work(LIMIT - b.work() - work).unwrap();
    let entry = b.storage();
    let result = trace.with_task_observation(b, |view, b| {
        let storage = b.storage();
        let before = b.work();
        let result = view.retain_identity(b);
        assert_eq!(b.storage(), storage);
        if let Ok((identity, charge)) = &result {
            assert_eq!(b.work() - before, View::IDENTITY_WORK);
            assert_eq!(charge.additional_storage(), identity.retained_storage());
            b.reserve_storage(charge.additional_storage())?;
        }
        result
    });
    assert_eq!(b.storage(), entry);
    match (mode, result) {
        ("retain-exact", Ok((identity, charge))) => {
            assert_eq!(b.work(), LIMIT);
            assert_eq!(b.peak_storage(), LIMIT);
            b.reserve_storage(charge.additional_storage()).unwrap();
            assert_eq!(Rc::strong_count(&trace.identity), 2);
            drop(identity);
            b.release_storage(charge.additional_storage()).unwrap();
        }
        (
            "retain-short-work" | "retain-postcheck-short-work",
            Err(Error::Resource(Resource::Work(_))),
        ) => {
            assert_eq!(b.failed_work(), Some(LIMIT + 1));
        }
        ("retain-short-storage", Err(Error::Resource(Resource::Storage(_)))) => {
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
        }
        _ => panic!("unexpected retain result for {mode}"),
    }
    assert_eq!(Rc::strong_count(&trace.identity), 1);
    b.release_storage(pressure).unwrap();
}

fn account_refusals(trace: &mut RootTaskTraceV2<'_>, b: &mut Budget<'_>) {
    let mut foreign_work = Work::new(LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(trace.retained_storage()).unwrap();
    assert!(matches!(
        (View { trace }).retain_identity(&mut foreign),
        Err(Error::Resource(Resource::Accounting))
    ));
    let address = trace.budget_address;
    trace.budget_address = address.wrapping_add(1);
    assert!(matches!(
        (View { trace }).retain_identity(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    trace.budget_address = address;
    let origin = trace.origin;
    trace.origin.2 = std::thread::spawn(|| std::thread::current().id())
        .join()
        .unwrap();
    assert!(matches!(
        (View { trace }).retain_identity(b),
        Err(Error::State(_))
    ));
    trace.origin = origin;
    b.release_storage(1).unwrap();
    assert!(matches!(
        (View { trace }).retain_identity(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    b.reserve_storage(1).unwrap();
    assert_eq!(Rc::strong_count(&trace.identity), 1);
    let identity = retain(trace, b);
    let charge = identity.retained_storage();
    drop(identity);
    b.release_storage(charge).unwrap();
    assert_ne!(trace.cancel(), Poll::Quarantined);
    assert!(matches!(
        (View { trace }).retain_identity(b),
        Err(Error::State(_))
    ));
}

fn terminal_refusal(trace: &RootTaskTraceV2<'_>, witness: &OwnedFd, b: &mut Budget<'_>) {
    let result = trace.with_task_observation(b, |view, b| {
        rustix::process::pidfd_send_signal(witness, Signal::KILL).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if rustix::process::waitid(
                WaitId::PidFd(witness.as_fd()),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            )
            .unwrap()
            .is_some()
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "identity child termination timeout"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        view.retain_identity(b)
    });
    assert!(matches!(result, Err(Error::State(_))));
    assert_eq!(Rc::strong_count(&trace.identity), 1);
}
