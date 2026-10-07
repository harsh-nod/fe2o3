use super::*;
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationProofInputsV1 as Inputs, WorkerV3ApplicationProofKindV1 as Kind,
};
use std::os::fd::AsFd;

fn sender() -> (i32, u32, u32) {
    (
        std::process::id() as i32,
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    )
}

fn request() -> Message {
    let inputs = Inputs::new([1; 32], ([2; 32], 3), ([4; 32], 5)).unwrap();
    Message::new(Kind::Request, [7; 32], 1, inputs.canonical_bytes()).unwrap()
}

fn raw_send(fd: BorrowedFd<'_>, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut ancillary = net::SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        net::sendmsg(
            fd,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
        )
        .unwrap(),
        bytes.len()
    );
}

#[test]
fn exact_request_preserves_two_original_objects_and_sets_cloexec() {
    let (a, b) = wire::control_pair().unwrap();
    let envelope = wire::seal(b"envelope").unwrap();
    let payload = wire::seal(b"payload").unwrap();
    let message = request();
    assert!(try_send(a.as_fd(), &message, &[envelope.as_fd(), payload.as_fd()]).unwrap());
    let (received, rights) = try_receive(b.as_fd(), sender(), [7; 32]).unwrap().unwrap();
    assert_eq!(received, message);
    for (original, received) in [envelope.as_fd(), payload.as_fd()].into_iter().zip(&rights) {
        let original = rustix::fs::fstat(original).unwrap();
        let received_stat = rustix::fs::fstat(received).unwrap();
        assert_eq!(
            (original.st_dev, original.st_ino),
            (received_stat.st_dev, received_stat.st_ino)
        );
        assert_eq!(
            rustix::io::fcntl_getfd(received).unwrap(),
            rustix::io::FdFlags::CLOEXEC
        );
    }
    assert_eq!(rights.len(), 2);
    assert!(try_receive(b.as_fd(), sender(), [7; 32]).unwrap().is_none());
}

#[test]
fn wrong_sender_session_missing_credentials_and_malformed_frames_reject() {
    let (a, b) = wire::control_pair().unwrap();
    let message = Message::new(Kind::Probe, [7; 32], 2, &[]).unwrap();
    let (pid, uid, gid) = sender();
    for expected in [
        (pid ^ 1, uid, gid),
        (pid, uid ^ 1, gid),
        (pid, uid, gid ^ 1),
    ] {
        assert!(try_send(a.as_fd(), &message, &[]).unwrap());
        assert!(try_receive(b.as_fd(), expected, [7; 32]).is_err());
    }
    assert!(try_send(a.as_fd(), &message, &[]).unwrap());
    assert!(try_receive(b.as_fd(), sender(), [8; 32]).is_err());
    net::sockopt::set_socket_passcred(&b, false).unwrap();
    assert!(try_send(a.as_fd(), &message, &[]).unwrap());
    assert!(try_receive(b.as_fd(), sender(), [7; 32]).is_err());
    net::sockopt::set_socket_passcred(&b, true).unwrap();
    for bytes in [
        vec![],
        vec![0; 63],
        vec![0; WORKER_V3_APPLICATION_PROOF_MAX_PACKET_BYTES_V1 + 1],
        {
            let mut bytes = message.canonical_bytes().to_vec();
            bytes[56] = 1;
            bytes
        },
    ] {
        raw_send(a.as_fd(), &bytes, &[]);
        assert!(try_receive(b.as_fd(), sender(), [7; 32]).is_err());
    }
}

#[test]
fn rejected_rights_are_closed_even_when_ancillary_is_truncated() {
    for count in [0, 1, 3, 4] {
        let (a, b) = wire::control_pair().unwrap();
        // Peer EOF proves no rejected SCM_RIGHTS alias survives, without process-wide FD counts.
        let (sentinel, watch) = wire::control_pair().unwrap();
        raw_send(
            a.as_fd(),
            request().canonical_bytes(),
            &vec![sentinel.as_fd(); count],
        );
        drop(sentinel);
        assert!(try_receive(b.as_fd(), sender(), [7; 32]).is_err());
        assert_eq!(
            net::recv(&watch, &mut [0], net::RecvFlags::DONTWAIT)
                .unwrap()
                .0,
            0
        );
    }
    let (a, b) = wire::control_pair().unwrap();
    let (sentinel, watch) = wire::control_pair().unwrap();
    let probe = Message::new(Kind::Probe, [7; 32], 2, &[]).unwrap();
    raw_send(a.as_fd(), probe.canonical_bytes(), &[sentinel.as_fd()]);
    drop(sentinel);
    assert!(try_receive(b.as_fd(), sender(), [7; 32]).is_err());
    assert_eq!(
        net::recv(&watch, &mut [0], net::RecvFlags::DONTWAIT)
            .unwrap()
            .0,
        0
    );
}

#[test]
fn outgoing_roster_errors_do_not_send_and_backpressure_preserves_frames() {
    let (a, b) = wire::control_pair().unwrap();
    assert!(try_send(a.as_fd(), &request(), &[]).is_err());
    assert!(try_receive(b.as_fd(), sender(), [7; 32]).unwrap().is_none());
    net::sockopt::set_socket_send_buffer_size(&a, 4096).unwrap();
    let mut sequence = 2;
    loop {
        let message = Message::new(Kind::Probe, [7; 32], sequence, &[]).unwrap();
        if !try_send(a.as_fd(), &message, &[]).unwrap() {
            break;
        }
        sequence += 1;
        assert!(sequence < 4096, "bounded socket never applied backpressure");
    }
    assert!(sequence > 2);
    for expected in 2..sequence {
        let (message, rights) = try_receive(b.as_fd(), sender(), [7; 32]).unwrap().unwrap();
        assert_eq!(message.sequence(), expected);
        assert!(rights.is_empty());
    }
    let retry = Message::new(Kind::Probe, [7; 32], sequence, &[]).unwrap();
    assert!(try_send(a.as_fd(), &retry, &[]).unwrap());
    assert_eq!(
        try_receive(b.as_fd(), sender(), [7; 32])
            .unwrap()
            .unwrap()
            .0,
        retry
    );
    assert!(try_receive(b.as_fd(), sender(), [7; 32]).unwrap().is_none());
    drop(a);
    assert!(try_receive(b.as_fd(), sender(), [7; 32]).is_err());
}
