use super::*;
use crate::{COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1, LIFECYCLE_PARENT_MODE_V1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
};

type Lease = CompilerExecutionProvisioningLifecycleLeaseV2;
const LIMIT: usize = 1_000_000;

struct Fixture {
    root: tempfile::TempDir,
    parent: PathBuf,
    lock: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(
            &parent,
            fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        fs::create_dir(parent.join("state")).unwrap();
        let lock = parent.join(lifecycle_name().unwrap());
        write_lock(&lock);
        Self { root, parent, lock }
    }

    fn open(&self, b: &mut Budget<'_>) -> Result<(Lease, Storage)> {
        Lease::open_at(&self.parent, uid(), gid(), b, |_| Ok(()))
    }

    fn retain(&self, b: &mut Budget<'_>) -> Lease {
        let (lease, charge) = self.open(b).unwrap();
        assert_eq!(charge.additional_storage(), lease.retained_storage());
        b.reserve_storage(charge.additional_storage()).unwrap();
        lease
    }

    fn check(&self, lease: &Lease, b: &mut Budget<'_>) -> Result<()> {
        lease.revalidate_at(&self.parent, b)
    }
}

fn uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

fn gid() -> u32 {
    rustix::process::getegid().as_raw()
}

fn write_lock(path: &Path) {
    fs::write(path, []).unwrap();
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
    )
    .unwrap();
}

fn retire(lease: Lease, b: &mut Budget<'_>) {
    let retained = lease.retained_storage();
    drop(lease);
    b.release_storage(retained).unwrap();
}

fn assert_blocked(path: &Path) {
    for operation in [
        FlockOperation::NonBlockingLockShared,
        FlockOperation::NonBlockingLockExclusive,
    ] {
        assert_eq!(
            flock(File::open(path).unwrap(), operation),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
    }
}

fn assert_available(path: &Path) {
    flock(
        File::open(path).unwrap(),
        FlockOperation::NonBlockingLockExclusive,
    )
    .unwrap();
}

fn refs(file: &File) -> usize {
    let expected = file.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| fs::metadata(entry.ok()?.path()).ok())
        .filter(|other| (other.dev(), other.ino()) == (expected.dev(), expected.ino()))
        .count()
}

#[test]
fn open_exact_work_and_storage_preserve_prefix_and_return_full_charge() {
    let f = Fixture::new();
    let floor = 37;
    let prior_work = 19;
    let mut work = Work::new(prior_work + Lease::ADMISSION_WORK);
    let mut b = Budget::new(&mut work, floor + Lease::IO_STORAGE);
    b.charge_work(prior_work).unwrap();
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (lease, charge) = f.open(&mut b).unwrap();
    assert_eq!(b.work(), prior_work + Lease::ADMISSION_WORK);
    assert_eq!(b.storage(), floor);
    assert_eq!(b.peak_storage(), floor + Lease::IO_STORAGE);
    assert_eq!(charge.additional_storage(), size_of::<(Lease, Storage)>());
    assert_eq!(charge.additional_storage(), lease.retained_storage());
    assert!(ledger == b.work_ledger_identity_v1());
    assert_blocked(&f.lock);
    b.reserve_storage(charge.additional_storage()).unwrap();
    retire(lease, &mut b);
    assert_eq!(b.storage(), floor);
    assert_available(&f.lock);
}

#[test]
fn revalidation_exact_work_and_storage_use_the_original_ledger() {
    let f = Fixture::new();
    let prefix = 31;
    let floor = prefix + Lease::RETAINED;
    let mut work = Work::new(Lease::ADMISSION_WORK + Lease::REVALIDATION_WORK);
    let mut b = Budget::new(&mut work, floor + Lease::IO_STORAGE);
    b.reserve_storage(prefix).unwrap();
    let lease = f.retain(&mut b);
    let ledger = b.work_ledger_identity_v1();
    f.check(&lease, &mut b).unwrap();
    assert_eq!(b.work(), Lease::ADMISSION_WORK + Lease::REVALIDATION_WORK);
    assert_eq!(b.storage(), floor);
    assert_eq!(b.peak_storage(), floor + Lease::IO_STORAGE);
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_blocked(&f.lock);
    retire(lease, &mut b);
    assert_eq!(b.storage(), prefix);
}

