use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_process_identity::{
    PreparedCommandIdentityV2, S09_PREPARED_COMMAND_EXPECTATION_FD_V2,
    measure_executable_sha256_v2, prepared_command_digest_v2,
    verify_actual_process_against_sealed_expectation_v2,
};

const CHILD_MODE: &str = "FE2O3_PROCESS_IDENTITY_RECEIVER_CHILD";
const EXPECT_SUCCESS: &str = "FE2O3_PROCESS_IDENTITY_EXPECT_SUCCESS";
const TEST_NAME: &str = "receiver_recomputes_actual_process";
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum DescriptorMode {
    Sealed,
    Unsealed,
    Swapped,
    Closed,
}

struct ChildCase<'a> {
    actual_value: &'a str,
    descriptor_mode: DescriptorMode,
    succeeds: bool,
    argv0: Option<&'a OsStr>,
    extra_argument: Option<&'a str>,
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-process-identity-{}-{id}",
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

fn sorted_environment(value: &str, succeeds: bool) -> Vec<(OsString, OsString)> {
    let mut environment = vec![
        (OsString::from(CHILD_MODE), OsString::from(value)),
        (
            OsString::from(EXPECT_SUCCESS),
            OsString::from(if succeeds { "1" } else { "0" }),
        ),
    ];
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
    executable: &Path,
    executable_sha256: [u8; 32],
    arguments: &[OsString],
    cwd: &Path,
    environment: &[(OsString, OsString)],
) -> [u8; 32] {
    prepared_command_digest_v2(&PreparedCommandIdentityV2 {
        executable_path: executable,
        executable_sha256,
        arguments_after_argv0: arguments,
        current_dir: cwd,
        environment,
    })
    .unwrap()
}

