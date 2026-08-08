use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationErrorV1,
    AttemptScopedHsacoPublicationOutcomeV1, BackendPublicationReceiptValidationErrorV1,
    BuildAttempt, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
    DurableArtifactBoundaryV1, DurableFaultTimingV1, DurableJournalBoundaryV1,
    DurableJournalStageV1, DurableLinkPublicationError, DurableLinkPublicationFaultPointV1,
    DurableLinkPublicationOptionsV1, DurableLinkPublicationPlanV1, FinalizationIdentityV1,
    FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    PackageIdentityV1, PersistedBackendReceiptV1, PinnedWorkerIdentityV1, ProducerIdentity,
    TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    begin_build_attempt, fail_build_attempt, finish_build_attempt,
    publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v1_with_options, read_backend_publication_receipt_v1,
    recover_durable_link_publication_v1, recover_published_hsaco_claim_for_attempt_v1,
    validate_backend_publication_receipt_v1,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-attempt-hsaco-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
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

fn identity(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn producer(crate_name: &str, source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen(crate_name, Some(Path::new(source))).unwrap()
}

fn session(seed: u8) -> BuildSession {
    BuildSession::from_bytes([seed; 16])
}

fn begin(output: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        output,
        producer,
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32]),
        session(seed),
    )
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
    attempt: BuildAttempt,
    publication_scope: LinkPublicationScopeV1,
    seed: u8,
    bytes: &[u8],
) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        publication_scope,
        CanonicalLinkRequestIdentityV1::from_bytes(identity(seed)),
        PinnedWorkerIdentityV1::from_bytes(identity(seed.wrapping_add(1))),
        ValidatedResponseIdentityV1::from_bytes(identity(seed.wrapping_add(2))),
        LinkedOutputIdentityV1::from_bytes(identity(seed.wrapping_add(3))),
        FinalizationIdentityV1::from_bytes(identity(seed.wrapping_add(4))),
        FinalizedOutputIdentityV1::from_bytes(Sha256::digest(bytes).into()),
        AtomicPublicationIdentityV1::from_bytes(identity(seed.wrapping_add(5))),
    )
}

fn upstream_evidence(plan: DurableLinkPublicationPlanV1) -> UpstreamCodeObjectEvidenceIdentityV1 {
    UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
        Sha256::digest(plan.finalization().as_bytes()).into(),
    )
}

fn publish_exact(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    bytes: &[u8],
) -> Result<
    fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV1,
    AttemptScopedHsacoPublicationErrorV1,
> {
    publish_exact_hsaco_evidence_for_attempt_v1(
        output,
        producer,
        attempt,
        plan,
        upstream_evidence(plan),
        bytes,
    )
}

fn publish_exact_with_options(
    output: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    bytes: &[u8],
    options: DurableLinkPublicationOptionsV1,
) -> Result<
    fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV1,
    AttemptScopedHsacoPublicationErrorV1,
> {
    publish_exact_hsaco_evidence_for_attempt_v1_with_options(
        output,
        producer,
        attempt,
        plan,
        upstream_evidence(plan),
        bytes,
        options,
    )
}

fn fake_attempt(attempt: BuildAttempt, seed: u8) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{}:{}:{}",
        attempt.generation(),
        session(seed),
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32])
    ))
    .unwrap()
}

