use super::*;
use crate::{COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1, LIFECYCLE_PARENT_MODE_V1};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{FlockOperation, flock};
use std::{
    fs,
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::fs::{MetadataExt, PermissionsExt},
    },
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::Mutex,
};

type Native = CompilerExecutionServiceLifecycleLeaseV2;
type Error = LifecycleLeaseErrorV2;
const LIMIT: usize = 1_000_000;
static PRIVATE_SLOT: Mutex<()> = Mutex::new(());

struct Fixture {
    root: tempfile::TempDir,
    state: File,
    lock: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(
            root.path(),
            fs::Permissions::from_mode(LIFECYCLE_PARENT_MODE_V1),
        )
        .unwrap();
        let state = root.path().join("state");
        fs::create_dir(&state).unwrap();
        let lock = root.path().join(lifecycle_name().unwrap());
        fs::write(&lock, []).unwrap();
        fs::set_permissions(
            &lock,
            fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
        )
        .unwrap();
        Self {
            root,
            state: File::open(state).unwrap(),
            lock,
        }
    }
    fn file(&self) -> File {
        File::open(&self.lock).unwrap()
    }
    fn admit(&self, budget: &mut Budget<'_>) -> Native {
        budget.reserve_storage(Native::FILE_STORAGE).unwrap();
        let (lease, charge) =
            Native::admit_for_owner(self.file(), &self.state, None, Owner::SameOwnerTest, budget)
                .unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        lease
    }
    fn exclusive_blocked(&self) {
        assert_eq!(
            flock(self.file(), FlockOperation::NonBlockingLockExclusive),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
    }
    fn exclusive_available(&self) {
        flock(self.file(), FlockOperation::NonBlockingLockExclusive).unwrap();
    }
}

fn retire(lease: Native, budget: &mut Budget<'_>) {
    let charge = lease.retained_storage();
    drop(lease);
    budget.release_storage(charge).unwrap();
}

fn refs(file: &File) -> usize {
    let metadata = file.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| fs::metadata(entry.ok()?.path()).ok())
        .filter(|other| (other.dev(), other.ino()) == (metadata.dev(), metadata.ino()))
        .count()
}

#[test]
fn admission_and_open_exact_quotas_return_growth_and_full_storage() {
    for opening in [false, true] {
        let fixture = Fixture::new();
        let floor = Native::STATE_ROOT_STORAGE + if opening { 0 } else { Native::FILE_STORAGE };
        let mut work = Work::new(Native::ADMISSION_WORK + Native::REVALIDATION_WORK);
        let mut budget = Budget::new(&mut work, floor + Native::IO_STORAGE);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let (lease, charge) = if opening {
            Native::open_for_owner(&fixture.state, Owner::SameOwnerTest, &mut budget)
        } else {
            Native::admit_for_owner(
                fixture.file(),
                &fixture.state,
                None,
                Owner::SameOwnerTest,
                &mut budget,
            )
        }
        .unwrap();
        assert_eq!(budget.work(), Native::ADMISSION_WORK);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor + Native::IO_STORAGE);
        assert_eq!(
            charge.additional_storage(),
            lease.retained_storage() - if opening { 0 } else { Native::FILE_STORAGE }
        );
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(
            budget.storage(),
            Native::STATE_ROOT_STORAGE + lease.retained_storage()
        );
        fixture.exclusive_blocked();
        // Revalidation needs a fresh scratch allowance above the newly retained growth.
        let mut revalidate_work = Work::new(Native::REVALIDATION_WORK);
        let mut revalidate = Budget::new(
            &mut revalidate_work,
            lease.retained_storage() + Native::IO_STORAGE,
        );
        revalidate
            .reserve_storage(lease.retained_storage())
            .unwrap();
        lease.revalidate(&mut revalidate).unwrap();
        assert_eq!(revalidate.work(), Native::REVALIDATION_WORK);
        assert!(ledger == budget.work_ledger_identity_v1());
        retire(lease, &mut budget);
        assert_eq!(budget.storage(), Native::STATE_ROOT_STORAGE);
        fixture.exclusive_available();
    }
}

struct UnreachableRoot;
impl AsFd for UnreachableRoot {
    fn as_fd(&self) -> BorrowedFd<'_> {
        panic!("unpaid descriptor access")
    }
}

