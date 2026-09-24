use super::*;
use bounded_io::read_bounded;
use status::{parse_decimal, parse_four_decimal, parse_hex_u64, parse_octal};

const EXACT: &[u8] = b"Name:\ttest\nUmask:\t0077\nTracerPid:\t0\nUid:\t1000\t1000\t1000\t1000\nGid:\t1001\t1001\t1001\t1001\nGroups:\t\nCapInh:\t0000000000000000\nCapPrm:\t0000000000000000\nCapEff:\t0000000000000000\nCapBnd:\t0000000000000000\nCapAmb:\t0000000000000000\nNoNewPrivs:\t1\n";

fn credentials() -> ProtectedServiceCredentialProfileV1 {
    ProtectedServiceCredentialProfileV1::new(1000, 1001).unwrap()
}

fn require(bytes: &[u8]) -> Result<(), Error> {
    ProcStatusProfile::parse(bytes)?.require(credentials())
}

fn replace(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let offset = bytes
        .windows(from.len())
        .position(|part| part == from)
        .unwrap();
    let mut replaced = Vec::with_capacity(bytes.len() - from.len() + to.len());
    replaced.extend_from_slice(&bytes[..offset]);
    replaced.extend_from_slice(to);
    replaced.extend_from_slice(&bytes[offset + from.len()..]);
    replaced
}

#[test]
fn credential_profile_rejects_root_and_sentinel_identities() {
    use crate::ProtectedServiceCredentialProfileErrorV1::{InvalidGid, InvalidUid};
    for (uid, gid, error) in [
        (0, 1, InvalidUid),
        (u32::MAX, 1, InvalidUid),
        (1, 0, InvalidGid),
        (1, u32::MAX, InvalidGid),
        (0, 0, InvalidUid),
    ] {
        assert_eq!(
            ProtectedServiceCredentialProfileV1::new(uid, gid),
            Err(error)
        );
    }
    assert_eq!(credentials().uid(), 1000);
    assert_eq!(credentials().gid(), 1001);
    assert_eq!(
        credentials().securebits(),
        crate::PROTECTED_SERVICE_SECUREBITS_V1
    );
}

#[test]
fn exact_status_and_legacy_whitespace_are_accepted() {
    require(EXACT).unwrap();
    let padded = replace(
        EXACT,
        b"NoNewPrivs:\t1",
        "NoNewPrivs:\u{2003}1\u{2003}".as_bytes(),
    );
    require(&padded).unwrap();
    require(&[EXACT, b"ignored line\nUnknown: ignored\nUnknown: ignored\n"].concat()).unwrap();
    let crlf = std::str::from_utf8(EXACT).unwrap().replace('\n', "\r\n");
    require(crlf.as_bytes()).unwrap();
}

#[test]
fn all_missing_and_duplicate_security_fields_keep_their_errors() {
    for (field, reason) in [
        ("Uid", "proc status lacks Uid"),
        ("Gid", "proc status lacks Gid"),
        ("Groups", "proc status lacks Groups"),
        ("CapInh", "proc status lacks a capability set"),
        ("CapPrm", "proc status lacks a capability set"),
        ("CapEff", "proc status lacks a capability set"),
        ("CapBnd", "proc status lacks a capability set"),
        ("CapAmb", "proc status lacks a capability set"),
        ("NoNewPrivs", "proc status lacks NoNewPrivs"),
        ("TracerPid", "proc status lacks TracerPid"),
        ("Umask", "proc status lacks Umask"),
    ] {
        let line = std::str::from_utf8(EXACT)
            .unwrap()
            .lines()
            .find(|line| line.split_once(':').is_some_and(|(name, _)| name == field))
            .unwrap();
        let missing = replace(EXACT, line.as_bytes(), b"");
        assert_eq!(
            require(&missing),
            Err(Error::ProcessProfile(reason)),
            "{field}"
        );
        let duplicate = [EXACT, line.as_bytes(), b"\n"].concat();
        assert_eq!(
            require(&duplicate),
            Err(Error::ProcessProfile(
                "proc status duplicates a security field"
            )),
            "{field}"
        );
    }
}