fn one_managed_entry(output: &Path, prefix: &str, suffix: &str) -> PathBuf {
    let entries = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(prefix) && name.ends_with(suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    entries.into_iter().next().unwrap()
}

#[test]
fn publishes_exact_bytes_and_complete_identity_chain_once() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/kernel.rs");
    let attempt = begin(&output, &producer, 1);
    let bytes = b"exact gfx942 hsaco bytes";
    let plan = plan(attempt, scope(0x10), 0x20, bytes);

    let result = publish_exact(&output, &producer, attempt, plan, bytes).unwrap();
    assert_eq!(
        result.outcome(),
        AttemptScopedHsacoPublicationOutcomeV1::Published
    );
    assert_eq!(result.snapshot().artifact().bytes(), bytes);
    let record = result.snapshot().record();
    assert_eq!(record.attempt(), attempt);
    assert_eq!(record.scope(), plan.scope());
    assert_eq!(record.request(), plan.request());
    assert_eq!(record.worker(), Some(plan.worker()));
    assert_eq!(record.response(), Some(plan.response()));
    assert_eq!(record.linked_output(), Some(plan.linked_output()));
    assert_eq!(record.finalization(), Some(plan.finalization()));
    assert_eq!(record.finalized_output(), Some(plan.finalized_output()));
    assert_eq!(record.publication(), Some(plan.publication()));
    assert!(!result.snapshot().grants_load_authority());
    assert!(!result.snapshot().grants_launch_authority());
    let receipt = result.receipt();
    assert_eq!(
        receipt.upstream_evidence_identity(),
        upstream_evidence(plan).as_bytes()
    );
    assert_eq!(
        receipt.finalized_output_identity(),
        *plan.finalized_output().as_bytes()
    );
    assert_eq!(
        receipt.publication_identity(),
        *plan.publication().as_bytes()
    );
    assert_eq!(
        read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap(),
        PersistedBackendReceiptV1::Provenance(receipt)
    );

    assert!(matches!(
        publish_exact(&output, &producer, attempt, plan, bytes),
        Err(AttemptScopedHsacoPublicationErrorV1::ReceiptAlreadyPersisted { .. })
    ));
    finish_build_attempt(&output, &producer, attempt).unwrap();

    let recovered = recover_durable_link_publication_v1(&output, plan.scope())
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record(), record);
    assert_eq!(recovered.artifact().bytes(), bytes);
}

#[test]
fn public_receipt_validation_rejects_typed_lineage_substitution() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let publisher = producer("kernel", "/src/receipt-validation.rs");
    let attempt = begin(&output, &publisher, 5);
    let bytes = b"exact finalized bytes";
    let publication_scope = scope(0x31);
    let publication_plan = plan(attempt, publication_scope, 0x41, bytes);
    let upstream = upstream_evidence(publication_plan);
    let result = publish_exact(&output, &publisher, attempt, publication_plan, bytes).unwrap();
    let receipt = result.receipt();

    assert_eq!(
        validate_backend_publication_receipt_v1(
            &publisher,
            attempt,
            publication_plan,
            upstream,
            receipt
        ),
        Ok(())
    );
    let substituted_producer = producer("kernel", "/src/substituted.rs");
    assert_eq!(
        validate_backend_publication_receipt_v1(
            &substituted_producer,
            attempt,
            publication_plan,
            upstream,
            receipt
        ),
        Err(BackendPublicationReceiptValidationErrorV1::ProducerIdentityMismatch)
    );
    assert_eq!(
        validate_backend_publication_receipt_v1(
            &publisher,
            attempt,
            publication_plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0xa5; 32]),
            receipt
        ),
        Err(BackendPublicationReceiptValidationErrorV1::UpstreamEvidenceIdentityMismatch)
    );
    let substituted_plan = plan(attempt, publication_scope, 0x42, bytes);
    assert_eq!(
        validate_backend_publication_receipt_v1(
            &publisher,
            attempt,
            substituted_plan,
            upstream,
            receipt
        ),
        Err(BackendPublicationReceiptValidationErrorV1::PlanCommitmentMismatch)
    );
    assert_eq!(
        validate_backend_publication_receipt_v1(
            &publisher,
            fake_attempt(attempt, 0x51),
            publication_plan,
            upstream,
            receipt
        ),
        Err(BackendPublicationReceiptValidationErrorV1::PlanAttemptMismatch)
    );
}

