use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationBoundaryV2,
    AttemptScopedHsacoPublicationErrorV1, AttemptScopedHsacoPublicationErrorV2,
    AttemptScopedHsacoPublicationErrorV3, AttemptScopedHsacoPublicationFaultPointV2,
    AttemptScopedHsacoPublicationFaultTimingV2, AttemptScopedHsacoPublicationOptionsV2,
    AttemptScopedHsacoPublicationOutcomeV3, BackendPublicationReceiptValidationErrorV3,
    BuildAttempt, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
    DurableJournalBoundaryV1, DurableJournalStageV1, DurableLinkPublicationError,
    DurableLinkPublicationFaultPointV1, DurableLinkPublicationOptionsV1,
    DurableLinkPublicationPlanV1, DurablePublishedClaimReacquisitionErrorV3,
    DurablePublishedHsacoClaimV3, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PersistedBackendReceiptV3, PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    VerifiedWorkerV3PublicationAuthorityV1, WorkerV3FinalizerReplayAttachmentsV1,
    WorkerV3PublicationBindingV1, begin_build_attempt, finish_build_attempt,
    persist_worker_v3_publication_intent_v1, publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v1_with_options,
    publish_exact_hsaco_evidence_for_attempt_v2,
    publish_exact_hsaco_evidence_for_attempt_v2_with_options,
    publish_exact_hsaco_evidence_for_attempt_v3,
    publish_exact_hsaco_evidence_for_attempt_v3_with_options,
    reacquire_current_hsaco_publication_lease_v3, read_backend_publication_receipt_v1,
    read_backend_publication_receipt_v2, read_backend_publication_receipt_v3,
    recover_published_hsaco_claim_for_attempt_v1, recover_published_hsaco_claim_for_attempt_v2,
    recover_published_hsaco_claim_for_attempt_v3, validate_backend_publication_receipt_v3,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const ATTEMPT_REGISTRY: &str = ".fe2o3-attempts-v1";

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-attempt-hsaco-v3-{}-{}",
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
    assert_ne!(seed, 0);
    [seed; 32]
}

fn producer(crate_name: &str, source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen(crate_name, Some(Path::new(source))).unwrap()
}

fn begin(output: &Path, owner: &ProducerIdentity, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        output,
        owner,
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32]),
        BuildSession::from_bytes([seed; 16]),
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
        LinkedOutputIdentityV1::from_bytes(Sha256::digest(bytes).into()),
        FinalizationIdentityV1::from_bytes(identity(seed.wrapping_add(4))),
        FinalizedOutputIdentityV1::from_bytes(Sha256::digest(bytes).into()),
        AtomicPublicationIdentityV1::from_bytes(identity(seed.wrapping_add(5))),
    )
}

fn upstream(plan: DurableLinkPublicationPlanV1) -> UpstreamCodeObjectEvidenceIdentityV1 {
    UpstreamCodeObjectEvidenceIdentityV1::from_bytes(
        Sha256::digest(plan.finalization().as_bytes()).into(),
    )
}

fn compiler_closure(seed: u8) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        identity(seed),
        identity(seed.wrapping_add(1)),
        identity(seed.wrapping_add(2)),
        identity(seed.wrapping_add(3)),
        identity(seed.wrapping_add(4)),
        identity(seed.wrapping_add(5)),
    )
    .unwrap()
}

fn binding_for_intent_identity(
    seed: u8,
    bytes: &[u8],
    publication_intent_record_identity: [u8; 32],
) -> WorkerV3PublicationBindingV1 {
    WorkerV3PublicationBindingV1::new(
        compiler_closure(seed),
        publication_intent_record_identity,
        identity(seed.wrapping_add(0x11)),
        identity(seed.wrapping_add(0x12)),
        identity(seed.wrapping_add(0x13)),
        identity(seed.wrapping_add(0x14)),
        Sha256::digest(bytes).into(),
        bytes.len() as u64,
        Sha256::digest(bytes).into(),
        bytes.len() as u64,
    )
    .unwrap()
}