#[test]
fn missing_fields_follow_original_presence_order() {
    let mut bytes = Vec::new();
    for (line, reason) in [
        (
            b"Uid: 1000 1000 1000 1000\n".as_slice(),
            "proc status lacks Uid",
        ),
        (b"Gid: 1001 1001 1001 1001\n", "proc status lacks Gid"),
        (b"Groups:\n", "proc status lacks Groups"),
        (
            b"CapInh: 0\nCapPrm: 0\nCapEff: 0\nCapBnd: 0\nCapAmb: 0\n",
            "proc status lacks a capability set",
        ),
        (b"NoNewPrivs: 1\n", "proc status lacks NoNewPrivs"),
        (b"TracerPid: 0\n", "proc status lacks TracerPid"),
        (b"Umask: 0077\n", "proc status lacks Umask"),
    ] {
        assert_eq!(require(&bytes), Err(Error::ProcessProfile(reason)));
        bytes.extend_from_slice(line);
    }
    require(&bytes).unwrap();
}

#[test]
fn missing_capability_is_not_hidden_by_an_earlier_nonzero_set() {
    let bytes = replace(EXACT, b"CapInh:\t0000000000000000", b"CapInh: 1");
    let bytes = replace(&bytes, b"CapAmb:\t0000000000000000\n", b"");
    assert_eq!(
        require(&bytes),
        Err(Error::ProcessProfile("proc status lacks a capability set"))
    );
}

#[test]
fn parser_reports_malformed_duplicate_before_duplicate_or_missing_fields() {
    for (line, reason) in [
        (
            b"Uid: 1 2 3 4 5 nope\n".as_slice(),
            "proc decimal field is malformed",
        ),
        (b"Gid: 1 2 3 4 4294967296\n", "proc decimal field overflows"),
        (b"CapBnd: nope\n", "proc capability field is malformed"),
        (b"Umask: 0088\n", "proc umask field is malformed"),
    ] {
        assert_eq!(
            require(&[EXACT, line].concat()),
            Err(Error::ProcessProfile(reason))
        );
        assert_eq!(require(line), Err(Error::ProcessProfile(reason)));
    }
}

#[test]
fn decimal_extra_fields_are_validated_before_cardinality() {
    assert_eq!(parse_four_decimal("1 2 3 4"), Ok([1, 2, 3, 4]));
    for input in ["", "1 2 3", "1 2 3 4 5", "1 2 3 4 5 6 7"] {
        assert_eq!(
            parse_four_decimal(input),
            Err(Error::ProcessProfile(
                "proc identity does not have four fields"
            ))
        );
    }
    for input in ["1 2 3 4 x", "1 2 3 4 5 x", "1 2 3 4 5 6 x"] {
        assert_eq!(
            parse_four_decimal(input),
            Err(Error::ProcessProfile("proc decimal field is malformed"))
        );
    }
    assert_eq!(
        parse_four_decimal("1 2 3 4 5 4294967296"),
        Err(Error::ProcessProfile("proc decimal field overflows"))
    );
    let mut many = "0 ".repeat(MAX_PROC_STATUS_BYTES / 2 - 1);
    many.push('x');
    assert_eq!(
        parse_four_decimal(&many),
        Err(Error::ProcessProfile("proc decimal field is malformed"))
    );
}

#[test]
fn numeric_formats_and_overflows_keep_exact_errors() {
    for input in ["", "+1", "-1", "1x", "42949672960x", "1 2"] {
        assert_eq!(
            parse_decimal(input),
            Err(Error::ProcessProfile("proc decimal field is malformed"))
        );
    }
    assert_eq!(
        parse_decimal("4294967296"),
        Err(Error::ProcessProfile("proc decimal field overflows"))
    );
    assert_eq!(parse_decimal("4294967295"), Ok(u32::MAX));
    for input in ["", "+0", "0x00", "gg", "1 2", "10000000000000000x"] {
        assert_eq!(
            parse_hex_u64(input),
            Err(Error::ProcessProfile("proc capability field is malformed"))
        );
    }
    assert_eq!(
        parse_hex_u64("10000000000000000"),
        Err(Error::ProcessProfile("proc capability field overflows"))
    );
    assert_eq!(parse_hex_u64("fFfFfFfFfFfFfFfF"), Ok(u64::MAX));
    assert_eq!(parse_hex_u64("00000000000000000000"), Ok(0));
    for input in ["", "+77", "0088", "400000000008"] {
        assert_eq!(
            parse_octal(input),
            Err(Error::ProcessProfile("proc umask field is malformed"))
        );
    }
    assert_eq!(
        parse_octal("40000000000"),
        Err(Error::ProcessProfile("proc umask field overflows"))
    );
}

