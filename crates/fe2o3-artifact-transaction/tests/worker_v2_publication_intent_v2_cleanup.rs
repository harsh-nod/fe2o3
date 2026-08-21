use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationBoundaryV2,
    AttemptScopedHsacoPublicationErrorV2, AttemptScopedHsacoPublicationFaultPointV2,
    AttemptScopedHsacoPublicationFaultTimingV2, AttemptScopedHsacoPublicationOptionsV2,
    BuildAttempt, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
    DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1, PackageIdentityV1,
    PersistedBackendReceiptV2, PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1,
    WorkerV2PublicationIntentBoundaryV2, WorkerV2PublicationIntentErrorV1,
    WorkerV2PublicationIntentErrorV2, WorkerV2PublicationIntentFaultPointV2,
    WorkerV2PublicationIntentFaultTimingV2, WorkerV2PublicationIntentIdentityV2,
    WorkerV2PublicationIntentOptionsV2, begin_build_attempt, clear_worker_v2_publication_intent_v1,
    clear_worker_v2_publication_intent_v2, emit_artifact_transaction_for_attempt,
    finish_build_attempt, persist_worker_v2_publication_intent_v1,
    persist_worker_v2_publication_intent_v2, persist_worker_v2_publication_intent_v2_with_options,
    publish_exact_hsaco_evidence_for_attempt_v1, publish_exact_hsaco_evidence_for_attempt_v2,
    publish_exact_hsaco_evidence_for_attempt_v2_with_options, read_backend_publication_receipt_v2,
    recover_worker_v2_publication_intent_v1, recover_worker_v2_publication_intent_v2,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const INTENT_PREFIX: &str = ".fe2o3-worker-v2-publication-intent-";
const V1_PREFIX: &str = ".fe2o3-worker-v2-publication-intent-v1-";
const V2_PREFIX: &str = ".fe2o3-worker-v2-publication-intent-v2-";

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        loop {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-worker-v2-intent-cleanup-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create test directory: {error}"),
            }
        }
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

fn identity(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn producer(source: &str) -> ProducerIdentity {
    ProducerIdentity::from_codegen("protected_kernel", Some(Path::new(source))).unwrap()
}

fn begin(output: &Path, owner: &ProducerIdentity, seed: u8) -> BuildAttempt {
    begin_build_attempt(
        output,
        owner,
        BuildInvocation::from_bytes(identity(seed.wrapping_add(1))),
        BuildSession::from_bytes([seed; 16]),
    )
    .unwrap()
}

fn plan(attempt: BuildAttempt, seed: u8, bytes: &[u8]) -> DurableLinkPublicationPlanV1 {
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
        LinkedOutputIdentityV1::from_bytes(identity(seed.wrapping_add(6))),
        FinalizationIdentityV1::from_bytes(identity(seed.wrapping_add(7))),
        FinalizedOutputIdentityV1::from_bytes(Sha256::digest(bytes).into()),
        AtomicPublicationIdentityV1::from_bytes(identity(seed.wrapping_add(8))),
    )
}

fn upstream(seed: u8) -> UpstreamCodeObjectEvidenceIdentityV1 {
    UpstreamCodeObjectEvidenceIdentityV1::from_bytes(identity(seed))
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

fn intent_snapshot(output: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(INTENT_PREFIX)
                .then(|| (name, fs::read(entry.path()).unwrap()))
        })
        .collect()
}

fn schema_snapshot(output: &Path, prefix: &str) -> BTreeMap<String, Vec<u8>> {
    intent_snapshot(output)
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .collect()
}

fn publish_v2(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    publication_plan: DurableLinkPublicationPlanV1,
    evidence: UpstreamCodeObjectEvidenceIdentityV1,
    closure: CompilerClosureV2,
    bytes: &[u8],
) {
    publish_exact_hsaco_evidence_for_attempt_v2(
        output,
        owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    )
    .unwrap();
}

