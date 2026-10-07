//! Synthetic transport controls: no deployed controller, real proof or load authority.
use super::*;
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationProofInputsV1 as ProofInputs, WorkerV3ApplicationProofKindV1 as ProofKind,
    WorkerV3ApplicationProofMessageV1 as ProofMessage,
    WorkerV3ApplicationProofSessionV1 as ProofSession,
};
use sha2::{Digest, Sha256};

pub(super) const CONTROL: &str = "FE2O3_TEST_CUSTODIAN_CONTROL";
const ENVELOPE: &[u8] = b"fixture only";
const PAYLOAD: &[u8] = b"inert payload; not executable evidence";
const SUBJECT: &[u8] = b"inert subject; not a proof";
const STRICT: &str = "FE2O3_TEST_CUSTODIAN_ALLOWLIST";

#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
#[path = "../../../../../cargo-fe2o3/src/application_sandbox.rs"]
mod cargo_sandbox;

pub(super) fn strict_allowlist() -> bool {
    std::env::var_os(STRICT).as_deref() == Some(std::ffi::OsStr::new("1"))
}

#[cfg(target_arch = "x86_64")]
pub(super) fn install_strict_allowlist() {
    let mut filter = cargo_sandbox::no_fork_application_filter();
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: unmodified production syscall filter restricts this isolated helper thread.
    // No exec listener is installed: this is post-entry API compatibility, not startup coverage.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(
            libc::syscall(
                libc::SYS_seccomp,
                libc::SECCOMP_SET_MODE_FILTER,
                0,
                &program
            ),
            0
        );
    }
    for syscall in [
        libc::SYS_pidfd_open,
        libc::SYS_pidfd_send_signal,
        libc::SYS_shutdown,
        libc::SYS_clone,
        libc::SYS_clone3,
        libc::SYS_fork,
        libc::SYS_socket,
        libc::SYS_socketpair,
        libc::SYS_connect,
        libc::SYS_setuid,
        libc::SYS_setgid,
    ] {
        // SAFETY: filter rejects each syscall before its invalid scalar arguments are used.
        assert_eq!(
            unsafe { libc::syscall(syscall, -1_i64, 0_i64, 0_i64, 0_i64, 0_i64, 0_i64) },
            -1
        );
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EPERM));
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn registration_rejected(case: &str) -> bool {
    matches!(
        case,
        "custodian_legacy_ready"
            | "custodian_wrong_pidfd"
            | "custodian_same_uid"
            | "custodian_missing_pidfd"
            | "custodian_extra_pidfd"
            | "custodian_root_role"
    )
}

fn request_not_sent(case: &str) -> bool {
    matches!(
        case,
        "custodian_envelope" | "custodian_empty_payload" | "custodian_expired"
    )
}

pub(super) fn application(
    endpoint: RetainedApplicationProofEndpointV1,
    inputs: WorkerV3ApplicationRegistrationInputsV1,
    case: &str,
) {
    let result = endpoint.register_custodian_pre_ack(inputs, deadline());
    if registration_rejected(case) {
        assert!(result.is_err(), "accepted invalid custodian: {case}");
        return;
    }
    let registered = result.unwrap();
    registered.revalidate().unwrap();
    if case == "custodian_root_after" {
        transmit(&take_control(), b"registered", &[]);
    }
    let session = registered.session().clone();
    let descriptor = registered.descriptor_identity();
    let proof = registered.request_proof(
        [42; 32],
        if case == "custodian_envelope" {
            b"different"
        } else {
            ENVELOPE
        },
        if case == "custodian_empty_payload" {
            b""
        } else {
            PAYLOAD
        },
        if case == "custodian_expired" {
            Instant::now()
        } else if case == "custodian_proof_timeout" {
            Instant::now() + Duration::from_millis(200)
        } else {
            deadline()
        },
    );
    if request_not_sent(case)
        || matches!(
            case,
            "custodian_wrong_sender"
                | "custodian_wrong_session"
                | "custodian_replay_active"
                | "custodian_proved_rights"
                | "custodian_first_subject"
                | "custodian_first_sequence"
                | "custodian_death_before_retained"
                | "custodian_proof_timeout"
                | "custodian_wrong_controller_uid"
                | "custodian_wrong_controller_gid"
        )
    {
        assert!(proof.is_err(), "accepted invalid proof: {case}");
        return;
    }
    let mut proof = proof.unwrap();
    assert_eq!(proof.session(), &session);
    assert_eq!(proof.descriptor_identity(), descriptor);
    assert_eq!(proof.subject_bytes(), SUBJECT);
    assert_eq!(proof.inputs().kernel(), [42; 32]);
    proof.revalidate().unwrap();
    let probe_deadline = if case == "custodian_timeout" {
        Instant::now() + Duration::from_millis(80)
    } else {
        deadline()
    };
    let result = proof.probe(probe_deadline);
    if case == "custodian_positive" || case == "custodian_root_after" {
        result.unwrap();
        proof.revalidate().unwrap();
    } else {
        assert!(result.is_err(), "accepted invalid retained proof: {case}");
        assert!(
            proof.revalidate().is_err(),
            "failed proof remained current: {case}"
        );
        assert!(
            proof.probe(deadline()).is_err(),
            "failed proof retried: {case}"
        );
    }
}

