use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    DurableArtifactBoundaryV1, DurableFaultTimingV1, DurableJournalBoundaryV1,
    DurableJournalStageV1, DurableLinkPublicationError, DurableLinkPublicationFaultPointV1,
    DurableLinkPublicationOptionsV1, DurableLinkPublicationOutcomeV1, DurableLinkPublicationPlanV1,
    FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationPhaseV1,
    LinkPublicationScopeV1, LinkPublicationStateV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PinnedWorkerIdentityV1, TargetIdentityV1, ValidatedResponseIdentityV1, publish_durable_link_v1,
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
        fs::create_dir(path.join("output")).unwrap();
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
        "e62fc430e9032d8b6156b7311c3707f4092839f7218395d5f1421119ed968a39"
    );

    let record_path = canonical_record(&first_output);
    let changed = first_record.len() / 2;
    let mut corrupted = first_record;
    corrupted[changed] ^= 1;
    fs::write(&record_path, &corrupted).unwrap();
    assert!(recover_durable_link_publication_v1(&first_output, plan.scope()).is_err());
    assert_eq!(fs::read(record_path).unwrap(), corrupted);
    assert!(artifact_path(&first_output, bytes).exists());
}

#[test]
fn crash_retry_requires_the_complete_plan_and_preserves_the_recovered_prefix() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"complete plan retry payload";
    let original = plan(1, 0x13, 0x43, bytes);
    let interrupted = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::ResponseValidated,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };
    assert!(matches!(
        publish_durable_link_v1_with_options(
            &output,
            original,
            DurableLinkPublicationOptionsV1::inject_crash(interrupted),
            |transaction| complete(transaction, bytes),
        ),
        Err(DurableLinkPublicationError::InjectedCrash { point }) if point == interrupted
    ));
    assert!(
        recover_durable_link_publication_v1(&output, original.scope())
            .unwrap()
            .is_none()
    );

    let changed_plans = [
        DurableLinkPublicationPlanV1::new(
            original.attempt(),
            original.scope(),
            original.request(),
            PinnedWorkerIdentityV1::from_bytes(identity(0xe0)),
            original.response(),
            original.linked_output(),
            original.finalization(),
            original.finalized_output(),
            original.publication(),
        ),
        DurableLinkPublicationPlanV1::new(
            original.attempt(),
            original.scope(),
            original.request(),
            original.worker(),
            ValidatedResponseIdentityV1::from_bytes(identity(0xe1)),
            original.linked_output(),
            original.finalization(),
            original.finalized_output(),
            original.publication(),
        ),
        DurableLinkPublicationPlanV1::new(
            original.attempt(),
            original.scope(),
            original.request(),
            original.worker(),
            original.response(),
            LinkedOutputIdentityV1::from_bytes(identity(0xe2)),
            original.finalization(),
            original.finalized_output(),
            original.publication(),
        ),
        DurableLinkPublicationPlanV1::new(
            original.attempt(),
            original.scope(),
            original.request(),
            original.worker(),
            original.response(),
            original.linked_output(),
            FinalizationIdentityV1::from_bytes(identity(0xe3)),
            original.finalized_output(),
            original.publication(),
        ),
        DurableLinkPublicationPlanV1::new(
            original.attempt(),
            original.scope(),
            original.request(),
            original.worker(),
            original.response(),
            original.linked_output(),
            original.finalization(),
            FinalizedOutputIdentityV1::from_bytes(identity(0xe4)),
            original.publication(),
        ),
        DurableLinkPublicationPlanV1::new(
            original.attempt(),
            original.scope(),
            original.request(),
            original.worker(),
            original.response(),
            original.linked_output(),
            original.finalization(),
            original.finalized_output(),
            AtomicPublicationIdentityV1::from_bytes(identity(0xe5)),
        ),
    ];
    for changed in changed_plans {
        assert!(matches!(
            publish_durable_link_v1(&output, changed, |_| {
                panic!("a changed complete plan must not reach work")
            }),
            Err(DurableLinkPublicationError::Protocol(_))
        ));
    }

    let worker_boundary = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::WorkerPinned,
        boundary: DurableJournalBoundaryV1::CreateRedoTemp,
        timing: DurableFaultTimingV1::Before,
    };
    let retried = publish_durable_link_v1_with_options(
        &output,
        original,
        DurableLinkPublicationOptionsV1::inject_crash(worker_boundary),
        |transaction| complete(transaction, bytes),
    )
    .expect("the recovered response prefix skips the worker journal stage");
    assert_eq!(retried.snapshot().artifact().bytes(), bytes);
}

