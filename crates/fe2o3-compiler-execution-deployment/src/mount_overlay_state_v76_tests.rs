#[test]
fn overlay_upper_mode_transition_is_exact_and_phase_specific() {
    let scratch = tempfile::tempdir().unwrap();
    let upper = File::open(scratch.path()).unwrap();
    fchmod(&upper, Mode::from_raw_mode(0o700)).unwrap();
    let before = snapshot(&fstat(&upper).unwrap());
    let owner = (before.uid, before.gid);
    assert!(validate_overlay_upper(&upper, owner, MountedRootStateV1::Pristine).is_err());
    prepare_overlay_upper(&upper, owner).unwrap();
    let after = snapshot(&fstat(&upper).unwrap());
    assert_eq!((after.device, after.inode), (before.device, before.inode));
    assert_eq!(after.mode & 0o7777, 0o755);
    let _proc_target = prepare_overlay_proc_directory(&upper, owner).unwrap();
    validate_overlay_upper(&upper, owner, MountedRootStateV1::Pristine).unwrap();
    validate_overlay_upper(&upper, owner, MountedRootStateV1::SystemdPreflight).unwrap();
    assert!(prepare_overlay_upper(&upper, owner).is_err());
    std::fs::write(scratch.path().join("copied-up"), b"fixture").unwrap();
    assert!(validate_overlay_upper(&upper, owner, MountedRootStateV1::Pristine).is_err());
    validate_overlay_upper(&upper, owner, MountedRootStateV1::SystemdPreflight).unwrap();
    for mode in [0o700, 0o750, 0o777, 0o1755] {
        fchmod(&upper, Mode::from_raw_mode(mode)).unwrap();
        assert!(
            validate_overlay_upper(&upper, owner, MountedRootStateV1::SystemdPreflight).is_err()
        );
    }
}

