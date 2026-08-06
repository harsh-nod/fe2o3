use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    DurableJournalBoundaryV1, DurableJournalStageV1, DurableLinkPublicationError,
    DurableLinkPublicationFaultPointV1, DurableLinkPublicationOptionsV1,
    DurableLinkPublicationOutcomeV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationPhaseV1, LinkPublicationScopeV1,
    LinkPublicationStateV1, LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1,
    TargetIdentityV1, ValidatedResponseIdentityV1, publish_durable_link_v1,
    publish_durable_link_v1_with_options, recover_durable_link_publication_v1,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const RECORD_PREFIX: &str = ".fe2o3-link-publication-v1-";
const RECORD_SUFFIX: &str = ".record";
const ARTIFACT_PREFIX: &str = ".fe2o3-link-artifact-v1-";
const ARTIFACT_SUFFIX: &str = ".bin";

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-durable-link-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn identity(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn attempt(generation: u64, seed: u8) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        hex(&[seed; 16]),
        hex(&[seed.wrapping_add(1); 32])
    ))
    .unwrap()
}

fn scope(seed: u8) -> LinkPublicationScopeV1 {
    LinkPublicationScopeV1::new(
        PackageIdentityV1::from_bytes(identity(seed)),
        KernelSetIdentityV1::from_bytes(identity(seed.wrapping_add(1))),
        TargetIdentityV1::from_bytes(identity(seed.wrapping_add(2))),
    )
}

fn plan(
    generation: u64,
    scope_seed: u8,
    request_seed: u8,
    bytes: &[u8],
) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt(generation, scope_seed),
        scope(scope_seed),
        CanonicalLinkRequestIdentityV1::from_bytes(identity(request_seed)),
        PinnedWorkerIdentityV1::from_bytes(identity(request_seed.wrapping_add(1))),
        ValidatedResponseIdentityV1::from_bytes(identity(request_seed.wrapping_add(2))),
        LinkedOutputIdentityV1::from_bytes(identity(request_seed.wrapping_add(3))),
        FinalizationIdentityV1::from_bytes(identity(request_seed.wrapping_add(4))),
        FinalizedOutputIdentityV1::from_bytes(Sha256::digest(bytes).into()),
        AtomicPublicationIdentityV1::from_bytes(identity(request_seed.wrapping_add(5))),
    )
}

fn complete(
    transaction: &mut fe2o3_artifact_transaction::DurableLinkPublicationTransactionV1<'_>,
    bytes: &[u8],
) -> Result<(), DurableLinkPublicationError> {
    transaction.record_worker_pinned()?;
    transaction.record_response_validated()?;
    transaction.record_finalized(bytes)
}