#[test]
fn fresh_and_completed_recovery_cleanup_require_exact_v2_receipts() {
    for completed in [false, true] {
        let temp = TestDirectory::new();
        let output = temp.output();
        let owner = producer(if completed {
            "/src/v2-cleanup-completed.rs"
        } else {
            "/src/v2-cleanup-fresh.rs"
        });
        let attempt = begin(&output, &owner, if completed { 0x12 } else { 0x11 });
        let bytes = b"exact protected output";
        let publication_plan = plan(attempt, 0x21, bytes);
        let evidence = upstream(0x31);
        let closure = compiler_closure(0x41);
        let persisted = persist_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            bytes,
        )
        .unwrap();

        publish_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            bytes,
        );
        if completed {
            finish_build_attempt(&output, &owner, attempt).unwrap();
        }
        let recovered =
            recover_worker_v2_publication_intent_v2(&output, &owner, attempt, closure).unwrap();
        assert_eq!(recovered.record(), persisted.record());
        clear_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            closure,
            persisted.record().identity(),
        )
        .unwrap();
        assert!(matches!(
            recover_worker_v2_publication_intent_v2(&output, &owner, attempt, closure),
            Err(WorkerV2PublicationIntentErrorV2::NotFound)
        ));
    }
}

#[test]
fn none_pending_and_crash_stage_rejections_do_not_mutate_intents() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("/src/v2-cleanup-pending.rs");
    let attempt = begin(&output, &owner, 0x51);
    let bytes = b"pending protected output";
    let publication_plan = plan(attempt, 0x52, bytes);
    let evidence = upstream(0x53);
    let closure = compiler_closure(0x54);
    let persisted = persist_worker_v2_publication_intent_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    )
    .unwrap();

    let record_name = schema_snapshot(&output, V2_PREFIX)
        .into_keys()
        .find(|name| name.ends_with(".record"))
        .unwrap();
    let redo_name = format!("{record_name}.redo");
    fs::rename(output.join(&record_name), output.join(&redo_name)).unwrap();
    let temp_name = format!(
        "{}.tmp-crash-stage",
        record_name.trim_end_matches(".record")
    );
    fs::write(output.join(&temp_name), b"private crash residue").unwrap();
    fs::set_permissions(output.join(&temp_name), fs::Permissions::from_mode(0o600)).unwrap();

    let before_none = intent_snapshot(&output);
    assert!(matches!(
        clear_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            closure,
            persisted.record().identity(),
        ),
        Err(WorkerV2PublicationIntentErrorV2::Attempt { .. })
    ));
    assert_eq!(intent_snapshot(&output), before_none);

    let point = AttemptScopedHsacoPublicationFaultPointV2 {
        boundary: AttemptScopedHsacoPublicationBoundaryV2::CommitPendingReceipt,
        timing: AttemptScopedHsacoPublicationFaultTimingV2::After,
    };
    assert!(matches!(
        publish_exact_hsaco_evidence_for_attempt_v2_with_options(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            bytes,
            AttemptScopedHsacoPublicationOptionsV2::inject_receipt_crash(point),
        ),
        Err(AttemptScopedHsacoPublicationErrorV2::ReceiptCommitInterrupted { point: actual })
            if actual == point
    ));
    assert!(matches!(
        read_backend_publication_receipt_v2(&output, &owner, attempt).unwrap(),
        PersistedBackendReceiptV2::PendingProvenance(_)
    ));
    let before_pending = intent_snapshot(&output);
    assert!(matches!(
        clear_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            closure,
            persisted.record().identity(),
        ),
        Err(WorkerV2PublicationIntentErrorV2::Attempt { .. })
    ));
    assert_eq!(intent_snapshot(&output), before_pending);

    publish_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    );
    clear_worker_v2_publication_intent_v2(
        &output,
        &owner,
        attempt,
        closure,
        persisted.record().identity(),
    )
    .unwrap();
    assert!(schema_snapshot(&output, V2_PREFIX).is_empty());
}

