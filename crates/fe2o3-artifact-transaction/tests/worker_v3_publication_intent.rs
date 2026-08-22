use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    PackageIdentityV1, PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    WorkerV2PublicationIntentOutcomeV1, WorkerV3FinalizerReplayAttachmentsV1,
    WorkerV3PublicationIntentBoundaryV1, WorkerV3PublicationIntentCodecErrorV1,
    WorkerV3PublicationIntentErrorV1, WorkerV3PublicationIntentFaultPointV1,
    WorkerV3PublicationIntentFaultTimingV1, WorkerV3PublicationIntentInvalidReasonV1,
    WorkerV3PublicationIntentOptionsV1, WorkerV3PublicationIntentOutcomeV1, begin_build_attempt,
    persist_worker_v2_publication_intent_v1, persist_worker_v3_publication_intent_v1,
    persist_worker_v3_publication_intent_v1_with_options,
    publish_exact_hsaco_evidence_for_attempt_v1, recover_worker_v2_publication_intent_v1,
    recover_worker_v3_publication_intent_v1,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v3-publication-intent-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn output(&self) -> PathBuf {
        self.0.join("output")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn producer(seed: u8) -> ProducerIdentity {
    ProducerIdentity::from_codegen(
        &format!("worker_v3_{seed}"),
        Some(Path::new(&format!("/src/worker-v3-{seed}.rs"))),
    )
    .unwrap()
}

fn begin(output: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        output,
        producer,
        BuildInvocation::from_bytes([seed; 32]),
        BuildSession::from_bytes([seed.wrapping_add(1); 16]),
    )
    .unwrap()
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn plan(attempt: BuildAttempt, output: &[u8], seed: u8) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        LinkPublicationScopeV1::new(
            PackageIdentityV1::from_bytes([seed; 32]),
            KernelSetIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
            TargetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
        ),
        CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
        PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
        ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
        LinkedOutputIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
        FinalizationIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
        FinalizedOutputIdentityV1::from_bytes(digest(output)),
        AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
    )
}

fn intent_entry(output: &Path, suffix: &str) -> PathBuf {
    let mut entries = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(".fe2o3-worker-v3-publication-intent-v1-") && name.ends_with(suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "expected one V3 intent {suffix} entry");
    entries.pop().unwrap()
}

fn replay(transcript: &[u8]) -> WorkerV3FinalizerReplayAttachmentsV1 {
    WorkerV3FinalizerReplayAttachmentsV1::new(
        b"exact outer V3 semantic handoff".to_vec(),
        vec![
            b"external provider alpha".to_vec(),
            b"external provider beta".to_vec(),
        ],
        transcript.to_vec(),
    )
    .unwrap()
}

#[test]
fn deduplicated_replay_inputs_and_output_round_trip_inertly_after_restart() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(1);
    let attempt = begin(&output_dir, &producer, 2);
    let output = b"exact finalized Worker V3 output".to_vec();
    let transcript = b"bounded canonical finalizer identity transcript";
    let plan = plan(attempt, &output, 3);
    let replay = replay(transcript);
    let output_ptr = output.as_ptr();
    let outer_ptr = replay.outer_handoff().as_ptr();

    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &producer,
        attempt,
        plan,
        replay,
        output,
    )
    .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    assert_eq!(persisted.record().attempt(), attempt);
    assert_eq!(persisted.record().plan(), plan);
    assert_eq!(
        persisted.record().output_length(),
        persisted.exact_output().len()
    );
    assert_eq!(persisted.record().transcript_length(), transcript.len());
    assert_eq!(
        persisted.exact_output(),
        b"exact finalized Worker V3 output"
    );
    assert_eq!(persisted.exact_output().as_ptr(), output_ptr);
    assert_eq!(persisted.outer_handoff().as_ptr(), outer_ptr);
    assert_eq!(
        persisted.outer_handoff(),
        b"exact outer V3 semantic handoff"
    );
    assert_eq!(persisted.external_providers().len(), 2);
    assert_eq!(persisted.finalizer_replay_transcript(), transcript);
    assert!(!persisted.authenticates_finalizer_transcript());
    assert!(!persisted.grants_publication_authority());
    assert!(!persisted.grants_load_authority());
    assert!(!persisted.grants_launch_authority());
    drop(persisted);

    let recovered =
        recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt).unwrap();
    assert_eq!(
        recovered.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Recovered
    );
    assert_eq!(
        recovered.exact_output(),
        b"exact finalized Worker V3 output"
    );
    assert_eq!(
        recovered.outer_handoff(),
        b"exact outer V3 semantic handoff"
    );
    assert_eq!(
        recovered.external_providers().get(0),
        Some(b"external provider alpha".as_slice())
    );
    assert_eq!(
        recovered.external_providers().get(1),
        Some(b"external provider beta".as_slice())
    );
    assert_eq!(recovered.finalizer_replay_transcript(), transcript);
    assert!(intent_entry(&output_dir, ".output").is_file());
    assert!(intent_entry(&output_dir, ".handoff").is_file());
    assert!(intent_entry(&output_dir, ".providers").is_file());
    assert!(intent_entry(&output_dir, ".transcript").is_file());
    assert!(intent_entry(&output_dir, ".record").is_file());
    assert!(fs::read_dir(&output_dir).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".raw")
    }));
}

