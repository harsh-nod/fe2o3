//! Parser and state tests only: no admitted domain, clone, or cleanup evidence.
//! The separately ignored mount tests exercise only read-only preparation.

use super::*;

#[test]
fn events_require_exact_unique_boolean_fields_in_either_order() {
    for populated in [false, true] {
        for frozen in [false, true] {
            let expected = Events { populated, frozen };
            for text in [
                format!(
                    "populated {}\nfrozen {}\n",
                    u8::from(populated),
                    u8::from(frozen)
                ),
                format!(
                    "frozen {}\npopulated {}\n",
                    u8::from(frozen),
                    u8::from(populated)
                ),
            ] {
                assert_eq!(parse_events(text.as_bytes()).unwrap(), expected);
            }
        }
    }
}

#[test]
fn events_reject_missing_duplicate_unknown_and_truncated_rows() {
    for text in [
        "",
        "\n",
        "populated 0\n",
        "frozen 0\n",
        "populated 0\nfrozen 0",
        "populated 0\npopulated 0\nfrozen 0\n",
        "populated 0\nfrozen 0\nfrozen 1\n",
        "populated 0\nfrozen 0\nunknown 0\n",
        "populated 0\nfrozen 0\n\n",
        "populated 2\nfrozen 0\n",
        "populated 00\nfrozen 0\n",
        "populated -1\nfrozen 0\n",
        "populated 0\nfrozen 01\n",
        "populated 0\nfrozen 0 1\n",
        "populated\nfrozen 0\n",
        "populated\t0\nfrozen 0\n",
        "populated  0\nfrozen 0\n",
        "populated 0 \nfrozen 0\n",
        "populated 0\r\nfrozen 0\n",
        "populated 0\nfrozen 0\0\n",
    ] {
        assert!(parse_events(text.as_bytes()).is_err(), "accepted {text:?}");
    }
    let oversized = [b'\n'; READ_LIMIT];
    assert!(parse_events(&oversized).is_err());
    let valid = b"populated 0\nfrozen 0\n";
    for end in 0..valid.len() {
        assert!(parse_events(&valid[..end]).is_err());
    }
}

#[test]
fn membership_extracts_exact_unified_entry_without_rewriting_names() {
    for (text, expected) in [
        ("0::/\n", &b"/"[..]),
        (
            "0::/system.slice/service.scope\n",
            &b"/system.slice/service.scope"[..],
        ),
        (
            "5:cpu,cpuacct:/legacy\n0::/owned/scope\n",
            &b"/owned/scope"[..],
        ),
        (
            "0::/a:b/space name\n3:name=systemd:/legacy\n",
            &b"/a:b/space name"[..],
        ),
    ] {
        let path = membership_path(text.as_bytes()).unwrap();
        assert_eq!(path, expected);
        let mut buffer = [0; READ_LIMIT];
        assert_eq!(
            relative_path(path, &mut buffer).unwrap().to_bytes(),
            if path == b"/" { &b"."[..] } else { &path[1..] }
        );
    }
}

#[test]
fn membership_rejects_ambiguous_escaped_deleted_or_noncanonical_paths() {
    for text in [
        "",
        "0::/",
        "0::/\n0::/\n",
        "0:cpu:/\n",
        "3:cpu:/legacy\n",
        "0::relative\n",
        "0::/../parent\n",
        "0::/a/./b\n",
        "0::/a/../b\n",
        "0:://a\n",
        "0::/a//b\n",
        "0::/a/\n",
        "0::/a (deleted)\n",
        "0::/a\0b\n",
        "0::/a\tb\n",
        "0::/a\r\n",
        "0::/\n\n",
        "00::/\n",
        "-1:cpu:/\n",
        "1::/legacy\n0::/\n",
        "4294967296:cpu:/\n0::/\n",
        "0::/\n1:cpu!:/legacy\n",
    ] {
        assert!(
            membership_path(text.as_bytes()).is_err(),
            "accepted {text:?}"
        );
    }
    let overlong_component = format!("0::/{}\n", "a".repeat(256));
    assert!(membership_path(overlong_component.as_bytes()).is_err());
    let mut too_long = [b'/'; READ_LIMIT];
    too_long[..4].copy_from_slice(b"0::/");
    too_long[READ_LIMIT - 1] = b'\n';
    assert!(membership_path(&too_long).is_err());
}

#[test]
fn membership_split_admits_only_the_final_actual_component() {
    for (path, prefix, leaf) in [
        (&b"/service"[..], &b"/"[..], &b"service"[..]),
        (
            b"/system.slice/fe2o3-compiler-execution.service",
            b"/system.slice",
            b"fe2o3-compiler-execution.service",
        ),
        (b"/a/b/service/child", b"/a/b/service", b"child"),
    ] {
        assert_eq!(membership_components(path).unwrap(), (prefix, leaf));
        assert!(!leaf.contains(&b'/'));
    }
    for path in [
        &b""[..],
        b"relative",
        b"//service",
        b"/a/../service",
        b"/a/./service",
        b"/a//service",
        b"/a/service/",
        b"/a/service\0suffix",
    ] {
        assert!(membership_components(path).is_err(), "accepted {path:?}");
    }
    let maximum = format!("/prefix/{}", "a".repeat(255));
    assert_eq!(
        membership_components(maximum.as_bytes()).unwrap().1.len(),
        255
    );
    assert!(membership_components(format!("{maximum}a").as_bytes()).is_err());
    assert_eq!(
        RESOLVE,
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV
    );
}

