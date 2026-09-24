use super::*;

fn assert_error(actual: Error, expected: Error) {
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
}

fn reader<'a>(
    source: &'a [u8],
    fragment: usize,
    calls: &'a mut usize,
) -> impl FnMut(&mut [u8]) -> rustix::io::Result<usize> + 'a {
    assert!(fragment != 0);
    let mut offset = 0;
    move |destination| {
        *calls += 1;
        assert!(!destination.is_empty());
        let count = (source.len() - offset).min(destination.len()).min(fragment);
        destination[..count].copy_from_slice(&source[offset..offset + count]);
        offset += count;
        Ok(count)
    }
}

#[test]
fn progress_reader_covers_empty_exact_limit_oversize_and_all_fragment_sizes() {
    for length in [0, 1, 4095, 4096, 4097, 4098] {
        for fragment in [1, 2, 7, 4096, 4097] {
            let source = vec![b'x'; length];
            let mut buffer = [0; RECORD_BYTES];
            let mut calls = 0;
            let observed = read_record_with(
                &mut buffer,
                Kind::InspectClientPidfd,
                "test record read",
                reader(&source, fragment, &mut calls),
            )
            .unwrap();
            let expected = length.min(RECORD_BYTES);
            assert_eq!(observed, expected);
            assert_eq!(&buffer[..observed], &source[..observed]);
            let expected_calls = expected.div_ceil(fragment) + usize::from(length < RECORD_BYTES);
            assert_eq!(
                calls, expected_calls,
                "length={length}, fragment={fragment}"
            );
            assert!(calls <= MAX_READ_CALLS);
            if fragment == 1 && length >= 4096 {
                assert_eq!(calls, 4097);
            }
        }
    }
}

#[test]
fn positive_short_reads_continue_until_eof_not_until_a_short_chunk() {
    let mut buffer = [0; RECORD_BYTES];
    let mut calls = 0;
    let result = read_record_with(
        &mut buffer,
        Kind::InspectClientPidfd,
        "test fragmented record",
        |bytes| {
            calls += 1;
            match calls {
                1 => {
                    bytes[0] = b'a';
                    Ok(1)
                }
                2 => {
                    bytes[..2].copy_from_slice(b"bc");
                    Ok(2)
                }
                3 => {
                    bytes[0] = b'd';
                    Ok(1)
                }
                4 => Ok(0),
                _ => panic!("read past EOF"),
            }
        },
    )
    .unwrap();
    assert_eq!(result, 4);
    assert_eq!(&buffer[..result], b"abcd");
    assert_eq!(calls, 4);
}

#[test]
fn read_errors_after_partial_progress_are_returned_without_retry() {
    for errno in [Errno::INTR, Errno::IO, Errno::AGAIN] {
        for partial in [false, true] {
            let mut buffer = [0; RECORD_BYTES];
            let mut calls = 0;
            let result = read_record_with(
                &mut buffer,
                Kind::InspectClientStartTime,
                "test native read error",
                |bytes| {
                    calls += 1;
                    if partial && calls == 1 {
                        bytes[..3].copy_from_slice(b"abc");
                        return Ok(3);
                    }
                    Err(errno)
                },
            );
            assert_error(
                result.unwrap_err(),
                Error::io(
                    Kind::InspectClientStartTime,
                    "test native read error",
                    errno,
                ),
            );
            assert_eq!(calls, 1 + usize::from(partial));
            if partial {
                assert_eq!(&buffer[..3], b"abc");
            }
        }
    }
}

#[test]
fn injected_impossible_read_count_is_rejected_without_indexing_past_buffer() {
    let mut buffer = [0; RECORD_BYTES];
    let mut calls = 0;
    let result = read_record_with(
        &mut buffer,
        Kind::InspectClientPidfd,
        "test invalid read count",
        |bytes| {
            calls += 1;
            Ok(bytes.len() + 1)
        },
    );
    assert_error(
        result.unwrap_err(),
        Error::new(
            Kind::InspectClientPidfd,
            "procfs read returned an invalid byte count",
        ),
    );
    assert_eq!(calls, 1);
}

