use super::*;
use crate::{
    InheritedCompilerExecutionDeploymentV2 as V2, InheritedCompilerExecutionDeploymentV3 as V3,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn exact_native_inherited_contract_and_move_only_owner_charges() {
    use crate::inherited::*;
    assert_eq!(
        DESCRIPTORS,
        [
            COMPILER_EXECUTION_COORDINATOR_RUNTIME_ROOT_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_ROOT_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ANCHOR_ROOT_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_LAUNCHER_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ISSUER_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ANCHOR_HELPER_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ANCHOR_DAEMON_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_SUPERVISOR_DEPLOYMENT_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_POLICY_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ANCHOR_DEPLOYMENT_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ANCHOR_PROVISIONING_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ISSUER_KEY_SEED_FD_V1,
            COMPILER_EXECUTION_COORDINATOR_ANCHOR_KEY_SEED_FD_V1
        ]
    );
    fn send<T: Send>() {}
    send::<V2>();
    send::<V3>();
    assert!(V2::SOURCE_STORAGE >= 5 * 128 * 1024 * 1024 + 14 * FILE_STORAGE);
    assert!(V3::SOURCE_STORAGE >= 5 * 128 * 1024 * 1024 + 14 * FILE_STORAGE);
    assert!(LISTENER_SCRATCH >= 4 * LISTENER_GROWTH);
}

#[test]
#[allow(unsafe_code)]
fn short_root_entry_work_refuses_before_descriptor_access_on_original_account() {
    for family in [2, 3] {
        let mut work = Work::new(7);
        let mut b = Budget::new(&mut work, 64);
        b.reserve_storage(13).unwrap();
        assert!(b.reserve_storage(65).is_err());
        let ledger = b.work_ledger_identity_v1();
        // SAFETY: one-short entry work must refuse before the unsafe intake;
        // this test deliberately provides no inherited ownership to transfer.
        let error = unsafe {
            if family == 2 {
                V2::admit(&mut b).unwrap_err()
            } else {
                V3::admit(&mut b).unwrap_err()
            }
        };
        assert!(matches!(error, Failure::Resource(_)));
        assert_eq!(b.storage(), 13);
        assert_eq!(b.failed_storage(), Some(78));
        assert!(b.failed_work().is_some());
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn root_error_preserves_source_and_resource_without_secret_or_descriptor() {
    let e = Failure::from(source::RootSourceErrorV2::Changed);
    assert!(matches!(
        e,
        Failure::Source(source::RootSourceErrorV2::Changed)
    ));
    assert!(std::error::Error::source(&e).is_some());
    assert_eq!(
        sum(&[usize::MAX, 1]).unwrap_err().to_string(),
        Resource::Arithmetic.to_string()
    );
}

#[test]
fn preflight_requires_bounded_images_exact_records_and_directory_roots() {
    use rustix::fs::FileType;
    let file = tempfile::tempfile().unwrap();
    let mut stat = rustix::fs::fstat(&file).unwrap();
    let limits = [64; 11];
    for index in 0..14 {
        stat.st_mode = FileType::RegularFile.as_raw_mode();
        stat.st_size = 64;
        assert_eq!(
            check_source_shape(index, &limits, &stat).is_ok(),
            index >= 3
        );
        stat.st_size = 63;
        assert_eq!(
            check_source_shape(index, &limits, &stat).is_ok(),
            (3..8).contains(&index)
        );
        for size in [-1, 0, 65] {
            stat.st_size = size;
            assert!(check_source_shape(index, &limits, &stat).is_err());
        }
        stat.st_mode = FileType::Directory.as_raw_mode();
        assert_eq!(check_source_shape(index, &limits, &stat).is_ok(), index < 3);
    }
    assert!(check_source_shape(14, &limits, &stat).is_err());
}

#[test]
fn bounded_thread_enumeration_requires_exactly_one_main_pid_directory() {
    for (names, succeeds) in [
        (vec!["42"], true),
        (vec![], false),
        (vec!["43"], false),
        (vec!["42", "43"], false),
        (vec!["not-a-pid"], false),
        (vec!["42949672960"], false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        for name in names {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        assert_eq!(
            single_thread_entries(File::open(dir.path()).unwrap().into(), 42).is_ok(),
            succeeds
        );
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("42"), b"").unwrap();
    assert!(single_thread_entries(File::open(dir.path()).unwrap().into(), 42).is_err());
}