fn persist_intent_and_binding(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    seed: u8,
    bytes: &[u8],
) -> WorkerV3PublicationBindingV1 {
    let replay = WorkerV3FinalizerReplayAttachmentsV1::new(
        format!("minimal outer handoff {seed}").into_bytes(),
        Vec::new(),
        format!("minimal replay transcript {seed}").into_bytes(),
    )
    .unwrap();
    let persisted = persist_worker_v3_publication_intent_v1(
        output,
        owner,
        attempt,
        publication_plan,
        replay,
        bytes.to_vec(),
    )
    .unwrap();
    assert_eq!(persisted.record().plan(), publication_plan);
    assert_eq!(persisted.exact_output(), bytes);
    binding_for_intent_identity(seed, bytes, persisted.record().identity().as_bytes())
}

fn missing_intent_binding(seed: u8, bytes: &[u8]) -> WorkerV3PublicationBindingV1 {
    binding_for_intent_identity(seed, bytes, identity(seed.wrapping_add(0x10)))
}

fn substitute_intent_identity(
    original: WorkerV3PublicationBindingV1,
    identity_seed: u8,
) -> WorkerV3PublicationBindingV1 {
    WorkerV3PublicationBindingV1::new(
        original.compiler_closure(),
        identity(identity_seed),
        original.finalization_identity(),
        original.source_evidence_identity(),
        original.compiler_handoff_binding_identity(),
        original.raw_inspection_identity(),
        original.raw_output_sha256(),
        original.raw_output_length(),
        original.finalized_output_sha256(),
        original.finalized_output_length(),
    )
    .unwrap()
}

fn substitute_finalized_length(
    original: WorkerV3PublicationBindingV1,
    finalized_output_length: u64,
) -> WorkerV3PublicationBindingV1 {
    WorkerV3PublicationBindingV1::new(
        original.compiler_closure(),
        original.publication_intent_record_identity(),
        original.finalization_identity(),
        original.source_evidence_identity(),
        original.compiler_handoff_binding_identity(),
        original.raw_inspection_identity(),
        original.raw_output_sha256(),
        original.raw_output_length(),
        original.finalized_output_sha256(),
        finalized_output_length,
    )
    .unwrap()
}

fn publish_v3(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    publication_binding: WorkerV3PublicationBindingV1,
    bytes: &[u8],
) -> Result<
    fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV3,
    AttemptScopedHsacoPublicationErrorV3,
> {
    // SAFETY: these transaction tests deliberately exercise the low-level boundary after
    // persisting exact restart storage and never promote the result to load authority.
    let authority = unsafe {
        VerifiedWorkerV3PublicationAuthorityV1::from_authenticated_finalizer_replay_unchecked(
            publication_binding,
        )
    };
    publish_exact_hsaco_evidence_for_attempt_v3(
        output,
        owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        authority,
        bytes,
    )
}

fn publish_v3_with_options(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    publication_binding: WorkerV3PublicationBindingV1,
    bytes: &[u8],
    options: AttemptScopedHsacoPublicationOptionsV2,
) -> Result<
    fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV3,
    AttemptScopedHsacoPublicationErrorV3,
> {
    // SAFETY: see `publish_v3`; this variant adds only deterministic transaction fault injection.
    let authority = unsafe {
        VerifiedWorkerV3PublicationAuthorityV1::from_authenticated_finalizer_replay_unchecked(
            publication_binding,
        )
    };
    publish_exact_hsaco_evidence_for_attempt_v3_with_options(
        output,
        owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        authority,
        bytes,
        options,
    )
}

fn before_planned_redo() -> DurableLinkPublicationFaultPointV1 {
    DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::CreateRedoTemp,
        timing: fe2o3_artifact_transaction::DurableFaultTimingV1::Before,
    }
}

fn interrupt_v3_with_pending_receipt(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    publication_binding: WorkerV3PublicationBindingV1,
    bytes: &[u8],
) {
    let point = before_planned_redo();
    assert!(matches!(
        publish_v3_with_options(
            output,
            owner,
            attempt,
            publication_plan,
            publication_binding,
            bytes,
            AttemptScopedHsacoPublicationOptionsV2::inject_durable_crash(point),
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::PublicationInterrupted(
            DurableLinkPublicationError::InjectedCrash { point: actual }
        )) if actual == point
    ));
}

fn registry_bytes(output: &Path) -> Vec<u8> {
    fs::read(output.join(ATTEMPT_REGISTRY)).unwrap()
}

