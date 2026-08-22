use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1, PackageIdentityV1, PinnedWorkerIdentityV1,
    ProducerIdentity, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
    ValidatedResponseIdentityV1, WorkerV2PublicationIntentOutcomeV1,
    WorkerV3FinalizerReplayAttachmentsV1, WorkerV3PublicationIntentBoundaryV1,
    WorkerV3PublicationIntentCodecErrorV1, WorkerV3PublicationIntentErrorV1,
    WorkerV3PublicationIntentFaultPointV1, WorkerV3PublicationIntentFaultTimingV1,
    WorkerV3PublicationIntentInvalidReasonV1, WorkerV3PublicationIntentOptionsV1,
    WorkerV3PublicationIntentOutcomeV1, WorkerV3PublicationIntentScavengeOutcomeV1,
    begin_build_attempt, clear_worker_v3_publication_intent_v1,
    clear_worker_v3_publication_intent_v1_with_options, persist_worker_v2_publication_intent_v1,
    persist_worker_v3_publication_intent_v1, persist_worker_v3_publication_intent_v1_with_options,
    producer_package_identity_v1, publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v2, recover_worker_v2_publication_intent_v1,
    recover_worker_v3_publication_intent_v1, scavenge_worker_v3_publication_intent_occurrence_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, symlink};
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

fn receipted_plan(
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    output: &[u8],
    seed: u8,
) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        LinkPublicationScopeV1::new(
            producer_package_identity_v1(owner),
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

fn compiler_closure(seed: u8) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
    )
    .unwrap()
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

fn replay_parts(
    outer: &[u8],
    providers: &[&[u8]],
    transcript: &[u8],
) -> WorkerV3FinalizerReplayAttachmentsV1 {
    WorkerV3FinalizerReplayAttachmentsV1::new(
        outer.to_vec(),
        providers.iter().map(|payload| payload.to_vec()).collect(),
        transcript.to_vec(),
    )
    .unwrap()
}

fn v3_namespace_entries(output: &Path) -> Vec<PathBuf> {
    fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".fe2o3-worker-v3-publication-intent-v1-")
        })
        .collect()
}

#[test]
fn compact_replay_inputs_and_output_round_trip_inertly_after_restart() {
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
fn current_retirement_requires_the_exact_identity_and_durable_v1_receipt() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let owner = producer(4);
    let attempt = begin(&output_dir, &owner, 5);
    let output = b"receipt-bound Worker V3 output";
    let publication_plan = receipted_plan(&owner, attempt, output, 6);
    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        attempt,
        publication_plan,
        replay(b"receipt-bound transcript"),
        output.to_vec(),
    )
    .unwrap();

    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &output_dir,
            &owner,
            attempt,
            persisted.record().identity(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable)
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), 5);

    let other_temp = TestDirectory::new();
    let other_output_dir = other_temp.output();
    let other_owner = producer(7);
    let other_attempt = begin(&other_output_dir, &other_owner, 8);
    let other_output = b"other identity output";
    let other = persist_worker_v3_publication_intent_v1(
        &other_output_dir,
        &other_owner,
        other_attempt,
        plan(other_attempt, other_output, 9),
        replay(b"other identity transcript"),
        other_output.to_vec(),
    )
    .unwrap();

    publish_exact_hsaco_evidence_for_attempt_v1(
        &output_dir,
        &owner,
        attempt,
        publication_plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes([10; 32]),
        output,
    )
    .unwrap();
    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &output_dir,
            &owner,
            attempt,
            other.record().identity(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::IdentityMismatch)
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), 5);

    clear_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        attempt,
        persisted.record().identity(),
    )
    .unwrap();
    assert!(v3_namespace_entries(&output_dir).is_empty());
    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &output_dir,
            &owner,
            attempt,
            persisted.record().identity(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::NotFound)
    ));
}

