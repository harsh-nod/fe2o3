use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, BuildInvocation, BuildSession,
    CanonicalLinkRequestIdentityV1, DurableLinkPublicationPlanV1, DurablePublishedHsacoClaimV3,
    FinalizationIdentityV1, FinalizedOutputIdentityV1, KernelSetIdentityV1, LinkPublicationScopeV1,
    LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1, ProducerIdentity,
    TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    VerifiedWorkerV3LoadEnvelopeAuthorityV1, VerifiedWorkerV3PublicationAuthorityV1,
    WorkerV3FinalizerReplayAttachmentsV1, WorkerV3LoadEnvelopeBindingV1,
    WorkerV3LoadReadinessBoundaryV1, WorkerV3LoadReadinessErrorV1,
    WorkerV3LoadReadinessFaultPointV1, WorkerV3LoadReadinessFaultTimingV1,
    WorkerV3LoadReadinessOptionsV1, WorkerV3LoadReadinessOutcomeV1, WorkerV3PublicationBindingV1,
    WorkerV3PublicationIntentBoundaryV1, WorkerV3PublicationIntentErrorV1,
    WorkerV3PublicationIntentFaultPointV1, WorkerV3PublicationIntentFaultTimingV1,
    WorkerV3PublicationIntentIdentityV1, WorkerV3PublicationIntentOptionsV1, begin_build_attempt,
    clear_worker_v3_publication_intent_v1, persist_worker_v3_publication_intent_v1,
    publish_exact_hsaco_evidence_for_attempt_v3, publish_worker_v3_load_readiness_v1,
    publish_worker_v3_load_readiness_v1_with_options, recover_published_hsaco_claim_for_attempt_v3,
    recover_worker_v3_load_readiness_v1,
    retire_worker_v3_publication_intent_after_load_readiness_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const CHILD_OUTPUT: &str = "FE2O3_LOAD_READY_CHILD_OUTPUT";
const CHILD_CLAIM: &str = "FE2O3_LOAD_READY_CHILD_CLAIM";

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-worker-v3-load-readiness-{label}-{}-{}",
            std::process::id(),
            NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed)
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

struct PublishedV3 {
    directory: TestDirectory,
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    claim: DurablePublishedHsacoClaimV3,
    intent: WorkerV3PublicationIntentIdentityV1,
    envelope: Vec<u8>,
}

impl PublishedV3 {
    fn output(&self) -> PathBuf {
        self.directory.output()
    }

    fn authority(&self) -> VerifiedWorkerV3LoadEnvelopeAuthorityV1 {
        load_authority(&self.envelope)
    }

