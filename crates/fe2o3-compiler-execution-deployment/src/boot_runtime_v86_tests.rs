use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::*;

const STAGING_NAME: &str = ".compiler-execution-qualification-v1-0123456789abcdef0123456789abcdef";

fn staged_fixture(parent: &Path) -> (PathBuf, File, (u32, u32)) {
    let stage = parent.join(STAGING_NAME);
    fs::create_dir(&stage).unwrap();
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).unwrap();
    for child in STAGING_CHILDREN_V86 {
        fs::create_dir(stage.join(child)).unwrap();
        fs::set_permissions(stage.join(child), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let root_path = stage.join("root");
    fs::create_dir(root_path.join("run")).unwrap();
    fs::create_dir(root_path.join(RUNTIME_DIRECTORY_V86)).unwrap();
    fs::set_permissions(
        root_path.join(RUNTIME_DIRECTORY_V86),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let root = File::open(root_path).unwrap();
    let stat = fstat(&root).unwrap();
    (stage, root, (stat.st_uid, stat.st_gid))
}

#[test]
fn machine_runtime_alias_capture_binds_only_exact_owned_staging_and_empty_target() {
    for mutant in 0..11 {
        let scratch = tempfile::tempdir().unwrap();
        let (stage, root, owner) = staged_fixture(scratch.path());
        MachineRuntimeSourceV86::capture(&root, STAGING_NAME, owner).unwrap();
        match mutant {
            0 => fs::set_permissions(&stage, fs::Permissions::from_mode(0o755)).unwrap(),
            1 => fs::write(stage.join("run/foreign"), b"x").unwrap(),
            2 => fs::set_permissions(stage.join("run"), fs::Permissions::from_mode(0o755)).unwrap(),
            3 => {
                fs::remove_dir(stage.join("run")).unwrap();
                std::os::unix::fs::symlink("evidence", stage.join("run")).unwrap();
            }
            4 => fs::create_dir(stage.join("unexpected")).unwrap(),
            5 => fs::remove_dir(stage.join("state")).unwrap(),
            6 => rustix::fs::fsetxattr(
                File::open(stage.join("run")).unwrap(),
                "user.unexpected",
                b"x",
                rustix::fs::XattrFlags::CREATE,
            )
            .unwrap(),
            7 => fs::write(stage.join("state/foreign"), b"x").unwrap(),
            8 => {
                fs::set_permissions(stage.join("state"), fs::Permissions::from_mode(0o755)).unwrap()
            }
            9 => {
                fs::remove_dir(stage.join("state")).unwrap();
                std::os::unix::fs::symlink("evidence", stage.join("state")).unwrap();
            }
            10 => rustix::fs::fsetxattr(
                File::open(stage.join("state")).unwrap(),
                "user.unexpected",
                b"x",
                rustix::fs::XattrFlags::CREATE,
            )
            .unwrap(),
            _ => unreachable!(),
        }
        assert!(
            MachineRuntimeSourceV86::capture(&root, STAGING_NAME, owner).is_err(),
            "mutant {mutant}"
        );
    }
}

#[test]
fn machine_image_alias_refuses_mounting_inside_its_captured_parent_namespace() {
    let scratch = tempfile::tempdir().unwrap();
    let (stage, root, owner) = staged_fixture(scratch.path());
    let stage_file = File::open(&stage).unwrap();
    let state = File::open(stage.join("state")).unwrap();
    let before = exact_snapshot(&state).unwrap();
    let source = image_v160::MachineImageSourceV160::capture(&stage_file, owner).unwrap();
    assert!(source.attach(&stage, &root, owner).is_err());
    assert_eq!(exact_snapshot(&state).unwrap(), before);
    assert_eq!(fs::read_dir(stage.join("state")).unwrap().count(), 0);
}

#[test]
fn machine_runtime_bind_source_refuses_alias_syntax_and_other_transactions() {
    let identity = QualificationMachineIdentityV1::from_staging_name(STAGING_NAME).unwrap();
    let valid = PathBuf::from("/qualification")
        .join(STAGING_NAME)
        .join("run");
    checked_bind_source(&valid, &identity).unwrap();
    for invalid in [
        valid.strip_prefix("/").unwrap().to_path_buf(),
        valid.join(".."),
        valid.with_file_name("root"),
        PathBuf::from("/qualification:other")
            .join(STAGING_NAME)
            .join("run"),
        PathBuf::from("/qualification\nother")
            .join(STAGING_NAME)
            .join("run"),
        PathBuf::from("/qualification")
            .join(".compiler-execution-qualification-v1-0123456789abcdef0123456789abcde0")
            .join("run"),
    ] {
        assert!(
            checked_bind_source(&invalid, &identity).is_err(),
            "{invalid:?}"
        );
    }
}

fn fixture() -> (tempfile::TempDir, File, (u32, u32)) {
    let scratch = tempfile::tempdir().unwrap();
    fs::create_dir(scratch.path().join("run")).unwrap();
    fs::create_dir(scratch.path().join(RUNTIME_DIRECTORY_V86)).unwrap();
    fs::set_permissions(
        scratch.path().join(RUNTIME_DIRECTORY_V86),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let root = File::open(scratch.path()).unwrap();
    let stat = fstat(&root).unwrap();
    (scratch, root, (stat.st_uid, stat.st_gid))
}

#[test]
fn machine_runtime_admission_retains_exact_empty_directory_and_exec_identity() {
    let (_scratch, root, owner) = fixture();
    let retained = MachineRuntimeDirectoryV86::open(&root, owner).unwrap();
    let inherited = inherit_exec_descriptor(&retained.directory, 12).unwrap();
    assert_eq!(snapshot(&fstat(&inherited).unwrap()), retained.snapshot);
    assert!(fcntl_getfd(&inherited).unwrap().is_empty());
    retained.revalidate(&root, owner).unwrap();
}

#[test]
fn machine_runtime_admission_refuses_missing_noncanonical_or_nonempty_sources() {
    for mutant in 0..9 {
        let (scratch, root, mut owner) = fixture();
        let path = scratch.path().join(RUNTIME_DIRECTORY_V86);
        match mutant {
            0 => fs::remove_dir(&path).unwrap(),
            1 => {
                fs::remove_dir(&path).unwrap();
                fs::write(&path, b"").unwrap();
            }
            2 => {
                fs::remove_dir(&path).unwrap();
                std::os::unix::fs::symlink("..", &path).unwrap();
            }
            3 => fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap(),
            4 => owner.0 = owner.0.wrapping_add(1),
            5 => owner.1 = owner.1.wrapping_add(1),
            6 => rustix::fs::fsetxattr(
                File::open(&path).unwrap(),
                "user.unexpected",
                b"",
                rustix::fs::XattrFlags::CREATE,
            )
            .unwrap(),
            7 => fs::write(path.join("preexisting-report"), b"").unwrap(),
            8 => fs::create_dir(path.join("preexisting-directory")).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            MachineRuntimeDirectoryV86::open(&root, owner).is_err(),
            "mutant {mutant}"
        );
    }
}

#[test]
fn machine_runtime_revalidation_refuses_replaced_named_directory() {
    let (scratch, root, owner) = fixture();
    let retained = MachineRuntimeDirectoryV86::open(&root, owner).unwrap();
    let path = scratch.path().join(RUNTIME_DIRECTORY_V86);
    fs::rename(&path, scratch.path().join("displaced")).unwrap();
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let replacement = File::open(&path).unwrap();
    let replacement = snapshot(&fstat(&replacement).unwrap());
    assert_ne!(
        (replacement.device, replacement.inode),
        (retained.snapshot.device, retained.snapshot.inode)
    );
    assert!(retained.revalidate(&root, owner).is_err());
    assert_eq!(fstat(&retained.directory).unwrap().st_nlink, 2);
}

#[test]
fn machine_runtime_revalidation_refuses_new_attributes_contents_and_metadata() {
    for mutant in 0..3 {
        let (scratch, root, owner) = fixture();
        let retained = MachineRuntimeDirectoryV86::open(&root, owner).unwrap();
        match mutant {
            0 => rustix::fs::fsetxattr(
                &retained.directory,
                "user.unexpected",
                b"y",
                rustix::fs::XattrFlags::CREATE,
            )
            .unwrap(),
            1 => fs::write(
                scratch
                    .path()
                    .join(RUNTIME_DIRECTORY_V86)
                    .join("unexpected"),
                b"x",
            )
            .unwrap(),
            2 => rustix::fs::fchmod(&retained.directory, Mode::from_raw_mode(0o775)).unwrap(),
            _ => unreachable!(),
        }
        let before = snapshot(&fstat(&retained.directory).unwrap());
        assert!(retained.revalidate(&root, owner).is_err());
        assert_eq!(snapshot(&fstat(&retained.directory).unwrap()), before);
    }
}

#[test]
#[ignore = "requires a dedicated disposable mount/PID namespace with CAP_SYS_ADMIN"]
fn qualification_machine_runtime_bind_survives_child_run_overmount() {
    use rustix::mount::{
        FsMountFlags, FsOpenFlags, MountAttrFlags, MountPropagationFlags, MoveMountFlags,
        UnmountFlags, fsconfig_create, fsconfig_set_string, fsmount, fsopen, mount_bind,
        mount_change, move_mount, unmount,
    };
    use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType, bind, socket_with};

    assert_eq!(
        std::env::var("FE2O3_PRIVATE_MOUNT_REGRESSION_V72").as_deref(),
        Ok("1")
    );
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWNS).unwrap();
    mount_change(
        "/",
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )
    .unwrap();
    let scratch = tempfile::tempdir().unwrap();
    struct Mounts(Vec<PathBuf>);
    impl Drop for Mounts {
        fn drop(&mut self) {
            // Panic-only fallback in the disposable namespace, never a success condition.
            for path in self.0.iter().rev() {
                let _ = unmount(path, UnmountFlags::DETACH);
            }
        }
    }
    let mut mounts = Mounts(Vec::new());
    let attach_tmpfs = |path: &Path| {
        let context = fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
        fsconfig_set_string(&context, "size", "4m").unwrap();
        fsconfig_create(&context).unwrap();
        let detached = fsmount(
            &context,
            FsMountFlags::FSMOUNT_CLOEXEC,
            MountAttrFlags::MOUNT_ATTR_NODEV | MountAttrFlags::MOUNT_ATTR_NOSUID,
        )
        .unwrap();
        let target = File::open(path).unwrap();
        move_mount(
            &detached,
            "",
            &target,
            "",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )
        .unwrap();
    };
    attach_tmpfs(scratch.path());
    mounts.0.push(scratch.path().to_owned());
    let (stage_path, root, owner) = staged_fixture(scratch.path());
    let base = File::open(stage_path.join("base")).unwrap();
    let run = stage_path.join("root/run");
    let runtime = run.join("fe2o3");
    let source = MachineRuntimeSourceV86::capture(&root, STAGING_NAME, owner).unwrap();
    let target_before = source.target_snapshot;
    let state_before = exact_snapshot(&File::open(stage_path.join("state")).unwrap()).unwrap();
    let image_path = stage_path.join("state/root");
    let image_state_path = stage_path.join("state");
    let retained = MachineRuntimeDirectoryV86::open(&root, owner).unwrap();
    let outer_namespace = File::open("/proc/thread-self/ns/mnt").unwrap();
    let enter = || {
        rustix::thread::unshare(rustix::thread::UnshareFlags::NEWNS).unwrap();
        mount_change(
            "/",
            MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
        )
        .unwrap();
    };
    use std::os::fd::AsFd as _;
    let restore = |namespace: &File| {
        rustix::thread::move_into_link_name_space(
            namespace.as_fd(),
            Some(rustix::thread::LinkNameSpaceType::Mount),
        )
        .unwrap();
    };
    enter();
    let helper_namespace = File::open("/proc/thread-self/ns/mnt").unwrap();
    assert_ne!(
        fstat(&helper_namespace).unwrap().st_ino,
        fstat(&outer_namespace).unwrap().st_ino
    );
    let alias = source.attach(&base, &root, owner).unwrap();
    let alias_path = alias.path().to_owned();
    alias.revalidate().unwrap();
    assert_eq!(
        exact_snapshot(alias.image_root()).unwrap(),
        exact_snapshot(&root).unwrap()
    );
    assert_eq!(
        fs::canonicalize(format!("/proc/self/fd/{}", alias.image_root().as_raw_fd())).unwrap(),
        image_path,
    );
    let lock_path = image_state_path.join(".#root.lck");
    fs::write(&lock_path, b"").unwrap();
    assert!(alias.revalidate().is_err());
    restore(&outer_namespace);
    assert!(!lock_path.exists());
    assert!(!image_path.exists());
    assert_eq!(
        exact_snapshot(&File::open(&image_state_path).unwrap()).unwrap(),
        state_before
    );
    super::super::super::verify_directory_children(
        &File::open(&stage_path).unwrap(),
        STAGING_CHILDREN_V86,
        "parent staging while image lock exists",
    )
    .unwrap();
    restore(&helper_namespace);
    assert!(lock_path.is_file());
    let identity = QualificationMachineIdentityV1::from_staging_name(STAGING_NAME).unwrap();
    let plan = pinned_systemd_nspawn_plan_v1(10, 11, &alias_path, &identity).unwrap();
    let bind_option = plan
        .arguments()
        .iter()
        .find(|arg| arg.starts_with("--bind="))
        .unwrap();
    let source = bind_option
        .strip_prefix("--bind=")
        .unwrap()
        .strip_suffix(":/run/fe2o3:norbind,noidmap")
        .unwrap();

    // Match both boundaries: the helper alias is attached before nspawn's NEWNS;
    // mount_all(/run tmpfs) then precedes mount_custom(bind) in the latter.
    enter();
    attach_tmpfs(&run);
    mounts.0.push(run.clone());
    assert_eq!(
        fs::metadata(&runtime).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    let negative = scratch.path().join("negative-bind");
    fs::create_dir(&negative).unwrap();
    assert_eq!(
        mount_bind(&runtime, &negative),
        Err(rustix::io::Errno::NOENT)
    );
    fs::remove_dir(&negative).unwrap();
    fs::create_dir(&runtime).unwrap();
    let current_root = absolute_directory(&stage_path.join("root")).unwrap();
    assert!(MachineRuntimeDirectoryV86::open(&current_root, owner).is_err()); // NO_XDEV remains closed.
    assert_eq!(
        mount_bind(
            format!("/proc/self/fd/{}", retained.directory.as_raw_fd()),
            &runtime
        ),
        Err(rustix::io::Errno::INVAL),
    );
    mount_bind(source, &runtime).unwrap();
    mounts.0.push(runtime.clone());
    let bound = File::open(&runtime).unwrap();
    assert_eq!(snapshot(&fstat(&bound).unwrap()), retained.snapshot);
    let socket_name = "supervisor.sock";
    let listener = socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    bind(
        &listener,
        // The canonical staging name exceeds sockaddr_un's short path budget.
        &SocketAddrUnix::new(format!("/proc/self/fd/{}/{socket_name}", bound.as_raw_fd())).unwrap(),
    )
    .unwrap();
    let through_bound = fs::metadata(runtime.join(socket_name)).unwrap();
    let retained_socket = openat2(
        &retained.directory,
        socket_name,
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
    .unwrap();
    use std::os::unix::fs::MetadataExt as _;
    let same = fstat(&retained_socket).unwrap();
    assert_eq!(
        (through_bound.dev(), through_bound.ino()),
        (same.st_dev, same.st_ino)
    );
    drop((retained_socket, listener, bound, current_root));
    unmount(&runtime, UnmountFlags::empty()).unwrap();
    mounts.0.pop();
    unmount(&run, UnmountFlags::empty()).unwrap();
    mounts.0.pop();
    unmount(&alias_path, UnmountFlags::empty()).unwrap();
    unmount(&image_path, UnmountFlags::empty()).unwrap();
    unmount(&image_state_path, UnmountFlags::empty()).unwrap();
    unmount(scratch.path(), UnmountFlags::empty()).unwrap();
    restore(&helper_namespace);
    drop(alias);
    unmount(&alias_path, UnmountFlags::empty()).unwrap();
    unmount(&image_path, UnmountFlags::empty()).unwrap();
    unmount(&image_state_path, UnmountFlags::empty()).unwrap();
    unmount(scratch.path(), UnmountFlags::empty()).unwrap();
    restore(&outer_namespace);
    let unchanged_target = absolute_directory(&alias_path).unwrap();
    assert_eq!(exact_snapshot(&unchanged_target).unwrap(), target_before);
    super::super::super::verify_directory_children(&unchanged_target, &[], "parent alias target")
        .unwrap();
    assert_eq!(
        exact_snapshot(&File::open(&image_state_path).unwrap()).unwrap(),
        state_before
    );
    assert_eq!(fs::read_dir(&image_state_path).unwrap().count(), 0);
    let parent_visible = fs::metadata(runtime.join(socket_name)).unwrap();
    assert_eq!(
        (parent_visible.dev(), parent_visible.ino()),
        (same.st_dev, same.st_ino)
    );
    fs::remove_file(runtime.join(socket_name)).unwrap();
    drop((
        unchanged_target,
        retained,
        root,
        base,
        helper_namespace,
        outer_namespace,
    ));
    unmount(scratch.path(), UnmountFlags::empty()).unwrap();
    mounts.0.pop();
    assert!(mounts.0.is_empty());
}