#[test]
fn resumed_finalized_callback_without_bytes_is_durably_terminal() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"resumed finalized bytes";
    let plan = plan(1, 0x14, 0x44, bytes);
    let finalized = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Finalized,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };
    assert!(matches!(
        publish_durable_link_v1_with_options(
            &output,
            plan,
            DurableLinkPublicationOptionsV1::inject_crash(finalized),
            |transaction| complete(transaction, bytes),
        ),
        Err(DurableLinkPublicationError::InjectedCrash { point }) if point == finalized
    ));
    assert!(
        recover_durable_link_publication_v1(&output, plan.scope())
            .unwrap()
            .is_none()
    );

    assert!(matches!(
        publish_durable_link_v1(&output, plan, |_| Ok(())),
        Err(DurableLinkPublicationError::InvalidDurableRecord { .. })
    ));
    assert!(!artifact_path(&output, bytes).exists());
    assert!(matches!(
        publish_durable_link_v1(&output, plan, |_| {
            panic!("missing resumed bytes must terminally reject future retry")
        }),
        Err(DurableLinkPublicationError::Protocol(_))
    ));
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
        DurableJournalBoundaryV1::CreateRedoTemp,
        DurableJournalBoundaryV1::WriteRedoTemp,
        DurableJournalBoundaryV1::SyncRedoTemp,
        DurableJournalBoundaryV1::RenameTempToRedo,
        DurableJournalBoundaryV1::SyncRedoName,
        DurableJournalBoundaryV1::RenameRedoToCanonical,
        DurableJournalBoundaryV1::SyncCanonicalName,
    ];
    let timings = [DurableFaultTimingV1::Before, DurableFaultTimingV1::After];

    for stage in stages {
        for boundary in boundaries {
            for timing in timings {
                let temp = TestDirectory::new();
                let output = temp.path.join("output");
                let bytes = b"journal fault artifact";
                let plan = plan(1, 0x20, 0x60, bytes);
                let point = DurableLinkPublicationFaultPointV1::Journal {
                    stage,
                    boundary,
                    timing,
                };
                let error = publish_durable_link_v1_with_options(
                    &output,
                    plan,
                    DurableLinkPublicationOptionsV1::inject_crash(point),
                    |transaction| complete(transaction, bytes),
                )
                .unwrap_err();
                assert!(
                    matches!(error, DurableLinkPublicationError::InjectedCrash { point: actual } if actual == point),
                    "unexpected {stage:?}/{boundary:?}/{timing:?} error: {error}"
                );

                let _ = recover_durable_link_publication_v1(&output, plan.scope()).unwrap();
                let retried = publish(&output, plan, bytes).unwrap_or_else(|error| {
                    panic!("retry failed after {stage:?}/{boundary:?}/{timing:?}: {error}")
                });
                assert_eq!(retried.snapshot().artifact().bytes(), bytes);
                assert!(managed_entries(&output, ".fe2o3-stage-").is_empty());
            }
        }
    }
}

