use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientErrorV1,
    CompilerExecutionClientV1,
};

#[test]
fn canonical_inherited_child_slot_is_consumed_for_valid_and_invalid_peers() {
    assert_reserved_slot_absent();
    let (client, _service) = socket_pair(libc::SOCK_SEQPACKET);
    install_reserved(client);
    let admitted = CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1))
        .expect("canonical inherited peer should be admitted");
    assert_reserved_slot_absent();
    drop(admitted);

    let (stream, _service) = socket_pair(libc::SOCK_STREAM);
    install_reserved(stream);
    assert!(matches!(
        CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)),
        Err(CompilerExecutionClientErrorV1::NotSeqpacket)
    ));
    assert_reserved_slot_absent();

    assert!(matches!(
        CompilerExecutionClientV1::admit_inherited_child(Duration::from_secs(1)),
        Err(CompilerExecutionClientErrorV1::Descriptor(_))
    ));
    assert_reserved_slot_absent();
}

fn install_reserved(source: OwnedFd) {
    assert_ne!(source.as_raw_fd(), COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
    // SAFETY: the reserved target was proven absent and dup3 installs a duplicate of the live
    // source descriptor without transferring source ownership.
    assert_eq!(
        unsafe {
            libc::dup3(
                source.as_raw_fd(),
                COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
                0,
            )
        },
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

fn socket_pair(socket_type: i32) -> (OwnedFd, OwnedFd) {
    let mut peers = [-1_i32; 2];
    // SAFETY: successful socketpair initializes both output descriptor slots.
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                socket_type | libc::SOCK_CLOEXEC,
                0,
                peers.as_mut_ptr(),
            )
        },
        0
    );
    // SAFETY: socketpair returned two distinct owned descriptors.
    unsafe {
        (
            OwnedFd::from_raw_fd(peers[0]),
            OwnedFd::from_raw_fd(peers[1]),
        )
    }
}