#[test]
fn current_retirement_accepts_an_exact_protected_v2_receipt() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let owner = producer(11);
    let attempt = begin(&output_dir, &owner, 12);
    let output = b"protected receipt Worker V3 output";
    let publication_plan = receipted_plan(&owner, attempt, output, 13);
    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        attempt,
        publication_plan,
        replay(b"protected receipt transcript"),
        output.to_vec(),
    )
    .unwrap();
    publish_exact_hsaco_evidence_for_attempt_v2(
        &output_dir,
        &owner,
        attempt,
        publication_plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes([14; 32]),
        compiler_closure(15),
        output,
    )
    .unwrap();

    clear_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        attempt,
        persisted.record().identity(),
    )
    .unwrap();
    assert!(v3_namespace_entries(&output_dir).is_empty());
}

#[test]
fn current_retirement_rejects_a_durable_receipt_for_another_plan() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let owner = producer(16);
    let attempt = begin(&output_dir, &owner, 17);
    let output = b"mismatched receipt Worker V3 output";
    let intent_plan = receipted_plan(&owner, attempt, output, 18);
    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        attempt,
        intent_plan,
        replay(b"mismatched receipt transcript"),
        output.to_vec(),
    )
    .unwrap();
    let other_plan = receipted_plan(&owner, attempt, output, 19);
    publish_exact_hsaco_evidence_for_attempt_v1(
        &output_dir,
        &owner,
        attempt,
        other_plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes([20; 32]),
        output,
    )
    .unwrap();

    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &output_dir,
            &owner,
            attempt,
            persisted.record().identity(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable)
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), 5);
}

#[test]
fn retirement_rejects_link_substitution_before_changing_any_protocol_name() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let owner = producer(21);
    let attempt = begin(&output_dir, &owner, 22);
    let output = b"hostile retirement output";
    let publication_plan = receipted_plan(&owner, attempt, output, 23);
    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        attempt,
        publication_plan,
        replay(b"hostile retirement transcript"),
        output.to_vec(),
    )
    .unwrap();
    publish_exact_hsaco_evidence_for_attempt_v1(
        &output_dir,
        &owner,
        attempt,
        publication_plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes([24; 32]),
        output,
    )
    .unwrap();
    let transcript = intent_entry(&output_dir, ".transcript");
    let hostile_link = output_dir.join("same-uid-hostile-link");
    fs::hard_link(&transcript, &hostile_link).unwrap();

    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &output_dir,
            &owner,
            attempt,
            persisted.record().identity(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::InvalidIntent {
            reason: WorkerV3PublicationIntentInvalidReasonV1::EntryNotPrivate,
            ..
        })
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), 5);
    assert!(intent_entry(&output_dir, ".record").is_file());
    assert!(
        !v3_namespace_entries(&output_dir)
            .iter()
            .any(|path| path.to_string_lossy().ends_with(".record.retiring"))
    );

    fs::remove_file(hostile_link).unwrap();
    clear_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        attempt,
        persisted.record().identity(),
    )
    .unwrap();
    assert!(v3_namespace_entries(&output_dir).is_empty());
}

