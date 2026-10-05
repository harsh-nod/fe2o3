use super::*;

fn object(ranges: &[(u64, u64)]) -> ExecutableObjectRanges<'_> {
    ExecutableObjectRanges::new(rustix::fs::makedev(8, 1), 42, ranges)
}

#[test]
fn file_ranges_require_nonempty_whole_object_relative_coverage() {
    let ranges = [(0x1000, 0x2000), (0x3000, 0x4000)];
    assert!(contains_file_range(0x1000, 0x1000, &ranges).unwrap());
    assert!(contains_file_range(0x1800, 1, &ranges).unwrap());
    for (offset, length) in [(0, 1), (0x1000, 0), (0x1800, 0x2000), (0x4000, 1)] {
        assert!(!contains_file_range(offset, length, &ranges).unwrap());
    }
    assert!(contains_file_range(u64::MAX, 1, &ranges).is_err());
    assert!(!contains_file_range(1, 1, &[]).unwrap());
}

#[test]
fn mapping_identity_uses_device_inode_and_range_not_pathname() {
    let ranges = [(0x1000, 0x2000)];
    let allowed = [object(&ranges)];
    assert!(mapping_file_range_is_allowed("08:01", "42", 0x1000, 0x1000, allowed).unwrap());
    for (device, inode, offset, length) in [
        ("08:02", "42", 0x1000, 0x1000),
        ("08:01", "43", 0x1000, 0x1000),
        ("08:01", "0", 0x1000, 0x1000),
        ("08:01", "42", 0, 0x1000),
        ("08:01", "42", 0x1000, 0),
    ] {
        assert!(!mapping_file_range_is_allowed(device, inode, offset, length, allowed).unwrap());
    }
    for (device, inode) in [
        ("08", "42"),
        ("xx:01", "42"),
        ("08:xx", "42"),
        ("08:01", "x"),
    ] {
        assert!(mapping_file_range_is_allowed(device, inode, 0x1000, 1, allowed).is_err());
    }
    assert!(mapping_file_range_is_allowed("08:01", "43", u64::MAX, 1, allowed).is_err());
}

#[test]
fn executable_rows_preserve_closed_identity_and_memory_policy() {
    let ranges = [(0x1000, 0x2000)];
    let allowed = [object(&ranges)];
    let check = |rows: &str| validate_executable_mapping_rows(rows, allowed.into_iter());
    assert!(check("1000-2000 r-xp 1000 08:01 42 /untrusted-name\n").is_ok());
    assert!(check("1000-2000 r-xp 1000 08:01 42 /renamed (deleted)\n").is_ok());
    for rows in [
        "",
        "1000-2000 rw-p 1000 08:01 42 /runtime\n",
        "1000-2000 rwxp 1000 08:01 42 /runtime\n",
        "1000-2000 r-xp 1000 08:01 41 /runtime\n",
        "1000-2000 r-xp 1000 08:02 42 /runtime\n",
        "1000-2000 r-xp 0000 08:01 42 /runtime\n",
        "1000-3000 r-xp 1000 08:01 42 /runtime\n",
        "1000-2000 r-xp 1000 08:01 42\n",
        "1000-2000 r-xp 1000 08:01 42 [anon]\n",
        "2000-1000 r-xp 1000 08:01 42 /runtime\n",
        "1000-2000 r-xp\n",
    ] {
        assert!(check(rows).is_err(), "{rows:?}");
    }
}

#[test]
fn kernel_mappings_keep_existing_policy_and_count_toward_bound() {
    let row = "1000-2000 r-xp 0 00:00 0 [vdso]\n";
    assert!(validate_executable_mapping_rows(&row.repeat(256), std::iter::empty()).is_ok());
    assert_eq!(
        validate_executable_mapping_rows(&row.repeat(257), std::iter::empty())
            .unwrap_err()
            .message(),
        "too many executable mappings"
    );
    assert!(
        validate_executable_mapping_rows(
            "1000-2000 r-xp 0 00:00 0 [vsyscall]\n",
            std::iter::empty(),
        )
        .is_ok()
    );
}

#[test]
fn nonexecutable_remap_requires_coverage_without_holes_or_exec() {
    let rows = "1000-2000 rw-p\n2000-3000 r--p\n3000-4000 r-xp\n";
    assert!(validate_nonexecutable_mapping_rows(rows, 0x1800, 0x1000).is_ok());
    for (start, length) in [(0, 1), (0x1000, 0), (0x2000, 0x1001), (u64::MAX, 1)] {
        assert!(validate_nonexecutable_mapping_rows(rows, start, length).is_err());
    }
    assert!(
        validate_nonexecutable_mapping_rows("1000-1800 rw-p\n2000-3000 rw-p\n", 0x1000, 0x2000)
            .is_err()
    );
}

#[test]
fn personality_and_proof_open_policy_never_modify_process_state() {
    assert!(validate_personality("00000000\n").is_ok());
    assert!(validate_personality("00400000\n").is_err());
    assert!(validate_personality("not-hex").is_err());
    assert!(validate_read_only_open(0).is_ok());
    assert!(validate_read_only_open(0x80000).is_ok());
    for flags in [1, 2, 3, 0x40, 0x200, 0x0040_0000] {
        assert!(validate_read_only_open(flags).is_err());
    }
}

