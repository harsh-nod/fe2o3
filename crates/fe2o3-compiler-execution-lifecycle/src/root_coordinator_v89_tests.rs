use super::*;
use std::os::unix::fs::{PermissionsExt, chown, symlink};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn directory(path: &Path, mode: u32) {
    std::fs::create_dir(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn state_fixture() -> (tempfile::TempDir, File, File) {
    let root = tempfile::tempdir().unwrap();
    directory(&root.path().join("compiler-execution"), 0o700);
    let parent = File::open(root.path()).unwrap();
    let state = File::open(root.path().join("compiler-execution")).unwrap();
    (root, parent, state)
}

#[test]
fn canonical_root_join_is_exact_and_repeated() {
    let (_root, parent, state) = state_fixture();
    let first = check_root(&parent, &state, "compiler-execution").unwrap();
    assert_eq!(
        first,
        check_root(&parent, &state, "compiler-execution").unwrap()
    );
    assert!(check_root(&parent, &state, "external-anchor").is_err());
    assert_eq!(
        CompilerExecutionLifecycleRootV89::Supervisor.name(),
        "compiler-execution"
    );
    assert_eq!(
        CompilerExecutionLifecycleRootV89::ExternalAnchor.name(),
        "external-anchor"
    );
}

#[test]
fn canonical_root_join_refuses_replacement_symlink_and_deleted_name() {
    let (root, parent, state) = state_fixture();
    let path = root.path().join("compiler-execution");
    std::fs::rename(&path, root.path().join("old")).unwrap();
    directory(&path, 0o700);
    assert!(matches!(
        check_root(&parent, &state, "compiler-execution"),
        Err(LifecycleLeaseErrorV1::PathChanged)
    ));
    std::fs::remove_dir(&path).unwrap();
    symlink("old", &path).unwrap();
    assert!(check_root(&parent, &state, "compiler-execution").is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(check_root(&parent, &state, "compiler-execution").is_err());
    std::fs::rename(root.path().join("old"), &path).unwrap();
    check_root(&parent, &state, "compiler-execution").unwrap();
}

#[test]
fn canonical_root_join_refuses_wrong_mode_and_path_only_inherited_fd() {
    let (root, parent, state) = state_fixture();
    let path = root.path().join("compiler-execution");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        check_root(&parent, &state, "compiler-execution"),
        Err(LifecycleLeaseErrorV1::InvalidParent)
    ));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path_only = openat(
        &parent,
        "compiler-execution",
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    assert!(matches!(
        check_root(&parent, &path_only, "compiler-execution"),
        Err(LifecycleLeaseErrorV1::InvalidParent)
    ));
    check_root(&parent, &state, "compiler-execution").unwrap();
}

#[test]
#[ignore = "requires explicit root opt-in; child chroots to a fresh owned fixture and drops DAC capabilities"]
fn root_coordinator_without_dac_uses_exact_canonical_parent() {
    assert_eq!(
        std::env::var("FE2O3_RUN_ROOT_LIFECYCLE_V89").as_deref(),
        Ok("1")
    );
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    if let Some(path) = std::env::var_os("FE2O3_ROOT_LIFECYCLE_CHILD_V89") {
        root_child(Path::new(&path));
        return;
    }
    let fixture = tempfile::tempdir().unwrap();
    std::fs::set_permissions(fixture.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    for name in ["var", "var/lib", "var/lib/fe2o3"] {
        directory(&fixture.path().join(name), 0o755);
    }
    for name in ["compiler-execution", "external-anchor"] {
        let path = fixture.path().join("var/lib/fe2o3").join(name);
        directory(&path, 0o700);
        chown(&path, Some(65534), Some(65534)).unwrap();
    }
    let lock = fixture
        .path()
        .join("var/lib/fe2o3/compiler-execution-lifecycle-v1");
    std::fs::write(&lock, []).unwrap();
    std::fs::set_permissions(
        &lock,
        std::fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
    )
    .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "root_coordinator_v89::tests::root_coordinator_without_dac_uses_exact_canonical_parent",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_RUN_ROOT_LIFECYCLE_V89", "1")
        .env("FE2O3_ROOT_LIFECYCLE_CHILD_V89", fixture.path())
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("root lifecycle child exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn root_child(path: &Path) {
    let supervisor = File::open(path.join("var/lib/fe2o3/compiler-execution")).unwrap();
    let anchor = File::open(path.join("var/lib/fe2o3/external-anchor")).unwrap();
    assert_eq!(state_snapshot(&supervisor).unwrap().object.uid, 65534);
    rustix::process::chroot(path).unwrap();
    rustix::process::chdir("/").unwrap();
    use rustix::thread::{CapabilitySet, CapabilitySets};
    let exact = CapabilitySet::CHOWN
        | CapabilitySet::KILL
        | CapabilitySet::SETGID
        | CapabilitySet::SETPCAP
        | CapabilitySet::SETUID;
    rustix::thread::set_capabilities(
        None,
        CapabilitySets {
            effective: exact,
            permitted: exact,
            inheritable: CapabilitySet::empty(),
        },
    )
    .unwrap();
    let caps = rustix::thread::capabilities(None).unwrap();
    assert_eq!(caps.effective, exact);
    assert_eq!(caps.permitted, exact);
    assert!(caps.inheritable.is_empty());
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    for state in [&supervisor, &anchor] {
        let old = openat(
            state,
            "..",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        );
        assert_eq!(old.unwrap_err(), rustix::io::Errno::ACCESS);
    }
    let open = |state: &File, role| {
        CompilerExecutionServiceLifecycleLeaseV1::open_for_root_coordinator_v89(state, role)
    };
    let first = open(&supervisor, CompilerExecutionLifecycleRootV89::Supervisor).unwrap();
    let second = open(&anchor, CompilerExecutionLifecycleRootV89::ExternalAnchor).unwrap();
    first.revalidate().unwrap();
    second.revalidate().unwrap();
    let lock = File::open(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1).unwrap();
    // An explicit test-only unlock must not release the other independently opened lease.
    flock(&first, FlockOperation::Unlock).unwrap();
    assert_eq!(
        flock(&lock, FlockOperation::NonBlockingLockExclusive).unwrap_err(),
        rustix::io::Errno::WOULDBLOCK
    );
    first.revalidate().unwrap();
    assert_eq!(
        flock(&lock, FlockOperation::NonBlockingLockExclusive).unwrap_err(),
        rustix::io::Errno::WOULDBLOCK
    );
    drop(first);
    assert_eq!(
        flock(&lock, FlockOperation::NonBlockingLockExclusive).unwrap_err(),
        rustix::io::Errno::WOULDBLOCK
    );
    drop(second);
    flock(&lock, FlockOperation::NonBlockingLockExclusive).unwrap();
    assert!(matches!(
        open(&supervisor, CompilerExecutionLifecycleRootV89::Supervisor),
        Err(LifecycleLeaseErrorV1::Busy)
    ));
    drop(lock);
    let path = Path::new("/var/lib/fe2o3/compiler-execution");
    std::fs::rename(path, "/var/lib/fe2o3/displaced").unwrap();
    directory(path, 0o700);
    chown(path, Some(65534), Some(65534)).unwrap();
    assert!(matches!(
        open(&supervisor, CompilerExecutionLifecycleRootV89::Supervisor),
        Err(LifecycleLeaseErrorV1::PathChanged)
    ));
    std::fs::remove_dir(path).unwrap();
    symlink("displaced", path).unwrap();
    assert!(open(&supervisor, CompilerExecutionLifecycleRootV89::Supervisor).is_err());
    std::fs::remove_file(path).unwrap();
    assert!(open(&supervisor, CompilerExecutionLifecycleRootV89::Supervisor).is_err());
    std::fs::rename("/var/lib/fe2o3/displaced", path).unwrap();
    open(&supervisor, CompilerExecutionLifecycleRootV89::Supervisor)
        .unwrap()
        .revalidate()
        .unwrap();
    std::fs::rename("/var/lib/fe2o3", "/var/lib/displaced-parent").unwrap();
    directory(Path::new("/var/lib/fe2o3"), 0o755);
    assert!(open(&supervisor, CompilerExecutionLifecycleRootV89::Supervisor).is_err());
}