#[test]
fn every_retirement_boundary_is_restart_safe() {
    let boundaries = [
        WorkerV3PublicationIntentBoundaryV1::RenameRecordToRetiring,
        WorkerV3PublicationIntentBoundaryV1::SyncRetiringName,
        WorkerV3PublicationIntentBoundaryV1::RemoveOuterHandoff,
        WorkerV3PublicationIntentBoundaryV1::RemoveExternalProviders,
        WorkerV3PublicationIntentBoundaryV1::RemoveTranscript,
        WorkerV3PublicationIntentBoundaryV1::RemoveOutput,
        WorkerV3PublicationIntentBoundaryV1::SyncRetiredAttachments,
        WorkerV3PublicationIntentBoundaryV1::RemoveRetiringRecord,
        WorkerV3PublicationIntentBoundaryV1::SyncRetirement,
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
        let owner = producer(30_u8.wrapping_add(index as u8));
        let attempt = begin(&output_dir, &owner, 60_u8.wrapping_add(index as u8));
        let output = format!("retirement output {boundary:?} {timing:?}").into_bytes();
        let publication_plan =
            receipted_plan(&owner, attempt, &output, 90_u8.wrapping_add(index as u8));
        let persisted = persist_worker_v3_publication_intent_v1(
            &output_dir,
            &owner,
            attempt,
            publication_plan,
            replay(format!("retirement transcript {index}").as_bytes()),
            output.clone(),
        )
        .unwrap();
        publish_exact_hsaco_evidence_for_attempt_v1(
            &output_dir,
            &owner,
            attempt,
            publication_plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
                [120_u8.wrapping_add(index as u8); 32],
            ),
            &output,
        )
        .unwrap();
        let point = WorkerV3PublicationIntentFaultPointV1 { boundary, timing };
        assert!(matches!(
            clear_worker_v3_publication_intent_v1_with_options(
                &output_dir,
                &owner,
                attempt,
                persisted.record().identity(),
                WorkerV3PublicationIntentOptionsV1::inject_crash(point),
            ),
            Err(WorkerV3PublicationIntentErrorV1::InjectedCrash { point: actual })
                if actual == point
        ));

        let resumed = clear_worker_v3_publication_intent_v1(
            &output_dir,
            &owner,
            attempt,
            persisted.record().identity(),
        );
        if !v3_namespace_entries(&output_dir).is_empty() {
            resumed.unwrap();
        } else {
            assert!(matches!(
                resumed,
                Ok(()) | Err(WorkerV3PublicationIntentErrorV1::NotFound)
            ));
        }
        assert!(v3_namespace_entries(&output_dir).is_empty());
    }
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
        let record_recoverable = matches!(
            boundary,
            WorkerV3PublicationIntentBoundaryV1::SyncRedoName
                | WorkerV3PublicationIntentBoundaryV1::RenameRedoToCanonical
                | WorkerV3PublicationIntentBoundaryV1::SyncCanonicalName
        ) || (boundary
            == WorkerV3PublicationIntentBoundaryV1::RenameRecordToRedo
            && timing == WorkerV3PublicationIntentFaultTimingV1::After);
        let direct_recovery =
            recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt);
        if record_recoverable {
            let recovered = direct_recovery.unwrap();
            assert_eq!(recovered.exact_output(), output);
            assert_eq!(recovered.finalizer_replay_transcript(), transcript);
        } else {
            assert!(matches!(
                direct_recovery,
                Err(WorkerV3PublicationIntentErrorV1::NotFound)
            ));
            assert!(v3_namespace_entries(&output_dir).is_empty());
        }
        let reconciled = persist_worker_v3_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            plan,
            replay(&transcript),
            output.clone(),
        )
        .unwrap();
        assert_eq!(
            reconciled.outcome(),
            if record_recoverable {
                WorkerV3PublicationIntentOutcomeV1::Recovered
            } else {
                WorkerV3PublicationIntentOutcomeV1::Persisted
            }
        );
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
fn same_input_retry_revalidates_and_reuses_every_canonical_attachment() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(84);
    let attempt = begin(&output_dir, &producer, 85);
    let output = b"same retry output";
    let transcript = b"same retry transcript";
    let publication_plan = plan(attempt, output, 86);
    let point = WorkerV3PublicationIntentFaultPointV1 {
        boundary: WorkerV3PublicationIntentBoundaryV1::RenameOutput,
        timing: WorkerV3PublicationIntentFaultTimingV1::After,
    };
    assert!(matches!(
        persist_worker_v3_publication_intent_v1_with_options(
            &output_dir,
            &producer,
            attempt,
            publication_plan,
            replay(transcript),
            output.to_vec(),
            WorkerV3PublicationIntentOptionsV1::inject_crash(point),
        ),
        Err(WorkerV3PublicationIntentErrorV1::InjectedCrash { .. })
    ));
    let before = [".handoff", ".providers", ".transcript", ".output"].map(|suffix| {
        let path = intent_entry(&output_dir, suffix);
        (
            path,
            fs::metadata(intent_entry(&output_dir, suffix))
                .unwrap()
                .ino(),
        )
    });

    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &producer,
        attempt,
        publication_plan,
        replay(transcript),
        output.to_vec(),
    )
    .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    for (path, inode) in before {
        assert_eq!(fs::metadata(path).unwrap().ino(), inode);
    }
}

