#[path = "support/process.rs"]
mod test_process;

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
    clear_worker_v3_publication_intent_v1, discover_worker_v3_load_readiness_attempts_v1,
    persist_worker_v3_publication_intent_v1, publish_exact_hsaco_evidence_for_attempt_v3,
    publish_worker_v3_load_readiness_v1, publish_worker_v3_load_readiness_v1_with_options,
    recover_published_hsaco_claim_for_attempt_v3, recover_worker_v3_load_readiness_for_attempt_v1,
    recover_worker_v3_load_readiness_v1, recover_worker_v3_publication_intent_v1,
    retire_worker_v3_publication_intent_after_load_readiness_v1,
    scavenge_superseded_worker_v3_load_readiness_v1,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::fs;
use std::num::NonZeroU8;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const CHILD_OUTPUT: &str = "FE2O3_LOAD_READY_CHILD_OUTPUT";
const ABRUPT_OUTPUT: &str = "FE2O3_LOAD_READY_ABRUPT_OUTPUT";
const ABRUPT_CLAIM: &str = "FE2O3_LOAD_READY_ABRUPT_CLAIM";
const ABRUPT_ENVELOPE: &str = "FE2O3_LOAD_READY_ABRUPT_ENVELOPE";
const ABRUPT_BOUNDARY: &str = "FE2O3_LOAD_READY_ABRUPT_BOUNDARY";
const ABRUPT_TIMING: &str = "FE2O3_LOAD_READY_ABRUPT_TIMING";
const ABRUPT_EXIT_CODE: u8 = 86;

const READINESS_BOUNDARIES: [WorkerV3LoadReadinessBoundaryV1; 16] = [
    WorkerV3LoadReadinessBoundaryV1::CreateEnvelopeTemp,
    WorkerV3LoadReadinessBoundaryV1::WriteEnvelopeTemp,
    WorkerV3LoadReadinessBoundaryV1::SyncEnvelopeTemp,
    WorkerV3LoadReadinessBoundaryV1::RenameEnvelope,
    WorkerV3LoadReadinessBoundaryV1::SyncEnvelopeName,
    WorkerV3LoadReadinessBoundaryV1::CreateClaimTemp,
    WorkerV3LoadReadinessBoundaryV1::WriteClaimTemp,
    WorkerV3LoadReadinessBoundaryV1::SyncClaimTemp,
    WorkerV3LoadReadinessBoundaryV1::RenameClaim,
    WorkerV3LoadReadinessBoundaryV1::SyncClaimName,
    WorkerV3LoadReadinessBoundaryV1::CreateReceiptTemp,
    WorkerV3LoadReadinessBoundaryV1::WriteReceiptTemp,
    WorkerV3LoadReadinessBoundaryV1::SyncReceiptTemp,
    WorkerV3LoadReadinessBoundaryV1::RenameReceipt,
    WorkerV3LoadReadinessBoundaryV1::SyncReceiptName,
    WorkerV3LoadReadinessBoundaryV1::CommitAttemptRegistry,
];

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
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
        load_authority(&self.envelope, &self.claim)
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