#[test]
fn overlay_upper_transition_refuses_nonpristine_input_without_chmod() {
    for mutant in 0..4 {
        let scratch = tempfile::tempdir().unwrap();
        let upper = File::open(scratch.path()).unwrap();
        fchmod(&upper, Mode::from_raw_mode(0o700)).unwrap();
        let observed = snapshot(&fstat(&upper).unwrap());
        let mut owner = (observed.uid, observed.gid);
        match mutant {
            0 => fchmod(&upper, Mode::from_raw_mode(0o755)).unwrap(),
            1 => std::fs::write(scratch.path().join("unexpected"), b"x").unwrap(),
            2 => owner.0 = owner.0.wrapping_add(1),
            3 => rustix::fs::fsetxattr(
                &upper,
                "user.unexpected",
                b"y",
                rustix::fs::XattrFlags::empty(),
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let before = snapshot(&fstat(&upper).unwrap());
        assert!(
            prepare_overlay_upper(&upper, owner).is_err(),
            "mutant {mutant}"
        );
        assert_eq!(snapshot(&fstat(&upper).unwrap()), before, "mutant {mutant}");
    }
}

#[test]
fn overlay_upper_rejects_user_attributes_in_both_mounted_phases() {
    let scratch = tempfile::tempdir().unwrap();
    let upper = File::open(scratch.path()).unwrap();
    fchmod(&upper, Mode::from_raw_mode(0o755)).unwrap();
    let observed = snapshot(&fstat(&upper).unwrap());
    let owner = (observed.uid, observed.gid);
    for name in [
        "user.x",
        "user.overlay.impure",
        "user.a-name-longer-than-the-exact-kernel-marker",
    ] {
        rustix::fs::fsetxattr(&upper, name, b"y", rustix::fs::XattrFlags::empty()).unwrap();
        assert!(validate_overlay_upper(&upper, owner, MountedRootStateV1::Pristine).is_err());
        assert!(
            validate_overlay_upper(&upper, owner, MountedRootStateV1::SystemdPreflight).is_err()
        );
        rustix::fs::fremovexattr(&upper, name).unwrap();
    }
}

#[test]
fn overlay_work_capture_refuses_missing_extra_wrong_mode_and_symlink() {
    for mutant in 0..4 {
        let scratch = tempfile::tempdir().unwrap();
        let work = File::open(scratch.path()).unwrap();
        fchmod(&work, Mode::from_raw_mode(0o700)).unwrap();
        let observed = snapshot(&fstat(&work).unwrap());
        let owner = (observed.uid, observed.gid);
        match mutant {
            0 => (),
            1 => {
                std::fs::create_dir(scratch.path().join("work")).unwrap();
                std::fs::create_dir(scratch.path().join("index")).unwrap();
            }
            2 => {
                std::fs::create_dir(scratch.path().join("work")).unwrap();
                fchmod(
                    File::open(scratch.path().join("work")).unwrap(),
                    Mode::from_raw_mode(0o700),
                )
                .unwrap();
            }
            3 => std::os::unix::fs::symlink(".", scratch.path().join("work")).unwrap(),
            _ => unreachable!(),
        }
        let before = snapshot(&fstat(&work).unwrap());
        assert!(open_overlay_work(&work, owner).is_err(), "mutant {mutant}");
        assert_eq!(snapshot(&fstat(&work).unwrap()), before, "mutant {mutant}");
    }
}

#[test]
fn overlay_metadata_profile_is_bound_before_attachment_and_released_before_staging() {
    let source = include_str!("mount.rs");
    let body = source
        .split_once("fn attach_overlay(")
        .unwrap()
        .1
        .split_once("#[derive(Clone, Copy)]")
        .unwrap()
        .0;
    let prepare = body.find("prepare_overlay_upper(").unwrap();
    let configure = body.find("configure_overlay_profile(&context)?;").unwrap();
    let create = body.find("fsconfig_create(&context)").unwrap();
    let capture = body.find("mounted.overlay_work = Some(").unwrap();
    let attach = body.find("move_mount(").unwrap();
    assert!(prepare < configure && configure < create && create < capture && capture < attach);
    let profile = source
        .split_once("fn configure_overlay_profile(")
        .unwrap()
        .1
        .split_once("fn prepare_overlay_upper(")
        .unwrap()
        .0;
    assert!(profile.contains("fsconfig_set_string(&context, \"uuid\", \"null\")"));
    assert!(profile.contains("fsconfig_set_string(&context, \"index\", \"off\")"));
    let cleanup = source
        .split_once("fn cleanup_internal_with_hooks(")
        .unwrap()
        .1
        .split_once("fn unmount_retained_child(")
        .unwrap()
        .0;
    assert!(
        cleanup.find("self.overlay_work.take();").unwrap()
            < cleanup.find(".staged\n            .take()").unwrap()
    );
}

#[test]
#[ignore = "requires a dedicated disposable mount/PID namespace with CAP_SYS_ADMIN and OverlayFS"]
fn qualification_overlay_state_preserves_exact_modes_work_custody_and_copyup_marker() {
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
    struct Mounts(Vec<std::path::PathBuf>);
    impl Drop for Mounts {
        fn drop(&mut self) {
            for path in self.0.iter().rev() {
                let _ = unmount(path, UnmountFlags::DETACH);
            }
        }
    }
    let mut cleanup = Mounts(Vec::new());
    let attributes = MountAttrFlags::MOUNT_ATTR_NODEV | MountAttrFlags::MOUNT_ATTR_NOSUID;
    let flags = MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH;
    let context = fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
    fsconfig_set_string(&context, "size", "16m").unwrap();
    fsconfig_create(&context).unwrap();
    let detached = fsmount(&context, FsMountFlags::FSMOUNT_CLOEXEC, attributes).unwrap();
    let target = File::open(scratch.path()).unwrap();
    move_mount(&detached, "", &target, "", flags).unwrap();
    cleanup.0.push(scratch.path().to_owned());
    drop((detached, context, target));
    for name in ["lower", "upper", "work", "root", "other"] {
        std::fs::create_dir(scratch.path().join(name)).unwrap();
        fchmod(
            File::open(scratch.path().join(name)).unwrap(),
            Mode::from_raw_mode(0o700),
        )
        .unwrap();
    }
    std::fs::create_dir(scratch.path().join("lower/etc")).unwrap();
    std::fs::create_dir(scratch.path().join("lower/proc")).unwrap();
    fchmod(
        File::open(scratch.path().join("lower/proc")).unwrap(),
        Mode::from_raw_mode(0o755),
    )
    .unwrap();
    std::fs::write(
        scratch.path().join("lower/etc/passwd"),
        b"root:x:0:0:root:/root:/bin/sh\n",
    )
    .unwrap();
    let lower = File::open(scratch.path().join("lower")).unwrap();
    let upper = File::open(scratch.path().join("upper")).unwrap();
    let work = File::open(scratch.path().join("work")).unwrap();
    prepare_overlay_upper(&upper, (0, 0)).unwrap();
    let proc_target = prepare_overlay_proc_target(&upper, (0, 0)).unwrap();
    let context = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC).unwrap();
    configure_overlay_profile(&context).unwrap();
    fsconfig_set_string(&context, "lowerdir", descriptor_path(&lower)).unwrap();
    fsconfig_set_string(&context, "upperdir", descriptor_path(&upper)).unwrap();
    fsconfig_set_string(&context, "workdir", descriptor_path(&work)).unwrap();
    fsconfig_create(&context).unwrap();
    let detached = fsmount(&context, FsMountFlags::FSMOUNT_CLOEXEC, attributes).unwrap();
    assert_eq!(fstat(&detached).unwrap().st_mode & 0o7777, 0o755);
    let kernel_work = open_overlay_work(&work, (0, 0)).unwrap();
    assert_eq!(fstat(&kernel_work).unwrap().st_mode & 0o7777, 0);
    assert!(verify_directory_children(&work, &[], "old empty work expectation").is_err());
    let root_path = scratch.path().join("root");
    let target = File::open(&root_path).unwrap();
    move_mount(&detached, "", &target, "", flags).unwrap();
    cleanup.0.push(root_path.clone());
    drop((detached, context, target));
    let root = File::open(&root_path).unwrap();
    validate_directory_mode(&root, Some((0, 0)), 0o755, "actual composed root").unwrap();
    validate_overlay_upper(&upper, (0, 0), MountedRootStateV1::Pristine).unwrap();
    revalidate_overlay_work(&work, &kernel_work, (0, 0)).unwrap();
    revalidate_overlay_proc_target(&upper, &proc_target, (0, 0), OverlayProcProfileV82::Opaque)
        .unwrap();
    let composed_proc = super::super::open_beneath(&root, "proc", true).unwrap();
    validate_directory_mode(&composed_proc, Some((0, 0)), 0o755, "actual proc target").unwrap();
    verify_directory_children(&composed_proc, &[], "actual proc target").unwrap();
    drop(composed_proc);
    assert!(validate_directory_mode(&upper, Some((0, 0)), 0o700, "old upper expectation").is_err());

    std::fs::write(
        root_path.join("etc/passwd"),
        b"root:x:0:0:root:/root:/bin/sh\nservice:x:900:900::/:/bin/false\n",
    )
    .unwrap();
    assert!(validate_overlay_upper(&upper, (0, 0), MountedRootStateV1::Pristine).is_err());
    validate_overlay_upper(&upper, (0, 0), MountedRootStateV1::SystemdPreflight).unwrap();
    revalidate_overlay_proc_target(&upper, &proc_target, (0, 0), OverlayProcProfileV82::Opaque)
        .unwrap();
    let mut marker = [0u8; 2];
    assert_eq!(
        fgetxattr(&upper, "trusted.overlay.impure", &mut marker).unwrap(),
        1
    );
    assert_eq!(marker[0], b'y');
    let mut short = [0u8; 1];
    assert_eq!(flistxattr(&root, &mut short), Err(rustix::io::Errno::RANGE));
    let mut complete = [0u8; 65_536];
    assert_eq!(flistxattr(&root, &mut complete).unwrap(), 0);
    crate::require_no_xattrs(&root, "actual copied-up composed root").unwrap();
    validate_directory_mode(&root, Some((0, 0)), 0o755, "actual composed root").unwrap();
    for name in [
        "user.unexpected",
        "trusted.unexpected",
        "user.overlay.unexpected",
    ] {
        rustix::fs::fsetxattr(&root, name, b"", rustix::fs::XattrFlags::CREATE).unwrap();
        assert!(flistxattr(&root, &mut complete).unwrap() > 0);
        assert_eq!(
            crate::require_no_xattrs(&root, "visible composed-root attribute")
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );
        assert_eq!(
            validate_directory_mode(&root, Some((0, 0)), 0o755, "actual composed root")
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );
        rustix::fs::fremovexattr(&root, name).unwrap();
        crate::require_no_xattrs(&root, "cleared composed-root attribute").unwrap();
    }
    assert!(
        validate_directory_mode(
            &upper,
            Some((0, 0)),
            0o755,
            "unchanged general directory validator"
        )
        .is_err()
    );
    revalidate_overlay_work(&work, &kernel_work, (0, 0)).unwrap();
    for bad in [
        b"n".as_slice(),
        b"yy".as_slice(),
        b"yyy".as_slice(),
        b"".as_slice(),
    ] {
        rustix::fs::fsetxattr(
            &upper,
            "trusted.overlay.impure",
            bad,
            rustix::fs::XattrFlags::empty(),
        )
        .unwrap();
        assert!(
            validate_overlay_upper(&upper, (0, 0), MountedRootStateV1::SystemdPreflight).is_err()
        );
    }
    rustix::fs::fsetxattr(
        &upper,
        "trusted.overlay.impure",
        b"y",
        rustix::fs::XattrFlags::empty(),
    )
    .unwrap();
    rustix::fs::fsetxattr(
        &upper,
        "trusted.overlay.uuid",
        b"unexpected",
        rustix::fs::XattrFlags::empty(),
    )
    .unwrap();
    assert!(validate_overlay_upper(&upper, (0, 0), MountedRootStateV1::SystemdPreflight).is_err());
    rustix::fs::fremovexattr(&upper, "trusted.overlay.uuid").unwrap();
    rustix::fs::fremovexattr(&upper, "trusted.overlay.impure").unwrap();
    for name in [
        "trusted.overlay.opaque",
        "trusted.overlay.unexpected-name-larger-than-the-profile",
    ] {
        rustix::fs::fsetxattr(&upper, name, b"y", rustix::fs::XattrFlags::empty()).unwrap();
        assert!(
            validate_overlay_upper(&upper, (0, 0), MountedRootStateV1::SystemdPreflight).is_err()
        );
        rustix::fs::fremovexattr(&upper, name).unwrap();
    }
    rustix::fs::fsetxattr(
        &upper,
        "trusted.overlay.impure",
        b"y",
        rustix::fs::XattrFlags::empty(),
    )
    .unwrap();
    validate_overlay_upper(&upper, (0, 0), MountedRootStateV1::SystemdPreflight).unwrap();

    let other = File::open(scratch.path().join("other")).unwrap();
    fchmod(&other, Mode::from_raw_mode(0)).unwrap();
    assert!(revalidate_overlay_work(&work, &other, (0, 0)).is_err());
    fchmod(&kernel_work, Mode::from_raw_mode(0o700)).unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    fchmod(&kernel_work, Mode::from_raw_mode(0)).unwrap();
    rustix::fs::fchown(&kernel_work, Some(rustix::process::Uid::from_raw(1)), None).unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    rustix::fs::fchown(&kernel_work, Some(rustix::process::Uid::from_raw(0)), None).unwrap();
    rustix::fs::fchown(&kernel_work, None, Some(rustix::process::Gid::from_raw(1))).unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    rustix::fs::fchown(&kernel_work, None, Some(rustix::process::Gid::from_raw(0))).unwrap();
    rustix::fs::fsetxattr(
        &kernel_work,
        "trusted.unexpected",
        b"x",
        rustix::fs::XattrFlags::empty(),
    )
    .unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    rustix::fs::fremovexattr(&kernel_work, "trusted.unexpected").unwrap();
    std::fs::write(scratch.path().join("work/work/unexpected"), b"x").unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    std::fs::remove_file(scratch.path().join("work/work/unexpected")).unwrap();
    std::fs::create_dir(scratch.path().join("work/index")).unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    std::fs::remove_dir(scratch.path().join("work/index")).unwrap();
    std::fs::rename(
        scratch.path().join("work/work"),
        scratch.path().join("retained-work"),
    )
    .unwrap();
    std::fs::create_dir(scratch.path().join("work/work")).unwrap();
    fchmod(
        File::open(scratch.path().join("work/work")).unwrap(),
        Mode::from_raw_mode(0),
    )
    .unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    std::fs::remove_dir(scratch.path().join("work/work")).unwrap();
    std::os::unix::fs::symlink("../retained-work", scratch.path().join("work/work")).unwrap();
    assert!(revalidate_overlay_work(&work, &kernel_work, (0, 0)).is_err());
    std::fs::remove_file(scratch.path().join("work/work")).unwrap();
    std::fs::rename(
        scratch.path().join("retained-work"),
        scratch.path().join("work/work"),
    )
    .unwrap();
    revalidate_overlay_work(&work, &kernel_work, (0, 0)).unwrap();
    for value in [b"".as_slice(), b"n", b"x", b"yy", &[b'y'; 256]] {
        rustix::fs::fsetxattr(
            &proc_target.0,
            "trusted.overlay.opaque",
            value,
            rustix::fs::XattrFlags::REPLACE,
        )
        .unwrap();
        assert_eq!(
            revalidate_overlay_proc_target(
                &upper,
                &proc_target,
                (0, 0),
                OverlayProcProfileV82::Opaque,
            )
            .unwrap_err()
            .kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );
    }
    rustix::fs::fsetxattr(
        &proc_target.0,
        "trusted.overlay.opaque",
        b"y",
        rustix::fs::XattrFlags::REPLACE,
    )
    .unwrap();
    for name in [
        "trusted.overlay.origin",
        "trusted.unexpected",
        "user.unexpected",
    ] {
        rustix::fs::fsetxattr(&proc_target.0, name, b"", rustix::fs::XattrFlags::CREATE).unwrap();
        assert_eq!(
            require_proc_opaque_marker(&proc_target.0)
                .unwrap_err()
                .kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );
        rustix::fs::fremovexattr(&proc_target.0, name).unwrap();
    }
    rustix::fs::fremovexattr(&proc_target.0, "trusted.overlay.opaque").unwrap();
    assert_eq!(
        require_proc_opaque_marker(&proc_target.0)
            .unwrap_err()
            .kind(),
        DeploymentVerificationErrorKindV1::ForbiddenAttributes
    );
    drop((root, kernel_work, proc_target, other, lower, upper, work));
    unmount(&root_path, UnmountFlags::empty()).unwrap();
    cleanup.0.pop();
    unmount(scratch.path(), UnmountFlags::empty()).unwrap();
    cleanup.0.pop();
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}