fn pending_v3_fixture(
    case: &str,
    seed: u8,
) -> (
    TestDirectory,
    PathBuf,
    ProducerIdentity,
    BuildAttempt,
    Vec<u8>,
    DurableLinkPublicationPlanV1,
    WorkerV3PublicationBindingV1,
) {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("pending_v3", &format!("/src/v3-pending-{case}.rs"));
    let attempt = begin(&output, &owner, seed);
    let bytes = format!("strict Worker V3 pending {case} substitution").into_bytes();
    let publication_plan = plan(
        attempt,
        scope(seed.wrapping_add(1)),
        seed.wrapping_add(0x10),
        &bytes,
    );
    let publication_binding = persist_intent_and_binding(
        &output,
        &owner,
        attempt,
        publication_plan,
        seed.wrapping_add(0x20),
        &bytes,
    );
    interrupt_v3_with_pending_receipt(
        &output,
        &owner,
        attempt,
        publication_plan,
        publication_binding,
        &bytes,
    );
    (
        temp,
        output,
        owner,
        attempt,
        bytes,
        publication_plan,
        publication_binding,
    )
}

#[test]
fn fresh_publish_read_claim_recover_and_reacquire_round_trip() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("fresh_v3", "/src/v3-fresh.rs");
    let attempt = begin(&output, &owner, 1);
    let bytes = b"strict Worker V3 finalized gfx942 HSACO";
    let publication_plan = plan(attempt, scope(0x10), 0x20, bytes);
    let publication_binding =
        persist_intent_and_binding(&output, &owner, attempt, publication_plan, 0x30, bytes);

    let result = publish_v3(
        &output,
        &owner,
        attempt,
        publication_plan,
        publication_binding,
        bytes,
    )
    .unwrap();
    assert_eq!(
        result.outcome(),
        AttemptScopedHsacoPublicationOutcomeV3::Published
    );
    assert_eq!(result.snapshot().artifact().bytes(), bytes);
    assert_eq!(result.publication_binding(), publication_binding);
    assert!(matches!(
        read_backend_publication_receipt_v3(&output, &owner, attempt).unwrap(),
        PersistedBackendReceiptV3::Provenance(receipt) if receipt == result.receipt()
    ));

    let claim = result.published_claim().clone();
    assert_eq!(claim.plan(), publication_plan);
    assert_eq!(claim.upstream_evidence(), upstream(publication_plan));
    assert_eq!(claim.worker_v3_binding(), publication_binding);
    let encoded = claim.encode_canonical().unwrap();
    assert_eq!(
        DurablePublishedHsacoClaimV3::decode_canonical(&encoded).unwrap(),
        claim
    );
    let recovered = recover_published_hsaco_claim_for_attempt_v3(
        &output,
        &owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        publication_binding,
        result.receipt(),
    )
    .unwrap();
    assert_eq!(recovered, claim);

    drop(result);
    finish_build_attempt(&output, &owner, attempt).unwrap();
    let reconstructed = publish_v3(
        &output,
        &owner,
        attempt,
        publication_plan,
        publication_binding,
        bytes,
    )
    .unwrap();
    assert_eq!(
        reconstructed.outcome(),
        AttemptScopedHsacoPublicationOutcomeV3::RecoveredCommittedPublication
    );
    assert_eq!(reconstructed.snapshot().artifact().bytes(), bytes);
    drop(reconstructed);
    let lease = reacquire_current_hsaco_publication_lease_v3(&output, &recovered).unwrap();
    assert_eq!(lease.published().attempt(), attempt);
    assert_eq!(lease.exact_artifact_bytes(), bytes);
    let token = lease.acquire_current_token().unwrap();
    token.revalidate_locked_currentness().unwrap();
    assert_eq!(token.exact_artifact_bytes(), bytes);
}

