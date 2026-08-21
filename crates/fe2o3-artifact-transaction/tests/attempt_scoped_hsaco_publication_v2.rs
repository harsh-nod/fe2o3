use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationBoundaryV2,
    AttemptScopedHsacoPublicationErrorV1, AttemptScopedHsacoPublicationErrorV2,
    AttemptScopedHsacoPublicationFaultPointV2, AttemptScopedHsacoPublicationFaultTimingV2,
    AttemptScopedHsacoPublicationOptionsV2, AttemptScopedHsacoPublicationOutcomeV2,
    BackendPublicationReceiptValidationErrorV2, BuildAttempt, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableArtifactBoundaryV1, DurableFaultTimingV1,
    DurableJournalBoundaryV1, DurableJournalStageV1, DurableLinkPublicationError,
    DurableLinkPublicationFaultPointV1, DurableLinkPublicationOptionsV1,
    DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PersistedBackendReceiptV2, PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, begin_build_attempt,
    finish_build_attempt, publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v1_with_options,
    publish_exact_hsaco_evidence_for_attempt_v2,
    publish_exact_hsaco_evidence_for_attempt_v2_with_options, read_backend_publication_receipt_v1,
    read_backend_publication_receipt_v2, recover_durable_link_publication_v1,
    recover_published_hsaco_claim_for_attempt_v1, recover_published_hsaco_claim_for_attempt_v2,
    validate_backend_publication_receipt_v2,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;

const ATTEMPT_REGISTRY: &str = ".fe2o3-attempts-v1";

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-attempt-hsaco-v2-{}-{}",
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

fn begin(output: &Path, owner: &ProducerIdentity, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        output,
        owner,
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32]),
        BuildSession::from_bytes([seed; 16]),
    )
    .unwrap()
}

fn fake_attempt(attempt: BuildAttempt, seed: u8) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{}:{}:{}",
        attempt.generation(),
        BuildSession::from_bytes([seed; 16]),
        BuildInvocation::from_bytes([seed.wrapping_add(0x40); 32])
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

fn closure_pins(closure: CompilerClosureV2) -> [[u8; 32]; 6] {
    [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ]
}

fn substitute_closure_role(
    original: CompilerClosureV2,
    alternate: CompilerClosureV2,
    role: usize,
) -> CompilerClosureV2 {
    let mut pins = closure_pins(original);
    pins[role] = closure_pins(alternate)[role];
    CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
}

fn closure_bytes(closure: CompilerClosureV2) -> Vec<u8> {
    let mut bytes = Vec::new();
    for pin in closure_pins(closure) {
        bytes.extend_from_slice(&pin);
    }
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&closure.identity_sha256());
    bytes
}

fn publish_v2(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    closure: CompilerClosureV2,
    bytes: &[u8],
) -> Result<
    fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV2,
    AttemptScopedHsacoPublicationErrorV2,
> {
    publish_exact_hsaco_evidence_for_attempt_v2(
        output,
        owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        closure,
        bytes,
    )
}

fn publish_v2_with_options(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    closure: CompilerClosureV2,
    bytes: &[u8],
    options: AttemptScopedHsacoPublicationOptionsV2,
) -> Result<
    fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV2,
    AttemptScopedHsacoPublicationErrorV2,
> {
    publish_exact_hsaco_evidence_for_attempt_v2_with_options(
        output,
        owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        closure,
        bytes,
        options,
    )
}

fn publish_v1(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    bytes: &[u8],
) -> Result<
    fe2o3_artifact_transaction::AttemptScopedHsacoPublicationResultV1,
    AttemptScopedHsacoPublicationErrorV1,
> {
    publish_exact_hsaco_evidence_for_attempt_v1(
        output,
        owner,
        attempt,
        publication_plan,
        upstream(publication_plan),
        bytes,
    )
}

fn before_planned_redo() -> DurableLinkPublicationFaultPointV1 {
    DurableLinkPublicationFaultPointV1::Journal {
        stage: DurableJournalStageV1::Planned,
        boundary: DurableJournalBoundaryV1::CreateRedoTemp,
        timing: DurableFaultTimingV1::Before,
    }
}

