//! Default tests exercise endpoint prerequisites and accounting, never Admission.
use super::*;

// An inert record/state fixture. It constructs neither endpoint nor Admission.
fn established() -> Established {
    use sha2::Digest;
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(b"F2O3CRC3");
    bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
    bytes[12..20].copy_from_slice(&(BYTES as u64).to_le_bytes());
    bytes[24..152].fill(1);
    bytes[152..160].copy_from_slice(&1u64.to_le_bytes());
    let mut digest = sha2::Sha256::new();
    digest.update(b"FE2O3/ROOT-ISSUER-CONTROL/V3\0");
    digest.update(&bytes[..BYTES - 32]);
    bytes[BYTES - 32..].copy_from_slice(&digest.finalize());
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(BYTES).unwrap();
    let (gate, _) = Record::decode(&bytes, &mut b).unwrap();
    Established {
        gate,
        deadline: Instant::now() + TIMEOUT,
        attempts: 73,
        next_sequence: 9,
    }
}

#[test]
fn established_refusal_or_unwind_consumes_state_without_renewing_its_limits() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, 1_000_000);
    let mut state = ConnectionState::Established(established());
    assert!(state.begin_handshake(&mut b).is_err());
    assert!(matches!(state, ConnectionState::Failed));

    let value = established();
    let deadline = value.deadline;
    let mut state = ConnectionState::Established(value);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let session = state.take_established(&mut b).unwrap();
        assert_eq!(session.deadline, deadline);
        assert_eq!((session.attempts, session.next_sequence), (73, 9));
        assert_eq!(session.following_sequence().unwrap(), 10);
        panic!("injected exchange failure after state consumption");
    }));
    assert!(result.is_err());
    assert!(matches!(state, ConnectionState::Failed));
    assert!(state.take_established(&mut b).is_err());
    assert!(state.begin_handshake(&mut b).is_err());
    assert_eq!(b.work(), 4 * ENTRY);
}

#[test]
fn exhausted_sequence_refuses_after_poisoning_without_wrapping() {
    let mut work = Work::new(2 * ENTRY);
    let mut b = Budget::new(&mut work, 1_000_000);
    let mut value = established();
    value.next_sequence = u64::MAX;
    let mut state = ConnectionState::Established(value);
    let session = state.take_established(&mut b).unwrap();
    assert_eq!(
        session.following_sequence().unwrap_err().resource(),
        Some(Resource::Arithmetic)
    );
    assert_eq!(session.next_sequence, u64::MAX);
    assert!(matches!(state, ConnectionState::Failed));
    assert!(state.take_established(&mut b).is_err());
    assert_eq!(b.work(), 2 * ENTRY);
}

#[test]
fn handshake_and_exchange_refusals_cannot_reset_the_connection() {
    let mut work = Work::new(5 * ENTRY);
    let mut b = Budget::new(&mut work, 1_000_000);
    let mut state = ConnectionState::Fresh;
    state.begin_handshake(&mut b).unwrap();
    assert!(matches!(state, ConnectionState::Failed));
    assert!(state.begin_handshake(&mut b).is_err());
    assert!(state.take_established(&mut b).is_err());
    assert!(matches!(state, ConnectionState::Failed));

    // Calling an exchange before the gate consumes the unauthenticated state.
    // This is an inert state test, not successful endpoint/Admission custody.
    let mut state = ConnectionState::Fresh;
    assert!(state.take_established(&mut b).is_err());
    assert!(state.begin_handshake(&mut b).is_err());
    assert!(matches!(state, ConnectionState::Failed));
    assert_eq!(b.work(), 5 * ENTRY);
}

#[test]
fn failed_handshake_unwind_keeps_the_same_terminal_state() {
    let mut work = Work::new(3 * ENTRY);
    let mut b = Budget::new(&mut work, 1_000_000);
    let mut state = ConnectionState::Fresh;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state.begin_handshake(&mut b).unwrap();
        panic!("injected handshake failure after one-use transition");
    }));
    assert!(result.is_err());
    assert!(matches!(state, ConnectionState::Failed));
    assert!(state.begin_handshake(&mut b).is_err());
    assert!(state.take_established(&mut b).is_err());
    assert_eq!(b.work(), 3 * ENTRY);
}