#[test]
fn recovered_claim_requires_the_exact_complete_publication_lineage() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let publisher = producer("kernel", "/src/recovered-claim.rs");
    let attempt = begin(&output, &publisher, 0x61);
    let bytes = b"exact recoverable claim bytes";
    let publication_scope = scope(0x62);
    let publication_plan = plan(attempt, publication_scope, 0x63, bytes);
    let upstream = upstream_evidence(publication_plan);
    let receipt = publish_exact(&output, &publisher, attempt, publication_plan, bytes)
        .unwrap()
        .receipt();

    let recovered = recover_published_hsaco_claim_for_attempt_v1(
        &output,
        &publisher,
        attempt,
        publication_plan,
        upstream,
        receipt,
    )
    .unwrap();
    assert_eq!(recovered.plan(), publication_plan);
    assert_eq!(recovered.upstream_evidence(), upstream);
    assert_eq!(recovered.receipt(), receipt);
    assert!(!recovered.grants_load_authority());
    assert!(!recovered.grants_launch_authority());

    let substituted_producer = producer("kernel", "/src/recovered-claim-substituted.rs");
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v1(
            &output,
            &substituted_producer,
            attempt,
            publication_plan,
            upstream,
            receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch)
    ));
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v1(
            &output,
            &publisher,
            fake_attempt(attempt, 0x64),
            publication_plan,
            upstream,
            receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch)
    ));
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v1(
            &output,
            &publisher,
            attempt,
            plan(attempt, publication_scope, 0x65, bytes),
            upstream,
            receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch)
    ));
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v1(
            &output,
            &publisher,
            attempt,
            publication_plan,
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x66; 32]),
            receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch)
    ));

    let other_temp = TestDirectory::new();
    let other_output = other_temp.output();
    let other_publisher = producer("other", "/src/other-recovered-claim.rs");
    let other_attempt = begin(&other_output, &other_publisher, 0x67);
    let other_plan = plan(other_attempt, scope(0x68), 0x69, b"other claim bytes");
    let other_receipt = publish_exact(
        &other_output,
        &other_publisher,
        other_attempt,
        other_plan,
        b"other claim bytes",
    )
    .unwrap()
    .receipt();
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v1(
            &output,
            &publisher,
            attempt,
            publication_plan,
            upstream,
            other_receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch)
    ));
}

#[test]
fn recovered_claim_rejects_durable_filesystem_substitution() {
    for substitution in ["missing", "mutated", "symlink", "record"] {
        let temp = TestDirectory::new();
        let output = temp.output();
        let publisher = producer("kernel", &format!("/src/recovery-{substitution}.rs"));
        let attempt = begin(&output, &publisher, 0x71);
        let bytes = b"filesystem-bound exact claim";
        let publication_plan = plan(attempt, scope(0x72), 0x73, bytes);
        let upstream = upstream_evidence(publication_plan);
        let receipt = publish_exact(&output, &publisher, attempt, publication_plan, bytes)
            .unwrap()
            .receipt();
        let artifact = one_managed_entry(&output, ".fe2o3-link-artifact-v1-", ".bin");
        match substitution {
            "missing" => fs::remove_file(&artifact).unwrap(),
            "mutated" => fs::write(&artifact, vec![b'x'; bytes.len()]).unwrap(),
            "symlink" => {
                let target = temp.path.join("substituted-artifact");
                fs::write(&target, bytes).unwrap();
                fs::remove_file(&artifact).unwrap();
                symlink(target, artifact).unwrap();
            }
            "record" => {
                let record = one_managed_entry(&output, ".fe2o3-link-publication-v1-", ".record");
                let mut bytes = fs::read(&record).unwrap();
                *bytes.last_mut().unwrap() ^= 1;
                fs::write(record, bytes).unwrap();
            }
            _ => unreachable!(),
        }

        assert!(
            recover_published_hsaco_claim_for_attempt_v1(
                &output,
                &publisher,
                attempt,
                publication_plan,
                upstream,
                receipt,
            )
            .is_err(),
            "{substitution} substitution unexpectedly recovered a claim"
        );
    }
}