fn interrupt_v2_before_planned_redo(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    closure: CompilerClosureV2,
    bytes: &[u8],
) {
    let point = before_planned_redo();
    assert!(matches!(
        publish_v2_with_options(
            output,
            owner,
            attempt,
            publication_plan,
            closure,
            bytes,
            AttemptScopedHsacoPublicationOptionsV2::inject_durable_crash(point),
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::PublicationInterrupted(
            DurableLinkPublicationError::InjectedCrash { point: actual }
        )) if actual == point
    ));
}

fn find_unique(haystack: &[u8], needle: &[u8]) -> usize {
    let offsets = haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == needle).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "expected one canonical byte sequence");
    offsets[0]
}

#[test]
fn exact_retry_recovers_every_success_journal_boundary_and_timing() {
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

    let mut case = 0_u8;
    for stage in stages {
        for boundary in boundaries {
            for timing in timings {
                let temp = TestDirectory::new();
                let output = temp.output();
                let owner = producer("journal_v2", &format!("/src/v2-journal-{case}.rs"));
                let attempt = begin(&output, &owner, case.wrapping_add(1));
                let bytes = format!("v2-journal-{stage:?}-{boundary:?}-{timing:?}").into_bytes();
                let publication_plan = plan(attempt, scope(case.wrapping_add(0x10)), 0x60, &bytes);
                let closure = compiler_closure(case.wrapping_add(0x80));
                let point = DurableLinkPublicationFaultPointV1::Journal {
                    stage,
                    boundary,
                    timing,
                };

                assert!(
                    matches!(
                        publish_v2_with_options(
                            &output,
                            &owner,
                            attempt,
                            publication_plan,
                            closure,
                            &bytes,
                            AttemptScopedHsacoPublicationOptionsV2::inject_durable_crash(point),
                        ),
                        Err(AttemptScopedHsacoPublicationErrorV2::PublicationInterrupted(
                            DurableLinkPublicationError::InjectedCrash { point: actual }
                        )) if actual == point
                    ),
                    "missing injected crash at {point:?}"
                );
                assert!(matches!(
                    read_backend_publication_receipt_v2(&output, &owner, attempt).unwrap(),
                    PersistedBackendReceiptV2::PendingProvenance(receipt)
                        if receipt.compiler_closure() == closure
                ));

                let recovered =
                    publish_v2(&output, &owner, attempt, publication_plan, closure, &bytes)
                        .unwrap();
                assert!(matches!(
                    recovered.outcome(),
                    AttemptScopedHsacoPublicationOutcomeV2::Published
                        | AttemptScopedHsacoPublicationOutcomeV2::RecoveredAndPublished
                        | AttemptScopedHsacoPublicationOutcomeV2::RecoveredCommittedPublication
                ));
                assert_eq!(recovered.compiler_closure(), closure);
                assert_eq!(recovered.snapshot().artifact().bytes(), bytes);
                assert_eq!(
                    read_backend_publication_receipt_v2(&output, &owner, attempt).unwrap(),
                    PersistedBackendReceiptV2::Provenance(recovered.receipt())
                );
                finish_build_attempt(&output, &owner, attempt).unwrap();
                case = case.wrapping_add(1);
            }
        }
    }
    assert_eq!(case, 70);
}

#[test]
fn exact_retry_recovers_every_artifact_boundary_and_timing() {
    let boundaries = [
        DurableArtifactBoundaryV1::CreateTemp,
        DurableArtifactBoundaryV1::WriteTemp,
        DurableArtifactBoundaryV1::SyncTemp,
        DurableArtifactBoundaryV1::RenameToContentAddress,
        DurableArtifactBoundaryV1::SyncDirectory,
    ];
    let timings = [DurableFaultTimingV1::Before, DurableFaultTimingV1::After];

    let mut case = 0_u8;
    for boundary in boundaries {
        for timing in timings {
            let temp = TestDirectory::new();
            let output = temp.output();
            let owner = producer("artifact_v2", &format!("/src/v2-artifact-{case}.rs"));
            let attempt = begin(&output, &owner, case.wrapping_add(0x20));
            let bytes = format!("v2-artifact-{boundary:?}-{timing:?}").into_bytes();
            let publication_plan = plan(attempt, scope(case.wrapping_add(0x30)), 0x70, &bytes);
            let closure = compiler_closure(case.wrapping_add(0x90));
            let point = DurableLinkPublicationFaultPointV1::Artifact { boundary, timing };

            assert!(matches!(
                publish_v2_with_options(
                    &output,
                    &owner,
                    attempt,
                    publication_plan,
                    closure,
                    &bytes,
                    AttemptScopedHsacoPublicationOptionsV2::inject_durable_crash(point),
                ),
                Err(AttemptScopedHsacoPublicationErrorV2::PublicationInterrupted(
                    DurableLinkPublicationError::InjectedCrash { point: actual }
                )) if actual == point
            ));
            let recovered =
                publish_v2(&output, &owner, attempt, publication_plan, closure, &bytes).unwrap();
            assert_eq!(recovered.snapshot().artifact().bytes(), bytes);
            assert_eq!(recovered.receipt().compiler_closure(), closure);
            finish_build_attempt(&output, &owner, attempt).unwrap();
            case = case.wrapping_add(1);
        }
    }
    assert_eq!(case, 10);
}

