use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableArtifactBoundaryV1, DurableFaultTimingV1,
    DurableJournalBoundaryV1, DurableJournalStageV1, DurableLinkPublicationFaultPointV1,
    DurableLinkPublicationOptionsV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES, MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES,
    PackageIdentityV1, PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    WorkerV2PublicationIntentBoundaryV1, WorkerV2PublicationIntentErrorV1,
    WorkerV2PublicationIntentFaultPointV1, WorkerV2PublicationIntentFaultTimingV1,
    WorkerV2PublicationIntentIdentityV1, WorkerV2PublicationIntentOptionsV1,
    WorkerV2PublicationIntentOutcomeV1, begin_build_attempt, clear_worker_v2_publication_intent_v1,
    consume_compiler_module_handoff_v1, persist_worker_v2_publication_intent_v1,
    persist_worker_v2_publication_intent_v1_with_options, publish_compiler_module_handoff_v1,
    publish_exact_hsaco_evidence_for_attempt_v1_with_options,
    recover_worker_v2_publication_intent_v1,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v2-publication-intent-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }

    fn output(&self) -> PathBuf {
        self.path.join("output")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn producer(crate_name: &str, source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen(crate_name, Some(Path::new(source))).unwrap()
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

fn scope(seed: u8) -> LinkPublicationScopeV1 {
    LinkPublicationScopeV1::new(
        PackageIdentityV1::from_bytes([seed; 32]),
        KernelSetIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
        TargetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
    )
}

fn plan(
    attempt: BuildAttempt,
    publication_scope: LinkPublicationScopeV1,
    seed: u8,
    output: &[u8],
) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        publication_scope,
        CanonicalLinkRequestIdentityV1::from_bytes([seed; 32]),
        PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
        ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
        LinkedOutputIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
        FinalizationIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
        FinalizedOutputIdentityV1::from_bytes(digest(output)),
        AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
    )
}

fn evidence(seed: u8) -> UpstreamCodeObjectEvidenceIdentityV1 {
    UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed; 32])
}

fn intent_entry(output: &Path, suffix: &str) -> PathBuf {
    let entries = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".fe2o3-worker-v2-publication-intent-v1-")
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with(suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1, "expected one intent {suffix} entry");
    entries.into_iter().next().unwrap()
}

#[test]
fn consumed_handoff_can_be_replaced_by_exact_restart_intent() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/consumed.rs");
    let attempt = begin(&output, &producer, 1);
    let handoff = b"canonical compiler handoff";
    publish_compiler_module_handoff_v1(&output, &producer, attempt, handoff).unwrap();
    let consumed = consume_compiler_module_handoff_v1(&output, &producer, attempt).unwrap();
    let exact_output = b"independently admitted Worker V2 HSACO";
    let plan = plan(attempt, scope(0x10), 0x20, exact_output);
    let upstream =
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes(*consumed.identity().as_bytes());

    let persisted = persist_worker_v2_publication_intent_v1(
        &output,
        &producer,
        attempt,
        plan,
        upstream,
        exact_output,
    )
    .unwrap();
    assert_eq!(
        persisted.outcome(),
        WorkerV2PublicationIntentOutcomeV1::Persisted
    );
    drop(persisted);

    let restarted = recover_worker_v2_publication_intent_v1(&output, &producer, attempt).unwrap();
    assert_eq!(
        restarted.outcome(),
        WorkerV2PublicationIntentOutcomeV1::Recovered
    );
    assert_eq!(restarted.record().attempt(), attempt);
    assert_eq!(restarted.record().plan(), plan);
    assert_eq!(restarted.record().upstream_evidence(), upstream);
    assert_eq!(
        restarted.record().output_identity(),
        plan.finalized_output()
    );
    assert_eq!(restarted.record().output_length(), exact_output.len());
    assert_eq!(restarted.exact_output(), exact_output);
    assert!(!restarted.record().grants_publication_authority());
    assert!(!restarted.record().grants_compiler_authority());
    assert!(!restarted.grants_load_authority());
    assert!(!restarted.grants_launch_authority());
    assert_eq!(
        MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES,
        fs::metadata(intent_entry(&output, ".record"))
            .unwrap()
            .len() as usize
    );
}