#[test]
fn legacy_and_v1_provenance_receipts_cannot_clear_v2_intents() {
    let cases = ["legacy", "v1-provenance"];
    for (index, case) in cases.into_iter().enumerate() {
        let temp = TestDirectory::new();
        let output = temp.output();
        let owner = producer(&format!("/src/v2-cleanup-{case}.rs"));
        let attempt = begin(&output, &owner, 0x61 + index as u8);
        let bytes = b"cross-schema protected output";
        let publication_plan = plan(attempt, 0x63, bytes);
        let evidence = upstream(0x64);
        let closure = compiler_closure(0x65);
        let persisted = persist_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            bytes,
        )
        .unwrap();

        if case == "legacy" {
            emit_artifact_transaction_for_attempt(
                &output,
                &owner,
                attempt,
                &["legacy_kernel"],
                |name| name,
                |name| {
                    Ok(format!(
                        "define amdgpu_kernel void @{name}() {{ ret void }}"
                    ))
                },
                |llvm_ir, hsaco| {
                    let ir = fs::read(llvm_ir)?;
                    fs::write(hsaco.with_extension("o"), &ir)?;
                    fs::write(hsaco, ir)?;
                    Ok(())
                },
            )
            .unwrap();
        } else {
            publish_exact_hsaco_evidence_for_attempt_v1(
                &output,
                &owner,
                attempt,
                publication_plan,
                evidence,
                bytes,
            )
            .unwrap();
        }
        let before = intent_snapshot(&output);
        assert!(matches!(
            clear_worker_v2_publication_intent_v2(
                &output,
                &owner,
                attempt,
                closure,
                persisted.record().identity(),
            ),
            Err(WorkerV2PublicationIntentErrorV2::Attempt { .. })
        ));
        assert_eq!(intent_snapshot(&output), before);
    }
}

#[test]
fn every_compiler_closure_role_requires_an_exact_v2_receipt() {
    for role in 0..6 {
        let temp = TestDirectory::new();
        let output = temp.output();
        let owner = producer(&format!("/src/v2-cleanup-closure-{role}.rs"));
        let attempt = begin(&output, &owner, 0x71 + role as u8);
        let bytes = b"closure role protected output";
        let publication_plan = plan(attempt, 0x79, bytes);
        let evidence = upstream(0x7a);
        let closure = compiler_closure(0x81);
        let receipt_closure = substitute_closure_role(closure, compiler_closure(0x91), role);
        let persisted = persist_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            bytes,
        )
        .unwrap();
        publish_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            receipt_closure,
            bytes,
        );
        let before = intent_snapshot(&output);
        assert!(matches!(
            clear_worker_v2_publication_intent_v2(
                &output,
                &owner,
                attempt,
                closure,
                persisted.record().identity(),
            ),
            Err(WorkerV2PublicationIntentErrorV2::Attempt { .. })
        ));
        assert_eq!(intent_snapshot(&output), before, "closure role {role}");
    }
}

#[test]
fn plan_upstream_and_output_receipt_substitutions_do_not_mutate_intents() {
    for case in ["plan", "upstream", "output"] {
        let temp = TestDirectory::new();
        let output = temp.output();
        let owner = producer(&format!("/src/v2-cleanup-{case}-substitution.rs"));
        let attempt = begin(&output, &owner, 0xa1);
        let bytes = b"intended protected output";
        let publication_plan = plan(attempt, 0xa2, bytes);
        let evidence = upstream(0xa3);
        let closure = compiler_closure(0xa4);
        let persisted = persist_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            bytes,
        )
        .unwrap();

        let substituted_bytes: &[u8] = if case == "output" {
            b"substituted protected output"
        } else {
            bytes
        };
        let receipt_plan = if matches!(case, "plan" | "output") {
            plan(attempt, 0xb2, substituted_bytes)
        } else {
            publication_plan
        };
        let receipt_evidence = if case == "upstream" {
            upstream(0xb3)
        } else {
            evidence
        };
        publish_v2(
            &output,
            &owner,
            attempt,
            receipt_plan,
            receipt_evidence,
            closure,
            substituted_bytes,
        );
        let before = intent_snapshot(&output);
        assert!(matches!(
            clear_worker_v2_publication_intent_v2(
                &output,
                &owner,
                attempt,
                closure,
                persisted.record().identity(),
            ),
            Err(WorkerV2PublicationIntentErrorV2::Attempt { .. })
        ));
        assert_eq!(intent_snapshot(&output), before, "{case} substitution");
    }
}

