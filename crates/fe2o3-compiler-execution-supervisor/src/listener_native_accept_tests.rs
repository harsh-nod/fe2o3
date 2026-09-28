use super::*;
use crate::listener::{
    ListenerFilesystemPolicyV1 as Policy, ProtectedIssuerSocketCustodyV1 as Socket,
    ProtectedIssuerSocketStateV1 as State, require_socket_state,
};
use std::{cell::Cell, os::unix::fs::PermissionsExt};

fn pipe() -> (OwnedFd, OwnedFd) {
    rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK)
        .unwrap()
}

#[test]
fn finite_accept_validates_bounds_before_any_observation() {
    for (attempts, timeout) in [
        (0, MAX_TIMEOUT),
        (MAX_ATTEMPTS + 1, MAX_TIMEOUT),
        (1, Duration::ZERO),
        (1, MAX_TIMEOUT + Duration::from_nanos(1)),
    ] {
        assert!(matches!(
            bounded(attempts, timeout, |_| panic!(
                "invalid limits reached observation"
            )),
            Err(AcceptError::InvalidLimits)
        ));
    }
}

#[test]
fn finite_accept_counts_pending_turns_and_does_not_retry_terminal_errors() {
    for attempts in [1, 3, MAX_ATTEMPTS] {
        let calls = Cell::new(0);
        assert!(matches!(
            bounded(attempts, MAX_TIMEOUT, |_| {
                calls.set(calls.get() + 1);
                Ok(None)
            }),
            Err(AcceptError::Attempts)
        ));
        assert_eq!(calls.get(), attempts);
    }
    let calls = Cell::new(0);
    assert!(matches!(
        bounded(3, MAX_TIMEOUT, |_| {
            calls.set(calls.get() + 1);
            Err(AcceptError::Socket(SocketError::InvalidListener("changed")))
        }),
        Err(AcceptError::Socket(SocketError::InvalidListener("changed")))
    ));
    assert_eq!(calls.get(), 1);
}

#[test]
fn finite_accept_keeps_first_and_last_success_owned_until_caller_drop() {
    for success in [1, 3] {
        let (reader, writer) = pipe();
        let mut writer = Some(writer);
        let mut calls = 0;
        let accepted = bounded(3, MAX_TIMEOUT, |_| {
            calls += 1;
            Ok(if calls == success {
                writer.take()
            } else {
                None
            })
        })
        .unwrap();
        assert_eq!(calls, success);
        assert_eq!(rustix::io::read(&reader, &mut [0]), Err(Errno::AGAIN));
        drop(accepted);
        assert_eq!(rustix::io::read(&reader, &mut [0]).unwrap(), 0);
    }
}

#[test]
fn finite_accept_late_result_is_closed_without_returning_control() {
    let (reader, writer) = pipe();
    assert!(matches!(
        observed(Some(writer), Instant::now() - Duration::from_millis(1)),
        Err(AcceptError::Timeout)
    ));
    assert_eq!(rustix::io::read(&reader, &mut [0]).unwrap(), 0);
}

#[test]
fn socket_errors_preserve_legacy_reason_operation_and_errno() {
    use crate::ProtectedIssuerServiceErrorV1 as Legacy;
    assert!(matches!(
        Legacy::from(SocketError::InvalidListener("exact reason")),
        Legacy::InvalidListener("exact reason")
    ));
    match Legacy::from(socket_io_error("exact operation", Errno::PERM.into())) {
        Legacy::Io { operation, source } => {
            assert_eq!(operation, "exact operation");
            assert_eq!(source.raw_os_error(), Some(Errno::PERM.raw_os_error()));
        }
        other => panic!("lost socket error: {other:?}"),
    }
}

struct Root(std::path::PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("s"));
        if let Err(error) = std::fs::remove_dir(&self.0) {
            if std::thread::panicking() {
                eprintln!("listener test cleanup failed: {error}");
            } else {
                panic!("listener test root not empty: {error}");
            }
        }
    }
}

#[test]
fn named_socket_accepts_exact_control_and_rejects_path_drift() {
    use rustix::net::{AddressFamily, SocketAddrUnix, SocketType, bind, connect, socket_with};
    let root =
        std::path::PathBuf::from(format!("/tmp/fe2o3-native-listener-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let root = Root(root);
    std::fs::set_permissions(&root.0, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.0.join("s");
    let flags = SocketFlags::CLOEXEC | SocketFlags::NONBLOCK;
    let descriptor = socket_with(AddressFamily::UNIX, SocketType::SEQPACKET, flags, None).unwrap();
    bind(&descriptor, &SocketAddrUnix::new(&path).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660)).unwrap();
    let policy = Policy {
        parent_owner: rustix::process::geteuid().as_raw(),
        parent_group: rustix::process::getegid().as_raw(),
        parent_mode: 0o700,
        socket_owner: rustix::process::geteuid().as_raw(),
        socket_group: rustix::process::getegid().as_raw(),
        socket_mode: 0o660,
    };
    let socket = Socket::admit(descriptor, &path, policy).unwrap();
    require_socket_state(socket.revalidate().unwrap(), State::Bound).unwrap();
    let socket = socket.activate().unwrap();
    require_socket_state(socket.revalidate().unwrap(), State::Listening).unwrap();
    let client = socket_with(AddressFamily::UNIX, SocketType::SEQPACKET, flags, None).unwrap();
    connect(&client, &SocketAddrUnix::new(&path).unwrap()).unwrap();
    let control = accept(&socket.descriptor, 1, Duration::from_secs(1)).unwrap();
    assert_eq!(
        rustix::io::fcntl_getfd(&control).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
    assert!(
        rustix::fs::fcntl_getfl(&control)
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
    rustix::net::send(&client, b"x", rustix::net::SendFlags::NOSIGNAL).unwrap();
    assert_eq!(rustix::io::read(&control, &mut [0]).unwrap(), 1);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        socket.revalidate(),
        Err(SocketError::InvalidListener(_))
    ));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660)).unwrap();
    socket.revalidate().unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        socket.revalidate(),
        Err(SocketError::Io {
            operation: "inspect issuer listener pathname",
            ..
        })
    ));
}
