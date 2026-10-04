use super::*;
use crate::PendingCompilerExecutionChildChannelV1;
use crate::child_channel::{PIDFD_OPEN_CALLS, RESERVED_CHILD_FD_LOCK};
use std::process::Command;
use std::time::{Duration, Instant};

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().expect("reap owned fixture child");
    }
}

fn sleeper() -> OwnedChild {
    OwnedChild(Command::new("/bin/sleep").arg("30").spawn().unwrap())
}

fn wait_for_exit(child: &Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: valid output storage; WNOWAIT preserves the original child for capture.
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id(),
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            },
            0
        );
        // SAFETY: successful waitid initialized the zeroed output.
        if unsafe { info.assume_init().si_pid() } != 0 {
            return;
        }
        assert!(Instant::now() < deadline, "child did not exit");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn original_child_and_parent_are_captured_once_and_duplicates_do_not_reopen() {
    let _lock = RESERVED_CHILD_FD_LOCK.lock().unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let child = OwnedChild(command.spawn().unwrap());
    let before = PIDFD_OPEN_CALLS.get();
    let retained = RetainedCompilerExecutionChildV1::capture(&child.0).unwrap();
    assert_eq!(PIDFD_OPEN_CALLS.get(), before + 2);
    assert_eq!(retained.child_pid(), child.0.id());
    assert_eq!(retained.submitter(), current_submitter().unwrap());
    let original = retained.child_pidfd.as_raw_fd();
    let parent_info = std::fs::read_to_string(format!(
        "/proc/self/fdinfo/{}",
        retained.parent_pidfd.as_raw_fd()
    ))
    .unwrap();
    assert!(
        parent_info
            .lines()
            .any(|line| line == format!("Pid:\t{}", std::process::id()))
    );
    let launch = pending
        .finish_until_with_retained_child(&retained, Instant::now() + Duration::from_secs(2))
        .unwrap();
    assert_eq!(PIDFD_OPEN_CALLS.get(), before + 2);
    assert_eq!(launch.client().pid(), child.0.id());
    assert_eq!(launch.submitter(), retained.submitter());
    assert_ne!(launch.client_pidfd.as_raw_fd(), original);
    let child_info = std::fs::read_to_string(format!(
        "/proc/self/fdinfo/{}",
        launch.client_pidfd.as_raw_fd()
    ))
    .unwrap();
    assert!(
        child_info
            .lines()
            .any(|line| line == format!("Pid:\t{}", child.0.id()))
    );
    assert_eq!(retained.child_pidfd.as_raw_fd(), original);
    retained.validate_live_transfer().unwrap();
    drop(launch);
    retained.validate_custody().unwrap();
}

#[test]
fn exited_unreaped_child_is_retained_for_cleanup_but_cannot_transfer() {
    let mut child = OwnedChild(Command::new("/bin/true").spawn().unwrap());
    wait_for_exit(&child.0);
    let retained = RetainedCompilerExecutionChildV1::capture(&child.0).unwrap();
    retained.validate_custody().unwrap();
    assert!(matches!(
        retained.clone_for_live_transfer(),
        Err(CompilerExecutionChildChannelErrorV1::ChildExited)
    ));
    assert!(child.0.wait().unwrap().success());
    retained.validate_custody().unwrap();
    assert!(matches!(
        retained.clone_for_live_transfer(),
        Err(CompilerExecutionChildChannelErrorV1::ChildWait(_))
    ));
}

#[test]
fn already_reaped_child_cannot_be_captured() {
    let mut child = OwnedChild(Command::new("/bin/true").spawn().unwrap());
    child.0.wait().unwrap();
    assert!(matches!(
        RetainedCompilerExecutionChildV1::capture(&child.0),
        Err(CompilerExecutionChildChannelErrorV1::ChildWait(_))
    ));
}

#[test]
fn channel_for_another_child_rejects_without_losing_original_custody() {
    let _lock = RESERVED_CHILD_FD_LOCK.lock().unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let _channel_child = OwnedChild(command.spawn().unwrap());
    let other = sleeper();
    let retained = RetainedCompilerExecutionChildV1::capture(&other.0).unwrap();
    let original = retained.child_pidfd.as_raw_fd();
    assert!(matches!(
        pending
            .finish_until_with_retained_child(&retained, Instant::now() + Duration::from_secs(2)),
        Err(CompilerExecutionChildChannelErrorV1::ChildPidMismatch)
    ));
    assert_eq!(retained.child_pidfd.as_raw_fd(), original);
    retained.validate_live_transfer().unwrap();
}

#[test]
fn expired_deadline_does_not_duplicate_or_reopen_process_handles() {
    let _lock = RESERVED_CHILD_FD_LOCK.lock().unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let child = OwnedChild(command.spawn().unwrap());
    let retained = RetainedCompilerExecutionChildV1::capture(&child.0).unwrap();
    let before = PIDFD_OPEN_CALLS.get();
    assert!(matches!(
        pending.finish_until_with_retained_child(&retained, Instant::now()),
        Err(CompilerExecutionChildChannelErrorV1::Timeout)
    ));
    assert_eq!(PIDFD_OPEN_CALLS.get(), before);
    retained.validate_live_transfer().unwrap();
}

#[test]
fn either_missing_cloexec_fails_without_releasing_custody() {
    let child = sleeper();
    let retained = RetainedCompilerExecutionChildV1::capture(&child.0).unwrap();
    for descriptor in [&retained.child_pidfd, &retained.parent_pidfd] {
        rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::empty()).unwrap();
        assert!(matches!(
            retained.validate_custody(),
            Err(CompilerExecutionChildChannelErrorV1::MissingCloseOnExec)
        ));
        assert!(matches!(
            retained.clone_for_live_transfer(),
            Err(CompilerExecutionChildChannelErrorV1::MissingCloseOnExec)
        ));
        rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::CLOEXEC).unwrap();
    }
    retained.validate_live_transfer().unwrap();
}

#[test]
fn each_changed_parent_identity_component_rejects() {
    let child = sleeper();
    let mut retained = RetainedCompilerExecutionChildV1::capture(&child.0).unwrap();
    let original = retained.submitter;
    for (pid, uid, gid) in [
        (original.pid() + 1, original.uid(), original.gid()),
        (original.pid(), original.uid() + 1, original.gid()),
        (original.pid(), original.uid(), original.gid() + 1),
    ] {
        retained.submitter = CompilerExecutionClientProcessIdentityV1::new(pid, uid, gid).unwrap();
        assert!(matches!(
            retained.validate_custody(),
            Err(CompilerExecutionChildChannelErrorV1::ParentCredentialsMismatch)
        ));
        assert!(matches!(
            retained.clone_for_live_transfer(),
            Err(CompilerExecutionChildChannelErrorV1::ParentCredentialsMismatch)
        ));
    }
    retained.submitter = original;
    retained.validate_live_transfer().unwrap();
}