#[test]
fn fdinfo_uses_shared_parser_after_utf8_and_length_checks() {
    let valid = b"pos:\t0\nflags:\t02000002\nPid:\t1234\nNSpid:\t1234\n";
    for length in [valid.len(), 4096, 4097] {
        let mut bytes = vec![b'\n'; length];
        bytes[..valid.len()].copy_from_slice(valid);
        for fragment in [1, 17, RECORD_BYTES] {
            let mut calls = 0;
            let result = read_pidfd_record_with(reader(&bytes, fragment, &mut calls));
            if length <= 4096 {
                assert_eq!(result.unwrap(), 1234);
            } else {
                assert_error(
                    result.unwrap_err(),
                    Error::new(
                        Kind::InspectClientPidfd,
                        "procfs client pidfd identity record exceeds 4096 bytes",
                    ),
                );
            }
            assert!(calls <= MAX_READ_CALLS);
        }
    }
    for bytes in [
        &b""[..],
        &b"Pid:\t1234\n"[..],
        &b"flags:\t02000002\nPid:\t1234\nPid:\t1234\n"[..],
        &b"flags:\t02000002\nPid:\t-1\n"[..],
        &b"flags:\t02000202\nPid:\t1234\n"[..],
    ] {
        let mut calls = 0;
        let actual = read_pidfd_record_with(reader(bytes, 3, &mut calls)).unwrap_err();
        let expected = checks::parse_pidfd_fdinfo(std::str::from_utf8(bytes).unwrap()).unwrap_err();
        assert_error(actual, expected);
    }
}

#[test]
fn invalid_utf8_precedes_fdinfo_oversize_but_io_error_precedes_utf8() {
    for length in [1, 4096, 4097] {
        let mut bytes = vec![b'\n'; length];
        bytes[0] = 0xff;
        let mut calls = 0;
        assert_error(
            read_pidfd_record_with(reader(&bytes, 1, &mut calls)).unwrap_err(),
            Error::new(
                Kind::InspectClientPidfd,
                "procfs client pidfd identity record is not valid UTF-8",
            ),
        );
        assert!(calls <= MAX_READ_CALLS);
    }
    let mut calls = 0;
    let result = read_pidfd_record_with(|bytes| {
        calls += 1;
        if calls == 1 {
            bytes[0] = 0xff;
            Ok(1)
        } else {
            Err(Errno::INTR)
        }
    });
    assert_error(
        result.unwrap_err(),
        Error::io(
            Kind::InspectClientPidfd,
            "cannot read the bounded procfs client pidfd identity record",
            Errno::INTR,
        ),
    );
    assert_eq!(calls, 2);
}

#[test]
fn stat_uses_shared_parser_and_preserves_binary_command_fields() {
    let mut valid = b"1234 (command with ) and \xff) ".to_vec();
    for _ in 0..19 {
        valid.extend_from_slice(b"R ");
    }
    valid.extend_from_slice(b"789\n");
    assert_eq!(
        checks::parse_process_start_time_ticks(&valid, 1234).unwrap(),
        789,
    );
    for length in [valid.len(), 4096, 4097] {
        let mut bytes = vec![b' '; length];
        bytes[..valid.len()].copy_from_slice(&valid);
        let mut calls = 0;
        let result = read_stat_record_with(1234, reader(&bytes, 1, &mut calls));
        if length <= 4096 {
            assert_eq!(result.unwrap(), 789);
        } else {
            assert_error(
                result.unwrap_err(),
                Error::new(
                    Kind::InspectClientStartTime,
                    "client procfs stat identity is empty or exceeds 4096 bytes",
                ),
            );
        }
        assert!(calls <= MAX_READ_CALLS);
    }
    let mut calls = 0;
    assert_error(
        read_stat_record_with(1234, reader(b"", 1, &mut calls)).unwrap_err(),
        Error::new(
            Kind::InspectClientStartTime,
            "client procfs stat identity is empty or exceeds 4096 bytes",
        ),
    );
    for (bytes, pid) in [(&valid[..], 1235), (&valid[..valid.len() - 4], 1234)] {
        let mut calls = 0;
        assert_error(
            read_stat_record_with(pid, reader(bytes, 7, &mut calls)).unwrap_err(),
            checks::parse_process_start_time_ticks(bytes, pid).unwrap_err(),
        );
    }
}