#[test]
fn every_artifact_boundary_before_and_after_leaves_no_visible_failed_publication() {
    let boundaries = [
        DurableArtifactBoundaryV1::CreateTemp,
        DurableArtifactBoundaryV1::WriteTemp,
        DurableArtifactBoundaryV1::SyncTemp,
        DurableArtifactBoundaryV1::RenameToContentAddress,
        DurableArtifactBoundaryV1::SyncDirectory,
    ];
    let timings = [DurableFaultTimingV1::Before, DurableFaultTimingV1::After];

    for boundary in boundaries {
        for timing in timings {
            let temp = TestDirectory::new();
            let output = temp.path.join("output");
            let bytes = b"artifact boundary payload";
            let plan = plan(1, 0x30, 0x70, bytes);
            let point = DurableLinkPublicationFaultPointV1::Artifact { boundary, timing };
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
            let renamed = boundary == DurableArtifactBoundaryV1::SyncDirectory
                || (boundary == DurableArtifactBoundaryV1::RenameToContentAddress
                    && timing == DurableFaultTimingV1::After);
            assert_eq!(
                artifact_path(&output, bytes).exists(),
                renamed,
                "{point:?} retained an unexpected artifact name"
            );
            assert_eq!(
                publish(&output, plan, bytes).unwrap().outcome(),
                DurableLinkPublicationOutcomeV1::Published
            );
        }
    }
}

#[test]
fn normal_failure_is_invalidated_and_preserves_prior_publication() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let old_bytes = b"old complete artifact";
    let old_plan = plan(1, 0x41, 0x81, old_bytes);
    let old_result = publish(&output, old_plan, old_bytes).unwrap();
    let old_record = old_result.snapshot().record().clone();
    let old_lease = old_result.into_current_lease();

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
    assert_eq!(current.record(), &old_record);
    assert_eq!(current.artifact().bytes(), old_bytes);
    assert_eq!(old_lease.exact_artifact_bytes(), old_bytes);
    assert!(matches!(
        old_lease.acquire_current_token(),
        Err(DurableLinkPublicationError::CurrentPublication { .. })
    ));
    assert!(matches!(
        publish(&output, old_plan, old_bytes),
        Err(DurableLinkPublicationError::CurrentPublication { .. })
    ));
    assert!(matches!(
        publish_durable_link_v1(&output, new_plan, |_| {
            panic!("an explicitly failed attempt must never run again")
        }),
        Err(DurableLinkPublicationError::Protocol(_))
    ));
    let replacement_plan = plan(3, 0x41, 0xa1, new_bytes);
    assert_eq!(
        publish(&output, replacement_plan, new_bytes)
            .unwrap()
            .outcome(),
        DurableLinkPublicationOutcomeV1::Published
    );
}

#[test]
fn invalidation_is_terminal_only_after_its_complete_record_is_exposed() {
    let boundaries = [
        DurableJournalBoundaryV1::CreateRedoTemp,
        DurableJournalBoundaryV1::WriteRedoTemp,
        DurableJournalBoundaryV1::SyncRedoTemp,
        DurableJournalBoundaryV1::RenameTempToRedo,
        DurableJournalBoundaryV1::SyncRedoName,
        DurableJournalBoundaryV1::RenameRedoToCanonical,
        DurableJournalBoundaryV1::SyncCanonicalName,
    ];
    let timings = [DurableFaultTimingV1::Before, DurableFaultTimingV1::After];

    for boundary in boundaries {
        for timing in timings {
            let temp = TestDirectory::new();
            let output = temp.path.join("output");
            let bytes = b"terminal invalidation matrix";
            let plan = plan(1, 0x42, 0x82, bytes);
            let point = DurableLinkPublicationFaultPointV1::Journal {
                stage: DurableJournalStageV1::Invalidated,
                boundary,
                timing,
            };
            let result = publish_durable_link_v1_with_options(
                &output,
                plan,
                DurableLinkPublicationOptionsV1::inject_crash(point),
                |transaction| {
                    transaction.record_worker_pinned()?;
                    Err(DurableLinkPublicationError::work("observed worker failure"))
                },
            );
            assert!(
                matches!(result, Err(DurableLinkPublicationError::InjectedCrash { point: actual }) if actual == point),
                "ordinary failure escaped before durable invalidation at {boundary:?}/{timing:?}"
            );

            let _ = recover_durable_link_publication_v1(&output, plan.scope()).unwrap();
            let terminal = matches!(
                (boundary, timing),
                (
                    DurableJournalBoundaryV1::RenameTempToRedo,
                    DurableFaultTimingV1::After
                ) | (DurableJournalBoundaryV1::SyncRedoName, _)
                    | (DurableJournalBoundaryV1::RenameRedoToCanonical, _)
                    | (DurableJournalBoundaryV1::SyncCanonicalName, _)
            );
            if terminal {
                assert!(matches!(
                    publish_durable_link_v1(&output, plan, |_| {
                        panic!("durably invalidated work must never run again")
                    }),
                    Err(DurableLinkPublicationError::Protocol(_))
                ));
            } else {
                assert_eq!(
                    publish(&output, plan, bytes).unwrap().outcome(),
                    DurableLinkPublicationOutcomeV1::Published,
                    "unexposed invalidation must recover as a crash at {boundary:?}/{timing:?}"
                );
            }
        }
    }
}

