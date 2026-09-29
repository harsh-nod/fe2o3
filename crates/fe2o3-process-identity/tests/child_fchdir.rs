use fe2o3_process_identity::PinnedWorkingDirectoryV3;
use rustix::io::FdFlags;
use std::{
    fs::{self, File},
    io,
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, RawFd},
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::Command,
};

const CASE_ENV: &str = "FE2O3_CHILD_FCHDIR_TEST_CASE";
const CASE_TEST: &str = "child_fchdir_lifetime_and_refusals";
const RECEIVER_TEST: &str = "child_fchdir_exec_receiver";
const EXPECTED_CWD: &str = "FE2O3_CHILD_FCHDIR_EXPECTED_CWD";
const EXPECTED_CONTENT: &str = "FE2O3_CHILD_FCHDIR_EXPECTED_CONTENT";
const CLOSED_FD: &str = "FE2O3_CHILD_FCHDIR_CLOSED_FD";
const FD_LIMIT: RawFd = 64;

// Every FD-table observation and limit change runs in a fresh, exact-test process.
#[test]
fn child_fchdir_lifetime_and_refusals() {
    if let Ok(case) = std::env::var(CASE_ENV) {
        let _limit = FileLimit::lower();
        match case.as_str() {
            "owner-drop-path-replacement" => owner_drop_path_replacement(),
            "unspawned-drop" => unspawned_drop(),
            "duplicate-failure" => duplicate_failure(),
            "invalid-source" => invalid_source(),
            "child-refusal" => child_refusal(),
            _ => panic!("unknown child case: {case}"),
        }
        return;
    }
    for case in [
        "owner-drop-path-replacement",
        "unspawned-drop",
        "duplicate-failure",
        "invalid-source",
        "child-refusal",
    ] {
        let output = test_command(CASE_TEST)
            .env(CASE_ENV, case)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case}: status={:?}\nstdout={}\nstderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn child_fchdir_exec_receiver() {
    let Some(expected_path) = std::env::var_os(EXPECTED_CWD) else {
        return;
    };
    if let Ok(fd) = std::env::var(CLOSED_FD) {
        assert_closed(fd.parse().unwrap());
    }
    let expected = fs::metadata(expected_path).unwrap();
    let actual = fs::metadata(".").unwrap();
    assert_eq!(
        (actual.dev(), actual.ino(), actual.mode()),
        (expected.dev(), expected.ino(), expected.mode()),
    );
    assert_eq!(
        fs::read("marker").unwrap(),
        std::env::var(EXPECTED_CONTENT).unwrap().as_bytes(),
    );
}

fn test_command(test: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env_remove(EXPECTED_CWD)
        .env_remove(EXPECTED_CONTENT)
        .env_remove(CLOSED_FD);
    command
}

fn receiver(path: &Path, content: &str) -> Command {
    let mut command = test_command(RECEIVER_TEST);
    command
        .env(EXPECTED_CWD, path)
        .env(EXPECTED_CONTENT, content);
    command
}

fn assert_success(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fe2o3-child-fchdir-{}", std::process::id(),));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("marker"), b"fallback").unwrap();
        fs::create_dir(path.join("selected")).unwrap();
        fs::write(path.join("selected/marker"), b"original").unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn descriptor_flags(fd: RawFd) -> io::Result<i32> {
    // SAFETY: this scalar probe permits an absent descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(flags)
    }
}

fn assert_closed(fd: RawFd) {
    assert_eq!(
        descriptor_flags(fd).unwrap_err().raw_os_error(),
        Some(libc::EBADF),
    );
}

fn descriptors() -> Vec<RawFd> {
    // No new descriptor can be allocated above this isolated child's soft limit.
    (0..FD_LIMIT)
        .filter(|fd| match descriptor_flags(*fd) {
            Ok(_) => true,
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(libc::EBADF));
                false
            }
        })
        .collect()
}

fn registered_descriptor(before: &[RawFd]) -> RawFd {
    let added: Vec<_> = descriptors()
        .into_iter()
        .filter(|fd| !before.contains(fd))
        .collect();
    assert_eq!(added.len(), 1, "registration must retain exactly one FD");
    let fd = added[0];
    assert!(fd >= 3);
    assert_eq!(descriptor_flags(fd).unwrap(), libc::FD_CLOEXEC);
    fd
}

fn owner_drop_path_replacement() {
    let root = Directory::new();
    let selected = root.0.join("selected");
    let moved = root.0.join("moved");
    let cwd = PinnedWorkingDirectoryV3::open(&selected).unwrap();
    let original_fd = cwd.native_source().unwrap().as_raw_fd();
    let mut command = receiver(&moved, "original");
    command.current_dir(&selected);
    let before = descriptors();
    cwd.configure_child_fchdir(&mut command).unwrap();
    let hook_fd = registered_descriptor(&before);

    // A shared status-flag change distinguishes duplication from reopening.
    let source = cwd.native_source().unwrap();
    let status = rustix::fs::fcntl_getfl(source).unwrap();
    rustix::fs::fcntl_setfl(source, status | rustix::fs::OFlags::NONBLOCK).unwrap();
    // SAFETY: command owns this live descriptor throughout the scalar probe.
    let duplicate_status = unsafe { libc::fcntl(hook_fd, libc::F_GETFL) };
    assert!(duplicate_status >= 0);
    assert_ne!(duplicate_status & libc::O_NONBLOCK, 0);
    rustix::fs::fcntl_setfl(source, status).unwrap();

    drop(cwd);
    fs::rename(&selected, &moved).unwrap();
    fs::create_dir(&selected).unwrap();
    fs::write(selected.join("marker"), b"replacement").unwrap();
    let replacement = File::open(&selected).unwrap();
    assert_eq!(
        replacement.as_raw_fd(),
        original_fd,
        "reuse the dropped owner's slot before spawning",
    );
    command.env(CLOSED_FD, hook_fd.to_string());
    let retained = descriptors();
    for _ in 0..2 {
        assert_success(&mut command);
        assert_eq!(descriptors(), retained);
        assert_eq!(descriptor_flags(hook_fd).unwrap(), libc::FD_CLOEXEC);
    }
    drop(command);
    assert_closed(hook_fd);
}

