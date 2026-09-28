use super::*;
use rustix::{
    fd::AsFd,
    net::{SendAncillaryBuffer, SendAncillaryMessage, SocketFlags, sendmsg, socketpair},
    process::{Gid, Pid, Uid, WaitOptions, getgid, getpid, getuid, waitpid},
};
use std::{
    io::{IoSlice, Read},
    os::{fd::AsRawFd, unix::net::UnixStream},
    sync::atomic::{AtomicUsize, Ordering},
};

const FLAGS: SocketFlags = SocketFlags::CLOEXEC.union(SocketFlags::NONBLOCK);

fn credentials() -> UCred {
    UCred {
        pid: getpid(),
        uid: getuid(),
        gid: getgid(),
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

fn descriptors() -> (OwnedFd, OwnedFd) {
    let pair = socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, FLAGS, None).unwrap();
    sockopt::set_socket_passcred(&pair.0, true).unwrap();
    sockopt::set_socket_passcred(&pair.1, true).unwrap();
    pair
}

fn sockets() -> (CredentialSocketV1, CredentialSocketV1) {
    let (left, right) = descriptors();
    (
        CredentialSocketV1::new(left, credentials()).unwrap(),
        CredentialSocketV1::new(right, credentials()).unwrap(),
    )
}

#[test]
fn packets_preserve_boundaries_and_the_maximum_size() {
    let (sender, receiver) = sockets();
    let bytes = vec![0x5a; PACKET_BYTES];
    sender.send_packet(&bytes, deadline()).unwrap();
    sender.send_packet(b"next", deadline()).unwrap();
    let mut buffer = vec![0; PACKET_BYTES + 1];
    assert_eq!(
        receiver.receive_packet(&mut buffer, deadline()).unwrap(),
        Some(PACKET_BYTES)
    );
    assert_eq!(&buffer[..PACKET_BYTES], bytes);
    assert_eq!(
        receiver.receive_packet(&mut buffer, deadline()).unwrap(),
        Some(4)
    );
    assert_eq!(&buffer[..4], b"next");
}

#[test]
fn empty_packets_are_not_eof_even_with_an_empty_buffer() {
    let (sender, receiver) = sockets();
    sender.send_packet(&[], deadline()).unwrap();
    sender.send_packet(&[], deadline()).unwrap();
    sender.send_packet(b"tail", deadline()).unwrap();
    drop(sender);
    let mut buffer = [0; 8];
    assert_eq!(
        receiver.receive_packet(&mut [], deadline()).unwrap(),
        Some(0)
    );
    assert_eq!(
        receiver.receive_packet(&mut buffer, deadline()).unwrap(),
        Some(0)
    );
    assert_eq!(
        receiver.receive_packet(&mut buffer, deadline()).unwrap(),
        Some(4)
    );
    assert_eq!(&buffer[..4], b"tail");
    assert_eq!(
        receiver.receive_packet(&mut buffer, deadline()).unwrap(),
        None
    );
}

#[test]
fn constructor_rejects_missing_flags_passcred_and_wrong_socket_type() {
    for flags in [
        SocketFlags::CLOEXEC,
        SocketFlags::NONBLOCK,
        SocketFlags::empty(),
    ] {
        let (fd, _peer) =
            socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, flags, None).unwrap();
        sockopt::set_socket_passcred(&fd, true).unwrap();
        assert!(CredentialSocketV1::new(fd, credentials()).is_err());
    }
    for kind in [SocketType::STREAM, SocketType::DGRAM] {
        let (fd, _peer) = socketpair(AddressFamily::UNIX, kind, FLAGS, None).unwrap();
        sockopt::set_socket_passcred(&fd, true).unwrap();
        assert!(CredentialSocketV1::new(fd, credentials()).is_err());
    }
    let (fd, _peer) = descriptors();
    sockopt::set_socket_passcred(&fd, false).unwrap();
    assert!(CredentialSocketV1::new(fd, credentials()).is_err());
    let file = std::fs::File::open("/dev/null").unwrap();
    assert!(CredentialSocketV1::new(file.into(), credentials()).is_err());
    let unconnected =
        net::socket_with(AddressFamily::UNIX, SocketType::SEQPACKET, FLAGS, None).unwrap();
    sockopt::set_socket_passcred(&unconnected, true).unwrap();
    assert!(CredentialSocketV1::new(unconnected, credentials()).is_err());
}

