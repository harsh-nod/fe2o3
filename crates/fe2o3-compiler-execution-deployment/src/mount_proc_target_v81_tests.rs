#[test]
fn native_proc_target_has_exact_profile_and_retained_identity() {
    let scratch = tempfile::tempdir().unwrap();
    let upper = File::open(scratch.path()).unwrap();
    let before = snapshot(&fstat(&upper).unwrap());
    let owner = (before.uid, before.gid);
    prepare_overlay_upper(&upper, owner).unwrap();
    let retained = prepare_overlay_proc_directory(&upper, owner).unwrap();
    assert_eq!(retained.1.mode & 0o7777, 0o755);
    assert_eq!((retained.1.uid, retained.1.gid), owner);
    assert_eq!(retained.1.device, before.device);
    validate_overlay_upper(&upper, owner, MountedRootStateV1::Pristine).unwrap();
    revalidate_overlay_proc_target(&upper, &retained, owner, OverlayProcProfileV82::Unmarked)
        .unwrap();
    assert!(prepare_overlay_proc_target(&upper, owner).is_err());
    assert_eq!(snapshot(&fstat(&retained.0).unwrap()), retained.1);
}

#[test]
fn native_proc_target_refuses_preexisting_entries_without_mutation() {
    for mutant in 0..4 {
        let scratch = tempfile::tempdir().unwrap();
        let upper = File::open(scratch.path()).unwrap();
        let observed = snapshot(&fstat(&upper).unwrap());
        let owner = (observed.uid, observed.gid);
        prepare_overlay_upper(&upper, owner).unwrap();
        let path = scratch.path().join("proc");
        match mutant {
            0 => std::fs::create_dir(&path).unwrap(),
            1 => std::fs::write(&path, b"hostile").unwrap(),
            2 => std::os::unix::fs::symlink(".", &path).unwrap(),
            3 => std::fs::create_dir(scratch.path().join("unexpected")).unwrap(),
            _ => unreachable!(),
        }
        let before = snapshot(&fstat(&upper).unwrap());
        assert!(
            prepare_overlay_proc_target(&upper, owner).is_err(),
            "{mutant}"
        );
        assert_eq!(snapshot(&fstat(&upper).unwrap()), before, "{mutant}");
    }
}

#[test]
fn native_proc_target_refuses_metadata_attributes_contents_and_replacement() {
    for mutant in 0..6 {
        let scratch = tempfile::tempdir().unwrap();
        let upper = File::open(scratch.path()).unwrap();
        let observed = snapshot(&fstat(&upper).unwrap());
        let owner = (observed.uid, observed.gid);
        prepare_overlay_upper(&upper, owner).unwrap();
        let retained = prepare_overlay_proc_directory(&upper, owner).unwrap();
        match mutant {
            0 => fchmod(&retained.0, Mode::from_raw_mode(0o700)).unwrap(),
            1 => rustix::fs::fsetxattr(
                &retained.0,
                "user.unexpected",
                b"x",
                rustix::fs::XattrFlags::empty(),
            )
            .unwrap(),
            2 => std::fs::write(scratch.path().join("proc/unexpected"), b"x").unwrap(),
            3 => {
                std::fs::rename(
                    scratch.path().join("proc"),
                    scratch.path().join("retained-proc"),
                )
                .unwrap();
                std::fs::create_dir(scratch.path().join("proc")).unwrap();
                fchmod(
                    File::open(scratch.path().join("proc")).unwrap(),
                    Mode::from_raw_mode(0o755),
                )
                .unwrap();
            }
            4 => {
                std::fs::rename(
                    scratch.path().join("proc"),
                    scratch.path().join("retained-proc"),
                )
                .unwrap();
                std::os::unix::fs::symlink("retained-proc", scratch.path().join("proc")).unwrap();
            }
            5 => (),
            _ => unreachable!(),
        }
        let check_owner = if mutant == 5 {
            (owner.0.wrapping_add(1), owner.1)
        } else {
            owner
        };
        assert!(
            revalidate_overlay_proc_target(
                &upper,
                &retained,
                check_owner,
                OverlayProcProfileV82::Unmarked,
            )
            .is_err(),
            "{mutant}"
        );
    }
}