#[test]
fn concurrent_callers_have_one_attempt_and_publication_winner() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/concurrent.rs");
    let attempt = begin(&output, &producer, 91);
    let bytes = b"concurrent exact hsaco";
    let plan = plan(attempt, scope(0x42), 0x72, bytes);
    let barrier = Arc::new(Barrier::new(3));

    let mut workers = Vec::new();
    for _ in 0..2 {
        let output = output.clone();
        let producer = producer.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            publish_exact(&output, &producer, attempt, plan, bytes)
        }));
    }
    barrier.wait();

    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(AttemptScopedHsacoPublicationErrorV1::ReceiptAlreadyPersisted { .. })
            ))
            .count(),
        1
    );
    finish_build_attempt(&output, &producer, attempt).unwrap();
    let recovered = recover_durable_link_publication_v1(&output, plan.scope())
        .unwrap()
        .unwrap();
    assert_eq!(recovered.artifact().bytes(), bytes);
}

#[test]
fn mismatched_plan_is_rejected_without_consuming_the_attempt() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/plan-mismatch.rs");
    let attempt = begin(&output, &producer, 2);
    let bytes = b"plan mismatch payload";
    let publication_scope = scope(0x11);
    let wrong = plan(fake_attempt(attempt, 99), publication_scope, 0x21, bytes);

    assert!(matches!(
        publish_exact(&output, &producer, attempt, wrong, bytes),
        Err(AttemptScopedHsacoPublicationErrorV1::PlanAttemptMismatch)
    ));

    let correct = plan(attempt, publication_scope, 0x21, bytes);
    publish_exact(&output, &producer, attempt, correct, bytes).unwrap();
    finish_build_attempt(&output, &producer, attempt).unwrap();
}

#[test]
fn wrong_producer_stale_and_failed_attempts_are_refused() {
    let wrong_temp = TestDirectory::new();
    let wrong_output = wrong_temp.output();
    let owner = producer("kernel", "/src/owner.rs");
    let intruder = producer("kernel", "/src/intruder.rs");
    let owner_attempt = begin(&wrong_output, &owner, 3);
    let bytes = b"owner payload";
    let owner_plan = plan(owner_attempt, scope(0x12), 0x22, bytes);
    assert!(matches!(
        publish_exact(&wrong_output, &intruder, owner_attempt, owner_plan, bytes),
        Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
    ));
    publish_exact(&wrong_output, &owner, owner_attempt, owner_plan, bytes).unwrap();
    finish_build_attempt(&wrong_output, &owner, owner_attempt).unwrap();

    let stale_temp = TestDirectory::new();
    let stale_output = stale_temp.output();
    let stale_producer = producer("kernel", "/src/stale.rs");
    let stale = begin(&stale_output, &stale_producer, 4);
    let current = begin(&stale_output, &stale_producer, 5);
    let stale_plan = plan(stale, scope(0x13), 0x23, b"stale");
    assert!(matches!(
        publish_exact(&stale_output, &stale_producer, stale, stale_plan, b"stale"),
        Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
    ));
    let current_plan = plan(current, scope(0x13), 0x24, b"current");
    publish_exact(
        &stale_output,
        &stale_producer,
        current,
        current_plan,
        b"current",
    )
    .unwrap();
    finish_build_attempt(&stale_output, &stale_producer, current).unwrap();

    let failed_temp = TestDirectory::new();
    let failed_output = failed_temp.output();
    let failed_producer = producer("kernel", "/src/failed.rs");
    let failed = begin(&failed_output, &failed_producer, 6);
    fail_build_attempt(&failed_output, &failed_producer, failed).unwrap();
    let failed_plan = plan(failed, scope(0x14), 0x25, b"failed");
    assert!(matches!(
        publish_exact(
            &failed_output,
            &failed_producer,
            failed,
            failed_plan,
            b"failed"
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
    ));
}

#[test]
fn failed_new_generation_preserves_previous_current_publication() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/rollback.rs");
    let publication_scope = scope(0x15);
    let first_bytes = b"first exact hsaco";
    let first = begin(&output, &producer, 7);
    let first_plan = plan(first, publication_scope, 0x30, first_bytes);
    publish_exact(&output, &producer, first, first_plan, first_bytes).unwrap();
    finish_build_attempt(&output, &producer, first).unwrap();

    let second = begin(&output, &producer, 8);
    let expected = b"second exact hsaco";
    let corrupted = b"mutated after plan binding";
    let second_plan = plan(second, publication_scope, 0x31, expected);
    assert!(matches!(
        publish_exact(&output, &producer, second, second_plan, corrupted),
        Err(AttemptScopedHsacoPublicationErrorV1::Durable(
            DurableLinkPublicationError::FinalizedArtifactDigestMismatch
        ))
    ));
    assert!(matches!(
        publish_exact(&output, &producer, second, second_plan, expected),
        Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
    ));

    let recovered = recover_durable_link_publication_v1(&output, publication_scope)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record().attempt(), first);
    assert_eq!(recovered.artifact().bytes(), first_bytes);
}