#[test]
fn admission_and_open_refuse_short_resources_before_touching_the_root() {
    for opening in [false, true] {
        for mode in 0..4 {
            let fixture = Fixture::new();
            let file = fixture.file();
            let witness = file.try_clone().unwrap();
            let required =
                Native::STATE_ROOT_STORAGE + if opening { 0 } else { Native::FILE_STORAGE };
            let floor = required - usize::from(mode == 0);
            let work_limit = match mode {
                1 => ENTRY_WORK - 1,
                2 => Native::ADMISSION_WORK - 1,
                _ => Native::ADMISSION_WORK,
            };
            let scratch = Native::IO_STORAGE - usize::from(mode == 3);
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, floor + scratch);
            budget.reserve_storage(floor).unwrap();
            let result = if opening {
                drop(file);
                Native::open_for_owner(&UnreachableRoot, Owner::SameOwnerTest, &mut budget)
            } else {
                Native::admit_for_owner(
                    file,
                    &UnreachableRoot,
                    None,
                    Owner::SameOwnerTest,
                    &mut budget,
                )
            };
            match mode {
                0 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
                1 | 2 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
            assert_eq!(
                budget.work(),
                match mode {
                    1 => 0,
                    3 => Native::ADMISSION_WORK,
                    _ => ENTRY_WORK,
                }
            );
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(refs(&witness), 1);
            fixture.exclusive_available();
        }
    }
}

#[test]
fn retained_owner_revalidation_requires_exact_floor_work_and_scratch() {
    let fixture = Fixture::new();
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    setup.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
    let lease = fixture.admit(&mut setup);
    for mode in 0..5 {
        let floor = lease.retained_storage() - usize::from(mode == 1);
        let mut work = Work::new(match mode {
            2 => ENTRY_WORK - 1,
            3 => Native::REVALIDATION_WORK - 1,
            _ => Native::REVALIDATION_WORK,
        });
        let scratch = Native::IO_STORAGE - usize::from(mode == 4);
        let mut budget = Budget::new(&mut work, floor + scratch);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = lease.revalidate(&mut budget);
        match mode {
            0 => result.unwrap(),
            1 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            2 | 3 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
        }
        assert_eq!(
            budget.work(),
            match mode {
                2 => 0,
                1 | 3 => ENTRY_WORK,
                _ => Native::REVALIDATION_WORK,
            }
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            budget.peak_storage(),
            if mode == 0 { floor + scratch } else { floor }
        );
        assert!(ledger == budget.work_ledger_identity_v1());
        fixture.exclusive_blocked();
    }
    retire(lease, &mut setup);
    fixture.exclusive_available();
}

#[test]
fn history_and_work_prefix_survive_success_and_policy_refusal() {
    let fixture = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
    let lease = fixture.admit(&mut budget);
    budget.reserve_storage(100_000).unwrap();
    budget.release_storage(100_000).unwrap();
    assert!(budget.charge_work(usize::MAX).is_err());
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let history = (
        budget.failed_work(),
        budget.failed_storage(),
        budget.peak_storage(),
    );
    let floor = budget.storage();
    let before = budget.work();
    let ledger = budget.work_ledger_identity_v1();
    lease.revalidate(&mut budget).unwrap();
    fs::rename(&fixture.lock, fixture.root.path().join("displaced")).unwrap();
    fs::write(&fixture.lock, []).unwrap();
    fs::set_permissions(
        &fixture.lock,
        fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
    )
    .unwrap();
    assert!(matches!(
        lease.revalidate(&mut budget),
        Err(Error::Lease(LeaseError::PathChanged))
    ));
    assert_eq!(budget.work(), before + 2 * Native::REVALIDATION_WORK);
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        (
            budget.failed_work(),
            budget.failed_storage(),
            budget.peak_storage()
        ),
        history
    );
    assert!(ledger == budget.work_ledger_identity_v1());
    retire(lease, &mut budget);
}