#[test]
fn attempt_producer_and_intent_identity_substitutions_do_not_mutate_intents() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("/src/v2-cleanup-binding-substitution.rs");
    let attempt = begin(&output, &owner, 0xc1);
    let bytes = b"binding protected output";
    let publication_plan = plan(attempt, 0xc2, bytes);
    let evidence = upstream(0xc3);
    let closure = compiler_closure(0xc4);
    let persisted = persist_worker_v2_publication_intent_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    )
    .unwrap();
    publish_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    );
    let before = intent_snapshot(&output);

    let foreign = producer("/src/v2-cleanup-foreign-producer.rs");
    assert!(
        clear_worker_v2_publication_intent_v2(
            &output,
            &foreign,
            attempt,
            closure,
            persisted.record().identity(),
        )
        .is_err()
    );
    let wrong_attempt = BuildAttempt::from_env_value(&format!(
        "{}:{}:{}",
        attempt.generation(),
        BuildSession::from_bytes([0xd1; 16]).to_hex(),
        BuildInvocation::from_bytes(identity(0xd2)).to_hex(),
    ))
    .unwrap();
    assert!(
        clear_worker_v2_publication_intent_v2(
            &output,
            &owner,
            wrong_attempt,
            closure,
            persisted.record().identity(),
        )
        .is_err()
    );
    assert!(matches!(
        clear_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            closure,
            WorkerV2PublicationIntentIdentityV2::from_bytes(identity(0xff)),
        ),
        Err(WorkerV2PublicationIntentErrorV2::IntentIdentityMismatch)
    ));
    assert_eq!(intent_snapshot(&output), before);
}

#[test]
fn v1_cleanup_and_wire_state_remain_schema_separated() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("/src/v1-cleanup-canary.rs");
    let attempt = begin(&output, &owner, 0xe1);
    let bytes = b"immutable V1 cleanup canary";
    let publication_plan = plan(attempt, 0xe2, bytes);
    let evidence = upstream(0xe3);
    let closure = compiler_closure(0xe4);
    let v1 = persist_worker_v2_publication_intent_v1(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        bytes,
    )
    .unwrap();
    let v2 = persist_worker_v2_publication_intent_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    )
    .unwrap();
    let v1_before = schema_snapshot(&output, V1_PREFIX);
    let record = v1_before
        .iter()
        .find(|(name, _)| name.ends_with(".record"))
        .unwrap()
        .1;
    assert_eq!(record.len(), 616);

    publish_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    );
    assert!(matches!(
        clear_worker_v2_publication_intent_v1(&output, &owner, attempt, v1.record().identity()),
        Err(WorkerV2PublicationIntentErrorV1::Attempt { .. })
    ));
    assert_eq!(schema_snapshot(&output, V1_PREFIX), v1_before);
    assert_eq!(
        recover_worker_v2_publication_intent_v1(&output, &owner, attempt)
            .unwrap()
            .record(),
        v1.record()
    );

    clear_worker_v2_publication_intent_v2(
        &output,
        &owner,
        attempt,
        closure,
        v2.record().identity(),
    )
    .unwrap();
    assert_eq!(schema_snapshot(&output, V1_PREFIX), v1_before);
}

#[test]
fn v2_intent_persistence_crash_recovery_still_cleans_after_exact_publication() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("/src/v2-cleanup-intent-crash.rs");
    let attempt = begin(&output, &owner, 0xf1);
    let bytes = b"intent crash protected output";
    let publication_plan = plan(attempt, 0xf2, bytes);
    let evidence = upstream(0xf3);
    let closure = compiler_closure(0xf4);
    let point = WorkerV2PublicationIntentFaultPointV2 {
        boundary: WorkerV2PublicationIntentBoundaryV2::RenameRecordToRedo,
        timing: WorkerV2PublicationIntentFaultTimingV2::After,
    };
    assert!(matches!(
        persist_worker_v2_publication_intent_v2_with_options(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            bytes,
            WorkerV2PublicationIntentOptionsV2::inject_crash(point),
        ),
        Err(WorkerV2PublicationIntentErrorV2::InjectedCrash { point: actual })
            if actual == point
    ));
    let recovered = persist_worker_v2_publication_intent_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    )
    .unwrap();
    publish_v2(
        &output,
        &owner,
        attempt,
        publication_plan,
        evidence,
        closure,
        bytes,
    );
    clear_worker_v2_publication_intent_v2(
        &output,
        &owner,
        attempt,
        closure,
        recovered.record().identity(),
    )
    .unwrap();
    assert!(schema_snapshot(&output, V2_PREFIX).is_empty());
}