fn load_authority(
    bytes: &[u8],
    claim: &DurablePublishedHsacoClaimV3,
) -> VerifiedWorkerV3LoadEnvelopeAuthorityV1 {
    let binding = WorkerV3LoadEnvelopeBindingV1::from_exact_bytes(bytes).unwrap();
    // SAFETY: these transaction tests model the upstream boundary that has verified the opaque
    // envelope contains every compact replay preimage. They never treat the result as load-ready.
    unsafe {
        VerifiedWorkerV3LoadEnvelopeAuthorityV1::from_complete_compact_replay_preimages_unchecked(
            binding, claim,
        )
        .unwrap()
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

fn publication_artifact(output: &Path) -> PathBuf {
    let mut matches = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(".fe2o3-link-artifact-v1-") && name.ends_with(".bin")
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one published artifact");
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
fn readiness_debug_is_bounded_and_omits_exact_envelope_bytes() {
    let state = setup(2);
    let readiness = state.publish_readiness();
    let debug = format!("{readiness:?}");
    assert!(debug.len() < 4096, "unexpectedly large debug output");
    assert!(debug.contains("envelope_length"));
    assert!(debug.contains(&state.envelope.len().to_string()));
    assert!(!debug.contains(std::str::from_utf8(&state.envelope).unwrap()));
}

#[test]
fn crash_sides_reconcile_to_one_exact_terminal_receipt() {
    let mut index = 0_u8;
    for boundary in READINESS_BOUNDARIES {
        for timing in [
            WorkerV3LoadReadinessFaultTimingV1::Before,
            WorkerV3LoadReadinessFaultTimingV1::After,
        ] {
            let point = WorkerV3LoadReadinessFaultPointV1 { boundary, timing };
            let state = setup(10 + index);
            index += 1;
            let error = publish_worker_v3_load_readiness_v1_with_options(
                &state.output(),
                &state.claim,
                state.authority(),
                state.envelope.clone(),
                WorkerV3LoadReadinessOptionsV1::inject_crash(point),
            )
            .unwrap_err();
            assert!(
                matches!(error, WorkerV3LoadReadinessErrorV1::InjectedCrash(actual) if actual == point)
            );
            let retry = state.publish_readiness();
            assert_eq!(retry.exact_envelope_bytes(), state.envelope);
            recover_worker_v3_load_readiness_v1(&state.output(), &state.claim).unwrap();
        }
    }
}

#[test]
fn abrupt_crash_child_terminates_at_one_exact_boundary() {
    let Ok(output) = std::env::var(ABRUPT_OUTPUT) else {
        return;
    };
    fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
    let claim = DurablePublishedHsacoClaimV3::decode_canonical(
        &fs::read(std::env::var(ABRUPT_CLAIM).unwrap()).unwrap(),
    )
    .unwrap();
    let envelope = fs::read(std::env::var(ABRUPT_ENVELOPE).unwrap()).unwrap();
    let boundary = READINESS_BOUNDARIES[std::env::var(ABRUPT_BOUNDARY)
        .unwrap()
        .parse::<usize>()
        .unwrap()];
    let timing = match std::env::var(ABRUPT_TIMING).unwrap().as_str() {
        "before" => WorkerV3LoadReadinessFaultTimingV1::Before,
        "after" => WorkerV3LoadReadinessFaultTimingV1::After,
        value => panic!("unexpected timing {value}"),
    };
    let authority = load_authority(&envelope, &claim);
    let point = WorkerV3LoadReadinessFaultPointV1 { boundary, timing };
    let _ = publish_worker_v3_load_readiness_v1_with_options(
        Path::new(&output),
        &claim,
        authority,
        envelope,
        WorkerV3LoadReadinessOptionsV1::inject_abrupt_exit(
            point,
            NonZeroU8::new(ABRUPT_EXIT_CODE).unwrap(),
        ),
    );
    panic!("abrupt fault injection returned instead of terminating");
}

#[test]
fn every_boundary_survives_abrupt_process_death_and_exact_retry() {
    if std::env::var_os(ABRUPT_OUTPUT).is_some() {
        return;
    }
    let mut seed = 100_u8;
    for (boundary_index, _) in READINESS_BOUNDARIES.iter().enumerate() {
        for (timing_name, timing) in [
            ("before", WorkerV3LoadReadinessFaultTimingV1::Before),
            ("after", WorkerV3LoadReadinessFaultTimingV1::After),
        ] {
            let state = setup(seed);
            seed = seed.wrapping_add(1);
            let claim_path = state.directory.0.join("abrupt-claim.v3");
            let envelope_path = state.directory.0.join("abrupt-envelope.v3");
            fs::write(&claim_path, state.claim.encode_canonical().unwrap()).unwrap();
            fs::write(&envelope_path, &state.envelope).unwrap();
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .arg("--exact")
                .arg("abrupt_crash_child_terminates_at_one_exact_boundary")
                .arg("--nocapture")
                .env(ABRUPT_OUTPUT, state.output())
                .env(ABRUPT_CLAIM, claim_path)
                .env(ABRUPT_ENVELOPE, envelope_path)
                .env(ABRUPT_BOUNDARY, boundary_index.to_string())
                .env(ABRUPT_TIMING, timing_name);
            let status = test_process::status(&mut command).unwrap();
            assert_eq!(
                status.code(),
                Some(i32::from(ABRUPT_EXIT_CODE)),
                "unexpected subprocess status at {:?} {timing:?}",
                READINESS_BOUNDARIES[boundary_index]
            );
            let retried = state.publish_readiness();
            assert_eq!(retried.exact_envelope_bytes(), state.envelope);
            recover_worker_v3_load_readiness_v1(&state.output(), &state.claim).unwrap();
        }
    }
}

#[test]
fn wrong_authority_claim_envelope_and_stale_generation_fail_closed() {
    let state = setup(30);
    let wrong_authority = load_authority(b"different envelope", &state.claim);
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
        Err(WorkerV3LoadReadinessErrorV1::AuthorityMismatch)
    ));

    state.publish_readiness();
    let wrong_bytes = b"substituted retry envelope".to_vec();
    assert!(matches!(
        publish_worker_v3_load_readiness_v1(
            &state.output(),
            &state.claim,
            load_authority(&wrong_bytes, &state.claim),
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

    let missing_claim = setup(42);
    missing_claim.publish_readiness();
    fs::remove_file(readiness_entry(&missing_claim.output(), ".claim")).unwrap();
    assert!(matches!(
        recover_worker_v3_load_readiness_for_attempt_v1(
            &missing_claim.output(),
            missing_claim.attempt,
        ),
        Err(WorkerV3LoadReadinessErrorV1::MissingClaim)
    ));

    let substituted = setup(43);
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

    let substituted_claim = setup(44);
    substituted_claim.publish_readiness();
    let claim_path = readiness_entry(&substituted_claim.output(), ".claim");
    let claim_bytes = fs::read(&claim_path).unwrap();
    fs::remove_file(&claim_path).unwrap();
    fs::write(&claim_path, claim_bytes).unwrap();
    fs::set_permissions(&claim_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        recover_worker_v3_load_readiness_for_attempt_v1(
            &substituted_claim.output(),
            substituted_claim.attempt,
        ),
        Err(WorkerV3LoadReadinessErrorV1::ReceiptMismatch)
    ));
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
fn retirement_preserves_intent_when_the_separate_publication_artifact_is_not_exact() {
    for (seed, mutation) in [
        (60, "missing"),
        (61, "replaced-inode"),
        (62, "mutated-bytes"),
    ] {
        let state = setup(seed);
        let readiness = state.publish_readiness();
        let artifact = publication_artifact(&state.output());
        let exact = fs::read(&artifact).unwrap();
        match mutation {
            "missing" => fs::remove_file(&artifact).unwrap(),
            "replaced-inode" => {
                let replacement = artifact.with_extension("replacement");
                fs::write(&replacement, &exact).unwrap();
                fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
                fs::rename(replacement, &artifact).unwrap();
            }
            "mutated-bytes" => {
                let mut changed = exact;
                changed[0] ^= 1;
                fs::write(&artifact, changed).unwrap();
            }
            _ => unreachable!(),
        }

        let retirement = retire_worker_v3_publication_intent_after_load_readiness_v1(
            &state.output(),
            &state.producer,
            state.attempt,
            state.intent,
            readiness.receipt(),
        );
        assert!(
            matches!(
                retirement,
                Err(WorkerV3PublicationIntentErrorV1::LoadReadiness(
                    WorkerV3LoadReadinessErrorV1::Claim(_)
                )) | Err(WorkerV3PublicationIntentErrorV1::ReceiptNotDurable)
            ),
            "unexpected {mutation} retirement result: {retirement:?}"
        );
        assert!(
            recover_worker_v3_publication_intent_v1(
                &state.output(),
                &state.producer,
                state.attempt,
            )
            .is_ok()
        );
    }
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
    fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
    let attempts = discover_worker_v3_load_readiness_attempts_v1(Path::new(&output)).unwrap();
    let [attempt] = attempts.as_slice() else {
        panic!("expected one discoverable custody attempt, got {attempts:?}");
    };
    let recovered =
        recover_worker_v3_load_readiness_for_attempt_v1(Path::new(&output), *attempt).unwrap();
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
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("process_restart_child_revalidates_terminal_custody")
        .arg("--nocapture")
        .env(CHILD_OUTPUT, state.output());
    let status = test_process::status(&mut command).unwrap();
    assert!(status.success());
}

#[test]
fn superseded_custody_is_scavenged_but_current_custody_is_retained() {
    let state = setup(70);
    let readiness = state.publish_readiness();
    assert_eq!(
        discover_worker_v3_load_readiness_attempts_v1(&state.output()).unwrap(),
        vec![state.attempt]
    );
    assert_eq!(
        scavenge_superseded_worker_v3_load_readiness_v1(&state.output()).unwrap(),
        0
    );
    recover_worker_v3_load_readiness_v1(&state.output(), &state.claim).unwrap();
    let claim_path = readiness_entry(&state.output(), ".claim");
    let receipt_path = readiness_entry(&state.output(), ".receipt");

    begin_build_attempt(
        &state.output(),
        &state.producer,
        BuildInvocation::from_bytes(identity(0xe1)),
        BuildSession::from_bytes([0xe2; 16]),
    )
    .unwrap();
    assert!(
        discover_worker_v3_load_readiness_attempts_v1(&state.output())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        scavenge_superseded_worker_v3_load_readiness_v1(&state.output()).unwrap(),
        3
    );
    assert!(!readiness.envelope_path().exists());
    assert!(!claim_path.exists());
    assert!(!receipt_path.exists());
}