fn exit_info(result: i64) -> [u8; 88] {
    let mut info = [0; 88];
    info[0] = 2;
    info[4..8].copy_from_slice(&0xc000_003e_u32.to_ne_bytes());
    info[24..32].copy_from_slice(&result.to_ne_bytes());
    info
}

#[test]
fn syscall_exit_requires_kernel_exit_abi_and_exact_register_agreement() {
    for result in [0, 1, -1, -4095] {
        assert_eq!(
            validate_syscall_exit(&exit_info(result), 9, 9, result as u64).unwrap(),
            result
        );
    }
    for size in 0..33 {
        assert!(validate_syscall_exit(&exit_info(0)[..size], 9, 9, 0).is_err());
    }
    for offset in [0, 4] {
        let mut wrong = exit_info(0);
        wrong[offset] ^= 1;
        assert!(validate_syscall_exit(&wrong, 9, 9, 0).is_err());
    }
    assert!(validate_syscall_exit(&exit_info(0), 9, 10, 0).is_err());
    assert!(validate_syscall_exit(&exit_info(0), 9, 9, 1).is_err());
    for restart in [-4, -512, -513, -514, -516] {
        assert!(validate_syscall_exit(&exit_info(restart), 9, 9, restart as u64).is_err());
    }
}

#[test]
fn strict_kernel_mapping_policy_requires_exact_derived_coordinates_not_names() {
    let row = "8000-a000 r-xp 00000000 00:00 0 [vdso]\n";
    assert!(validate_executable_mapping_rows(row, std::iter::empty()).is_ok());
    assert!(
        validate_executable_mapping_rows_with_kernel_ranges(row, std::iter::empty(), &[]).is_err()
    );
    assert!(
        validate_executable_mapping_rows_with_kernel_ranges(
            row,
            std::iter::empty(),
            &[(0x8000, 0xa000)]
        )
        .is_ok()
    );
    for (old, new) in [
        ("8000-a000", "9000-a000"),
        ("r-xp", "r-xs"),
        ("00000000", "00001000"),
        ("00:00", "08:01"),
        ("0 [vdso]", "1 [vdso]"),
    ] {
        assert!(
            validate_executable_mapping_rows_with_kernel_ranges(
                &row.replace(old, new),
                std::iter::empty(),
                &[(0x8000, 0xa000)]
            )
            .is_err()
        );
    }
    let renamed = row.replace("[vdso]", "not-an-authority");
    assert!(
        validate_executable_mapping_rows_with_kernel_ranges(
            &renamed,
            std::iter::empty(),
            &[(0x8000, 0xa000)]
        )
        .is_ok()
    );
    assert!(
        validate_executable_mapping_rows_with_kernel_ranges(
            "ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0 [vsyscall]\n",
            std::iter::empty(),
            &[]
        )
        .is_err()
    );
}

#[test]
fn native_x86_gate_requires_exact_kernel_abi_shape_and_is_not_a_name_exception() {
    let row = "ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0 [vsyscall]\n";
    for permissions in ["--xp", "r-xp"] {
        for name in ["[vsyscall]", "not-authority", ""] {
            let actual = row.replace("--xp", permissions).replace("[vsyscall]", name);
            assert!(
                validate_native_x86_executable_mapping_rows(&actual, std::iter::empty(), &[])
                    .is_ok()
            );
        }
    }
    for (old, new) in [
        ("ffffffffff600000", "ffffffffff5ff000"),
        ("ffffffffff601000", "ffffffffff602000"),
        ("--xp", "rwxp"),
        ("--xp", "--xs"),
        ("--xp", "r-xs"),
        ("00000000", "00001000"),
        ("00:00", "08:01"),
        ("0 [vsyscall]", "1 [vsyscall]"),
    ] {
        assert!(
            validate_native_x86_executable_mapping_rows(
                &row.replace(old, new),
                std::iter::empty(),
                &[]
            )
            .is_err(),
            "{old} -> {new}"
        );
    }
    assert!(
        validate_native_x86_executable_mapping_rows(
            &format!("{row}{row}"),
            std::iter::empty(),
            &[]
        )
        .is_err()
    );
    assert!(
        validate_native_x86_executable_mapping_rows(
            "8000-9000 r-xp 00000000 00:00 0 [vsyscall]\n",
            std::iter::empty(),
            &[]
        )
        .is_err()
    );
}

#[test]
fn direct_object_ranges_keep_identity_and_whole_interval_requirements() {
    let device = rustix::fs::makedev(8, 7);
    let ranges = [(0x1000, 0x3000)];
    let allowed = || [ExecutableObjectRanges::new(device, 42, &ranges)];
    assert!(executable_object_range_is_allowed(device, 42, 0x1000, 0x2000, allowed()).unwrap());
    for (dev, inode, offset, length) in [
        (device + 1, 42, 0x1000, 1),
        (device, 43, 0x1000, 1),
        (device, 42, 0x1000, 0),
        (device, 42, 0x2000, 0x2000),
    ] {
        assert!(
            !executable_object_range_is_allowed(dev, inode, offset, length, allowed()).unwrap()
        );
    }
    assert!(executable_object_range_is_allowed(device, 42, u64::MAX, 2, allowed()).is_err());
}