    fn publish_readiness(&self) -> fe2o3_artifact_transaction::WorkerV3LoadReadinessResultV1 {
        publish_worker_v3_load_readiness_v1(
            &self.output(),
            &self.claim,
            self.authority(),
            self.envelope.clone(),
        )
        .unwrap()
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn identity(seed: u8) -> [u8; 32] {
    [seed; 32]
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

fn publication_plan(
    attempt: BuildAttempt,
    exact_output: &[u8],
    seed: u8,
) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        LinkPublicationScopeV1::new(
            PackageIdentityV1::from_bytes(identity(seed)),
            KernelSetIdentityV1::from_bytes(identity(seed.wrapping_add(1))),
            TargetIdentityV1::from_bytes(identity(seed.wrapping_add(2))),
        ),
        CanonicalLinkRequestIdentityV1::from_bytes(identity(seed.wrapping_add(3))),
        PinnedWorkerIdentityV1::from_bytes(identity(seed.wrapping_add(4))),
        ValidatedResponseIdentityV1::from_bytes(identity(seed.wrapping_add(5))),
        LinkedOutputIdentityV1::from_bytes(digest(exact_output)),
        FinalizationIdentityV1::from_bytes(identity(seed.wrapping_add(6))),
        FinalizedOutputIdentityV1::from_bytes(digest(exact_output)),
        AtomicPublicationIdentityV1::from_bytes(identity(seed.wrapping_add(7))),
    )
}

fn publication_binding(
    exact_output: &[u8],
    intent: WorkerV3PublicationIntentIdentityV1,
    seed: u8,
) -> WorkerV3PublicationBindingV1 {
    WorkerV3PublicationBindingV1::new(
        compiler_closure(seed),
        intent.as_bytes(),
        identity(seed.wrapping_add(0x10)),
        identity(seed.wrapping_add(0x11)),
        identity(seed.wrapping_add(0x12)),
        identity(seed.wrapping_add(0x13)),
        digest(exact_output),
        exact_output.len() as u64,
        digest(exact_output),
        exact_output.len() as u64,
    )
    .unwrap()
}

fn load_authority(bytes: &[u8]) -> VerifiedWorkerV3LoadEnvelopeAuthorityV1 {
    let binding = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(bytes).unwrap();
    // SAFETY: these transaction tests model the upstream boundary that has verified the opaque
    // envelope contains every compact replay preimage. They never treat the result as load-ready.
    unsafe {
        VerifiedWorkerV3LoadEnvelopeAuthorityV1::from_complete_compact_replay_preimages_unchecked(
            binding,
        )
    }
}

fn setup(seed: u8) -> PublishedV3 {
    let directory = TestDirectory::new(&format!("{seed}"));
    let output = directory.output();
    let producer = ProducerIdentity::from_codegen(
        &format!("load_ready_{seed}"),
        Some(Path::new(&format!("/src/load-ready-{seed}.rs"))),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &output,
        &producer,
        BuildInvocation::from_bytes(identity(seed.wrapping_add(0x20))),
        BuildSession::from_bytes([seed.wrapping_add(0x30); 16]),
    )
    .unwrap();
    let exact_output = format!("exact finalized Worker V3 output {seed}").into_bytes();
    let plan = publication_plan(attempt, &exact_output, seed.wrapping_add(0x40));
    let replay = WorkerV3FinalizerReplayAttachmentsV1::new(
        format!("complete outer handoff {seed}").into_bytes(),
        vec![format!("provider preimage {seed}").into_bytes()],
        format!("complete compact replay transcript {seed}").into_bytes(),
    )
    .unwrap();
    let persisted = persist_worker_v3_publication_intent_v1(
        &output,
        &producer,
        attempt,
        plan,
        replay,
        exact_output.clone(),
    )
    .unwrap();
    let intent = persisted.record().identity();
    drop(persisted);
    let binding = publication_binding(&exact_output, intent, seed.wrapping_add(0x50));
    // SAFETY: exact restart inputs and the exact finalizer binding were retained above.
    let authority = unsafe {
        VerifiedWorkerV3PublicationAuthorityV1::from_authenticated_finalizer_replay_unchecked(
            binding,
        )
    };
    let published = publish_exact_hsaco_evidence_for_attempt_v3(
        &output,
        &producer,
        attempt,
        plan,
        UpstreamCodeObjectEvidenceIdentityV1::from_bytes(digest(plan.finalization().as_bytes())),
        authority,
        &exact_output,
    )
    .unwrap();
    let claim = published.published_claim().clone();
    drop(published);
    let envelope =
        format!("opaque canonical V3 envelope with complete compact replay preimages {seed}")
            .into_bytes();
    PublishedV3 {
        directory,
        producer,
        attempt,
        claim,
        intent,
        envelope,
    }
}

fn readiness_entry(output: &Path, suffix: &str) -> PathBuf {
    let mut matches = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(".fe2o3-worker-v3-load-readiness-v1-") && name.ends_with(suffix)
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one readiness entry {suffix}");
    matches.pop().unwrap()
}

#[test]
fn exact_publication_retry_and_restart_recovery_are_idempotent_and_inert() {
    let state = setup(1);
    let first = state.publish_readiness();
    assert_eq!(first.outcome(), WorkerV3LoadReadinessOutcomeV1::Published);
    assert_eq!(fs::read(first.envelope_path()).unwrap(), state.envelope);
    assert!(!first.authenticates_descriptor_source());
    assert!(!first.grants_semantic_load_admission());
    assert!(!first.establishes_hsa_readiness());
    assert!(!first.grants_load_authority());
    assert!(!first.grants_launch_authority());

    let retry = state.publish_readiness();
    assert_eq!(retry.outcome(), WorkerV3LoadReadinessOutcomeV1::Recovered);
    assert_eq!(retry.receipt(), first.receipt());
    let recovered = recover_worker_v3_load_readiness_v1(&state.output(), &state.claim).unwrap();
    assert_eq!(recovered.receipt(), first.receipt());
    let reconstructed = recover_published_hsaco_claim_for_attempt_v3(
        &state.output(),
        &state.producer,
        state.attempt,
        state.claim.plan(),
        state.claim.upstream_evidence(),
        state.claim.worker_v3_binding(),
        state.claim.receipt(),
    )
    .unwrap();
    assert_eq!(reconstructed, state.claim);
}

#[test]
fn crash_sides_reconcile_to_one_exact_terminal_receipt() {
    let points = [
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::SyncEnvelopeTemp,
            timing: WorkerV3LoadReadinessFaultTimingV1::After,
        },
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::RenameEnvelope,
            timing: WorkerV3LoadReadinessFaultTimingV1::After,
        },
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::SyncEnvelopeName,
            timing: WorkerV3LoadReadinessFaultTimingV1::After,
        },
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::SyncReceiptTemp,
            timing: WorkerV3LoadReadinessFaultTimingV1::After,
        },
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::RenameReceipt,
            timing: WorkerV3LoadReadinessFaultTimingV1::After,
        },
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::SyncReceiptName,
            timing: WorkerV3LoadReadinessFaultTimingV1::After,
        },
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::CommitAttemptRegistry,
            timing: WorkerV3LoadReadinessFaultTimingV1::Before,
        },
        WorkerV3LoadReadinessFaultPointV1 {
            boundary: WorkerV3LoadReadinessBoundaryV1::CommitAttemptRegistry,
            timing: WorkerV3LoadReadinessFaultTimingV1::After,
        },
    ];
    for (index, point) in points.into_iter().enumerate() {
        let state = setup(10 + index as u8);
        let error = publish_worker_v3_load_readiness_v1_with_options(
            &state.output(),
            &state.claim,
            state.authority(),
            state.envelope.clone(),
            WorkerV3LoadReadinessOptionsV1 {
                injected_crash: Some(point),
            },
        )
        .unwrap_err();
        assert!(
            matches!(error, WorkerV3LoadReadinessErrorV1::InjectedCrash(actual) if actual == point)
        );
        let retry = state.publish_readiness();
        assert_eq!(fs::read(retry.envelope_path()).unwrap(), state.envelope);
        recover_worker_v3_load_readiness_v1(&state.output(), &state.claim).unwrap();
    }
}

