use super::*;
use std::thread;

const BINDING: SessionBindingV1 = SessionBindingV1 {
    session: [17; 32],
    runtime: [23; 32],
};

fn pair() -> (ExecutorChannelV1, ExecutorChannelV1) {
    let (left, right) = sockets();
    let deadline = absolute_deadline(Instant::now() + Duration::from_secs(5)).unwrap();
    (
        ExecutorChannelV1::new(left, BINDING, deadline).unwrap(),
        ExecutorChannelV1::new(right, BINDING, deadline).unwrap(),
    )
}

fn sockets() -> (Socket, Socket) {
    use rustix::net::{self, AddressFamily, SocketFlags, SocketType};
    let (left, right) = net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    net::sockopt::set_socket_passcred(&left, true).unwrap();
    net::sockopt::set_socket_passcred(&right, true).unwrap();
    // These fixtures send from threads in the creating process. This is not a
    // bootstrap rule for children of that process; the socket tests cover that.
    let left_sender = net::sockopt::socket_peercred(&left).unwrap();
    let right_sender = net::sockopt::socket_peercred(&right).unwrap();
    (
        Socket::new(left, left_sender).unwrap(),
        Socket::new(right, right_sender).unwrap(),
    )
}

fn source() -> Source {
    Source::new(b"verus! { proof fn sample() { assert(true); } }\n".to_vec()).unwrap()
}

fn output(code: i32) -> RetainedFunctionalRefinementRuntimeOutputV1 {
    RetainedFunctionalRefinementRuntimeOutputV1 {
        policy: super::super::GeneratedProofProcessPolicyV2::LegacySingleSolverV1,
        exit_code: Some(code),
        signal: None,
        stdout: b"output\n".to_vec(),
        stderr: b"diagnostic\n".to_vec(),
    }
}

fn assert_expired(error: ChannelErrorV1) {
    match error {
        ChannelErrorV1::Deadline => {}
        ChannelErrorV1::Io(error) => assert_eq!(error.kind(), io::ErrorKind::TimedOut),
        error => panic!("expected deadline expiry, got {error:?}"),
    }
}

#[test]
fn legacy_executor_channel_refuses_context_policy_owner_and_output_substitution() {
    assert!(require_legacy_policy(GeneratedProofProcessPolicyV2::LegacySingleSolverV1).is_ok());
    for policy in [
        GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV2,
        GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV3,
    ] {
        assert!(matches!(
            require_legacy_policy(policy),
            Err(ChannelErrorV1::Association)
        ));
        let (mut server, _) = pair();
        server.begin().unwrap();
        let header = request_header(&server);
        let mut substituted = output(0);
        substituted.policy = policy;
        assert!(matches!(
            server.send_output(header, substituted),
            Err(ChannelErrorV1::Association)
        ));
        assert_eq!(server.output_bytes, 0);
        assert!(server.terminal);
        assert!(matches!(server.begin(), Err(ChannelErrorV1::Terminal)));
    }
}

fn request_header(channel: &ExecutorChannelV1) -> [u8; HEADER_BYTES] {
    let source = source();
    let mut header = channel.header(REQUEST, channel.deadline);
    header[96..128].copy_from_slice(&source.identity().as_bytes());
    put32(&mut header, 136, source.byte_len() as u32);
    put32(&mut header, 140, 64);
    seal(&mut header, source.source());
    header
}

