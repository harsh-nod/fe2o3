use super::*;
use rustix::net::{
    AddressFamily, SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType,
    sendmsg, socketpair,
};
use std::{
    io::IoSlice,
    mem::MaybeUninit,
    os::{fd::AsFd, unix::fs::MetadataExt},
    process::{Command, Stdio},
};

// Private kernel-shaped records; integers acquire ownership only at take_control.
#[allow(unsafe_code)]
pub(super) fn control(kind: i32, payload: &[i32]) -> (Control, usize) {
    let length = size_of::<libc::cmsghdr>() + size_of_val(payload);
    let padded = length.next_multiple_of(size_of::<usize>());
    assert!(padded <= CONTROL_BYTES);
    let mut control = Control {
        words: [0; CONTROL_BYTES / size_of::<usize>()],
    };
    // SAFETY: the zeroed aligned buffer holds one initialized cmsghdr and the
    // checked payload. No descriptor is taken or closed by this serialization.
    unsafe {
        let header = (&raw mut control).cast::<libc::cmsghdr>();
        (*header).cmsg_len = length as _;
        (*header).cmsg_level = libc::SOL_SOCKET;
        (*header).cmsg_type = kind;
        std::ptr::copy_nonoverlapping(
            payload.as_ptr(),
            header
                .cast::<u8>()
                .add(size_of::<libc::cmsghdr>())
                .cast::<i32>(),
            payload.len(),
        );
    }
    (control, padded)
}

#[derive(Default)]
struct Watch(usize);
impl Observer for Watch {
    type Error = ();
    fn before_attempt(&mut self, boundary: Boundary) -> Result<(), ()> {
        assert_eq!(boundary, Boundary::ReadyTransfer);
        self.0 += 1;
        Ok(())
    }
    fn is_live(&mut self) -> Result<bool, ()> {
        panic!("all test records must already be queued")
    }
}