#[test]
fn exact_intent_survives_backend_claim_and_process_restart() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/claimed.rs");
    let attempt = begin(&output, &producer, 2);
    let exact_output = b"claim-bound Worker V2 output";
    let plan = plan(attempt, scope(0x11), 0x21, exact_output);
    let upstream = evidence(0x31);
    persist_worker_v2_publication_intent_v1(
        &output,
        &producer,
        attempt,
        plan,
        upstream,
        exact_output,
    )
    .unwrap();

    let point = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };
    assert!(
        publish_exact_hsaco_evidence_for_attempt_v1_with_options(
            &output,
            &producer,
            attempt,
            plan,
            upstream,
            exact_output,
            DurableLinkPublicationOptionsV1::inject_crash(point),
        )
        .is_err()
    );

    let restarted = recover_worker_v2_publication_intent_v1(&output, &producer, attempt).unwrap();
    assert_eq!(restarted.record().plan(), plan);
    assert_eq!(restarted.exact_output(), exact_output);
    assert_eq!(
        persist_worker_v2_publication_intent_v1(
            &output,
            &producer,
            attempt,
            plan,
            upstream,
            exact_output,
        )
        .unwrap()
        .outcome(),
        WorkerV2PublicationIntentOutcomeV1::Recovered
    );
}

#[test]
fn every_persistence_boundary_recovers_exactly() {
    let boundaries = [
        WorkerV2PublicationIntentBoundaryV1::CreateOutputTemp,
        WorkerV2PublicationIntentBoundaryV1::WriteOutputTemp,
        WorkerV2PublicationIntentBoundaryV1::SyncOutputTemp,
        WorkerV2PublicationIntentBoundaryV1::RenameOutput,
        WorkerV2PublicationIntentBoundaryV1::SyncOutputName,
        WorkerV2PublicationIntentBoundaryV1::CreateRecordTemp,
        WorkerV2PublicationIntentBoundaryV1::WriteRecordTemp,
        WorkerV2PublicationIntentBoundaryV1::SyncRecordTemp,
        WorkerV2PublicationIntentBoundaryV1::RenameRecordToRedo,
        WorkerV2PublicationIntentBoundaryV1::SyncRedoName,
        WorkerV2PublicationIntentBoundaryV1::RenameRedoToCanonical,
        WorkerV2PublicationIntentBoundaryV1::SyncCanonicalName,
    ];
    let timings = [
        WorkerV2PublicationIntentFaultTimingV1::Before,
        WorkerV2PublicationIntentFaultTimingV1::After,
    ];
    for (index, (boundary, timing)) in boundaries
        .into_iter()
        .flat_map(|boundary| timings.into_iter().map(move |timing| (boundary, timing)))
        .enumerate()
    {
        let temp = TestDirectory::new();
        let output = temp.output();
        let producer = producer("kernel", &format!("/src/crash-{index}.rs"));
        let attempt = begin(&output, &producer, 20 + index as u8);
        let exact_output = format!("crash boundary {boundary:?} {timing:?}").into_bytes();
        let plan = plan(attempt, scope(0x40 + index as u8), 0x60, &exact_output);
        let upstream = evidence(0x70);
        let point = WorkerV2PublicationIntentFaultPointV1 { boundary, timing };
        assert!(matches!(
            persist_worker_v2_publication_intent_v1_with_options(
                &output,
                &producer,
                attempt,
                plan,
                upstream,
                &exact_output,
                WorkerV2PublicationIntentOptionsV1::inject_crash(point),
            ),
            Err(WorkerV2PublicationIntentErrorV1::InjectedCrash { point: actual }) if actual == point
        ));

        let reconciled = persist_worker_v2_publication_intent_v1(
            &output,
            &producer,
            attempt,
            plan,
            upstream,
            &exact_output,
        )
        .unwrap();
        assert_eq!(reconciled.record().plan(), plan);
        assert_eq!(reconciled.exact_output(), exact_output);
        let restarted =
            recover_worker_v2_publication_intent_v1(&output, &producer, attempt).unwrap();
        assert_eq!(restarted.record(), reconciled.record());
        assert_eq!(restarted.exact_output(), exact_output);
    }
}

#[test]
fn producer_attempt_upstream_plan_and_candidate_output_substitution_fail_closed() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("kernel", "/src/owner.rs");
    let intruder = producer("kernel", "/src/intruder.rs");
    let attempt = begin(&output, &owner, 60);
    let exact_output = b"owner output";
    let original = plan(attempt, scope(0x55), 0x65, exact_output);
    let upstream = evidence(0x75);
    persist_worker_v2_publication_intent_v1(
        &output,
        &owner,
        attempt,
        original,
        upstream,
        exact_output,
    )
    .unwrap();

    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&output, &intruder, attempt),
        Err(WorkerV2PublicationIntentErrorV1::Attempt { .. })
    ));
    let unrelated = begin(&output, &intruder, 61);
    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&output, &owner, unrelated),
        Err(WorkerV2PublicationIntentErrorV1::Attempt { .. })
    ));

    assert!(matches!(
        persist_worker_v2_publication_intent_v1(
            &output,
            &owner,
            attempt,
            original,
            evidence(0x76),
            exact_output,
        ),
        Err(WorkerV2PublicationIntentErrorV1::ConflictingIntent)
    ));
    let replacement_output = b"different valid output";
    let replacement_plan = plan(attempt, original.scope(), 0x66, replacement_output);
    assert!(matches!(
        persist_worker_v2_publication_intent_v1(
            &output,
            &owner,
            attempt,
            replacement_plan,
            upstream,
            replacement_output,
        ),
        Err(WorkerV2PublicationIntentErrorV1::ConflictingIntent)
    ));
    assert!(matches!(
        persist_worker_v2_publication_intent_v1(
            &output,
            &owner,
            attempt,
            original,
            upstream,
            b"wrong bytes",
        ),
        Err(WorkerV2PublicationIntentErrorV1::OutputDigestMismatch)
    ));
}

