//! Transport/descriptor checks only; these fixtures grant no application authority.
use super::*;
use crate::{authority_v2_test_process::pair as raw_pair, handoff_v2::tests::references};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, sendmsg};
use std::{
    io::IoSlice,
    mem::MaybeUninit,
    os::fd::{AsFd, BorrowedFd},
};

fn pair() -> (OwnedFd, OwnedFd) {
    let value = raw_pair();
    for fd in [&value.0, &value.1] {
        rustix::net::sockopt::set_socket_passcred(fd, true).unwrap();
    }
    value
}
fn sender_identity() -> Client {
    Client::new(
        std::process::id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap()
}
fn receive(
    fd: &OwnedFd,
    deadline: Instant,
) -> std::result::Result<([u8; BYTES], [OwnedFd; 4]), HandoffError> {
    transport::receive(fd, sender_identity(), deadline)
}

fn send(control: &OwnedFd, payload: &[u8], rights: &[BorrowedFd<'_>]) {
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !rights.is_empty() {
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(rights)));
    }
    assert_eq!(
        sendmsg(
            control,
            &[IoSlice::new(payload)],
            &mut ancillary,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT
        )
        .unwrap(),
        payload.len()
    );
}

#[test]
fn native_application_receiver_requires_exact_four_rights_and_closes_all_refusals() {
    for count in 0..=8 {
        let (sender, receiver) = pair();
        let rights: Vec<_> = (0..count).map(|_| pair().0).collect();
        let objects: Vec<_> = rights
            .iter()
            .map(|fd| checks::snapshot(fd).unwrap())
            .collect();
        send(
            &sender,
            &[0; BYTES],
            &rights.iter().map(AsFd::as_fd).collect::<Vec<_>>(),
        );
        let result = receive(&receiver, Instant::now() + Duration::from_secs(2));
        assert_eq!(result.is_ok(), count == 4, "rights={count}");
        drop(result);
        for object in objects {
            assert_eq!(references(object), 1, "rights={count}");
        }
    }
}

#[test]
fn native_application_receiver_rejects_short_long_and_compiler_only_shapes() {
    for length in [
        0,
        1,
        BYTES - 1,
        BYTES + 1,
        fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V3,
    ] {
        let (sender, receiver) = pair();
        let rights: Vec<_> = (0..4).map(|_| pair().0).collect();
        let objects: Vec<_> = rights
            .iter()
            .map(|fd| checks::snapshot(fd).unwrap())
            .collect();
        send(
            &sender,
            &vec![0; length],
            &rights.iter().map(AsFd::as_fd).collect::<Vec<_>>(),
        );
        assert!(receive(&receiver, Instant::now() + Duration::from_secs(2)).is_err());
        for object in objects {
            assert_eq!(references(object), 1);
        }
    }
    let (sender, receiver) = pair();
    assert!(matches!(
        receive(&receiver, Instant::now()),
        Err(HandoffError::Timeout)
    ));
    drop(sender);
    assert!(matches!(
        receive(&receiver, Instant::now() + Duration::from_secs(2)),
        Err(HandoffError::ControlClosed | HandoffError::MalformedTransfer)
    ));
}

#[test]
fn native_application_receiver_requires_actual_queued_sender_credentials() {
    for changed in [false, true] {
        let (sender, receiver) = pair();
        let rights: Vec<_> = (0..4).map(|_| raw_pair().0).collect();
        send(
            &sender,
            &[0; BYTES],
            &rights.iter().map(AsFd::as_fd).collect::<Vec<_>>(),
        );
        let expected = sender_identity();
        let expected = if changed {
            Client::new(
                expected.pid(),
                expected.uid().wrapping_add(1),
                expected.gid(),
            )
            .unwrap()
        } else {
            expected
        };
        assert_eq!(
            transport::receive(&receiver, expected, Instant::now() + Duration::from_secs(2))
                .is_ok(),
            !changed
        );
    }
    let (sender, receiver) = raw_pair();
    let rights: Vec<_> = (0..4).map(|_| raw_pair().0).collect();
    send(
        &sender,
        &[0; BYTES],
        &rights.iter().map(AsFd::as_fd).collect::<Vec<_>>(),
    );
    assert!(receive(&receiver, Instant::now() + Duration::from_secs(2)).is_err());
}

#[test]
fn native_application_role_aliases_are_rejected_except_pidfd_metadata_coincidence() {
    let baseline = [
        Snapshot(1, 1, 1),
        Snapshot(1, 2, 1),
        Snapshot(1, 3, 1),
        Snapshot(1, 4, 1),
        Snapshot(1, 5, 1),
    ];
    assert!(distinct_roles(&baseline).is_ok());
    for right in 1..5 {
        for left in 0..right {
            let mut objects = baseline;
            objects[right] = objects[left];
            assert_eq!(distinct_roles(&objects).is_ok(), (left, right) == (2, 4));
        }
    }
}

fn proof_pair() -> (OwnedFd, OwnedFd) {
    let pair = rustix::net::socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    for fd in [&pair.0, &pair.1] {
        rustix::net::sockopt::set_socket_passcred(fd, true).unwrap();
        rustix::net::bind(fd, &SocketAddrUnix::new_unnamed()).unwrap();
    }
    pair
}

#[test]
fn native_application_proof_peer_requires_exact_creator_flags_and_distinct_addresses() {
    let (application, peer) = proof_pair();
    let creator = Client::new(
        std::process::id(),
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    let snapshot = proof_socket_snapshot(&peer, creator).unwrap();
    let reversed = proof_socket_snapshot(&application, creator).unwrap();
    assert_eq!(snapshot.local, reversed.remote);
    assert_eq!(snapshot.remote, reversed.local);
    assert_ne!(snapshot.object, reversed.object);
    let other = Client::new(creator.pid(), creator.uid().wrapping_add(1), creator.gid()).unwrap();
    assert!(proof_socket_snapshot(&peer, other).is_err());
    rustix::net::sockopt::set_socket_passcred(&peer, false).unwrap();
    assert!(proof_socket_snapshot(&peer, creator).is_err());
    rustix::net::sockopt::set_socket_passcred(&peer, true).unwrap();
    rustix::io::fcntl_setfd(&peer, rustix::io::FdFlags::empty()).unwrap();
    assert!(proof_socket_snapshot(&peer, creator).is_err());
    rustix::io::fcntl_setfd(&peer, rustix::io::FdFlags::CLOEXEC).unwrap();
    rustix::fs::fcntl_setfl(&peer, OFlags::RDWR).unwrap();
    assert!(proof_socket_snapshot(&peer, creator).is_err());
    assert!(proof_socket_snapshot(&pair().0, creator).is_err());
}