#[test]
fn independently_opened_exclusive_contender_is_busy_until_close() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.retain(&mut b);
    let floor = b.storage();
    assert!(matches!(
        f.open(&mut b),
        Err(Error::Lease(LeaseError::Busy))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), 2 * Lease::ADMISSION_WORK);
    f.check(&lease, &mut b).unwrap();
    assert_blocked(&f.lock);
    retire(lease, &mut b);
    let next = f.retain(&mut b);
    retire(next, &mut b);
}

#[test]
fn open_short_entry_work_full_work_and_scratch_refuse_before_filesystem_access() {
    for short in 0..3 {
        let f = Fixture::new();
        let floor = 23;
        let quota = match short {
            0 => ENTRY_WORK - 1,
            1 => Lease::ADMISSION_WORK - 1,
            _ => Lease::ADMISSION_WORK,
        };
        let mut work = Work::new(quota);
        let mut b = Budget::new(
            &mut work,
            floor + Lease::IO_STORAGE - usize::from(short == 2),
        );
        b.reserve_storage(floor).unwrap();
        let result = Lease::open_at(
            &f.root.path().join("absent-parent"),
            uid(),
            gid(),
            &mut b,
            |_| panic!("unpaid open"),
        );
        if short == 2 {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        }
        assert_eq!(
            b.work(),
            match short {
                0 => 0,
                1 => ENTRY_WORK,
                _ => Lease::ADMISSION_WORK,
            }
        );
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor);
        assert_eq!(
            b.failed_work(),
            match short {
                0 => Some(ENTRY_WORK),
                1 => Some(Lease::ADMISSION_WORK),
                _ => None,
            }
        );
        assert_eq!(
            b.failed_storage(),
            (short == 2).then_some(floor + Lease::IO_STORAGE)
        );
        assert_available(&f.lock);
    }
}

#[test]
fn revalidation_refuses_short_owner_floor_work_and_scratch_before_path_access() {
    let f = Fixture::new();
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let lease = f.retain(&mut setup);
    for short in 0..4 {
        let floor = lease.retained_storage() - usize::from(short == 0);
        let mut work = Work::new(match short {
            1 => ENTRY_WORK - 1,
            2 => Lease::REVALIDATION_WORK - 1,
            _ => Lease::REVALIDATION_WORK,
        });
        let mut b = Budget::new(
            &mut work,
            floor + Lease::IO_STORAGE - usize::from(short == 3),
        );
        b.reserve_storage(floor).unwrap();
        let result = lease.revalidate_at(&f.root.path().join("absent-parent"), &mut b);
        match short {
            0 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            1 | 2 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
        }
        assert_eq!(
            b.work(),
            match short {
                1 => 0,
                3 => Lease::REVALIDATION_WORK,
                _ => ENTRY_WORK,
            }
        );
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor);
        assert_eq!(
            b.failed_work(),
            match short {
                1 => Some(ENTRY_WORK),
                2 => Some(Lease::REVALIDATION_WORK),
                _ => None,
            }
        );
        assert_eq!(
            b.failed_storage(),
            (short == 3).then_some(floor + Lease::IO_STORAGE)
        );
        assert_blocked(&f.lock);
    }
    retire(lease, &mut setup);
}

#[test]
fn overflow_refuses_before_open_without_refunding_work() {
    for storage in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        let floor = if storage {
            usize::MAX - Lease::IO_STORAGE + 1
        } else {
            0
        };
        b.reserve_storage(floor).unwrap();
        if !storage {
            b.charge_work(usize::MAX - ENTRY_WORK + 1).unwrap();
        }
        let result = Lease::open_at(
            &f.root.path().join("absent-parent"),
            uid(),
            gid(),
            &mut b,
            |_| panic!("overflow reached open"),
        );
        if storage {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(b.work(), Lease::ADMISSION_WORK);
            assert_eq!(b.failed_storage(), Some(usize::MAX));
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(b.work(), usize::MAX - ENTRY_WORK + 1);
            assert_eq!(b.failed_work(), Some(usize::MAX));
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor);
    }
}