#[test]
fn decimal_paths_cover_zero_digit_transitions_and_maximum_identifiers() {
    for (value, expected) in [
        (0, "0"),
        (9, "9"),
        (10, "10"),
        (99, "99"),
        (100, "100"),
        (i32::MAX as u32, "2147483647"),
        (u32::MAX, "4294967295"),
    ] {
        for (prefix, suffix) in [(&b""[..], &b""[..]), (&b"/proc/"[..], &b"/stat"[..])] {
            let mut bytes = [0xa5; PATH_BYTES];
            let path = decimal_path(prefix, value, suffix, &mut bytes).unwrap();
            assert_eq!(
                path.to_bytes(),
                [prefix, expected.as_bytes(), suffix].concat(),
            );
            assert_eq!(path.to_bytes_with_nul().last(), Some(&0));
        }
    }
    let mut bytes = [0; PATH_BYTES];
    assert_eq!(
        decimal_path(&[b'x'; 30], 0, b"", &mut bytes)
            .unwrap()
            .to_bytes()
            .len(),
        31,
    );
    assert!(decimal_path(&[b'x'; 31], 0, b"", &mut bytes).is_err());
    assert!(decimal_path(b"bad\0", 1, b"", &mut bytes).is_err());
}

#[test]
fn poll_primitive_returns_raw_observation_and_never_retries_errors() {
    let file = File::open("/dev/null").unwrap();
    let descriptor: OwnedFd = file.into();
    for (ready, revents) in [
        (0, 0),
        (1, libc::POLLIN),
        (1, libc::POLLHUP),
        (1, libc::POLLERR | libc::POLLNVAL),
    ] {
        let mut calls = 0;
        let observed = poll_once_with(&descriptor, |pollfd| {
            calls += 1;
            assert_eq!(pollfd.fd, descriptor.as_raw_fd());
            assert_eq!(pollfd.events, libc::POLLIN);
            assert_eq!(pollfd.revents, 0);
            pollfd.revents = revents;
            Ok(ready)
        })
        .unwrap();
        assert_eq!(observed, (ready, revents));
        assert_eq!(calls, 1);
    }
    for errno in [Errno::INTR, Errno::IO] {
        let mut calls = 0;
        assert_error(
            poll_once_with(&descriptor, |_| {
                calls += 1;
                Err(errno)
            })
            .unwrap_err(),
            Error::io(
                Kind::InspectClientPidfd,
                "cannot poll client pidfd for liveness",
                errno,
            ),
        );
        assert_eq!(calls, 1);
    }
}

#[test]
fn real_procfs_observations_match_legacy_without_changing_borrowed_pidfd() {
    let pid = rustix::process::getpid();
    let pidfd = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).unwrap();
    let before = rustix::fs::fstat(&pidfd).unwrap();
    let flags = rustix::io::fcntl_getfd(&pidfd).unwrap();
    let legacy = super::super::inspect_pidfd_target_from_procfs(&pidfd).unwrap();
    let native = inspect_pidfd_target_from_procfs(&pidfd).unwrap();
    assert_eq!(native, legacy);
    assert_eq!(native.pid, std::process::id());
    assert_eq!(native.source, PidfdIdentitySourceV1::ProcfsFdinfo);
    let legacy_start = super::super::inspect_process_start_time_ticks(native.pid).unwrap();
    assert_eq!(
        inspect_process_start_time_ticks(native.pid).unwrap(),
        legacy_start,
    );
    assert!(legacy_start != 0);
    assert_eq!(poll_once(&pidfd).unwrap(), (0, 0));
    assert_eq!(
        poll_once(&pidfd).unwrap(),
        super::super::poll_legacy(&pidfd).unwrap(),
    );
    let after = rustix::fs::fstat(&pidfd).unwrap();
    assert_eq!((before.st_dev, before.st_ino), (after.st_dev, after.st_ino));
    assert_eq!(rustix::io::fcntl_getfd(&pidfd).unwrap(), flags);
}