#[test]
fn exact_retry_reconciles_pending_and_final_receipt_commit_boundaries() {
    let boundaries = [
        AttemptScopedHsacoPublicationBoundaryV2::CommitPendingReceipt,
        AttemptScopedHsacoPublicationBoundaryV2::CommitFinalReceipt,
    ];
    let timings = [
        AttemptScopedHsacoPublicationFaultTimingV2::Before,
        AttemptScopedHsacoPublicationFaultTimingV2::After,
    ];

    let mut case = 0_u8;
    for boundary in boundaries {
        for timing in timings {
            let temp = TestDirectory::new();
            let output = temp.output();
            let owner = producer("receipt_v3", &format!("/src/v3-receipt-{case}.rs"));
            let attempt = begin(&output, &owner, case.wrapping_add(0x20));
            let bytes = format!("v3-receipt-{boundary:?}-{timing:?}").into_bytes();
            let publication_plan = plan(attempt, scope(case.wrapping_add(0x30)), 0x60, &bytes);
            let publication_binding = persist_intent_and_binding(
                &output,
                &owner,
                attempt,
                publication_plan,
                case.wrapping_add(0x70),
                &bytes,
            );
            let point = AttemptScopedHsacoPublicationFaultPointV2 { boundary, timing };

            assert!(matches!(
                publish_v3_with_options(
                    &output,
                    &owner,
                    attempt,
                    publication_plan,
                    publication_binding,
                    &bytes,
                    AttemptScopedHsacoPublicationOptionsV2::inject_receipt_crash(point),
                ),
                Err(AttemptScopedHsacoPublicationErrorV3::ReceiptCommitInterrupted {
                    point: actual
                }) if actual == point
            ));

            let recovered = publish_v3(
                &output,
                &owner,
                attempt,
                publication_plan,
                publication_binding,
                &bytes,
            )
            .unwrap();
            if boundary == AttemptScopedHsacoPublicationBoundaryV2::CommitFinalReceipt
                && timing == AttemptScopedHsacoPublicationFaultTimingV2::After
            {
                assert_eq!(
                    recovered.outcome(),
                    AttemptScopedHsacoPublicationOutcomeV3::RecoveredCommittedPublication
                );
            } else {
                assert!(matches!(
                    recovered.outcome(),
                    AttemptScopedHsacoPublicationOutcomeV3::Published
                        | AttemptScopedHsacoPublicationOutcomeV3::RecoveredAndPublished
                        | AttemptScopedHsacoPublicationOutcomeV3::RecoveredCommittedPublication
                ));
            }
            assert_eq!(recovered.snapshot().artifact().bytes(), bytes);
            assert_eq!(recovered.publication_binding(), publication_binding);
            assert!(matches!(
                read_backend_publication_receipt_v3(&output, &owner, attempt).unwrap(),
                PersistedBackendReceiptV3::Provenance(receipt)
                    if receipt.publication_binding() == publication_binding
            ));
            finish_build_attempt(&output, &owner, attempt).unwrap();
            case = case.wrapping_add(1);
        }
    }
    assert_eq!(case, 4);
}

