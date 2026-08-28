#![cfg(target_os = "linux")]

use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionChildChannelErrorV1,
    PendingCompilerExecutionChildChannelV1,
};

static RESERVED_FD_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn child_creates_exact_pid_bound_service_channel() {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg("test -S /proc/self/fd/195 && exec /bin/sleep 30");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let mut child = command.spawn().unwrap();
    let launch = pending.finish(child.id(), Duration::from_secs(2)).unwrap();

    assert_eq!(launch.client().pid(), child.id());
    assert_eq!(launch.client().uid(), rustix::process::geteuid().as_raw());
    assert_eq!(launch.client().gid(), rustix::process::getegid().as_raw());
    assert_eq!(launch.submitter().pid(), std::process::id());
    assert_eq!(launch.submitter().uid(), launch.client().uid());
    assert_eq!(launch.submitter().gid(), launch.client().gid());
    assert_ne!(launch.submitter().pid(), launch.client().pid());
    assert!(
        rustix::io::fcntl_getfd(launch.service_peer())
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert!(
        rustix::io::fcntl_getfd(launch.client_pidfd())
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );

    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn occupied_reserved_descriptor_is_rejected_before_spawn() {
    let _guard = RESERVED_FD_LOCK.lock().unwrap();
    // SAFETY: dup2 either returns a fresh owned alias at the requested descriptor or reports an
    // error; the resulting File closes exactly that alias on drop.
    let occupied = unsafe {
        let source = File::open("/dev/null").unwrap();
        let descriptor = libc::dup2(source.as_raw_fd(), COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
        assert_eq!(descriptor, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
        File::from_raw_fd(descriptor)
    };
    let mut command = Command::new("/bin/true");
    assert!(matches!(
        PendingCompilerExecutionChildChannelV1::prepare(&mut command),
        Err(CompilerExecutionChildChannelErrorV1::ReservedDescriptorInUse)
    ));
    drop(occupied);
}

#[test]
fn invalid_finish_inputs_fail_without_waiting() {
    let mut command = Command::new("/bin/true");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    assert!(matches!(
        pending.finish(0, Duration::from_secs(1)),
        Err(CompilerExecutionChildChannelErrorV1::InvalidChildPid)
    ));

    let mut command = Command::new("/bin/true");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    assert!(matches!(
        pending.finish(std::process::id(), Duration::ZERO),
        Err(CompilerExecutionChildChannelErrorV1::InvalidTimeout)
    ));
}

#[test]
fn child_exit_before_admission_fails_closed() {
    let mut command = Command::new("/bin/true");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    let mut child = command.spawn().unwrap();
    while child.try_wait().unwrap().is_none() {
        std::thread::yield_now();
    }
    let result = pending.finish(child.id(), Duration::from_secs(1));
    assert!(
        matches!(
            result,
            Err(CompilerExecutionChildChannelErrorV1::ChildExited)
        ),
        "unexpected child-exit result: {result:?}"
    );
}

#[test]
fn later_child_callback_cannot_remove_the_installed_client_peer() {
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let pending = PendingCompilerExecutionChildChannelV1::prepare(&mut command).unwrap();
    // SAFETY: close is async-signal-safe and operates only on the fixed descriptor installed by
    // the preceding channel callback.
    unsafe {
        command.pre_exec(|| {
            if libc::close(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    assert!(matches!(
        pending.finish(child.id(), Duration::from_secs(2)),
        Err(CompilerExecutionChildChannelErrorV1::ServicePeerClosed)
    ));
    child.kill().unwrap();
    child.wait().unwrap();
}
