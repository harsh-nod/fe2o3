fn installed_namespace_inventory(
    installed: &install::InstalledCompilerExecutionDeploymentV1,
) -> [(i32, ObjectSnapshotV1); 2] {
    use std::os::fd::AsRawFd as _;
    installed.namespace_descriptors_for_test().map(|file| {
        (
            file.as_raw_fd(),
            snapshot(&rustix::fs::fstat(file).unwrap()),
        )
    })
}

#[test]
fn installed_namespace_refresh_preserves_all_descriptors_and_sealed_projection() {
    let (mut installed, _parent) = installed_for_qualification();
    let before = installed_namespace_inventory(&installed);
    let identities = (
        installed.manifest_sha256(),
        installed.git_commit().to_owned(),
        installed.root_name().to_owned(),
        installed.publication(),
    );
    installed
        .refresh_mount_namespace_descriptors(current_owner())
        .unwrap();
    let after = installed_namespace_inventory(&installed);
    for ((old_fd, old), (new_fd, new)) in before.iter().zip(after) {
        assert_ne!(*old_fd, new_fd);
        assert_eq!(*old, new);
    }
    assert_eq!(
        identities,
        (
            installed.manifest_sha256(),
            installed.git_commit().to_owned(),
            installed.root_name().to_owned(),
            installed.publication()
        )
    );
    install::revalidate_installed_deployment_for_test_v1(&installed, current_owner()).unwrap();
}