#[test]
fn exact_retry_reconciles_every_receipt_commit_side() {
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
            let owner = producer("receipt_v2", &format!("/src/v2-receipt-{case}.rs"));
            let attempt = begin(&output, &owner, case.wrapping_add(0x40));
            let bytes = format!("v2-receipt-{boundary:?}-{timing:?}").into_bytes();
            let publication_plan = plan(attempt, scope(case.wrapping_add(0x50)), 0xa0, &bytes);
            let closure = compiler_closure(case.wrapping_add(0xb0));
            let point = AttemptScopedHsacoPublicationFaultPointV2 { boundary, timing };

            assert!(matches!(
                publish_v2_with_options(
                    &output,
                    &owner,
                    attempt,
                    publication_plan,
                    closure,
                    &bytes,
                    AttemptScopedHsacoPublicationOptionsV2::inject_receipt_crash(point),
                ),
                Err(AttemptScopedHsacoPublicationErrorV2::ReceiptCommitInterrupted {
                    point: actual
                }) if actual == point
            ));

            let retry = publish_v2(&output, &owner, attempt, publication_plan, closure, &bytes);
            if boundary == AttemptScopedHsacoPublicationBoundaryV2::CommitFinalReceipt
                && timing == AttemptScopedHsacoPublicationFaultTimingV2::After
            {
                assert!(matches!(
                    retry,
                    Err(AttemptScopedHsacoPublicationErrorV2::ReceiptAlreadyPersisted { .. })
                ));
            } else {
                assert_eq!(retry.unwrap().compiler_closure(), closure);
            }
            assert!(matches!(
                read_backend_publication_receipt_v2(&output, &owner, attempt).unwrap(),
                PersistedBackendReceiptV2::Provenance(receipt)
                    if receipt.compiler_closure() == closure
            ));
            finish_build_attempt(&output, &owner, attempt).unwrap();
            case = case.wrapping_add(1);
        }
    }
    assert_eq!(case, 4);
}

#[test]
fn exact_replay_and_two_caller_recovery_have_one_publication_winner() {
    let replay_temp = TestDirectory::new();
    let replay_output = replay_temp.output();
    let replay_owner = producer("replay_v2", "/src/v2-replay.rs");
    let replay_attempt = begin(&replay_output, &replay_owner, 0x51);
    let replay_bytes = b"v2 exact replay";
    let replay_plan = plan(replay_attempt, scope(0x52), 0x53, replay_bytes);
    let replay_closure = compiler_closure(0x54);
    let first = publish_v2(
        &replay_output,
        &replay_owner,
        replay_attempt,
        replay_plan,
        replay_closure,
        replay_bytes,
    )
    .unwrap();
    let first_receipt = first.receipt();
    drop(first);
    assert!(matches!(
        publish_v2(
            &replay_output,
            &replay_owner,
            replay_attempt,
            replay_plan,
            replay_closure,
            replay_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::ReceiptAlreadyPersisted { receipt })
            if *receipt == first_receipt
    ));

    let concurrent_temp = TestDirectory::new();
    let output = concurrent_temp.output();
    let owner = producer("concurrent_v2", "/src/v2-concurrent-retry.rs");
    let attempt = begin(&output, &owner, 0x61);
    let bytes = b"v2 concurrent exact retry";
    let publication_plan = plan(attempt, scope(0x62), 0x63, bytes);
    let closure = compiler_closure(0x64);
    interrupt_v2_before_planned_redo(&output, &owner, attempt, publication_plan, closure, bytes);
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let output = output.clone();
        let owner = owner.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            barrier.wait();
            publish_v2(&output, &owner, attempt, publication_plan, closure, bytes)
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
                Err(AttemptScopedHsacoPublicationErrorV2::ReceiptAlreadyPersisted { .. })
            ))
            .count(),
        1
    );
    let recovered = recover_durable_link_publication_v1(&output, publication_plan.scope())
        .unwrap()
        .unwrap();
    assert_eq!(recovered.artifact().bytes(), bytes);
}

