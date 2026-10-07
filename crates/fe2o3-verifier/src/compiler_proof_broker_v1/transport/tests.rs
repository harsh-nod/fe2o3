use super::*;

fn hello() -> Frame {
    Frame {
        kind: HELLO,
        session: [1; 32],
        sequence: 1,
        challenge: [2; 32],
        body: vec![3; 64],
    }
}

#[test]
fn canonical_frames_reject_reserved_fields_truncation_and_replay() {
    let frame = hello();
    let bytes = frame.encode().unwrap();
    assert_eq!(Frame::decode(&bytes).unwrap(), frame);
    for end in 0..bytes.len() {
        assert!(Frame::decode(&bytes[..end]).is_err());
    }
    for offset in [0, 8, 9, 15, 88, 92, 95] {
        let mut invalid = bytes.clone();
        invalid[offset] = 255;
        assert!(Frame::decode(&invalid).is_err(), "offset {offset}");
    }
    let mut reply = frame.reply(HELLO_ACK, frame.body.clone());
    reply.require_reply(&frame, HELLO_ACK).unwrap();
    reply.sequence += 1;
    assert!(reply.require_reply(&frame, HELLO_ACK).is_err());
    reply.sequence -= 1;
    reply.challenge[0] ^= 1;
    assert!(reply.require_reply(&frame, HELLO_ACK).is_err());
    reply.challenge = frame.challenge;
    reply.session[0] ^= 1;
    assert!(reply.require_reply(&frame, HELLO_ACK).is_err());
}

#[test]
fn authenticated_packets_preserve_credentials_and_rights_roster() {
    let (left, right) = pair().unwrap();
    let left = Endpoint::admit(left).unwrap();
    let right = Endpoint::admit(right).unwrap();
    let deadline = Instant::now() + FRAME_TIMEOUT;
    let frame = hello();
    left.send(&frame, &[], deadline, || Ok(())).unwrap();
    assert_eq!(
        right
            .receive(current_credentials(), deadline, || Ok(()))
            .unwrap()
            .0,
        frame
    );
    let source = seal_source(b"fn main() {}\n").unwrap();
    let frame = Frame {
        kind: EXECUTE,
        ..hello()
    };
    assert!(left.send(&frame, &[], deadline, || Ok(())).is_err());
    left.send(&frame, &[source.as_fd()], deadline, || Ok(()))
        .unwrap();
    let (received, mut rights) = right
        .receive(current_credentials(), deadline, || Ok(()))
        .unwrap();
    assert_eq!(received, frame);
    assert_eq!(rights.len(), 1);
    assert_eq!(
        read_source(File::from(rights.pop().unwrap()), current_credentials(), 13)
            .unwrap()
            .source(),
        b"fn main() {}\n"
    );
}

#[test]
fn endpoint_rejects_changed_flags_wrong_sender_and_closed_peer() {
    let (left, right) = pair().unwrap();
    let left = Endpoint::admit(left).unwrap();
    let right = Endpoint::admit(right).unwrap();
    let deadline = Instant::now() + FRAME_TIMEOUT;
    left.send(&hello(), &[], deadline, || Ok(())).unwrap();
    let mut foreign = current_credentials();
    foreign.0 += 1;
    assert!(right.receive(foreign, deadline, || Ok(())).is_err());
    net::sockopt::set_socket_passcred(&right.fd, false).unwrap();
    assert!(right.revalidate().is_err());
    net::sockopt::set_socket_passcred(&right.fd, true).unwrap();
    right.revalidate().unwrap();
    rustix::fs::fcntl_setfl(&right.fd, OFlags::RDWR).unwrap();
    assert!(right.revalidate().is_err());
    drop(right);
    assert!(left.revalidate().is_err());
}

#[test]
fn source_requires_exact_immutable_readonly_object() {
    let bytes = b"fn main() {}\n";
    let source = seal_source(bytes).unwrap();
    assert_eq!(
        read_source(
            source.try_clone().unwrap(),
            current_credentials(),
            bytes.len() as u64
        )
        .unwrap()
        .source(),
        bytes
    );
    assert!(
        read_source(
            source.try_clone().unwrap(),
            current_credentials(),
            bytes.len() as u64 + 1
        )
        .is_err()
    );
    let mut foreign = current_credentials();
    foreign.1 += 1;
    assert!(read_source(source.try_clone().unwrap(), foreign, bytes.len() as u64).is_err());
    source
        .set_permissions(std::fs::Permissions::from_mode(0o600))
        .unwrap();
    let writable = File::from(
        rustix::fs::open(
            format!("/proc/self/fd/{}", source.as_raw_fd()),
            OFlags::RDWR | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    );
    source
        .set_permissions(std::fs::Permissions::from_mode(0o400))
        .unwrap();
    assert!(read_source(writable, current_credentials(), bytes.len() as u64).is_err());
    source
        .set_permissions(std::fs::Permissions::from_mode(0o600))
        .unwrap();
    assert!(read_source(source, current_credentials(), bytes.len() as u64).is_err());
    assert!(seal_source(&[]).is_err());
}

#[test]
fn remote_deadline_never_extends_budget() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let encoded = encode_deadline(deadline).unwrap();
    assert!(decode_deadline(encoded).unwrap() <= deadline);
    assert!(encode_deadline(Instant::now()).is_err());
    assert!(encode_deadline(Instant::now() + MAX_PROOF_TIMEOUT + Duration::from_secs(1)).is_err());
    assert!(decode_deadline(0).is_err());
    assert!(decode_deadline(u64::MAX).is_err());
}

fn raw_send(endpoint: &Endpoint, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        net::sendmsg(
            &endpoint.fd,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL
        )
        .unwrap(),
        bytes.len()
    );
}

#[test]
fn incoming_missing_extra_and_truncated_rights_are_rejected() {
    for count in [0, 2, 3] {
        let (left, right) = pair().unwrap();
        let left = Endpoint::admit(left).unwrap();
        let right = Endpoint::admit(right).unwrap();
        let source = seal_source(b"fn main() {}\n").unwrap();
        let frame = Frame {
            kind: EXECUTE,
            ..hello()
        };
        raw_send(
            &left,
            &frame.encode().unwrap(),
            &vec![source.as_fd(); count],
        );
        assert!(
            right
                .receive(
                    current_credentials(),
                    Instant::now() + FRAME_TIMEOUT,
                    || Ok(())
                )
                .is_err()
        );
    }
}

#[test]
fn oversized_packets_and_queued_data_after_eof_are_rejected() {
    let (left, right) = pair().unwrap();
    let left = Endpoint::admit(left).unwrap();
    let right = Endpoint::admit(right).unwrap();
    let mut bytes = hello().encode().unwrap();
    bytes.resize(HEADER + MAX_BODY + 1, 0);
    raw_send(&left, &bytes, &[]);
    assert!(
        right
            .receive(
                current_credentials(),
                Instant::now() + FRAME_TIMEOUT,
                || Ok(())
            )
            .is_err()
    );
    raw_send(&left, &hello().encode().unwrap(), &[]);
    drop(left);
    assert!(
        right
            .receive(
                current_credentials(),
                Instant::now() + FRAME_TIMEOUT,
                || Ok(())
            )
            .is_err()
    );
}

#[test]
fn missing_reply_obeys_absolute_deadline() {
    let (_left, right) = pair().unwrap();
    let right = Endpoint::admit(right).unwrap();
    let start = Instant::now();
    let result = right.receive(
        current_credentials(),
        start + Duration::from_millis(30),
        || Ok(()),
    );
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(1));
}