#[test]
fn multiple_requests_keep_exact_association_and_return_only_inert_bytes() {
    let (mut client, mut server) = pair();
    let worker = thread::spawn(move || {
        for expected in 1..=3 {
            server.begin().unwrap();
            let request = server
                .receive(server.deadline, ExpectedFrame::RequestOrFinish)
                .unwrap();
            assert_eq!(field64(&request.header, 24), expected);
            assert_eq!(request.body, source().source());
            server.charge_source(request.body.len() as u64).unwrap();
            server
                .send_output(request.header, output(expected as i32 - 1))
                .unwrap();
            server.terminal = false;
        }
        server.begin().unwrap();
        let request = server
            .receive(server.deadline, ExpectedFrame::RequestOrFinish)
            .unwrap();
        assert_eq!(request.kind(), FINISH);
        let mut reply = server.header(FINISHED, server.deadline);
        seal(&mut reply, &[]);
        write_all(&mut server.stream, &reply, server.deadline).unwrap();
    });
    for code in 0..3 {
        let reply = client
            .exchange(&source(), Instant::now() + Duration::from_secs(3), 64)
            .unwrap();
        assert_eq!((reply.exit_code, reply.signal), (Some(code), None));
        assert_eq!(reply.stdout, b"output\n");
        assert_eq!(reply.stderr, b"diagnostic\n");
    }
    assert_eq!(client.sequence, 3);
    assert_eq!(client.source_bytes, source().byte_len() * 3);
    assert_eq!(client.output_bytes, 18 * 3);
    client.finish().unwrap();
    assert!(matches!(client.finish(), Err(ChannelErrorV1::Terminal)));
    worker.join().unwrap();
}

#[test]
fn header_rejects_each_wrong_identity_and_unknown_field_before_body_read() {
    for offset in [0, 20, 24, 32, 64] {
        let (mut client, mut server) = pair();
        client.begin().unwrap();
        server.begin().unwrap();
        let mut header = request_header(&client);
        header[offset] ^= 1;
        seal(&mut header, source().source());
        write_all(&mut client.stream, &header, client.deadline).unwrap();
        assert!(
            server
                .receive(server.deadline, ExpectedFrame::RequestOrFinish)
                .is_err(),
            "offset {offset}"
        );
        assert!(server.terminal);
        assert!(matches!(server.begin(), Err(ChannelErrorV1::Terminal)));
    }
    for kind in [0, 5, u32::MAX] {
        let (mut client, mut server) = pair();
        client.begin().unwrap();
        server.begin().unwrap();
        let mut header = request_header(&client);
        put32(&mut header, 16, kind);
        write_all(&mut client.stream, &header, client.deadline).unwrap();
        assert!(matches!(
            server.receive(server.deadline, ExpectedFrame::RequestOrFinish),
            Err(ChannelErrorV1::Framing)
        ));
    }
}

#[test]
fn replays_wrong_source_and_changed_request_bounds_poison_the_client() {
    for offset in [24, 32, 64, 96, 128, 136, 140] {
        let (mut client, mut server) = pair();
        let worker = thread::spawn(move || {
            server.begin().unwrap();
            let request = server
                .receive(server.deadline, ExpectedFrame::RequestOrFinish)
                .unwrap();
            let mut reply = request.header;
            put32(&mut reply, 16, REPLY);
            put32(&mut reply, 152, 1);
            reply[offset] ^= 1;
            seal(&mut reply, &[]);
            write_all(&mut server.stream, &reply, server.deadline).unwrap();
        });
        assert!(
            client
                .exchange(&source(), Instant::now() + Duration::from_secs(3), 64)
                .is_err()
        );
        assert!(matches!(
            client.exchange(&source(), Instant::now(), 64),
            Err(ChannelErrorV1::Terminal)
        ));
        worker.join().unwrap();
    }
}

#[test]
fn truncated_corrupt_and_overlong_bodies_are_not_successful_replies() {
    for mode in 0..4 {
        let (mut client, mut server) = pair();
        let worker = thread::spawn(move || {
            server.begin().unwrap();
            let request = server
                .receive(server.deadline, ExpectedFrame::RequestOrFinish)
                .unwrap();
            let mut header = request.header;
            put32(&mut header, 16, REPLY);
            put32(&mut header, 152, 1);
            put32(&mut header, 144, if mode == 2 { 65 } else { 4 });
            seal(&mut header, b"body");
            if mode == 3 {
                header[160] ^= 1;
            }
            write_all(&mut server.stream, &header, server.deadline).unwrap();
            if mode != 2 {
                write_all(
                    &mut server.stream,
                    if mode == 0 { b"bod" } else { b"bOdy" },
                    server.deadline,
                )
                .unwrap();
            }
        });
        assert!(
            client
                .exchange(&source(), Instant::now() + Duration::from_secs(3), 64)
                .is_err()
        );
        assert!(client.terminal);
        worker.join().unwrap();
    }
}