fn publish(
    output: &Path,
    plan: DurableLinkPublicationPlanV1,
    bytes: &[u8],
) -> Result<fe2o3_artifact_transaction::DurableLinkPublicationResultV1, DurableLinkPublicationError>
{
    publish_durable_link_v1(output, plan, |transaction| complete(transaction, bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    result
}

fn artifact_path(output: &Path, bytes: &[u8]) -> PathBuf {
    output.join(format!(
        "{ARTIFACT_PREFIX}{}{ARTIFACT_SUFFIX}",
        hex(&Sha256::digest(bytes))
    ))
}

fn managed_entries(output: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut entries = fs::read_dir(output)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(prefix)
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn canonical_record(output: &Path) -> PathBuf {
    managed_entries(output, RECORD_PREFIX)
        .into_iter()
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(RECORD_SUFFIX)
        })
        .expect("canonical durable record")
}

#[test]
fn publishes_recovers_and_exact_replay_skips_work() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"valid gfx942 code object";
    let plan = plan(1, 0x10, 0x40, bytes);

    let first = publish(&output, plan, bytes).unwrap();
    assert_eq!(first.outcome(), DurableLinkPublicationOutcomeV1::Published);
    assert_eq!(first.snapshot().artifact().bytes(), bytes);
    assert_eq!(
        first.snapshot().record().state(),
        LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published)
    );
    assert!(!first.snapshot().grants_load_authority());
    assert!(!first.snapshot().grants_launch_authority());

    let recovered = recover_durable_link_publication_v1(&output, plan.scope())
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record(), first.snapshot().record());
    assert_eq!(recovered.artifact().bytes(), bytes);

    let replay = publish_durable_link_v1(&output, plan, |_| {
        panic!("exact replay must not rerun direct-link work")
    })
    .unwrap();
    assert_eq!(
        replay.outcome(),
        DurableLinkPublicationOutcomeV1::AlreadyPublished
    );
    assert_eq!(replay.snapshot().artifact().bytes(), bytes);
    assert_eq!(
        fs::metadata(artifact_path(&output, bytes))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(canonical_record(&output))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn durable_envelope_is_deterministic_and_checksum_bound() {
    let first = TestDirectory::new();
    let second = TestDirectory::new();
    let first_output = first.path.join("output");
    let second_output = second.path.join("output");
    let bytes = b"canonical durable envelope payload";
    let plan = plan(7, 0x12, 0x42, bytes);
    publish(&first_output, plan, bytes).unwrap();
    publish(&second_output, plan, bytes).unwrap();
    let first_record = fs::read(canonical_record(&first_output)).unwrap();
    let second_record = fs::read(canonical_record(&second_output)).unwrap();
    assert_eq!(first_record, second_record);
    assert_eq!(
        hex(&Sha256::digest(&first_record)),
        "d88176e0e2781eb01426353e61846ee30e86cefe9da37f69ca3db8809a44d525"
    );

    let record_path = canonical_record(&first_output);
    let changed = first_record.len() / 2;
    let mut corrupted = first_record;
    corrupted[changed] ^= 1;
    fs::write(record_path, corrupted).unwrap();
    assert!(
        recover_durable_link_publication_v1(&first_output, plan.scope())
            .unwrap()
            .is_none()
    );
    assert!(artifact_path(&first_output, bytes).exists());
}

#[test]
fn exclusive_artifact_lock_is_held_through_callback_and_commit() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let first_bytes = b"first serialized payload";
    let second_bytes = b"second serialized payload";
    let first_plan = plan(1, 0x11, 0x41, first_bytes);
    let second_plan = plan(2, 0x11, 0x51, second_bytes);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();

    let first_output = output.clone();
    let first_entered = entered_tx.clone();
    let first = thread::spawn(move || {
        publish_durable_link_v1(&first_output, first_plan, |transaction| {
            first_entered.send(1).unwrap();
            release_rx.recv().unwrap();
            complete(transaction, first_bytes)
        })
    });
    assert_eq!(entered_rx.recv().unwrap(), 1);

    let second_output = output.clone();
    let second = thread::spawn(move || {
        publish_durable_link_v1(&second_output, second_plan, |transaction| {
            entered_tx.send(2).unwrap();
            complete(transaction, second_bytes)
        })
    });
    assert!(matches!(
        entered_rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    release_tx.send(()).unwrap();
    assert_eq!(entered_rx.recv_timeout(Duration::from_secs(5)).unwrap(), 2);
    assert_eq!(
        first.join().unwrap().unwrap().outcome(),
        DurableLinkPublicationOutcomeV1::Published
    );
    assert_eq!(
        second.join().unwrap().unwrap().outcome(),
        DurableLinkPublicationOutcomeV1::Published
    );
    assert_eq!(
        recover_durable_link_publication_v1(&output, second_plan.scope())
            .unwrap()
            .unwrap()
            .artifact()
            .bytes(),
        second_bytes
    );
}

#[test]
fn every_journal_boundary_recovers_or_replays_idempotently() {
    let stages = [
        DurableJournalStageV1::Planned,
        DurableJournalStageV1::WorkerPinned,
        DurableJournalStageV1::ResponseValidated,
        DurableJournalStageV1::Finalized,
        DurableJournalStageV1::Published,
    ];
    let boundaries = [
        DurableJournalBoundaryV1::CreateRedo,
        DurableJournalBoundaryV1::WriteRedo,
        DurableJournalBoundaryV1::SyncRedo,
        DurableJournalBoundaryV1::SyncRedoName,
        DurableJournalBoundaryV1::RenameRedo,
        DurableJournalBoundaryV1::SyncCanonicalName,
    ];

    for stage in stages {
        for boundary in boundaries {
            let temp = TestDirectory::new();
            let output = temp.path.join("output");
            let bytes = b"journal fault artifact";
            let plan = plan(1, 0x20, 0x60, bytes);
            let point = DurableLinkPublicationFaultPointV1::Journal { stage, boundary };
            let error = publish_durable_link_v1_with_options(
                &output,
                plan,
                DurableLinkPublicationOptionsV1::inject_crash(point),
                |transaction| complete(transaction, bytes),
            )
            .unwrap_err();
            assert!(
                matches!(error, DurableLinkPublicationError::InjectedCrash { point: actual } if actual == point),
                "unexpected {stage:?}/{boundary:?} error: {error}"
            );

            let _ = recover_durable_link_publication_v1(&output, plan.scope()).unwrap();
            let retried = publish(&output, plan, bytes).unwrap_or_else(|error| {
                panic!("retry failed after {stage:?}/{boundary:?}: {error}")
            });
            assert_eq!(retried.snapshot().artifact().bytes(), bytes);
            assert!(managed_entries(&output, ".fe2o3-stage-").is_empty());
        }
    }
}

#[test]
fn every_artifact_boundary_leaves_no_visible_failed_publication() {
    let points = [
        DurableLinkPublicationFaultPointV1::ArtifactCreate,
        DurableLinkPublicationFaultPointV1::ArtifactWrite,
        DurableLinkPublicationFaultPointV1::ArtifactSync,
        DurableLinkPublicationFaultPointV1::ArtifactRename,
        DurableLinkPublicationFaultPointV1::ArtifactDirectorySync,
    ];

    for point in points {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let bytes = b"artifact boundary payload";
        let plan = plan(1, 0x30, 0x70, bytes);
        let error = publish_durable_link_v1_with_options(
            &output,
            plan,
            DurableLinkPublicationOptionsV1::inject_crash(point),
            |transaction| complete(transaction, bytes),
        )
        .unwrap_err();
        assert!(
            matches!(error, DurableLinkPublicationError::InjectedCrash { point: actual } if actual == point)
        );

        assert!(
            recover_durable_link_publication_v1(&output, plan.scope())
                .unwrap()
                .is_none(),
            "{point:?} exposed an incomplete publication"
        );
        assert!(
            !artifact_path(&output, bytes).exists(),
            "{point:?} left a referenced artifact"
        );
        assert_eq!(
            publish(&output, plan, bytes).unwrap().outcome(),
            DurableLinkPublicationOutcomeV1::Published
        );
    }
}

#[test]
fn normal_failure_is_invalidated_and_preserves_prior_publication() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let old_bytes = b"old complete artifact";
    let old_plan = plan(1, 0x41, 0x81, old_bytes);
    publish(&output, old_plan, old_bytes).unwrap();

    let new_bytes = b"new incomplete artifact";
    let new_plan = plan(2, 0x41, 0x91, new_bytes);
    let error = publish_durable_link_v1(&output, new_plan, |transaction| {
        transaction.record_worker_pinned()?;
        Err(DurableLinkPublicationError::work("worker rejected input"))
    })
    .unwrap_err();
    assert!(matches!(error, DurableLinkPublicationError::Work { .. }));
    assert!(!artifact_path(&output, new_bytes).exists());

    let current = recover_durable_link_publication_v1(&output, old_plan.scope())
        .unwrap()
        .unwrap();
    assert_eq!(
        current.record(),
        publish(&output, old_plan, old_bytes)
            .unwrap()
            .snapshot()
            .record()
    );
    assert_eq!(current.artifact().bytes(), old_bytes);
    assert_eq!(
        publish(&output, new_plan, new_bytes).unwrap().outcome(),
        DurableLinkPublicationOutcomeV1::Published
    );
}

#[test]
fn stale_attempt_request_and_scope_are_isolated() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"shared finalized bytes";
    let current = plan(2, 0x50, 0xa0, bytes);
    publish(&output, current, bytes).unwrap();

    let stale = plan(1, 0x50, 0xa0, bytes);
    assert!(matches!(
        publish(&output, stale, bytes),
        Err(DurableLinkPublicationError::Protocol(_))
    ));
    let changed_request = plan(2, 0x50, 0xa1, bytes);
    assert!(matches!(
        publish(&output, changed_request, bytes),
        Err(DurableLinkPublicationError::Protocol(_))
    ));

    let other_scope = plan(1, 0x51, 0xb0, bytes);
    assert_eq!(
        publish(&output, other_scope, bytes).unwrap().outcome(),
        DurableLinkPublicationOutcomeV1::Published
    );
    assert_eq!(managed_entries(&output, ARTIFACT_PREFIX).len(), 1);
    assert_eq!(managed_entries(&output, RECORD_PREFIX).len(), 2);
    assert_eq!(
        recover_durable_link_publication_v1(&output, current.scope())
            .unwrap()
            .unwrap()
            .artifact()
            .bytes(),
        bytes
    );
}