#[test]
fn profile_mismatches_follow_original_validation_order() {
    let cases: &[(&[u8], &[u8], &str)] = &[
        (
            b"Uid:\t1000\t1000\t1000\t1000",
            b"Uid: 1000 1000 1000 1002",
            "real, effective, saved, or filesystem UID differs",
        ),
        (
            b"Gid:\t1001\t1001\t1001\t1001",
            b"Gid: 1001 1001 1001 1002",
            "real, effective, saved, or filesystem GID differs",
        ),
        (
            b"Groups:\t\n",
            b"Groups: 1001\n",
            "supplementary group set is not empty",
        ),
        (
            b"CapEff:\t0000000000000000",
            b"CapEff: 1",
            "a capability set is not empty",
        ),
        (
            b"NoNewPrivs:\t1",
            b"NoNewPrivs: 0",
            "no_new_privs is not set",
        ),
        (
            b"TracerPid:\t0",
            b"TracerPid: 9",
            "service process is traced",
        ),
        (b"Umask:\t0077", b"Umask: 0022", "umask is not 077"),
    ];
    let mut bytes = EXACT.to_vec();
    for &(from, to, reason) in cases.iter().rev() {
        bytes = replace(&bytes, from, to);
        assert_eq!(require(&bytes), Err(Error::ProcessProfile(reason)));
    }
}

#[test]
fn each_capability_set_must_be_empty() {
    for name in ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"] {
        let from = format!("{name}:\t0000000000000000");
        for (value, reason) in [
            ("1", "a capability set is not empty"),
            ("nope", "proc capability field is malformed"),
            ("10000000000000000", "proc capability field overflows"),
        ] {
            let to = format!("{name}: {value}");
            assert_eq!(
                require(&replace(EXACT, from.as_bytes(), to.as_bytes())),
                Err(Error::ProcessProfile(reason))
            );
        }
    }
}

#[test]
fn status_limits_and_utf8_fail_closed() {
    let mut bytes = EXACT.to_vec();
    bytes.resize(MAX_PROC_STATUS_BYTES, b' ');
    require(&bytes).unwrap();
    bytes.push(b' ');
    assert_eq!(
        require(&bytes),
        Err(Error::ProcessProfile("proc status exceeds the fixed bound"))
    );
    assert_eq!(
        require(b"\xff"),
        Err(Error::ProcessProfile("proc status is not UTF-8"))
    );
}

#[test]
fn bounded_reader_accepts_short_reads_only_after_explicit_eof() {
    let mut bytes = [0; MAX_PROC_STATUS_BYTES + 1];
    let mut calls = 0;
    let len = read_bounded(&mut bytes, "read test", "too long", |buffer| {
        let count = usize::from(calls < MAX_PROC_STATUS_BYTES);
        calls += 1;
        if count != 0 {
            buffer[0] = b'x';
        }
        Ok(count)
    })
    .unwrap();
    assert_eq!(len, MAX_PROC_STATUS_BYTES);
    assert_eq!(calls, MAX_PROC_STATUS_BYTES + 1);
    assert!(bytes[..len].iter().all(|byte| *byte == b'x'));

    let mut offset = 0;
    let len = read_bounded(&mut bytes, "read test", "too long", |buffer| {
        let count = 3.min(EXACT.len() - offset);
        buffer[..count].copy_from_slice(&EXACT[offset..offset + count]);
        offset += count;
        Ok(count)
    })
    .unwrap();
    assert_eq!(&bytes[..len], EXACT);
    require(&bytes[..len]).unwrap();
}