fn expectation_file(digest: [u8; 32], sealed: bool) -> File {
    let mut file = File::from(
        rustix::fs::memfd_create(
            "fe2o3-process-identity-test",
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

fn run_child(executable: &Path, cwd: &Path, expected: [u8; 32], case: ChildCase<'_>) -> ExitStatus {
    let expectation = expectation_file(
        expected,
        matches!(case.descriptor_mode, DescriptorMode::Sealed),
    );
    let expectation_fd = expectation.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .args(child_arguments(case.extra_argument))
        .current_dir(cwd)
        .env_clear()
        .env(CHILD_MODE, case.actual_value)
        .env(EXPECT_SUCCESS, if case.succeeds { "1" } else { "0" });
    if let Some(argv0) = case.argv0 {
        command.arg0(argv0);
    }
    // SAFETY: the callback performs descriptor-only operations before exec.
    unsafe {
        command.pre_exec(move || {
            match case.descriptor_mode {
                DescriptorMode::Sealed | DescriptorMode::Unsealed => {
                    if libc::dup3(expectation_fd, S09_PREPARED_COMMAND_EXPECTATION_FD_V2, 0)
                        != S09_PREPARED_COMMAND_EXPECTATION_FD_V2
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    let target = BorrowedFd::borrow_raw(S09_PREPARED_COMMAND_EXPECTATION_FD_V2);
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
                        S09_PREPARED_COMMAND_EXPECTATION_FD_V2,
                        0,
                    ) != S09_PREPARED_COMMAND_EXPECTATION_FD_V2
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    let target = BorrowedFd::borrow_raw(S09_PREPARED_COMMAND_EXPECTATION_FD_V2);
                    rustix::io::fcntl_setfd(target, rustix::io::FdFlags::empty())
                        .map_err(std::io::Error::from)?;
                }
                DescriptorMode::Closed => {
                    libc::close(S09_PREPARED_COMMAND_EXPECTATION_FD_V2);
                }
            }
            Ok(())
        });
    }
    command.status().unwrap()
}

#[test]
fn receiver_recomputes_actual_process() {
    if std::env::var_os(CHILD_MODE).is_some() {
        let expected_success = std::env::var(EXPECT_SUCCESS).unwrap() == "1";
        let result = verify_actual_process_against_sealed_expectation_v2(
            S09_PREPARED_COMMAND_EXPECTATION_FD_V2,
        );
        assert_eq!(result.is_ok(), expected_success, "{result:?}");
        // SAFETY: fcntl only probes whether the fixed descriptor remains live.
        assert_eq!(
            unsafe { libc::fcntl(S09_PREPARED_COMMAND_EXPECTATION_FD_V2, libc::F_GETFD) },
            -1
        );
        return;
    }

    let executable = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let executable_sha256 = measure_executable_sha256_v2(&executable).unwrap();
    let root = TestDirectory::new();
    let cwd = &root.0;
    let arguments = child_arguments(None);
    let environment = sorted_environment("baseline", true);
    let valid = expected_digest(
        &executable,
        executable_sha256,
        &arguments,
        cwd,
        &environment,
    );
    assert!(
        run_child(
            &executable,
            cwd,
            valid,
            ChildCase {
                actual_value: "baseline",
                descriptor_mode: DescriptorMode::Sealed,
                succeeds: true,
                argv0: Some(OsStr::new("deliberately-different-argv0")),
                extra_argument: None,
            },
        )
        .success()
    );

    let invalid_cases = [
        (valid, DescriptorMode::Closed, "baseline", None),
        (valid, DescriptorMode::Swapped, "baseline", None),
        (valid, DescriptorMode::Unsealed, "baseline", None),
        ([0x55; 32], DescriptorMode::Sealed, "baseline", None),
        (valid, DescriptorMode::Sealed, "changed-env", None),
        (
            valid,
            DescriptorMode::Sealed,
            "baseline",
            Some("--test-threads=1"),
        ),
    ];
    for (digest, descriptor_mode, value, extra) in invalid_cases {
        assert!(
            run_child(
                &executable,
                cwd,
                digest,
                ChildCase {
                    actual_value: value,
                    descriptor_mode,
                    succeeds: false,
                    argv0: None,
                    extra_argument: extra,
                },
            )
            .success()
        );
    }

    let wrong_cwd = expected_digest(
        &executable,
        executable_sha256,
        &arguments,
        Path::new("/"),
        &sorted_environment("baseline", false),
    );
    assert!(
        run_child(
            &executable,
            cwd,
            wrong_cwd,
            ChildCase {
                actual_value: "baseline",
                descriptor_mode: DescriptorMode::Sealed,
                succeeds: false,
                argv0: None,
                extra_argument: None,
            },
        )
        .success()
    );
    let wrong_executable = expected_digest(
        Path::new("/bin/true"),
        executable_sha256,
        &arguments,
        cwd,
        &sorted_environment("baseline", false),
    );
    assert!(
        run_child(
            &executable,
            cwd,
            wrong_executable,
            ChildCase {
                actual_value: "baseline",
                descriptor_mode: DescriptorMode::Sealed,
                succeeds: false,
                argv0: None,
                extra_argument: None,
            },
        )
        .success()
    );
}

#[test]
fn pathname_replacement_executes_but_fails_the_receiver_measurement() {
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
    let selected_sha256 = measure_executable_sha256_v2(&selected).unwrap();
    File::options()
        .append(true)
        .open(&replacement)
        .unwrap()
        .write_all(b"replacement")
        .unwrap();
    fs::rename(&replacement, &selected).unwrap();

    let arguments = child_arguments(None);
    let environment = sorted_environment("replacement", false);
    let expected = expected_digest(
        &selected,
        selected_sha256,
        &arguments,
        &root.0,
        &environment,
    );
    assert!(
        run_child(
            &selected,
            &root.0,
            expected,
            ChildCase {
                actual_value: "replacement",
                descriptor_mode: DescriptorMode::Sealed,
                succeeds: false,
                argv0: None,
                extra_argument: None,
            },
        )
        .success()
    );
}