#[test]
fn native_proc_opaque_profile_refuses_missing_alternate_multiple_and_oversize_markers() {
    assert!(proc_opaque_marker_is_exact(PROC_OPAQUE_NAME_V82, b"y"));
    for names in [
        b"".as_slice(),
        b"trusted.overlay.origin\0",
        b"user.overlay.opaque\0",
        b"trusted.overlay.opaque",
        b"trusted.overlay.opaque\0user.extra\0",
        b"trusted.overlay.opaque\0trusted.overlay.opaque\0",
        &[b'x'; 256],
    ] {
        assert!(!proc_opaque_marker_is_exact(names, b"y"));
    }
    for value in [b"".as_slice(), b"n", b"x", b"y\0", b"yy", &[b'y'; 256]] {
        assert!(!proc_opaque_marker_is_exact(PROC_OPAQUE_NAME_V82, value));
    }
    let scratch = tempfile::tempdir().unwrap();
    let target = File::open(scratch.path()).unwrap();
    assert_eq!(
        require_proc_opaque_marker(&target).unwrap_err().kind(),
        DeploymentVerificationErrorKindV1::ForbiddenAttributes
    );
    rustix::fs::fsetxattr(
        &target,
        "user.unexpected",
        b"y",
        rustix::fs::XattrFlags::CREATE,
    )
    .unwrap();
    assert_eq!(
        require_proc_opaque_marker(&target).unwrap_err().kind(),
        DeploymentVerificationErrorKindV1::ForbiddenAttributes
    );
    for length in [32, 255] {
        let name = format!("user.{}", "x".repeat(length - 5));
        rustix::fs::fsetxattr(&target, &name, b"", rustix::fs::XattrFlags::CREATE).unwrap();
        assert_eq!(
            require_proc_opaque_marker(&target).unwrap_err().kind(),
            DeploymentVerificationErrorKindV1::ForbiddenAttributes
        );
        rustix::fs::fremovexattr(&target, &name).unwrap();
    }
}

#[test]
fn native_proc_target_precedes_overlay_attachment_and_keeps_pid1_strict() {
    let source = include_str!("mount.rs");
    let body = source
        .split_once("fn attach_overlay(")
        .unwrap()
        .1
        .split_once("#[derive(Clone, Copy)]")
        .unwrap()
        .0;
    let base = body.find("validate_sealed_base_proc_target(").unwrap();
    let create = body
        .find("mounted.overlay_proc = Some(prepare_overlay_proc_target(")
        .unwrap();
    let context = body.find("fsopen(\"overlay\"").unwrap();
    let attach = body.find("move_mount(").unwrap();
    assert!(base < create && create < context && context < attach);
    let cleanup = source
        .split_once("fn cleanup_internal_with_hooks(")
        .unwrap()
        .1
        .split_once("fn unmount_retained_child(")
        .unwrap()
        .0;
    assert!(
        cleanup.find("self.overlay_proc.take();").unwrap()
            < cleanup.find(".staged\n            .take()").unwrap()
    );
    let helper = include_str!("preflight_namespace_v79.rs")
        .split_once("pub(super) fn mount_private_proc_v79()")
        .unwrap()
        .1;
    assert!(
        helper.find("super::validate_directory_mode(").unwrap()
            < helper.find("rustix::mount::mount(").unwrap()
    );
    assert!(!helper.contains("NOTSUP"));
    let general = include_str!("lib.rs")
        .split_once("fn require_no_xattrs(")
        .unwrap()
        .1
        .split_once("fn read_bounded(")
        .unwrap()
        .0;
    assert!(general.contains(
        "Err(source) => Err(io_error(\"inspect deployment extended attributes\", source))"
    ));
}