#[test]
fn wrong_authority_claim_envelope_and_stale_generation_fail_closed() {
    let state = setup(30);
    let wrong_authority = load_authority(b"different envelope");
    assert!(matches!(
        publish_worker_v3_load_readiness_v1(
            &state.output(),
            &state.claim,
            wrong_authority,
            state.envelope.clone(),
        ),
        Err(WorkerV3LoadReadinessErrorV1::AuthorityMismatch)
    ));

    let foreign = setup(31);
    assert!(matches!(
        publish_worker_v3_load_readiness_v1(
            &state.output(),
            &foreign.claim,
            state.authority(),
            state.envelope.clone(),
        ),
        Err(WorkerV3LoadReadinessErrorV1::Claim(_))
    ));

    state.publish_readiness();
    let wrong_bytes = b"substituted retry envelope".to_vec();
    assert!(matches!(
        publish_worker_v3_load_readiness_v1(
            &state.output(),
            &state.claim,
            load_authority(&wrong_bytes),
            wrong_bytes,
        ),
        Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch)
    ));

    begin_build_attempt(
        &state.output(),
        &state.producer,
        BuildInvocation::from_bytes(identity(0xf1)),
        BuildSession::from_bytes([0xf2; 16]),
    )
    .unwrap();
    assert!(matches!(
        recover_worker_v3_load_readiness_v1(&state.output(), &state.claim),
        Err(WorkerV3LoadReadinessErrorV1::Claim(_))
    ));
}

#[test]
fn missing_and_inode_substituted_terminal_files_fail_closed() {
    let missing_envelope = setup(40);
    missing_envelope.publish_readiness();
    fs::remove_file(readiness_entry(&missing_envelope.output(), ".envelope")).unwrap();
    assert!(matches!(
        recover_worker_v3_load_readiness_v1(&missing_envelope.output(), &missing_envelope.claim),
        Err(WorkerV3LoadReadinessErrorV1::MissingEnvelope)
    ));

    let missing_receipt = setup(41);
    missing_receipt.publish_readiness();
    fs::remove_file(readiness_entry(&missing_receipt.output(), ".receipt")).unwrap();
    assert!(matches!(
        recover_worker_v3_load_readiness_v1(&missing_receipt.output(), &missing_receipt.claim),
        Err(WorkerV3LoadReadinessErrorV1::MissingReceipt)
    ));

    let substituted = setup(42);
    substituted.publish_readiness();
    let envelope_path = readiness_entry(&substituted.output(), ".envelope");
    fs::remove_file(&envelope_path).unwrap();
    fs::write(&envelope_path, &substituted.envelope).unwrap();
    fs::set_permissions(&envelope_path, fs::Permissions::from_mode(0o600)).unwrap();
    let error =
        recover_worker_v3_load_readiness_v1(&substituted.output(), &substituted.claim).unwrap_err();
    assert!(
        matches!(error, WorkerV3LoadReadinessErrorV1::EnvelopeMismatch),
        "unexpected substitution error: {error:?}"
    );
}

