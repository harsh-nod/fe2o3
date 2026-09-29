use super::*;

impl NativeUserNamespaceV1 {
    /// Inert pipe custody for cleanup tests, never an admitted namespace owner.
    /// Both mapping entry points refuse before inspecting these descriptors.
    pub(crate) fn poisoned_fixture_for_cleanup(fd: OwnedFd) -> Self {
        let parent = std::array::from_fn(|_| Namespace {
            fd: fd.try_clone().unwrap(),
            identity: Identity {
                device: 0,
                inode: 0,
            },
        });
        Self {
            origin: rustix::process::getpid(),
            proc_root: fd,
            parent,
            uid_map: IdentityMap::new(1000, 1001).unwrap(),
            gid_map: IdentityMap::new(1000, 1001).unwrap(),
            phase: Phase::Refused,
            child: None,
        }
    }
}

#[test]
fn identity_maps_sort_and_deduplicate_uid_and_gid_independently() {
    let helper = Credentials::new(2100, 3200).unwrap();
    let peer = Credentials::new(1100, 3200).unwrap();
    let uid = IdentityMap::new(helper.uid(), peer.uid()).unwrap();
    let gid = IdentityMap::new(helper.gid(), peer.gid()).unwrap();
    assert_eq!(
        uid,
        IdentityMap {
            ids: [0, 1100, 2100],
            len: 3
        }
    );
    assert_eq!(
        gid,
        IdentityMap {
            ids: [0, 3200, 0],
            len: 2
        }
    );
    assert_eq!(uid, IdentityMap::new(peer.uid(), helper.uid()).unwrap());
    for (helper, peer) in [(0, 1), (1, 0), (u32::MAX, 1), (1, u32::MAX)] {
        assert!(IdentityMap::new(helper, peer).is_err());
    }
}

#[test]
fn identity_map_encoding_is_fixed_bounded_and_exact() {
    for map in [
        IdentityMap::new(1, 1).unwrap(),
        IdentityMap::new(u32::MAX - 1, u32::MAX - 2).unwrap(),
    ] {
        let mut bytes = [0; MAP_BYTES];
        let count = map.encode(&mut bytes).unwrap();
        assert!(count <= MAP_BYTES);
        map.require_record(&bytes[..count]).unwrap();
    }
    let mut bytes = [0; MAP_BYTES];
    let count = IdentityMap::new(42, 42)
        .unwrap()
        .encode(&mut bytes)
        .unwrap();
    assert_eq!(&bytes[..count], b"0 0 1\n42 42 1\n");
    IdentityMap::new(10, 20).unwrap().require_record(
        b"         0          0          1\n        10         10          1\n        20         20          1\n",
    ).unwrap();
}

#[test]
fn identity_map_readback_rejects_nonexact_or_ambiguous_rows() {
    let expected = IdentityMap::new(10, 20).unwrap();
    for bytes in [
        &b""[..],
        b"\n",
        b"0 0 1\n10 10 1\n20 20 1",
        b"0 0 1\n10 10 1\n",
        b"0 0 1\n10 10 1\n20 20 1\n30 30 1\n",
        b"0 0 1\n10 10 1\n10 10 1\n",
        b"0 0 1\n20 20 1\n10 10 1\n",
        b"0 0 1\n10 11 1\n20 20 1\n",
        b"0 0 1\n10 10 2\n20 20 1\n",
        b"0 0 1\n10 10 0\n20 20 1\n",
        b"0 0 1\n10 10\n20 20 1\n",
        b"0 0 1\n10 10 1 extra\n20 20 1\n",
        b"0 0 1\n010 10 1\n20 20 1\n",
        b"0 0 1\n+10 10 1\n20 20 1\n",
        b"0 0 1\n-10 10 1\n20 20 1\n",
        b"0 0 1\n18446744073709551616 10 1\n20 20 1\n",
        b"0 0 1\n10 10 1\r\n20 20 1\n",
        b"0 0 1\n\n10 10 1\n20 20 1\n",
        b"0 0 1\n10\0 10 1\n20 20 1\n",
    ] {
        assert!(
            expected.require_record(bytes).is_err(),
            "accepted {bytes:?}"
        );
    }
    let oversized = [b' '; READ_BYTES];
    assert!(expected.require_record(&oversized).is_err());
}

#[test]
fn proc_numeric_components_are_single_bounded_c_strings() {
    for (value, expected) in [(0, c"0"), (1, c"1"), (u32::MAX, c"4294967295")] {
        let mut bytes = [0xff; 11];
        assert_eq!(number_name(value, &mut bytes).unwrap(), expected);
    }
}

