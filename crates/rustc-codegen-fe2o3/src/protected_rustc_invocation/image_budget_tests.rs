use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Write, os::fd::AsRawFd};

fn image() -> (File, std::path::PathBuf) {
    let fd = rustix::fs::memfd_create("fe2o3-image-budget-test", rustix::fs::MemfdFlags::CLOEXEC)
        .expect("memfd_create");
    let mut file = File::from(fd);
    file.write_all(b"compiler image").unwrap();
    let path = format!("/proc/self/fd/{}", file.as_raw_fd()).into();
    (file, path)
}

fn scratch() -> usize {
    COMPILER_IMAGE_MEASUREMENT_STORAGE_V1 + std::mem::size_of::<ImageError<Resource>>()
}

#[test]
fn image_budget_exact_limits_preserve_the_original_ledger_and_release_scratch() {
    let (_file, path) = image();
    let floor = path.as_os_str().len();
    let total = 16 + floor + 5 * 64 * 1024 + b"compiler image".len();
    let mut work = Work::new(total);
    let mut budget = Budget::new(&mut work, floor + scratch());
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let digest = measure_image_with_budget(&path, ImageRole::CodegenBackend, &mut budget).unwrap();
    let expected: [u8; 32] = Sha256::digest(b"compiler image").into();
    assert_eq!(digest, expected);
    assert_eq!(budget.work(), total);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor + scratch());
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_work(), None);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn image_budget_one_short_work_retains_the_first_denial_and_input_owner() {
    let (file, path) = image();
    let floor = path.as_os_str().len();
    let total = 16 + floor + 5 * 64 * 1024 + b"compiler image".len();
    let mut work = Work::new(total - 1);
    let mut budget = Budget::new(&mut work, floor + scratch());
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert!(matches!(
        measure_image_with_budget(&path, ImageRole::CodegenBackend, &mut budget),
        Err(ImageError::Work(Resource::Work(_)))
    ));
    assert_eq!(budget.work(), 16 + floor + 2 * 64 * 1024);
    assert_eq!(budget.failed_work(), Some(total));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor + scratch());
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(file.metadata().is_ok());
}

#[test]
fn image_budget_refuses_unpaid_path_or_scratch_before_opening() {
    let (_file, path) = image();
    let missing = path.with_extension("missing");
    let floor = missing.as_os_str().len();
    for (prepaid, capacity, missing_input) in [
        (floor - 1, floor + scratch(), true),
        (floor, floor + scratch() - 1, false),
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, capacity);
        budget.reserve_storage(prepaid).unwrap();
        let result = measure_image_with_budget(&missing, ImageRole::CodegenBackend, &mut budget);
        if missing_input {
            assert!(matches!(
                result,
                Err(ImageError::Work(Resource::Accounting))
            ));
            assert_eq!(budget.failed_storage(), None);
        } else {
            assert!(matches!(
                result,
                Err(ImageError::Work(Resource::Storage(_)))
            ));
            assert_eq!(budget.failed_storage(), Some(floor + scratch()));
        }
        assert_eq!(budget.work(), 8);
        assert_eq!(budget.storage(), prepaid);
    }
}

#[test]
fn image_budget_io_failure_releases_scratch_without_refunding_work() {
    let (_file, path) = image();
    let missing = path.with_extension("missing");
    let floor = missing.as_os_str().len();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, floor + scratch());
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        measure_image_with_budget(&missing, ImageRole::CodegenBackend, &mut budget),
        Err(ImageError::Io(_))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), 16 + floor + 64 * 1024);
    assert_eq!(budget.peak_storage(), floor + scratch());
}