#[test]
fn callback_cannot_forge_a_crash_to_bypass_terminal_invalidation() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"forged crash callback";
    let plan = plan(1, 0x43, 0x83, bytes);
    let forged = DurableLinkPublicationFaultPointV1::SnapshotRead;

    assert!(matches!(
        publish_durable_link_v1(&output, plan, |_| {
            Err(DurableLinkPublicationError::InjectedCrash { point: forged })
        }),
        Err(DurableLinkPublicationError::InjectedCrash { point }) if point == forged
    ));
    assert!(matches!(
        publish_durable_link_v1(&output, plan, |_| {
            panic!("forged crash failure must be durably terminal")
        }),
        Err(DurableLinkPublicationError::Protocol(_))
    ));
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
fn higher_generation_redo_requires_one_exact_next_planned_transition() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let first_bytes = b"higher generation prior publication";
    let next_bytes = b"higher generation next publication";
    let first = plan(1, 0x54, 0xa4, first_bytes);
    let next = plan(2, 0x54, 0xb4, next_bytes);
    publish(&output, first, first_bytes).unwrap();
    let point = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::RenameTempToRedo,
        timing: DurableFaultTimingV1::After,
    };
    assert!(matches!(
        publish_durable_link_v1_with_options(
            &output,
            next,
            DurableLinkPublicationOptionsV1::inject_crash(point),
            |_| panic!("planned redo is exposed before work"),
        ),
        Err(DurableLinkPublicationError::InjectedCrash { point: actual }) if actual == point
    ));

    let recovered = recover_durable_link_publication_v1(&output, next.scope())
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record().attempt(), first.attempt());
    assert_eq!(recovered.artifact().bytes(), first_bytes);
    assert_eq!(
        publish(&output, next, next_bytes).unwrap().outcome(),
        DurableLinkPublicationOutcomeV1::Published
    );
    let skipped = plan(4, 0x54, 0xd4, b"skipped scope generation");
    assert!(matches!(
        publish_durable_link_v1(&output, skipped, |_| {
            panic!("a noncontiguous scope generation must fail before work")
        }),
        Err(DurableLinkPublicationError::InvalidDurableRecord { .. })
    ));

    for impossible_generation in [2, 3] {
        let canonical_temp = TestDirectory::new();
        let forged_temp = TestDirectory::new();
        let canonical_output = canonical_temp.path.join("output");
        let forged_output = forged_temp.path.join("output");
        let canonical_plan = plan(1, 0x55, 0xa5, first_bytes);
        publish(&canonical_output, canonical_plan, first_bytes).unwrap();
        let canonical = canonical_record(&canonical_output);
        let canonical_bytes = fs::read(&canonical).unwrap();

        let impossible = plan(impossible_generation, 0x55, 0xc5, next_bytes);
        let complete_point = DurableLinkPublicationFaultPointV1::Journal {
            stage: DurableJournalStageV1::Planned,
            boundary: DurableJournalBoundaryV1::SyncCanonicalName,
            timing: DurableFaultTimingV1::After,
        };
        assert!(matches!(
            publish_durable_link_v1_with_options(
                &forged_output,
                impossible,
                DurableLinkPublicationOptionsV1::inject_crash(complete_point),
                |_| panic!("planned record commits before work"),
            ),
            Err(DurableLinkPublicationError::InjectedCrash { point: actual }) if actual == complete_point
        ));
        let redo = PathBuf::from(format!("{}.redo", canonical.display()));
        fs::write(&redo, fs::read(canonical_record(&forged_output)).unwrap()).unwrap();
        fs::set_permissions(&redo, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(matches!(
            recover_durable_link_publication_v1(&canonical_output, canonical_plan.scope()),
            Err(DurableLinkPublicationError::ConflictingRedo { .. })
        ));
        assert_eq!(fs::read(canonical).unwrap(), canonical_bytes);
        assert!(redo.exists());
    }
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
    let current_record = fs::read(&record).unwrap();

    let redo = PathBuf::from(format!("{}.redo", record.display()));
    fs::write(&redo, stale_record).unwrap();
    fs::set_permissions(&redo, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        recover_durable_link_publication_v1(&output, second.scope()),
        Err(DurableLinkPublicationError::ConflictingRedo { .. })
    ));
    assert_eq!(fs::read(&record).unwrap(), current_record);
    assert!(redo.exists());
}