#[test]
fn v1_and_v2_pending_and_final_receipts_never_enter_v3_apis() {
    for final_receipt in [false, true] {
        let v1_temp = TestDirectory::new();
        let v1_output = v1_temp.output();
        let v1_owner = producer("cross_v1", "/src/v1-into-v3.rs");
        let seed = if final_receipt { 0x42 } else { 0x41 };
        let v1_attempt = begin(&v1_output, &v1_owner, seed);
        let v1_bytes = b"V1 state presented to strict Worker V3";
        let v1_plan = plan(v1_attempt, scope(0x43), 0x44, v1_bytes);
        let v1_binding =
            persist_intent_and_binding(&v1_output, &v1_owner, v1_attempt, v1_plan, 0x50, v1_bytes);
        if final_receipt {
            drop(
                publish_exact_hsaco_evidence_for_attempt_v1(
                    &v1_output,
                    &v1_owner,
                    v1_attempt,
                    v1_plan,
                    upstream(v1_plan),
                    v1_bytes,
                )
                .unwrap(),
            );
        } else {
            let point = before_planned_redo();
            assert!(matches!(
                publish_exact_hsaco_evidence_for_attempt_v1_with_options(
                    &v1_output,
                    &v1_owner,
                    v1_attempt,
                    v1_plan,
                    upstream(v1_plan),
                    v1_bytes,
                    DurableLinkPublicationOptionsV1::inject_crash(point),
                ),
                Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(
                    DurableLinkPublicationError::InjectedCrash { point: actual }
                )) if actual == point
            ));
        }
        let v1_registry = registry_bytes(&v1_output);
        assert!(matches!(
            read_backend_publication_receipt_v3(&v1_output, &v1_owner, v1_attempt),
            Err(AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&v1_output), v1_registry);
        assert!(matches!(
            publish_v3(
                &v1_output, &v1_owner, v1_attempt, v1_plan, v1_binding, v1_bytes,
            ),
            Err(AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&v1_output), v1_registry);

        let v1_donor = TestDirectory::new();
        let v1_donor_attempt = begin(&v1_donor.output(), &v1_owner, seed);
        assert_eq!(v1_donor_attempt, v1_attempt);
        let v1_donor_binding = persist_intent_and_binding(
            &v1_donor.output(),
            &v1_owner,
            v1_donor_attempt,
            v1_plan,
            0x50,
            v1_bytes,
        );
        assert_eq!(v1_donor_binding, v1_binding);
        let v3_receipt = publish_v3(
            &v1_donor.output(),
            &v1_owner,
            v1_donor_attempt,
            v1_plan,
            v1_donor_binding,
            v1_bytes,
        )
        .unwrap()
        .receipt();
        assert!(matches!(
            recover_published_hsaco_claim_for_attempt_v3(
                &v1_output,
                &v1_owner,
                v1_attempt,
                v1_plan,
                upstream(v1_plan),
                v1_binding,
                v3_receipt,
            ),
            Err(AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&v1_output), v1_registry);

        let v2_temp = TestDirectory::new();
        let v2_output = v2_temp.output();
        let v2_owner = producer("cross_v2", "/src/v2-into-v3.rs");
        let v2_attempt = begin(&v2_output, &v2_owner, seed);
        let v2_bytes = b"V2 state presented to strict Worker V3";
        let v2_plan = plan(v2_attempt, scope(0x53), 0x54, v2_bytes);
        let closure = compiler_closure(0x60);
        let v2_binding =
            persist_intent_and_binding(&v2_output, &v2_owner, v2_attempt, v2_plan, 0x70, v2_bytes);
        if final_receipt {
            drop(
                publish_exact_hsaco_evidence_for_attempt_v2(
                    &v2_output,
                    &v2_owner,
                    v2_attempt,
                    v2_plan,
                    upstream(v2_plan),
                    closure,
                    v2_bytes,
                )
                .unwrap(),
            );
        } else {
            let point = before_planned_redo();
            assert!(matches!(
                publish_exact_hsaco_evidence_for_attempt_v2_with_options(
                    &v2_output,
                    &v2_owner,
                    v2_attempt,
                    v2_plan,
                    upstream(v2_plan),
                    closure,
                    v2_bytes,
                    AttemptScopedHsacoPublicationOptionsV2::inject_durable_crash(point),
                ),
                Err(AttemptScopedHsacoPublicationErrorV2::PublicationInterrupted(
                    DurableLinkPublicationError::InjectedCrash { point: actual }
                )) if actual == point
            ));
        }
        let v2_registry = registry_bytes(&v2_output);
        assert!(matches!(
            read_backend_publication_receipt_v3(&v2_output, &v2_owner, v2_attempt),
            Err(AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&v2_output), v2_registry);
        assert!(matches!(
            publish_v3(
                &v2_output, &v2_owner, v2_attempt, v2_plan, v2_binding, v2_bytes,
            ),
            Err(AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&v2_output), v2_registry);

        let v2_donor = TestDirectory::new();
        let v2_donor_attempt = begin(&v2_donor.output(), &v2_owner, seed);
        assert_eq!(v2_donor_attempt, v2_attempt);
        let v2_donor_binding = persist_intent_and_binding(
            &v2_donor.output(),
            &v2_owner,
            v2_donor_attempt,
            v2_plan,
            0x70,
            v2_bytes,
        );
        assert_eq!(v2_donor_binding, v2_binding);
        let v3_receipt = publish_v3(
            &v2_donor.output(),
            &v2_owner,
            v2_donor_attempt,
            v2_plan,
            v2_donor_binding,
            v2_bytes,
        )
        .unwrap()
        .receipt();
        assert!(matches!(
            recover_published_hsaco_claim_for_attempt_v3(
                &v2_output,
                &v2_owner,
                v2_attempt,
                v2_plan,
                upstream(v2_plan),
                v2_binding,
                v3_receipt,
            ),
            Err(AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&v2_output), v2_registry);
    }
}

#[test]
fn v3_pending_and_final_receipts_never_enter_v1_or_v2_apis() {
    for final_receipt in [false, true] {
        let temp = TestDirectory::new();
        let output = temp.output();
        let owner = producer("cross_v3", "/src/v3-into-older-schemas.rs");
        let seed = if final_receipt { 0x82 } else { 0x81 };
        let attempt = begin(&output, &owner, seed);
        let bytes = b"strict Worker V3 state presented to V1 and V2";
        let publication_plan = plan(attempt, scope(0x83), 0x84, bytes);
        let publication_binding =
            persist_intent_and_binding(&output, &owner, attempt, publication_plan, 0x90, bytes);
        if final_receipt {
            drop(
                publish_v3(
                    &output,
                    &owner,
                    attempt,
                    publication_plan,
                    publication_binding,
                    bytes,
                )
                .unwrap(),
            );
        } else {
            interrupt_v3_with_pending_receipt(
                &output,
                &owner,
                attempt,
                publication_plan,
                publication_binding,
                bytes,
            );
        }
        let registry = registry_bytes(&output);

        assert!(read_backend_publication_receipt_v1(&output, &owner, attempt).is_err());
        assert_eq!(registry_bytes(&output), registry);
        assert!(matches!(
            publish_exact_hsaco_evidence_for_attempt_v1(
                &output,
                &owner,
                attempt,
                publication_plan,
                upstream(publication_plan),
                bytes,
            ),
            Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
        ));
        assert_eq!(registry_bytes(&output), registry);

        let v1_donor = TestDirectory::new();
        let v1_donor_attempt = begin(&v1_donor.output(), &owner, seed);
        assert_eq!(v1_donor_attempt, attempt);
        let v1_receipt = publish_exact_hsaco_evidence_for_attempt_v1(
            &v1_donor.output(),
            &owner,
            v1_donor_attempt,
            publication_plan,
            upstream(publication_plan),
            bytes,
        )
        .unwrap()
        .receipt();
        assert!(
            recover_published_hsaco_claim_for_attempt_v1(
                &output,
                &owner,
                attempt,
                publication_plan,
                upstream(publication_plan),
                v1_receipt,
            )
            .is_err()
        );
        assert_eq!(registry_bytes(&output), registry);

        let closure = compiler_closure(0xa0);
        assert!(matches!(
            read_backend_publication_receipt_v2(&output, &owner, attempt),
            Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&output), registry);
        assert!(matches!(
            publish_exact_hsaco_evidence_for_attempt_v2(
                &output,
                &owner,
                attempt,
                publication_plan,
                upstream(publication_plan),
                closure,
                bytes,
            ),
            Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&output), registry);

        let v2_donor = TestDirectory::new();
        let v2_donor_attempt = begin(&v2_donor.output(), &owner, seed);
        assert_eq!(v2_donor_attempt, attempt);
        let v2_receipt = publish_exact_hsaco_evidence_for_attempt_v2(
            &v2_donor.output(),
            &owner,
            v2_donor_attempt,
            publication_plan,
            upstream(publication_plan),
            closure,
            bytes,
        )
        .unwrap()
        .receipt();
        assert!(matches!(
            recover_published_hsaco_claim_for_attempt_v2(
                &output,
                &owner,
                attempt,
                publication_plan,
                upstream(publication_plan),
                closure,
                v2_receipt,
            ),
            Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
        ));
        assert_eq!(registry_bytes(&output), registry);
    }
}

#[test]
fn completed_binding_plan_and_producer_substitutions_fail_without_registry_mutation() {
    let final_temp = TestDirectory::new();
    let final_output = final_temp.output();
    let owner = producer("substitution_v3", "/src/v3-substitution-owner.rs");
    let attempt = begin(&final_output, &owner, 0xb1);
    let bytes = b"strict Worker V3 substitution target";
    let publication_plan = plan(attempt, scope(0xb2), 0xb3, bytes);
    let original_binding = persist_intent_and_binding(
        &final_output,
        &owner,
        attempt,
        publication_plan,
        0xc0,
        bytes,
    );
    let receipt = publish_v3(
        &final_output,
        &owner,
        attempt,
        publication_plan,
        original_binding,
        bytes,
    )
    .unwrap()
    .receipt();
    let registry = registry_bytes(&final_output);

    let substituted_binding = substitute_intent_identity(original_binding, 0xd8);
    assert_eq!(
        validate_backend_publication_receipt_v3(
            &owner,
            attempt,
            publication_plan,
            upstream(publication_plan),
            substituted_binding,
            receipt,
        ),
        Err(BackendPublicationReceiptValidationErrorV3::PublicationBindingMismatch)
    );
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v3(
            &final_output,
            &owner,
            attempt,
            publication_plan,
            upstream(publication_plan),
            substituted_binding,
            receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::PublicationBindingMismatch)
    ));
    assert_eq!(registry_bytes(&final_output), registry);

    let substituted_plan = plan(attempt, publication_plan.scope(), 0xb4, bytes);
    assert_eq!(
        validate_backend_publication_receipt_v3(
            &owner,
            attempt,
            substituted_plan,
            upstream(publication_plan),
            original_binding,
            receipt,
        ),
        Err(BackendPublicationReceiptValidationErrorV3::PlanCommitmentMismatch)
    );
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v3(
            &final_output,
            &owner,
            attempt,
            substituted_plan,
            upstream(publication_plan),
            original_binding,
            receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::ReceiptPublicationMismatch)
    ));
    assert_eq!(registry_bytes(&final_output), registry);

    let intruder = producer("substitution_v3", "/src/v3-substitution-intruder.rs");
    assert_eq!(
        validate_backend_publication_receipt_v3(
            &intruder,
            attempt,
            publication_plan,
            upstream(publication_plan),
            original_binding,
            receipt,
        ),
        Err(BackendPublicationReceiptValidationErrorV3::ProducerIdentityMismatch)
    );
    assert!(matches!(
        recover_published_hsaco_claim_for_attempt_v3(
            &final_output,
            &intruder,
            attempt,
            publication_plan,
            upstream(publication_plan),
            original_binding,
            receipt,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::ReceiptPublicationMismatch)
    ));
    assert_eq!(registry_bytes(&final_output), registry);
}

#[test]
fn missing_and_substituted_intents_fail_before_registry_mutation() {
    let missing_temp = TestDirectory::new();
    let missing_output = missing_temp.output();
    let missing_owner = producer("missing_intent_v3", "/src/v3-missing-intent.rs");
    let missing_attempt = begin(&missing_output, &missing_owner, 0xb8);
    let missing_bytes = b"strict Worker V3 missing intent target";
    let missing_plan = plan(missing_attempt, scope(0xb9), 0xba, missing_bytes);
    let missing_binding = missing_intent_binding(0xc2, missing_bytes);
    let missing_registry = registry_bytes(&missing_output);

    assert!(matches!(
        publish_v3(
            &missing_output,
            &missing_owner,
            missing_attempt,
            missing_plan,
            missing_binding,
            missing_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::PublicationIntent(
            fe2o3_artifact_transaction::WorkerV3PublicationIntentErrorV1::NotFound
        ))
    ));
    assert_eq!(registry_bytes(&missing_output), missing_registry);

    let substituted_temp = TestDirectory::new();
    let substituted_output = substituted_temp.output();
    let substituted_owner = producer("substituted_intent_v3", "/src/v3-substituted-intent.rs");
    let substituted_attempt = begin(&substituted_output, &substituted_owner, 0xbb);
    let substituted_bytes = b"strict Worker V3 substituted intent target";
    let substituted_plan = plan(substituted_attempt, scope(0xbc), 0xbd, substituted_bytes);
    let exact_binding = persist_intent_and_binding(
        &substituted_output,
        &substituted_owner,
        substituted_attempt,
        substituted_plan,
        0xc3,
        substituted_bytes,
    );
    let substituted_binding = substitute_intent_identity(exact_binding, 0xd7);
    let substituted_registry = registry_bytes(&substituted_output);

    assert!(matches!(
        publish_v3(
            &substituted_output,
            &substituted_owner,
            substituted_attempt,
            substituted_plan,
            substituted_binding,
            substituted_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::PublicationBindingMismatch)
    ));
    assert_eq!(registry_bytes(&substituted_output), substituted_registry);
}

#[test]
fn finalized_length_substitution_fails_before_registry_mutation() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("length_substitution_v3", "/src/v3-length-substitution.rs");
    let attempt = begin(&output, &owner, 0xb5);
    let bytes = b"strict Worker V3 finalized length target";
    let publication_plan = plan(attempt, scope(0xb6), 0xb7, bytes);
    let original_binding =
        persist_intent_and_binding(&output, &owner, attempt, publication_plan, 0xc1, bytes);
    let substituted_binding = substitute_finalized_length(
        original_binding,
        original_binding.finalized_output_length() + 1,
    );
    let registry = registry_bytes(&output);
    let mut entries = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();

    assert!(matches!(
        publish_v3(
            &output,
            &owner,
            attempt,
            publication_plan,
            substituted_binding,
            bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::PublicationBindingMismatch)
    ));
    assert_eq!(registry_bytes(&output), registry);
    let mut current_entries = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    current_entries.sort();
    assert_eq!(current_entries, entries);
}