#[test]
fn history_survives_success_path_refusal_and_unwind() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.charge_work(29).unwrap();
    b.reserve_storage(41).unwrap();
    b.reserve_storage(100_000).unwrap();
    b.release_storage(100_000).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
    let ledger = b.work_ledger_identity_v1();
    let lease = f.retain(&mut b);
    f.check(&lease, &mut b).unwrap();
    fs::rename(&f.lock, f.parent.join("displaced")).unwrap();
    write_lock(&f.lock);
    assert!(matches!(
        f.check(&lease, &mut b),
        Err(Error::Lease(LeaseError::PathChanged))
    ));
    assert_eq!(b.storage(), 41 + lease.retained_storage());
    retire(lease, &mut b);
    let result = catch_unwind(AssertUnwindSafe(|| {
        Lease::open_at(&f.parent, uid(), gid(), &mut b, |_| panic!("locked unwind"))
    }));
    assert!(result.is_err());
    assert_eq!(b.storage(), 41);
    assert_eq!(
        b.work(),
        29 + 2 * Lease::ADMISSION_WORK + 2 * Lease::REVALIDATION_WORK
    );
    assert_eq!(
        (b.failed_work(), b.failed_storage(), b.peak_storage()),
        history
    );
    assert!(ledger == b.work_ledger_identity_v1());
    assert_available(&f.lock);
}

#[test]
fn parent_and_lock_path_substitution_refuse_while_original_custody_survives() {
    for parent in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.retain(&mut b);
        let original = if parent {
            let displaced = f.root.path().join("displaced-parent");
            fs::rename(&f.parent, &displaced).unwrap();
            fs::create_dir(&f.parent).unwrap();
            fs::set_permissions(
                &f.parent,
                fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
            )
            .unwrap();
            write_lock(&f.lock);
            displaced.join(lifecycle_name().unwrap())
        } else {
            let displaced = f.parent.join("displaced-lock");
            fs::rename(&f.lock, &displaced).unwrap();
            write_lock(&f.lock);
            displaced
        };
        let result = f.check(&lease, &mut b);
        if parent {
            assert!(matches!(result, Err(Error::ParentChanged)));
        } else {
            assert!(matches!(result, Err(Error::Lease(LeaseError::PathChanged))));
        }
        assert_eq!(b.storage(), lease.retained_storage());
        assert_eq!(b.work(), Lease::ADMISSION_WORK + Lease::REVALIDATION_WORK);
        assert_blocked(&original);
        assert_available(&f.lock);
        retire(lease, &mut b);
        assert_available(&original);
    }
}

#[test]
fn post_lock_substitution_is_rejected_before_returning_an_owner() {
    for parent in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(Lease::ADMISSION_WORK);
        let mut b = Budget::new(&mut work, Lease::IO_STORAGE);
        let original = if parent {
            f.root
                .path()
                .join("displaced-parent")
                .join(lifecycle_name().unwrap())
        } else {
            f.parent.join("displaced-lock")
        };
        let result = Lease::open_at(&f.parent, uid(), gid(), &mut b, |_| {
            assert_blocked(&f.lock);
            if parent {
                fs::rename(&f.parent, original.parent().unwrap()).unwrap();
                fs::create_dir(&f.parent).unwrap();
                fs::set_permissions(
                    &f.parent,
                    fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
                )
                .unwrap();
            } else {
                fs::rename(&f.lock, &original).unwrap();
            }
            write_lock(&f.lock);
            Ok(())
        });
        if parent {
            assert!(matches!(result, Err(Error::ParentChanged)));
        } else {
            assert!(matches!(result, Err(Error::Lease(LeaseError::PathChanged))));
        }
        assert_eq!(b.storage(), 0);
        assert_eq!(b.work(), Lease::ADMISSION_WORK);
        assert_available(&original);
        assert_available(&f.lock);
    }
}

