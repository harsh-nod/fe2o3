use super::*;
use crate::{COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1, LIFECYCLE_PARENT_MODE_V1};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{FlockOperation, flock};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
};

type Native = CompilerExecutionServiceLifecycleLeaseV2;
type Failure = LifecycleLeaseErrorV2;
const LIMIT: usize = 2_000_000;

struct Fixture {
    parent: tempfile::TempDir,
    state: File,
    lock: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let parent = tempfile::tempdir().unwrap();
        fs::set_permissions(
            parent.path(),
            fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        let state = parent.path().join("state");
        fs::create_dir(&state).unwrap();
        let lock = parent.path().join(lifecycle_name().unwrap());
        fs::write(&lock, []).unwrap();
        fs::set_permissions(
            &lock,
            fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        Self {
            parent,
            state: File::open(state).unwrap(),
            lock,
        }
    }

    fn admit(&self, b: &mut Budget<'_>) -> Native {
        b.reserve_storage(Native::STATE_ROOT_STORAGE + Native::FILE_STORAGE)
            .unwrap();
        let (lease, growth) = Native::admit_for_owner(
            File::open(&self.lock).unwrap(),
            &self.state,
            None,
            Owner::SameOwnerTest,
            b,
        )
        .unwrap();
        b.reserve_storage(growth.additional_storage()).unwrap();
        lease
    }

    fn blocked(&self) {
        assert_eq!(
            flock(
                File::open(&self.lock).unwrap(),
                FlockOperation::NonBlockingLockExclusive,
            ),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
    }
}

fn retire(lease: Native, b: &mut Budget<'_>) {
    let charge = lease.retained_storage();
    drop(lease);
    b.release_storage(charge).unwrap();
}

// The witness remains owned throughout each comparison, preventing inode reuse.
fn refs(witness: &File) -> usize {
    let expected = witness.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter_map(|entry| match fs::metadata(entry.path()) {
            Ok(m) => Some(m),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => panic!("inspect descriptor: {e}"),
        })
        .filter(|m| (m.dev(), m.ino()) == (expected.dev(), expected.ino()))
        .count()
}

#[test]
fn exact_root_binding_work_floor_and_scratch_stay_on_original_ledger() {
    let f = Fixture::new();
    let floor = Native::RETAINED + Native::STATE_ROOT_STORAGE;
    let prior_work = 17;
    let mut work = Work::new(prior_work + Native::ADMISSION_WORK + Native::ROOT_BINDING_WORK);
    let mut b = Budget::new(&mut work, floor + Native::ROOT_BINDING_SCRATCH);
    b.charge_work(prior_work).unwrap();
    let lease = f.admit(&mut b);
    assert_eq!(b.storage(), floor);
    let ledger = b.work_ledger_identity_v1();
    let before = refs(&lease.inner.parent);
    lease.revalidate_for_root(&f.state, &mut b).unwrap();
    assert_eq!(refs(&lease.inner.parent), before);
    assert_eq!(b.storage(), floor);
    assert_eq!(b.peak_storage(), floor + Native::ROOT_BINDING_SCRATCH);
    assert_eq!(
        b.work(),
        prior_work + Native::ADMISSION_WORK + Native::ROOT_BINDING_WORK
    );
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
    assert!(ledger == b.work_ledger_identity_v1());
    f.blocked();
    retire(lease, &mut b);
    flock(
        File::open(&f.lock).unwrap(),
        FlockOperation::NonBlockingLockExclusive,
    )
    .unwrap();
    drop(f);
    b.release_storage(Native::STATE_ROOT_STORAGE).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn same_root_alias_and_sibling_root_share_the_canonical_parent_lock() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    b.reserve_storage(2 * Native::STATE_ROOT_STORAGE).unwrap();
    let alias = File::from(rustix::io::fcntl_dupfd_cloexec(&f.state, 0).unwrap());
    let sibling = f.parent.path().join("sibling-state");
    fs::create_dir(&sibling).unwrap();
    let sibling = File::open(sibling).unwrap();
    let before = b.work();
    for root in [&f.state, &alias, &sibling] {
        lease.revalidate_for_root(root, &mut b).unwrap();
    }
    assert_eq!(b.work(), before + 3 * Native::ROOT_BINDING_WORK);
    assert_eq!(
        b.storage(),
        lease.retained_storage() + 3 * Native::STATE_ROOT_STORAGE
    );
    drop((alias, sibling));
    b.release_storage(2 * Native::STATE_ROOT_STORAGE).unwrap();
    f.blocked();
    retire(lease, &mut b);
}

#[test]
fn independently_valid_leases_do_not_bind_each_others_roots() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    let other = Fixture::new();
    let other_lease = other.admit(&mut b);
    let floor = b.storage();
    let before = b.work();
    for (lease, own, wrong) in [(&lease, &f, &other), (&other_lease, &other, &f)] {
        lease.revalidate_for_root(&own.state, &mut b).unwrap();
        let parent_refs = refs(&lease.inner.parent);
        assert!(matches!(
            lease.revalidate_for_root(&wrong.state, &mut b),
            Err(Failure::ParentChanged)
        ));
        assert_eq!(refs(&lease.inner.parent), parent_refs);
        assert_eq!(b.storage(), floor);
        own.blocked();
    }
    assert_eq!(b.work(), before + 4 * Native::ROOT_BINDING_WORK);
    retire(other_lease, &mut b);
    retire(lease, &mut b);
}

#[test]
fn moving_the_actual_root_refuses_even_while_retained_lease_remains_valid() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    let other = Fixture::new();
    b.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
    let original = f.state.metadata().unwrap();
    fs::rename(
        f.parent.path().join("state"),
        other.parent.path().join("moved-state"),
    )
    .unwrap();
    let moved = f.state.metadata().unwrap();
    assert_eq!((original.dev(), original.ino()), (moved.dev(), moved.ino()));
    lease.revalidate(&mut b).unwrap();
    let before = b.storage();
    assert!(matches!(
        lease.revalidate_for_root(&f.state, &mut b),
        Err(Failure::ParentChanged)
    ));
    assert_eq!(b.storage(), before);
    f.blocked();
    retire(lease, &mut b);
}