#[test]
fn constructor_rejects_named_connections_and_listeners() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let name = format!(
        "fe2o3-executor-socket-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let address = SocketAddrUnix::new_abstract_name(name.as_bytes()).unwrap();
    let listener =
        net::socket_with(AddressFamily::UNIX, SocketType::SEQPACKET, FLAGS, None).unwrap();
    net::bind(&listener, &address).unwrap();
    net::listen(&listener, 1).unwrap();
    let client = net::socket_with(AddressFamily::UNIX, SocketType::SEQPACKET, FLAGS, None).unwrap();
    net::connect(&client, &address).unwrap();
    let server = net::accept_with(&listener, FLAGS).unwrap();
    for fd in [client, server, listener] {
        sockopt::set_socket_passcred(&fd, true).unwrap();
        assert!(CredentialSocketV1::new(fd, credentials()).is_err());
    }
}

#[test]
fn losing_passcred_cannot_turn_an_empty_packet_into_eof() {
    let (sender, receiver) = sockets();
    sockopt::set_socket_passcred(&receiver.fd, false).unwrap();
    sender.send_packet(&[], deadline()).unwrap();
    assert_eq!(
        receiver
            .receive_packet(&mut [], deadline())
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
    // Confirm the rejected operation did not consume the credential-free packet.
    let mut storage = CredentialControl {
        alignment: [],
        bytes: [MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmCredentials(1))],
    };
    let mut control = RecvAncillaryBuffer::new(&mut storage.bytes);
    let result = net::recvmsg(
        &receiver.fd,
        &mut [IoSliceMut::new(&mut [])],
        &mut control,
        RecvFlags::DONTWAIT,
    )
    .unwrap();
    assert_eq!(result.bytes, 0);
    assert_eq!(control.drain().count(), 0);
}

#[test]
fn credentials_are_checked_for_every_packet_including_empty_packets() {
    for empty in [false, true] {
        for field in 0..3 {
            let (sender, fd) = descriptors();
            let mut expected = credentials();
            match field {
                0 => {
                    expected.pid = Pid::from_raw(if getpid().as_raw_nonzero().get() == 1 {
                        2
                    } else {
                        1
                    })
                    .unwrap()
                }
                1 => expected.uid = Uid::from_raw(expected.uid.as_raw() ^ 1),
                _ => expected.gid = Gid::from_raw(expected.gid.as_raw() ^ 1),
            }
            // Construction deliberately makes no positive identity judgment.
            let receiver = CredentialSocketV1::new(fd, expected).unwrap();
            let payload: &[u8] = if empty { b"" } else { b"wrong" };
            net::send(&sender, payload, SendFlags::DONTWAIT | SendFlags::NOSIGNAL).unwrap();
            assert_eq!(
                receiver
                    .receive_packet(&mut [0; 16], deadline())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::PermissionDenied
            );
        }
    }
}

#[test]
fn oversized_sends_are_rejected_before_transmission() {
    let (sender, receiver) = sockets();
    assert_eq!(
        sender
            .send_packet(&vec![0; PACKET_BYTES + 1], deadline())
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        net::recv(&receiver.fd, &mut [0; 1], RecvFlags::DONTWAIT).unwrap_err(),
        Errno::AGAIN
    );
}