#[test]
fn exact_plan_recovers_every_journal_stage_after_a_crash() {
    let stages = [
        DurableJournalStageV1::Planned,
        DurableJournalStageV1::WorkerPinned,
        DurableJournalStageV1::ResponseValidated,
        DurableJournalStageV1::Finalized,
        DurableJournalStageV1::Published,
    ];
    for (index, stage) in stages.into_iter().enumerate() {
        let temp = TestDirectory::new();
        let output = temp.output();
        let producer = producer("kernel", &format!("/src/journal-{index}.rs"));
        let attempt = begin(&output, &producer, 20 + index as u8);
        let bytes = format!("journal-stage-{stage:?}").into_bytes();
        let plan = plan(attempt, scope(0x20 + index as u8), 0x40, &bytes);
        let point = DurableLinkPublicationFaultPointV1::Journal {
            stage,
            boundary: DurableJournalBoundaryV1::SyncCanonicalName,
            timing: DurableFaultTimingV1::After,
        };

        assert!(matches!(
            publish_exact_with_options(
                &output,
                &producer,
                attempt,
                plan,
                &bytes,
                DurableLinkPublicationOptionsV1::inject_crash(point),
            ),
            Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(
                DurableLinkPublicationError::InjectedCrash { point: actual }
            )) if actual == point
        ));
        assert!(matches!(
            read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap(),
            PersistedBackendReceiptV1::PendingProvenance(_)
        ));
        let recovered = publish_exact(&output, &producer, attempt, plan, &bytes).unwrap();
        assert_eq!(
            recovered.outcome(),
            if stage == DurableJournalStageV1::Published {
                AttemptScopedHsacoPublicationOutcomeV1::RecoveredCommittedPublication
            } else {
                AttemptScopedHsacoPublicationOutcomeV1::RecoveredAndPublished
            }
        );
        assert_eq!(recovered.snapshot().artifact().bytes(), bytes);
        assert_eq!(
            read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap(),
            PersistedBackendReceiptV1::Provenance(recovered.receipt())
        );
        finish_build_attempt(&output, &producer, attempt).unwrap();
    }
}

#[test]
fn crash_after_publication_before_receipt_is_reconciled_by_exact_retry() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/post-publication-receipt.rs");
    let attempt = begin(&output, &producer, 35);
    let bytes = b"published before receipt";
    let plan = plan(attempt, scope(0x2f), 0x4f, bytes);
    let point = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Published,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };
    assert!(matches!(
        publish_exact_with_options(
            &output,
            &producer,
            attempt,
            plan,
            bytes,
            DurableLinkPublicationOptionsV1::inject_crash(point),
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(_))
    ));
    assert!(matches!(
        read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap(),
        PersistedBackendReceiptV1::PendingProvenance(_)
    ));

    let reconciled = publish_exact(&output, &producer, attempt, plan, bytes).unwrap();
    assert_eq!(
        reconciled.outcome(),
        AttemptScopedHsacoPublicationOutcomeV1::RecoveredCommittedPublication
    );
    assert_eq!(
        reconciled.durable_outcome(),
        fe2o3_artifact_transaction::DurableLinkPublicationOutcomeV1::AlreadyPublished
    );
    assert_eq!(
        read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap(),
        PersistedBackendReceiptV1::Provenance(reconciled.receipt())
    );
    finish_build_attempt(&output, &producer, attempt).unwrap();
}

