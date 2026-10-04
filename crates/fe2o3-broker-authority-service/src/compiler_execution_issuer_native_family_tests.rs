//! Cross-family disk rejection through the real shared recovery path.
use super::{native, native_v3};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    fs::File,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
};

fn recover(version: u16, root: &File, published: bool, b: &mut Budget<'_>) -> bool {
    match version {
        2 => native::service::recover_family_fixture(root, published, b).is_ok(),
        3 => native_v3::service::recover_family_fixture(root, published, b).is_ok(),
        _ => unreachable!(),
    }
}

fn directory() -> (tempfile::TempDir, File) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let fd = File::open(dir.path()).unwrap();
    (dir, fd)
}

fn snapshot(root: &Path) -> Vec<(std::ffi::OsString, Vec<u8>, u64, i64, i64)> {
    let mut entries: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            (
                entry.file_name(),
                std::fs::read(entry.path()).unwrap(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            )
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[test]
fn native_families_cannot_initialize_over_each_others_ready_or_published_journals() {
    for source in [2, 3] {
        for published in [false, true] {
            let mut work = Work::new(usize::MAX);
            let mut b = Budget::new(&mut work, 256 * 1024 * 1024);
            b.reserve_storage(23).unwrap();
            let (dir, root) = directory();
            assert!(recover(source, &root, published, &mut b));
            let before = snapshot(dir.path());
            let original_work = b.work();
            assert!(!recover(5 - source, &root, false, &mut b));
            assert_eq!(b.storage(), 23);
            assert!(b.work() > original_work);
            assert_eq!(snapshot(dir.path()), before);
            assert!(recover(source, &root, false, &mut b));
        }
    }
}

#[test]
fn native_families_reject_mixed_worker_and_anchor_before_any_recovery_mutation() {
    use crate::compiler_execution_journal_recovery::{
        NATIVE_ANCHOR_STATE_FILES, NATIVE_WORKER_STATE_FILES,
    };
    for target in [2, 3] {
        for family in [NATIVE_WORKER_STATE_FILES, NATIVE_ANCHOR_STATE_FILES] {
            for name in family {
                let mut work = Work::new(usize::MAX);
                let mut b = Budget::new(&mut work, 256 * 1024 * 1024);
                let (dir, root) = directory();
                let (donor, donor_root) = directory();
                assert!(recover(target, &root, true, &mut b));
                assert!(recover(5 - target, &donor_root, true, &mut b));
                let foreign = std::fs::read(donor.path().join(family[0])).unwrap();
                std::fs::write(dir.path().join(name), foreign).unwrap();
                std::fs::set_permissions(
                    dir.path().join(name),
                    std::fs::Permissions::from_mode(0o600),
                )
                .unwrap();
                let before = snapshot(dir.path());
                assert!(!recover(target, &root, false, &mut b));
                assert_eq!(b.storage(), 0);
                assert_eq!(snapshot(dir.path()), before, "family {target}, {name}");
            }
        }
    }
}
