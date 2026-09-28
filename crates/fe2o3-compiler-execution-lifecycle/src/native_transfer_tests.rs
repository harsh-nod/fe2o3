use super::*;
use crate::{COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1, LIFECYCLE_PARENT_MODE_V1};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use rustix::fs::{FlockOperation, flock};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

type Native = CompilerExecutionServiceLifecycleLeaseV2;
type Error = LifecycleLeaseErrorV2;
const LIMIT: usize = 2_000_000;

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
    fn admit(&self, b: &mut Budget<'_>) -> Native {
        b.reserve_storage(Native::STATE_ROOT_STORAGE + Native::FILE_STORAGE)
            .unwrap();
        let (lease, c) =
            Native::admit_for_owner(self.file(), &self.state, None, Owner::SameOwnerTest, b)
                .unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        lease
    }
    fn blocked(&self) {
        assert_eq!(
            flock(self.file(), FlockOperation::NonBlockingLockExclusive),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
    }
    fn available(&self) {
        flock(self.file(), FlockOperation::NonBlockingLockExclusive).unwrap();
    }
}

fn retire(lease: Native, b: &mut Budget<'_>) {
    let n = lease.retained_storage();
    drop(lease);
    b.release_storage(n).unwrap();
}
fn transfer(lease: &Native, b: &mut Budget<'_>) -> File {
    let (file, c) = lease.try_clone_for_transfer(b).unwrap();
    assert_eq!(c.additional_storage(), Native::FILE_STORAGE);
    b.reserve_storage(c.additional_storage()).unwrap();
    file
}
fn retire_file(file: File, b: &mut Budget<'_>) {
    drop(file);
    b.release_storage(Native::FILE_STORAGE).unwrap();
}
fn refs(file: &File) -> usize {
    let m = file.metadata().unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter_map(|entry| match fs::metadata(entry.path()) {
            Ok(m) => Some(m),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => panic!("inspect descriptor: {e}"),
        })
        .filter(|other| (m.dev(), m.ino()) == (other.dev(), other.ino()))
        .count()
}

#[test]
fn exact_transfer_quotas_return_full_file_charge_and_restore_storage() {
    let f = Fixture::new();
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let lease = f.admit(&mut setup);
    let candidate = transfer(&lease, &mut setup);
    for validating in [false, true] {
        let floor = lease.retained_storage() + if validating { Native::FILE_STORAGE } else { 0 };
        let mut work = Work::new(Native::TRANSFER_WORK);
        let mut b = Budget::new(&mut work, floor + Native::IO_STORAGE);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let before = refs(&candidate);
        if validating {
            lease.validate_transfer(&candidate, &mut b).unwrap();
        } else {
            let file = transfer(&lease, &mut b);
            assert_eq!(
                rustix::io::fcntl_getfd(&file).unwrap(),
                rustix::io::FdFlags::CLOEXEC
            );
            assert_eq!(refs(&candidate), before + 1);
            retire_file(file, &mut b);
        }
        assert_eq!(refs(&candidate), before);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor + Native::IO_STORAGE);
        assert_eq!(b.work(), Native::TRANSFER_WORK);
        assert_eq!(b.failed_work(), None);
        assert_eq!(b.failed_storage(), None);
        assert!(ledger == b.work_ledger_identity_v1());
    }
    retire_file(candidate, &mut setup);
    retire(lease, &mut setup);
    f.available();
}