#[test]
fn pidfd_records_require_exact_process_pid_and_unique_fields() {
    let valid = b"pos:\t0\nflags:\t02000002\nmnt_id:\t15\nino:\t1066\nPid:\t123\nNSpid:\t123\n";
    require_pidfd_record(valid, 123).unwrap();
    assert!(require_pidfd_record(valid, 124).is_err());
    for bytes in [
        &b""[..],
        b"Pid:\t123\n",
        b"flags:\t02000002\n",
        b"Pid:\t123\nPid:\t123\nflags:\t02000002\n",
        b"Pid:\t123\nflags:\t02000002\nflags:\t02000002\n",
        b"Pid:\t123 124\nflags:\t02000002\n",
        b"Pid:\t123\nflags:\t02000002 extra\n",
        b"Pid:\t-1\nflags:\t02000002\n",
        b"Pid:\t0123\nflags:\t02000002\n",
        b"Pid:\t+123\nflags:\t02000002\n",
        b"Pid:\t123\nflags:\t02000008\n",
        b"Pid:\t123\nflags:\t02000202\n", // PIDFD_THREAD / O_EXCL.
        b"Pid:\t123\nflags:\t02000002",
        b"Pid:\t123\nflags:\t02000002\nnot-a-field\n",
        b"Pid:\t18446744073709551616\nflags:\t02000002\n",
    ] {
        assert!(
            require_pidfd_record(bytes, 123).is_err(),
            "accepted {bytes:?}"
        );
    }
    assert!(require_pidfd_record(b"Pid:\t0\nflags:\t02000002\n", 0).is_err());
    assert!(require_pidfd_record(b"Pid:\t2147483648\nflags:\t02000002\n", 2147483648).is_err());
}

#[test]
fn parser_radices_and_record_bounds_are_explicit() {
    assert_eq!(decimal(b"18446744073709551615", 10).unwrap(), u64::MAX);
    assert_eq!(decimal(b"02000002", 8).unwrap(), 0o2000002);
    for (bytes, radix) in [(&b""[..], 10), (b"00", 10), (b"8", 8), (b"1", 16)] {
        assert!(decimal(bytes, radix).is_err());
    }
    let mut boundary = [b'x'; READ_BYTES];
    boundary[READ_BYTES - 2] = b'\n';
    assert!(record_lines(&boundary[..READ_BYTES - 1]).is_ok());
    boundary[READ_BYTES - 1] = b'\n';
    assert!(record_lines(&boundary).is_err());
}

#[test]
fn configuration_schedule_is_single_use_even_after_partial_failure() {
    let mut phase = Phase::Prepared;
    assert!(phase.require_configured().is_err());
    phase.begin().unwrap();
    assert_eq!(phase, Phase::Configuring);
    assert!(phase.begin().is_err());
    assert!(phase.require_configured().is_err());
    for mut phase in [Phase::Configured, Phase::Refused] {
        let before = phase;
        assert!(phase.begin().is_err());
        assert_eq!(phase, before);
    }
    assert!(Phase::Configured.require_configured().is_ok());
    assert!(Phase::Refused.require_configured().is_err());
}

#[test]
fn poisoned_cleanup_fixture_refuses_without_namespace_io_and_retains_pipe() {
    let (reader, writer) = rustix::pipe::pipe_with(
        rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )
    .unwrap();
    let mut owner = NativeUserNamespaceV1::poisoned_fixture_for_cleanup(writer);
    let pid = rustix::process::getpid();
    assert!(matches!(
        owner.configure_child(pid, &reader),
        Err(Error::State("user namespace configuration is single-use"))
    ));
    assert!(matches!(
        owner.revalidate_child(pid, &reader),
        Err(Error::State("user namespace configuration is incomplete"))
    ));
    assert_eq!(owner.phase, Phase::Refused);
    assert!(owner.child.is_none());
    let mut byte = [0];
    assert_eq!(rustix::io::read(&reader, &mut byte), Err(Errno::AGAIN));
    drop(owner);
    assert_eq!(rustix::io::read(&reader, &mut byte), Ok(0));
}

#[test]
fn logical_bounds_cover_retained_and_temporary_descriptor_frames() {
    assert_eq!(FD_COUNT, 1 + NAMESPACES.len() + 7);
    assert_eq!(
        NativeUserNamespaceV1::STORAGE,
        size_of::<NativeUserNamespaceV1>() + FD_COUNT * size_of::<usize>()
    );
    assert!(TEMP_FD_SCRATCH >= 32 * size_of::<OwnedFd>());
    let fixed = 4 * size_of::<NativeUserNamespaceV1>()
        + 4 * READ_BYTES
        + 16 * size_of::<Stat>()
        + TEMP_FD_SCRATCH;
    for scratch in [
        NativeUserNamespaceV1::PREPARE_SCRATCH,
        NativeUserNamespaceV1::CONFIGURE_SCRATCH,
        NativeUserNamespaceV1::REVALIDATE_SCRATCH,
    ] {
        assert!(scratch >= fixed);
    }
    assert!(NativeUserNamespaceV1::CONFIGURE_WORK > NativeUserNamespaceV1::REVALIDATE_WORK);
    assert!(NativeUserNamespaceV1::REVALIDATE_WORK > NativeUserNamespaceV1::PREPARE_WORK);
}
