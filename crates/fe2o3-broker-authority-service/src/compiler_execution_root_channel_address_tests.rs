//! Real socket mechanics only; no root channel or issuer admission is fabricated.
use super::*;
use transport::MessageSender;

#[test]
fn address_shape_accepts_only_unnamed_or_linux_autobind_form() {
    assert!(socketpair_address(
        net::SocketAddrUnix::new_unnamed().into()
    ));
    for name in [b"00000".as_slice(), b"abcde", b"12f90", b"fffff"] {
        assert!(socketpair_address(
            net::SocketAddrUnix::new_abstract_name(name).unwrap().into()
        ));
    }
    for name in [
        b"".as_slice(),
        b"1234",
        b"123456",
        b"ABCDE",
        b"1234g",
        b"12\0ab",
    ] {
        assert!(!socketpair_address(
            net::SocketAddrUnix::new_abstract_name(name).unwrap().into()
        ));
    }
    assert!(!socketpair_address(
        net::SocketAddrUnix::new("/tmp/fe2o3-not-a-root-endpoint")
            .unwrap()
            .into()
    ));
    assert!(!socketpair_address(
        net::SocketAddrV4::new(net::Ipv4Addr::LOCALHOST, 1234).into()
    ));
}

#[test]
fn credentialed_socketpair_roundtrip_preserves_shape_after_both_autobinds() {
    let (root, issuer) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    for fd in [&root, &issuer] {
        net::sockopt::set_socket_passcred(fd, true).unwrap();
        assert_eq!(
            net::getsockname(fd).unwrap(),
            net::SocketAddrUnix::new_unnamed().into()
        );
    }
    let sender = MessageSender::new(
        process::getpid().as_raw_pid(),
        process::getuid().as_raw(),
        process::getgid().as_raw(),
    );
    for (writer, reader) in [(&root, &issuer), (&issuer, &root), (&root, &issuer)] {
        assert_eq!(
            transport::send_packet(writer.as_fd(), b"gate").unwrap(),
            Some(())
        );
        for fd in [&root, &issuer] {
            assert!(socketpair_address(net::getsockname(fd).unwrap()));
            assert!(
                net::getpeername(fd)
                    .unwrap()
                    .is_some_and(socketpair_address)
            );
            let peer = net::sockopt::socket_peercred(fd).unwrap();
            assert_eq!(peer.pid, process::getpid());
            assert_eq!(peer.uid, process::getuid());
            assert_eq!(peer.gid, process::getgid());
        }
        assert_eq!(
            transport::receive_authenticated_packet::<4>(reader.as_fd(), sender).unwrap(),
            Some(*b"gate")
        );
    }
    for fd in [&root, &issuer] {
        let address = net::SocketAddrUnix::try_from(net::getsockname(fd).unwrap()).unwrap();
        assert_eq!(address.abstract_name().unwrap().len(), 5);
    }
}