#[test]
fn short_transfer_resources_refuse_before_owner_or_candidate_checks() {
    let f = Fixture::new();
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let lease = f.admit(&mut setup);
    let candidate = transfer(&lease, &mut setup);
    // An unfunded operation must return Resource, before inspecting this invalid owner.
    fs::set_permissions(&f.lock, fs::Permissions::from_mode(0o600)).unwrap();
    for validating in [false, true] {
        for mode in 0..4 {
            let required =
                lease.retained_storage() + if validating { Native::FILE_STORAGE } else { 0 };
            let floor = required - usize::from(mode == 0);
            let mut work = Work::new(match mode {
                1 => ENTRY_WORK - 1,
                2 => Native::TRANSFER_WORK - 1,
                _ => Native::TRANSFER_WORK,
            });
            let mut b = Budget::new(
                &mut work,
                floor + Native::IO_STORAGE - usize::from(mode == 3),
            );
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let before = refs(&candidate);
            let result = if validating {
                lease.validate_transfer(&candidate, &mut b)
            } else {
                lease
                    .clone_for_transfer_with(&mut b, |_| panic!("unpaid duplicate"))
                    .map(|_| ())
            };
            match mode {
                0 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
                1 | 2 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
            assert_eq!(
                b.work(),
                match mode {
                    1 => 0,
                    3 => Native::TRANSFER_WORK,
                    _ => ENTRY_WORK,
                }
            );
            assert_eq!(b.storage(), floor);
            assert_eq!(b.peak_storage(), floor);
            assert!(ledger == b.work_ledger_identity_v1());
            assert_eq!(refs(&candidate), before);
            f.blocked();
        }
    }
    retire_file(candidate, &mut setup);
    retire(lease, &mut setup);
    f.available();
}

#[test]
fn transfer_overflow_refuses_without_descriptor_growth() {
    let f = Fixture::new();
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let lease = f.admit(&mut setup);
    let candidate = transfer(&lease, &mut setup);
    for validating in [false, true] {
        for storage in [false, true] {
            let mut work = Work::new(usize::MAX);
            let mut b = Budget::new(&mut work, usize::MAX);
            let floor = if storage {
                usize::MAX - Native::IO_STORAGE + 1
            } else {
                lease.retained_storage() + Native::FILE_STORAGE
            };
            b.reserve_storage(floor).unwrap();
            if !storage {
                b.charge_work(usize::MAX - ENTRY_WORK + 1).unwrap();
            }
            let before = refs(&candidate);
            let result = if validating {
                lease.validate_transfer(&candidate, &mut b)
            } else {
                lease.try_clone_for_transfer(&mut b).map(|_| ())
            };
            if storage {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(b.work(), Native::TRANSFER_WORK);
                assert_eq!(b.failed_storage(), Some(usize::MAX));
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(b.work(), usize::MAX - ENTRY_WORK + 1);
                assert_eq!(b.failed_work(), Some(usize::MAX));
            }
            assert_eq!(b.storage(), floor);
            assert_eq!(b.peak_storage(), floor);
            assert_eq!(refs(&candidate), before);
        }
    }
    retire_file(candidate, &mut setup);
    retire(lease, &mut setup);
}

#[test]
fn transfer_success_and_refusal_preserve_prior_denials_work_and_peak() {
    let f = Fixture::new();
    let other = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    b.reserve_storage(Native::FILE_STORAGE).unwrap();
    let wrong = other.file();
    b.reserve_storage(100_000).unwrap();
    b.release_storage(100_000).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
    let ledger = b.work_ledger_identity_v1();
    let floor = b.storage();
    let before = b.work();
    let file = transfer(&lease, &mut b);
    lease.validate_transfer(&file, &mut b).unwrap();
    assert!(matches!(
        lease.validate_transfer(&wrong, &mut b),
        Err(Error::Lease(LeaseError::FileChanged))
    ));
    retire_file(file, &mut b);
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), before + 3 * Native::TRANSFER_WORK);
    assert_eq!(
        (b.failed_work(), b.failed_storage(), b.peak_storage()),
        history
    );
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(refs(&wrong), 1);
    f.blocked();
    retire_file(wrong, &mut b);
    retire(lease, &mut b);
}

#[test]
fn independent_canonical_reopen_acquires_its_own_shared_custody() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    b.reserve_storage(Native::FILE_STORAGE).unwrap();
    let reopened = f.file();
    lease.validate_transfer(&reopened, &mut b).unwrap();
    retire(lease, &mut b);
    // Metadata equality alone would leave this independently opened File unlocked.
    f.blocked();
    retire_file(reopened, &mut b);
    f.available();
}

#[test]
fn last_duplicate_retains_shared_custody_after_owner_and_other_aliases_drop() {
    let f = Fixture::new();
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let lease = f.admit(&mut b);
    let first = transfer(&lease, &mut b);
    b.reserve_storage(Native::FILE_STORAGE).unwrap();
    let staged = File::from(rustix::io::fcntl_dupfd_cloexec(&first, 300).unwrap());
    lease.validate_transfer(&staged, &mut b).unwrap();
    retire_file(first, &mut b);
    retire(lease, &mut b);
    f.blocked();
    assert_eq!(refs(&staged), 1);
    retire_file(staged, &mut b);
    f.available();
    assert_eq!(b.storage(), Native::STATE_ROOT_STORAGE);
}

#[test]
fn final_staged_file_flags_and_same_policy_inode_substitution_are_rejected() {
    for mode in 0..3 {
        let f = Fixture::new();
        let other = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.admit(&mut b);
        let file = if mode == 0 {
            b.reserve_storage(Native::FILE_STORAGE).unwrap();
            other.file()
        } else {
            transfer(&lease, &mut b)
        };
        if mode == 1 {
            rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        }
        if mode == 2 {
            let flags = rustix::fs::fcntl_getfl(&file).unwrap();
            rustix::fs::fcntl_setfl(&file, flags | OFlags::APPEND).unwrap();
        }
        let before = refs(&file);
        let floor = b.storage();
        assert!(matches!(
            lease.validate_transfer(&file, &mut b),
            Err(Error::Lease(_))
        ));
        assert_eq!(b.storage(), floor);
        assert_eq!(refs(&file), before);
        f.blocked();
        retire_file(file, &mut b);
        retire(lease, &mut b);
        f.available();
    }
}

