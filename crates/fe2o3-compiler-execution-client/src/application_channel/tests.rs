use super::*;
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const CHILD_FD: &str = "FE2O3_TEST_APPLICATION_PROOF_FD";

#[test]
fn reserved_fd195_is_relocated_in_an_isolated_process() {
    const CASE: &str = "FE2O3_TEST_PROOF_RESERVED_FD";
    if std::env::var_os(CASE).is_none() {
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application_channel::tests::reserved_fd195_is_relocated_in_an_isolated_process",
                "--nocapture",
            ])
            .env(CASE, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    let reserved =
        rustix::io::fcntl_dupfd_cloexec(&prepared.child, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1)
            .unwrap();
    assert_eq!(reserved.as_raw_fd(), COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
    let relocated = outside_reserved_slots(reserved).unwrap();
    assert!(relocated.as_raw_fd() > COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
    assert_eq!(
        EndpointSnapshot::inspect(&relocated).unwrap(),
        prepared.setup.snapshot
    );
    // SAFETY: scalar observation does not create an owner for the closed reserved number.
    assert_eq!(
        unsafe { libc::fcntl(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, libc::F_GETFD) },
        -1
    );
    assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EBADF));
}

struct ReapedChild(Child);

impl Drop for ReapedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn prepared_pair_has_exact_creator_addresses_and_flags() {
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    let child = EndpointSnapshot::inspect(&prepared.child).unwrap();
    let peer = EndpointSnapshot::inspect(&prepared.peer.peer).unwrap();
    assert_eq!(child.creator, current_creator(std::process::id()).unwrap());
    assert_eq!(child.creator, peer.creator);
    assert_eq!(child.local, peer.remote);
    assert_eq!(child.remote, peer.local);
    assert_ne!(child.object, peer.object);
    for fd in [&prepared.child, &prepared.peer.peer] {
        assert!(fd.as_raw_fd() > 2);
        assert_ne!(fd.as_raw_fd(), COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
    }
}

#[test]
fn after_spawn_closes_application_alias_even_with_setup_alive() {
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    let setup = prepared.child_setup();
    let peer = prepared.after_spawn();
    peer.revalidate().unwrap();
    let mut poll = libc::pollfd {
        fd: peer.peer.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll points to one initialized descriptor entry.
    assert_eq!(unsafe { libc::poll(&mut poll, 1, 1000) }, 1);
    assert_ne!(poll.revents & libc::POLLHUP, 0);
    assert!(setup.descriptor() > 2);
}

#[test]
fn creator_cannot_admit_its_own_endpoint_as_inherited() {
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    assert!(RetainedApplicationProofEndpointV1::admit_inherited(prepared.child).is_err());
}

#[test]
fn inherited_endpoint_rejects_unnamed_stream_and_wrong_flags() {
    for kind in [SocketType::SEQPACKET, SocketType::STREAM] {
        let (child, _peer) = rustix::net::socketpair(
            AddressFamily::UNIX,
            kind,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        assert!(EndpointSnapshot::inspect(&child).is_err());
    }
    for alteration in 0..3 {
        let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
        match alteration {
            0 => rustix::net::sockopt::set_socket_passcred(&prepared.child, false).unwrap(),
            1 => rustix::fs::fcntl_setfl(&prepared.child, OFlags::RDWR).unwrap(),
            _ => rustix::io::fcntl_setfd(&prepared.child, rustix::io::FdFlags::empty()).unwrap(),
        }
        assert!(EndpointSnapshot::inspect(&prepared.child).is_err());
    }
}

#[test]
fn peer_revalidation_rejects_changed_flags() {
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    prepared.peer.revalidate().unwrap();
    rustix::net::sockopt::set_socket_passcred(&prepared.peer.peer, false).unwrap();
    assert!(prepared.peer.revalidate().is_err());
}

#[test]
fn child_setup_rejects_substitution_without_changing_parent_flags() {
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    let mut setup = prepared.child_setup();
    let other = PreparedApplicationProofChannelV1::prepare().unwrap();
    setup.descriptor = other.child.as_raw_fd();
    let mut command = Command::new("/bin/true");
    // SAFETY: only the fork child's descriptor table is inspected, while both owners are live.
    unsafe {
        command.pre_exec(move || setup.expose_before_exec());
    }
    assert_eq!(
        command.spawn().unwrap_err().raw_os_error(),
        Some(libc::ESTALE)
    );
    assert_eq!(
        rustix::io::fcntl_getfd(&other.child).unwrap(),
        rustix::io::FdFlags::CLOEXEC
    );
}

#[test]
fn original_child_claims_and_revalidates_inherited_endpoint() {
    let prepared = PreparedApplicationProofChannelV1::prepare().unwrap();
    let setup = prepared.child_setup();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "application_channel::tests::inherited_endpoint_child",
            "--nocapture",
        ])
        .env(CHILD_FD, setup.descriptor().to_string());
    // SAFETY: the fork child has exclusive descriptor-table access and the pair is retained.
    unsafe {
        command.pre_exec(move || setup.expose_before_exec());
    }
    let mut child = ReapedChild(command.spawn().unwrap());
    let peer = prepared.after_spawn();
    peer.revalidate().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "inherited proof child timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut poll = libc::pollfd {
        fd: peer.peer.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll points to one initialized descriptor entry.
    assert_eq!(unsafe { libc::poll(&mut poll, 1, 1000) }, 1);
    assert_ne!(poll.revents & libc::POLLHUP, 0);
}

#[test]
fn inherited_endpoint_child() {
    let Ok(raw) = std::env::var(CHILD_FD) else {
        return;
    };
    let raw = raw.parse::<RawFd>().unwrap();
    // SAFETY: the parent transferred only this owned descriptor to this exact helper process.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    assert_eq!(
        rustix::io::fcntl_getfd(&fd).unwrap(),
        rustix::io::FdFlags::empty()
    );
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC).unwrap();
    let endpoint = RetainedApplicationProofEndpointV1::admit_inherited(fd).unwrap();
    endpoint.revalidate().unwrap();
    rustix::net::sockopt::set_socket_passcred(&endpoint.peer, false).unwrap();
    assert!(endpoint.revalidate().is_err());
}