#[test]
fn conflicting_same_generation_redo_never_replaces_canonical_active_state() {
    let canonical_temp = TestDirectory::new();
    let conflicting_temp = TestDirectory::new();
    let canonical_output = canonical_temp.path.join("output");
    let conflicting_output = conflicting_temp.path.join("output");
    let bytes = b"same generation conflict";
    let canonical_plan = plan(1, 0x53, 0xb3, bytes);
    let conflicting_plan = plan(1, 0x53, 0xc3, bytes);
    let point = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };

    for (output, plan) in [
        (&canonical_output, canonical_plan),
        (&conflicting_output, conflicting_plan),
    ] {
        assert!(matches!(
            publish_durable_link_v1_with_options(
                output,
                plan,
                DurableLinkPublicationOptionsV1::inject_crash(point),
                |_| panic!("planned-record fault occurs before work"),
            ),
            Err(DurableLinkPublicationError::InjectedCrash { point: actual }) if actual == point
        ));
    }

    let canonical = canonical_record(&canonical_output);
    let canonical_bytes = fs::read(&canonical).unwrap();
    let conflicting_bytes = fs::read(canonical_record(&conflicting_output)).unwrap();
    let redo = PathBuf::from(format!("{}.redo", canonical.display()));
    fs::write(&redo, conflicting_bytes).unwrap();
    fs::set_permissions(&redo, fs::Permissions::from_mode(0o600)).unwrap();

    assert!(matches!(
        recover_durable_link_publication_v1(&canonical_output, canonical_plan.scope()),
        Err(DurableLinkPublicationError::ConflictingRedo { .. })
    ));
    assert_eq!(fs::read(&canonical).unwrap(), canonical_bytes);
    assert!(redo.exists());
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
fn transient_snapshot_failure_does_not_mutate_or_poison_publication() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"transient snapshot payload";
    let plan = plan(1, 0x61, 0xc1, bytes);
    publish(&output, plan, bytes).unwrap();
    let record = canonical_record(&output);
    let canonical = fs::read(&record).unwrap();

    assert!(matches!(
        publish_durable_link_v1_with_options(
            &output,
            plan,
            DurableLinkPublicationOptionsV1::inject_fault(
                DurableLinkPublicationFaultPointV1::SnapshotRead,
            ),
            |_| panic!("snapshot failure occurs before work"),
        ),
        Err(DurableLinkPublicationError::Filesystem(_))
    ));
    assert_eq!(fs::read(&record).unwrap(), canonical);
    assert!(managed_entries(&output, ".fe2o3-link-quarantine-v1-").is_empty());

    let replay = publish_durable_link_v1(&output, plan, |_| {
        panic!("a transient read failure must not revoke exact replay")
    })
    .unwrap();
    assert_eq!(
        replay.outcome(),
        DurableLinkPublicationOutcomeV1::AlreadyPublished
    );
    assert_eq!(replay.snapshot().artifact().bytes(), bytes);
}