#[test]
fn exact_v2_and_v3_intents_coexist_without_wire_or_namespace_cross_use() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(10);
    let attempt = begin(&output_dir, &producer, 11);
    let output = b"same exact finalized output";
    let plan = plan(attempt, output, 12);
    let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x44; 32]);

    assert_eq!(
        persist_worker_v2_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            plan,
            upstream,
            output,
        )
        .unwrap()
        .outcome(),
        WorkerV2PublicationIntentOutcomeV1::Persisted
    );
    persist_worker_v3_publication_intent_v1(
        &output_dir,
        &producer,
        attempt,
        plan,
        replay(b"V3-only transcript"),
        output.to_vec(),
    )
    .unwrap();

    assert_eq!(
        recover_worker_v2_publication_intent_v1(&output_dir, &producer, attempt)
            .unwrap()
            .exact_output(),
        output
    );
    assert_eq!(
        recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt)
            .unwrap()
            .finalizer_replay_transcript(),
        b"V3-only transcript"
    );
}

#[test]
fn every_durable_boundary_is_idempotently_recoverable() {
    let boundaries = [
        WorkerV3PublicationIntentBoundaryV1::CreateOuterHandoffTemp,
        WorkerV3PublicationIntentBoundaryV1::WriteOuterHandoffTemp,
        WorkerV3PublicationIntentBoundaryV1::SyncOuterHandoffTemp,
        WorkerV3PublicationIntentBoundaryV1::RenameOuterHandoff,
        WorkerV3PublicationIntentBoundaryV1::SyncOuterHandoffName,
        WorkerV3PublicationIntentBoundaryV1::CreateExternalProvidersTemp,
        WorkerV3PublicationIntentBoundaryV1::WriteExternalProvidersTemp,
        WorkerV3PublicationIntentBoundaryV1::SyncExternalProvidersTemp,
        WorkerV3PublicationIntentBoundaryV1::RenameExternalProviders,
        WorkerV3PublicationIntentBoundaryV1::SyncExternalProvidersName,
        WorkerV3PublicationIntentBoundaryV1::CreateTranscriptTemp,
        WorkerV3PublicationIntentBoundaryV1::WriteTranscriptTemp,
        WorkerV3PublicationIntentBoundaryV1::SyncTranscriptTemp,
        WorkerV3PublicationIntentBoundaryV1::RenameTranscript,
        WorkerV3PublicationIntentBoundaryV1::SyncTranscriptName,
        WorkerV3PublicationIntentBoundaryV1::CreateOutputTemp,
        WorkerV3PublicationIntentBoundaryV1::WriteOutputTemp,
        WorkerV3PublicationIntentBoundaryV1::SyncOutputTemp,
        WorkerV3PublicationIntentBoundaryV1::RenameOutput,
        WorkerV3PublicationIntentBoundaryV1::SyncOutputName,
        WorkerV3PublicationIntentBoundaryV1::CreateRecordTemp,
        WorkerV3PublicationIntentBoundaryV1::WriteRecordTemp,
        WorkerV3PublicationIntentBoundaryV1::SyncRecordTemp,
        WorkerV3PublicationIntentBoundaryV1::RenameRecordToRedo,
        WorkerV3PublicationIntentBoundaryV1::SyncRedoName,
        WorkerV3PublicationIntentBoundaryV1::RenameRedoToCanonical,
        WorkerV3PublicationIntentBoundaryV1::SyncCanonicalName,
    ];
    let timings = [
        WorkerV3PublicationIntentFaultTimingV1::Before,
        WorkerV3PublicationIntentFaultTimingV1::After,
    ];
    for (index, (boundary, timing)) in boundaries
        .into_iter()
        .flat_map(|boundary| timings.into_iter().map(move |timing| (boundary, timing)))
        .enumerate()
    {
        let temp = TestDirectory::new();
        let output_dir = temp.output();
        let producer = producer(20_u8.wrapping_add(index as u8));
        let attempt = begin(&output_dir, &producer, 60_u8.wrapping_add(index as u8));
        let output = format!("output {boundary:?} {timing:?}").into_bytes();
        let transcript = format!("transcript {boundary:?} {timing:?}").into_bytes();
        let plan = plan(attempt, &output, 100_u8.wrapping_add(index as u8));
        let point = WorkerV3PublicationIntentFaultPointV1 { boundary, timing };

        assert!(matches!(
            persist_worker_v3_publication_intent_v1_with_options(
                &output_dir,
                &producer,
                attempt,
                plan,
                replay(&transcript),
                output.clone(),
                WorkerV3PublicationIntentOptionsV1::inject_crash(point),
            ),
            Err(WorkerV3PublicationIntentErrorV1::InjectedCrash { point: actual })
                if actual == point
        ));
        let reconciled = persist_worker_v3_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            plan,
            replay(&transcript),
            output.clone(),
        )
        .unwrap();
        assert_eq!(reconciled.exact_output(), output);
        assert_eq!(reconciled.finalizer_replay_transcript(), transcript);
        let restarted =
            recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt).unwrap();
        assert_eq!(restarted.record(), reconciled.record());
        assert_eq!(restarted.exact_output(), output);
        assert_eq!(restarted.finalizer_replay_transcript(), transcript);
    }
}