fn unspawned_drop() {
    let root = Directory::new();
    let baseline = descriptors();
    let cwd = PinnedWorkingDirectoryV3::open(&root.0).unwrap();
    let original_fd = cwd.native_source().unwrap().as_raw_fd();
    let mut command = receiver(&root.0, "fallback");
    let before = descriptors();
    cwd.configure_child_fchdir(&mut command).unwrap();
    let hook_fd = registered_descriptor(&before);
    drop(cwd);
    assert_closed(original_fd);
    assert_eq!(descriptor_flags(hook_fd).unwrap(), libc::FD_CLOEXEC);
    drop(command);
    assert_eq!(descriptors(), baseline);
}

fn duplicate_failure() {
    let root = Directory::new();
    let cwd = PinnedWorkingDirectoryV3::open(&root.0.join("selected")).unwrap();
    let mut command = receiver(&root.0, "fallback");
    command.current_dir(&root.0);
    let baseline = descriptors();
    let mut occupied = Vec::with_capacity(FD_LIMIT as usize);
    loop {
        match rustix::io::fcntl_dupfd_cloexec(cwd.native_source().unwrap(), 3) {
            Ok(fd) => occupied.push(fd),
            Err(error) => {
                assert_eq!(error, rustix::io::Errno::MFILE);
                break;
            }
        }
    }
    let full = descriptors();
    assert_eq!(
        cwd.configure_child_fchdir(&mut command)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EMFILE),
    );
    assert_eq!(descriptors(), full);
    drop(occupied);
    assert_eq!(descriptors(), baseline);
    drop(cwd);
    assert_success(&mut command);
}

fn invalid_source() {
    let root = Directory::new();
    let cwd = PinnedWorkingDirectoryV3::open(&root.0.join("selected")).unwrap();
    let mut command = receiver(&root.0, "fallback");
    command.current_dir(&root.0);
    let before = descriptors();
    let source = cwd.native_source().unwrap();
    rustix::io::fcntl_setfd(source, FdFlags::empty()).unwrap();
    assert_eq!(
        cwd.configure_child_fchdir(&mut command)
            .unwrap_err()
            .raw_os_error(),
        Some(libc::ESTALE),
    );
    assert_eq!(descriptors(), before);
    rustix::io::fcntl_setfd(source, FdFlags::CLOEXEC).unwrap();
    drop(cwd);
    assert_success(&mut command);
}

fn child_refusal() {
    let root = Directory::new();
    let selected = root.0.join("selected");
    let cwd = PinnedWorkingDirectoryV3::open(&selected).unwrap();
    let mut command = receiver(&selected, "original");
    let before = descriptors();
    cwd.configure_child_fchdir(&mut command).unwrap();
    let hook_fd = registered_descriptor(&before);
    drop(cwd);
    let original_mode = fs::metadata(&selected).unwrap().mode();
    fs::set_permissions(&selected, fs::Permissions::from_mode(original_mode ^ 0o001)).unwrap();
    let retained = descriptors();
    assert_eq!(
        command.output().unwrap_err().raw_os_error(),
        Some(libc::ESTALE),
    );
    assert_eq!(descriptors(), retained);
    assert_eq!(descriptor_flags(hook_fd).unwrap(), libc::FD_CLOEXEC);
    fs::set_permissions(&selected, fs::Permissions::from_mode(original_mode)).unwrap();
    assert_success(&mut command);
    drop(command);
    assert_closed(hook_fd);
}

struct FileLimit(libc::rlimit);
impl FileLimit {
    fn lower() -> Self {
        let mut original = MaybeUninit::uninit();
        // SAFETY: successful getrlimit initializes the complete output record.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, original.as_mut_ptr()) },
            0,
        );
        // SAFETY: getrlimit succeeded.
        let original = unsafe { original.assume_init() };
        assert!(original.rlim_cur >= FD_LIMIT as libc::rlim_t);
        let lowered = libc::rlimit {
            rlim_cur: FD_LIMIT as libc::rlim_t,
            rlim_max: original.rlim_max,
        };
        // SAFETY: only this exact-test child changes its limit; the hard limit is unchanged.
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &lowered) }, 0);
        Self(original)
    }
}
impl Drop for FileLimit {
    fn drop(&mut self) {
        // SAFETY: restore this isolated child's original limit.
        if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &self.0) } != 0 {
            std::process::abort();
        }
    }
}