#[test]
fn exact_plan_recovers_every_artifact_boundary_after_a_crash() {
    let boundaries = [
        DurableArtifactBoundaryV1::CreateTemp,
        DurableArtifactBoundaryV1::WriteTemp,
        DurableArtifactBoundaryV1::SyncTemp,
        DurableArtifactBoundaryV1::RenameToContentAddress,
        DurableArtifactBoundaryV1::SyncDirectory,
    ];
    let timings = [DurableFaultTimingV1::Before, DurableFaultTimingV1::After];
    for (index, (boundary, timing)) in boundaries
        .into_iter()
        .flat_map(|boundary| timings.into_iter().map(move |timing| (boundary, timing)))
        .enumerate()
    {
        let temp = TestDirectory::new();
        let output = temp.output();
        let producer = producer("kernel", &format!("/src/artifact-{index}.rs"));
        let attempt = begin(&output, &producer, 40 + index as u8);
        let bytes = format!("artifact-boundary-{boundary:?}-{timing:?}").into_bytes();
        let plan = plan(attempt, scope(0x30 + index as u8), 0x50, &bytes);
        let point = DurableLinkPublicationFaultPointV1::Artifact { boundary, timing };

        assert!(matches!(
            publish_exact_with_options(
                &output,
                &producer,
                attempt,
                plan,
                &bytes,
                DurableLinkPublicationOptionsV1::inject_crash(point),
            ),
            Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(
                DurableLinkPublicationError::InjectedCrash { point: actual }
            )) if actual == point
        ));
        let recovered = publish_exact(&output, &producer, attempt, plan, &bytes).unwrap();
        assert_eq!(
            recovered.outcome(),
            AttemptScopedHsacoPublicationOutcomeV1::RecoveredAndPublished
        );
        assert_eq!(recovered.snapshot().artifact().bytes(), bytes);
        finish_build_attempt(&output, &producer, attempt).unwrap();
    }
}

#[test]
fn changed_upstream_evidence_cannot_rebind_a_pending_receipt() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/evidence-recovery.rs");
    let attempt = begin(&output, &producer, 69);
    let bytes = b"upstream-evidence-bound bytes";
    let plan = plan(attempt, scope(0x3f), 0x5f, bytes);
    let original_evidence = upstream_evidence(plan);
    let point = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };
    assert!(matches!(
        publish_exact_hsaco_evidence_for_attempt_v1_with_options(
            &output,
            &producer,
            attempt,
            plan,
            original_evidence,
            bytes,
            DurableLinkPublicationOptionsV1::inject_crash(point),
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(_))
    ));
    let pending = match read_backend_publication_receipt_v1(&output, &producer, attempt).unwrap() {
        PersistedBackendReceiptV1::PendingProvenance(receipt) => receipt,
        state => panic!("expected pending receipt, got {state:?}"),
    };
    assert_eq!(
        pending.upstream_evidence_identity(),
        original_evidence.as_bytes()
    );

    let changed_evidence = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0xee; 32]);
    assert!(matches!(
        publish_exact_hsaco_evidence_for_attempt_v1(
            &output,
            &producer,
            attempt,
            plan,
            changed_evidence,
            bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::PendingReceiptMismatch)
    ));
    assert!(matches!(
        publish_exact_hsaco_evidence_for_attempt_v1(
            &output,
            &producer,
            attempt,
            plan,
            original_evidence,
            bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
    ));
}