fn pair(passcred: bool) -> (OwnedFd, OwnedFd) {
    let pair = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
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

#[test]
fn authenticated_canonical_ready_precedes_bootstrap_eof() {
    // Canonical anchor-ready bytes are inert here; family decoding and endpoint
    // admission remain coordinator obligations, not transport test authority.
    let ready = *b"F2O3AHR1\x01\x00\x02\x00\x00\x00\x00\x00";
    let (sender, receiver) = pair(true);
    let (endpoint, _peer) = pair(false);
    let expected = rustix::fs::fstat(&endpoint).unwrap();
    let fds = [endpoint.as_fd()];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    assert!(control.push(SendAncillaryMessage::ScmRights(&fds)));
    assert_eq!(
        sendmsg(
            &sender,
            &[IoSlice::new(&ready)],
            &mut control,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
        )
        .unwrap(),
        ready.len()
    );
    drop(sender);
    let mut watch = Watch::default();
    let (bytes, received) = receive_ready_from::<16, true, _>(
        receiver.as_fd(),
        this_sender(),
        &mut watch,
        Instant::now() + MAX_TIMEOUT,
    )
    .unwrap();
    assert_eq!(bytes, ready);
    let received = received.unwrap();
    let actual = rustix::fs::fstat(&received).unwrap();
    assert_eq!(
        (actual.st_dev, actual.st_ino, actual.st_mode),
        (expected.st_dev, expected.st_ino, expected.st_mode)
    );
    assert!(
        rustix::io::fcntl_getfd(&received)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_eq!(watch.0, 1);
}

#[test]
fn authenticated_pre_ready_eof_is_distinct_from_an_empty_record() {
    for empty_record in [false, true] {
        let (sender, receiver) = pair(true);
        if empty_record {
            assert_eq!(
                rustix::net::send(&sender, &[], SendFlags::NOSIGNAL).unwrap(),
                0
            );
        }
        drop(sender);
        let mut watch = Watch::default();
        if empty_record {
            assert!(matches!(
                receive_ready_from::<16, true, _>(
                    receiver.as_fd(),
                    this_sender(),
                    &mut watch,
                    Instant::now() + MAX_TIMEOUT,
                ),
                Err(Error::Failure(Failure::MalformedReadyTransfer))
            ));
        }
        assert!(matches!(
            receive_ready_from::<16, true, _>(
                receiver.as_fd(),
                this_sender(),
                &mut watch,
                Instant::now() + MAX_TIMEOUT,
            ),
            Err(Error::Failure(Failure::ChildExited(
                "bootstrap EOF before service-ready transfer"
            )))
        ));
        assert_eq!(watch.0, 1 + usize::from(empty_record));
    }
}

#[test]
fn authenticated_one_byte_ready_records_remain_failure_stages() {
    let (sender, receiver) = pair(true);
    for stage in 0..=u8::MAX {
        rustix::net::send(&sender, &[stage], SendFlags::NOSIGNAL).unwrap();
        let mut watch = Watch::default();
        assert!(matches!(
            receive_ready_from::<16, true, _>(
                receiver.as_fd(),
                this_sender(),
                &mut watch,
                Instant::now() + MAX_TIMEOUT,
            ),
            Err(Error::Failure(Failure::ChildStage(value))) if value == stage
        ));
        assert_eq!(watch.0, 1);
    }
}

#[test]
fn ready_eof_diagnostic_preserves_authentication_and_control_refusals() {
    for mode in 0..7 {
        for rights in [false, true] {
            let mut packet = ReadyPacket::empty();
            match mode {
                0 => packet.rights.invalid = true,
                1 => packet.flags = ReturnFlags::TRUNC,
                2 => packet.flags = ReturnFlags::CTRUNC,
                3 => packet.rights.credentials = Some(this_sender()),
                4 => {
                    let mut wrong = this_sender();
                    wrong.pid += 1;
                    packet.rights.credentials = Some(wrong);
                }
                5 => packet.bytes = 1,
                6 => packet.bytes = 16,
                _ => unreachable!(),
            }
            assert!(matches!(
                packet.validate_from(16, rights, Some(this_sender())),
                Err(Failure::MalformedReadyTransfer)
            ));
        }
    }
    // Legacy receives cannot distinguish an unmarked empty record from EOF.
    assert!(matches!(
        ReadyPacket::empty().validate_from(16, true, None),
        Err(Failure::MalformedReadyTransfer)
    ));
    // Even zero-byte transfers carrying a real received right remain malformed.
    let (sender, receiver) = pair(true);
    let (endpoint, _peer) = pair(false);
    let fds = [endpoint.as_fd()];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    assert!(control.push(SendAncillaryMessage::ScmRights(&fds)));
    assert_eq!(
        sendmsg(
            &sender,
            &[IoSlice::new(&[])],
            &mut control,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
        )
        .unwrap(),
        0
    );
    let mut packet = receive_packet(receiver.as_fd()).unwrap();
    assert!(packet.rights.fd.is_some());
    packet.rights.credentials = None;
    assert!(matches!(
        packet.validate_from(16, true, Some(this_sender())),
        Err(Failure::MalformedReadyTransfer)
    ));
}

#[test]
fn credential_and_rights_receive_is_exact_and_disposes_all_rejected_rights() {
    let (sender, receiver) = pair(true);
    let file = tempfile::NamedTempFile::new().unwrap();
    let metadata = file.as_file().metadata().unwrap();
    let refs = || {
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
            .filter(|m| (m.dev(), m.ino()) == (metadata.dev(), metadata.ino()))
            .count()
    };
    for length in [0, 1, 2, MAX_READY_BYTES, MAX_READY_BYTES + 1] {
        for count in 0..=12 {
            for wrong_sender in [false, true] {
                let fds = [file.as_fd(); 12];
                let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(12))];
                let mut control = SendAncillaryBuffer::new(&mut space);
                if count > 0 {
                    assert!(control.push(SendAncillaryMessage::ScmRights(&fds[..count])));
                }
                sendmsg(
                    &sender,
                    &[IoSlice::new(&[0xa5; MAX_READY_BYTES + 1][..length])],
                    &mut control,
                    SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
                )
                .unwrap();
                let mut expected = this_sender();
                if wrong_sender {
                    expected.pid += 1;
                }
                let mut watch = Watch::default();
                let result = receive_ready_from::<MAX_READY_BYTES, true, _>(
                    receiver.as_fd(),
                    expected,
                    &mut watch,
                    Instant::now() + MAX_TIMEOUT,
                );
                if length == MAX_READY_BYTES && count == 1 && !wrong_sender {
                    let (bytes, fd) = result.unwrap();
                    assert_eq!(bytes, [0xa5; MAX_READY_BYTES]);
                    let fd = fd.unwrap();
                    assert!(
                        rustix::io::fcntl_getfd(&fd)
                            .unwrap()
                            .contains(rustix::io::FdFlags::CLOEXEC)
                    );
                    assert_eq!(refs(), 2);
                    drop(fd);
                } else if length == 1 && count == 0 && !wrong_sender {
                    assert!(matches!(
                        result,
                        Err(Error::Failure(Failure::ChildStage(0xa5)))
                    ));
                } else {
                    assert!(matches!(
                        result,
                        Err(Error::Failure(Failure::MalformedReadyTransfer))
                    ));
                }
                assert_eq!(refs(), 1, "leaked right: length={length} count={count}");
                assert_eq!(watch.0, 1);
            }
        }
    }
}

