//! CPU-only parsers and refusal policy. Never opens /proc, /dev, or a GPU.
use super::*;
fn environment<'a>(
    values: &'a [(&'a str, &'a str)],
) -> impl Iterator<Item = (OsString, OsString)> + 'a {
    values
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
}
#[test]
fn only_empty_or_exact_locale_environment_is_admitted() {
    assert!(check_environment(std::iter::empty()).is_ok());
    assert!(check_environment(environment(&[("LANG", "C"), ("LC_ALL", "C.UTF-8")])).is_ok());
    for key in [
        "LD_PRELOAD",
        "LD_AUDIT",
        "LD_LIBRARY_PATH",
        "GLIBC_TUNABLES",
        "HSA_TOOLS_LIB",
        "ROCP_TOOL_LIB",
        "ROCR_VISIBLE_DEVICES",
        "HIP_VISIBLE_DEVICES",
        "CUDA_INJECTION64_PATH",
        "RUST_BACKTRACE",
        "PATH",
        "HOME",
        "UNKNOWN",
    ] {
        assert!(
            check_environment(environment(&[(key, "")])).is_err(),
            "{key}"
        );
    }
    assert!(check_environment(environment(&[("LANG", "en_US.UTF-8")])).is_err());
    assert!(check_environment(environment(&[("LANG", "C"), ("LC_ALL", "bad")])).is_err());
}
#[test]
fn environment_census_and_value_bounds_refuse() {
    assert!(check_environment((0..17).map(|_| ("LANG".into(), "C".into()))).is_err());
    assert!(check_environment(std::iter::once(("LANG".into(), "x".repeat(4097).into()))).is_err());
}
#[test]
fn task_census_requires_one_exact_main_thread() {
    assert!(task_identity(["123".into()].into_iter(), 123).is_ok());
    for names in [
        vec![],
        vec!["124"],
        vec!["123", "124"],
        vec!["0123"],
        vec!["x"],
        vec!["4294967296"],
    ] {
        assert!(task_identity(names.into_iter().map(OsString::from), 123).is_err());
    }
}
#[test]
fn gpu_fd_roster_has_no_fresh_gpu_owner_and_one_exact_prepared_pair() {
    for (prepared, kfd, drm, accepted) in [
        (false, 0, 0, true),
        (false, 1, 0, false),
        (false, 0, 1, false),
        (false, 1, 1, false),
        (true, 1, 1, true),
        (true, 0, 0, false),
        (true, 2, 1, false),
        (true, 1, 2, false),
    ] {
        assert_eq!(gpu_fd_count_policy(prepared, kfd, drm).is_ok(), accepted);
    }
}
#[test]
fn only_exact_host_library_and_owned_device_paths_are_admitted() {
    let executable = Path::new("/work/observer");
    let gpu = vec![
        PathBuf::from("/dev/kfd"),
        PathBuf::from("/dev/dri/renderD128"),
    ];
    assert_eq!(
        mapping_path_policy(executable, executable, false, &[]),
        Ok(false)
    );
    for path in LIBRARIES {
        assert_eq!(
            mapping_path_policy(Path::new(path), executable, false, &[]),
            Ok(false)
        );
    }
    for path in [
        "/opt/rocm/lib/libhsa-runtime64.so",
        "/opt/rocm/lib/libamdhip64.so",
        "/usr/lib/x86_64-linux-gnu/libc.so.6.extra",
        "/tmp/libc.so.6",
        "/work/other",
        "/memfd:injected",
    ] {
        assert!(mapping_path_policy(Path::new(path), executable, true, &gpu).is_err());
    }
    assert!(mapping_path_policy(Path::new("/dev/kfd"), executable, false, &gpu).is_err());
    assert_eq!(
        mapping_path_policy(Path::new("/dev/kfd"), executable, true, &gpu),
        Ok(true)
    );
    assert!(mapping_path_policy(Path::new("/dev/dri/renderD129"), executable, true, &gpu).is_err());
}
#[test]
fn map_rows_parse_exact_identifiers_and_permissions_without_authority() {
    let rows = parse_maps(
        b"1000-2000 r-xp 00000000 08:01 42 /work/observer\n2000-3000 rw-p 00000000 00:00 0\n",
    )
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        MapRow {
            executable: true,
            major: 8,
            minor: 1,
            inode: 42,
            path: "/work/observer"
        }
    );
    assert_eq!(rows[1].path, "");
}
#[test]
fn malformed_truncated_oversize_and_writable_executable_maps_refuse() {
    for text in [
        "",
        "1000-2000 r-xp 00000000 08:01 42 /work/observer",
        "2000-1000 r-xp 0 08:01 42 /work/observer\n",
        "1000-2000 rwxp 0 08:01 42 /work/observer\n",
        "1000-2000 x--p 0 08:01 42 /work/observer\n",
        "1000-2000 r-xp z 08:01 42 /work/observer\n",
        "1000-2000 r-xp 0 zz:01 42 /work/observer\n",
        "1000-2000 r-xp 0 08:01 -1 /work/observer\n",
        "1000-2000 r-xp 0 08:01 42 /work/observer (deleted)\n",
    ] {
        assert!(parse_maps(text.as_bytes()).is_err(), "{text}");
    }
    assert!(parse_maps(&vec![b'x'; MAP_BYTES + 1]).is_err());
    let too_many = "1000-2000 rw-p 0 00:00 0\n".repeat(MAP_ROWS + 1);
    assert!(parse_maps(too_many.as_bytes()).is_err());
}
#[test]
fn bounded_snapshot_reader_never_consumes_unbounded_input() {
    assert_eq!(read_bounded(&b"abc"[..], 3).unwrap(), b"abc");
    assert!(read_bounded(&b"abcd"[..], 3).is_err());
    assert!(read_bounded(&b""[..], usize::MAX).is_err());
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("synthetic read failure"))
        }
    }
    assert!(read_bounded(Broken, 10).is_err());
}