#[test]
fn symlink_and_hardlink_artifact_substitution_fail_closed() {
    for hardlink in [false, true] {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
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
fn record_symlink_and_hardlink_substitution_fail_closed_without_deleting_target() {
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

        assert!(recover_durable_link_publication_v1(&output, plan.scope()).is_err());
        assert_eq!(fs::read(&unrelated).unwrap(), b"do not delete");
        assert!(record.symlink_metadata().is_ok());
        assert!(artifact_path(&output, bytes).exists());
    }
}

#[test]
fn malformed_truncated_and_oversized_records_fail_closed_without_path_mutation() {
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

        assert!(recover_durable_link_publication_v1(&output, plan.scope()).is_err());
        assert_eq!(fs::read(unrelated).unwrap(), b"keep");
        assert_eq!(fs::read(&record).unwrap(), corrupt);
        assert!(artifact_path(&output, bytes).exists());
        assert!(managed_entries(&output, ".fe2o3-link-quarantine-v1-").is_empty());
        assert!(publish(&output, plan, bytes).is_err());
    }
}

#[test]
fn conservative_v1_never_deletes_unreferenced_managed_entries() {
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
    assert!(orphan.exists());
    assert!(hardlink.exists());
    assert!(mismatched.exists());
    assert_eq!(fs::read(hardlink_target).unwrap(), b"keep hardlink");
    assert!(artifact_path(&output, bytes).exists());
}

#[test]
fn missing_output_directory_is_rejected_without_creating_topology() {
    let temp = TestDirectory::new();
    let output = temp.path.join("missing-output");
    let bytes = b"missing topology payload";
    let plan = plan(1, 0x75, 0xd5, bytes);

    assert!(publish(&output, plan, bytes).is_err());
    assert!(!output.exists());
    assert!(recover_durable_link_publication_v1(&output, plan.scope()).is_err());
    assert!(!output.exists());
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

#[test]
fn current_lease_retains_exact_descriptor_snapshot_and_revalidates_under_lock() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let bytes = b"current exact descriptor payload";
    let plan = plan(1, 0x76, 0xd6, bytes);

    let result = publish(&output, plan, bytes).unwrap();
    let lease = result.into_current_lease();
    assert_eq!(lease.published().attempt(), plan.attempt());
    assert_eq!(lease.published().scope(), plan.scope());
    assert_eq!(lease.exact_artifact_bytes(), bytes);
    assert!(!lease.grants_load_authority());
    assert!(!lease.grants_launch_authority());

    let token = lease.acquire_current_token().unwrap();
    assert_eq!(token.exact_artifact_bytes(), bytes);
    assert!(!token.grants_load_authority());
    assert!(!token.grants_launch_authority());
}

#[test]
fn newer_planned_or_failed_generation_invalidates_old_lease_currentness() {
    for fail_normally in [false, true] {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let old_bytes = b"immutable old lease payload";
        let old_plan = plan(1, 0x77, 0xd7, old_bytes);
        let old_lease = publish(&output, old_plan, old_bytes)
            .unwrap()
            .into_current_lease();
        let next_bytes = b"superseding attempt payload";
        let next = plan(2, 0x77, 0xe7, next_bytes);

        if fail_normally {
            assert!(matches!(
                publish_durable_link_v1(&output, next, |_| {
                    Err(DurableLinkPublicationError::work("expected failure"))
                }),
                Err(DurableLinkPublicationError::Work { .. })
            ));
        } else {
            let point = DurableLinkPublicationFaultPointV1::Journal {
                stage: DurableJournalStageV1::Planned,
                boundary: DurableJournalBoundaryV1::SyncCanonicalName,
                timing: DurableFaultTimingV1::After,
            };
            assert!(matches!(
                publish_durable_link_v1_with_options(
                    &output,
                    next,
                    DurableLinkPublicationOptionsV1::inject_crash(point),
                    |_| panic!("planned crash fires before callback"),
                ),
                Err(DurableLinkPublicationError::InjectedCrash { point: actual }) if actual == point
            ));
        }

        assert_eq!(old_lease.exact_artifact_bytes(), old_bytes);
        assert!(matches!(
            old_lease.acquire_current_token(),
            Err(DurableLinkPublicationError::CurrentPublication { .. })
        ));
    }
}