#[test]
fn changed_plan_cannot_reuse_crash_consumed_authority() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/changed-recovery.rs");
    let attempt = begin(&output, &producer, 70);
    let bytes = b"crash-bound exact plan";
    let publication_scope = scope(0x40);
    let original = plan(attempt, publication_scope, 0x60, bytes);
    let point = DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::SyncCanonicalName,
        timing: DurableFaultTimingV1::After,
    };
    assert!(matches!(
        publish_exact_with_options(
            &output,
            &producer,
            attempt,
            original,
            bytes,
            DurableLinkPublicationOptionsV1::inject_crash(point),
        ),
        Err(
            AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(
                DurableLinkPublicationError::InjectedCrash { .. }
            )
        )
    ));

    let changed = plan(attempt, publication_scope, 0x61, bytes);
    assert!(matches!(
        publish_exact(&output, &producer, attempt, changed, bytes),
        Err(AttemptScopedHsacoPublicationErrorV1::PendingReceiptMismatch)
    ));
    assert!(matches!(
        publish_exact(&output, &producer, attempt, original, bytes),
        Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
    ));
}

#[test]
fn prior_lease_becomes_stale_after_newer_generation_commits() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let producer = producer("kernel", "/src/currentness.rs");
    let publication_scope = scope(0x41);
    let first = begin(&output, &producer, 80);
    let first_bytes = b"lease generation one";
    let first_plan = plan(first, publication_scope, 0x70, first_bytes);
    let first_result = publish_exact(&output, &producer, first, first_plan, first_bytes).unwrap();
    let first_lease = first_result.into_current_lease();
    drop(first_lease.acquire_current_token().unwrap());
    finish_build_attempt(&output, &producer, first).unwrap();

    let second = begin(&output, &producer, 81);
    let second_bytes = b"lease generation two";
    let second_plan = plan(second, publication_scope, 0x71, second_bytes);
    publish_exact(&output, &producer, second, second_plan, second_bytes).unwrap();
    finish_build_attempt(&output, &producer, second).unwrap();

    assert!(matches!(
        first_lease.acquire_current_token(),
        Err(DurableLinkPublicationError::CurrentPublication { .. })
    ));
    let recovered = recover_durable_link_publication_v1(&output, publication_scope)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record().attempt(), second);
    assert_eq!(recovered.artifact().bytes(), second_bytes);
}

#[test]
fn unrelated_global_attempts_may_skip_generations_within_one_scope() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("kernel", "/src/scope-owner.rs");
    let publication_scope = scope(0x45);
    let first = begin(&output, &owner, 90);
    let first_bytes = b"scope generation one";
    let first_plan = plan(first, publication_scope, 0x75, first_bytes);
    publish_exact(&output, &owner, first, first_plan, first_bytes).unwrap();
    finish_build_attempt(&output, &owner, first).unwrap();

    for (seed, source) in [(91, "/src/unrelated-a.rs"), (92, "/src/unrelated-b.rs")] {
        let unrelated = producer("unrelated", source);
        let attempt = begin(&output, &unrelated, seed);
        fail_build_attempt(&output, &unrelated, attempt).unwrap();
    }

    let later = begin(&output, &owner, 93);
    assert!(later.generation() > first.generation() + 1);
    let later_bytes = b"scope generation after global gap";
    let later_plan = plan(later, publication_scope, 0x76, later_bytes);
    let result = publish_exact(&output, &owner, later, later_plan, later_bytes).unwrap();
    assert_eq!(
        result.outcome(),
        AttemptScopedHsacoPublicationOutcomeV1::Published
    );
    finish_build_attempt(&output, &owner, later).unwrap();

    let recovered = recover_durable_link_publication_v1(&output, publication_scope)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.record().attempt(), later);
    assert_eq!(recovered.artifact().bytes(), later_bytes);
}