#[test]
fn truncated_packets_are_never_reported_as_partial_success() {
    for (payload_len, buffer_len) in [(1, 0), (8, 4), (PACKET_BYTES + 1, PACKET_BYTES + 1)] {
        let (sender, receiver) = sockets();
        net::send(
            &sender.fd,
            &vec![0; payload_len],
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )
        .unwrap();
        let error = receiver
            .receive_packet(&mut vec![0; buffer_len], deadline())
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}

#[test]
fn partial_send_results_are_errors_not_continuation_packets() {
    // SEQPACKET sends are atomic; exercise the defensive syscall-result check.
    assert!(require_complete_packet(0, 0).is_ok());
    assert!(require_complete_packet(PACKET_BYTES, PACKET_BYTES).is_ok());
    for (sent, expected) in [(0, 1), (3, 4), (5, 4)] {
        assert_eq!(
            require_complete_packet(sent, expected).unwrap_err().kind(),
            io::ErrorKind::WriteZero
        );
    }
}

#[test]
fn rights_are_rejected_and_all_transferred_writers_are_closed() {
    for count in [1, 8] {
        let (sender, receiver) = sockets();
        let (mut reader, writer) = UnixStream::pair().unwrap();
        reader.set_nonblocking(true).unwrap();
        let descriptors = vec![writer.as_fd(); count];
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
        let mut control = SendAncillaryBuffer::new(&mut storage);
        assert!(control.push(SendAncillaryMessage::ScmRights(&descriptors)));
        assert_eq!(
            sendmsg(
                &sender.fd,
                &[IoSlice::new(b"rights")],
                &mut control,
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL
            )
            .unwrap(),
            6
        );
        drop(control);
        drop(descriptors);
        drop(writer);
        assert_eq!(
            receiver
                .receive_packet(&mut [0; 16], deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        // EOF is descriptor-local evidence, unlike a racy process-wide FD count.
        assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
    }
}

#[test]
fn unknown_timestamp_control_is_rejected() {
    use std::ffi::{c_int, c_void};
    unsafe extern "C" {
        fn setsockopt(
            fd: c_int,
            level: c_int,
            name: c_int,
            value: *const c_void,
            length: u32,
        ) -> c_int;
    }
    let (sender, receiver) = sockets();
    let enabled: c_int = 1;
    // SAFETY: a live socket and initialized int of the stated length. Linux
    // SOL_SOCKET=1, SO_TIMESTAMP_OLD=29; rustix has no timestamp sockopt API.
    assert_eq!(
        unsafe {
            setsockopt(
                receiver.fd.as_raw_fd(),
                1,
                29,
                (&enabled as *const c_int).cast(),
                size_of::<c_int>() as u32,
            )
        },
        0
    );
    sender.send_packet(b"timestamp", deadline()).unwrap();
    assert_eq!(
        receiver
            .receive_packet(&mut [0; 16], deadline())
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn expired_deadline_does_not_consume_or_send_a_packet() {
    let (sender, receiver) = sockets();
    sender.send_packet(b"queued", deadline()).unwrap();
    let expired = Instant::now();
    assert_eq!(
        sender.send_packet(b"late", expired).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    let mut buffer = [0; 16];
    assert_eq!(
        receiver
            .receive_packet(&mut buffer, expired)
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(
        receiver.receive_packet(&mut buffer, deadline()).unwrap(),
        Some(6)
    );
    assert_eq!(&buffer[..6], b"queued");
    assert_eq!(
        net::recv(&receiver.fd, &mut buffer, RecvFlags::DONTWAIT).unwrap_err(),
        Errno::AGAIN
    );
}

#[test]
fn idle_receive_expires_at_the_absolute_deadline() {
    let (_sender, receiver) = sockets();
    let until = Instant::now() + Duration::from_millis(20);
    assert_eq!(
        receiver
            .receive_packet(&mut [0; 16], until)
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    assert!(Instant::now() >= until);
}

#[test]
fn backpressure_waits_on_the_socket_until_the_absolute_deadline() {
    let (sender, _receiver) = sockets();
    sockopt::set_socket_send_buffer_size(&sender.fd, 4096).unwrap();
    let mut full = false;
    for _ in 0..1024 {
        match net::send(
            &sender.fd,
            &[0; 512],
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(512) => {}
            Err(Errno::AGAIN) => {
                full = true;
                break;
            }
            other => panic!("unexpected queue fill result: {other:?}"),
        }
    }
    assert!(full);
    let until = Instant::now() + Duration::from_millis(20);
    assert_eq!(
        sender.send_packet(&[0; 512], until).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert!(Instant::now() >= until);
}

#[test]
fn closed_peer_send_returns_an_error_without_sigpipe() {
    let (sender, receiver) = sockets();
    drop(receiver);
    assert!(sender.send_packet(b"closed", deadline()).is_err());
}

// Raw Linux x86_64 syscalls bypass libc's registered atfork callbacks. The child
// never touches the test harness, allocator, spawn coordinator or destructors.
fn fork_writer(fd: &OwnedFd) -> Pid {
    // This is a one-way fixture: only its receiving endpoint needs PASSCRED.
    // Keep the writer unnamed even on kernels that autobind PASSCRED senders.
    sockopt::set_socket_passcred(fd, false).unwrap();
    let raw_fd = fd.as_raw_fd();
    let flags = (SendFlags::DONTWAIT | SendFlags::NOSIGNAL).bits() as usize;
    let pointer = b"child".as_ptr();
    fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| {
        let child: i64;
        // SAFETY: SYS_fork=57 on this module's Linux x86_64 target. Only the
        // syscall-only branch below runs in the child and it never returns.
        unsafe {
            std::arch::asm!(
                "syscall",
                inlateout("rax") 57_i64 => child,
                lateout("rcx") _, lateout("r11") _,
            );
        }
        if child == 0 {
            let sent: i64;
            // SAFETY: SYS_sendto=44 receives a live inherited socket, static
            // five-byte payload and no address. SYS_exit_group=231 closes all
            // child aliases. Trap if exit unexpectedly returns; never unwind.
            unsafe {
                std::arch::asm!(
                    "syscall",
                    inlateout("rax") 44_i64 => sent,
                    in("rdi") raw_fd as usize,
                    in("rsi") pointer,
                    in("rdx") 5_usize,
                    in("r10") flags,
                    in("r8") 0_usize, in("r9") 0_usize,
                    lateout("rcx") _, lateout("r11") _,
                );
                std::arch::asm!(
                    "syscall", "ud2",
                    in("rax") 231_usize,
                    in("rdi") if sent == 5 { 0_usize } else { 1_usize },
                    options(noreturn),
                );
            }
        }
        if child < 0 {
            return Err(io::Error::from_raw_os_error(-child as i32));
        }
        let child = Pid::from_raw(child as i32).unwrap();
        // This child does not exec: hold the artifact spawn boundary until
        // terminal wait confirms its inherited lock descriptors were closed.
        reap(child);
        Ok(child)
    })
    .unwrap()
}

fn reap(child: Pid) {
    loop {
        match waitpid(Some(child), WaitOptions::empty()) {
            Err(Errno::INTR) => continue,
            result => {
                let (observed, status) = result.unwrap().unwrap();
                assert_eq!(observed, child);
                assert_eq!(status.exit_status(), Some(0));
                return;
            }
        }
    }
}

#[test]
fn prefork_peercred_is_not_the_current_packet_writer() {
    let (sender, fd) = descriptors();
    let creator = sockopt::socket_peercred(&fd).unwrap();
    assert_eq!(creator.pid, getpid());
    let child = fork_writer(&sender);
    assert_eq!(sockopt::socket_peercred(&fd).unwrap(), creator);
    assert_ne!(child, creator.pid);
    let receiver = CredentialSocketV1::new(
        fd,
        UCred {
            pid: child,
            ..credentials()
        },
    )
    .unwrap();
    let mut bytes = [0; 8];
    assert_eq!(
        receiver.receive_packet(&mut bytes, deadline()).unwrap(),
        Some(5)
    );
    assert_eq!(&bytes[..5], b"child");
    // Credentials are checked again: the creator cannot continue as the child.
    net::send(
        &sender,
        b"parent",
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    )
    .unwrap();
    assert_eq!(
        receiver
            .receive_packet(&mut bytes, deadline())
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}

#[test]
fn a_forked_writer_cannot_pass_the_creators_expected_credentials() {
    let (sender, fd) = descriptors();
    let creator = sockopt::socket_peercred(&fd).unwrap();
    let child = fork_writer(&sender);
    assert_ne!(child, creator.pid);
    let receiver = CredentialSocketV1::new(fd, creator).unwrap();
    assert_eq!(
        receiver
            .receive_packet(&mut [0; 8], deadline())
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}
