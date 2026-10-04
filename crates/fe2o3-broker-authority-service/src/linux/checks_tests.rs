use super::*;
use std::fmt::Write;

struct Rendered {
    bytes: [u8; 256],
    length: usize,
}

impl fmt::Write for Rendered {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}

fn assert_diagnostic(error: CheckError, kind: Kind, message: &str, errno: Option<i32>) {
    assert_eq!(error.kind(), kind);
    assert_eq!(error.errno(), errno);
    let mut rendered = Rendered {
        bytes: [0; 256],
        length: 0,
    };
    write!(&mut rendered, "{error}").unwrap();
    assert_eq!(&rendered.bytes[..rendered.length], message.as_bytes());

    let legacy = ProtectedServiceAdmissionErrorV1::from(error);
    assert_eq!(legacy.kind(), kind);
    assert_eq!(legacy.to_string(), message);
    let source = std::error::Error::source(&legacy);
    match errno {
        Some(errno) => assert_eq!(
            source
                .unwrap()
                .downcast_ref::<io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(errno)
        ),
        None => assert!(source.is_none()),
    }
}

fn assert_refused<T>(result: Result<T>, kind: Kind, message: &str, errno: Option<i32>) {
    match result {
        Err(error) => assert_diagnostic(error, kind, message, errno),
        Ok(_) => panic!("unexpected acceptance"),
    }
}

#[test]
fn fixed_messages_and_legacy_conversion_preserve_kind_text_and_source() {
    assert!(std::mem::size_of::<CheckError>() <= 128);
    assert_diagnostic(
        CheckError::new(Kind::ClientAlreadyDead, "client already dead"),
        Kind::ClientAlreadyDead,
        "client already dead",
        None,
    );
    assert_diagnostic(
        CheckError::io(Kind::InspectPeer, "inspect peer", rustix::io::Errno::IO),
        Kind::InspectPeer,
        "inspect peer",
        Some(libc::EIO),
    );
    assert_diagnostic(
        CheckError::parts(
            Kind::InspectRoot,
            "cannot inspect retained ",
            "root",
            " descriptor",
            Some(libc::EBADF),
        ),
        Kind::InspectRoot,
        "cannot inspect retained root descriptor",
        Some(libc::EBADF),
    );
    assert_diagnostic(
        CheckError::parts(
            Kind::PeerCloseOnExec,
            "",
            "transferred peer",
            " descriptor does not have FD_CLOEXEC",
            None,
        ),
        Kind::PeerCloseOnExec,
        "transferred peer descriptor does not have FD_CLOEXEC",
        None,
    );
}

#[test]
fn scalar_messages_keep_decimal_pid_and_signed_i16_hex_rendering() {
    assert_refused(
        require_pidfd_target(u32::MAX, 0),
        Kind::ClientPidfdTargetMismatch,
        "client pidfd targets PID 4294967295, expected exact PID 0",
        None,
    );
    assert!(require_pidfd_target(0, 0).is_ok());
    assert!(require_client_start_time(0, 0).is_ok());
    assert_refused(
        require_client_start_time(1, 2),
        Kind::ClientStartTimeChanged,
        "retained client process start time changed",
        None,
    );
    for (events, expected) in [
        (0, "client pidfd returned unexpected poll events 0x0"),
        (1, "client pidfd returned unexpected poll events 0x1"),
        (
            i16::MAX,
            "client pidfd returned unexpected poll events 0x7fff",
        ),
        (-1, "client pidfd returned unexpected poll events 0xffff"),
        (
            i16::MIN,
            "client pidfd returned unexpected poll events 0x8000",
        ),
    ] {
        assert_diagnostic(
            CheckError::poll_events(events),
            Kind::InspectClientPidfd,
            expected,
            None,
        );
    }
}

#[test]
fn fdinfo_accepts_legacy_non_thread_flags_and_line_ending_quirks() {
    let flags = u32::try_from(libc::O_RDWR | libc::O_CLOEXEC).unwrap();
    for flags in [0, flags, flags | libc::O_NONBLOCK as u32, flags | (1 << 30)] {
        let record = format!("pos:\t0\nflags:\t0{flags:o}\nPid:\t1234\nNSpid:\t9\n");
        assert_eq!(parse_pidfd_fdinfo(&record).unwrap(), 1234);
    }
    assert_eq!(
        parse_pidfd_fdinfo("flags:\t00\r\nPid:\t4294967295").unwrap(),
        u32::MAX
    );
    assert_eq!(
        parse_pidfd_fdinfo("unrelated\nPid:\t1\nflags:\t00\n").unwrap(),
        1
    );
}