#[test]
fn close_only_drop_never_unlocks_a_private_test_duplicate() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.retain(&mut b);
    // Private white-box duplicate detects accidental LOCK_UN during drop.
    let alias = lease.file.try_clone().unwrap();
    retire(lease, &mut b);
    assert_blocked(&f.lock);
    drop(alias);
    assert_available(&f.lock);
}

#[test]
fn error_and_unwind_close_both_owned_descriptors_and_restore_floor() {
    for unwind in [false, true] {
        let f = Fixture::new();
        let witness = File::open(&f.lock).unwrap();
        let parent_witness = File::open(&f.parent).unwrap();
        let before = (refs(&witness), refs(&parent_witness));
        let floor = 13;
        let mut work = Work::new(Lease::ADMISSION_WORK);
        let mut b = Budget::new(&mut work, floor + Lease::IO_STORAGE);
        b.reserve_storage(floor).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            Lease::open_at(&f.parent, uid(), gid(), &mut b, |_| {
                assert_eq!(
                    (refs(&witness), refs(&parent_witness)),
                    (before.0 + 1, before.1 + 1)
                );
                assert_blocked(&f.lock);
                if unwind {
                    panic!("post-lock observer unwind");
                }
                Err(Error::ParentChanged)
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result.unwrap(), Err(Error::ParentChanged)));
        }
        assert_eq!((refs(&witness), refs(&parent_witness)), before);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor + Lease::IO_STORAGE);
        assert_eq!(b.work(), Lease::ADMISSION_WORK);
        assert_available(&f.lock);
    }
}

#[test]
fn invalid_parent_file_and_fifo_are_rejected_without_repair() {
    for shape in 0..6 {
        let f = Fixture::new();
        match shape {
            0 => fs::set_permissions(&f.parent, fs::Permissions::from_mode(0o777)).unwrap(),
            1 => fs::set_permissions(&f.lock, fs::Permissions::from_mode(0o600)).unwrap(),
            2 => {
                fs::set_permissions(&f.lock, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&f.lock, [1]).unwrap();
                fs::set_permissions(
                    &f.lock,
                    fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
                )
                .unwrap();
            }
            3 => fs::hard_link(&f.lock, f.parent.join("extra-link")).unwrap(),
            4 | 5 => {
                fs::remove_file(&f.lock).unwrap();
                if shape == 4 {
                    symlink("state", &f.lock).unwrap();
                } else {
                    rustix::fs::mknodat(CWD, &f.lock, rustix::fs::FileType::Fifo, Mode::RUSR, 0)
                        .unwrap();
                }
            }
            _ => unreachable!(),
        }
        let mut work = Work::new(Lease::ADMISSION_WORK);
        let mut b = Budget::new(&mut work, Lease::IO_STORAGE);
        let result = f.open(&mut b);
        match shape {
            0 => assert!(matches!(
                result,
                Err(Error::Lease(LeaseError::InvalidParent))
            )),
            4 => assert!(matches!(result, Err(Error::Lease(LeaseError::Io { .. })))),
            _ => assert!(matches!(result, Err(Error::Lease(LeaseError::InvalidFile)))),
        }
        assert_eq!(b.storage(), 0);
        assert_eq!(b.work(), Lease::ADMISSION_WORK);
    }
}

#[test]
fn retained_descriptor_flags_and_metadata_drift_are_rejected() {
    for shape in 0..6 {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.retain(&mut b);
        match shape {
            0 => rustix::io::fcntl_setfd(&lease.file, rustix::io::FdFlags::empty()).unwrap(),
            1 => rustix::io::fcntl_setfd(&lease.parent, rustix::io::FdFlags::empty()).unwrap(),
            2 => {
                let flags = rustix::fs::fcntl_getfl(&lease.file).unwrap();
                rustix::fs::fcntl_setfl(&lease.file, flags | OFlags::APPEND).unwrap();
            }
            3 => fs::set_permissions(&f.parent, fs::Permissions::from_mode(0o700)).unwrap(),
            4 => fs::set_permissions(&f.lock, fs::Permissions::from_mode(0o600)).unwrap(),
            5 => fs::hard_link(&f.lock, f.parent.join("extra-link")).unwrap(),
            _ => unreachable!(),
        }
        let result = f.check(&lease, &mut b);
        if matches!(shape, 1 | 3) {
            assert!(matches!(
                result,
                Err(Error::Lease(LeaseError::InvalidParent))
            ));
        } else {
            assert!(matches!(result, Err(Error::Lease(LeaseError::InvalidFile))));
        }
        assert_blocked(&f.lock);
        retire(lease, &mut b);
    }
}

