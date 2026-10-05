use super::*;
use crate::native_spawn::RootOwnedRetainedServiceChildV2 as Retained;
use rustix::process::{Signal, WaitId, WaitIdOptions};
use std::{
    fs::File,
    os::fd::{AsFd, OwnedFd},
    panic::{AssertUnwindSafe, catch_unwind},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

const MARKER: &str = "FE2O3_NATIVE_ROOT_TRACE_CASE";
const COMPLETE: &str = "FE2O3_NATIVE_ROOT_TRACE_COMPLETE";
const RUNTIME_FIXTURE: &str = "FE2O3_NATIVE_RUNTIME_FIXTURE";

#[path = "native_root_trace_confirmation_tests.rs"]
mod confirmation;

#[path = "native_root_observation_tests.rs"]
mod observation;

#[path = "native_runtime_trace_process_tests.rs"]
mod runtime;

#[test]
fn real_root_trace_schedules_in_bounded_subprocesses() {
    for mode in [
        "exec",
        "held-exec",
        "unobserved-exec",
        "signal",
        "fatal",
        "group",
        "unwind",
        "echld",
        "retained",
        "retained-drop",
        "retained-echld",
        "refused",
        "ledger",
        "thread",
    ] {
        subprocess(mode);
    }
}

#[test]
fn runtime_trace_original_takeover_and_retirement_are_bounded() {
    for mode in [
        "runtime-cancel",
        "runtime-account",
        "runtime-census",
        "runtime-expired",
    ] {
        subprocess(mode);
    }
}

#[test]
#[ignore = "requires the separately compiled, pinned runtime-checkpoint-v1 component fixture"]
fn runtime_trace_observes_repeated_thread_lifecycles() {
    assert!(std::env::var_os(RUNTIME_FIXTURE).is_some());
    subprocess("runtime-lifecycle");
    subprocess("runtime-tree-cancel");
}

#[test]
fn moved_budget_refuses_original_ledger_at_another_address() {
    subprocess("confirm-budget-address");
}

#[test]
fn confirm_exec_requires_owned_stop_and_exact_funding() {
    for mode in [
        "confirm-exact",
        "confirm-resumed",
        "confirm-short-work",
        "confirm-short-storage",
    ] {
        subprocess(mode);
    }
}

fn subprocess(mode: &str) {
    let completion = tempfile::NamedTempFile::new().unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "native_spawn::child::trace::tests::processes::root_trace_subprocess",
            "--nocapture",
        ])
        .env_clear()
        .env(MARKER, mode)
        .env(COMPLETE, completion.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if matches!(mode, "runtime-lifecycle" | "runtime-tree-cancel") {
        command.env(RUNTIME_FIXTURE, std::env::var_os(RUNTIME_FIXTURE).unwrap());
    }
    let mut process = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while process.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            let _ = process.kill();
            let _ = process.wait();
            panic!("root trace subprocess timed out: {mode}");
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let output = process.wait_with_output().unwrap();
    if matches!(mode, "runtime-lifecycle" | "runtime-tree-cancel") {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    assert!(
        output.status.success(),
        "{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read(completion.path()).unwrap(),
        b"complete",
        "{mode}"
    );
}

// This fixture adopts a direct Command child only in tests. Its second exec is
// gated on stdin; it is not evidence for the production clone/credential path.
fn spawn(slot: crate::process_reaper::ReapSlotV1<'static>) -> (Owner, OwnedFd, OwnedFd) {
    spawn_fixture(slot, false)
}

fn spawn_fixture(
    slot: crate::process_reaper::ReapSlotV1<'static>,
    runtime: bool,
) -> (Owner, OwnedFd, OwnedFd) {
    let lease = fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1().unwrap();
    let (read, gate) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
    let (ready, report_ready) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    let mut command = if runtime {
        Command::new(std::env::var_os(RUNTIME_FIXTURE).unwrap())
    } else {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf r; read token; exec /bin/true"]);
        command
    };
    let mut process = command
        .env_clear()
        .stdin(Stdio::from(File::from(read)))
        .stdout(Stdio::from(File::from(report_ready)))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = Pid::from_raw(process.id() as i32).unwrap();
    let pidfd = match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = process.kill();
            let _ = process.wait();
            panic!("pidfd for root trace fixture: {error}");
        }
    };
    let witness = rustix::io::fcntl_dupfd_cloexec(&pidfd, 0).unwrap();
    let child = Owner::new(pid, Some(pidfd), lease, slot);
    drop(process);
    // CLOEXEC pipe closure can wake Command::spawn before its initial exec
    // event finishes. Wait for userspace before SEIZE; only the gated exec is
    // then eligible for tracing. This fixture token releases no spawn lease.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut token = [0];
    loop {
        match rustix::io::read(&ready, &mut token) {
            Ok(1) => {
                assert_eq!(token, [b'r']);
                break;
            }
            Err(Errno::AGAIN | Errno::INTR) => {}
            other => panic!("root trace fixture readiness failed: {other:?}"),
        }
        assert!(
            Instant::now() < deadline,
            "root trace fixture readiness timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    (child, gate, witness)
}

fn next(trace: &mut RootTaskTraceV2<'_>, b: &mut Budget<'_>) -> Event {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let event = trace.poll(b).unwrap().state;
        if event != Event::Pending {
            return event;
        }
        assert!(Instant::now() < deadline, "no root trace event");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn drain(service: &mut Service) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        service.pump(64).unwrap();
        match service.shutdown() {
            Ok(_) => return,
            Err(crate::ProtectedServiceCleanupErrorV2::Busy) => {}
            Err(error) => panic!("cleanup refused: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "root trace cleanup remained busy"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn require_reaped(witness: &OwnedFd) {
    assert!(matches!(
        rustix::process::waitid(
            WaitId::PidFd(witness.as_fd()),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG
        ),
        Err(Errno::CHILD)
    ));
}

#[test]
fn root_trace_subprocess() {
    let Ok(mode) = std::env::var(MARKER) else {
        return;
    };
    let mut service = pool();
    // This explicitly bounded component schedule reserves for at most 256
    // kernel events; it does not alter any production work or runtime limit.
    let limit = if matches!(mode.as_str(), "runtime-lifecycle" | "runtime-tree-cancel") {
        LIMIT + 256 * 16 * crate::native_spawn::RootRuntimeTraceV1::OPERATION_WORK
    } else {
        LIMIT
    };
    let mut work = Work::new(limit);
    let mut b = Budget::new(&mut work, limit);
    if mode.starts_with("observe-") {
        observation::run(&mode, &mut service, &mut b);
    } else if mode.starts_with("runtime-") {
        runtime::run(&mode, &mut service, &mut b);
    } else if mode.starts_with("confirm-") {
        confirmation::run(&mode, &mut service, &mut b);
    } else if mode.starts_with("retained") {
        retained(&mode, &mut service, &mut b);
    } else {
        run(&mode, &mut service, &mut b);
    }
    std::fs::write(std::env::var_os(COMPLETE).unwrap(), b"complete").unwrap();
}

fn run(mode: &str, service: &mut Service, b: &mut Budget<'_>) {
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let (child, gate, witness) = spawn(slot);
    let retained = Owner::STORAGE + Owner::ROOT_TRACE_GROWTH;
    b.reserve_storage(retained).unwrap();
    if mode == "refused" {
        b.release_storage(1).unwrap();
        assert!(matches!(
            child.into_root_trace(b),
            Err(Error::Resource(Resource::Accounting))
        ));
        drain(service);
        require_reaped(&witness);
        return;
    }
    let identity = b.work_ledger_identity_v1();
    let mut trace = child.into_root_trace(b).unwrap();
    assert_eq!(trace.retained_storage(), retained);
    assert!(trace.child.record().unwrap().retains_spawn_lease());
    assert!(matches!(trace.resume(b), Err(Error::State(_))));
    assert_eq!(b.storage(), retained);
    assert!(identity == b.work_ledger_identity_v1());

    if mode == "ledger" {
        let mut alien_work = Work::new(LIMIT);
        let mut alien = Budget::new(&mut alien_work, LIMIT);
        alien.reserve_storage(retained).unwrap();
        assert!(matches!(
            trace.poll(&mut alien),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert!(matches!(
            trace.resume(&mut alien),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(trace.held, Event::Pending);
        assert_eq!(trace.poll(b).unwrap().state, Event::Pending);
    } else if mode == "thread" {
        let original = trace.origin;
        let foreign_thread = std::thread::spawn(|| std::thread::current().id())
            .join()
            .unwrap();
        trace.origin.2 = foreign_thread;
        assert!(matches!(trace.poll(b), Err(Error::State(_))));
        assert!(matches!(trace.resume(b), Err(Error::State(_))));
        assert_eq!(trace.held, Event::Pending);
        trace.origin = original;
        assert_eq!(trace.poll(b).unwrap().state, Event::Pending);
    } else if mode == "echld" {
        steal_terminal(&witness);
        assert!(matches!(
            trace.poll(b),
            Err(Error::Io {
                source: Errno::CHILD,
                ..
            })
        ));
        assert_eq!(
            trace.child.record().unwrap().last_errno(),
            Some(Errno::CHILD)
        );
        assert!(trace.child.record().unwrap().retains_spawn_lease());
        assert_eq!(trace.cancel(), Poll::Quarantined);
        assert_eq!(trace.cancel(), Poll::Quarantined);
        drop(trace);
        service.pump(64).unwrap();
        assert!(matches!(
            service.shutdown(),
            Err(crate::ProtectedServiceCleanupErrorV2::Busy)
        ));
        require_reaped(&witness);
        return;
    } else if mode == "unobserved-exec" {
        rustix::io::write(&gate, b"go\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let status = rustix::process::waitid(
                WaitId::PidFd(witness.as_fd()),
                WaitIdOptions::STOPPED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            )
            .unwrap();
            if let Some(status) = status {
                assert_eq!(decode(status).unwrap(), Event::Exec);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(trace.held, Event::Pending);
    } else if mode == "fatal" {
        rustix::process::pidfd_send_signal(&witness, Signal::TERM).unwrap();
        assert_eq!(next(&mut trace, b), Event::SignalStop(libc::SIGTERM));
        trace.resume(b).unwrap();
        assert_eq!(
            next(&mut trace, b),
            Event::Signaled {
                signal: libc::SIGTERM,
                core_dumped: false
            }
        );
        let terminal = trace.poll(b).unwrap();
        assert!(terminal.is_terminal());
        assert_eq!(terminal.terminating_signal(), Some(libc::SIGTERM));
        assert!(!terminal.core_dumped());
        assert!(!trace.child.record().unwrap().retains_spawn_lease());
        require_reaped(&witness);
        assert_eq!(trace.cancel(), Poll::Reaped);
    } else if matches!(mode, "signal" | "group" | "unwind") {
        rustix::process::pidfd_send_signal(&witness, Signal::STOP).unwrap();
        assert_eq!(next(&mut trace, b), Event::SignalStop(libc::SIGSTOP));
        if mode == "group" {
            trace.resume(b).unwrap();
            assert_eq!(next(&mut trace, b), Event::GroupStop(libc::SIGSTOP));
            trace.resume(b).unwrap();
            assert_eq!(trace.poll(b).unwrap().state, Event::Pending);
        }
    } else {
        // A separately owned terminal sibling must survive root-only polling.
        let mut sibling = Command::new("/bin/true").env_clear().spawn().unwrap();
        rustix::io::write(&gate, b"go\n").unwrap();
        assert_eq!(next(&mut trace, b), Event::Exec);
        assert_eq!(trace.poll(b).unwrap().state, Event::Exec);
        assert!(trace.child.record().unwrap().retains_spawn_lease());
        if mode == "exec" {
            trace.resume(b).unwrap();
            assert_eq!(next(&mut trace, b), Event::Exited(0));
            assert!(!trace.child.record().unwrap().retains_spawn_lease());
            require_reaped(&witness);
            let terminal = trace.poll(b).unwrap();
            assert!(terminal.is_terminal());
            assert_eq!(terminal.exit_code(), Some(0));
            assert_eq!(trace.cancel(), Poll::Reaped);
            assert_eq!(trace.cancel(), Poll::Reaped);
        }
        assert!(sibling.wait().unwrap().success());
    }

    if mode == "unwind" {
        assert!(
            catch_unwind(AssertUnwindSafe(move || {
                let _trace = trace;
                panic!("root trace unwind fixture");
            }))
            .is_err()
        );
    } else {
        drop(trace);
    }
    // Foreground ptrace is thread-bound; only the existing killed-root terminal
    // waiter is transferred to a different parent thread in this test.
    std::thread::scope(|scope| scope.spawn(|| drain(service)).join().unwrap());
    require_reaped(&witness);
    assert_eq!(b.storage(), retained);
}

struct Backing(Arc<AtomicUsize>);
impl Drop for Backing {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn steal_terminal(witness: &OwnedFd) {
    rustix::process::pidfd_send_signal(witness, Signal::KILL).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let status = rustix::process::waitid(
            WaitId::PidFd(witness.as_fd()),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG,
        )
        .unwrap();
        if let Some(status) = status {
            assert!(status.killed());
            return;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn retained(mode: &str, service: &mut Service, b: &mut Budget<'_>) {
    const INPUT: usize = 4096;
    let drops = Arc::new(AtomicUsize::new(0));
    b.reserve_storage(INPUT).unwrap();
    let (reservation, resources, charge) = service
        .reserve_retaining(Backing(drops.clone()), INPUT, b)
        .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (child, gate, witness) = spawn(reservation.into_slot());
    let original = Retained::<Backing>::storage_for(INPUT).unwrap();
    b.reserve_storage(original - b.storage()).unwrap();
    let retained = Retained::new(child, resources, original);
    let complete = retained.root_trace_storage().unwrap();
    b.reserve_storage(complete - original).unwrap();
    let mut trace = retained.into_root_trace(b).unwrap();
    assert_eq!(trace.retained_storage(), complete);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    trace
        .with_resources::<_, Error>(b, |backing, _| {
            assert!(Arc::ptr_eq(&backing.0, &drops));
            Ok(())
        })
        .unwrap();
    b.release_storage(1).unwrap();
    assert!(matches!(
        trace.poll(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        trace.resume(b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        trace.with_resources::<(), Error>(b, |_, _| panic!("unfunded backing callback")),
        Err(Error::Resource(Resource::Accounting))
    ));
    b.reserve_storage(1).unwrap();
    if mode == "retained-echld" {
        steal_terminal(&witness);
        assert!(matches!(
            trace.poll(b),
            Err(Error::Io {
                source: Errno::CHILD,
                ..
            })
        ));
        assert_eq!(trace.cancel(), Poll::Quarantined);
        drop(trace);
        service.pump(64).unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(service.report().unwrap().storage > Service::STORAGE);
        assert!(matches!(
            service.shutdown(),
            Err(crate::ProtectedServiceCleanupErrorV2::Busy)
        ));
        require_reaped(&witness);
        return;
    }
    if mode == "retained-drop" {
        let disposition = trace.cancel();
        assert_ne!(disposition, Poll::Quarantined);
        drop(trace);
        if disposition == Poll::Pending {
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            assert!(service.report().unwrap().storage > Service::STORAGE);
        }
        drain(service);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        require_reaped(&witness);
        assert_eq!(b.storage(), complete);
        return;
    }
    rustix::io::write(&gate, b"go\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match trace.poll(b).unwrap().state {
            Event::Pending => {}
            Event::Exec => trace.resume(b).unwrap(),
            Event::Exited(0) => break,
            other => panic!("unexpected retained event: {other:?}"),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(trace.cancel(), Poll::Reaped);
    require_reaped(&witness);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(trace);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(b.storage(), complete);
    drain(service);
}