fn wait_closed(peer: &OwnedFd) {
    let until = deadline();
    loop {
        let mut bytes = [0; 1];
        match rustix::net::recv(peer, &mut bytes, RecvFlags::DONTWAIT) {
            Ok((0, _)) | Err(rustix::io::Errno::CONNRESET) => return,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                assert!(
                    Instant::now() < until,
                    "application did not close proof endpoint"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            other => panic!("unexpected traffic after terminal transition: {other:?}"),
        }
    }
}

fn packet(peer: &OwnedFd, session: &ProofSession, kind: ProofKind, sequence: u64, body: &[u8]) {
    transmit(
        peer,
        ProofMessage::new(kind, session.identity(), sequence, body)
            .unwrap()
            .canonical_bytes(),
        &[],
    );
}

fn take_control() -> OwnedFd {
    let raw = std::env::var(CONTROL).unwrap().parse::<RawFd>().unwrap();
    // SAFETY: this isolated helper alone owns the descriptor inherited from its fixture parent.
    let control = unsafe { OwnedFd::from_raw_fd(raw) };
    rustix::io::fcntl_setfd(&control, rustix::io::FdFlags::CLOEXEC).unwrap();
    control
}

pub(super) fn root_sender(peer: OwnedFd) {
    let control = take_control();
    let (bytes, rights) = receive_test(&control);
    assert!(rights.is_empty());
    let challenge = Message::decode(&bytes).unwrap();
    let root = pidfd(std::process::id());
    transmit(&peer, &bytes, &[root.as_fd()]);
    let (bytes, rights) = receive_test(&peer);
    assert!(rights.is_empty());
    let accept = Message::decode(&bytes).unwrap();
    assert_eq!(accept.kind(), Kind::Accept);
    assert_eq!(accept.transcript(), challenge.transcript());
    transmit(&control, b"accepted", &[]);
    let (ready, rights) = receive_test(&control);
    assert_eq!(rights.len(), 1);
    transmit(&peer, &ready, &[rights[0].as_fd()]);
    assert_eq!(receive_test(&control).0, b"exit");
}

pub(super) fn controller(peer: OwnedFd, case: &str) {
    let control = take_control();
    let (bytes, rights) = receive_test(&control);
    assert!(rights.is_empty());
    let session = ProofSession::decode(&bytes).unwrap();
    if registration_rejected(case) || request_not_sent(case) || case == "custodian_wrong_sender" {
        wait_closed(&peer);
        return;
    }
    if case == "custodian_wrong_session" {
        transmit(
            &peer,
            ProofMessage::new(ProofKind::Active, [55; 32], 1, &[])
                .unwrap()
                .canonical_bytes(),
            &[],
        );
        wait_closed(&peer);
        return;
    }
    packet(&peer, &session, ProofKind::Active, 1, &[]);
    if matches!(
        case,
        "custodian_wrong_controller_uid" | "custodian_wrong_controller_gid"
    ) {
        wait_closed(&peer);
        return;
    }
    let (bytes, files) = receive_test(&peer);
    let request = ProofMessage::decode(&bytes).unwrap();
    assert_eq!(request.kind(), ProofKind::Request);
    assert_eq!(request.session(), session.identity());
    assert_eq!(request.sequence(), 1);
    assert_eq!(files.len(), 2);
    let inputs = ProofInputs::decode(request.body()).unwrap();
    assert_eq!(inputs.kernel(), [42; 32]);
    assert_eq!(
        inputs.envelope(),
        (Sha256::digest(ENVELOPE).into(), ENVELOPE.len() as u64)
    );
    assert_eq!(
        inputs.payload(),
        (Sha256::digest(PAYLOAD).into(), PAYLOAD.len() as u64)
    );
    let mut objects = Vec::new();
    for (file, expected) in files.iter().zip([ENVELOPE, PAYLOAD]) {
        let stat = rustix::fs::fstat(file).unwrap();
        assert_eq!((stat.st_uid, stat.st_gid, stat.st_nlink), (1000, 1000, 0));
        assert_eq!(stat.st_mode & 0o7777, 0o400);
        assert_eq!(stat.st_size, expected.len() as i64);
        assert_eq!(
            rustix::fs::fcntl_getfl(file).unwrap() & OFlags::ACCMODE,
            OFlags::RDONLY
        );
        assert!(
            rustix::io::fcntl_getfd(file)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        assert_eq!(
            rustix::fs::fcntl_get_seals(file).unwrap(),
            rustix::fs::SealFlags::SEAL
                | rustix::fs::SealFlags::SHRINK
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::WRITE
        );
        let mut bytes = vec![0; expected.len()];
        assert_eq!(
            rustix::io::pread(file, &mut bytes, 0).unwrap(),
            expected.len()
        );
        assert_eq!(bytes, expected);
        objects.push((stat.st_dev, stat.st_ino));
    }
    assert_ne!(objects[0], objects[1]);
    if case == "custodian_proof_timeout" {
        wait_closed(&peer);
        return;
    }
    if case == "custodian_replay_active" {
        packet(&peer, &session, ProofKind::Active, 1, &[]);
        wait_closed(&peer);
        return;
    }
    let proved = ProofMessage::new(ProofKind::Proved, session.identity(), 1, SUBJECT).unwrap();
    let rights = if case == "custodian_proved_rights" {
        vec![files[0].as_fd()]
    } else {
        vec![]
    };
    transmit(&peer, proved.canonical_bytes(), &rights);
    if case == "custodian_death_before_retained" {
        return;
    }
    if !rights.is_empty() {
        wait_closed(&peer);
        return;
    }
    for sequence in [2, 3] {
        let (bytes, rights) = receive_test(&peer);
        assert!(rights.is_empty());
        let probe = ProofMessage::decode(&bytes).unwrap();
        assert_eq!(probe.kind(), ProofKind::Probe);
        assert_eq!(probe.session(), session.identity());
        assert_eq!(probe.sequence(), sequence);
        if sequence == 3 && case == "custodian_controller_death" {
            return;
        }
        if sequence == 3 && case == "custodian_timeout" {
            wait_closed(&peer);
            return;
        }
        let body = if (sequence == 2 && case == "custodian_first_subject")
            || (sequence == 3 && case == "custodian_later_subject")
        {
            b"substitution"
        } else {
            SUBJECT
        };
        let reply_sequence = if (sequence == 2 && case == "custodian_first_sequence")
            || (sequence == 3 && case == "custodian_later_sequence")
        {
            sequence + 1
        } else {
            sequence
        };
        packet(&peer, &session, ProofKind::Retained, reply_sequence, body);
        if body != SUBJECT || reply_sequence != sequence {
            break;
        }
    }
    wait_closed(&peer);
}

fn run(case: &str, strict: bool) {
    let (root_control, cargo_control) = rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    let raw = cargo_control.as_raw_fd();
    let mut command = helper("cargo", case, raw);
    command.env(STRICT, if strict { "1" } else { "0" });
    // SAFETY: only this fixture child's credentials and inherited descriptor are changed.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(raw, libc::F_SETFD, 0) != 0
                || libc::setgroups(0, std::ptr::null()) != 0
                || libc::setgid(1000) != 0
                || libc::setuid(1000) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut cargo = ChildGuard(command.spawn().unwrap());
    drop(cargo_control);
    let (bytes, mut rights) = receive_test(&root_control);
    assert_eq!(rights.len(), 1);
    let peer = rights.pop().unwrap();
    let binding = Binding::decode(&bytes).unwrap();
    let (bytes, rights) = receive_test(&peer);
    assert!(rights.is_empty());
    let hello = Message::decode(&bytes).unwrap();
    assert_eq!(hello.kind(), Kind::Hello);
    assert!(hello.inputs().unwrap().matches_binding(&binding));
    let root = pidfd(std::process::id());
    let challenge = Message::challenge(binding, hello.app_nonce(), [9; 32]).unwrap();
    let mut independent_root = if case == "custodian_root_after" {
        let (control, child_control) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        let raw = peer.as_raw_fd();
        let child_raw = child_control.as_raw_fd();
        let mut command = helper("custodian_root", case, raw);
        command.env(CONTROL, child_raw.to_string());
        // SAFETY: exposes only the two fixture-owned endpoints in the fork child.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(raw, libc::F_SETFD, 0) != 0
                    || libc::fcntl(child_raw, libc::F_SETFD, 0) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let process = ChildGuard(command.spawn().unwrap());
        drop(child_control);
        transmit(&control, challenge.canonical_bytes(), &[]);
        assert_eq!(receive_test(&control).0, b"accepted");
        Some((process, control))
    } else {
        transmit(&peer, challenge.canonical_bytes(), &[root.as_fd()]);
        let (bytes, rights) = receive_test(&peer);
        assert!(rights.is_empty());
        let accept = Message::decode(&bytes).unwrap();
        assert_eq!(accept.kind(), Kind::Accept);
        assert_eq!(accept.transcript(), challenge.transcript());
        None
    };

    let (control, child_control) = rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    let raw = peer.as_raw_fd();
    let child_raw = child_control.as_raw_fd();
    let mut command = helper("custodian_controller", case, raw);
    command.env(CONTROL, child_raw.to_string());
    // SAFETY: the fixture child receives these two endpoints and dedicated test credentials.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(raw, libc::F_SETFD, 0) != 0
                || libc::fcntl(child_raw, libc::F_SETFD, 0) != 0
                || libc::setgroups(0, std::ptr::null()) != 0
                || libc::setgid(1001) != 0
                || libc::setuid(1001) != 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut controller = ChildGuard(command.spawn().unwrap());
    drop(child_control);
    let controller_pidfd = pidfd(controller.0.id());
    let session = ProofSession::new(
        challenge.transcript().unwrap(),
        [20; 32],
        [21; 32],
        (
            if case == "custodian_root_role" {
                std::process::id()
            } else {
                controller.0.id()
            },
            if case == "custodian_same_uid" {
                1000
            } else if case == "custodian_wrong_controller_uid" {
                1002
            } else {
                1001
            },
            if case == "custodian_wrong_controller_gid" {
                1002
            } else {
                1001
            },
        ),
    )
    .unwrap();
    let ready = if case == "custodian_legacy_ready" {
        Message::ready(challenge.transcript().unwrap())
    } else {
        Message::custodian_ready(session.clone())
    };
    let rights = match case {
        "custodian_missing_pidfd" | "custodian_legacy_ready" => vec![],
        "custodian_wrong_pidfd" => vec![root.as_fd()],
        "custodian_extra_pidfd" => vec![controller_pidfd.as_fd(), controller_pidfd.as_fd()],
        _ => vec![controller_pidfd.as_fd()],
    };
    if let Some((process, root_channel)) = independent_root.as_mut() {
        transmit(root_channel, ready.canonical_bytes(), &rights);
        assert_eq!(receive_test(&root_control).0, b"registered");
        transmit(root_channel, b"exit", &[]);
        process.wait();
        // The actual root sender is positively reaped before any controller Active packet.
    } else {
        transmit(&peer, ready.canonical_bytes(), &rights);
    }
    if case == "custodian_wrong_sender" {
        packet(&peer, &session, ProofKind::Active, 1, &[]);
    }
    transmit(&control, session.canonical_bytes(), &[]);
    drop(peer);
    cargo.wait();
    controller.wait();
}

#[test]
#[ignore = "requires root in an isolated namespace; synthetic controller transport only"]
fn root_custodian_transport_campaign() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    for case in [
        "custodian_positive",
        "custodian_legacy_ready",
        "custodian_wrong_pidfd",
        "custodian_same_uid",
        "custodian_missing_pidfd",
        "custodian_extra_pidfd",
        "custodian_root_role",
        "custodian_envelope",
        "custodian_empty_payload",
        "custodian_expired",
        "custodian_wrong_sender",
        "custodian_wrong_controller_uid",
        "custodian_wrong_controller_gid",
        "custodian_wrong_session",
        "custodian_replay_active",
        "custodian_proved_rights",
        "custodian_first_subject",
        "custodian_first_sequence",
        "custodian_later_subject",
        "custodian_later_sequence",
        "custodian_timeout",
        "custodian_controller_death",
        "custodian_death_before_retained",
        "custodian_proof_timeout",
        "custodian_root_after",
    ] {
        eprintln!("ROOT_APPLICATION_CUSTODIAN_CASE={case}");
        run(case, false);
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
#[ignore = "requires isolated real root; exact Cargo filter installed after helper entry, synthetic proof"]
fn root_custodian_cargo_allowlist_campaign() {
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    for case in [
        "custodian_positive",
        "custodian_timeout",
        "custodian_controller_death",
        "custodian_root_after",
    ] {
        eprintln!("ROOT_APPLICATION_CUSTODIAN_ALLOWLIST_CASE={case}");
        run(case, true);
    }
}