#[test]
fn bounded_reader_rejects_overflow_without_waiting_for_eof() {
    for one_byte in [false, true] {
        let mut bytes = [0; MAX_PROC_STATUS_BYTES + 1];
        let mut calls = 0;
        let result = read_bounded(&mut bytes, "read test", "too long", |buffer| {
            calls += 1;
            Ok(if one_byte { 1 } else { buffer.len() })
        });
        assert_eq!(result, Err(Error::ProcessProfile("too long")));
        assert_eq!(
            calls,
            if one_byte {
                MAX_PROC_STATUS_BYTES + 1
            } else {
                1
            }
        );
    }
}

#[test]
fn bounded_reader_never_retries_errors_including_interruption() {
    for source in [Errno::INTR, Errno::AGAIN, Errno::IO] {
        for successful_reads in [0, 1, 8] {
            let mut bytes = [0; 9];
            let mut calls = 0;
            let result = read_bounded(&mut bytes, "read test", "too long", |_| {
                calls += 1;
                if calls <= successful_reads {
                    Ok(1)
                } else {
                    Err(source)
                }
            });
            assert_eq!(
                result,
                Err(Error::Io {
                    operation: "read test",
                    source
                })
            );
            assert_eq!(calls, successful_reads + 1);
        }
    }
}