#[test]
fn non_directory_borrowed_root_is_refused_without_closing_or_unlocking_inputs() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    b.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
    let wrong = File::open(&f.lock).unwrap();
    let before = refs(&wrong);
    let floor = b.storage();
    assert!(matches!(
        lease.revalidate_for_root(&wrong, &mut b),
        Err(Failure::Lease(LeaseError::Io { .. }))
    ));
    assert_eq!(refs(&wrong), before);
    assert_eq!(b.storage(), floor);
    wrong.metadata().unwrap();
    f.blocked();
    drop(wrong);
    b.release_storage(Native::STATE_ROOT_STORAGE).unwrap();
    retire(lease, &mut b);
}

#[test]
fn short_root_binding_resources_refuse_before_invalid_owner_inspection() {
    for short in 0..4 {
        let f = Fixture::new();
        let full = Native::RETAINED + Native::STATE_ROOT_STORAGE;
        let root_work = match short {
            1 => ENTRY_WORK - 1,
            2 => Native::ROOT_BINDING_WORK - 1,
            _ => Native::ROOT_BINDING_WORK,
        };
        let mut work = Work::new(Native::ADMISSION_WORK + root_work);
        let mut b = Budget::new(
            &mut work,
            full + Native::ROOT_BINDING_SCRATCH - usize::from(short == 3),
        );
        let lease = f.admit(&mut b);
        // Deliberately violate one unit of the caller's prepaid floor for this refusal case.
        if short == 0 {
            b.release_storage(1).unwrap();
        }
        fs::set_permissions(&f.lock, fs::Permissions::from_mode(0o600)).unwrap();
        let floor = b.storage();
        let peak = b.peak_storage();
        let ledger = b.work_ledger_identity_v1();
        let before = refs(&lease.inner.parent);
        let result =
            lease.revalidate_for_root_with(&f.state, &mut b, |_| panic!("unpaid root binding"));
        match short {
            0 => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Accounting))
            )),
            1 | 2 => assert!(matches!(result, Err(Failure::Resource(Resource::Work(_))))),
            _ => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Storage(_)))
            )),
        }
        assert_eq!(
            b.work(),
            Native::ADMISSION_WORK
                + match short {
                    1 => 0,
                    3 => Native::ROOT_BINDING_WORK,
                    _ => ENTRY_WORK,
                }
        );
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), peak);
        assert!(ledger == b.work_ledger_identity_v1());
        assert_eq!(refs(&lease.inner.parent), before);
        assert_eq!(
            b.failed_work(),
            match short {
                1 => Some(Native::ADMISSION_WORK + ENTRY_WORK),
                2 => Some(Native::ADMISSION_WORK + Native::ROOT_BINDING_WORK),
                _ => None,
            }
        );
        assert_eq!(
            b.failed_storage(),
            (short == 3).then_some(full + Native::ROOT_BINDING_SCRATCH)
        );
        f.blocked();
        if short == 0 {
            b.reserve_storage(1).unwrap();
        }
        retire(lease, &mut b);
    }
}

#[test]
fn root_binding_storage_and_work_overflow_preserve_custody_and_entry_storage() {
    for storage in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        let lease = f.admit(&mut b);
        if storage {
            b.reserve_storage(usize::MAX - Native::ROOT_BINDING_SCRATCH + 1 - b.storage())
                .unwrap();
        } else {
            b.charge_work(usize::MAX - ENTRY_WORK + 1 - b.work())
                .unwrap();
        }
        let floor = b.storage();
        let peak = b.peak_storage();
        let before = b.work();
        let parent_refs = refs(&lease.inner.parent);
        let result = lease.revalidate_for_root(&f.state, &mut b);
        if storage {
            assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Storage(_)))
            ));
            assert_eq!(b.work(), before + Native::ROOT_BINDING_WORK);
            assert_eq!(b.failed_storage(), Some(usize::MAX));
        } else {
            assert!(matches!(result, Err(Failure::Resource(Resource::Work(_)))));
            assert_eq!(b.work(), before);
            assert_eq!(b.failed_work(), Some(usize::MAX));
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), peak);
        assert_eq!(refs(&lease.inner.parent), parent_refs);
        f.blocked();
        retire(lease, &mut b);
    }
}