#[test]
fn fdinfo_error_text_and_adjacent_fault_precedence_are_frozen() {
    for (record, message) in [
        (
            "",
            "descriptor is not a pidfd with a procfs Pid identity field",
        ),
        (
            "flags:\t00\n",
            "descriptor is not a pidfd with a procfs Pid identity field",
        ),
        (
            "Pid:\t1\n",
            "descriptor has no exact procfs octal flags identity field",
        ),
        (
            "Pid:\t-1\n",
            "descriptor has no exact procfs octal flags identity field",
        ),
        (
            "Pid: 1\nflags:\t00\n",
            "procfs client pidfd identity record has a malformed Pid field",
        ),
        (
            "Pid:\t1\nPid: 2\nflags:\t00\n",
            "procfs client pidfd identity record has a malformed Pid field",
        ),
        (
            "Pid:\t1\nPid:\tx\n",
            "procfs client pidfd identity record has duplicate Pid fields",
        ),
        (
            "Pid:\t+1\nflags:\t00\n",
            "procfs client pidfd identity record has a non-canonical decimal Pid field",
        ),
        (
            "Pid:\t01\nflags:\t00\n",
            "procfs client pidfd identity record has a non-canonical decimal Pid field",
        ),
        (
            "Pid:\t-2\nflags:\t00\n",
            "procfs client pidfd identity record has a non-canonical decimal Pid field",
        ),
        (
            "Pid:\t1\0\nflags:\t00\n",
            "procfs client pidfd identity record has a non-canonical decimal Pid field",
        ),
        (
            "Pid:\t9223372036854775808\n",
            "procfs client pidfd identity record has a malformed Pid field",
        ),
        (
            "Pid:\t0\nflags:\t00\n",
            "procfs client pidfd identity is not positive in the selected procfs namespace view",
        ),
        (
            "Pid:\t4294967296\nflags:\t00\n",
            "procfs client pidfd identity is not positive in the selected procfs namespace view",
        ),
        (
            "flags: 00\nPid:\t1\n",
            "procfs client pidfd identity record has a malformed flags field",
        ),
        (
            "flags:\t00\nflags: 00\nPid:\t1\n",
            "procfs client pidfd identity record has a malformed flags field",
        ),
        (
            "flags:\t00\nflags:\t09\nPid:\t1\n",
            "procfs client pidfd identity record has duplicate flags fields",
        ),
        (
            "flags:\t0\nPid:\t1\n",
            "procfs client pidfd identity record has a malformed octal flags field",
        ),
        (
            "Pid:\t-1\nflags:\t08\n",
            "procfs client pidfd identity record has a malformed octal flags field",
        ),
        (
            "Pid:\t-1\nflags:\t077777777777\n",
            "procfs client pidfd identity record has an out-of-range octal flags field",
        ),
    ] {
        assert_refused(
            parse_pidfd_fdinfo(record),
            Kind::InspectClientPidfd,
            message,
            None,
        );
    }
    assert_refused(
        parse_pidfd_fdinfo("flags:\t00\nPid:\t-1\n"),
        Kind::ClientAlreadyDead,
        "client pidfd target was already reaped",
        None,
    );
    let threaded_dead = format!("Pid:\t-1\nflags:\t0{:o}\n", libc::PIDFD_THREAD);
    assert_refused(
        parse_pidfd_fdinfo(&threaded_dead),
        Kind::ClientPidfdThread,
        "procfs client pidfd flags contain Linux PIDFD_THREAD (O_EXCL)",
        None,
    );
}

fn stat_record(pid: &str, start: &str) -> Vec<u8> {
    let mut bytes = format!("{pid} (").into_bytes();
    bytes.extend_from_slice(b"opaque\xff command ) with delimiters) ");
    for _ in 0..19 {
        bytes.extend_from_slice(b"R\t");
    }
    bytes.extend_from_slice(start.as_bytes());
    bytes.push(b'\n');
    bytes
}

#[test]
fn stat_parser_preserves_opaque_command_and_unvalidated_intermediate_fields() {
    assert_eq!(
        parse_process_start_time_ticks(&stat_record("17", "123"), 17).unwrap(),
        123
    );
    assert_eq!(
        parse_process_start_time_ticks(
            &stat_record("4294967295", "18446744073709551615"),
            u32::MAX
        )
        .unwrap(),
        u64::MAX
    );
    // The surrounding nominal PID admission rejects zero, not this comparison leaf.
    assert_eq!(
        parse_process_start_time_ticks(&stat_record("0", "1"), 0).unwrap(),
        1
    );
}

#[test]
fn stat_parser_error_text_and_command_pid_start_time_precedence_are_frozen() {
    for (contents, expected) in [
        (
            &b""[..],
            "client procfs stat identity has no command terminator",
        ),
        (
            &b")"[..],
            "client procfs stat identity has no PID terminator",
        ),
        (
            &b"2 x)"[..],
            "client procfs stat identity has a malformed command field",
        ),
        (
            &b"01 (command)"[..],
            "client procfs stat identity has a noncanonical PID",
        ),
        (
            &b"2 (command)"[..],
            "client procfs stat PID does not match the retained pidfd target",
        ),
        (
            &b"1 (command)"[..],
            "client procfs stat identity has no start-time field",
        ),
    ] {
        assert_refused(
            parse_process_start_time_ticks(contents, 1),
            Kind::InspectClientStartTime,
            expected,
            None,
        );
    }
    for start in ["01", "-1", "+1", "x"] {
        assert_refused(
            parse_process_start_time_ticks(&stat_record("1", start), 1),
            Kind::InspectClientStartTime,
            "client procfs stat identity has a noncanonical start time",
            None,
        );
    }
    for start in ["0", "18446744073709551616"] {
        assert_refused(
            parse_process_start_time_ticks(&stat_record("1", start), 1),
            Kind::InspectClientStartTime,
            "client procfs stat identity has an invalid start time",
            None,
        );
    }
    assert_refused(
        parse_process_start_time_ticks(&stat_record("01", "0"), 1),
        Kind::InspectClientStartTime,
        "client procfs stat identity has a noncanonical PID",
        None,
    );
    assert_refused(
        parse_process_start_time_ticks(&stat_record("4294967296", "0"), 1),
        Kind::InspectClientStartTime,
        "client procfs stat PID does not match the retained pidfd target",
        None,
    );
}

