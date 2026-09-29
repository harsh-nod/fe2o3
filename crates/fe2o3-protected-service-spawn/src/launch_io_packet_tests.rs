use super::*;
use rustix::net::{
    AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType,
    sendmsg, socketpair,
};
use std::{
    io::IoSlice,
    mem::MaybeUninit,
    os::{fd::AsFd, unix::fs::MetadataExt},
};

fn pair(passcred: bool) -> (OwnedFd, OwnedFd) {
    // Leave the descriptors blocking: the receive must supply MSG_DONTWAIT.
    let pair = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    rustix::net::sockopt::set_socket_passcred(&pair.1, passcred).unwrap();
    pair
}

fn this_sender() -> MessageSender {
    MessageSender::new(
        rustix::process::getpid().as_raw_pid(),
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
    )
}

fn packet<const N: usize>() -> Packet<N> {
    let mut packet = Packet::empty();
    packet.payload = [0xa5; N];
    packet.bytes = N;
    packet.rights.credentials = Some(this_sender());
    packet
}

fn refs(metadata: &std::fs::Metadata) -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (metadata.dev(), metadata.ino()))
        .count()
}

#[test]
fn quotes_cover_full_envelopes_and_saturate_without_changing_readiness() {
    const SCRATCH: usize = packet_receive_scratch(AUTHENTICATED_PACKET_MAX_BYTES);
    const WORK: usize = packet_receive_work(AUTHENTICATED_PACKET_MAX_BYTES);
    assert_eq!(MAX_READY_BYTES, 88);
    assert_eq!(AUTHENTICATED_PACKET_MAX_BYTES, 4096);
    assert_eq!(size_of::<ReadyPacket>(), size_of::<Packet<88>>());
    assert!(
        SCRATCH
            >= 4 * (size_of::<Packet<4096>>()
                + size_of::<Control>()
                + size_of::<libc::iovec>()
                + size_of::<libc::msghdr>())
                + 4 * 4096
                + 8 * size_of::<Failure>()
    );
    assert!(WORK >= SCRATCH * 64 + (2 + CONTROL_BYTES / size_of::<i32>()) * (1024 + 64));
    for n in 2..AUTHENTICATED_PACKET_MAX_BYTES {
        assert!(packet_receive_scratch(n + 1) > packet_receive_scratch(n));
        assert!(packet_receive_work(n + 1) > packet_receive_work(n));
    }
    assert_eq!(packet_receive_scratch(usize::MAX), usize::MAX);
    assert_eq!(packet_receive_work(usize::MAX), usize::MAX);
}

#[test]
fn transient_syscall_results_return_none_and_other_errors_remain_io_failures() {
    for source in [Errno::AGAIN, Errno::INTR] {
        assert_eq!(
            authenticated_packet_result::<16>(Err(source), this_sender()),
            Ok(None)
        );
    }
    assert_eq!(
        authenticated_packet_result::<16>(Err(Errno::BADF), this_sender()),
        Err(Failure::Io {
            operation: "receive authenticated packet",
            source: Errno::BADF,
        })
    );
    assert_eq!(
        authenticated_packet_result(Ok(packet::<16>()), this_sender()),
        Ok(Some([0xa5; 16]))
    );
}

#[test]
fn packet_validation_requires_exact_shape_without_stage_or_eof_interpretation() {
    for bytes in [0, 1, 15, 16, 17] {
        for flags in [
            ReturnFlags::empty(),
            ReturnFlags::TRUNC,
            ReturnFlags::CTRUNC,
            ReturnFlags::TRUNC | ReturnFlags::CTRUNC,
        ] {
            for credentials in [None, Some(this_sender())] {
                let mut packet = packet::<16>();
                packet.bytes = bytes;
                packet.flags = flags;
                packet.rights.credentials = credentials;
                let result = packet.authenticate(this_sender());
                if bytes == 16 && flags.is_empty() && credentials.is_some() {
                    assert_eq!(result, Ok([0xa5; 16]));
                } else {
                    assert_eq!(result, Err(Failure::MalformedReadyTransfer));
                }
            }
        }
    }
    for stage in 0..=u8::MAX {
        let mut packet = packet::<16>();
        packet.bytes = 1;
        packet.payload[0] = stage;
        assert_eq!(
            packet.authenticate(this_sender()),
            Err(Failure::MalformedReadyTransfer)
        );
    }
}