#[test]
fn unpaid_connection_entry_still_poisons_without_refunding_prior_work() {
    for limit in [0, ENTRY - 1, ENTRY] {
        for handshake in [false, true] {
            let mut work = Work::new(17 + limit);
            let mut b = Budget::new(&mut work, 100);
            b.charge_work(17).unwrap();
            b.reserve_storage(23).unwrap();
            let account = b.work_ledger_identity_v1();
            let mut state = if handshake {
                ConnectionState::Fresh
            } else {
                ConnectionState::Established(established())
            };
            let result = if handshake {
                state.begin_handshake(&mut b)
            } else {
                state.take_established(&mut b).map(drop)
            };
            if limit < ENTRY {
                assert!(result.unwrap_err().resource().is_some());
                assert_eq!(b.work(), 17);
            } else {
                result.unwrap();
                assert_eq!(b.work(), 17 + ENTRY);
            }
            assert!(matches!(state, ConnectionState::Failed));
            assert_eq!(b.storage(), 23);
            assert!(account == b.work_ledger_identity_v1());
        }
    }
}

fn pair(kind: net::SocketType, flags: net::SocketFlags) -> (OwnedFd, OwnedFd) {
    net::socketpair(net::AddressFamily::UNIX, kind, flags, None).unwrap()
}

fn assert_closed(peer: &OwnedFd) {
    assert_eq!(io::read(peer, &mut [0u8; 1]).unwrap(), 0);
}

#[test]
fn preflight_resource_refusals_close_consumed_endpoint_and_preserve_history() {
    if !crate::test_isolation::enter(concat!(
        module_path!(),
        "::preflight_resource_refusals_close_consumed_endpoint_and_preserve_history"
    )) {
        return;
    }
    for (work, scratch, spent) in [
        (ENTRY - 1, CHECK_FRAME, 0),
        (CHECK_WORK - 1, CHECK_FRAME, ENTRY),
        (CHECK_WORK, CHECK_FRAME - 1, CHECK_WORK),
    ] {
        let (endpoint, peer) = pair(
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        );
        let floor = 23 + ENDPOINT_STORAGE;
        let mut work = Work::new(11 + work);
        let mut b = Budget::new(&mut work, floor + scratch);
        b.charge_work(11).unwrap();
        b.reserve_storage(floor).unwrap();
        assert!(b.charge_work(usize::MAX).is_err());
        assert!(b.reserve_storage(usize::MAX).is_err());
        let history = (b.failed_work(), b.failed_storage());
        let error = RootEndpoint::new(endpoint, &mut b).err().unwrap();
        assert!(error.resource().is_some());
        assert_eq!((b.storage(), b.work()), (floor, 11 + spent));
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        assert_closed(&peer);
    }
}

#[test]
fn input_floor_refusal_precedes_endpoint_inspection_and_closes_it() {
    if !crate::test_isolation::enter(concat!(
        module_path!(),
        "::input_floor_refusal_precedes_endpoint_inspection_and_closes_it"
    )) {
        return;
    }
    let (endpoint, peer) = pair(net::SocketType::STREAM, net::SocketFlags::CLOEXEC);
    let mut work = Work::new(CHECK_WORK);
    let mut b = Budget::new(&mut work, ENDPOINT_STORAGE + CHECK_FRAME);
    b.reserve_storage(ENDPOINT_STORAGE - 1).unwrap();
    let error = RootEndpoint::new(endpoint, &mut b).err().unwrap();
    assert_eq!(error.resource(), Some(Resource::Accounting));
    assert_eq!((b.storage(), b.work()), (ENDPOINT_STORAGE - 1, ENTRY));
    assert_closed(&peer);
}