#[test]
fn zero_progress_is_eof_and_never_spins_or_admits_missing_fields() {
    let mut bytes = [0; MAX_PROC_STATUS_BYTES + 1];
    let mut calls = 0;
    let len = read_bounded(&mut bytes, "read test", "too long", |_| {
        calls += 1;
        Ok(0)
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        require(&bytes[..len]),
        Err(Error::ProcessProfile("proc status lacks Uid"))
    );

    let mut bytes = [0; 0];
    assert_eq!(
        read_bounded(&mut bytes, "read test", "too long", |_| panic!(
            "empty buffer must not read"
        )),
        Err(Error::InvalidState("proc read has no sentinel capacity"))
    );
    let mut bytes = [0; 2];
    assert_eq!(
        read_bounded(&mut bytes, "read test", "too long", |_| Ok(3)),
        Err(Error::InvalidState(
            "proc read exceeded the supplied buffer"
        ))
    );
}

#[test]
fn cap_ceiling_numeric_errors_and_legacy_encoding_error_are_preserved() {
    for (bytes, value) in [(b"0\n".as_slice(), 0), (b"63\n", 63), (b" +40 \n", 40)] {
        assert_eq!(parse_cap_last_cap(bytes), Ok(value));
    }
    for bytes in [b"".as_slice(), b"-1", b"x", b"4294967296", b"40 41"] {
        assert_eq!(
            parse_cap_last_cap(bytes),
            Err(Error::ProcessProfile(
                "kernel capability ceiling is malformed"
            ))
        );
    }
    assert_eq!(
        parse_cap_last_cap(b"64\n"),
        Err(Error::ProcessProfile(
            "kernel capability ceiling exceeds the supported 64-bit set"
        ))
    );
    let error = parse_cap_last_cap(b"\xff").unwrap_err();
    assert_eq!(
        error,
        Error::Io {
            operation: "read kernel capability ceiling",
            source: Errno::ILSEQ
        }
    );
    let crate::ProtectedServiceProfileErrorV1::Io { operation, source } =
        crate::map_observation_error(error)
    else {
        panic!("legacy error changed")
    };
    assert_eq!(operation, "read kernel capability ceiling");
    assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(source.to_string(), "stream did not contain valid UTF-8");
}

#[test]
fn cap_ceiling_buffer_accepts_exact_limit_and_rejects_sentinel() {
    let mut bytes = [0; MAX_CAP_LAST_CAP_BYTES + 1];
    let mut calls = 0;
    let len = read_bounded(
        &mut bytes,
        "read kernel capability ceiling",
        "ceiling too long",
        |buffer| {
            calls += 1;
            if calls == 1 {
                buffer[..MAX_CAP_LAST_CAP_BYTES].fill(b' ');
                buffer[0] = b'4';
                buffer[1] = b'0';
                Ok(MAX_CAP_LAST_CAP_BYTES)
            } else {
                Ok(0)
            }
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(parse_cap_last_cap(&bytes[..len]), Ok(40));
    assert_eq!(
        read_bounded(
            &mut bytes,
            "read kernel capability ceiling",
            "ceiling too long",
            |buffer| Ok(buffer.len())
        ),
        Err(Error::ProcessProfile("ceiling too long"))
    );
}

#[test]
fn fixed_paths_include_full_pid_and_longest_namespace() {
    let pid = Pid::from_raw(i32::MAX).unwrap();
    assert_eq!(
        ProcPath::new(Some(pid), "ns/time_for_children")
            .unwrap()
            .as_c_str()
            .unwrap(),
        c"/proc/2147483647/ns/time_for_children"
    );
    assert_eq!(
        ProcPath::new(None, "status").unwrap().as_c_str().unwrap(),
        c"/proc/thread-self/status"
    );
    assert_eq!(CURRENT_STATUS_PATH, c"/proc/thread-self/status");
    assert_eq!(
        ProcPath::new(None, "ns/time_for_children")
            .unwrap()
            .as_c_str()
            .unwrap(),
        c"/proc/thread-self/ns/time_for_children"
    );
    assert_eq!(
        ProcPath::new(None, "sta\0tus").unwrap().as_c_str(),
        Err(Error::InvalidState("proc path contains an interior NUL"))
    );
    assert!(matches!(
        ProcPath::new(None, &"x".repeat(PROC_PATH_BYTES)),
        Err(Error::InvalidState("proc path exceeds the fixed bound"))
    ));
}

#[test]
fn namespace_snapshot_and_legacy_adapter_revalidate_without_drift() {
    let namespaces = NamespaceSet::capture_self().unwrap();
    namespaces.revalidate_self().unwrap();
    namespaces
        .revalidate_process(rustix::process::getpid())
        .unwrap();
    let legacy = crate::ProtectedServiceNamespaceSetV1::capture_self().unwrap();
    legacy.revalidate_self().unwrap();
    legacy
        .revalidate_process(rustix::process::getpid())
        .unwrap();
}

#[test]
fn namespace_child_mismatch_order_is_preserved() {
    let mut namespaces = NamespaceSet {
        identities: [NamespaceIdentity {
            device: 1,
            inode: 1,
        }; 10],
    };
    namespaces.identities[3].inode = 2;
    namespaces.identities[9].inode = 2;
    assert_eq!(
        namespaces.require_children_unchanged(),
        Err(Error::Namespace("pid-for-children"))
    );
    namespaces.identities[3].inode = 1;
    assert_eq!(
        namespaces.require_children_unchanged(),
        Err(Error::Namespace("time-for-children"))
    );
}

#[test]
fn legacy_error_adapter_keeps_categories_reasons_and_errno() {
    use crate::ProtectedServiceProfileErrorV1 as Legacy;
    assert!(matches!(
        crate::map_observation_error(Error::ProcessProfile("profile")),
        Legacy::ProcessProfile("profile")
    ));
    assert!(matches!(
        crate::map_observation_error(Error::Namespace("namespace")),
        Legacy::Namespace("namespace")
    ));
    assert!(matches!(
        crate::map_observation_error(Error::InvalidState("state")),
        Legacy::InvalidState("state")
    ));
    for source in [Errno::INTR, Errno::ACCESS, Errno::NOENT] {
        let Legacy::Io {
            operation,
            source: actual,
        } = crate::map_observation_error(Error::Io {
            operation: "read proc process status",
            source,
        })
        else {
            panic!("legacy error changed")
        };
        assert_eq!(operation, "read proc process status");
        assert_eq!(actual.raw_os_error(), Some(source.raw_os_error()));
    }
    assert!(!std::mem::needs_drop::<Error>());
}

#[test]
fn legacy_process_adapter_keeps_const_getters_and_failure_mapping() {
    let core = ProcessProfile {
        credentials: credentials(),
        cap_last_cap: 63,
    };
    let legacy = crate::ProtectedServiceProcessProfileV1 { observation: core };
    assert_eq!(legacy.credentials(), credentials());
    assert_eq!(legacy.cap_last_cap(), 63);
    let pid = Pid::from_raw(i32::MAX).unwrap();
    let core_error = legacy.observation.revalidate_process(pid).unwrap_err();
    assert_eq!(
        legacy.revalidate_process(pid).unwrap_err().to_string(),
        crate::map_observation_error(core_error).to_string()
    );
    assert_eq!(validate_process(credentials(), pid), Err(core_error));
}
