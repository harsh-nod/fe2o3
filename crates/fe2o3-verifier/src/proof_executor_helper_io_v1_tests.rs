use super::*;

fn channel() -> (OwnedFd, OwnedFd) {
    net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}

#[test]
fn bootstrap_rejects_missing_passcred_and_wrong_actual_parent() {
    let (left, _right) = channel();
    assert!(validate_bootstrap(&left, rustix::process::getpid()).is_err());
    net::sockopt::set_socket_passcred(&left, true).unwrap();
    assert!(validate_bootstrap(&left, rustix::process::getppid().unwrap()).is_err());
    assert!(Parent::capture(&left).is_err());
    assert_eq!(rustix::io::fcntl_getfd(&left).unwrap(), FdFlags::CLOEXEC);
}

#[test]
fn bootstrap_rejects_stream_blocking_and_inheritable_descriptors() {
    let (stream, _other) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::STREAM,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&stream, true).unwrap();
    assert!(validate_bootstrap(&stream, rustix::process::getpid()).is_err());
    let (left, _right) = channel();
    net::sockopt::set_socket_passcred(&left, true).unwrap();
    rustix::fs::fcntl_setfl(&left, OFlags::empty()).unwrap();
    assert!(validate_bootstrap(&left, rustix::process::getpid()).is_err());
    rustix::fs::fcntl_setfl(&left, OFlags::NONBLOCK).unwrap();
    rustix::io::fcntl_setfd(&left, FdFlags::empty()).unwrap();
    assert!(validate_bootstrap(&left, rustix::process::getpid()).is_err());
}

#[test]
fn actual_parent_observation_is_metered_and_not_a_child_wait_owner() {
    let pid = rustix::process::getppid().unwrap();
    let parent = Parent {
        pid,
        pidfd: rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).unwrap(),
    };
    parent.check().unwrap();
    let mut work = super::super::Work::new(PARENT_WORK - 1);
    let mut b = Budget::new(&mut work, 4096);
    let mut observer = Observer {
        parent: &parent,
        budget: &mut b,
    };
    assert!(matches!(observer.check(), Err(Error::Resource(_))));
    parent.check().unwrap();
    let wrong = Parent {
        pid: rustix::process::getpid(),
        pidfd: rustix::process::pidfd_open(
            rustix::process::getpid(),
            rustix::process::PidfdFlags::empty(),
        )
        .unwrap(),
    };
    assert!(wrong.check().is_err());
}

#[test]
fn raw_startup_slot_is_closed_on_refusal_or_transferred_once() {
    const CHILD: &str = "FE2O3_HELPER_RAW_SLOT_TEST";
    if let Ok(case) = std::env::var(CHILD) {
        // This is an isolated test process. No Rust object owns raw slot 3.
        let (left, _right) = channel();
        let private = rustix::io::fcntl_dupfd_cloexec(&left, 256).unwrap();
        drop(left);
        // SAFETY: subprocess test owns the unused raw slot and never returns to harness.
        assert_eq!(unsafe { libc::dup2(private.as_raw_fd(), 3) }, 3);
        let source = unsafe { Source::take() };
        match case.as_str() {
            "drop" => drop(source),
            "flags" => {
                assert_eq!(
                    unsafe { libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) },
                    0
                );
                assert!(source.validate().is_err());
                drop(source);
            }
            "transfer" => {
                let retained = source.into_owned().unwrap();
                assert!(retained.as_raw_fd() >= 256);
                assert_eq!(
                    rustix::io::fcntl_getfd(&retained).unwrap(),
                    FdFlags::CLOEXEC
                );
                drop(retained);
            }
            _ => panic!("unknown case"),
        }
        assert_eq!(unsafe { libc::fcntl(3, libc::F_GETFD) }, -1);
        std::process::exit(0);
    }
    for case in ["drop", "flags", "transfer"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "proof_executor_helper_v1::io::tests::raw_startup_slot_is_closed_on_refusal_or_transferred_once", "--nocapture"])
            .env(CHILD, case).output().unwrap();
        assert!(output.status.success(), "{case}: {:?}", output);
    }
}