#[test]
fn extended_access_and_default_acls_are_rejected_even_with_unchanged_mode() {
    for shape in 0..3 {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.retain(&mut b);
        let (file, attribute, owner_perm, other_perm) = match shape {
            0 => (&lease.file, "system.posix_acl_access", 4, 0),
            1 => (&lease.parent, "system.posix_acl_access", 7, 5),
            _ => (&lease.parent, "system.posix_acl_default", 7, 5),
        };
        let original_mode = file.metadata().unwrap().mode();
        // Linux ACL v2 with a named user; the mask preserves the existing mode.
        let mut acl = Vec::from(2_u32.to_le_bytes());
        for (tag, perm, id) in [
            (1_u16, owner_perm as u16, u32::MAX),
            // Keep the named entry valid in a single-UID user namespace too.
            (2, 0, uid()),
            (4, other_perm, u32::MAX),
            (16, other_perm, u32::MAX),
            (32, other_perm, u32::MAX),
        ] {
            acl.extend_from_slice(&tag.to_le_bytes());
            acl.extend_from_slice(&perm.to_le_bytes());
            acl.extend_from_slice(&id.to_le_bytes());
        }
        match rustix::fs::fsetxattr(file, attribute, &acl, rustix::fs::XattrFlags::empty()) {
            Ok(()) => {
                assert_eq!(file.metadata().unwrap().mode(), original_mode);
                assert!(matches!(
                    f.check(&lease, &mut b),
                    Err(Error::Lease(LeaseError::ForbiddenAttributes))
                ));
                assert!(matches!(
                    f.open(&mut b),
                    Err(Error::Lease(LeaseError::ForbiddenAttributes))
                ));
                assert_eq!(b.storage(), lease.retained_storage());
                assert_blocked(&f.lock);
            }
            Err(
                rustix::io::Errno::OPNOTSUPP | rustix::io::Errno::PERM | rustix::io::Errno::ACCESS,
            ) => {}
            Err(error) => panic!("unexpected ACL setup refusal: {error}"),
        }
        retire(lease, &mut b);
    }
}

#[test]
fn capability_metadata_is_rejected_when_the_fixture_can_set_it() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.retain(&mut b);
    // Linux revision-2 file capabilities: one permitted bit, no effective bits.
    let mut capability = [0_u8; 20];
    capability[..4].copy_from_slice(&0x0200_0000_u32.to_le_bytes());
    capability[4..8].copy_from_slice(&1_u32.to_le_bytes());
    match rustix::fs::fsetxattr(
        &lease.file,
        "security.capability",
        &capability,
        rustix::fs::XattrFlags::empty(),
    ) {
        Ok(()) => {
            assert!(matches!(
                f.check(&lease, &mut b),
                Err(Error::Lease(LeaseError::ForbiddenAttributes))
            ));
            assert!(matches!(
                f.open(&mut b),
                Err(Error::Lease(LeaseError::ForbiddenAttributes))
            ));
            assert_eq!(b.storage(), lease.retained_storage());
        }
        Err(rustix::io::Errno::OPNOTSUPP | rustix::io::Errno::PERM | rustix::io::Errno::ACCESS) => {
        }
        Err(error) => panic!("unexpected capability setup refusal: {error}"),
    }
    retire(lease, &mut b);
}