fn socket_pair(socket_type: SocketType) -> (OwnedFd, OwnedFd) {
    rustix::net::socketpair(
        AddressFamily::UNIX,
        socket_type,
        rustix::net::SocketFlags::CLOEXEC | rustix::net::SocketFlags::NONBLOCK,
        None,
    )
    .unwrap()
}

#[test]
fn socket_leaves_preserve_shape_credentials_and_exact_anchor_status() {
    let (peer, _other) = socket_pair(SocketType::SEQPACKET);
    let object = inspect_object(&peer, Kind::InspectPeer, "peer").unwrap();
    assert_eq!(validate_peer_shape(&peer).unwrap(), object);
    require_close_on_exec(&peer, Kind::PeerCloseOnExec, "service peer").unwrap();
    validate_external_anchor_peer_status(&peer).unwrap();
    let credentials = inspect_peer_credentials(&peer).unwrap();
    assert_eq!(credentials.pid, std::process::id());
    assert_eq!(credentials.uid, rustix::process::geteuid().as_raw());
    assert_eq!(credentials.gid, rustix::process::getegid().as_raw());

    rustix::fs::fcntl_setfl(&peer, OFlags::RDWR).unwrap();
    assert_refused(
        validate_external_anchor_peer_status(&peer),
        Kind::PeerStatusFlags,
        "external-anchor peer is not an exact nonblocking read-write endpoint",
        None,
    );
    let (stream, _other) = socket_pair(SocketType::STREAM);
    assert_refused(
        validate_peer_shape(&stream),
        Kind::PeerSocketType,
        "retained service peer is not SOCK_SEQPACKET",
        None,
    );
}

#[test]
fn syscall_errors_and_remote_address_override_keep_legacy_sources() {
    let ordinary: OwnedFd = File::open("/dev/null").unwrap().into();
    assert_refused(
        validate_peer_shape(&ordinary),
        Kind::PeerDomain,
        "retained service peer is not a socket with an inspectable domain",
        Some(libc::ENOTSOCK),
    );
    assert_refused(
        inspect_peer_credentials(&ordinary),
        Kind::InspectPeer,
        "cannot inspect retained peer SO_PEERCRED",
        Some(libc::ENOTSOCK),
    );
    assert_refused(
        require_unnamed_unix_address(
            &ordinary,
            UnixAddressSideV1::Remote,
            Kind::PeerRemoteAddress,
            "remote",
        ),
        Kind::PeerNotConnected,
        "cannot inspect retained service peer remote address",
        Some(libc::ENOTSOCK),
    );
    assert_refused(
        require_unnamed_unix_address(
            &ordinary,
            UnixAddressSideV1::Local,
            Kind::PeerLocalAddress,
            "local",
        ),
        Kind::PeerLocalAddress,
        "cannot inspect retained service peer local address",
        Some(libc::ENOTSOCK),
    );
}

#[test]
fn generic_cloexec_and_procfs_leaves_preserve_their_label_prefixes() {
    let descriptor = File::open("/dev/null").unwrap();
    require_close_on_exec(&descriptor, Kind::PeerCloseOnExec, "service peer").unwrap();
    rustix::io::fcntl_setfd(&descriptor, rustix::io::FdFlags::empty()).unwrap();
    let rejected = require_close_on_exec(&descriptor, Kind::PeerCloseOnExec, "service peer");
    rustix::io::fcntl_setfd(&descriptor, rustix::io::FdFlags::CLOEXEC).unwrap();
    assert_refused(
        rejected,
        Kind::PeerCloseOnExec,
        "retained service peer descriptor does not have FD_CLOEXEC",
        None,
    );
    assert_refused(
        require_procfs(&descriptor, "test identity record"),
        Kind::InspectClientPidfd,
        "test identity record is not backed by procfs",
        None,
    );
}

#[test]
fn distinctness_compares_object_identity_not_mutable_metadata() {
    let peer = ObjectIdentityV1 {
        device: 7,
        inode: 9,
        mode: libc::S_IFSOCK,
        uid: 11,
        gid: 13,
        links: 1,
    };
    assert_refused(
        require_distinct_peer_and_pidfd(peer, ObjectIdentityV1 { uid: 12, ..peer }),
        Kind::DuplicateDescriptors,
        "external-anchor peer and pidfd resolve to the same object",
        None,
    );
    assert!(require_distinct_peer_and_pidfd(peer, ObjectIdentityV1 { inode: 10, ..peer }).is_ok());
}