#[test]
fn valid_alternate_closure_publishes_but_each_role_substitution_fails_recovery() {
    let positive_temp = TestDirectory::new();
    let positive_output = positive_temp.output();
    let positive_owner = producer("alternate_v2", "/src/v2-valid-alternate.rs");
    let positive_attempt = begin(&positive_output, &positive_owner, 0x70);
    let positive_bytes = b"valid alternate closure";
    let positive_plan = plan(positive_attempt, scope(0x71), 0x72, positive_bytes);
    let alternate = compiler_closure(0xd0);
    let positive = publish_v2(
        &positive_output,
        &positive_owner,
        positive_attempt,
        positive_plan,
        alternate,
        positive_bytes,
    )
    .unwrap();
    assert_eq!(positive.receipt().compiler_closure(), alternate);
    assert_eq!(positive.published_claim().compiler_closure(), alternate);

    let original = compiler_closure(0x80);
    for role in 0..6 {
        let temp = TestDirectory::new();
        let output = temp.output();
        let owner = producer("roles_v2", &format!("/src/v2-closure-role-{role}.rs"));
        let attempt = begin(&output, &owner, 0x81_u8.wrapping_add(role as u8));
        let bytes = format!("closure role {role}").into_bytes();
        let publication_plan = plan(
            attempt,
            scope(0x82_u8.wrapping_add(role as u8)),
            0x90,
            &bytes,
        );
        interrupt_v2_before_planned_redo(
            &output,
            &owner,
            attempt,
            publication_plan,
            original,
            &bytes,
        );

        let substituted = substitute_closure_role(original, alternate, role);
        assert_ne!(substituted, original);
        assert!(
            matches!(
                publish_v2(
                    &output,
                    &owner,
                    attempt,
                    publication_plan,
                    substituted,
                    &bytes,
                ),
                Err(AttemptScopedHsacoPublicationErrorV2::PendingReceiptMismatch)
            ),
            "closure role {role} substitution was accepted"
        );
        assert!(
            recover_durable_link_publication_v1(&output, publication_plan.scope())
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn receipt_validation_and_pending_recovery_reject_every_lineage_substitution() {
    let validation_temp = TestDirectory::new();
    let validation_output = validation_temp.output();
    let owner = producer("lineage_v2", "/src/v2-lineage.rs");
    let attempt = begin(&validation_output, &owner, 0x91);
    let bytes = b"v2 lineage substitutions";
    let publication_plan = plan(attempt, scope(0x92), 0x93, bytes);
    let closure = compiler_closure(0x94);
    let receipt = publish_v2(
        &validation_output,
        &owner,
        attempt,
        publication_plan,
        closure,
        bytes,
    )
    .unwrap()
    .receipt();
    assert_eq!(
        validate_backend_publication_receipt_v2(
            &owner,
            attempt,
            publication_plan,
            upstream(publication_plan),
            closure,
            receipt,
        ),
        Ok(())
    );
    assert_eq!(
        validate_backend_publication_receipt_v2(
            &owner,
            attempt,
            publication_plan,
            upstream(publication_plan),
            compiler_closure(0xa4),
            receipt,
        ),
        Err(BackendPublicationReceiptValidationErrorV2::CompilerClosureMismatch)
    );
    assert_eq!(
        validate_backend_publication_receipt_v2(
            &owner,
            attempt,
            plan(attempt, publication_plan.scope(), 0x95, bytes),
            upstream(publication_plan),
            closure,
            receipt,
        ),
        Err(BackendPublicationReceiptValidationErrorV2::PlanCommitmentMismatch)
    );

    let plan_temp = TestDirectory::new();
    let plan_output = plan_temp.output();
    let plan_owner = producer("plan_v2", "/src/v2-plan-substitution.rs");
    let plan_attempt = begin(&plan_output, &plan_owner, 0xa1);
    let plan_bytes = b"v2 plan substitution";
    let original_plan = plan(plan_attempt, scope(0xa2), 0xa3, plan_bytes);
    interrupt_v2_before_planned_redo(
        &plan_output,
        &plan_owner,
        plan_attempt,
        original_plan,
        closure,
        plan_bytes,
    );
    assert!(matches!(
        publish_v2(
            &plan_output,
            &plan_owner,
            plan_attempt,
            plan(plan_attempt, original_plan.scope(), 0xa4, plan_bytes),
            closure,
            plan_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::PendingReceiptMismatch)
    ));

    let output_temp = TestDirectory::new();
    let output = output_temp.output();
    let output_owner = producer("output_v2", "/src/v2-output-substitution.rs");
    let output_attempt = begin(&output, &output_owner, 0xb1);
    let original_bytes = b"v2 original exact output";
    let changed_bytes = b"v2 changed exact output!";
    assert_eq!(original_bytes.len(), changed_bytes.len());
    let output_plan = plan(output_attempt, scope(0xb2), 0xb3, original_bytes);
    interrupt_v2_before_planned_redo(
        &output,
        &output_owner,
        output_attempt,
        output_plan,
        closure,
        original_bytes,
    );
    assert!(matches!(
        publish_v2(
            &output,
            &output_owner,
            output_attempt,
            output_plan,
            closure,
            changed_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::Durable(
            DurableLinkPublicationError::FinalizedArtifactDigestMismatch
        ))
    ));

    let identity_temp = TestDirectory::new();
    let identity_output = identity_temp.output();
    let identity_owner = producer("identity_v2", "/src/v2-identity-owner.rs");
    let identity_attempt = begin(&identity_output, &identity_owner, 0xc1);
    let identity_bytes = b"v2 identity substitutions";
    let identity_plan = plan(identity_attempt, scope(0xc2), 0xc3, identity_bytes);
    interrupt_v2_before_planned_redo(
        &identity_output,
        &identity_owner,
        identity_attempt,
        identity_plan,
        closure,
        identity_bytes,
    );
    let registry_before = fs::read(identity_output.join(ATTEMPT_REGISTRY)).unwrap();
    let intruder = producer("identity_v2", "/src/v2-identity-intruder.rs");
    assert!(matches!(
        publish_v2(
            &identity_output,
            &intruder,
            identity_attempt,
            identity_plan,
            closure,
            identity_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::Attempt(_))
    ));
    let substituted_attempt = fake_attempt(identity_attempt, 0xc4);
    assert!(matches!(
        publish_v2(
            &identity_output,
            &identity_owner,
            substituted_attempt,
            plan(
                substituted_attempt,
                identity_plan.scope(),
                0xc3,
                identity_bytes
            ),
            closure,
            identity_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::Attempt(_))
    ));
    let other_temp = TestDirectory::new();
    assert!(matches!(
        publish_v2(
            &other_temp.output(),
            &identity_owner,
            identity_attempt,
            identity_plan,
            closure,
            identity_bytes,
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::Attempt(_))
    ));
    assert_eq!(
        fs::read(identity_output.join(ATTEMPT_REGISTRY)).unwrap(),
        registry_before
    );
}

#[test]
fn registry_protocol_and_aggregate_mutation_fail_closed_without_rewrite() {
    for (case, final_receipt) in [("protocol", false), ("aggregate", true)] {
        let temp = TestDirectory::new();
        let output = temp.output();
        let owner = producer("registry_v2", &format!("/src/v2-registry-{case}.rs"));
        let attempt = begin(&output, &owner, if final_receipt { 0xd1 } else { 0xd0 });
        let bytes = format!("v2 registry {case}").into_bytes();
        let publication_plan = plan(attempt, scope(0xd2), 0xd3, &bytes);
        let closure = compiler_closure(0xe0);
        if final_receipt {
            drop(publish_v2(&output, &owner, attempt, publication_plan, closure, &bytes).unwrap());
        } else {
            interrupt_v2_before_planned_redo(
                &output,
                &owner,
                attempt,
                publication_plan,
                closure,
                &bytes,
            );
        }

        let registry_path = output.join(ATTEMPT_REGISTRY);
        let mut mutated = fs::read(&registry_path).unwrap();
        let closure_offset = find_unique(&mutated, &closure_bytes(closure));
        if final_receipt {
            mutated[closure_offset + 6 * 32 + 2] ^= 0x80;
        } else {
            mutated[closure_offset + 6 * 32..closure_offset + 6 * 32 + 2]
                .copy_from_slice(&2_u16.to_le_bytes());
        }
        fs::write(&registry_path, &mutated).unwrap();

        assert!(matches!(
            read_backend_publication_receipt_v2(&output, &owner, attempt),
            Err(AttemptScopedHsacoPublicationErrorV2::Attempt(_))
        ));
        assert!(matches!(
            publish_v2(&output, &owner, attempt, publication_plan, closure, &bytes),
            Err(AttemptScopedHsacoPublicationErrorV2::Attempt(_))
        ));
        assert_eq!(fs::read(&registry_path).unwrap(), mutated);
    }
}

#[test]
fn v1_pending_and_final_receipts_never_enter_v2_apis_or_change_registry() {
    for final_receipt in [false, true] {
        let target = TestDirectory::new();
        let output = target.output();
        let owner = producer("cross_v1", "/src/v1-into-v2.rs");
        let seed = if final_receipt { 0x22 } else { 0x21 };
        let attempt = begin(&output, &owner, seed);
        let bytes = b"v1 state presented to v2";
        let publication_plan = plan(attempt, scope(0x23), 0x24, bytes);
        if final_receipt {
            drop(publish_v1(&output, &owner, attempt, publication_plan, bytes).unwrap());
        } else {
            let point = before_planned_redo();
            assert!(matches!(
                publish_exact_hsaco_evidence_for_attempt_v1_with_options(
                    &output,
                    &owner,
                    attempt,
                    publication_plan,
                    upstream(publication_plan),
                    bytes,
                    DurableLinkPublicationOptionsV1::inject_crash(point),
                ),
                Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(
                    DurableLinkPublicationError::InjectedCrash { point: actual }
                )) if actual == point
            ));
        }
        let registry_path = output.join(ATTEMPT_REGISTRY);
        let registry_before = fs::read(&registry_path).unwrap();
        let closure = compiler_closure(0x25);

        assert!(matches!(
            read_backend_publication_receipt_v2(&output, &owner, attempt),
            Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
        ));
        assert!(matches!(
            publish_v2(&output, &owner, attempt, publication_plan, closure, bytes,),
            Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
        ));

        let donor = TestDirectory::new();
        let donor_output = donor.output();
        let donor_attempt = begin(&donor_output, &owner, seed);
        assert_eq!(donor_attempt, attempt);
        let donor_receipt = publish_v2(
            &donor_output,
            &owner,
            donor_attempt,
            publication_plan,
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
                donor_receipt,
            ),
            Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
        ));
        assert_eq!(fs::read(&registry_path).unwrap(), registry_before);
    }
}