#[test]
fn canonical_parent_symlink_even_to_the_retained_inode_is_rejected() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.retain(&mut b);
    let moved = f.root.path().join("moved-parent");
    fs::rename(&f.parent, &moved).unwrap();
    symlink(&moved, &f.parent).unwrap();
    assert!(matches!(
        f.check(&lease, &mut b),
        Err(Error::Lease(LeaseError::Io { .. }))
    ));
    assert_blocked(&moved.join(lifecycle_name().unwrap()));
    retire(lease, &mut b);
    assert!(matches!(
        f.open(&mut b),
        Err(Error::Lease(LeaseError::Io { .. }))
    ));
    // An intermediate symlink must also fail, even if the final parent is a directory.
    let alias = f.root.path().join("ancestor-alias");
    symlink(f.root.path(), &alias).unwrap();
    assert!(matches!(
        Lease::open_at(
            &alias.join("moved-parent"),
            uid(),
            gid(),
            &mut b,
            |_| Ok(())
        ),
        Err(Error::Lease(LeaseError::Io { .. }))
    ));
}

#[test]
fn production_policy_is_fixed_and_never_substitutes_current_owner() {
    assert_eq!(canonical_parent(), Path::new("/var/lib/fe2o3"));
    assert_eq!(
        canonical_parent().join(lifecycle_name().unwrap()),
        Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
    );
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let result = Lease::open_at(&f.parent, ROOT_ID_V1, ROOT_ID_V1, &mut b, |_| Ok(()));
    if (uid(), gid()) == (ROOT_ID_V1, ROOT_ID_V1) {
        let (lease, charge) = result.unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        retire(lease, &mut b);
    } else {
        assert!(matches!(
            result,
            Err(Error::Lease(LeaseError::InvalidParent))
        ));
    }
}

#[cfg(feature = "test-support")]
#[test]
fn native_shared_and_exclusive_owners_exclude_each_other_in_both_directions() {
    use crate::CompilerExecutionServiceLifecycleLeaseV2 as Shared;

    for shared_first in [false, true] {
        let f = Fixture::new();
        let state = File::open(f.parent.join("state")).unwrap();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(Shared::STATE_ROOT_STORAGE).unwrap();
        if shared_first {
            let (shared, charge) =
                Shared::open_non_authoritative_same_owner_test(&state, &mut b).unwrap();
            b.reserve_storage(charge.additional_storage()).unwrap();
            assert!(matches!(
                f.open(&mut b),
                Err(Error::Lease(LeaseError::Busy))
            ));
            shared.revalidate(&mut b).unwrap();
            let retained = shared.retained_storage();
            drop(shared);
            b.release_storage(retained).unwrap();
            let exclusive = f.retain(&mut b);
            retire(exclusive, &mut b);
        } else {
            let exclusive = f.retain(&mut b);
            assert!(matches!(
                Shared::open_non_authoritative_same_owner_test(&state, &mut b),
                Err(Error::Lease(LeaseError::Busy))
            ));
            f.check(&exclusive, &mut b).unwrap();
            assert_blocked(&f.lock);
            retire(exclusive, &mut b);
            let (shared, charge) =
                Shared::open_non_authoritative_same_owner_test(&state, &mut b).unwrap();
            b.reserve_storage(charge.additional_storage()).unwrap();
            let retained = shared.retained_storage();
            drop(shared);
            b.release_storage(retained).unwrap();
        }
        assert_eq!(b.storage(), Shared::STATE_ROOT_STORAGE);
        assert_available(&f.lock);
    }
}

#[test]
fn same_policy_retained_descriptor_substitution_is_not_identity_equality() {
    for parent in [false, true] {
        let f = Fixture::new();
        let other = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let mut lease = f.retain(&mut b);
        if parent {
            lease.parent = File::open(&other.parent).unwrap();
        } else {
            lease.file = File::open(&other.lock).unwrap();
        }
        let result = f.check(&lease, &mut b);
        if parent {
            assert!(matches!(result, Err(Error::ParentChanged)));
        } else {
            assert!(matches!(result, Err(Error::Lease(LeaseError::FileChanged))));
        }
        assert_available(&other.lock);
        retire(lease, &mut b);
    }
}
