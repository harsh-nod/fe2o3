use super::*;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};

use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1, CompilerExecutionExternalAnchorServiceIdentityV1,
    CompilerExecutionIssuerMeasurementV1,
};

fn request(kind: Kind) -> Packet {
    Packet {
        kind,
        session: [1; 32],
        launch: [2; 32],
        sequence: 1,
        operation: 1,
        nonce: [3; 32],
        body: Vec::new(),
    }
}

fn pair() -> (Endpoint, Endpoint) {
    let (a, b) = observer_pair().unwrap();
    (Endpoint::admit(a).unwrap(), Endpoint::admit(b).unwrap())
}

#[test]
fn receive_distinguishes_closed_channel_from_zero_length_datagram() {
    let (sender, receiver) = pair();
    let identity = current_identity().unwrap();
    raw_send(&sender, &[], &[]);
    assert!(matches!(
        receiver.receive(&identity),
        Err(CompilerExecutionObserverErrorV1::Protocol(_))
    ));
    sender.close();
    assert!(matches!(
        receiver.receive(&identity),
        Err(CompilerExecutionObserverErrorV1::Closed)
    ));
}

struct ChildOwner(Child);
impl Drop for ChildOwner {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn child_identity(child: &Child) -> LiveClientPidfdIdentityV1 {
    let pid = rustix::process::Pid::from_raw(child.id() as i32).unwrap();
    LiveClientPidfdIdentityV1::admit(
        rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).unwrap(),
        ExpectedClientProcessIdentityV1::new(
            child.id(),
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn canonical_packets_reject_replay_binding_mutations_and_malformed_headers() {
    for kind in [
        Kind::Begin,
        Kind::Revalidate,
        Kind::Finish,
        Kind::Revalidated,
        Kind::Finished,
    ] {
        let packet = request(kind);
        let bytes = packet.encode().unwrap();
        assert_eq!(Packet::decode(&bytes).unwrap(), packet);
        for length in 0..bytes.len() {
            assert!(Packet::decode(&bytes[..length]).is_err());
        }
        for offset in [0, 8, 9, 15] {
            let mut changed = bytes.clone();
            changed[offset] = 255;
            assert!(Packet::decode(&changed).is_err());
        }
        for (start, end) in [(16, 48), (48, 80), (80, 88), (88, 96), (96, 128)] {
            let mut changed = bytes.clone();
            changed[start..end].fill(0);
            assert!(Packet::decode(&changed).is_err());
        }
    }
    let packet = request(Kind::Begin);
    let response = packet.response(Kind::Begun, vec![0; OCCURRENCE_BODY]);
    assert!(packet.matches_response(&response, Kind::Begun));
    for offset in [16, 48, 80, 96] {
        let mut bytes = response.encode().unwrap();
        bytes[offset] ^= 4;
        assert!(!packet.matches_response(&Packet::decode(&bytes).unwrap(), Kind::Begun));
    }
    let mut wrong_operation = response.clone();
    wrong_operation.operation += 1;
    assert!(!packet.matches_response(&wrong_operation, Kind::Begun));
    assert!(!packet.matches_response(&response, Kind::Finished));
}

#[test]
fn packet_sender_is_not_the_root_created_socket_credentials() {
    let (sender, receiver) = pair();
    let mut command = Command::new("/bin/sleep");
    command.arg("30").stdin(Stdio::null());
    let child = ChildOwner(crate::test_process_execution::spawn(&mut command).unwrap());
    let actual = current_identity().unwrap();
    let other = child_identity(&child.0);
    assert_eq!(receiver.creator, actual.expected_client.credentials());
    assert!(sender.send(&request(Kind::Begin), &[]).unwrap());
    assert!(
        receiver.receive(&other).is_err(),
        "same UID and socket creator must not authenticate another sender PID"
    );
    assert!(sender.send(&request(Kind::Begin), &[]).unwrap());
    assert!(receiver.receive(&actual).unwrap().is_some());
    other.validate_parent(&actual).unwrap();
    assert!(actual.validate_parent(&other).is_err());
}

fn raw_send(endpoint: &Endpoint, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(control.push(SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        sendmsg(
            &endpoint.peer,
            &[IoSlice::new(bytes)],
            &mut control,
            SendFlags::NOSIGNAL
        )
        .unwrap(),
        bytes.len()
    );
}

#[test]
fn exact_descriptor_roster_and_bounded_datagrams_are_required() {
    let (sender, receiver) = pair();
    let identity = current_identity().unwrap();
    let file = tempfile::tempfile().unwrap();
    let begun = request(Kind::Begin)
        .response(Kind::Begun, vec![0; OCCURRENCE_BODY])
        .encode()
        .unwrap();
    for count in 0..=3 {
        let rights = vec![file.as_fd(); count];
        raw_send(&sender, &begun, &rights);
        let result = receiver.receive(&identity);
        assert_eq!(result.is_ok(), count == 2);
    }
    raw_send(
        &sender,
        &request(Kind::Begin).encode().unwrap(),
        &[file.as_fd()],
    );
    assert!(receiver.receive(&identity).is_err());
    raw_send(&sender, &vec![0; MAX_PACKET + 1], &[]);
    assert!(receiver.receive(&identity).is_err());
    assert!(receiver.receive(&identity).unwrap().is_none());
}

#[test]
fn nonblocking_backpressure_preserves_one_complete_packet() {
    let (sender, receiver) = pair();
    let identity = current_identity().unwrap();
    let packet = request(Kind::Begin);
    let mut queued = 0;
    loop {
        if !sender.send(&packet, &[]).unwrap() {
            break;
        }
        queued += 1;
        assert!(queued < 100_000);
    }
    assert!(queued > 0);
    for _ in 0..queued {
        let (received, rights) = receiver.receive(&identity).unwrap().unwrap();
        assert_eq!(received, packet);
        assert!(rights.is_empty());
    }
    assert!(receiver.receive(&identity).unwrap().is_none());
    assert!(sender.send(&packet, &[]).unwrap());
    assert_eq!(receiver.receive(&identity).unwrap().unwrap().0, packet);
}

fn policy() -> CompilerExecutionIssuerPolicyV1 {
    CompilerExecutionIssuerPolicyV1::new(
        1,
        CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
        crate::sealed_static_issuer_runtime_measurement_v1(),
        ed25519_dalek::SigningKey::from_bytes(&[2; 32])
            .verifying_key()
            .to_bytes(),
        ed25519_dalek::SigningKey::from_bytes(&[3; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap()
}

fn inert_channel() -> (ProtectedCompilerExecutionObserverV1, Endpoint) {
    let (endpoint, remote) = pair();
    let issuer = current_identity().unwrap();
    let expected = issuer.expected_client;
    let launch = CompilerExecutionServiceLaunchManifestV1::new(
        CompilerExecutionClientProcessIdentityV1::new(expected.pid, expected.uid, expected.gid)
            .unwrap(),
        CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 61001).unwrap(),
        &policy(),
    );
    let channel = ProtectedCompilerExecutionObserverV1 {
        service_binding: (
            endpoint.identity,
            remote.identity,
            (expected.pid, issuer.start_time_ticks),
        ),
        endpoint,
        root: current_identity().unwrap(),
        issuer,
        supervisor: (
            ExpectedClientProcessIdentityV1::new(
                rustix::process::getppid().unwrap().as_raw_pid() as u32,
                expected.uid,
                expected.gid,
            )
            .unwrap(),
            1,
        ),
        namespaces: ProtectedServiceNamespaceSetV1::capture_self().unwrap(),
        launch,
        session: [1; 32],
        production: false,
        state: RefCell::new(ChannelState {
            next_sequence: 1,
            active: None,
            poisoned: false,
        }),
    };
    (channel, remote)
}

#[test]
fn live_root_session_closure_is_terminal_before_any_new_exchange() {
    let (channel, remote) = inert_channel();
    channel.validate_continuity().unwrap();
    remote.close();
    channel.root.validate_liveness().unwrap();
    assert!(channel.validate_continuity().is_err());
    assert!(matches!(
        channel.validate_continuity(),
        Err(CompilerExecutionObserverErrorV1::Poisoned)
    ));
    assert!(matches!(
        channel.begin(),
        Err(CompilerExecutionObserverErrorV1::Poisoned)
    ));
}

#[test]
fn overlapping_operation_poison_is_terminal_without_refcell_panic() {
    let (channel, _remote) = inert_channel();
    channel.state.borrow_mut().active = Some(1);
    assert!(channel.begin().is_err());
    assert!(matches!(
        channel.validate_continuity(),
        Err(CompilerExecutionObserverErrorV1::Poisoned)
    ));
}

#[test]
fn dead_root_takes_precedence_over_queued_response() {
    let (sender, receiver) = pair();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let mut child = ChildOwner(crate::test_process_execution::spawn(&mut command).unwrap());
    let root = child_identity(&child.0);
    assert!(sender.send(&request(Kind::Revalidated), &[]).unwrap());
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    assert!(wait_for(&receiver, &root, libc::POLLIN, Instant::now() + TIMEOUT).is_err());
}

fn bytes_file(bytes: &[u8]) -> OwnedFd {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0)).unwrap();
    file.into()
}

/// Actual compiler occurrence, with test-only same-UID process-profile admission.
/// This does not qualify the production static issuer or deployment chain.
pub(crate) fn exercise_occurrence_channel(
    client: RetainedCompilerClientSessionV1,
    service: &ProtectedServiceAdmissionV1,
    scenario: &str,
    mut step_hook: impl FnMut(usize),
) {
    let policy = policy();
    let expected = client.client();
    let launch = CompilerExecutionServiceLaunchManifestV1::new(
        CompilerExecutionClientProcessIdentityV1::new(expected.pid, expected.uid, expected.gid)
            .unwrap(),
        CompilerExecutionExternalAnchorServiceIdentityV1::new(61001, 61001).unwrap(),
        &policy,
    );
    let (prepared, [peer, root_pidfd]) = PreparedRootCompilerExecutionObserverV1::prepare_inner(
        client,
        launch.clone(),
        &policy,
        false,
    )
    .unwrap();
    let descriptors = [
        peer,
        root_pidfd,
        service.try_clone_service_root().unwrap(),
        rustix::io::fcntl_dupfd_cloexec(service.service_peer(), 0).unwrap(),
        rustix::io::fcntl_dupfd_cloexec(service.client_pidfd(), 0).unwrap(),
        bytes_file(policy.canonical_bytes()),
        bytes_file(launch.canonical_bytes()),
    ];
    let sources: Vec<_> = descriptors
        .iter()
        .map(|fd| rustix::io::fcntl_dupfd_cloexec(fd, 240).unwrap())
        .collect();
    let raw: Vec<_> = sources.iter().map(AsRawFd::as_raw_fd).collect();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "linux::observer_channel::issuer::tests::issuer_helper",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_OBSERVER_TEST_SCENARIO", scenario)
        .stdin(Stdio::null());
    // SAFETY: sources are kept alive through spawn, destinations are disjoint, and the child
    // performs only async-signal-safe dup2/fcntl operations before exec.
    unsafe {
        command.pre_exec(move || {
            for (index, source) in raw.iter().enumerate() {
                let destination = 200 + index as i32;
                if libc::dup2(*source, destination) != destination
                    || libc::fcntl(destination, libc::F_SETFD, 0) != 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = ChildOwner(crate::test_process_execution::spawn(&mut command).unwrap());
    drop(sources);
    drop(descriptors);
    let mut root = prepared
        .bind(current_identity().unwrap(), child_identity(&child.0))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut replay_rejected = false;
    let mut mutation_rejected = false;
    let mut progress = 0;
    loop {
        match root.step() {
            Ok(RootCompilerExecutionObserverProgressV1::Exited) => break,
            Ok(RootCompilerExecutionObserverProgressV1::Progress) => progress += 1,
            Err(CompilerExecutionObserverErrorV1::Protocol(
                "session, launch, or request replay mismatch",
            )) => replay_rejected = true,
            Err(CompilerExecutionObserverErrorV1::Occurrence(_)) if scenario == "mutation" => {
                mutation_rejected = true
            }
            Err(error) => panic!("unexpected observer failure: {error}"),
            _ => (),
        }
        step_hook(progress);
        if scenario == "commit_failure" && progress == 5 {
            // The Revalidated response is now sent. Let the helper inject the commit failure
            // and assert poison before driving root containment of the abandoned operation.
            while child.0.try_wait().unwrap().is_none() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        assert!(Instant::now() < deadline, "observer test exceeded deadline");
        std::thread::sleep(Duration::from_millis(1));
    }
    let status = child.0.wait().unwrap();
    if scenario == "replay" || scenario == "mutation" {
        assert!(if scenario == "replay" {
            replay_rejected
        } else {
            mutation_rejected
        });
        assert_eq!(status.signal(), Some(libc::SIGKILL));
    } else {
        assert!(status.success(), "issuer helper failed: {status}");
    }
}

fn inherited(number: i32) -> OwnedFd {
    // SAFETY: the parent transfers each known live descriptor once to this helper.
    let fd = unsafe { OwnedFd::from_raw_fd(number) };
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC).unwrap();
    fd
}

fn read_inherited(number: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    File::from(inherited(number))
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

#[test]
#[ignore = "private subprocess helper; requires parent-owned observer and compiler descriptors"]
fn issuer_helper() {
    let peer = inherited(200);
    let root = inherited(201);
    let service_root = inherited(202);
    let service_peer = inherited(203);
    let client_pidfd = inherited(204);
    let policy = CompilerExecutionIssuerPolicyV1::decode(&read_inherited(205)).unwrap();
    let launch = CompilerExecutionServiceLaunchManifestV1::decode(&read_inherited(206)).unwrap();
    let client = launch.client();
    let client = LiveClientPidfdIdentityV1::admit(
        client_pidfd,
        ExpectedClientProcessIdentityV1::new(client.pid(), client.uid(), client.gid()).unwrap(),
    )
    .unwrap();
    let service = ProtectedServiceAdmissionV1::admit_non_authoritative_same_uid_session_test(
        service_root,
        service_peer,
        client,
    )
    .unwrap();
    let channel = ProtectedCompilerExecutionObserverV1::admit_inner(
        peer, root, &launch, &policy, &service, false,
    )
    .unwrap();
    let scenario = std::env::var("FE2O3_OBSERVER_TEST_SCENARIO").unwrap();
    if scenario == "replay" {
        let _guard = channel.begin().unwrap();
        let replay = Packet {
            session: channel.session,
            launch: *launch.identity().as_bytes(),
            ..request(Kind::Begin)
        };
        assert!(channel.endpoint.send(&replay, &[]).unwrap());
        std::thread::sleep(Duration::from_secs(30));
        panic!("root did not contain replaying issuer");
    }
    if scenario == "mutation" {
        let guard = channel.begin().unwrap();
        let _ = guard.revalidate();
        // The parent verifies the observation failure and contains this exact issuer.
        std::thread::sleep(Duration::from_secs(30));
        panic!("root did not contain changed observation");
    }
    crate::compiler_execution_issuer_durable::observer_tests::exercise(
        &channel,
        &policy,
        &ed25519_dalek::SigningKey::from_bytes(&[2; 32]),
        scenario == "commit_failure",
    );
}