#[test]
fn pending_binding_plan_and_producer_substitutions_are_rejected() {
    let (
        _binding_temp,
        binding_output,
        binding_owner,
        binding_attempt,
        binding_bytes,
        binding_plan,
        original_binding,
    ) = pending_v3_fixture("binding", 0x25);
    let substituted_binding = substitute_intent_identity(original_binding, 0xd9);
    assert!(matches!(
        publish_v3(
            &binding_output,
            &binding_owner,
            binding_attempt,
            binding_plan,
            substituted_binding,
            &binding_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::PendingReceiptMismatch)
    ));

    let (
        _plan_temp,
        plan_output,
        plan_owner,
        plan_attempt,
        plan_bytes,
        original_plan,
        plan_binding,
    ) = pending_v3_fixture("plan", 0x35);
    assert!(matches!(
        publish_v3(
            &plan_output,
            &plan_owner,
            plan_attempt,
            plan(plan_attempt, original_plan.scope(), 0xe0, &plan_bytes),
            plan_binding,
            &plan_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::PendingReceiptMismatch)
    ));

    let (
        _producer_temp,
        producer_output,
        original_owner,
        producer_attempt,
        producer_bytes,
        producer_plan,
        producer_binding,
    ) = pending_v3_fixture("producer", 0x45);
    let intruder = producer("pending_v3", "/src/v3-pending-producer-intruder.rs");
    let registry = registry_bytes(&producer_output);
    assert!(matches!(
        publish_v3(
            &producer_output,
            &intruder,
            producer_attempt,
            producer_plan,
            producer_binding,
            &producer_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV3::Attempt(_))
    ));
    assert_ne!(intruder, original_owner);
    assert_eq!(registry_bytes(&producer_output), registry);
}

