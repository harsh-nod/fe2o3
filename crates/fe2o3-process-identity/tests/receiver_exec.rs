use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_process_identity::{
    LinuxObjectIdentityV3, ParentPreparedProcessConsistencyV3, PinnedWorkingDirectoryV3,
    S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3,
    compare_child_observation_with_parent_preparation_v3, measure_executable_sha256_v3,
    parent_prepared_process_consistency_digest_v3,
};

const CHILD_MODE: &str = "FE2O3_PROCESS_CONSISTENCY_RECEIVER_CHILD";
const TEST_NAME: &str = "receiver_compares_exact_child_observation";
const SOURCE: &str = "source.rs";
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum DescriptorMode {
    Sealed,
    Unsealed,
    Swapped,
    Closed,
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-process-consistency-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn object_identity(file: &File) -> LinuxObjectIdentityV3 {
    let metadata = file.metadata().unwrap();
    LinuxObjectIdentityV3::from_linux_stat(metadata.dev(), metadata.ino(), metadata.mode())
}

fn sorted_environment(extra: Option<(&str, &str)>) -> Vec<(OsString, OsString)> {
    let mut environment = vec![(OsString::from(CHILD_MODE), OsString::from("1"))];
    if let Some((name, value)) = extra {
        environment.push((OsString::from(name), OsString::from(value)));
    }
    environment.sort_unstable();
    environment
}

fn child_arguments(extra: Option<&str>) -> Vec<OsString> {
    let mut arguments = vec![
        OsString::from("--exact"),
        OsString::from(TEST_NAME),
        OsString::from("--nocapture"),
    ];
    if let Some(extra) = extra {
        arguments.push(OsString::from(extra));
    }
    arguments
}

fn expected_digest(
    executable: &File,
    argv0: &OsStr,
    arguments: &[OsString],
    cwd: &PinnedWorkingDirectoryV3,
    environment: &[(OsString, OsString)],
) -> [u8; 32] {
    let executable_path = PathBuf::from(format!("/proc/self/fd/{}", executable.as_raw_fd()));
    let executable_sha256 = measure_executable_sha256_v3(&executable_path).unwrap();
    let source = cwd
        .measure_protected_source_tree(Path::new(SOURCE))
        .unwrap();
    let mut argv = vec![argv0.to_owned()];
    argv.extend_from_slice(arguments);
    parent_prepared_process_consistency_digest_v3(&ParentPreparedProcessConsistencyV3 {
        executable_object: object_identity(executable),
        executable_sha256,
        argv: &argv,
        current_dir_object: cwd.object_identity(),
        protected_source_tree_sha256: source.identity_sha256(),
        environment,
    })
    .unwrap()
}

fn expectation_file(digest: [u8; 32], sealed: bool) -> File {
    let mut file = File::from(
        rustix::fs::memfd_create(
            "fe2o3-process-consistency-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    file.write_all(&digest).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    if sealed {
        rustix::fs::fcntl_add_seals(
            &file,
            rustix::fs::SealFlags::WRITE
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::SHRINK
                | rustix::fs::SealFlags::SEAL,
        )
        .unwrap();
    }
    file
}

fn run_child(
    executable: &File,
    cwd: &PinnedWorkingDirectoryV3,
    expected: [u8; 32],
    descriptor_mode: DescriptorMode,
    argv0: &OsStr,
    extra_argument: Option<&str>,
    extra_environment: Option<(&str, &str)>,
) -> ExitStatus {
    let expectation = expectation_file(expected, matches!(descriptor_mode, DescriptorMode::Sealed));
    let expectation_fd = expectation.as_raw_fd();
    let executable_path = PathBuf::from(format!("/proc/self/fd/{}", executable.as_raw_fd()));
    let mut command = Command::new(executable_path);
    command
        .arg0(argv0)
        .args(child_arguments(extra_argument))
        .env_clear()
        .env(CHILD_MODE, "1");
    if let Some((name, value)) = extra_environment {
        command.env(name, value);
    }
    cwd.configure_child_fchdir(&mut command);
    // SAFETY: the callback performs descriptor-only operations before exec.
    unsafe {
        command.pre_exec(move || {
            match descriptor_mode {
                DescriptorMode::Sealed | DescriptorMode::Unsealed => {
                    if libc::dup3(expectation_fd, S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3, 0)
                        != S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    let target = BorrowedFd::borrow_raw(S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3);
                    rustix::io::fcntl_setfd(target, rustix::io::FdFlags::empty())
                        .map_err(std::io::Error::from)?;
                }
                DescriptorMode::Swapped => {
                    let replacement = rustix::fs::open(
                        "/dev/null",
                        rustix::fs::OFlags::RDONLY,
                        rustix::fs::Mode::empty(),
                    )
                    .map_err(std::io::Error::from)?;
                    if libc::dup3(
                        replacement.as_raw_fd(),
                        S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3,
                        0,
                    ) != S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    let target = BorrowedFd::borrow_raw(S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3);
                    rustix::io::fcntl_setfd(target, rustix::io::FdFlags::empty())
                        .map_err(std::io::Error::from)?;
                }
                DescriptorMode::Closed => {
                    libc::close(S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3);
                }
            }
            Ok(())
        });
    }
    command.status().unwrap()
}

#[test]
fn receiver_compares_exact_child_observation() {
    if std::env::var_os(CHILD_MODE).is_some() {
        let cwd = PinnedWorkingDirectoryV3::open(Path::new(".")).unwrap();
        let source = cwd
            .measure_protected_source_tree(Path::new(SOURCE))
            .unwrap();
        compare_child_observation_with_parent_preparation_v3(
            S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3,
            source.identity_sha256(),
        )
        .unwrap();
        // SAFETY: fcntl only probes whether the consumed fixed descriptor remains live.
        assert_eq!(
            unsafe { libc::fcntl(S09_PROCESS_CONSISTENCY_EXPECTATION_FD_V3, libc::F_GETFD,) },
            -1
        );
        return;
    }

    let executable_path = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let executable = File::open(&executable_path).unwrap();
    let root = TestDirectory::new();
    fs::write(root.0.join(SOURCE), b"protected source\n").unwrap();
    let cwd = PinnedWorkingDirectoryV3::open(&root.0).unwrap();
    let argv0 = executable_path.as_os_str();
    let arguments = child_arguments(None);
    let environment = sorted_environment(None);
    let valid = expected_digest(&executable, argv0, &arguments, &cwd, &environment);
    assert!(
        run_child(
            &executable,
            &cwd,
            valid,
            DescriptorMode::Sealed,
            argv0,
            None,
            None,
        )
        .success()
    );

    for descriptor_mode in [
        DescriptorMode::Closed,
        DescriptorMode::Swapped,
        DescriptorMode::Unsealed,
    ] {
        assert!(
            !run_child(&executable, &cwd, valid, descriptor_mode, argv0, None, None,).success()
        );
    }
    assert!(
        !run_child(
            &executable,
            &cwd,
            [0x55; 32],
            DescriptorMode::Sealed,
            argv0,
            None,
            None,
        )
        .success()
    );
    assert!(
        !run_child(
            &executable,
            &cwd,
            valid,
            DescriptorMode::Sealed,
            OsStr::new("different-raw-argv0"),
            None,
            None,
        )
        .success(),
        "raw argv0 mismatch was normalized away"
    );
    assert!(
        !run_child(
            &executable,
            &cwd,
            valid,
            DescriptorMode::Sealed,
            argv0,
            Some("--test-threads=1"),
            None,
        )
        .success()
    );
    assert!(
        !run_child(
            &executable,
            &cwd,
            valid,
            DescriptorMode::Sealed,
            argv0,
            None,
            Some(("CHANGED", "1")),
        )
        .success()
    );
    fs::write(root.0.join(SOURCE), b"mutated protected source\n").unwrap();
    assert!(
        !run_child(
            &executable,
            &cwd,
            valid,
            DescriptorMode::Sealed,
            argv0,
            None,
            None,
        )
        .success(),
        "protected source-tree mutation was not observed"
    );
}

#[test]
fn pinned_cwd_survives_path_replacement() {
    if std::env::var_os(CHILD_MODE).is_some() {
        return;
    }
    let executable_path = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let executable = File::open(&executable_path).unwrap();
    let root = TestDirectory::new();
    let selected = root.0.join("selected-cwd");
    let moved = root.0.join("moved-cwd");
    fs::create_dir(&selected).unwrap();
    fs::write(selected.join(SOURCE), b"original source\n").unwrap();
    let cwd = PinnedWorkingDirectoryV3::open(&selected).unwrap();
    let argv0 = executable_path.as_os_str();
    let expected = expected_digest(
        &executable,
        argv0,
        &child_arguments(None),
        &cwd,
        &sorted_environment(None),
    );

    fs::rename(&selected, &moved).unwrap();
    fs::create_dir(&selected).unwrap();
    fs::write(selected.join(SOURCE), b"replacement source\n").unwrap();
    assert!(
        run_child(
            &executable,
            &cwd,
            expected,
            DescriptorMode::Sealed,
            argv0,
            None,
            None,
        )
        .success(),
        "child followed a replaced cwd pathname instead of the pinned object"
    );
}

#[test]
fn retained_executable_descriptor_prevents_path_trampoline() {
    if std::env::var_os(CHILD_MODE).is_some() {
        return;
    }
    let source = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let root = TestDirectory::new();
    let selected = root.0.join("selected-test");
    let replacement = root.0.join("replacement-test");
    fs::copy(&source, &selected).unwrap();
    fs::copy(&source, &replacement).unwrap();
    let mut permissions = fs::metadata(&selected).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&selected, permissions.clone()).unwrap();
    fs::set_permissions(&replacement, permissions).unwrap();
    let executable = File::open(&selected).unwrap();
    fs::write(root.0.join(SOURCE), b"protected source\n").unwrap();
    let cwd = PinnedWorkingDirectoryV3::open(&root.0).unwrap();
    let argv0 = selected.as_os_str();
    let expected = expected_digest(
        &executable,
        argv0,
        &child_arguments(None),
        &cwd,
        &sorted_environment(None),
    );

    File::options()
        .append(true)
        .open(&replacement)
        .unwrap()
        .write_all(b"replacement")
        .unwrap();
    fs::rename(&replacement, &selected).unwrap();
    assert!(
        run_child(
            &executable,
            &cwd,
            expected,
            DescriptorMode::Sealed,
            argv0,
            None,
            None,
        )
        .success(),
        "execution reopened the replaced pathname"
    );
}