#[test]
fn v2_pending_and_final_receipts_never_enter_v1_apis_or_change_registry() {
    for final_receipt in [false, true] {
        let target = TestDirectory::new();
        let output = target.output();
        let owner = producer("cross_v2", "/src/v2-into-v1.rs");
        let seed = if final_receipt { 0x32 } else { 0x31 };
        let attempt = begin(&output, &owner, seed);
        let bytes = b"v2 state presented to v1";
        let publication_plan = plan(attempt, scope(0x33), 0x34, bytes);
        let closure = compiler_closure(0x35);
        if final_receipt {
            drop(publish_v2(&output, &owner, attempt, publication_plan, closure, bytes).unwrap());
        } else {
            interrupt_v2_before_planned_redo(
                &output,
                &owner,
                attempt,
                publication_plan,
                closure,
                bytes,
            );
        }
        let registry_path = output.join(ATTEMPT_REGISTRY);
        let registry_before = fs::read(&registry_path).unwrap();

        assert!(read_backend_publication_receipt_v1(&output, &owner, attempt).is_err());
        assert!(matches!(
            publish_v1(&output, &owner, attempt, publication_plan, bytes),
            Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
        ));

        let donor = TestDirectory::new();
        let donor_output = donor.output();
        let donor_attempt = begin(&donor_output, &owner, seed);
        assert_eq!(donor_attempt, attempt);
        let donor_receipt = publish_v1(
            &donor_output,
            &owner,
            donor_attempt,
            publication_plan,
            bytes,
        )
        .unwrap()
        .receipt();
        assert!(matches!(
            recover_published_hsaco_claim_for_attempt_v1(
                &output,
                &owner,
                attempt,
                publication_plan,
                upstream(publication_plan),
                donor_receipt,
            ),
            Err(AttemptScopedHsacoPublicationErrorV1::Attempt(_))
        ));
        assert_eq!(fs::read(&registry_path).unwrap(), registry_before);
    }
}
