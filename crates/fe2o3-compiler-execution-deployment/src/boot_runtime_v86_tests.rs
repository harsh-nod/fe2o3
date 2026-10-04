use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::*;

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
    let run = scratch.path().join("run");
    let runtime = run.join("fe2o3");
    fs::create_dir(&run).unwrap();
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o755)).unwrap();
    let root = File::open(scratch.path()).unwrap();
    let retained = MachineRuntimeDirectoryV86::open(&root, (0, 0)).unwrap();
    let inherited = inherit_exec_descriptor(&retained.directory, 12).unwrap();
    retained.revalidate(&root, (0, 0)).unwrap();
    let identity = QualificationMachineIdentityV1::from_staging_name(
        ".compiler-execution-qualification-v1-0123456789abcdef0123456789abcdef",
    )
    .unwrap();
    let base_number = inherited.as_raw_fd() + 1;
    let root_number = inherited.as_raw_fd() + 2;
    let plan =
        pinned_systemd_nspawn_plan_v1(base_number, root_number, inherited.as_raw_fd(), &identity)
            .unwrap();
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

    // Match nspawn's ordering: mount_all(/run tmpfs), then mount_custom(bind).
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
    assert!(MachineRuntimeDirectoryV86::open(&root, (0, 0)).is_err()); // NO_XDEV remains closed.
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
        &SocketAddrUnix::new(runtime.join(socket_name)).unwrap(),
    )
    .unwrap();
    let through_bound = fs::metadata(runtime.join(socket_name)).unwrap();
    let through_retained = File::open(format!("/proc/self/fd/{}", inherited.as_raw_fd())).unwrap();
    let retained_socket = openat2(
        &through_retained,
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
    drop((retained_socket, through_retained, listener, bound));
    unmount(&runtime, UnmountFlags::empty()).unwrap();
    mounts.0.pop();
    unmount(&run, UnmountFlags::empty()).unwrap();
    mounts.0.pop();
    let parent_visible = fs::metadata(runtime.join(socket_name)).unwrap();
    assert_eq!(
        (parent_visible.dev(), parent_visible.ino()),
        (same.st_dev, same.st_ino)
    );
    fs::remove_file(runtime.join(socket_name)).unwrap();
    drop((inherited, retained, root));
    unmount(scratch.path(), UnmountFlags::empty()).unwrap();
    mounts.0.pop();
    assert!(mounts.0.is_empty());
}
