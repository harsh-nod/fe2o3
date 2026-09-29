//! Inert resource and root-TCB comparison tests, not native endpoint qualification.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    ffi::{OsStr, OsString},
    path::Path,
    process::Command,
};

#[test]
fn native_capture_prepaid_exact_and_one_short_never_reset_the_original_account() {
    let command = Command::new("/toolchain/rustc");
    let retained = PARENT_MAX_STORAGE + CAPTURE_SCRATCH + size_of::<InvocationAuthority>();
    for (work, storage, succeeds) in [
        (CAPTURE_WORK, retained, true),
        (CAPTURE_WORK - 1, retained, false),
        (CAPTURE_WORK, retained - 1, false),
    ] {
        let mut work = Work::new(work);
        let mut b = Budget::new(&mut work, storage + 19);
        b.reserve_storage(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let used = Cell::new(false);
        let result = prepay_capture_once(
            &used,
            &command,
            OsStr::new("/toolchain/rustc"),
            Path::new("/workspace"),
            &[],
            &mut b,
        );
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(b.storage(), 19 + retained);
        } else {
            assert!(matches!(result, Err(Error::Resource(_))));
        }
        let charged = b.storage();
        let spent = b.work();
        let denial = b.failed_storage();
        assert!(matches!(
            prepay_capture_once(
                &used,
                &command,
                OsStr::new("/toolchain/rustc"),
                Path::new("/workspace"),
                &[],
                &mut b
            ),
            Err(Error::Rejected(_))
        ));
        assert_eq!(b.storage(), charged);
        assert_eq!(b.work(), spent);
        assert_eq!(b.failed_storage(), denial);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn borrowed_capture_shapes_are_bounded_before_native_copying() {
    use fe2o3_rustc_invocation::{
        MAX_ARGUMENT_BYTES_V2, MAX_COMPILE_ENVIRONMENT_ENTRIES_V2, MAX_RUSTC_ARGUMENTS_V2,
    };
    let mut command = Command::new("rustc");
    let argv0 = OsStr::new("rustc");
    let cwd = Path::new("/workspace");
    assert!(check_capture_shape(&command, argv0, cwd, &[]).is_ok());
    command.arg("x".repeat(MAX_ARGUMENT_BYTES_V2 + 1));
    assert!(check_capture_shape(&command, argv0, cwd, &[]).is_err());
    let mut command = Command::new("rustc");
    command.args(std::iter::repeat_n("", MAX_RUSTC_ARGUMENTS_V2));
    assert!(check_capture_shape(&command, argv0, cwd, &[]).is_err());
    let command = Command::new("rustc");
    let environment =
        vec![(OsString::from("A"), OsString::new()); MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 + 1];
    assert!(check_capture_shape(&command, argv0, cwd, &environment).is_err());
    let mut command = Command::new("rustc");
    command.args(std::iter::repeat_n("x".repeat(MAX_ARGUMENT_BYTES_V2), 65));
    assert!(check_capture_shape(&command, argv0, cwd, &[]).is_err());
}

#[test]
fn root_peer_comparison_does_not_reinterpret_nonroot_supervisor_credentials() {
    let pid = rustix::process::getpid();
    for (uid, gid, accepted) in [
        (0, 0, true),
        (1000, 1000, false),
        (0, 1000, false),
        (1000, 0, false),
    ] {
        let peer = net::UCred {
            pid,
            uid: rustix::process::Uid::from_raw(uid),
            gid: rustix::process::Gid::from_raw(gid),
        };
        assert_eq!(root_sender(peer).is_ok(), accepted);
    }
    // This unit test supplies comparison facts only; actual production transport
    // obtains SO_PEERCRED and independently checks every SCM_CREDENTIALS record.
}

#[test]
fn fixed_directory_shape_rejects_nonroot_writable_and_noncanonical_runtime_modes() {
    let directory = Identity {
        device: 1,
        inode: 2,
        mode: libc::S_IFDIR | 0o755,
        uid: 0,
        gid: 0,
    };
    assert!(directory.directory(true).is_ok());
    for changed in [
        Identity {
            uid: 1000,
            ..directory
        },
        Identity {
            gid: 1000,
            ..directory
        },
        Identity {
            mode: libc::S_IFDIR | 0o775,
            ..directory
        },
        Identity {
            mode: libc::S_IFDIR | 0o700,
            ..directory
        },
        Identity {
            mode: libc::S_IFREG | 0o755,
            ..directory
        },
    ] {
        assert!(changed.directory(true).is_err());
    }
}