#[test]
fn hierarchy_root_is_parseable_but_not_an_admissible_parent() {
    // The parser still accepts real proc output; admission cannot infer a
    // domain type from the hierarchy root, where cgroup.type does not exist.
    let path = membership_path(b"0::/\n").unwrap();
    assert_eq!(path, b"/");
    assert!(matches!(
        membership_components(path),
        Err(Error::State(
            "hierarchy-root cgroup membership is unsupported"
        ))
    ));
}

#[test]
fn parent_type_requires_one_complete_ordinary_domain_record() {
    assert!(validate_parent_type(b"domain\n").is_ok());
    for bytes in [
        &b"domain threaded\n"[..],
        b"threaded\n",
        b"domain invalid\n",
        b"domain \n",
        b" domain\n",
        b"domain\r\n",
        b"domain\0\n",
        b"domain\n\0",
        b"domain\nthreaded\n",
        b"domain\ndomain\n",
        b"domain\n\n",
    ] {
        assert!(
            matches!(
                validate_parent_type(bytes),
                Err(Error::State("membership cgroup is not an ordinary domain"))
            ),
            "accepted {bytes:?}"
        );
    }
    for end in 0..b"domain\n".len() {
        assert!(validate_parent_type(&b"domain\n"[..end]).is_err());
    }
    let mut overlong = [b'x'; READ_LIMIT];
    overlong[..7].copy_from_slice(b"domain\n");
    assert!(validate_parent_type(&overlong).is_err());
}

#[test]
fn membership_metadata_requires_same_filesystem_and_protected_root_owner() {
    // A metadata-only fixture, not evidence of cgroup2 or current membership.
    let fd = fs::open(c"/", READ_FLAGS | OFlags::DIRECTORY, Mode::empty()).unwrap();
    let root = fs::fstat(&fd).unwrap();
    let mut current = root;
    current.st_uid = 0;
    current.st_gid = 0;
    current.st_mode = libc::S_IFDIR | 0o755;
    current.st_nlink = 1;
    assert!(validate_membership_stat(&root, &current).is_ok());
    for field in 0..6 {
        let mut wrong = current;
        match field {
            0 => wrong.st_dev = wrong.st_dev.wrapping_add(1),
            1 => wrong.st_uid = 1,
            2 => wrong.st_gid = 1,
            3 => wrong.st_mode |= 0o020,
            4 => wrong.st_mode = libc::S_IFLNK | 0o755,
            5 => wrong.st_nlink = 0,
            _ => unreachable!(),
        }
        assert!(
            validate_membership_stat(&root, &wrong).is_err(),
            "field {field}"
        );
    }
}

#[test]
fn generated_component_is_fixed_bounded_and_contains_no_caller_path() {
    let uuid = b"12345678-90ab-cdef-0123-456789abcdef\n";
    let generated = generated_name(uuid).unwrap();
    let name = CStr::from_bytes_with_nul(&generated).unwrap().to_bytes();
    assert_eq!(name, b"fe2o3-native-1234567890abcdef0123456789abcdef");
    assert_eq!(name.len(), NAME_BYTES - 1);
    assert!(!name.contains(&b'/'));
    for index in 0..uuid.len() {
        let mut bad = *uuid;
        bad[index] = if matches!(index, 8 | 13 | 18 | 23) {
            b'0'
        } else {
            b'/'
        };
        assert!(generated_name(&bad).is_err());
    }
    for end in 0..uuid.len() {
        assert!(generated_name(&uuid[..end]).is_err());
    }
}

#[test]
fn directory_identity_rejects_replacement_wrong_type_and_removed_inode() {
    // Only metadata comparisons over a read-only fixture, never domain custody
    // or cleanup evidence. No test creates, removes or adopts an actual cgroup.
    let directory = fs::open(c"/", READ_FLAGS | OFlags::DIRECTORY, Mode::empty()).unwrap();
    let actual = fs::fstat(&directory).unwrap();
    let expected = Identity::of(&actual);
    assert!(same_directory(expected, &actual, &actual));
    let mut changed = actual;
    changed.st_ino = changed.st_ino.wrapping_add(1);
    assert!(!same_directory(expected, &actual, &changed));
    assert!(!same_directory(expected, &changed, &actual));
    changed = actual;
    changed.st_dev = changed.st_dev.wrapping_add(1);
    assert!(!same_directory(expected, &actual, &changed));
    changed = actual;
    changed.st_mode = libc::S_IFLNK | 0o700;
    assert!(!same_directory(expected, &actual, &changed));
    changed = actual;
    changed.st_nlink = 0;
    assert!(!same_directory(expected, &actual, &changed));
}

