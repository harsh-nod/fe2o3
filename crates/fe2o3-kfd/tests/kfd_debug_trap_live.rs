#![cfg(all(
    feature = "live-validation",
    target_os = "linux",
    target_arch = "x86_64"
))]
#![allow(unsafe_code)]

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use std::os::unix::process::CommandExt;

use fe2o3_kfd::{
    DEFAULT_KFD_PATH, DeviceSelector, KfdAdapterError, KfdDebugExceptionInfoV1,
    KfdDebugQueueOperationStateV1, KfdDebugSessionErrorV1, KfdDebugSessionPlanV1,
    KfdLiveDebugSessionErrorV1, KfdLiveDebugSessionV1, KfdOpaqueCheckpointObservationV1,
    KfdStoppedAvailabilityV1, KfdStoppedContextSaveObservationV1, KfdStoppedQueueCapturePlanV1,
    KfdStoppedSnapshotOwnershipV1, KfdStoppedStateScopeV1, KfdStoppedUnavailableReasonV1,
    KfdTargetRuntimeDebugTokenV1, OpenedKfd,
};
use fe2o3_kfd_uapi::{
    KfdDebugExceptionMaskV1, KfdDebugRuntimeStateV1, KfdDebugTrapExceptionCodeV1,
    KfdDebugTrapWaveLaunchModeV1,
};

const HELPER_ENV: &str = "FE2O3_KFD_DEBUG_TRAP_LIVE_HELPER";
const WAIT_BOUND: Duration = Duration::from_secs(10);

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn child(&mut self) -> &mut Child {
        self.0.as_mut().expect("child remains owned")
    }

    fn finish(mut self, deadline: Instant) {
        let mut child = self.0.take().expect("child remains owned");
        let Some(status) = wait_for_exit(&mut child, deadline) else {
            let _ = child.kill();
            let _ = wait_for_exit(&mut child, Instant::now() + Duration::from_secs(2));
            panic!("timed out waiting for detached target exit");
        };
        assert!(status.success(), "target helper failed: {status}");
    }

    fn terminate(mut self) {
        let mut child = self.0.take().expect("child remains owned");
        let _ = child.kill();
        reap_bounded(&mut child, Instant::now() + Duration::from_secs(2));
    }
}

fn reap_bounded(child: &mut Child, deadline: Instant) {
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_exit(child: &mut Child, deadline: Instant) -> Option<std::process::ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {}
            Err(error) => panic!("failed to poll target exit: {error}"),
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(5));
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            reap_bounded(child, Instant::now() + Duration::from_secs(2));
        }
    }
}

fn wait_for_stop(child: &mut Child, deadline: Instant) -> i32 {
    loop {
        let mut status = 0;
        // SAFETY: status points to one live integer and the child PID is owned
        // by this test. WNOHANG keeps the cleanup deadline enforceable.
        let result = unsafe {
            libc::waitpid(
                child.id() as libc::pid_t,
                &mut status,
                libc::WNOHANG | libc::WUNTRACED,
            )
        };
        if result == child.id() as libc::pid_t {
            assert!(
                libc::WIFSTOPPED(status),
                "target exited before stop: {status}"
            );
            return status;
        }
        assert!(
            result >= 0,
            "waitpid failed: {}",
            std::io::Error::last_os_error()
        );
        assert!(
            Instant::now() < deadline,
            "timed out waiting for target stop"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn continue_tracee(child: &Child) {
    // SAFETY: the test owns a ptrace-stopped child and passes no address/data.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_CONT,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    assert_eq!(
        result,
        0,
        "PTRACE_CONT failed: {}",
        std::io::Error::last_os_error()
    );
}

fn continue_group_stopped_tracee(child: &Child) {
    // SAFETY: the test owns the ptrace-stopped child. Injecting SIGCONT both
    // advances ptrace and clears the process-wide SIGSTOP state.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_CONT,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            libc::SIGCONT as usize as *mut libc::c_void,
        )
    };
    assert_eq!(
        result,
        0,
        "PTRACE_CONT(SIGCONT) failed: {}",
        std::io::Error::last_os_error()
    );
}

fn stop_tracee_process_leader() {
    let pid = std::process::id() as libc::pid_t;
    // SAFETY: tgkill receives the current process and leader task IDs and a
    // fixed signal. Targeting the leader makes the ptrace wait identity stable
    // even though libtest runs the helper body on a worker thread.
    let result = unsafe { libc::syscall(libc::SYS_tgkill, pid, pid, libc::SIGSTOP) };
    assert_eq!(
        result,
        0,
        "tgkill(SIGSTOP) failed: {}",
        std::io::Error::last_os_error()
    );
}

fn wait_for_debugger_release() {
    let mut byte = [0_u8; 1];
    std::io::stdin()
        .read_exact(&mut byte)
        .expect("debugger release pipe closed");
    assert_eq!(byte, [b'.']);
}

fn release_helper(child: &mut Child) {
    child
        .stdin
        .as_mut()
        .expect("helper stdin remains owned")
        .write_all(b".")
        .expect("failed to release helper stage");
}