#[test]
fn credentials_are_mandatory_and_each_field_is_compared() {
    for field in 0..4 {
        let (sender, receiver) = pair(true);
        rustix::net::send(&sender, &[9, 8], SendFlags::NOSIGNAL).unwrap();
        let mut expected = this_sender();
        match field {
            1 => expected.pid += 1,
            2 => expected.uid ^= 1,
            3 => expected.gid ^= 1,
            _ => {}
        }
        let result = receive_ready_from::<2, false, _>(
            receiver.as_fd(),
            expected,
            &mut Watch::default(),
            Instant::now() + MAX_TIMEOUT,
        );
        if field == 0 {
            assert_eq!(result.unwrap().0, [9, 8]);
        } else {
            assert!(matches!(
                result,
                Err(Error::Failure(Failure::MalformedReadyTransfer))
            ));
        }
    }
    let (sender, receiver) = pair(false);
    rustix::net::send(&sender, &[9, 8], SendFlags::NOSIGNAL).unwrap();
    assert!(matches!(
        receive_ready_from::<2, false, _>(
            receiver.as_fd(),
            this_sender(),
            &mut Watch::default(),
            Instant::now() + MAX_TIMEOUT
        ),
        Err(Error::Failure(Failure::Io {
            source: Errno::INVAL,
            ..
        }))
    ));
    let packet = receive_packet(receiver.as_fd()).unwrap();
    assert!(packet.rights.credentials.is_none());
    assert!(matches!(
        packet.validate_from(2, false, Some(this_sender())),
        Err(Failure::MalformedReadyTransfer)
    ));
}

#[test]
fn packet_writer_is_not_the_socket_creator() {
    const CHILD: &str = "FE2O3_CREDENTIAL_WRITER_TEST";
    if std::env::var_os(CHILD).is_some() {
        rustix::net::send(std::io::stdin().as_fd(), &[5, 6], SendFlags::NOSIGNAL).unwrap();
        return;
    }
    for expect_creator in [false, true] {
        let (sender, receiver) = pair(true);
        let creator = this_sender();
        assert_eq!(
            rustix::net::sockopt::socket_peercred(&receiver)
                .unwrap()
                .pid
                .as_raw_pid(),
            creator.pid
        );
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "launch_io::credential_tests::packet_writer_is_not_the_socket_creator",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .stdin(Stdio::from(sender))
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let writer = MessageSender::new(child.id().try_into().unwrap(), creator.uid, creator.gid);
        let limit = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= limit {
                let _ = child.kill();
                let _ = child.wait();
                panic!("credential writer subprocess timed out");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(child.wait().unwrap().success());
        let result = receive_ready_from::<2, false, _>(
            receiver.as_fd(),
            if expect_creator { creator } else { writer },
            &mut Watch::default(),
            Instant::now() + MAX_TIMEOUT,
        );
        if expect_creator {
            assert!(matches!(
                result,
                Err(Error::Failure(Failure::MalformedReadyTransfer))
            ));
        } else {
            assert_eq!(result.unwrap().0, [5, 6]);
        }
    }
}

#[test]
fn exec_eof_does_not_close_a_separate_bootstrap() {
    let (bootstrap_child, bootstrap_root) = pair(true);
    let (exec_child, exec_root) = pair(false);
    drop(exec_child);
    let mut io = SystemIo;
    assert_eq!(
        io.status(exec_root.as_fd(), &mut [0; 2], true).unwrap(),
        (0, 0)
    );
    rustix::net::send(&bootstrap_child, &[1, 2], SendFlags::NOSIGNAL).unwrap();
    assert_eq!(
        receive_ready_from::<2, false, _>(
            bootstrap_root.as_fd(),
            this_sender(),
            &mut Watch::default(),
            Instant::now() + MAX_TIMEOUT
        )
        .unwrap()
        .0,
        [1, 2]
    );
}