#[test]
fn root_binding_success_and_context_refusal_preserve_prior_history() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    let other = Fixture::new();
    b.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
    b.reserve_storage(100_000).unwrap();
    b.release_storage(100_000).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
    let ledger = b.work_ledger_identity_v1();
    let floor = b.storage();
    let before = b.work();
    lease.revalidate_for_root(&f.state, &mut b).unwrap();
    assert!(matches!(
        lease.revalidate_for_root(&other.state, &mut b),
        Err(Failure::ParentChanged)
    ));
    assert_eq!(b.work(), before + 2 * Native::ROOT_BINDING_WORK);
    assert_eq!(b.storage(), floor);
    assert_eq!(
        (b.failed_work(), b.failed_storage(), b.peak_storage()),
        history
    );
    assert!(ledger == b.work_ledger_identity_v1());
    f.blocked();
    retire(lease, &mut b);
}

#[test]
fn temporary_parent_closes_on_injected_error_and_unwind_without_refunding_work() {
    for unwind in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.admit(&mut b);
        b.reserve_storage(100_000).unwrap();
        b.release_storage(100_000).unwrap();
        assert!(b.charge_work(usize::MAX).is_err());
        assert!(b.reserve_storage(usize::MAX).is_err());
        let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
        let floor = b.storage();
        let before = b.work();
        let ledger = b.work_ledger_identity_v1();
        let parent_refs = refs(&lease.inner.parent);
        let root_refs = refs(&f.state);
        let result = catch_unwind(AssertUnwindSafe(|| {
            lease.revalidate_for_root_with(&f.state, &mut b, |parent| {
                assert_eq!(refs(parent), parent_refs + 1);
                if unwind {
                    panic!("temporary root-binding parent is live");
                }
                Err(LeaseError::InvalidParent.into())
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(Failure::Lease(LeaseError::InvalidParent)))
            ));
        }
        assert_eq!(refs(&lease.inner.parent), parent_refs);
        assert_eq!(refs(&f.state), root_refs);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), before + Native::ROOT_BINDING_WORK);
        assert_eq!(
            (b.failed_work(), b.failed_storage(), b.peak_storage()),
            history
        );
        assert!(ledger == b.work_ledger_identity_v1());
        f.blocked();
        lease.revalidate_for_root(&f.state, &mut b).unwrap();
        retire(lease, &mut b);
    }
}

#[test]
fn retained_parent_and_file_policy_drift_refuse_root_binding() {
    for parent in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.admit(&mut b);
        if parent {
            fs::set_permissions(f.parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
        } else {
            fs::set_permissions(&f.lock, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let floor = b.storage();
        let before = b.work();
        let result = lease.revalidate_for_root(&f.state, &mut b);
        if parent {
            assert!(matches!(
                result,
                Err(Failure::Lease(LeaseError::InvalidParent))
            ));
        } else {
            assert!(matches!(
                result,
                Err(Failure::Lease(LeaseError::InvalidFile))
            ));
        }
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), before + Native::ROOT_BINDING_WORK);
        f.blocked();
        retire(lease, &mut b);
    }
}

#[test]
fn freshly_derived_parent_policy_and_canonical_sibling_are_actually_checked() {
    for parent in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.admit(&mut b);
        let floor = b.storage();
        let before = b.work();
        let parent_refs = refs(&lease.inner.parent);
        let result = lease.revalidate_for_root_with(&f.state, &mut b, |_| {
            if parent {
                fs::set_permissions(f.parent.path(), fs::Permissions::from_mode(0o700)).unwrap();
            } else {
                fs::rename(&f.lock, f.parent.path().join("displaced-lock")).unwrap();
                fs::write(&f.lock, []).unwrap();
                fs::set_permissions(
                    &f.lock,
                    fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
                )
                .unwrap();
            }
            Ok(())
        });
        if parent {
            assert!(matches!(
                result,
                Err(Failure::Lease(LeaseError::InvalidParent))
            ));
        } else {
            assert!(matches!(
                result,
                Err(Failure::Lease(LeaseError::PathChanged))
            ));
        }
        assert_eq!(refs(&lease.inner.parent), parent_refs);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), before + Native::ROOT_BINDING_WORK);
        // Check the original owned inode, not the replaced pathname.
        let held = File::open(format!(
            "/proc/self/fd/{}",
            std::os::fd::AsRawFd::as_raw_fd(&lease.inner.file)
        ))
        .unwrap();
        assert_eq!(
            flock(&held, FlockOperation::NonBlockingLockExclusive),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
        drop(held);
        retire(lease, &mut b);
    }
}