#[test]
fn stale_valid_redo_cannot_replace_a_newer_canonical_publication() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let first_bytes = b"first redo generation";
    let second_bytes = b"second redo generation";
    let first = plan(1, 0x52, 0xa2, first_bytes);
    let second = plan(2, 0x52, 0xb2, second_bytes);
    publish(&output, first, first_bytes).unwrap();
    let record = canonical_record(&output);
    let stale_record = fs::read(&record).unwrap();
    publish(&output, second, second_bytes).unwrap();

    let redo = PathBuf::from(format!("{}.redo", record.display()));
    fs::write(&redo, stale_record).unwrap();
    fs::set_permissions(&redo, fs::Permissions::from_mode(0o600)).unwrap();
    let recovered = recover_durable_link_publication_v1(&output, second.scope())
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record().attempt(), second.attempt());
    assert_eq!(recovered.artifact().bytes(), second_bytes);
    assert!(!redo.exists());
    assert_eq!(
        managed_entries(&output, ".fe2o3-link-quarantine-v1-").len(),
        1
    );
}

#[test]
fn finalized_bytes_must_match_validated_sha256_identity() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let expected = b"expected finalized bytes";
    let plan = plan(1, 0x60, 0xc0, expected);
    let error = publish_durable_link_v1(&output, plan, |transaction| {
        transaction.record_worker_pinned()?;
        transaction.record_response_validated()?;
        transaction.record_finalized(b"substituted bytes")
    })
    .unwrap_err();
    assert!(matches!(
        error,
        DurableLinkPublicationError::FinalizedArtifactDigestMismatch
    ));
    assert!(
        recover_durable_link_publication_v1(&output, plan.scope())
            .unwrap()
            .is_none()
    );
}