#[test]
fn work_and_storage_overflow_reject_without_admission() {
    for storage_overflow in [false, true] {
        let fixture = Fixture::new();
        let file = fixture.file();
        let witness = file.try_clone().unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let floor = if storage_overflow {
            usize::MAX - Native::IO_STORAGE + 1
        } else {
            Native::FILE_STORAGE + Native::STATE_ROOT_STORAGE
        };
        budget.reserve_storage(floor).unwrap();
        if !storage_overflow {
            budget.charge_work(usize::MAX - ENTRY_WORK + 1).unwrap();
        }
        let result = Native::admit_for_owner(
            file,
            &UnreachableRoot,
            None,
            Owner::SameOwnerTest,
            &mut budget,
        );
        if storage_overflow {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(budget.work(), Native::ADMISSION_WORK);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(budget.work(), usize::MAX - ENTRY_WORK + 1);
            assert_eq!(budget.failed_work(), Some(usize::MAX));
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(refs(&witness), 1);
    }
}

#[test]
fn unwind_closes_consumed_file_and_restores_only_scratch() {
    let fixture = Fixture::new();
    let file = fixture.file();
    let witness = file.try_clone().unwrap();
    let mut work = Work::new(Native::ADMISSION_WORK);
    let floor = Native::FILE_STORAGE + Native::STATE_ROOT_STORAGE;
    let mut budget = Budget::new(&mut work, floor + Native::IO_STORAGE);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = catch_unwind(AssertUnwindSafe(|| {
        Native::admit_for_owner(
            file,
            &UnreachableRoot,
            None,
            Owner::SameOwnerTest,
            &mut budget,
        )
    }));
    assert!(result.is_err());
    assert_eq!(refs(&witness), 1);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor + Native::IO_STORAGE);
    assert_eq!(budget.work(), Native::ADMISSION_WORK);
    assert!(ledger == budget.work_ledger_identity_v1());
}