#[test]
fn requests_have_closed_shape_and_bounds() {
    let (mut channel, _) = pair();
    channel.begin().unwrap();
    let header = request_header(&channel);
    assert_eq!(body_length(&header).unwrap(), source().byte_len() as usize);
    for (offset, value) in [
        (136, 0),
        (136, MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 as u32 + 1),
        (140, 0),
        (140, MAX_OUTPUT as u32 + 1),
        (144, 1),
        (148, 1),
        (152, 1),
        (156, 1),
    ] {
        let mut changed = header;
        put32(&mut changed, offset, value);
        assert!(body_length(&changed).is_err(), "{offset}={value}");
    }
    let mut changed = header;
    changed[96..128].fill(0);
    assert!(body_length(&changed).is_err());
    for kind in [FINISH, FINISHED] {
        let mut header = channel.header(kind, channel.deadline);
        assert_eq!(body_length(&header).unwrap(), 0);
        for offset in 96..128 {
            header[offset] = 1;
            assert!(body_length(&header).is_err());
            header[offset] = 0;
        }
        for offset in 136..160 {
            header[offset] = 1;
            assert!(body_length(&header).is_err());
            header[offset] = 0;
        }
    }
}

#[test]
fn terminal_status_is_exact_and_invalid_backend_statuses_do_not_panic() {
    let (mut channel, _) = pair();
    channel.begin().unwrap();
    let mut header = request_header(&channel);
    put32(&mut header, 16, REPLY);
    for code in 1..=256 {
        put32(&mut header, 152, code);
        assert_eq!(termination(&header).unwrap(), (Some(code as i32 - 1), None));
    }
    put32(&mut header, 152, 0);
    for signal in 1..=64 {
        put32(&mut header, 156, signal);
        assert_eq!(termination(&header).unwrap(), (None, Some(signal as i32)));
    }
    for (code, signal) in [(0, 0), (1, 1), (257, 0), (0, 65), (u32::MAX, 0)] {
        put32(&mut header, 152, code);
        put32(&mut header, 156, signal);
        assert!(termination(&header).is_err());
    }
    for (code, signal) in [
        (Some(-1), None),
        (Some(256), None),
        (None, None),
        (Some(0), Some(1)),
        (None, Some(-1)),
    ] {
        let mut invalid = output(0);
        invalid.exit_code = code;
        invalid.signal = signal;
        assert!(matches!(
            channel.send_output(request_header(&channel), invalid),
            Err(ChannelErrorV1::Framing)
        ));
    }
}

#[test]
fn cumulative_limits_are_checked_before_receiving_body_and_close_has_its_own_slot() {
    for source_exhausted in [false, true] {
        let (mut client, mut server) = pair();
        client.begin().unwrap();
        server.begin().unwrap();
        if source_exhausted {
            server.source_bytes = MAX_TOTAL_SOURCE;
        } else {
            server.sequence = MAX_REQUESTS + 1;
            client.sequence = server.sequence;
        }
        let header = request_header(&client);
        write_all(&mut client.stream, &header, client.deadline).unwrap();
        assert!(matches!(
            server.receive(server.deadline, ExpectedFrame::RequestOrFinish),
            Err(ChannelErrorV1::Limit)
        ));
    }
    let (mut client, mut server) = pair();
    client.begin().unwrap();
    server.begin().unwrap();
    server.output_bytes = MAX_TOTAL_OUTPUT;
    let request = request_header(&client);
    let mut header = request;
    put32(&mut header, 16, REPLY);
    put32(&mut header, 144, 1);
    put32(&mut header, 152, 1);
    write_all(&mut client.stream, &header, client.deadline).unwrap();
    assert!(matches!(
        server.receive(server.deadline, ExpectedFrame::ReplyTo(&request)),
        Err(ChannelErrorV1::Limit)
    ));

    let (mut channel, _) = pair();
    channel.sequence = MAX_REQUESTS;
    channel.begin().unwrap();
    assert!(matches!(
        channel.charge_source(1),
        Err(ChannelErrorV1::Limit)
    ));
    assert_eq!(
        body_length(&channel.header(FINISH, channel.deadline)).unwrap(),
        0
    );
    channel.terminal = false;
    assert!(matches!(channel.begin(), Err(ChannelErrorV1::Limit)));
}