#[test]
fn recovery_rejects_independent_attachment_mutation() {
    for (seed, suffix) in [
        (100, ".handoff"),
        (110, ".providers"),
        (130, ".transcript"),
        (140, ".output"),
    ] {
        let temp = TestDirectory::new();
        let output_dir = temp.output();
        let producer = producer(seed);
        let attempt = begin(&output_dir, &producer, seed.wrapping_add(1));
        let output = b"output";
        let transcript = b"transcript";
        persist_worker_v3_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            plan(attempt, output, seed.wrapping_add(2)),
            replay(transcript),
            output.to_vec(),
        )
        .unwrap();
        let path = intent_entry(&output_dir, suffix);
        let mut bytes = fs::read(&path).unwrap();
        bytes[0] ^= 1;
        fs::write(path, bytes).unwrap();
        let error =
            recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt).unwrap_err();
        match suffix {
            ".handoff" => assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::OuterHandoffDigestMismatch
            )),
            ".providers" => assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::InvalidIntent {
                    reason: WorkerV3PublicationIntentInvalidReasonV1::ProviderArchiveCodec(
                        WorkerV3PublicationIntentCodecErrorV1::ProviderArchiveMagicMismatch
                    ),
                    ..
                }
            )),
            ".transcript" => assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::TranscriptDigestMismatch
            )),
            ".output" => assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::OutputDigestMismatch
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn recovery_reports_provider_payload_and_archive_checksum_mutation_separately() {
    for (seed, mutation) in [(145, "payload"), (146, "checksum")] {
        let temp = TestDirectory::new();
        let output_dir = temp.output();
        let producer = producer(seed);
        let attempt = begin(&output_dir, &producer, seed.wrapping_add(1));
        let output = b"output";
        persist_worker_v3_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            plan(attempt, output, seed.wrapping_add(2)),
            replay(b"transcript"),
            output.to_vec(),
        )
        .unwrap();
        let providers = intent_entry(&output_dir, ".providers");
        let mut bytes = fs::read(&providers).unwrap();
        if mutation == "payload" {
            let offset = bytes
                .windows(b"external provider alpha".len())
                .position(|window| window == b"external provider alpha")
                .unwrap();
            bytes[offset] ^= 1;
        } else {
            let last = bytes.last_mut().unwrap();
            *last ^= 1;
        }
        fs::write(providers, bytes).unwrap();

        let error =
            recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt).unwrap_err();
        match mutation {
            "payload" => assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::InvalidIntent {
                    reason: WorkerV3PublicationIntentInvalidReasonV1::ProviderArchiveCodec(
                        WorkerV3PublicationIntentCodecErrorV1::ProviderPayloadDigestMismatch {
                            index: 0
                        }
                    ),
                    ..
                }
            )),
            "checksum" => assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::InvalidIntent {
                    reason: WorkerV3PublicationIntentInvalidReasonV1::ProviderArchiveCodec(
                        WorkerV3PublicationIntentCodecErrorV1::ProviderArchiveChecksumMismatch
                    ),
                    ..
                }
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn recovery_rejects_attachment_truncation_and_trailing_bytes() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let first_producer = producer(150);
    let attempt = begin(&output_dir, &first_producer, 151);
    let output = b"output";
    persist_worker_v3_publication_intent_v1(
        &output_dir,
        &first_producer,
        attempt,
        plan(attempt, output, 152),
        replay(b"transcript"),
        output.to_vec(),
    )
    .unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(intent_entry(&output_dir, ".transcript"))
        .unwrap()
        .write_all(b"trailing")
        .unwrap();
    assert!(matches!(
        recover_worker_v3_publication_intent_v1(&output_dir, &first_producer, attempt),
        Err(WorkerV3PublicationIntentErrorV1::InvalidIntent {
            reason: WorkerV3PublicationIntentInvalidReasonV1::FileLengthMismatch { .. },
            ..
        })
    ));

    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(160);
    let attempt = begin(&output_dir, &producer, 161);
    persist_worker_v3_publication_intent_v1(
        &output_dir,
        &producer,
        attempt,
        plan(attempt, output, 162),
        replay(b"transcript"),
        output.to_vec(),
    )
    .unwrap();
    let providers = intent_entry(&output_dir, ".providers");
    let mut bytes = fs::read(&providers).unwrap();
    bytes.pop();
    fs::write(providers, bytes).unwrap();
    assert!(matches!(
        recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt),
        Err(WorkerV3PublicationIntentErrorV1::InvalidIntent {
            reason: WorkerV3PublicationIntentInvalidReasonV1::FileLengthMismatch { .. },
            ..
        })
    ));
}