#[test]
fn current_token_serializes_future_cooperating_publication() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let old_bytes = b"locked old payload";
    let old_plan = plan(1, 0x78, 0xd8, old_bytes);
    let lease = publish(&output, old_plan, old_bytes)
        .unwrap()
        .into_current_lease();
    let token = lease.acquire_current_token().unwrap();
    let next_bytes = b"blocked next payload";
    let next = plan(2, 0x78, 0xe8, next_bytes);
    let (entered_tx, entered_rx) = mpsc::channel();

    thread::scope(|scope| {
        let output = output.clone();
        let handle = scope.spawn(move || {
            publish_durable_link_v1(&output, next, |transaction| {
                entered_tx.send(()).unwrap();
                complete(transaction, next_bytes)
            })
        });
        assert!(matches!(
            entered_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        drop(token);
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        handle.join().unwrap().unwrap();
    });
    assert!(lease.acquire_current_token().is_err());
}

#[test]
fn record_and_artifact_path_replacement_with_identical_bytes_fail_closed() {
    for replace_record in [false, true] {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let bytes = b"identical replacement payload";
        let plan = plan(1, 0x79, 0xd9, bytes);
        let lease = publish(&output, plan, bytes).unwrap().into_current_lease();
        let path = if replace_record {
            canonical_record(&output)
        } else {
            artifact_path(&output, bytes)
        };
        let original = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(lease.acquire_current_token().is_err());
        assert_eq!(lease.exact_artifact_bytes(), bytes);
    }
}

#[test]
fn lease_revalidation_rejects_symlink_hardlink_and_in_place_artifact_changes() {
    for attack in ["symlink", "hardlink", "mutate", "truncate", "grow"] {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let bytes = b"artifact adversary payload";
        let plan = plan(1, 0x7a, 0xda, bytes);
        let lease = publish(&output, plan, bytes).unwrap().into_current_lease();
        let artifact = artifact_path(&output, bytes);
        match attack {
            "symlink" => {
                let target = temp.path.join("symlink-target");
                fs::write(&target, bytes).unwrap();
                fs::remove_file(&artifact).unwrap();
                symlink(target, &artifact).unwrap();
            }
            "hardlink" => {
                let target = temp.path.join("hardlink-target");
                fs::write(&target, bytes).unwrap();
                fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
                fs::remove_file(&artifact).unwrap();
                fs::hard_link(target, &artifact).unwrap();
            }
            "mutate" => fs::write(&artifact, vec![b'x'; bytes.len()]).unwrap(),
            "truncate" => fs::write(&artifact, b"short").unwrap(),
            "grow" => fs::write(&artifact, [bytes.as_slice(), b"extra"].concat()).unwrap(),
            _ => unreachable!(),
        }
        assert!(lease.acquire_current_token().is_err(), "attack={attack}");
    }
}

#[test]
fn lease_revalidation_rejects_output_parent_substitution() {
    let temp = TestDirectory::new();
    let output = temp.path.join("output");
    let moved = temp.path.join("moved-output");
    let bytes = b"parent substitution lease payload";
    let plan = plan(1, 0x7b, 0xdb, bytes);
    let lease = publish(&output, plan, bytes).unwrap().into_current_lease();

    fs::rename(&output, &moved).unwrap();
    fs::create_dir(&output).unwrap();
    assert!(lease.acquire_current_token().is_err());
    assert_eq!(lease.exact_artifact_bytes(), bytes);
}
