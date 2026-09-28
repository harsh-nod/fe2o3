use std::os::fd::{AsRawFd, IntoRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

use rustix::net::{AddressFamily, SocketFlags, SocketType};

use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientErrorV1,
    CompilerExecutionClientV1,
};

#[test]
fn canonical_inherited_child_slot_is_consumed_for_valid_and_invalid_peers() {
    if isolated() {
        return;
    }
    assert_reserved_slot_absent();
    let (client, service) = socket_pair(SocketType::SEQPACKET);
    install_reserved(client);
    // SAFETY: install_reserved relinquished the sole owner in this isolated
    // child; no other thread uses the slot, and this consumes that transfer once.
    let admitted =
        unsafe { CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)) }
            .expect("canonical inherited peer should be admitted");
    assert_reserved_slot_absent();
    let replacement =
        rustix::io::fcntl_dupfd_cloexec(&service, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1).unwrap();
    assert_eq!(
        replacement.as_raw_fd(),
        COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
    );
    drop(admitted);
    rustix::fs::fstat(&replacement).unwrap();
    drop(replacement);

    let (stream, _service) = socket_pair(SocketType::STREAM);
    install_reserved(stream);
    // SAFETY: this is a fresh, exclusively relinquished fixture descriptor, not
    // a retry of the consumed transfer. No other fixture thread touches the slot.
    assert!(matches!(
        unsafe { CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)) },
        Err(CompilerExecutionClientErrorV1::NotSeqpacket)
    ));
    assert_reserved_slot_absent();

    // SAFETY: this isolated missing-input case leaves the slot vacant for the
    // whole attempt, with no competing descriptor allocations or owners.
    assert!(matches!(
        unsafe { CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)) },
        Err(CompilerExecutionClientErrorV1::MissingInheritedPeer)
    ));
    assert_reserved_slot_absent();

    let (close_on_exec, _service) = socket_pair(SocketType::SEQPACKET);
    install_reserved_with_flags(close_on_exec, rustix::io::FdFlags::CLOEXEC);
    // SAFETY: the fresh fixture relinquished sole ownership despite its invalid
    // flags. This attempt alone may close the slot on refusal.
    assert!(matches!(
        unsafe { CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)) },
        Err(CompilerExecutionClientErrorV1::InheritedPeerCloseOnExec)
    ));
    assert_reserved_slot_absent();
}

fn install_reserved(source: OwnedFd) {
    install_reserved_with_flags(source, rustix::io::FdFlags::empty());
}

fn install_reserved_with_flags(source: OwnedFd, flags: rustix::io::FdFlags) {
    let installed =
        rustix::io::fcntl_dupfd_cloexec(&source, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1).unwrap();
    assert_eq!(
        installed.as_raw_fd(),
        COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
    );
    rustix::io::fcntl_setfd(&installed, flags).unwrap();
    assert_eq!(
        installed.into_raw_fd(),
        COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
    );
}

fn assert_reserved_slot_absent() {
    // SAFETY: F_GETFD reads descriptor flags without pointer arguments.
    assert_eq!(
        unsafe { libc::fcntl(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, libc::F_GETFD) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
}

fn socket_pair(socket_type: SocketType) -> (OwnedFd, OwnedFd) {
    rustix::net::socketpair(AddressFamily::UNIX, socket_type, SocketFlags::CLOEXEC, None).unwrap()
}

fn isolated() -> bool {
    const NAME: &str = "canonical_inherited_child_slot_is_consumed_for_valid_and_invalid_peers";
    const SENTINEL: &str = "FE2O3_CLIENT_V1_INHERITED_SLOT_TEST";
    if let Some(value) = std::env::var_os(SENTINEL) {
        assert_eq!(value, NAME);
        return false;
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", NAME, "--test-threads=1", "--nocapture"])
        .env(SENTINEL, NAME);
    // SAFETY: only close runs after fork, discarding this child's inherited
    // copy. It cannot affect owners in the parent test process.
    unsafe {
        command.pre_exec(|| {
            if libc::close(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1) != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::EBADF) {
                    return Err(error);
                }
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                assert!(status.success(), "isolated admission test: {status}");
                return true;
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated admission timed out or failed: {result:?}");
            }
        }
    }
}