#[test]
fn different_input_retry_replaces_the_complete_uncommitted_attachment_set() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(88);
    let attempt = begin(&output_dir, &producer, 89);
    let old_output = b"old finalized output";
    let point = WorkerV3PublicationIntentFaultPointV1 {
        boundary: WorkerV3PublicationIntentBoundaryV1::RenameOutput,
        timing: WorkerV3PublicationIntentFaultTimingV1::After,
    };
    assert!(matches!(
        persist_worker_v3_publication_intent_v1_with_options(
            &output_dir,
            &producer,
            attempt,
            plan(attempt, old_output, 90),
            replay_parts(
                b"old outer handoff",
                &[b"old provider"],
                b"old transcript",
            ),
            old_output.to_vec(),
            WorkerV3PublicationIntentOptionsV1::inject_crash(point),
        ),
        Err(WorkerV3PublicationIntentErrorV1::InjectedCrash { point: actual }) if actual == point
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), 4);

    let new_output = b"new finalized output";
    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &producer,
        attempt,
        plan(attempt, new_output, 91),
        replay_parts(
            b"new outer handoff",
            &[b"new provider alpha", b"new provider beta"],
            b"new transcript",
        ),
        new_output.to_vec(),
    )
    .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV3PublicationIntentOutcomeV1::Persisted
    );
    assert_eq!(persisted.outer_handoff(), b"new outer handoff");
    assert_eq!(persisted.external_providers().len(), 2);
    assert_eq!(persisted.finalizer_replay_transcript(), b"new transcript");
    assert_eq!(persisted.exact_output(), new_output);

    let recovered =
        recover_worker_v3_publication_intent_v1(&output_dir, &producer, attempt).unwrap();
    assert_eq!(recovered.outer_handoff(), b"new outer handoff");
    assert_eq!(
        recovered.external_providers().get(0),
        Some(&b"new provider alpha"[..])
    );
    assert_eq!(recovered.exact_output(), new_output);
}

#[test]
fn superseded_same_producer_can_scavenge_only_an_uncommitted_exact_namespace() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(92);
    let stale = begin(&output_dir, &producer, 93);
    let output = b"abandoned output";
    let point = WorkerV3PublicationIntentFaultPointV1 {
        boundary: WorkerV3PublicationIntentBoundaryV1::RenameOutput,
        timing: WorkerV3PublicationIntentFaultTimingV1::After,
    };
    assert!(matches!(
        persist_worker_v3_publication_intent_v1_with_options(
            &output_dir,
            &producer,
            stale,
            plan(stale, output, 94),
            replay(b"abandoned transcript"),
            output.to_vec(),
            WorkerV3PublicationIntentOptionsV1::inject_crash(point),
        ),
        Err(WorkerV3PublicationIntentErrorV1::InjectedCrash { .. })
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), 4);
    let current = begin(&output_dir, &producer, 95);
    assert!(current.generation() > stale.generation());

    assert_eq!(
        scavenge_worker_v3_publication_intent_occurrence_v1(&output_dir, &producer, stale).unwrap(),
        WorkerV3PublicationIntentScavengeOutcomeV1::Removed { entries: 4 }
    );
    assert!(v3_namespace_entries(&output_dir).is_empty());
    assert_eq!(
        scavenge_worker_v3_publication_intent_occurrence_v1(&output_dir, &producer, stale).unwrap(),
        WorkerV3PublicationIntentScavengeOutcomeV1::NotFound
    );

    let current_output = b"current committed output";
    persist_worker_v3_publication_intent_v1(
        &output_dir,
        &producer,
        current,
        plan(current, current_output, 96),
        replay(b"current transcript"),
        current_output.to_vec(),
    )
    .unwrap();
    let _successor = begin(&output_dir, &producer, 97);
    assert_eq!(
        scavenge_worker_v3_publication_intent_occurrence_v1(&output_dir, &producer, current)
            .unwrap(),
        WorkerV3PublicationIntentScavengeOutcomeV1::Removed { entries: 5 }
    );
    assert!(v3_namespace_entries(&output_dir).is_empty());
}