#[test]
fn newer_generation_invalidates_the_old_current_lease_and_claim() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("currentness_v3", "/src/v3-currentness.rs");
    let publication_scope = scope(0xe0);

    let first_attempt = begin(&output, &owner, 0x21);
    let first_bytes = b"strict Worker V3 generation one";
    let first_plan = plan(first_attempt, publication_scope, 0x30, first_bytes);
    let first_binding = persist_intent_and_binding(
        &output,
        &owner,
        first_attempt,
        first_plan,
        0x40,
        first_bytes,
    );
    let first_result = publish_v3(
        &output,
        &owner,
        first_attempt,
        first_plan,
        first_binding,
        first_bytes,
    )
    .unwrap();
    let first_claim = first_result.published_claim().clone();
    let first_lease = first_result.into_current_lease();
    drop(first_lease.acquire_current_token().unwrap());
    finish_build_attempt(&output, &owner, first_attempt).unwrap();

    let second_attempt = begin(&output, &owner, 0x22);
    let second_bytes = b"strict Worker V3 generation two";
    let second_plan = plan(second_attempt, publication_scope, 0x31, second_bytes);
    let second_binding = persist_intent_and_binding(
        &output,
        &owner,
        second_attempt,
        second_plan,
        0x41,
        second_bytes,
    );
    let second_result = publish_v3(
        &output,
        &owner,
        second_attempt,
        second_plan,
        second_binding,
        second_bytes,
    )
    .unwrap();
    let second_claim = second_result.published_claim().clone();
    drop(second_result);
    finish_build_attempt(&output, &owner, second_attempt).unwrap();

    assert!(matches!(
        first_lease.acquire_current_token(),
        Err(DurableLinkPublicationError::CurrentPublication { .. })
    ));
    assert!(matches!(
        reacquire_current_hsaco_publication_lease_v3(&output, &first_claim),
        Err(DurablePublishedClaimReacquisitionErrorV3::AttemptNotFound)
            | Err(DurablePublishedClaimReacquisitionErrorV3::AttemptState)
            | Err(DurablePublishedClaimReacquisitionErrorV3::ReceiptMismatch)
            | Err(DurablePublishedClaimReacquisitionErrorV3::Publication(_))
    ));
    let current = reacquire_current_hsaco_publication_lease_v3(&output, &second_claim).unwrap();
    assert_eq!(current.published().attempt(), second_attempt);
    assert_eq!(current.exact_artifact_bytes(), second_bytes);
    drop(current.acquire_current_token().unwrap());
}