#[test]
fn replaced_path_and_parent_policy_refuse_without_closing_transfers() {
    for parent in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.admit(&mut b);
        let file = transfer(&lease, &mut b);
        if parent {
            fs::set_permissions(f.root.path(), fs::Permissions::from_mode(0o700)).unwrap();
        } else {
            fs::rename(&f.lock, f.root.path().join("displaced")).unwrap();
            fs::write(&f.lock, []).unwrap();
            fs::set_permissions(
                &f.lock,
                fs::Permissions::from_mode(COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1),
            )
            .unwrap();
        }
        let floor = b.storage();
        assert!(matches!(
            lease.try_clone_for_transfer(&mut b),
            Err(Error::Lease(_))
        ));
        assert!(matches!(
            lease.validate_transfer(&file, &mut b),
            Err(Error::Lease(_))
        ));
        assert_eq!(refs(&file), 2);
        assert_eq!(b.storage(), floor);
        let retained_inode = if parent {
            f.file()
        } else {
            File::open(f.root.path().join("displaced")).unwrap()
        };
        assert_eq!(
            flock(&retained_inode, FlockOperation::NonBlockingLockExclusive),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
        retire_file(file, &mut b);
        retire(lease, &mut b);
        flock(&retained_inode, FlockOperation::NonBlockingLockExclusive).unwrap();
    }
}

#[test]
fn clone_late_error_unwind_and_mutation_close_only_the_new_duplicate() {
    for mode in 0..3 {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.admit(&mut b);
        let witness = transfer(&lease, &mut b);
        let before = refs(&witness);
        let floor = b.storage();
        let before_work = b.work();
        let ledger = b.work_ledger_identity_v1();
        assert!(b.charge_work(usize::MAX).is_err());
        assert!(b.reserve_storage(usize::MAX).is_err());
        let history = (b.failed_work(), b.failed_storage());
        let result = catch_unwind(AssertUnwindSafe(|| {
            lease.clone_for_transfer_with(&mut b, |file| {
                assert_eq!(refs(file), before + 1);
                match mode {
                    0 => Err(LeaseError::FileChanged.into()),
                    1 => panic!("injected transfer unwind"),
                    _ => {
                        rustix::io::fcntl_setfd(file, rustix::io::FdFlags::empty()).unwrap();
                        Ok(())
                    }
                }
            })
        }));
        match mode {
            1 => assert!(result.is_err()),
            _ => assert!(matches!(result, Ok(Err(Error::Lease(_))))),
        }
        assert_eq!(refs(&witness), before);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor + Native::IO_STORAGE);
        assert_eq!(b.work(), before_work + Native::TRANSFER_WORK);
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        assert!(ledger == b.work_ledger_identity_v1());
        lease.validate_transfer(&witness, &mut b).unwrap();
        retire(lease, &mut b);
        f.blocked();
        retire_file(witness, &mut b);
        f.available();
    }
}

const HOLDER_READY: &str = "FE2O3_NATIVE_TRANSFER_READY";

#[test]
fn transfer_process_holder() {
    let Some(ready) = std::env::var_os(HOLDER_READY) else {
        return;
    };
    // Stdio transfers the controlled lease alias through exec as fd 2. No new
    // flock is acquired here: the parent test observes only inherited custody.
    fs::write(ready, b"ready").unwrap();
    let mut command = [0];
    std::io::stdin().read_exact(&mut command).unwrap();
    assert_eq!(command, [1]);
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn transferred_lock_survives_exec_and_parent_close_until_child_exit_or_kill() {
    for kill in [false, true] {
        let f = Fixture::new();
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, LIMIT);
        let lease = f.admit(&mut b);
        let file = transfer(&lease, &mut b);
        lease.validate_transfer(&file, &mut b).unwrap();
        let ready = f.root.path().join("ready");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "native::transfer_tests::transfer_process_holder",
                "--nocapture",
            ])
            .env(HOLDER_READY, &ready)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(file));
        let mut child = ChildGuard(command.spawn().unwrap());
        drop(command);
        b.release_storage(Native::FILE_STORAGE).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "holder exited before readiness"
            );
            assert!(Instant::now() < deadline, "holder readiness timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        retire(lease, &mut b);
        let witness = f.file();
        assert_eq!(refs(&witness), 1);
        f.blocked();
        if kill {
            child.0.kill().unwrap();
        } else {
            child.0.stdin.as_mut().unwrap().write_all(&[1]).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "holder exit timed out");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.success(), !kill);
        f.available();
        assert_eq!(b.storage(), Native::STATE_ROOT_STORAGE);
    }
}