#[test]
fn credential_parser_requires_one_exact_ucred_and_rejects_unknown_control() {
    let sender = this_sender();
    for kind in [libc::SCM_CREDENTIALS, i32::MAX] {
        for payload in 0..=size_of::<libc::ucred>() + 1 {
            let mut packet = packet::<16>();
            packet.rights.credentials = None;
            let parsed = packet.rights.header(libc::SOL_SOCKET, kind, payload);
            if kind == libc::SCM_CREDENTIALS && payload == size_of::<libc::ucred>() {
                assert_eq!(parsed, ControlKind::Credentials);
                packet.rights.credentials = Some(sender);
                assert_eq!(packet.authenticate(sender), Ok([0xa5; 16]));
            } else {
                assert_eq!(parsed, ControlKind::Rejected);
                assert_eq!(
                    packet.authenticate(sender),
                    Err(Failure::MalformedReadyTransfer)
                );
            }
        }
    }
    let mut packet = packet::<16>();
    // The first credential is already recorded; a second complete credential
    // must refuse too, independently of the receive buffer's truncation flag.
    assert_eq!(
        packet.rights.header(
            libc::SOL_SOCKET,
            libc::SCM_CREDENTIALS,
            size_of::<libc::ucred>()
        ),
        ControlKind::Rejected
    );
    assert!(packet.rights.invalid);
    assert_eq!(
        packet.authenticate(sender),
        Err(Failure::MalformedReadyTransfer)
    );
}

#[test]
fn malformed_descriptor_headers_still_require_ownership_of_complete_fds() {
    for kind in [libc::SCM_RIGHTS, SCM_PIDFD] {
        for payload in 0..=2 * size_of::<i32>() + 1 {
            let mut rights = Rights::default();
            assert_eq!(
                rights.header(libc::SOL_SOCKET, kind, payload),
                ControlKind::Descriptors
            );
            assert_eq!(
                rights.invalid,
                kind == SCM_PIDFD || payload == 0 || payload % size_of::<i32>() != 0
            );
        }
    }
    for kind in [libc::SCM_CREDENTIALS, libc::SCM_RIGHTS, SCM_PIDFD] {
        let mut rights = Rights::default();
        assert_eq!(rights.header(-1, kind, 12), ControlKind::Rejected);
        assert!(rights.invalid);
    }
}

#[test]
fn socket_receive_is_nonblocking_and_consumes_one_record_at_each_supported_size() {
    fn check<const N: usize>() {
        let (sender, receiver) = pair(true);
        let first = std::array::from_fn::<_, N, _>(|i| i as u8);
        let second = [0x5a; N];
        assert_eq!(
            receive_authenticated_packet::<N>(receiver.as_fd(), this_sender()),
            Ok(None)
        );
        for payload in [&first, &second] {
            assert_eq!(
                rustix::net::send(&sender, payload, SendFlags::NOSIGNAL).unwrap(),
                N
            );
        }
        for payload in [first, second] {
            assert_eq!(
                receive_authenticated_packet::<N>(receiver.as_fd(), this_sender()),
                Ok(Some(payload))
            );
        }
        assert_eq!(
            receive_authenticated_packet::<N>(receiver.as_fd(), this_sender()),
            Ok(None)
        );
        drop(sender);
        assert_eq!(
            receive_authenticated_packet::<N>(receiver.as_fd(), this_sender()),
            Err(Failure::MalformedReadyTransfer)
        );
    }
    check::<2>();
    check::<88>();
    check::<89>();
    check::<4096>();
}

#[test]
fn invalid_sizes_and_disabled_passcred_refuse_without_consuming_or_enabling() {
    let (sender, receiver) = pair(true);
    rustix::net::send(&sender, &[7, 8], SendFlags::NOSIGNAL).unwrap();
    assert_eq!(
        receive_authenticated_packet::<0>(receiver.as_fd(), this_sender()),
        Err(Failure::MalformedReadyTransfer)
    );
    assert_eq!(
        receive_authenticated_packet::<1>(receiver.as_fd(), this_sender()),
        Err(Failure::MalformedReadyTransfer)
    );
    assert_eq!(
        receive_authenticated_packet::<4097>(receiver.as_fd(), this_sender()),
        Err(Failure::MalformedReadyTransfer)
    );
    assert_eq!(
        receive_authenticated_packet::<2>(receiver.as_fd(), this_sender()),
        Ok(Some([7, 8]))
    );

    let (sender, receiver) = pair(false);
    rustix::net::send(&sender, &[7, 8], SendFlags::NOSIGNAL).unwrap();
    assert_eq!(
        receive_authenticated_packet::<2>(receiver.as_fd(), this_sender()),
        Err(Failure::Io {
            operation: "receive authenticated packet",
            source: Errno::INVAL,
        })
    );
    assert!(!rustix::net::sockopt::socket_passcred(&receiver).unwrap());
    let packet = receive_fixed_packet::<2>(receiver.as_fd()).unwrap();
    assert_eq!(packet.payload, [7, 8]);
    assert_eq!(packet.rights.credentials, None);
    assert_eq!(
        packet.authenticate(this_sender()),
        Err(Failure::MalformedReadyTransfer)
    );
}

