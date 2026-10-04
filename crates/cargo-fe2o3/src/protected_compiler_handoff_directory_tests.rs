//! Inert wrapper input custody, not protected execution or pathname approval.
use super::*;
use crate::pinned_executable_test_directory::TestDirectory;
use rustix::io::FdFlags;
use std::{
    fs,
    os::fd::{AsFd, AsRawFd},
};

#[test]
fn owned_directory_survives_path_replacement_and_drop() {
    // Isolate FD reuse probes from other tests; no raw-FD borrowing is needed.
    const SENTINEL: &str = "FE2O3_WRAPPER_DIRECTORY_TEST";
    if std::env::var_os(SENTINEL).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "protected_compiler_handoff_v3::native::tests::directory::owned_directory_survives_path_replacement_and_drop",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(SENTINEL, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "directory fixture failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    let temp = TestDirectory::new();
    let selected = temp.path().join("selected");
    let moved = temp.path().join("moved");
    fs::create_dir(&selected).unwrap();
    let directory = PinnedWorkingDirectoryV3::open(&selected).unwrap();
    let retained_fd = directory.native_source().unwrap().as_raw_fd();
    let identity = directory.object_identity();
    let mut command = Command::new("/bin/pwd");
    command.arg("-P");
    directory.configure_child_fchdir(&mut command).unwrap();
    let custody = parent_with_directory(0, directory, &selected);
    fs::rename(&selected, &moved).unwrap();
    fs::create_dir(&selected).unwrap();
    let replacement = PinnedWorkingDirectoryV3::open(&selected).unwrap();
    assert_ne!(replacement.object_identity(), identity);

    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    let floor = custody.native_retained_storage().unwrap();
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    custody.retain_through(|custody| {
        custody.revalidate().unwrap();
        custody.revalidate_native(&mut b).unwrap();
        assert_eq!(
            custody.invocation.descriptor().rustc().working_directory(),
            selected.to_str().unwrap(),
        );
        assert_eq!(
            custody
                .native_working_directory(&mut b)
                .unwrap()
                .as_raw_fd(),
            retained_fd,
        );
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim_end(),
            moved.to_str().unwrap(),
        );
        drop(command);
        custody.revalidate_native(&mut b).unwrap();
        assert!(!custody.grants_compiler_authority());
    });
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
    let probe =
        rustix::io::fcntl_dupfd_cloexec(replacement.native_source().unwrap(), retained_fd).unwrap();
    assert_eq!(probe.as_raw_fd(), retained_fd, "custody leaked its cwd");
    drop(probe);
    b.release_storage(floor).unwrap();

    // Nonprotected/query custody remains absent; the prepared command's own
    // duplicate must still select the directory after retain drops its input.
    for capture in [
        None,
        Some(InertPreparedRustcInvocationCapture::V2(capture_v2(
            0, &selected,
        ))),
    ] {
        let directory = PinnedWorkingDirectoryV3::open(&selected).unwrap();
        let retained_fd = directory.native_source().unwrap().as_raw_fd();
        let mut command = Command::new("/bin/pwd");
        command.arg("-P");
        directory.configure_child_fchdir(&mut command).unwrap();
        assert!(
            ParentRustcInvocationCustody::retain(capture, None, None, directory)
                .unwrap()
                .is_none()
        );
        let probe =
            rustix::io::fcntl_dupfd_cloexec(replacement.native_source().unwrap(), retained_fd)
                .unwrap();
        assert_eq!(probe.as_raw_fd(), retained_fd);
        drop(probe);
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim_end(),
            selected.to_str().unwrap()
        );
    }
}

#[test]
fn native_directory_transfer_rejects_substitution_and_flag_drift() {
    let temp = TestDirectory::new();
    let custody = parent(0);
    let foreign = PinnedWorkingDirectoryV3::open(temp.path()).unwrap();
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(custody.native_retained_storage().unwrap() + size_of::<OwnedFd>())
        .unwrap();
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let source = custody.native_working_directory(&mut b).unwrap();
    let transfer = rustix::io::fcntl_dupfd_cloexec(source, 3).unwrap();
    custody
        .validate_native_working_directory_transfer(transfer.as_fd(), &mut b)
        .unwrap();
    assert!(matches!(
        custody
            .validate_native_working_directory_transfer(foreign.native_source().unwrap(), &mut b),
        Err(Error::Io {
            errno: libc::ESTALE,
            ..
        }),
    ));
    rustix::io::fcntl_setfd(&transfer, FdFlags::empty()).unwrap();
    assert!(
        custody
            .validate_native_working_directory_transfer(transfer.as_fd(), &mut b)
            .is_err()
    );
    rustix::io::fcntl_setfd(&transfer, FdFlags::CLOEXEC).unwrap();
    custody
        .validate_native_working_directory_transfer(transfer.as_fd(), &mut b)
        .unwrap();
    rustix::io::fcntl_setfd(source, FdFlags::empty()).unwrap();
    assert!(custody.revalidate().is_err());
    assert!(matches!(
        custody.revalidate_native(&mut b),
        Err(Error::Io {
            errno: libc::ESTALE,
            ..
        })
    ));
    assert!(custody.native_working_directory(&mut b).is_err());
    assert!(
        custody
            .validate_native_working_directory_transfer(transfer.as_fd(), &mut b)
            .is_err()
    );
    rustix::io::fcntl_setfd(source, FdFlags::CLOEXEC).unwrap();
    custody.revalidate_native(&mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn native_directory_access_and_transfer_require_original_account_resources() {
    let custody = parent(0);
    let source = custody.working_directory.native_source().unwrap();
    let transfer = rustix::io::fcntl_dupfd_cloexec(source, 3).unwrap();
    for transfer_check in [false, true] {
        let floor = custody.native_retained_storage().unwrap()
            + if transfer_check {
                size_of::<OwnedFd>()
            } else {
                0
            };
        let mut measured = (WORK, LIMIT);
        for case in 0..5 {
            let mut work = Work::new(measured.0 - usize::from(case == 2));
            let mut b = Budget::new(&mut work, measured.1 - usize::from(case == 3));
            let input = floor - usize::from(case == 4);
            b.reserve_storage(input).unwrap();
            b.charge_work(19).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let result = if transfer_check {
                custody.validate_native_working_directory_transfer(transfer.as_fd(), &mut b)
            } else {
                custody.native_working_directory(&mut b).map(|_| ())
            };
            assert_eq!(result.is_ok(), case < 2, "case {case}: {result:?}");
            if case == 0 {
                measured = (b.work(), b.peak_storage());
            }
            if case >= 2 {
                assert!(matches!(result, Err(Error::Resource(_))));
            }
            if case == 3 {
                assert!(b.failed_storage().is_some());
            }
            assert_eq!(b.storage(), input);
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }
    // An exhausted account refuses before attempting even invalid descriptor I/O.
    rustix::io::fcntl_setfd(source, FdFlags::empty()).unwrap();
    let mut work = Work::new(0);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(custody.native_retained_storage().unwrap())
        .unwrap();
    assert!(matches!(
        custody.native_working_directory(&mut b),
        Err(Error::Resource(Resource::Work(_)))
    ));
    rustix::io::fcntl_setfd(source, FdFlags::CLOEXEC).unwrap();
}