#[test]
fn record_output_checksum_and_filesystem_substitution_fail_closed() {
    // Output-byte substitution.
    let temp = TestDirectory::new();
    let output = temp.output();
    let output_producer = producer("kernel", "/src/output-substitute.rs");
    let attempt = begin(&output, &output_producer, 70);
    let bytes = b"original bytes";
    let output_plan = plan(attempt, scope(0x61), 0x71, bytes);
    persist_worker_v2_publication_intent_v1(
        &output,
        &output_producer,
        attempt,
        output_plan,
        evidence(0x81),
        bytes,
    )
    .unwrap();
    fs::write(intent_entry(&output, ".output"), b"replaced bytes").unwrap();
    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&output, &output_producer, attempt),
        Err(WorkerV2PublicationIntentErrorV1::OutputDigestMismatch)
    ));

    // Checksummed record substitution.
    let temp = TestDirectory::new();
    let output = temp.output();
    let checksum_producer = producer("kernel", "/src/checksum.rs");
    let attempt = begin(&output, &checksum_producer, 71);
    let bytes = b"checksum bytes";
    let checksum_plan = plan(attempt, scope(0x62), 0x72, bytes);
    persist_worker_v2_publication_intent_v1(
        &output,
        &checksum_producer,
        attempt,
        checksum_plan,
        evidence(0x82),
        bytes,
    )
    .unwrap();
    let record_path = intent_entry(&output, ".record");
    let mut record = fs::read(&record_path).unwrap();
    let middle = record.len() / 2;
    record[middle] ^= 1;
    fs::write(&record_path, record).unwrap();
    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&output, &checksum_producer, attempt),
        Err(WorkerV2PublicationIntentErrorV1::InvalidIntent { .. })
    ));

    // Symlink substitution is rejected without following it.
    let temp = TestDirectory::new();
    let output = temp.output();
    let symlink_producer = producer("kernel", "/src/symlink.rs");
    let attempt = begin(&output, &symlink_producer, 72);
    let bytes = b"symlink bytes";
    let symlink_plan = plan(attempt, scope(0x63), 0x73, bytes);
    persist_worker_v2_publication_intent_v1(
        &output,
        &symlink_producer,
        attempt,
        symlink_plan,
        evidence(0x83),
        bytes,
    )
    .unwrap();
    let output_path = intent_entry(&output, ".output");
    let outside = temp.path.join("outside");
    fs::write(&outside, bytes).unwrap();
    fs::remove_file(&output_path).unwrap();
    symlink(&outside, &output_path).unwrap();
    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&output, &symlink_producer, attempt),
        Err(WorkerV2PublicationIntentErrorV1::InvalidIntent { .. })
    ));
    assert_eq!(fs::read(outside).unwrap(), bytes);

    // Hard-link substitution is rejected without unlinking the other name.
    let temp = TestDirectory::new();
    let output = temp.output();
    let hardlink_producer = producer("kernel", "/src/hardlink.rs");
    let attempt = begin(&output, &hardlink_producer, 73);
    let bytes = b"hardlink bytes";
    let hardlink_plan = plan(attempt, scope(0x64), 0x74, bytes);
    persist_worker_v2_publication_intent_v1(
        &output,
        &hardlink_producer,
        attempt,
        hardlink_plan,
        evidence(0x84),
        bytes,
    )
    .unwrap();
    let output_path = intent_entry(&output, ".output");
    let outside = temp.path.join("outside");
    fs::rename(&output_path, &outside).unwrap();
    fs::hard_link(&outside, &output_path).unwrap();
    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&output, &hardlink_producer, attempt),
        Err(WorkerV2PublicationIntentErrorV1::InvalidIntent { .. })
    ));
    assert_eq!(fs::read(outside).unwrap(), bytes);
}