#[test]
fn exact_preflight_quote_reaches_real_endpoint_shape_and_creator_checks() {
    if !crate::test_isolation::enter(concat!(
        module_path!(),
        "::exact_preflight_quote_reaches_real_endpoint_shape_and_creator_checks"
    )) {
        return;
    }
    for (kind, flags, passcred, expected) in [
        (
            net::SocketType::STREAM,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            true,
            "endpoint shape",
        ),
        (
            net::SocketType::SEQPACKET,
            net::SocketFlags::NONBLOCK,
            true,
            "endpoint shape",
        ),
        (
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC,
            true,
            "endpoint shape",
        ),
        (
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            false,
            "endpoint shape",
        ),
        (
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            true,
            "actual root parent socket creator",
        ),
    ] {
        let (endpoint, peer) = pair(kind, flags);
        net::sockopt::set_socket_passcred(&endpoint, passcred).unwrap();
        let mut work = Work::new(CHECK_WORK);
        let mut b = Budget::new(&mut work, ENDPOINT_STORAGE + CHECK_FRAME);
        b.reserve_storage(ENDPOINT_STORAGE).unwrap();
        // Even when run as root, a socket created by this process is not a
        // socket created by its actual parent. There is no successful fake owner.
        let error = RootEndpoint::new(endpoint, &mut b).err().unwrap();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(error.resource(), None);
        assert_eq!((b.storage(), b.work()), (ENDPOINT_STORAGE, CHECK_WORK));
        assert_eq!(b.peak_storage(), ENDPOINT_STORAGE + CHECK_FRAME);
        assert_closed(&peer);
    }
}

#[test]
fn handshake_has_one_finite_attempt_counter_and_original_deadline() {
    assert_eq!(TIMEOUT, Duration::from_secs(120));
    assert_eq!(MAX_ATTEMPTS, transport::MAX_PHASE_ATTEMPTS);
    assert_eq!(POLL_INTERVAL, Duration::from_millis(1));
    let deadline = Instant::now() + TIMEOUT;
    let mut attempts = 0;
    // Waiting for the root's actual image measurement must cross the old cap.
    for expected in 1..=129 {
        permit_attempt(deadline, &mut attempts).unwrap();
        assert_eq!(attempts, expected);
    }
    attempts = MAX_ATTEMPTS - 1;
    permit_attempt(deadline, &mut attempts).unwrap();
    assert_eq!(attempts, MAX_ATTEMPTS);
    assert!(permit_attempt(deadline, &mut attempts).is_err());
    assert_eq!(attempts, MAX_ATTEMPTS);
    for deadline in [
        Instant::now(),
        Instant::now() + TIMEOUT + Duration::from_secs(1),
    ] {
        let mut attempts = 17;
        assert!(permit_attempt(deadline, &mut attempts).is_err());
        assert_eq!(attempts, 17);
    }
}

#[test]
fn codec_resource_errors_keep_original_category() {
    assert_eq!(
        codec_error(CodecError::Resource(Resource::Accounting)).resource(),
        Some(Resource::Accounting)
    );
    assert_eq!(
        codec_error(CodecError::Framing("inert refusal")).resource(),
        None
    );
    assert!(PACKET_WORK >= transport::packet_receive_work(BYTES));
    assert!(PACKET_FRAME >= transport::packet_receive_scratch(BYTES));
    assert!(CHECK_FRAME >= RootEndpoint::STORAGE);
}