#[test]
fn byte_counters_allow_exact_limits_and_reject_one_more() {
    let (mut channel, _) = pair();
    channel.begin().unwrap();
    channel.charge_source(MAX_TOTAL_SOURCE).unwrap();
    assert!(matches!(
        channel.charge_source(1),
        Err(ChannelErrorV1::Limit)
    ));
    channel.charge_output(MAX_TOTAL_OUTPUT).unwrap();
    assert!(matches!(
        channel.charge_output(1),
        Err(ChannelErrorV1::Limit)
    ));
}

#[test]
fn dead_peer_and_unanswered_request_are_terminal() {
    let (mut client, server) = pair();
    drop(server);
    assert!(
        client
            .exchange(&source(), Instant::now() + Duration::from_secs(1), 64)
            .is_err()
    );
    assert!(client.terminal);

    let (mut client, _server) = pair();
    let started = Instant::now();
    assert!(
        client
            .exchange(&source(), started + Duration::from_millis(20), 64)
            .is_err()
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(client.terminal);
}

#[test]
fn absolute_deadlines_do_not_restart_on_receive_or_conversion() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let absolute = absolute_deadline(deadline).unwrap();
    assert!(local_deadline(absolute).unwrap() <= deadline);
    assert!(matches!(remaining(0), Err(ChannelErrorV1::Deadline)));
    assert!(matches!(
        absolute_deadline(Instant::now() - Duration::from_secs(1)),
        Err(ChannelErrorV1::Deadline)
    ));

    let (mut client, mut server) = pair();
    client.begin().unwrap();
    server.begin().unwrap();
    let mut header = request_header(&client);
    header[128..136].copy_from_slice(&0u64.to_le_bytes());
    write_all(&mut client.stream, &header, client.deadline).unwrap();
    assert!(matches!(
        server.receive(server.deadline, ExpectedFrame::RequestOrFinish),
        Err(ChannelErrorV1::Deadline)
    ));
}

#[test]
fn empty_identity_and_unbounded_sessions_are_rejected() {
    for binding in [
        SessionBindingV1 {
            session: [0; 32],
            ..BINDING
        },
        SessionBindingV1 {
            runtime: [0; 32],
            ..BINDING
        },
    ] {
        let (stream, _) = sockets();
        assert!(matches!(
            ExecutorChannelV1::new(
                stream,
                binding,
                absolute_deadline(Instant::now() + Duration::from_secs(1)).unwrap()
            ),
            Err(ChannelErrorV1::Limit)
        ));
    }
    let (stream, _) = sockets();
    assert!(matches!(
        ExecutorChannelV1::new(
            stream,
            BINDING,
            absolute_deadline(Instant::now() + MAX_SESSION + Duration::from_secs(1)).unwrap()
        ),
        Err(ChannelErrorV1::Limit)
    ));
}

#[test]
fn close_acknowledgement_does_not_allow_trailing_frames() {
    let (mut client, mut server) = pair();
    let worker = thread::spawn(move || {
        server.begin().unwrap();
        let request = server
            .receive(server.deadline, ExpectedFrame::RequestOrFinish)
            .unwrap();
        assert_eq!(request.kind(), FINISH);
        let mut header = server.header(FINISHED, server.deadline);
        seal(&mut header, &[]);
        write_all(&mut server.stream, &header, server.deadline).unwrap();
        write_all(&mut server.stream, b"x", server.deadline).unwrap();
    });
    assert!(matches!(client.finish(), Err(ChannelErrorV1::Framing)));
    worker.join().unwrap();
}