#[test]
fn socket_receive_compares_pid_uid_and_gid() {
    let (sender, receiver) = pair(true);
    for field in 0..4 {
        rustix::net::send(&sender, &[9, 8], SendFlags::NOSIGNAL).unwrap();
        let mut expected = this_sender();
        match field {
            1 => expected.pid ^= 1,
            2 => expected.uid ^= 1,
            3 => expected.gid ^= 1,
            _ => {}
        }
        let result = receive_authenticated_packet::<2>(receiver.as_fd(), expected);
        assert_eq!(
            result,
            if field == 0 {
                Ok(Some([9, 8]))
            } else {
                Err(Failure::MalformedReadyTransfer)
            }
        );
    }
}

#[test]
fn socket_receive_rejects_empty_stage_short_and_extra_payloads() {
    fn check<const N: usize>() {
        let (sender, receiver) = pair(true);
        for length in [0, 1, N - 1, N + 1] {
            rustix::net::send(&sender, &vec![0xa5; length], SendFlags::NOSIGNAL).unwrap();
            assert_eq!(
                receive_authenticated_packet::<N>(receiver.as_fd(), this_sender()),
                Err(Failure::MalformedReadyTransfer),
                "N={N}, length={length}"
            );
        }
    }
    check::<2>();
    check::<16>();
    check::<4096>();
}

#[test]
fn socket_refusals_close_rights_including_ancillary_overflow() {
    let (sender, receiver) = pair(true);
    let file = tempfile::NamedTempFile::new().unwrap();
    let metadata = file.as_file().metadata().unwrap();
    for length in [0, 1, 15, 16, 17] {
        for count in [1, 2, 12] {
            for wrong_sender in [false, true] {
                let fds = [file.as_fd(); 12];
                let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(12))];
                let mut control = SendAncillaryBuffer::new(&mut space);
                assert!(control.push(SendAncillaryMessage::ScmRights(&fds[..count])));
                sendmsg(
                    &sender,
                    &[IoSlice::new(&[0xa5; 17][..length])],
                    &mut control,
                    SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
                )
                .unwrap();
                let mut expected = this_sender();
                if wrong_sender {
                    expected.pid ^= 1;
                }
                assert_eq!(
                    receive_authenticated_packet::<16>(receiver.as_fd(), expected),
                    Err(Failure::MalformedReadyTransfer)
                );
                assert_eq!(
                    refs(&metadata),
                    1,
                    "leaked right: length={length}, count={count}, wrong_sender={wrong_sender}"
                );
            }
        }
    }
}

#[test]
fn parser_disposes_rights_and_pidfds_even_after_an_unknown_header() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let metadata = file.as_file().metadata().unwrap();
    for kind in [libc::SCM_RIGHTS, SCM_PIDFD] {
        for count in [1, 2] {
            for unknown_first in [false, true] {
                for truncated in [false, true] {
                    let mut packet = packet::<16>();
                    if unknown_first {
                        assert_eq!(
                            packet.rights.header(libc::SOL_SOCKET, i32::MAX, 0),
                            ControlKind::Rejected
                        );
                    }
                    if truncated {
                        packet.flags = ReturnFlags::CTRUNC;
                    }
                    assert_eq!(
                        packet
                            .rights
                            .header(libc::SOL_SOCKET, kind, count * size_of::<i32>()),
                        ControlKind::Descriptors
                    );
                    // Exercise the parser's owned-FD sink without synthesizing
                    // raw ownership or requiring kernel SCM_PIDFD support.
                    for _ in 0..count {
                        packet.rights.push(rustix::io::dup(&file).unwrap());
                    }
                    assert_eq!(
                        packet.authenticate(this_sender()),
                        Err(Failure::MalformedReadyTransfer)
                    );
                    assert_eq!(refs(&metadata), 1, "leaked kind={kind}, count={count}");
                }
            }
        }
    }
}