#[test]
fn recovery_rejects_record_mutation_truncation_and_every_attachment_symlink() {
    for (seed, mutation) in [(170_u8, "checksum"), (180, "truncate")] {
        let temp = TestDirectory::new();
        let output_dir = temp.output();
        let producer = producer(seed);
        let attempt = begin(&output_dir, &producer, seed.wrapping_add(1));
        let output = b"output";
        persist_worker_v3_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            plan(attempt, output, seed.wrapping_add(2)),
            replay(b"transcript"),
            output.to_vec(),
        )
        .unwrap();
        let record = intent_entry(&output_dir, ".record");
        let mut bytes = fs::read(&record).unwrap();
        if mutation == "checksum" {
            bytes[64] ^= 1;
        } else {
            bytes.pop();
        }
        fs::write(record, bytes).unwrap();
        let error =
            recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt).unwrap_err();
        if mutation == "checksum" {
            assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::InvalidIntent {
                    reason: WorkerV3PublicationIntentInvalidReasonV1::RecordCodec(
                        WorkerV3PublicationIntentCodecErrorV1::ChecksumMismatch
                    ),
                    ..
                }
            ));
        } else {
            assert!(matches!(
                error,
                WorkerV3PublicationIntentErrorV1::InvalidIntent {
                    reason: WorkerV3PublicationIntentInvalidReasonV1::FileLengthMismatch { .. },
                    ..
                }
            ));
        }
    }

    for (seed, suffix) in [
        (190, ".handoff"),
        (194, ".providers"),
        (198, ".transcript"),
        (202, ".output"),
        (206, ".record"),
    ] {
        let temp = TestDirectory::new();
        let output_dir = temp.output();
        let producer = producer(seed);
        let attempt = begin(&output_dir, &producer, seed.wrapping_add(1));
        let output = b"output";
        persist_worker_v3_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            plan(attempt, output, seed.wrapping_add(2)),
            replay(b"transcript"),
            output.to_vec(),
        )
        .unwrap();
        let entry = intent_entry(&output_dir, suffix);
        fs::remove_file(&entry).unwrap();
        symlink("nonexistent", entry).unwrap();
        assert!(matches!(
            recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt),
            Err(WorkerV3PublicationIntentErrorV1::InvalidIntent {
                reason: WorkerV3PublicationIntentInvalidReasonV1::EntryNotPrivate,
                ..
            })
        ));
    }
}