#[test]
fn record_from_another_attempt_cannot_be_substituted() {
    let first = TestDirectory::new();
    let first_output = first.output();
    let first_producer = producer("kernel", "/src/first.rs");
    let first_attempt = begin(&first_output, &first_producer, 80);
    let first_bytes = b"first output";
    persist_worker_v2_publication_intent_v1(
        &first_output,
        &first_producer,
        first_attempt,
        plan(first_attempt, scope(0x71), 0x81, first_bytes),
        evidence(0x91),
        first_bytes,
    )
    .unwrap();

    let second = TestDirectory::new();
    let second_output = second.output();
    let second_producer = producer("kernel", "/src/second.rs");
    let second_attempt = begin(&second_output, &second_producer, 81);
    let second_bytes = b"second outpt";
    persist_worker_v2_publication_intent_v1(
        &second_output,
        &second_producer,
        second_attempt,
        plan(second_attempt, scope(0x72), 0x82, second_bytes),
        evidence(0x92),
        second_bytes,
    )
    .unwrap();

    fs::copy(
        intent_entry(&second_output, ".record"),
        intent_entry(&first_output, ".record"),
    )
    .unwrap();
    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&first_output, &first_producer, first_attempt,),
        Err(WorkerV2PublicationIntentErrorV1::InvalidIntent { .. })
    ));
}

#[test]
fn fresh_intent_cannot_be_fabricated_after_backend_claim() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/late.rs");
    let attempt = begin(&output, &producer, 90);
    let bytes = b"late intent output";
    let plan = plan(attempt, scope(0x81), 0x91, bytes);
    let upstream = evidence(0xa1);
    let point = DurableLinkPublicationFaultPointV1::Artifact {
        boundary: DurableArtifactBoundaryV1::CreateTemp,
        timing: DurableFaultTimingV1::Before,
    };
    assert!(
        publish_exact_hsaco_evidence_for_attempt_v1_with_options(
            &output,
            &producer,
            attempt,
            plan,
            upstream,
            bytes,
            DurableLinkPublicationOptionsV1::inject_crash(point),
        )
        .is_err()
    );
    assert!(matches!(
        persist_worker_v2_publication_intent_v1(&output, &producer, attempt, plan, upstream, bytes,),
        Err(WorkerV2PublicationIntentErrorV1::Attempt { .. })
    ));
}

#[test]
fn exact_identity_is_required_to_clear_an_intent() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/clear.rs");
    let attempt = begin(&output, &producer, 100);
    let bytes = b"clearable output";
    let plan = plan(attempt, scope(0x91), 0xa1, bytes);
    let persisted = persist_worker_v2_publication_intent_v1(
        &output,
        &producer,
        attempt,
        plan,
        evidence(0xb1),
        bytes,
    )
    .unwrap();
    assert!(matches!(
        clear_worker_v2_publication_intent_v1(
            &output,
            &producer,
            attempt,
            WorkerV2PublicationIntentIdentityV1::from_bytes([0xff; 32]),
        ),
        Err(WorkerV2PublicationIntentErrorV1::IntentIdentityMismatch)
    ));
    assert!(matches!(
        clear_worker_v2_publication_intent_v1(
            &output,
            &producer,
            attempt,
            persisted.record().identity(),
        ),
        Err(WorkerV2PublicationIntentErrorV1::Attempt { .. })
    ));
    publish_exact_hsaco_evidence_for_attempt_v1_with_options(
        &output,
        &producer,
        attempt,
        plan,
        evidence(0xb1),
        bytes,
        DurableLinkPublicationOptionsV1::default(),
    )
    .unwrap();
    clear_worker_v2_publication_intent_v1(
        &output,
        &producer,
        attempt,
        persisted.record().identity(),
    )
    .unwrap();
    assert!(matches!(
        recover_worker_v2_publication_intent_v1(&output, &producer, attempt),
        Err(WorkerV2PublicationIntentErrorV1::NotFound)
    ));
}

#[test]
fn output_size_is_bounded_before_store_mutation() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/bounds.rs");
    let attempt = begin(&output, &producer, 110);
    let empty_plan = plan(attempt, scope(0xa1), 0xb1, b"");
    assert!(matches!(
        persist_worker_v2_publication_intent_v1(
            &output,
            &producer,
            attempt,
            empty_plan,
            evidence(0xc1),
            b"",
        ),
        Err(WorkerV2PublicationIntentErrorV1::InvalidOutputSize { actual: 0, .. })
    ));

    let oversized = vec![0u8; MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES + 1];
    let oversized_plan = plan(attempt, scope(0xa2), 0xb2, &oversized);
    assert!(matches!(
        persist_worker_v2_publication_intent_v1(
            &output,
            &producer,
            attempt,
            oversized_plan,
            evidence(0xc2),
            &oversized,
        ),
        Err(WorkerV2PublicationIntentErrorV1::InvalidOutputSize { .. })
    ));
    assert!(fs::read_dir(&output).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".fe2o3-worker-v2-publication-intent-v1-")
    }));
}