// Native role, not a synthetic admission fixture or the production FD ABI.
// An isolated root parent must exec this exact ignored test in a sealed-static
// test binary, with a root-created PASSCRED SEQPACKET on stdin. Before readiness
// it queues one 24-byte F2IG frame: client PID/UID/GID and anchor UID/GID (LE u32),
// plus six SCM_RIGHTS: journal directory, client peer, client pidfd, anchor peer,
// anchor pidfd, readiness writer. It retains the real distinct-UID peers alive.
// Policy generation is 1 with fixture seeds 0x31/0x71 and actual image measures.
// The parent queues a real Cancel before readiness, checks that no service reply
// arrives while the gate is closed, waits for readiness EOF, sends the canonical
// gate request, validates its reply, then observes Cancel completion. Missing
// native prerequisites fail this explicitly selected role, never skip-success.
#[test]
#[ignore = "isolated static native issuer plus real root-parent closed-gate driver required"]
fn native_closed_gate_service_role() {
    use crate::{
        ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
        ProtectedExternalAnchorServiceAdmissionV2 as Anchor, ProtectedIssuerProcessV1 as Process,
        ProtectedServiceAdmissionV2 as Service, current_static_issuer_measurements_v1,
    };
    use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Key;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionClientProcessIdentityV1 as ClientIdentity,
        CompilerExecutionExternalAnchorServiceIdentityV1 as AnchorIdentity,
        CompilerExecutionIssuerPolicyV3 as Policy,
    };
    use std::{io::IoSliceMut, mem::MaybeUninit};

    assert_eq!(
        std::env::var("FE2O3_RUN_NATIVE_ISSUER_ROOT_GATE").as_deref(),
        Ok("1")
    );
    assert!(
        std::path::Path::new("/.dockerenv").is_file(),
        "isolated native container required"
    );
    let measurements =
        current_static_issuer_measurements_v1().expect("real sealed-static image required");
    let input = std::io::stdin();
    let endpoint = io::fcntl_dupfd_cloexec(input.as_fd(), 3).unwrap();
    validate_endpoint(endpoint.as_fd(), process::getppid().unwrap()).unwrap();
    let mut frame = [0u8; 24];
    let mut ancillary_bytes =
        [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(6), ScmCredentials(1))];
    let mut ancillary = net::RecvAncillaryBuffer::new(&mut ancillary_bytes);
    let received = net::recvmsg(
        &endpoint,
        &mut [IoSliceMut::new(&mut frame)],
        &mut ancillary,
        net::RecvFlags::DONTWAIT | net::RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(received.bytes, frame.len());
    assert!(
        !received
            .flags
            .intersects(net::ReturnFlags::TRUNC | net::ReturnFlags::CTRUNC)
    );
    assert_eq!(&frame[..4], b"F2IG");
    let mut fds = Vec::new();
    let mut credentials = 0;
    for message in ancillary.drain() {
        match message {
            net::RecvAncillaryMessage::ScmRights(rights) => fds.extend(rights),
            net::RecvAncillaryMessage::ScmCredentials(c) => {
                assert_eq!(Some(c.pid), process::getppid());
                assert_eq!((c.uid.as_raw(), c.gid.as_raw()), (0, 0));
                credentials += 1;
            }
            _ => panic!("unexpected fixture ancillary message"),
        }
    }
    assert_eq!(credentials, 1);
    let [
        root,
        client_peer,
        client_pidfd,
        anchor_peer,
        anchor_pidfd,
        writer,
    ]: [OwnedFd; 6] = fds.try_into().unwrap();
    let fields: Vec<_> = frame[4..]
        .chunks_exact(4)
        .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
        .collect();
    let [pid, uid, gid, anchor_uid, anchor_gid]: [u32; 5] = fields.try_into().unwrap();
    let mut work = Work::new(1_000_000_000_000);
    let mut b = Budget::new(&mut work, 512 * 1024 * 1024);
    b.reserve_storage(
        Admission::PROCESS_STORAGE
            + Service::FD_PAIR_STORAGE
            + Client::FD_STORAGE
            + Anchor::PAIR_STORAGE
            + Admission::READINESS_WRITER_STORAGE
            + Admission::ROOT_CONTROL_ENDPOINT_STORAGE,
    )
    .unwrap();
    let process = Process::harden().unwrap();
    let (client, growth) =
        Client::admit(client_pidfd, Expected::new(pid, uid, gid).unwrap(), &mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let (service, growth) = Service::admit(root, client_peer, client, &mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let anchor_identity = AnchorIdentity::new(anchor_uid, anchor_gid).unwrap();
    let (anchor, growth) =
        Anchor::admit(anchor_peer, anchor_pidfd, anchor_identity, &mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let key = |seed| {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    };
    let (policy, charge) = Policy::new(
        1,
        measurements.executable(),
        measurements.runtime(),
        key(0x31),
        key(0x71),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let mut seed = [0x31; 32];
    b.reserve_storage(seed.len()).unwrap();
    let (key, charge) = Key::create_and_zeroize(&mut seed, &policy, &mut b).unwrap();
    b.release_storage(seed.len()).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(seed, [0; 32]);
    let (admission, growth) =
        Admission::admit(process, service, policy, key, anchor, &mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let (manifest, charge) = Manifest::new(
        ClientIdentity::new(pid, uid, gid).unwrap(),
        anchor_identity,
        admission.policy(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let floor = b.storage();
    admission
        .serve_native_with_root_readiness(&manifest, writer, endpoint, &mut b)
        .unwrap();
    assert_eq!(b.storage(), floor);
    println!("FE2O3_NATIVE_ISSUER_ROOT_GATE_CANCEL_OK");
}