#[test]
fn successor_scavenge_resumes_an_interrupted_receipt_bound_retirement() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let owner = producer(107);
    let stale = begin(&output_dir, &owner, 108);
    let output = b"partially retired predecessor output";
    let publication_plan = receipted_plan(&owner, stale, output, 109);
    let persisted = persist_worker_v3_publication_intent_v1(
        &output_dir,
        &owner,
        stale,
        publication_plan,
        replay(b"partially retired predecessor transcript"),
        output.to_vec(),
    )
    .unwrap();
    publish_exact_hsaco_evidence_for_attempt_v2(
        &output_dir,
        &owner,
        stale,
        publication_plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes([110; 32]),
        compiler_closure(111),
        output,
    )
    .unwrap();
    let point = WorkerV3PublicationIntentFaultPointV1 {
        boundary: WorkerV3PublicationIntentBoundaryV1::RemoveOuterHandoff,
        timing: WorkerV3PublicationIntentFaultTimingV1::After,
    };
    assert!(matches!(
        clear_worker_v3_publication_intent_v1_with_options(
            &output_dir,
            &owner,
            stale,
            persisted.record().identity(),
            WorkerV3PublicationIntentOptionsV1::inject_crash(point),
        ),
        Err(WorkerV3PublicationIntentErrorV1::InjectedCrash { point: actual }) if actual == point
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), 4);

    let successor = begin(&output_dir, &owner, 112);
    assert!(successor.generation() > stale.generation());
    assert_eq!(
        scavenge_worker_v3_publication_intent_occurrence_v1(&output_dir, &owner, stale).unwrap(),
        WorkerV3PublicationIntentScavengeOutcomeV1::Removed { entries: 4 }
    );
    assert!(v3_namespace_entries(&output_dir).is_empty());
}

#[test]
fn scavenge_rejects_wrong_authority_and_unsafe_exact_entries_without_partial_cleanup() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(98);
    let stale = begin(&output_dir, &producer, 99);
    let output = b"unsafe abandoned output";
    let point = WorkerV3PublicationIntentFaultPointV1 {
        boundary: WorkerV3PublicationIntentBoundaryV1::RenameOutput,
        timing: WorkerV3PublicationIntentFaultTimingV1::After,
    };
    assert!(
        persist_worker_v3_publication_intent_v1_with_options(
            &output_dir,
            &producer,
            stale,
            plan(stale, output, 100),
            replay(b"unsafe transcript"),
            output.to_vec(),
            WorkerV3PublicationIntentOptionsV1::inject_crash(point),
        )
        .is_err()
    );
    let wrong_producer =
        ProducerIdentity::from_codegen("wrong_crate_name", Some(Path::new("/src/worker-v3-98.rs")))
            .unwrap();
    assert!(matches!(
        scavenge_worker_v3_publication_intent_occurrence_v1(&output_dir, &wrong_producer, stale,),
        Err(WorkerV3PublicationIntentErrorV1::ScavengeNotAuthorized)
    ));

    let _current = begin(&output_dir, &producer, 103);
    let unsafe_entry = intent_entry(&output_dir, ".transcript");
    fs::remove_file(&unsafe_entry).unwrap();
    symlink("nonexistent", &unsafe_entry).unwrap();
    let entries_before = v3_namespace_entries(&output_dir).len();
    assert!(matches!(
        scavenge_worker_v3_publication_intent_occurrence_v1(&output_dir, &producer, stale),
        Err(WorkerV3PublicationIntentErrorV1::InvalidIntent {
            reason: WorkerV3PublicationIntentInvalidReasonV1::EntryNotPrivate,
            ..
        })
    ));
    assert_eq!(v3_namespace_entries(&output_dir).len(), entries_before);
}

#[test]
fn persist_rejects_oversized_spare_output_capacity_before_writing() {
    let temp = TestDirectory::new();
    let output_dir = temp.output();
    let producer = producer(104);
    let attempt = begin(&output_dir, &producer, 105);
    let mut output = Vec::with_capacity(MAX_WORKER_V3_PUBLICATION_INTENT_OUTPUT_BYTES_V1 + 1);
    output.extend_from_slice(b"small output in oversized owner");
    let publication_plan = plan(attempt, &output, 106);
    assert!(matches!(
        persist_worker_v3_publication_intent_v1(
            &output_dir,
            &producer,
            attempt,
            publication_plan,
            replay(b"capacity transcript"),
            output,
        ),
        Err(WorkerV3PublicationIntentErrorV1::Codec(
            WorkerV3PublicationIntentCodecErrorV1::InvalidOutputCapacity { .. }
        ))
    ));
    assert!(v3_namespace_entries(&output_dir).is_empty());
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