#[test]
fn full_size_source_and_packet_tails_use_one_canonical_chunk_schedule() {
    for bytes in [
        PACKET_BYTES - 1,
        PACKET_BYTES,
        PACKET_BYTES + 1,
        MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3,
    ] {
        let mut input = vec![b'a'; bytes];
        input[bytes - 1] = b'\n';
        let source = Source::new(input).unwrap();
        let identity = source.identity();
        let (mut client, mut server) = pair();
        let worker = thread::spawn(move || {
            server.begin().unwrap();
            let request = server
                .receive(server.deadline, ExpectedFrame::RequestOrFinish)
                .unwrap();
            assert_eq!(request.body.len(), bytes);
            assert_eq!(Source::new(request.body).unwrap().identity(), identity);
            server.send_output(request.header, output(0)).unwrap();
        });
        assert_eq!(
            client
                .exchange(&source, Instant::now() + Duration::from_secs(3), 64)
                .unwrap()
                .exit_code,
            Some(0)
        );
        worker.join().unwrap();
    }
}

#[test]
fn response_association_is_checked_before_any_body_read() {
    for offset in [16, 96, 128, 136, 140] {
        let (mut client, mut server) = pair();
        client.begin().unwrap();
        let request = request_header(&client);
        let mut reply = request;
        put32(&mut reply, 16, REPLY);
        put32(&mut reply, 144, 64);
        put32(&mut reply, 152, 1);
        reply[offset] ^= 1;
        write_all(&mut server.stream, &reply, server.deadline).unwrap();
        // Keep the peer open without supplying the advertised body. A body read
        // would time out instead of returning this header-association refusal.
        assert!(matches!(
            client.receive(client.deadline, ExpectedFrame::ReplyTo(&request)),
            Err(ChannelErrorV1::Association)
        ));
    }
}

#[test]
fn request_duration_is_checked_before_body_allocation_or_read() {
    let (mut client, mut server) = pair();
    let deadline =
        absolute_deadline(Instant::now() + MAX_REQUEST + Duration::from_secs(10)).unwrap();
    client.deadline = deadline;
    server.deadline = deadline;
    client.begin().unwrap();
    server.begin().unwrap();
    let header = request_header(&client);
    write_all(&mut client.stream, &header, deadline).unwrap();
    assert!(matches!(
        server.receive(deadline, ExpectedFrame::RequestOrFinish),
        Err(ChannelErrorV1::Limit)
    ));
}

#[test]
fn partial_body_and_acknowledgement_without_close_expire() {
    let (mut client, mut server) = pair();
    client.begin().unwrap();
    server.begin().unwrap();
    let deadline = absolute_deadline(Instant::now() + Duration::from_millis(50)).unwrap();
    let mut header = request_header(&client);
    header[128..136].copy_from_slice(&deadline.to_le_bytes());
    put32(&mut header, 136, (PACKET_BYTES + 1) as u32);
    write_all(&mut client.stream, &header, deadline).unwrap();
    write_all(&mut client.stream, &vec![b'a'; PACKET_BYTES], deadline).unwrap();
    assert_expired(
        server
            .receive(server.deadline, ExpectedFrame::RequestOrFinish)
            .err()
            .unwrap(),
    );
    assert!(server.terminal);

    let (mut client, mut server) = pair();
    let deadline = absolute_deadline(Instant::now() + Duration::from_millis(50)).unwrap();
    client.deadline = deadline;
    server.deadline = deadline;
    server.begin().unwrap();
    let mut header = server.header(FINISHED, deadline);
    seal(&mut header, &[]);
    write_all(&mut server.stream, &header, deadline).unwrap();
    assert_expired(client.finish().unwrap_err());
    assert!(client.terminal);
}

#[test]
fn finish_rejects_wrong_operation_or_echo_before_body_read() {
    for offset in [16, 96, 128, 136, 140] {
        let (mut client, mut server) = pair();
        server.begin().unwrap();
        let mut header = server.header(FINISHED, server.deadline);
        header[offset] ^= 1;
        write_all(&mut server.stream, &header, server.deadline).unwrap();
        assert!(matches!(client.finish(), Err(ChannelErrorV1::Association)));
        assert!(client.terminal);
    }
}