#[test]
fn symlink_and_hardlink_artifact_substitution_fail_closed() {
    for hardlink in [false, true] {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        fs::create_dir(&output).unwrap();
        let bytes = b"substitution target bytes";
        let plan = plan(1, 0x70, 0xd0, bytes);
        let unrelated = temp.path.join("unrelated");
        fs::write(&unrelated, b"unrelated data").unwrap();
        fs::set_permissions(&unrelated, fs::Permissions::from_mode(0o600)).unwrap();
        let artifact = artifact_path(&output, bytes);
        if hardlink {
            fs::hard_link(&unrelated, &artifact).unwrap();
        } else {
            symlink(&unrelated, &artifact).unwrap();
        }

        assert!(publish(&output, plan, bytes).is_err());
        assert_eq!(fs::read(&unrelated).unwrap(), b"unrelated data");
        assert!(artifact.symlink_metadata().is_ok());
        assert!(
            recover_durable_link_publication_v1(&output, plan.scope())
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn record_symlink_and_hardlink_substitution_invalidate_without_deleting_target() {
    for hardlink in [false, true] {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let bytes = b"record substitution artifact";
        let plan = plan(1, 0x71, 0xd1, bytes);
        publish(&output, plan, bytes).unwrap();
        let record = canonical_record(&output);
        let unrelated = temp.path.join("record-target");
        fs::write(&unrelated, b"do not delete").unwrap();
        fs::set_permissions(&unrelated, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_file(&record).unwrap();
        if hardlink {
            fs::hard_link(&unrelated, &record).unwrap();
        } else {
            symlink(&unrelated, &record).unwrap();
        }

        assert!(
            recover_durable_link_publication_v1(&output, plan.scope())
                .unwrap()
                .is_none()
        );
        assert_eq!(fs::read(&unrelated).unwrap(), b"do not delete");
        assert!(artifact_path(&output, bytes).exists());
    }
}

#[test]
fn malformed_truncated_and_oversized_records_are_quarantined() {
    for corrupt in [vec![0x13], vec![0x55; 2_048]] {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let bytes = b"record corruption artifact";
        let plan = plan(1, 0x72, 0xd2, bytes);
        publish(&output, plan, bytes).unwrap();
        let record = canonical_record(&output);
        fs::write(&record, &corrupt).unwrap();
        let unrelated = output.join("unrelated.txt");
        fs::write(&unrelated, b"keep").unwrap();

        assert!(
            recover_durable_link_publication_v1(&output, plan.scope())
                .unwrap()
                .is_none()
        );
        assert_eq!(fs::read(unrelated).unwrap(), b"keep");
        assert!(artifact_path(&output, bytes).exists());
        assert_eq!(
            managed_entries(&output, ".fe2o3-link-quarantine-v1-").len(),
            1
        );
        assert!(publish(&output, plan, bytes).is_err());
    }
}

#[test]
fn orphan_cleanup_removes_only_private_canonical_artifacts() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"live artifact";
    let plan = plan(1, 0x73, 0xd3, bytes);
    publish(&output, plan, bytes).unwrap();

    let orphan_bytes = b"orphan";
    let orphan = artifact_path(&output, orphan_bytes);
    fs::write(&orphan, orphan_bytes).unwrap();
    fs::set_permissions(&orphan, fs::Permissions::from_mode(0o600)).unwrap();
    let hardlink_target = temp.path.join("hardlink-target");
    fs::write(&hardlink_target, b"keep hardlink").unwrap();
    fs::set_permissions(&hardlink_target, fs::Permissions::from_mode(0o600)).unwrap();
    let hardlink = output.join(format!(
        "{ARTIFACT_PREFIX}{}{ARTIFACT_SUFFIX}",
        "bb".repeat(32)
    ));
    fs::hard_link(&hardlink_target, &hardlink).unwrap();
    let mismatched = output.join(format!(
        "{ARTIFACT_PREFIX}{}{ARTIFACT_SUFFIX}",
        "cc".repeat(32)
    ));
    fs::write(&mismatched, b"not the named digest").unwrap();
    fs::set_permissions(&mismatched, fs::Permissions::from_mode(0o600)).unwrap();

    recover_durable_link_publication_v1(&output, plan.scope()).unwrap();
    assert!(!orphan.exists());
    assert!(hardlink.exists());
    assert!(mismatched.exists());
    assert_eq!(fs::read(hardlink_target).unwrap(), b"keep hardlink");
    assert!(artifact_path(&output, bytes).exists());
}

#[test]
fn output_directory_substitution_never_redirects_publication() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let moved = temp.path.join("moved-output");
    let bytes = b"directory substitution payload";
    let plan = plan(1, 0x74, 0xd4, bytes);

    let result = publish_durable_link_v1(&output, plan, |transaction| {
        transaction.record_worker_pinned()?;
        fs::rename(&output, &moved).unwrap();
        fs::create_dir(&output).unwrap();
        transaction.record_response_validated()
    });
    assert!(result.is_err());
    assert!(!artifact_path(&output, bytes).exists());
    assert!(!artifact_path(&moved, bytes).exists());

    fs::remove_dir(&output).unwrap();
    fs::rename(&moved, &output).unwrap();
    assert!(
        recover_durable_link_publication_v1(&output, plan.scope())
            .unwrap()
            .is_none()
    );
}