#[test]
fn close_only_drop_preserves_an_existing_open_file_description_duplicate() {
    let fixture = Fixture::new();
    let file = fixture.file();
    let duplicate = file.try_clone().unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(Native::FILE_STORAGE + Native::STATE_ROOT_STORAGE)
        .unwrap();
    let (lease, charge) = Native::admit_for_owner(
        file,
        &fixture.state,
        None,
        Owner::SameOwnerTest,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    fixture.exclusive_blocked();
    retire(lease, &mut budget);
    fixture.exclusive_blocked();
    assert_eq!(refs(&duplicate), 1);
    drop(duplicate);
    fixture.exclusive_available();
}

#[test]
fn native_open_and_legacy_leases_use_independent_shared_locks() {
    let fixture = Fixture::new();
    let (uid, gid) = Owner::SameOwnerTest.ids();
    let legacy = Lease::open_for_owner(&fixture.state, uid, gid).unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
    let (native, charge) =
        Native::open_for_owner(&fixture.state, Owner::SameOwnerTest, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    // An unrelated V1 open description can unlock without releasing native custody.
    flock(&legacy.file, FlockOperation::Unlock).unwrap();
    fixture.exclusive_blocked();
    native.revalidate(&mut budget).unwrap();
    legacy.revalidate().unwrap();
    retire(native, &mut budget);
    fixture.exclusive_blocked();
    drop(legacy);
    fixture.exclusive_available();
}

#[test]
fn exclusive_provisioner_lock_refuses_and_closes_native_input() {
    let fixture = Fixture::new();
    let exclusive = fixture.file();
    flock(&exclusive, FlockOperation::NonBlockingLockExclusive).unwrap();
    let file = fixture.file();
    let witness = file.try_clone().unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = Native::FILE_STORAGE + Native::STATE_ROOT_STORAGE;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        Native::admit_for_owner(
            file,
            &fixture.state,
            None,
            Owner::SameOwnerTest,
            &mut budget
        ),
        Err(Error::Lease(LeaseError::Busy))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(refs(&witness), 2); // The exclusive File and its independent witness remain.
    drop(exclusive);
    fixture.exclusive_available();
}

#[test]
fn exact_private_parent_slot_survives_root_drop_and_busy_slot_is_not_overwritten() {
    let _serial = PRIVATE_SLOT.lock().unwrap();
    let fixture = Fixture::new();
    let reservation = rustix::io::fcntl_dupfd_cloexec(&fixture.state, 512).unwrap();
    let slot = reservation.as_raw_fd();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(Native::FILE_STORAGE + Native::STATE_ROOT_STORAGE)
        .unwrap();
    let file = fixture.file();
    let witness = file.try_clone().unwrap();
    assert!(
        matches!(Native::admit_for_owner(file, &fixture.state, Some(slot), Owner::SameOwnerTest,
        &mut budget), Err(Error::Lease(LeaseError::PrivateParentDescriptorBusy { descriptor })) if descriptor == slot)
    );
    assert_eq!(refs(&witness), 1);
    assert_eq!(
        fstat(&reservation).unwrap().st_ino,
        fstat(&fixture.state).unwrap().st_ino
    );
    drop(reservation);
    // Ordinary opens use low slots; this test is the only native exact-slot user.
    let (lease, charge) = Native::admit_for_owner(
        fixture.file(),
        &fixture.state,
        Some(slot),
        Owner::SameOwnerTest,
        &mut budget,
    )
    .unwrap();
    assert_eq!(lease.inner.parent.as_raw_fd(), slot);
    budget.reserve_storage(charge.additional_storage()).unwrap();
    drop(fixture.state);
    budget.release_storage(Native::STATE_ROOT_STORAGE).unwrap();
    lease.revalidate(&mut budget).unwrap();
    retire(lease, &mut budget);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn same_policy_parent_and_file_substitutions_are_not_identity_matches() {
    for parent in [false, true] {
        let fixture = Fixture::new();
        let other = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
        let mut lease = fixture.admit(&mut budget);
        // Model exact-slot replacement internally without exposing a public rebinding API.
        if parent {
            lease.inner.parent = File::open(other.root.path()).unwrap();
        } else {
            lease.inner.file = other.file();
        }
        let result = lease.revalidate(&mut budget);
        if parent {
            assert!(matches!(result, Err(Error::ParentChanged)));
        } else {
            assert!(matches!(result, Err(Error::Lease(LeaseError::FileChanged))));
        }
        retire(lease, &mut budget);
    }
}

#[test]
fn shared_file_and_parent_policy_and_nonblocking_open_remain_fail_closed() {
    for shape in 0..6 {
        let fixture = Fixture::new();
        match shape {
            0 => {
                fs::set_permissions(fixture.root.path(), fs::Permissions::from_mode(0o777)).unwrap()
            }
            1 => fs::set_permissions(&fixture.lock, fs::Permissions::from_mode(0o600)).unwrap(),
            2 => {
                fs::set_permissions(&fixture.lock, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&fixture.lock, [1]).unwrap();
                fs::set_permissions(
                    &fixture.lock,
                    fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
                )
                .unwrap();
            }
            3 => fs::hard_link(&fixture.lock, fixture.root.path().join("extra-link")).unwrap(),
            4 | 5 => {
                fs::remove_file(&fixture.lock).unwrap();
                if shape == 4 {
                    std::os::unix::fs::symlink("state", &fixture.lock).unwrap();
                } else {
                    rustix::fs::mknodat(
                        rustix::fs::CWD,
                        &fixture.lock,
                        rustix::fs::FileType::Fifo,
                        Mode::RUSR,
                        0,
                    )
                    .unwrap();
                }
            }
            _ => unreachable!(),
        }
        let mut work = Work::new(Native::ADMISSION_WORK);
        let mut budget = Budget::new(&mut work, Native::STATE_ROOT_STORAGE + Native::IO_STORAGE);
        budget.reserve_storage(Native::STATE_ROOT_STORAGE).unwrap();
        let result = Native::open_for_owner(&fixture.state, Owner::SameOwnerTest, &mut budget);
        match shape {
            0 => assert!(matches!(
                result,
                Err(Error::Lease(LeaseError::InvalidParent))
            )),
            4 => assert!(matches!(result, Err(Error::Lease(LeaseError::Io { .. })))),
            _ => assert!(matches!(result, Err(Error::Lease(LeaseError::InvalidFile)))),
        }
        assert_eq!(budget.storage(), Native::STATE_ROOT_STORAGE);
    }
}

#[test]
fn production_admission_never_substitutes_current_owner_for_root() {
    let fixture = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(Native::FILE_STORAGE + Native::STATE_ROOT_STORAGE)
        .unwrap();
    let result = Native::admit(fixture.file(), &fixture.state, &mut budget);
    if Owner::SameOwnerTest.ids() == (ROOT_ID_V1, ROOT_ID_V1) {
        let (lease, charge) = result.unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        retire(lease, &mut budget);
    } else {
        assert!(matches!(
            result,
            Err(Error::Lease(LeaseError::InvalidParent))
        ));
    }
}

#[cfg(feature = "test-support")]
#[test]
fn explicitly_named_feature_fixture_methods_use_the_same_metered_contract() {
    let fixture = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(Native::FILE_STORAGE + Native::STATE_ROOT_STORAGE)
        .unwrap();
    let (lease, charge) = Native::admit_non_authoritative_same_owner_test(
        fixture.file(),
        &fixture.state,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    retire(lease, &mut budget);
    let (lease, charge) =
        Native::open_non_authoritative_same_owner_test(&fixture.state, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    retire(lease, &mut budget);
    assert_eq!(budget.storage(), Native::STATE_ROOT_STORAGE);
}