#[test]
fn mkdir_collision_and_known_precreation_refusals_never_adopt_existing_state() {
    for errno in [Errno::EXIST, Errno::ACCESS, Errno::PERM, Errno::ROFS] {
        assert_eq!(Phase::after_mkdir(Err(errno)), Phase::NoCreation);
        assert!(Phase::after_mkdir(Err(errno)).may_release());
    }
    // Even successful mkdir is not yet permission to expose the clone target.
    assert_eq!(Phase::after_mkdir(Ok(())), Phase::Uncertain);
    for errno in [Errno::INTR, Errno::IO, Errno::NOMEM, Errno::NOSPC] {
        assert_eq!(Phase::after_mkdir(Err(errno)), Phase::Quarantined);
        assert!(!Phase::after_mkdir(Err(errno)).may_release());
    }
}

fn inert_owner(phase: Phase) -> NativeCgroupDomainV1 {
    // No OS descriptor or actual creation: only no-I/O transitions may run.
    NativeCgroupDomainV1 {
        origin: rustix::process::Pid::INIT,
        phase,
        name: [0; NAME_BYTES],
        parent_identity: Identity {
            device: 0,
            inode: 0,
        },
        directory_identity: None,
        parent: None,
        directory: None,
        kill: None,
        events: None,
    }
}

impl NativeCgroupDomainV1 {
    pub(crate) fn quarantined_fixture_for_cleanup() -> Self {
        inert_owner(Phase::Quarantined)
    }
}

#[test]
fn prepared_and_no_creation_retire_without_touching_an_os_object() {
    for phase in [Phase::Prepared, Phase::NoCreation] {
        let mut owner = inert_owner(phase);
        assert_eq!(owner.step(), CleanupPollV1::Reaped);
        assert_eq!(owner.phase, Phase::Removed);
        assert_eq!(owner.step(), CleanupPollV1::Reaped);
        assert!(owner.clone_cgroup_fd().is_err());
    }
}

#[test]
fn uncertainty_never_turns_into_absence_or_allows_a_clone_descriptor() {
    for phase in [Phase::Uncertain, Phase::Quarantined] {
        let mut owner = inert_owner(phase);
        assert_eq!(owner.step(), CleanupPollV1::Quarantined);
        assert_eq!(owner.phase, Phase::Quarantined);
        assert_eq!(owner.step(), CleanupPollV1::Quarantined);
        assert!(owner.clone_cgroup_fd().is_err());
        assert!(!owner.phase.may_release());
    }
    for phase in [Phase::Created, Phase::Cleaning] {
        assert!(!phase.may_release());
    }
    assert!(inert_owner(Phase::Cleaning).clone_cgroup_fd().is_err());
}

#[test]
fn logical_bounds_cover_owner_and_fixed_frames() {
    type Domain = NativeCgroupDomainV1;
    assert_eq!(
        Domain::STORAGE,
        size_of::<Domain>() + 4 * size_of::<usize>()
    );
    // Independent old envelope plus the bounded admission delta. Storage and
    // cleanup remain unchanged; no new owner survives preparation failure.
    assert_eq!(
        Domain::PREPARE_SCRATCH,
        6 * 4096 + 4 * size_of::<Domain>() + 12 * size_of::<Stat>() + 1024
    );
    assert_eq!(Domain::PREPARE_WORK, 96 * (1024 + 64) + 48 * 4096);
    let admission_delta = Domain::PREPARE_WORK - (64 * (1024 + 64) + 32 * 4096);
    assert_eq!(admission_delta, 100_352);
    // Six extra mount-admission calls, six parent-type calls, three temporary
    // closes, and a conservative eight full-buffer passes fit the same delta.
    assert!(admission_delta >= (6 + 6 + 3) * (1024 + 64) + 8 * 4096);
    // Both bounded control reads are sequential, not two simultaneous buffers.
    assert!(Domain::PREPARE_SCRATCH >= 5 * READ_LIMIT + 256 + 8 * size_of::<Stat>());
    assert!(Domain::CREATE_SCRATCH >= 2 * READ_LIMIT);
    assert!(Domain::STEP_SCRATCH >= READ_LIMIT);
    assert!(Domain::CLONE_FD_SCRATCH >= 8 * size_of::<Stat>());
    assert!(Domain::PREPARE_WORK >= 64 * OPERATION_WORK);
    assert!(Domain::CREATE_WORK >= 64 * OPERATION_WORK);
    assert!(Domain::STEP_WORK >= 64 * OPERATION_WORK);
    assert!(Domain::CLONE_FD_WORK >= 32 * OPERATION_WORK);
}

#[path = "native_cgroup_mount_tests.rs"]
mod mounts;