fn detach_tracee(child: &Child) {
    // SAFETY: the test owns a ptrace-stopped child. SIGCONT is an integer
    // signal operand, encoded in ptrace's untyped data slot, which clears the
    // helper's process-wide SIGSTOP while detaching it.
    let result = unsafe {
        libc::ptrace(
            libc::PTRACE_DETACH,
            child.id() as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            libc::SIGCONT as usize as *mut libc::c_void,
        )
    };
    assert_eq!(
        result,
        0,
        "PTRACE_DETACH failed: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: the PID still belongs to the retained Child. A separate SIGCONT
    // also clears group-stop state on kernels that do not consume it through
    // the detach signal operand before returning.
    let resumed = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGCONT) };
    if resumed != 0 {
        let error = std::io::Error::last_os_error();
        assert_eq!(
            error.raw_os_error(),
            Some(libc::ESRCH),
            "SIGCONT failed: {error}"
        );
    }
}

fn acknowledge_runtime_state(
    session: &mut KfdLiveDebugSessionV1,
    expected: KfdDebugRuntimeStateV1,
    deadline: Instant,
) {
    let runtime_mask =
        KfdDebugExceptionMaskV1::from_code(KfdDebugTrapExceptionCodeV1::ProcessRuntime);
    loop {
        let _ = session.drain_notifications(1_024).unwrap();
        if let Some(event) = session.query_event(KfdDebugExceptionMaskV1::NONE).unwrap()
            && event
                .exceptions()
                .contains(KfdDebugTrapExceptionCodeV1::ProcessRuntime)
        {
            let info = session
                .query_exception_info(0, KfdDebugTrapExceptionCodeV1::ProcessRuntime, true)
                .unwrap();
            assert!(matches!(
                info,
                KfdDebugExceptionInfoV1::Runtime(runtime) if runtime.state() == expected
            ));
            session.acknowledge_runtime_transition(event).unwrap();
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected:?}"
        );
        // Reasserting the subscribed mask is not needed; this only proves the
        // test uses the same reviewed process-runtime bit as session admission.
        assert!(runtime_mask.contains(KfdDebugTrapExceptionCodeV1::ProcessRuntime));
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_one_queue(
    session: &mut KfdLiveDebugSessionV1,
    exceptions_to_clear: KfdDebugExceptionMaskV1,
    deadline: Instant,
) -> fe2o3_kfd::KfdDebugQueueObservationV1 {
    loop {
        let queues = session.queue_snapshot(exceptions_to_clear).unwrap();
        if let [queue] = queues.as_slice() {
            return *queue;
        }
        assert!(queues.is_empty(), "unexpected queue snapshot: {queues:?}");
        assert!(
            Instant::now() < deadline,
            "timed out waiting for one KFD queue snapshot"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn live_target_helper() {
    if std::env::var_os(HELPER_ENV).is_none() {
        return;
    }
    let unique_id = fe2o3_kfd::topology::discover_default_topology()
        .unwrap()
        .topology()
        .gpu_nodes()
        .first()
        .expect("MI300X live validation requires one GPU")
        .unique_id();
    let device = OpenedKfd::open_default()
        .unwrap()
        .admit_uapi()
        .unwrap()
        .bind_gfx942_xnack_minus(DeviceSelector::UniqueId(unique_id))
        .unwrap();
    let token = KfdTargetRuntimeDebugTokenV1::enable_current_process().unwrap();
    let queue = token.create_compute_aql_queue(device, 4096).unwrap();
    assert_eq!(queue.observation().queue_id(), 0);
    stop_tracee_process_leader();
    wait_for_debugger_release();
    let teardown = queue.destroy().unwrap();
    stop_tracee_process_leader();
    wait_for_debugger_release();
    let destroyed = teardown.finish().unwrap();
    assert_eq!(destroyed.queue_id(), 0);
    // Give the debugger a live, stopped target while it disables debug-trap.
    stop_tracee_process_leader();
    wait_for_debugger_release();
}

#[test]
fn mi300x_ptrace_runtime_handshake_and_typed_gate() {
    if std::env::var_os(HELPER_ENV).is_some() {
        return;
    }
    let executable = std::env::current_exe().unwrap();
    let mut command = Command::new(executable);
    command
        .args(["--exact", "live_target_helper", "--nocapture"])
        .env(HELPER_ENV, "1")
        .env_remove("RUST_MIN_STACK")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    // SAFETY: the post-fork hook performs one pointer-free, async-signal-safe
    // ptrace syscall. Successful exec then stops the new process leader with
    // SIGTRAP before the helper can open KFD.
    unsafe {
        command.pre_exec(|| {
            let result = libc::ptrace(
                libc::PTRACE_TRACEME,
                0,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            );
            if result == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let child = command.spawn().unwrap();
    let mut child = ChildGuard(Some(child));
    wait_for_stop(child.child(), Instant::now() + WAIT_BOUND);

    let runtime_mask =
        KfdDebugExceptionMaskV1::from_code(KfdDebugTrapExceptionCodeV1::ProcessRuntime);
    let plan = KfdDebugSessionPlanV1::new(child.child().id(), std::process::id(), runtime_mask, 64)
        .unwrap();
    let mut session = match KfdLiveDebugSessionV1::attach(plan) {
        Ok(session) => session,
        Err(KfdLiveDebugSessionErrorV1::Kfd(KfdAdapterError::Open { path, source }))
            if path.as_path() == std::path::Path::new(DEFAULT_KFD_PATH)
                && source == rustix::io::Errno::NOENT =>
        {
            // A genuinely missing device is an environment skip. Every other
            // KFD error remains a hard failure and an existing device is never
            // masked by this branch.
            eprintln!("SKIP: {DEFAULT_KFD_PATH} is absent after ptrace/pidfd admission");
            detach_tracee(child.child());
            child.terminate();
            return;
        }
        Err(error) => panic!("live KFD debug attach failed: {error}"),
    };
    assert_eq!(
        session.runtime_observation().state(),
        KfdDebugRuntimeStateV1::Disabled
    );
    assert!(matches!(
        session.set_launch_mode(KfdDebugTrapWaveLaunchModeV1::Halt),
        Err(KfdLiveDebugSessionErrorV1::Session(
            KfdDebugSessionErrorV1::RuntimeNotEnabled
        ))
    ));

    let queue_new = KfdDebugExceptionMaskV1::from_code(KfdDebugTrapExceptionCodeV1::QueueNew);
    let subscribed = KfdDebugExceptionMaskV1::new(runtime_mask.bits() | queue_new.bits()).unwrap();
    session.set_exceptions(subscribed).unwrap();
    continue_tracee(child.child());
    acknowledge_runtime_state(
        &mut session,
        KfdDebugRuntimeStateV1::Enabled,
        Instant::now() + WAIT_BOUND,
    );
    wait_for_stop(child.child(), Instant::now() + WAIT_BOUND);
    let queue = wait_for_one_queue(&mut session, queue_new, Instant::now() + WAIT_BOUND);
    assert_eq!(queue.queue_id(), 0);
    assert_eq!(queue.ring_size(), 4096);
    // Depending on whether QueueNew publication raced the snapshot's clear,
    // the subscribed event can still be pending. Querying it with the same
    // clear mask is idempotent and closes both admitted KFD orderings.
    if let Some(event) = session.query_event(queue_new).unwrap() {
        assert!(
            event
                .exceptions()
                .contains(KfdDebugTrapExceptionCodeV1::QueueNew),
            "unexpected event while clearing QueueNew: {event:?}"
        );
    }

    let suspension = session
        .suspend_queues(&[queue.queue_id()], KfdDebugExceptionMaskV1::NONE, u32::MAX)
        .unwrap();
    assert_eq!(
        suspension.len(),
        1,
        "unexpected suspend result: {suspension:?}"
    );
    assert_eq!(
        suspension[0].state(),
        KfdDebugQueueOperationStateV1::Complete,
        "unexpected suspend result: {suspension:?}"
    );
    let stopped = session
        .capture_stopped_queue_v1(KfdStoppedQueueCapturePlanV1::new(
            queue.queue_id(),
            KfdStoppedStateScopeV1::new([0x73; 32]).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        stopped.ownership(),
        KfdStoppedSnapshotOwnershipV1::SessionRetainedSuspension
    );
    let layout = match stopped.context_save() {
        KfdStoppedContextSaveObservationV1::Available(layout) => layout,
        other => panic!("live gfx942 context-save header capture unavailable: {other:?}"),
    };
    assert_eq!(layout.context_bytes_per_xcc(), 0x162_1000);
    assert_eq!(layout.total_allocation_bytes(), 0xb16_7000);
    assert_eq!(layout.headers().len(), 8);
    let checkpoint = match stopped.opaque_checkpoint() {
        KfdOpaqueCheckpointObservationV1::Complete(checkpoint) => checkpoint,
        other => panic!("live gfx942 opaque checkpoint capture unavailable: {other:?}"),
    };
    assert_eq!(checkpoint.captured_bytes(), 0);
    assert!(checkpoint.segments().is_empty());
    assert_eq!(
        stopped.waves(),
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::WaveRecordLayoutNotInKfdUapi
        )
    );
    let resumed = session.resume_queues(&[queue.queue_id()]).unwrap();
    assert!(matches!(
        resumed.as_slice(),
        [observation] if observation.state() == KfdDebugQueueOperationStateV1::Complete
    ));

    continue_group_stopped_tracee(child.child());
    release_helper(child.child());
    wait_for_stop(child.child(), Instant::now() + WAIT_BOUND);
    assert!(
        session
            .queue_snapshot(KfdDebugExceptionMaskV1::NONE)
            .unwrap()
            .is_empty()
    );

    continue_group_stopped_tracee(child.child());
    release_helper(child.child());
    acknowledge_runtime_state(
        &mut session,
        KfdDebugRuntimeStateV1::Disabled,
        Instant::now() + WAIT_BOUND,
    );
    wait_for_stop(child.child(), Instant::now() + WAIT_BOUND);
    session.finish().unwrap();
    detach_tracee(child.child());
    release_helper(child.child());
    child.finish(Instant::now() + WAIT_BOUND);
}
