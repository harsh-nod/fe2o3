//! Exercises the actual journal/recovery consumer with sealed native key custody.
//! Public service transport is not covered here or by the admission-only isolated
//! fixture. These tests do not construct a synthetic Admission or observed occurrence.
use super::*;
use fe2o3_artifact_transaction::{
    RetainedDurableDirectoryHooksV1, RetainedDurableFaultTimingV1 as Timing,
    RetainedDurableRecordBoundaryV1 as Boundary,
    RetainedDurableRecoveryBoundaryV1 as RecoveryBoundary,
    RetainedDurableRecoveryMutationBoundaryV1 as MutationBoundary,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs::File,
    os::{
        fd::{AsFd, OwnedFd},
        unix::fs::{MetadataExt, PermissionsExt},
    },
};

fn custody(b: &mut Budget<'_>) -> (Policy, Key) {
    let measurement =
        fe2o3_runtime_protocol::CompilerExecutionIssuerMeasurementV1::new([3; 32], 4096).unwrap();
    let public = ed25519_dalek::SigningKey::from_bytes(&[7; 32])
        .verifying_key()
        .to_bytes();
    let anchor = ed25519_dalek::SigningKey::from_bytes(&[9; 32])
        .verifying_key()
        .to_bytes();
    let p = retain(
        Policy::new(1, measurement, measurement, public, anchor, b).unwrap(),
        b,
    )
    .unwrap();
    b.reserve_storage(32).unwrap();
    let mut seed = [7; 32];
    let (key, charge) = Key::create_and_zeroize(&mut seed, &p, b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(seed, [0; 32]);
    (p, key)
}
fn directory() -> (tempfile::TempDir, File) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let file = File::open(directory.path()).unwrap();
    (directory, file)
}
#[test]
fn native_consumer_genesis_recovery_holds_the_shared_singleton() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    let (p, key) = custody(&mut b);
    let (_directory, root) = directory();
    let floor = b.storage();
    let ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    b.reserve_storage(Ledger::STORAGE).unwrap();
    ledger.validate(&mut b).unwrap();
    let original = *ledger.record.bytes();
    let accepted = b.work();
    assert!(Ledger::recover(root.as_fd(), &p, &key, &mut b).is_err());
    assert!(crate::compiler_execution_journal_recovery::SingletonLock::acquire(&root).is_err());
    assert!(b.work() > accepted);
    drop(ledger);
    let recovered = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    assert_eq!(recovered.record.bytes(), &original);
    recovered.validate(&mut b).unwrap();
    let old = fe2o3_artifact_transaction::RetainedDurableDirectoryV1::admit_service_owned(
        rustix::io::fcntl_dupfd_cloexec(&root, 0).unwrap(),
    )
    .unwrap();
    assert!(crate::compiler_execution_journal_recovery::reject_legacy_state(&old).is_err());
}

struct Fault {
    boundary: Boundary,
    timing: Timing,
    fired: bool,
}
impl RetainedDurableDirectoryHooksV1 for Fault {
    fn record(&mut self, boundary: Boundary, timing: Timing) -> std::io::Result<()> {
        if (boundary, timing) == (self.boundary, self.timing) {
            self.fired = true;
            Err(std::io::Error::from_raw_os_error(libc::EIO))
        } else {
            Ok(())
        }
    }
}
#[test]
fn native_consumer_genesis_crashes_never_return_a_live_ledger() {
    for boundary in [
        Boundary::CreateTemp,
        Boundary::WriteTemp,
        Boundary::SyncTemp,
        Boundary::RenameTempToRedo,
        Boundary::SyncRedoName,
        Boundary::RenameRedoToCanonical,
        Boundary::SyncCanonicalName,
    ] {
        for timing in [Timing::Before, Timing::After] {
            let mut work = Work::new(usize::MAX);
            let mut b = Budget::new(&mut work, usize::MAX);
            let (p, key) = custody(&mut b);
            let (_directory, root) = directory();
            let floor = b.storage();
            let mut fault = Fault {
                boundary,
                timing,
                fired: false,
            };
            assert!(
                Ledger::recover_with_hooks(root.as_fd(), &p, &key, &mut fault, &mut b).is_err()
            );
            assert!(fault.fired, "{boundary:?}/{timing:?}");
            assert_eq!(b.storage(), floor);
            let recovered = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
            assert_eq!(recovered.record.sequence, 1);
            assert!(matches!(recovered.record.body, Body::Ready));
        }
    }
}
#[test]
fn native_consumer_recovery_crash_matrix_preserves_exact_signed_bytes() {
    use crate::compiler_execution_journal_recovery::test_support::RecoveryFault;
    for mut fault in RecoveryFault::cases(1) {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        let (p, key) = custody(&mut b);
        let (_directory, root) = directory();
        let ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        let bytes = *ledger.record.bytes();
        drop(ledger);
        assert!(Ledger::recover_with_hooks(root.as_fd(), &p, &key, &mut fault, &mut b).is_err());
        assert!(fault.fired, "{fault:?}");
        let ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        assert_eq!(ledger.record.bytes(), &bytes);
    }
}