#[test]
fn only_exact_durable_readiness_retires_current_intent_without_admitting_load() {
    let state = setup(50);
    let readiness = state.publish_readiness();
    let foreign = setup(51);
    let foreign_readiness = foreign.publish_readiness();
    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &state.output(),
            &state.producer,
            state.attempt,
            state.intent,
        ),
        Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable)
    ));
    assert!(!readiness.receipt().authenticates_descriptor_source());
    assert!(!readiness.receipt().grants_semantic_load_admission());
    assert!(!readiness.receipt().establishes_hsa_readiness());
    assert!(!readiness.receipt().grants_load_authority());
    assert!(!readiness.receipt().grants_launch_authority());
    assert!(matches!(
        retire_worker_v3_publication_intent_after_load_readiness_v1(
            &state.output(),
            &state.producer,
            state.attempt,
            state.intent,
            foreign_readiness.receipt(),
        ),
        Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable)
    ));

    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &state.output(),
        &state.producer,
        state.attempt,
        state.intent,
        readiness.receipt(),
    )
    .unwrap();
    assert!(fs::read_dir(state.output()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".fe2o3-worker-v3-publication-intent-v1-")
    }));
    recover_worker_v3_load_readiness_v1(&state.output(), &state.claim).unwrap();
}

#[test]
fn current_retirement_crash_resumes_only_with_the_same_readiness() {
    let state = setup(52);
    let readiness = state.publish_readiness();
    let point = WorkerV3PublicationIntentFaultPointV1 {
        boundary: WorkerV3PublicationIntentBoundaryV1::SyncRetiringName,
        timing: WorkerV3PublicationIntentFaultTimingV1::After,
    };
    let error =
        fe2o3_artifact_transaction::retire_worker_v3_publication_intent_after_load_readiness_v1_with_options(
            &state.output(),
            &state.producer,
            state.attempt,
            state.intent,
            readiness.receipt(),
            WorkerV3PublicationIntentOptionsV1::inject_crash(point),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        WorkerV3PublicationIntentErrorV1::InjectedCrash { point: actual } if actual == point
    ));
    assert!(matches!(
        clear_worker_v3_publication_intent_v1(
            &state.output(),
            &state.producer,
            state.attempt,
            state.intent,
        ),
        Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable)
    ));
    retire_worker_v3_publication_intent_after_load_readiness_v1(
        &state.output(),
        &state.producer,
        state.attempt,
        state.intent,
        readiness.receipt(),
    )
    .unwrap();
}

#[test]
fn process_restart_child_revalidates_terminal_custody() {
    let Ok(output) = std::env::var(CHILD_OUTPUT) else {
        return;
    };
    let claim_path = std::env::var(CHILD_CLAIM).unwrap();
    let claim =
        DurablePublishedHsacoClaimV3::decode_canonical(&fs::read(claim_path).unwrap()).unwrap();
    let recovered = recover_worker_v3_load_readiness_v1(Path::new(&output), &claim).unwrap();
    assert_eq!(
        recovered.outcome(),
        WorkerV3LoadReadinessOutcomeV1::Recovered
    );
    assert!(!recovered.grants_load_authority());
}

#[test]
fn process_restart_recovery_uses_only_durable_state() {
    if std::env::var_os(CHILD_OUTPUT).is_some() {
        return;
    }
    let state = setup(60);
    state.publish_readiness();
    let claim_path = state.directory.0.join("claim.v3");
    fs::write(&claim_path, state.claim.encode_canonical().unwrap()).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("process_restart_child_revalidates_terminal_custody")
        .arg("--nocapture")
        .env(CHILD_OUTPUT, state.output())
        .env(CHILD_CLAIM, claim_path)
        .status()
        .unwrap();
    assert!(status.success());
}