#[test]
fn conflicts_stale_occurrences_and_plan_attempt_cross_use_fail_closed() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let first_producer = producer(160);
    let attempt = begin(&output_dir, &first_producer, 161);
    let output = b"output";
    let plan = plan(attempt, output, 162);
    persist_worker_v3_publication_intent_v1(
        &output_dir,
        &first_producer,
        attempt,
        plan,
        replay(b"first transcript"),
        output.to_vec(),
    )
    .unwrap();
    assert!(matches!(
        persist_worker_v3_publication_intent_v1(
            &output_dir,
            &first_producer,
            attempt,
            plan,
            replay(b"other transcript"),
            output.to_vec(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::ConflictingIntent)
    ));

    let other_producer = producer(170);
    let other_attempt = begin(&output_dir, &other_producer, 171);
    assert!(matches!(
        persist_worker_v3_publication_intent_v1(
            &output_dir,
            &other_producer,
            other_attempt,
            plan,
            replay(b"transcript"),
            output.to_vec(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::PlanAttemptMismatch)
    ));

    let _new_attempt = begin(&output_dir, &first_producer, 180);
    assert!(matches!(
        recover_worker_v3_publication_intent_v1(&output_dir, &first_producer, attempt),
        Err(WorkerV3PublicationIntentErrorV1::Attempt { .. })
    ));
}

#[test]
fn recovery_remains_inert_after_the_exact_backend_claim_is_durable() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(190);
    let attempt = begin(&output_dir, &producer, 191);
    let output = b"claimed output";
    let plan = plan(attempt, output, 192);
    let transcript = b"claimed transcript";
    persist_worker_v3_publication_intent_v1(
        &output_dir,
        &producer,
        attempt,
        plan,
        replay(transcript),
        output.to_vec(),
    )
    .unwrap();
    publish_exact_hsaco_evidence_for_attempt_v1(
        &output_dir,
        &producer,
        attempt,
        plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x77; 32]),
        output,
    )
    .unwrap();

    let recovered =
        recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt).unwrap();
    assert_eq!(recovered.exact_output(), output);
    assert_eq!(recovered.finalizer_replay_transcript(), transcript);
    assert!(!recovered.grants_publication_authority());
}

#[test]
fn concurrent_exact_callers_commit_one_record_and_recover_identical_bytes() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(200);
    let attempt = begin(&output_dir, &producer, 201);
    let output = b"concurrent output";
    let transcript = b"concurrent transcript";
    let plan = plan(attempt, output, 202);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let output_dir = output_dir.clone();
        let producer = producer.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            persist_worker_v3_publication_intent_v1(
                &output_dir,
                &producer,
                attempt,
                plan,
                replay(transcript),
                output.to_vec(),
            )
            .unwrap()
        }));
    }
    barrier.wait();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| result.outcome() == WorkerV3PublicationIntentOutcomeV1::Persisted)
            .count(),
        1
    );
    assert!(results.windows(2).all(|pair| {
        pair[0].record() == pair[1].record()
            && pair[0].exact_output() == pair[1].exact_output()
            && pair[0].finalizer_replay_transcript() == pair[1].finalizer_replay_transcript()
    }));
}