#[test]
fn installed_namespace_refresh_refuses_parent_replacement_and_symlink_without_transfer() {
    for symlink_parent in [false, true] {
        let (mut installed, parent) = installed_for_qualification();
        let before = installed_namespace_inventory(&installed);
        let displaced = parent.path().with_extension("displaced-v77");
        fs::rename(parent.path(), &displaced).unwrap();
        if symlink_parent {
            symlink(&displaced, parent.path()).unwrap();
        } else {
            fs::create_dir(parent.path()).unwrap();
            fs::set_permissions(parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
        }
        let error = installed
            .refresh_mount_namespace_descriptors(current_owner())
            .unwrap_err();
        assert_eq!(
            error.kind(),
            if symlink_parent {
                DeploymentVerificationErrorKindV1::Io
            } else {
                DeploymentVerificationErrorKindV1::InputChanged
            }
        );
        assert_eq!(
            before.map(|value| value.0),
            installed_namespace_inventory(&installed).map(|value| value.0)
        );
        if symlink_parent {
            fs::remove_file(parent.path()).unwrap();
        } else {
            fs::remove_dir(parent.path()).unwrap();
        }
        fs::rename(displaced, parent.path()).unwrap();
    }
}

#[test]
fn installed_namespace_refresh_refuses_root_replacement_without_transfer() {
    for replacement in [0, 1, 2] {
        let (mut installed, parent) = installed_for_qualification();
        let before = installed_namespace_inventory(&installed);
        let root = parent.path().join(installed.root_name());
        let displaced = parent.path().join("displaced-v77");
        fs::rename(&root, &displaced).unwrap();
        if replacement == 0 {
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        } else if replacement == 2 {
            symlink(&displaced, &root).unwrap();
        }
        assert_eq!(
            installed
                .refresh_mount_namespace_descriptors(current_owner())
                .unwrap_err()
                .kind(),
            if replacement == 2 {
                DeploymentVerificationErrorKindV1::Io
            } else {
                DeploymentVerificationErrorKindV1::InputChanged
            }
        );
        assert_eq!(
            before.map(|value| value.0),
            installed_namespace_inventory(&installed).map(|value| value.0)
        );
        if replacement == 0 {
            fs::remove_dir(&root).unwrap();
        } else if replacement == 2 {
            fs::remove_file(&root).unwrap();
        }
        fs::rename(displaced, root).unwrap();
    }
}

#[test]
fn installed_namespace_refresh_keeps_original_content_mode_and_xattr_gates() {
    for mutation in [0, 1, 2] {
        let (mut installed, parent) = installed_for_qualification();
        let before = installed_namespace_inventory(&installed);
        let root = parent.path().join(installed.root_name());
        let expected = match mutation {
            0 => {
                let file = root.join("usr/share/fe2o3/compiler-execution/BUILD-INFO");
                let mut bytes = fs::read(&file).unwrap();
                bytes[0] ^= 1;
                fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
                fs::write(&file, bytes).unwrap();
                fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
                DeploymentVerificationErrorKindV1::ContentMismatch
            }
            1 => {
                fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
                DeploymentVerificationErrorKindV1::InvalidMetadata
            }
            _ => {
                rustix::fs::setxattr(&root, "user.namespace-v77", b"x", XattrFlags::empty())
                    .unwrap();
                DeploymentVerificationErrorKindV1::InvalidMetadata
            }
        };
        assert_eq!(
            installed
                .refresh_mount_namespace_descriptors(current_owner())
                .unwrap_err()
                .kind(),
            expected
        );
        assert_eq!(
            before.map(|value| value.0),
            installed_namespace_inventory(&installed).map(|value| value.0)
        );
    }
}

#[test]
fn staged_namespace_refresh_transfers_installed_lower_before_mount_attachment() {
    let (prepared, parent, _base, _install) = prepared_for_staging();
    let mut staged =
        staging::stage_compiler_execution_qualification_for_test_v1(prepared, current_owner())
            .unwrap();
    let before = installed_namespace_inventory(staged.prepared().installed());
    staging::refresh_mount_namespace_descriptors_for_test_v1(&mut staged, current_owner()).unwrap();
    for ((old_fd, old), (new_fd, new)) in before
        .iter()
        .zip(installed_namespace_inventory(staged.prepared().installed()))
    {
        assert_ne!(*old_fd, new_fd);
        assert_eq!(*old, new);
    }
    staged.cleanup().unwrap();
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
#[ignore = "requires a dedicated disposable mount/PID namespace with CAP_SYS_ADMIN"]
fn qualification_installed_lower_refresh_clones_only_current_namespace_mounts() {
    use rustix::mount::{
        FsMountFlags, FsOpenFlags, MountAttrFlags, MountPropagationFlags, MoveMountFlags,
        UnmountFlags, fsconfig_create, fsconfig_set_string, fsmount, fsopen, mount_change,
        move_mount, unmount,
    };
    use std::fs::File;
    use std::os::fd::AsRawFd as _;
    assert_eq!(
        std::env::var("FE2O3_PRIVATE_MOUNT_REGRESSION_V72").as_deref(),
        Ok("1")
    );
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let (prepared, parent, _base, _install) = prepared_for_staging();
    let mut staged =
        staging::stage_compiler_execution_qualification_for_test_v1(prepared, current_owner())
            .unwrap();
    let stale = staged
        .prepared()
        .installed()
        .retained_root()
        .try_clone()
        .unwrap();
    let before = installed_namespace_inventory(staged.prepared().installed());
    rustix::thread::unshare(rustix::thread::UnshareFlags::NEWNS).unwrap();
    mount_change(
        "/",
        MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
    )
    .unwrap();
    let scratch = tempfile::tempdir().unwrap();
    fs::create_dir(scratch.path().join("empty-lower")).unwrap();
    fs::create_dir(scratch.path().join("target")).unwrap();
    let lower = File::open(scratch.path().join("empty-lower")).unwrap();
    let overlay = |installed: &File| {
        let context = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
        fsconfig_set_string(
            &context,
            "lowerdir",
            format!(
                "/proc/self/fd/{}:/proc/self/fd/{}",
                installed.as_raw_fd(),
                lower.as_raw_fd()
            ),
        )
        .unwrap();
        context
    };
    let context = overlay(&stale);
    assert_eq!(fsconfig_create(&context), Err(rustix::io::Errno::INVAL));
    drop(context);
    staging::refresh_mount_namespace_descriptors_for_test_v1(&mut staged, current_owner()).unwrap();
    for ((old_fd, old), (new_fd, new)) in before
        .iter()
        .zip(installed_namespace_inventory(staged.prepared().installed()))
    {
        assert_ne!(*old_fd, new_fd);
        assert_eq!(*old, new);
    }
    let context = overlay(staged.prepared().installed().retained_root());
    fsconfig_create(&context).unwrap();
    let detached = fsmount(
        &context,
        FsMountFlags::FSMOUNT_CLOEXEC,
        MountAttrFlags::MOUNT_ATTR_RDONLY
            | MountAttrFlags::MOUNT_ATTR_NODEV
            | MountAttrFlags::MOUNT_ATTR_NOSUID,
    )
    .unwrap();
    let target_path = scratch.path().join("target");
    let target = File::open(&target_path).unwrap();
    move_mount(
        &detached,
        "",
        &target,
        "",
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
    )
    .unwrap();
    struct Attached(Option<PathBuf>);
    impl Drop for Attached {
        fn drop(&mut self) {
            if let Some(path) = &self.0 {
                let _ = rustix::mount::unmount(path, rustix::mount::UnmountFlags::DETACH);
            }
        }
    }
    let mut cleanup = Attached(Some(target_path.clone()));
    drop((detached, context, target));
    let composed = File::open(&target_path).unwrap();
    install::verify_installed_projection(&composed, staged.prepared().installed(), current_owner())
        .unwrap();
    drop(composed);
    unmount(&target_path, UnmountFlags::empty()).unwrap();
    cleanup.0 = None;
    let context = overlay(&stale);
    assert_eq!(fsconfig_create(&context), Err(rustix::io::Errno::INVAL));
    drop((context, stale, lower));
    staged.cleanup().unwrap();
    assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
}