#[derive(Default)]
struct RecoveryWrites(usize);
impl RetainedDurableDirectoryHooksV1 for RecoveryWrites {
    fn record(&mut self, _: Boundary, _: Timing) -> std::io::Result<()> {
        self.0 += 1;
        Ok(())
    }
    fn recovery(&mut self, _: RecoveryBoundary, _: Timing) -> std::io::Result<()> {
        self.0 += 1;
        Ok(())
    }
    fn recovery_mutation(&mut self, _: MutationBoundary, _: Timing) -> std::io::Result<()> {
        self.0 += 1;
        Ok(())
    }
    fn sync_directory(&mut self, directory: &OwnedFd) -> std::io::Result<()> {
        self.0 += 1;
        rustix::fs::fsync(directory).map_err(std::io::Error::from)
    }
}

#[test]
fn native_consumer_rejects_signed_unsupported_position_without_disk_changes() {
    for name in crate::compiler_execution_journal_recovery::NATIVE_ISSUER_STATE_FILES {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, usize::MAX);
        let (p, key) = custody(&mut b);
        b.reserve_storage(record::BYTES).unwrap();
        let bytes = record::signed_unsupported_ready_for_test(&p, &key, &mut b).unwrap();
        let (directory, root) = directory();
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let identity = |m: std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.mode(),
                m.nlink(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        let before = identity(std::fs::metadata(&path).unwrap());
        let floor = b.storage();
        let mut writes = RecoveryWrites::default();
        let error = Ledger::recover_with_hooks(root.as_fd(), &p, &key, &mut writes, &mut b)
            .err()
            .expect("unsupported signed position must be rejected");
        assert_eq!(
            error.to_string(),
            "native issuer service refused: native journal requires the unjoined Worker/anchor ledger",
            "{name}",
        );
        assert_eq!(writes.0, 0, "no promotion, stabilization, or sync: {name}");
        assert_eq!(b.storage(), floor);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(identity(std::fs::metadata(&path).unwrap()), before);
        assert!(crate::compiler_execution_journal_recovery::SingletonLock::acquire(&root).is_ok());
    }
}

#[test]
fn native_consumer_rejects_unjoined_worker_state_and_changed_canonical_bytes() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    let (p, key) = custody(&mut b);
    let (directory, root) = directory();
    let worker = directory.path().join("compiler-execution-worker-v2.state");
    std::fs::write(&worker, b"not absent").unwrap();
    assert!(Ledger::recover(root.as_fd(), &p, &key, &mut b).is_err());
    assert!(
        !directory
            .path()
            .join("compiler-execution-issuer-v3.state")
            .exists()
    );
    std::fs::remove_file(worker).unwrap();
    let ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let mut changed = *ledger.record.bytes();
    changed[64] ^= 1;
    std::fs::write(
        directory.path().join("compiler-execution-issuer-v3.state"),
        changed,
    )
    .unwrap();
    assert!(ledger.validate(&mut b).is_err());
    drop(ledger);
    assert!(Ledger::recover(root.as_fd(), &p, &key, &mut b).is_err());
}
#[test]
fn native_consumer_quota_refuses_before_touching_the_root() {
    let mut source = Work::new(usize::MAX);
    let mut original = Budget::new(&mut source, usize::MAX);
    let (p, key) = custody(&mut original);
    let (directory, root) = directory();
    let mut denied = Work::new(7);
    let mut b = Budget::new(&mut denied, usize::MAX);
    b.reserve_storage(p.retained_storage() + key.retained_storage())
        .unwrap();
    let result = Ledger::recover(root.as_fd(), &p, &key, &mut b);
    assert!(matches!(result, Err(ref e) if matches!(e.resource(), Some(Resource::Work(_)))));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
#[test]
fn native_transport_attempts_share_one_cumulative_limit_and_nonce_is_fresh() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    let mut attempts = 0;
    for _ in 0..IO_ATTEMPTS {
        permit_io(&mut b, &mut attempts).unwrap();
    }
    assert!(permit_io(&mut b, &mut attempts).is_err());
    assert_eq!(b.work(), 4096 * (IO_ATTEMPTS + 1));
    assert_ne!(fresh_nonce(&mut b).unwrap(), fresh_nonce(&mut b).unwrap());
}
