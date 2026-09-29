use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{io::Write, os::unix::fs::PermissionsExt};

// Ordinary file fixtures test only mechanics, never fabricate a retained child.
fn file(bytes: &[u8]) -> File {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o700))
        .unwrap();
    file
}

#[test]
fn quote_checks_bounds_and_includes_both_liveness_calls() {
    for length in [
        0,
        MAX_COMPILER_EXECUTION_ISSUER_IMAGE_BYTES_V1 + 1,
        u64::MAX,
    ] {
        assert_eq!(
            retained_issuer_image_quota_v3(length).unwrap_err().kind(),
            Some(Kind::ExecutableSize)
        );
    }
    for length in [1, 4097, MAX_COMPILER_EXECUTION_ISSUER_IMAGE_BYTES_V1] {
        let quote = retained_issuer_image_quota_v3(length).unwrap();
        let (work, scratch) = image_resources(length as usize).unwrap();
        assert_eq!(
            quote.work(),
            CHILD_IO_WORK + work + 2 * Child::OPERATION_WORK
        );
        assert_eq!(
            quote.scratch(),
            CHILD_FRAME + scratch.max(Child::OPERATION_SCRATCH)
        );
    }
}

#[test]
fn wrong_length_refuses_before_parser_allocation_or_payload_read() {
    let file = file(b"not an ELF image");
    for expected in [1, 64] {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 0);
        let error = RootIssuerImageErrorV3::from(
            measure_expected_image(&file, expected, &mut budget).unwrap_err(),
        );
        assert_eq!(error.kind(), Some(Kind::ExecutablePolicyMismatch));
        assert_eq!(error.resource(), None);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn reopened_identity_rejects_another_inode_even_with_identical_bytes() {
    let original = file(b"same executable bytes");
    let replacement = file(b"same executable bytes");
    let snapshot = inspect_executable(&original).unwrap();
    require_current_image(&original, snapshot).unwrap();
    let error =
        RootIssuerImageErrorV3::from(require_current_image(&replacement, snapshot).unwrap_err());
    assert_eq!(error.kind(), Some(Kind::ExecutableChanged));
    original.set_len(snapshot.size + 1).unwrap();
    let error =
        RootIssuerImageErrorV3::from(require_current_image(&original, snapshot).unwrap_err());
    assert_eq!(error.kind(), Some(Kind::ExecutableChanged));
}

#[test]
fn actual_procfs_reopens_the_same_running_executable() {
    let process = open_child_proc(rustix::process::getpid()).unwrap();
    let image = open_process_image(&process).unwrap();
    // The test harness itself need not satisfy the issuer size/static-image policy.
    let snapshot = Snapshot::inspect(&image, Kind::ExecutableInspect).unwrap();
    let current = open_process_image(&process).unwrap();
    assert_eq!(
        Snapshot::inspect(&current, Kind::ExecutableInspect).unwrap(),
        snapshot
    );
}

#[test]
fn ordinary_directory_is_not_a_process_view() {
    let dir = tempfile::tempdir().unwrap();
    let directory = File::open(dir.path()).unwrap();
    let error = RootIssuerImageErrorV3::from(require_procfs(&directory).unwrap_err());
    assert_eq!(error.kind(), Some(Kind::ExecutableInspect));
}

#[test]
fn public_error_keeps_policy_axes_and_nested_resource_categories() {
    let measurement = Measurement::new([1; 32], 1).unwrap();
    let measurements = Measurements {
        executable: measurement,
        runtime: sealed_static_issuer_runtime_measurement_v1(),
        sealed_static_identity: [2; 32],
    };
    let replacement = Measurement::new([3; 32], 1).unwrap();
    let error = RootIssuerImageErrorV3::from(
        check_policy(measurements, replacement, measurements.runtime).unwrap_err(),
    );
    assert_eq!(error.kind(), Some(Kind::ExecutablePolicyMismatch));
    let error = RootIssuerImageErrorV3::from(
        check_policy(measurements, measurement, replacement).unwrap_err(),
    );
    assert_eq!(error.kind(), Some(Kind::RuntimePolicyMismatch));
    for error in [
        RootIssuerImageErrorV3::from(Resource::Arithmetic),
        RootIssuerImageErrorV3::from(SpawnError::Resource(Resource::Accounting)),
    ] {
        assert!(error.resource().is_some());
        assert_eq!(error.kind(), None);
    }
}
