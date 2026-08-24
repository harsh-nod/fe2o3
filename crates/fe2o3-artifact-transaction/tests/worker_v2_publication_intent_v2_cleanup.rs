use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, AttemptScopedHsacoPublicationBoundaryV2,
    AttemptScopedHsacoPublicationErrorV2, AttemptScopedHsacoPublicationFaultPointV2,
    AttemptScopedHsacoPublicationFaultTimingV2, AttemptScopedHsacoPublicationOptionsV2,
    BuildAttempt, BuildInvocation, BuildSession, CanonicalLinkRequestIdentityV1,
    DurableLinkPublicationPlanV1, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1, PackageIdentityV1,
    PersistedBackendReceiptV2, PinnedWorkerIdentityV1, ProducerIdentity,
    RecoveredWorkerV2PublicationIntentV2, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
    ValidatedResponseIdentityV1, WorkerV2PublicationIntentBoundaryV2,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1,
    WorkerV2PublicationIntentCleanupEscrowErrorV1,
    WorkerV2PublicationIntentCleanupEscrowFaultPointV1,
    WorkerV2PublicationIntentCleanupEscrowFaultTimingV1,
    WorkerV2PublicationIntentCleanupEscrowOptionsV1, WorkerV2PublicationIntentCleanupEscrowStateV1,
    WorkerV2PublicationIntentCleanupEscrowV1, WorkerV2PublicationIntentErrorV1,
    WorkerV2PublicationIntentErrorV2, WorkerV2PublicationIntentFaultPointV2,
    WorkerV2PublicationIntentFaultTimingV2, WorkerV2PublicationIntentIdentityV2,
    WorkerV2PublicationIntentOptionsV2, acquire_worker_v2_publication_intent_lease_v2,
    begin_build_attempt, clear_worker_v2_publication_intent_v1,
    clear_worker_v2_publication_intent_v2,
    commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2,
    commit_worker_v2_publication_intent_cleanup_escrow_v1,
    commit_worker_v2_publication_intent_cleanup_escrow_v1_with_options,
    emit_artifact_transaction_for_attempt, finish_build_attempt,
    persist_worker_v2_publication_intent_v1, persist_worker_v2_publication_intent_v2,
    persist_worker_v2_publication_intent_v2_with_options,
    prepare_worker_v2_publication_intent_cleanup_escrow_v1,
    prepare_worker_v2_publication_intent_cleanup_escrow_v1_with_options,
    producer_package_identity_v1, publish_exact_hsaco_evidence_for_attempt_v1,
    publish_exact_hsaco_evidence_for_attempt_v2,
    publish_exact_hsaco_evidence_for_attempt_v2_with_options, read_backend_publication_receipt_v2,
    recover_worker_v2_publication_intent_cleanup_escrow_v1,
    recover_worker_v2_publication_intent_v1, recover_worker_v2_publication_intent_v2,
    rollback_worker_v2_publication_intent_cleanup_escrow_v1,
    rollback_worker_v2_publication_intent_cleanup_escrow_v1_with_options,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const INTENT_PREFIX: &str = ".fe2o3-worker-v2-publication-intent-";
const V1_PREFIX: &str = ".fe2o3-worker-v2-publication-intent-v1-";
const V2_PREFIX: &str = ".fe2o3-worker-v2-publication-intent-v2-";
const ESCROW_PREFIX: &str = ".fe2o3-worker-v2-intent-cleanup-escrow-v1-";
const ESCROW_CRASH_HELPER_OPERATION: &str = "FE2O3_ESCROW_CRASH_HELPER_OPERATION";
const ESCROW_CRASH_HELPER_OUTPUT: &str = "FE2O3_ESCROW_CRASH_HELPER_OUTPUT";
const ESCROW_CRASH_HELPER_SOURCE: &str = "FE2O3_ESCROW_CRASH_HELPER_SOURCE";
const ESCROW_CRASH_HELPER_ATTEMPT: &str = "FE2O3_ESCROW_CRASH_HELPER_ATTEMPT";
const ESCROW_CRASH_HELPER_CLOSURE_SEED: &str = "FE2O3_ESCROW_CRASH_HELPER_CLOSURE_SEED";
const ESCROW_CRASH_HELPER_CAPSULE: &str = "FE2O3_ESCROW_CRASH_HELPER_CAPSULE";
const ESCROW_CRASH_HELPER_BOUNDARY: &str = "FE2O3_ESCROW_CRASH_HELPER_BOUNDARY";
const ESCROW_CRASH_HELPER_TIMING: &str = "FE2O3_ESCROW_CRASH_HELPER_TIMING";
const ESCROW_CAPSULE_MAGIC: &[u8] = b"FE2O3-WORKER-V2-INTENT-CLEANUP-ESCROW-CAPSULE-V1\0";
const ESCROW_CAPSULE_CHECKSUM_DOMAIN: &[u8] =
    b"fe2o3.worker-v2-intent-cleanup-escrow.capsule-checksum.v1\0";
const ESCROW_RECEIPT_IDENTITY_DOMAIN: &[u8] =
    b"fe2o3.worker-v2-intent-cleanup-escrow.receipt-identity.v1\0";
// Mirrors the crate-private artifact-store bound for public lifecycle boundary tests.
const CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES: usize = 4096 * 7;
const _: () = assert!(MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1 <= 2 * 1024);

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
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

fn escrow_plan(
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
    seed: u8,
    bytes: &[u8],
) -> DurableLinkPublicationPlanV1 {
    DurableLinkPublicationPlanV1::new(
        attempt,
        LinkPublicationScopeV1::new(
            producer_package_identity_v1(owner),
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

fn exact_v2_receipt(
    output: &Path,
    owner: &ProducerIdentity,
    attempt: BuildAttempt,
) -> fe2o3_artifact_transaction::BackendPublicationReceiptV2 {
    match read_backend_publication_receipt_v2(output, owner, attempt).unwrap() {
        PersistedBackendReceiptV2::Provenance(receipt) => receipt,
        receipt => panic!("expected durable V2 provenance, got {receipt:?}"),
    }
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

const PREPARE_ESCROW_BOUNDARIES: &[WorkerV2PublicationIntentCleanupEscrowBoundaryV1] = &[
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameRecordToQuarantine,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedRecordName,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameOutputToQuarantine,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedOutputName,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::CreateManifestTemp,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::WriteManifestTemp,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncManifestTemp,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameManifest,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncManifestName,
];
const COMMIT_ESCROW_BOUNDARIES: &[WorkerV2PublicationIntentCleanupEscrowBoundaryV1] = &[
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::CreateManifestTemp,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::WriteManifestTemp,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncManifestTemp,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameManifest,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncManifestName,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkQuarantinedRecord,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedRecordDeletion,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkQuarantinedOutput,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncQuarantinedOutputDeletion,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkCommittedManifest,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncCommittedManifestDeletion,
];
const ROLLBACK_ESCROW_BOUNDARIES: &[WorkerV2PublicationIntentCleanupEscrowBoundaryV1] = &[
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameOutputToCanonical,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncCanonicalOutputName,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameRecordToCanonical,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncCanonicalRecordName,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkRolledBackManifest,
    WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncRolledBackManifestDeletion,
];

fn escrow_entry_names(output: &Path) -> Vec<String> {
    let mut names = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(ESCROW_PREFIX))
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn output_entry_count(output: &Path) -> usize {
    fs::read_dir(output).unwrap().count()
}

fn fill_output_with_unrelated_entries(output: &Path, target_entries: usize) {
    let existing = output_entry_count(output);
    assert!(existing <= target_entries);
    for index in existing..target_entries {
        fs::File::create(output.join(format!("cleanup-capacity-unrelated-{index}"))).unwrap();
    }
    assert_eq!(output_entry_count(output), target_entries);
}

fn escrow_entry_with_suffix(output: &Path, suffix: &str) -> PathBuf {
    let names = escrow_entry_names(output);
    let matches = names
        .iter()
        .filter(|name| name.ends_with(suffix))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "unexpected escrow entries: {names:?}");
    output.join(matches[0])
}

fn schema_entry_with_suffix(output: &Path, prefix: &str, suffix: &str) -> PathBuf {
    let matches = fs::read_dir(output)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(prefix) && name.ends_with(suffix)
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "unexpected schema entries: {matches:?}");
    matches.into_iter().next().unwrap()
}

fn sha256_domain_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    for part in parts {
        digest.update(part);
    }
    digest.finalize().into()
}

fn reseal_cleanup_capsule(bytes: &mut [u8]) {
    let body_len = bytes.len() - 32;
    let checksum = sha256_domain_parts(ESCROW_CAPSULE_CHECKSUM_DOMAIN, &[&bytes[..body_len]]);
    bytes[body_len..].copy_from_slice(&checksum);
}

struct CleanupEscrowFixture {
    _temp: TestDirectory,
    output: PathBuf,
    source: String,
    owner: ProducerIdentity,
    attempt: BuildAttempt,
    closure_seed: u8,
    closure: CompilerClosureV2,
    intent: WorkerV2PublicationIntentIdentityV2,
    receipt: fe2o3_artifact_transaction::BackendPublicationReceiptV2,
}

impl CleanupEscrowFixture {
    fn new(seed: u8, label: &str) -> Self {
        let temp = TestDirectory::new();
        let output = temp.output();
        let source = format!("/src/v2-cleanup-escrow-{label}-{seed:02x}.rs");
        let owner = producer(&source);
        let _superseded = begin(&output, &owner, seed);
        let attempt = begin(&output, &owner, seed.wrapping_add(0x20));
        let bytes = format!("artifact-owned escrow fixture {label} {seed:02x}").into_bytes();
        let publication_plan = escrow_plan(&owner, attempt, seed.wrapping_add(1), &bytes);
        let evidence = upstream(seed.wrapping_add(2));
        let closure_seed = seed.wrapping_add(3);
        let closure = compiler_closure(closure_seed);
        let persisted = persist_worker_v2_publication_intent_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            &bytes,
        )
        .unwrap();
        publish_v2(
            &output,
            &owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            &bytes,
        );
        let receipt = exact_v2_receipt(&output, &owner, attempt);
        Self {
            _temp: temp,
            output,
            source,
            owner,
            attempt,
            closure_seed,
            closure,
            intent: persisted.record().identity(),
            receipt,
        }
    }

    fn prepare(&self) -> WorkerV2PublicationIntentCleanupEscrowV1 {
        prepare_worker_v2_publication_intent_cleanup_escrow_v1(
            &self.output,
            &self.owner,
            self.attempt,
            self.closure,
            self.intent,
            self.receipt,
        )
        .unwrap()
    }

    fn successor(&self, seed: u8) -> ExactSuccessor {
        let attempt = begin(&self.output, &self.owner, seed);
        let bytes = format!("exact successor output {seed:02x}").into_bytes();
        let publication_plan = escrow_plan(&self.owner, attempt, seed.wrapping_add(1), &bytes);
        let evidence = upstream(seed.wrapping_add(2));
        let closure = compiler_closure(seed.wrapping_add(3));
        let persisted = persist_worker_v2_publication_intent_v2(
            &self.output,
            &self.owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            &bytes,
        )
        .unwrap();
        publish_v2(
            &self.output,
            &self.owner,
            attempt,
            publication_plan,
            evidence,
            closure,
            &bytes,
        );
        let recovered =
            recover_worker_v2_publication_intent_v2(&self.output, &self.owner, attempt, closure)
                .unwrap();
        assert_eq!(recovered.record(), persisted.record());
        assert_eq!(recovered.exact_output(), bytes);
        ExactSuccessor {
            attempt,
            closure,
            intent: recovered.record().identity(),
            recovered,
        }
    }

    fn run_crash_subprocess(
        &self,
        operation: &str,
        point: WorkerV2PublicationIntentCleanupEscrowFaultPointV1,
        capsule: Option<WorkerV2PublicationIntentCleanupEscrowV1>,
    ) {
        let capsule_path = self._temp.0.join("escrow-capsule.bin");
        if let Some(capsule) = capsule {
            fs::write(&capsule_path, capsule.to_bytes()).unwrap();
            fs::set_permissions(&capsule_path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("cleanup_escrow_subprocess_crash_helper")
            .arg("--nocapture")
            .env(ESCROW_CRASH_HELPER_OPERATION, operation)
            .env(ESCROW_CRASH_HELPER_OUTPUT, &self.output)
            .env(ESCROW_CRASH_HELPER_SOURCE, &self.source)
            .env(ESCROW_CRASH_HELPER_ATTEMPT, self.attempt.to_env_value())
            .env(
                ESCROW_CRASH_HELPER_CLOSURE_SEED,
                self.closure_seed.to_string(),
            )
            .env(ESCROW_CRASH_HELPER_CAPSULE, &capsule_path)
            .env(
                ESCROW_CRASH_HELPER_BOUNDARY,
                format!("{:?}", point.boundary),
            )
            .env(ESCROW_CRASH_HELPER_TIMING, format!("{:?}", point.timing))
            .output()
            .unwrap();
        assert_eq!(
            child.status.code(),
            Some(86),
            "crash helper failed for {operation} at {point:?}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
    }
}

struct ExactSuccessor {
    attempt: BuildAttempt,
    closure: CompilerClosureV2,
    intent: WorkerV2PublicationIntentIdentityV2,
    recovered: RecoveredWorkerV2PublicationIntentV2,
}

fn attempt_with_generation(attempt: BuildAttempt, generation: u64) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        attempt.session().to_hex(),
        attempt.invocation().to_hex()
    ))
    .unwrap()
}

fn exact_successor_lease(
    fixture: &CleanupEscrowFixture,
    successor: &ExactSuccessor,
) -> fe2o3_artifact_transaction::WorkerV2PublicationIntentLeaseV2 {
    acquire_worker_v2_publication_intent_lease_v2(
        &fixture.output,
        &fixture.owner,
        successor.attempt,
        successor.closure,
        successor.intent,
    )
    .unwrap()
}

fn assert_predecessor_prepared(
    fixture: &CleanupEscrowFixture,
    capsule: WorkerV2PublicationIntentCleanupEscrowV1,
) {
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &fixture.output,
            &fixture.owner,
            capsule,
        )
        .unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
    );
}

#[test]
fn cleanup_escrow_subprocess_crash_helper() {
    let Ok(operation) = std::env::var(ESCROW_CRASH_HELPER_OPERATION) else {
        return;
    };
    fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
    let output = PathBuf::from(std::env::var_os(ESCROW_CRASH_HELPER_OUTPUT).unwrap());
    let source = std::env::var(ESCROW_CRASH_HELPER_SOURCE).unwrap();
    let owner = producer(&source);
    let attempt =
        BuildAttempt::from_env_value(&std::env::var(ESCROW_CRASH_HELPER_ATTEMPT).unwrap()).unwrap();
    let closure_seed = std::env::var(ESCROW_CRASH_HELPER_CLOSURE_SEED)
        .unwrap()
        .parse::<u8>()
        .unwrap();
    let boundary_name = std::env::var(ESCROW_CRASH_HELPER_BOUNDARY).unwrap();
    let boundary = PREPARE_ESCROW_BOUNDARIES
        .iter()
        .chain(COMMIT_ESCROW_BOUNDARIES)
        .chain(ROLLBACK_ESCROW_BOUNDARIES)
        .copied()
        .find(|candidate| format!("{candidate:?}") == boundary_name)
        .unwrap();
    let timing = match std::env::var(ESCROW_CRASH_HELPER_TIMING).unwrap().as_str() {
        "Before" => WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
        "After" => WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        timing => panic!("unexpected fault timing {timing}"),
    };
    let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 { boundary, timing };
    let options = WorkerV2PublicationIntentCleanupEscrowOptionsV1::inject_crash(point);
    let result = match operation.as_str() {
        "prepare" => {
            let closure = compiler_closure(closure_seed);
            let recovered =
                recover_worker_v2_publication_intent_v2(&output, &owner, attempt, closure).unwrap();
            let receipt = exact_v2_receipt(&output, &owner, attempt);
            prepare_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
                &output,
                &owner,
                attempt,
                closure,
                recovered.record().identity(),
                receipt,
                options,
            )
            .map(|_| ())
        }
        "commit" | "rollback" => {
            let capsule_path =
                PathBuf::from(std::env::var_os(ESCROW_CRASH_HELPER_CAPSULE).unwrap());
            let capsule = WorkerV2PublicationIntentCleanupEscrowV1::from_bytes(
                &fs::read(capsule_path).unwrap(),
            )
            .unwrap();
            if operation == "commit" {
                commit_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
                    &output, &owner, capsule, options,
                )
            } else {
                rollback_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
                    &output, &owner, capsule, options,
                )
            }
        }
        operation => panic!("unexpected crash-helper operation {operation}"),
    };
    match result {
        Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::InjectedCrash { point: actual })
            if actual == point =>
        {
            std::process::exit(86);
        }
        result => panic!("expected injected crash at {point:?}, got {result:?}"),
    }
}

#[test]
fn exact_v2_intent_lease_revalidates_the_pinned_local_snapshot() {
    let fixture = CleanupEscrowFixture::new(0x19, "exact-local-lease");
    let lease = acquire_worker_v2_publication_intent_lease_v2(
        &fixture.output,
        &fixture.owner,
        fixture.attempt,
        fixture.closure,
        fixture.intent,
    )
    .unwrap();
    assert_eq!(lease.recovered().record().identity(), fixture.intent);
    assert!(!lease.grants_publication_authority());
    assert!(!lease.grants_load_authority());
    lease.revalidate().unwrap();
    drop(lease);

    assert!(matches!(
        acquire_worker_v2_publication_intent_lease_v2(
            &fixture.output,
            &fixture.owner,
            fixture.attempt,
            fixture.closure,
            WorkerV2PublicationIntentIdentityV2::from_bytes([0x55; 32]),
        ),
        Err(WorkerV2PublicationIntentErrorV2::IntentIdentityMismatch)
    ));

    assert!(matches!(
        acquire_worker_v2_publication_intent_lease_v2(
            &fixture.output,
            &fixture.owner,
            fixture.attempt,
            compiler_closure(fixture.closure_seed.wrapping_add(1)),
            fixture.intent,
        ),
        Err(WorkerV2PublicationIntentErrorV2::CompilerClosureMismatch)
    ));

    let wrong_owner = producer("/src/v2-cleanup-escrow-wrong-owner.rs");
    assert!(matches!(
        acquire_worker_v2_publication_intent_lease_v2(
            &fixture.output,
            &wrong_owner,
            fixture.attempt,
            fixture.closure,
            fixture.intent,
        ),
        Err(WorkerV2PublicationIntentErrorV2::Attempt { .. })
    ));

    let wrong_attempt = BuildAttempt::from_env_value(&format!(
        "{}:{}:{}",
        fixture.attempt.generation() + 1,
        fixture.attempt.session().to_hex(),
        fixture.attempt.invocation().to_hex()
    ))
    .unwrap();
    assert!(matches!(
        acquire_worker_v2_publication_intent_lease_v2(
            &fixture.output,
            &fixture.owner,
            wrong_attempt,
            fixture.closure,
            fixture.intent,
        ),
        Err(WorkerV2PublicationIntentErrorV2::Attempt { .. })
    ));
}

#[test]
fn exact_v2_intent_lease_holds_off_a_cooperating_mutator_until_drop() {
    let fixture = CleanupEscrowFixture::new(0x1a, "lease-lock-retention");
    let lease = acquire_worker_v2_publication_intent_lease_v2(
        &fixture.output,
        &fixture.owner,
        fixture.attempt,
        fixture.closure,
        fixture.intent,
    )
    .unwrap();
    let output = fixture.output.clone();
    let owner = fixture.owner.clone();
    let attempt = fixture.attempt;
    let closure = fixture.closure;
    let intent = fixture.intent;
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result =
            clear_worker_v2_publication_intent_v2(&output, &owner, attempt, closure, intent);
        done_tx.send(result).unwrap();
    });

    started_rx.recv().unwrap();
    assert!(matches!(
        done_rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    lease.revalidate().unwrap();
    drop(lease);
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
}

#[test]
fn exact_v2_intent_lease_detects_record_output_and_directory_replacement() {
    for (seed, suffix) in [(0x1b, ".record"), (0x1c, ".output")] {
        let fixture = CleanupEscrowFixture::new(seed, "lease-file-replacement");
        let lease = acquire_worker_v2_publication_intent_lease_v2(
            &fixture.output,
            &fixture.owner,
            fixture.attempt,
            fixture.closure,
            fixture.intent,
        )
        .unwrap();
        let entry = schema_entry_with_suffix(&fixture.output, V2_PREFIX, suffix);
        let displaced = fixture.output.join(format!("displaced-lease{suffix}"));
        fs::rename(&entry, &displaced).unwrap();
        fs::copy(&displaced, &entry).unwrap();
        fs::set_permissions(&entry, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            lease.revalidate(),
            Err(WorkerV2PublicationIntentErrorV2::ConflictingIntent)
        ));
    }

    let fixture = CleanupEscrowFixture::new(0x1d, "lease-directory-replacement");
    let lease = acquire_worker_v2_publication_intent_lease_v2(
        &fixture.output,
        &fixture.owner,
        fixture.attempt,
        fixture.closure,
        fixture.intent,
    )
    .unwrap();
    let displaced = fixture._temp.0.join("displaced-output-directory");
    fs::rename(&fixture.output, &displaced).unwrap();
    fs::create_dir(&fixture.output).unwrap();
    assert!(lease.revalidate().is_err());
}

#[test]
fn atomic_successor_commit_returns_bounded_inert_evidence_and_is_idempotent() {
    let fixture = CleanupEscrowFixture::new(0xd1, "atomic-successor-idempotent");
    let capsule = fixture.prepare();
    let successor = fixture.successor(0xd2);

    let evidence = commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
        exact_successor_lease(&fixture, &successor),
        &fixture.owner,
        capsule,
        successor.attempt,
        successor.closure,
        successor.intent,
        &successor.recovered,
        WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
    )
    .unwrap();
    assert_eq!(evidence.predecessor(), capsule.identity());
    assert_eq!(evidence.successor(), successor.recovered.record());
    assert!(!evidence.grants_publication_authority());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
    assert!(std::mem::size_of_val(&evidence) < 2048);
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &fixture.output,
            &fixture.owner,
            capsule,
        )
        .unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
    );
    assert_eq!(
        recover_worker_v2_publication_intent_v2(
            &fixture.output,
            &fixture.owner,
            successor.attempt,
            successor.closure,
        )
        .unwrap()
        .record(),
        successor.recovered.record()
    );

    let replay = commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
        exact_successor_lease(&fixture, &successor),
        &fixture.owner,
        capsule,
        successor.attempt,
        successor.closure,
        successor.intent,
        &successor.recovered,
        WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
    )
    .unwrap();
    assert_eq!(replay, evidence);
}

#[test]
fn atomic_successor_commit_rejects_older_equal_and_mismatched_expectations() {
    let fixture = CleanupEscrowFixture::new(0xd3, "atomic-successor-expectations");
    let capsule = fixture.prepare();
    let successor = fixture.successor(0xd4);
    assert_eq!(fixture.attempt.generation(), 2);
    assert_eq!(successor.attempt.generation(), 3);

    for generation in [1, fixture.attempt.generation()] {
        let candidate = attempt_with_generation(successor.attempt, generation);
        assert!(matches!(
            commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
                exact_successor_lease(&fixture, &successor),
                &fixture.owner,
                capsule,
                candidate,
                successor.closure,
                successor.intent,
                &successor.recovered,
                WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
            ),
            Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::SuccessorGenerationNotNewer {
                predecessor: 2,
                successor
            }) if successor == generation
        ));
        assert_predecessor_prepared(&fixture, capsule);
    }

    let wrong_owner = producer("/src/atomic-successor-wrong-owner.rs");
    let wrong_attempt = attempt_with_generation(successor.attempt, 4);
    let wrong_closure = compiler_closure(0xe1);
    let wrong_intent = WorkerV2PublicationIntentIdentityV2::from_bytes([0xe2; 32]);
    for (owner, attempt, closure, intent) in [
        (
            &wrong_owner,
            successor.attempt,
            successor.closure,
            successor.intent,
        ),
        (
            &fixture.owner,
            wrong_attempt,
            successor.closure,
            successor.intent,
        ),
        (
            &fixture.owner,
            successor.attempt,
            wrong_closure,
            successor.intent,
        ),
        (
            &fixture.owner,
            successor.attempt,
            successor.closure,
            wrong_intent,
        ),
    ] {
        assert!(matches!(
            commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
                exact_successor_lease(&fixture, &successor),
                owner,
                capsule,
                attempt,
                closure,
                intent,
                &successor.recovered,
                WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
            ),
            Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::SuccessorExpectationMismatch)
        ));
        assert_predecessor_prepared(&fixture, capsule);
    }

    commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
        exact_successor_lease(&fixture, &successor),
        &fixture.owner,
        capsule,
        successor.attempt,
        successor.closure,
        successor.intent,
        &successor.recovered,
        WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
    )
    .unwrap();
}

#[test]
fn atomic_successor_commit_rejects_pinned_file_and_directory_replacement() {
    for (seed, suffix) in [(0xd5, ".record"), (0xd6, ".output")] {
        let fixture = CleanupEscrowFixture::new(seed, "atomic-successor-file-replacement");
        let capsule = fixture.prepare();
        let successor = fixture.successor(seed.wrapping_add(1));
        let lease = exact_successor_lease(&fixture, &successor);
        let entry = schema_entry_with_suffix(&fixture.output, V2_PREFIX, suffix);
        let displaced = fixture.output.join(format!("displaced-atomic{suffix}"));
        fs::rename(&entry, &displaced).unwrap();
        fs::copy(&displaced, &entry).unwrap();
        fs::set_permissions(&entry, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(matches!(
            commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
                lease,
                &fixture.owner,
                capsule,
                successor.attempt,
                successor.closure,
                successor.intent,
                &successor.recovered,
                WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
            ),
            Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::Intent(
                WorkerV2PublicationIntentErrorV2::ConflictingIntent
            ))
        ));
        assert!(entry.exists(), "replacement was destructively removed");
        assert_predecessor_prepared(&fixture, capsule);
    }

    let fixture = CleanupEscrowFixture::new(0xd8, "atomic-successor-directory-replacement");
    let capsule = fixture.prepare();
    let successor = fixture.successor(0xd9);
    let lease = exact_successor_lease(&fixture, &successor);
    let displaced = fixture._temp.0.join("displaced-atomic-output");
    fs::rename(&fixture.output, &displaced).unwrap();
    fs::create_dir(&fixture.output).unwrap();
    assert!(
        commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
            lease,
            &fixture.owner,
            capsule,
            successor.attempt,
            successor.closure,
            successor.intent,
            &successor.recovered,
            WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
        )
        .is_err()
    );
    fs::remove_dir(&fixture.output).unwrap();
    fs::rename(&displaced, &fixture.output).unwrap();
    assert_predecessor_prepared(&fixture, capsule);
}

#[test]
fn atomic_successor_commit_retains_lock_through_predecessor_commit() {
    let fixture = CleanupEscrowFixture::new(0xda, "atomic-successor-lock-retention");
    let capsule = fixture.prepare();
    let successor = fixture.successor(0xdb);
    let lease = exact_successor_lease(&fixture, &successor);
    let output = fixture.output.clone();
    let owner = fixture.owner.clone();
    let attempt = successor.attempt;
    let closure = successor.closure;
    let intent = successor.intent;
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        started_tx.send(()).unwrap();
        done_tx
            .send(clear_worker_v2_publication_intent_v2(
                &output, &owner, attempt, closure, intent,
            ))
            .unwrap();
    });
    started_rx.recv().unwrap();
    assert!(matches!(
        done_rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));

    commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
        lease,
        &fixture.owner,
        capsule,
        successor.attempt,
        successor.closure,
        successor.intent,
        &successor.recovered,
        WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
    )
    .unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &fixture.output,
            &fixture.owner,
            capsule,
        )
        .unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
    );
}

#[test]
fn atomic_successor_commit_recovers_every_predecessor_commit_boundary() {
    for (boundary_index, boundary) in COMMIT_ESCROW_BOUNDARIES.iter().copied().enumerate() {
        for timing in [
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        ] {
            let seed = 0x20_u8.wrapping_add(boundary_index as u8);
            let fixture = CleanupEscrowFixture::new(seed, "atomic-successor-crash-boundary");
            let capsule = fixture.prepare();
            let successor = fixture.successor(seed.wrapping_add(0x40));
            let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 { boundary, timing };
            assert!(matches!(
                commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
                    exact_successor_lease(&fixture, &successor),
                    &fixture.owner,
                    capsule,
                    successor.attempt,
                    successor.closure,
                    successor.intent,
                    &successor.recovered,
                    WorkerV2PublicationIntentCleanupEscrowOptionsV1::inject_crash(point),
                ),
                Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::InjectedCrash {
                    point: actual
                }) if actual == point
            ));
            if point
                == (WorkerV2PublicationIntentCleanupEscrowFaultPointV1 {
                    boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1::SyncManifestName,
                    timing: WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
                })
            {
                assert_eq!(
                    recover_worker_v2_publication_intent_cleanup_escrow_v1(
                        &fixture.output,
                        &fixture.owner,
                        capsule,
                    )
                    .unwrap(),
                    WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
                );
            }

            let evidence =
                commit_worker_v2_publication_intent_cleanup_escrow_after_exact_successor_v2(
                    exact_successor_lease(&fixture, &successor),
                    &fixture.owner,
                    capsule,
                    successor.attempt,
                    successor.closure,
                    successor.intent,
                    &successor.recovered,
                    WorkerV2PublicationIntentCleanupEscrowOptionsV1::default(),
                )
                .unwrap();
            assert_eq!(evidence.successor(), successor.recovered.record());
        }
    }
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

#[test]
fn artifact_owned_cleanup_escrow_roundtrips_rolls_back_and_commits_idempotently() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("/src/v2-cleanup-escrow-lifecycle.rs");
    let attempt = begin(&output, &owner, 0x21);
    let bytes = b"artifact-owned escrow output";
    let publication_plan = escrow_plan(&owner, attempt, 0x31, bytes);
    let evidence = upstream(0x41);
    let closure = compiler_closure(0x51);
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
    let receipt = exact_v2_receipt(&output, &owner, attempt);

    let prepared = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
        &output,
        &owner,
        attempt,
        closure,
        persisted.record().identity(),
        receipt,
    )
    .unwrap();
    let capsule_bytes = prepared.to_bytes();
    assert_eq!(
        capsule_bytes.len(),
        MAX_WORKER_V2_PUBLICATION_INTENT_CLEANUP_ESCROW_CAPSULE_BYTES_V1
    );
    assert_eq!(
        WorkerV2PublicationIntentCleanupEscrowV1::from_bytes(&capsule_bytes).unwrap(),
        prepared
    );
    for index in 0..capsule_bytes.len() {
        let mut mutated = capsule_bytes.clone();
        mutated[index] ^= 0x01;
        assert!(WorkerV2PublicationIntentCleanupEscrowV1::from_bytes(&mutated).is_err());
    }
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, prepared).unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
    );
    assert!(matches!(
        recover_worker_v2_publication_intent_v2(&output, &owner, attempt, closure),
        Err(WorkerV2PublicationIntentErrorV2::NotFound)
    ));

    rollback_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, prepared).unwrap();
    assert_eq!(
        recover_worker_v2_publication_intent_v2(&output, &owner, attempt, closure)
            .unwrap()
            .exact_output(),
        bytes
    );
    rollback_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, prepared).unwrap();

    let prepared = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
        &output,
        &owner,
        attempt,
        closure,
        persisted.record().identity(),
        receipt,
    )
    .unwrap();
    commit_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, prepared).unwrap();
    assert!(escrow_entry_names(&output).is_empty());
    commit_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, prepared).unwrap();
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, prepared).unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
    );
    assert!(matches!(
        recover_worker_v2_publication_intent_v2(&output, &owner, attempt, closure),
        Err(WorkerV2PublicationIntentErrorV2::NotFound)
    ));
    assert!(escrow_entry_names(&output).is_empty());
}

#[test]
fn fresh_cleanup_escrow_prepare_at_or_one_below_the_entry_cap_is_non_mutating() {
    let fixture = CleanupEscrowFixture::new(0x23, "fresh-capacity-rejection");
    fill_output_with_unrelated_entries(
        &fixture.output,
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 1,
    );
    for target_entries in [
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 1,
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES,
    ] {
        if output_entry_count(&fixture.output) < target_entries {
            fs::File::create(fixture.output.join("cleanup-capacity-exact-cap-extra")).unwrap();
        }
        let before_intent = intent_snapshot(&fixture.output);
        let before_escrow = escrow_entry_names(&fixture.output);

        let error = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
            &fixture.output,
            &fixture.owner,
            fixture.attempt,
            fixture.closure,
            fixture.intent,
            fixture.receipt,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WorkerV2PublicationIntentCleanupEscrowErrorV1::InvalidEscrow { ref reason, .. }
                if reason
                    == "artifact directory lacks capacity for a crash-recoverable cleanup escrow manifest write"
        ));
        assert_eq!(intent_snapshot(&fixture.output), before_intent);
        assert_eq!(escrow_entry_names(&fixture.output), before_escrow);
        assert_eq!(output_entry_count(&fixture.output), target_entries);
    }
}

#[test]
fn highest_admissible_cleanup_escrow_prepare_and_commit_stay_within_the_entry_cap() {
    let fixture = CleanupEscrowFixture::new(0x24, "highest-admissible-capacity");
    fill_output_with_unrelated_entries(
        &fixture.output,
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 2,
    );

    let capsule = fixture.prepare();

    assert_eq!(
        output_entry_count(&fixture.output),
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 1
    );
    assert_eq!(
        escrow_entry_names(&fixture.output)
            .iter()
            .filter(|name| name.contains(".manifest.tmp-"))
            .count(),
        0
    );
    let replacement_capacity_entry = fixture.output.join("cleanup-capacity-replacement-extra");
    fs::File::create(&replacement_capacity_entry).unwrap();
    assert_eq!(
        output_entry_count(&fixture.output),
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES
    );
    let error = commit_worker_v2_publication_intent_cleanup_escrow_v1(
        &fixture.output,
        &fixture.owner,
        capsule,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkerV2PublicationIntentCleanupEscrowErrorV1::InvalidEscrow { ref reason, .. }
            if reason
                == "artifact directory lacks capacity for a crash-recoverable cleanup escrow manifest write"
    ));
    assert_eq!(
        output_entry_count(&fixture.output),
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES
    );
    assert_eq!(
        escrow_entry_names(&fixture.output)
            .iter()
            .filter(|name| name.contains(".manifest.tmp-"))
            .count(),
        0
    );
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &fixture.output,
            &fixture.owner,
            capsule,
        )
        .unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
    );
    fs::remove_file(replacement_capacity_entry).unwrap();
    commit_worker_v2_publication_intent_cleanup_escrow_v1(&fixture.output, &fixture.owner, capsule)
        .unwrap();
    assert!(escrow_entry_names(&fixture.output).is_empty());
    assert_eq!(
        output_entry_count(&fixture.output),
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 4
    );
}

#[test]
fn highest_admissible_create_temp_crash_is_bounded_recovered_and_retryable() {
    let fixture = CleanupEscrowFixture::new(0x25, "highest-admissible-create-crash");
    fill_output_with_unrelated_entries(
        &fixture.output,
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 2,
    );
    let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 {
        boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1::CreateManifestTemp,
        timing: WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
    };

    fixture.run_crash_subprocess("prepare", point, None);

    assert_eq!(
        output_entry_count(&fixture.output),
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 1
    );
    let crash_entries = escrow_entry_names(&fixture.output);
    assert_eq!(
        crash_entries
            .iter()
            .filter(|name| name.contains(".manifest.tmp-"))
            .count(),
        1
    );
    assert!(!crash_entries.iter().any(|name| name.ends_with(".manifest")));

    let capsule = fixture.prepare();

    assert_eq!(
        output_entry_count(&fixture.output),
        CLEANUP_CAPACITY_TEST_MAX_OUTPUT_ENTRIES - 1
    );
    assert_eq!(
        escrow_entry_names(&fixture.output)
            .iter()
            .filter(|name| name.contains(".manifest.tmp-"))
            .count(),
        0
    );
    commit_worker_v2_publication_intent_cleanup_escrow_v1(&fixture.output, &fixture.owner, capsule)
        .unwrap();
    assert!(escrow_entry_names(&fixture.output).is_empty());
}

#[test]
fn cleanup_escrow_prepare_rejects_raw_non_utf8_temp_without_touching_its_alias() {
    let fixture = CleanupEscrowFixture::new(0x26, "raw-intent-temp-alias");
    let record_path = schema_entry_with_suffix(&fixture.output, V2_PREFIX, ".record");
    let output_path = schema_entry_with_suffix(&fixture.output, V2_PREFIX, ".output");
    let record_before = fs::read(&record_path).unwrap();
    let output_before = fs::read(&output_path).unwrap();
    let record_name = record_path.file_name().unwrap().to_str().unwrap();
    let temp_prefix = format!("{}.tmp-", record_name.strip_suffix(".record").unwrap());
    let raw_name = OsString::from_vec([temp_prefix.as_bytes(), &[0xff]].concat());
    let alias_name = format!("{temp_prefix}\u{fffd}");
    assert_eq!(raw_name.to_string_lossy(), alias_name);
    let raw_path = fixture.output.join(&raw_name);
    let alias_path = fixture.output.join(&alias_name);
    std::os::unix::fs::symlink("missing-target", &raw_path).unwrap();
    fs::write(&alias_path, b"valid UTF-8 alias").unwrap();
    fs::set_permissions(&alias_path, fs::Permissions::from_mode(0o600)).unwrap();

    for _ in 0..2 {
        let error = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
            &fixture.output,
            &fixture.owner,
            fixture.attempt,
            fixture.closure,
            fixture.intent,
            fixture.receipt,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WorkerV2PublicationIntentCleanupEscrowErrorV1::Intent(
                WorkerV2PublicationIntentErrorV2::InvalidIntent { ref path, ref reason }
            ) if path == &raw_path && reason == "temporary entry is not private"
        ));
        assert!(
            fs::symlink_metadata(&raw_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&alias_path).unwrap(), b"valid UTF-8 alias");
        assert_eq!(fs::read(&record_path).unwrap(), record_before);
        assert_eq!(fs::read(&output_path).unwrap(), output_before);
        assert!(escrow_entry_names(&fixture.output).is_empty());
    }
}

#[test]
fn cleanup_escrow_capsule_rejects_resealed_noncanonical_relationships() {
    let fixture = CleanupEscrowFixture::new(0x2f, "resealed-capsule-relations");
    let canonical = fixture.prepare().to_bytes();
    let slot_offset = ESCROW_CAPSULE_MAGIC.len() + 2 + 32 + 32 + 8 + 16 + 32;
    let intent_offset = slot_offset + 32;
    let record_identity_offset = intent_offset + 32;
    let output_identity_offset = record_identity_offset + 32;
    let receipt_identity_offset = output_identity_offset + 32;
    let receipt_offset = receipt_identity_offset + 32;
    let receipt_end = canonical.len() - 32 - (2 * 5 * 8);
    let record_snapshot_offset = receipt_end;
    let output_snapshot_offset = record_snapshot_offset + (5 * 8);

    let reject = |mut bytes: Vec<u8>, label: &str| {
        reseal_cleanup_capsule(&mut bytes);
        assert!(
            WorkerV2PublicationIntentCleanupEscrowV1::from_bytes(&bytes).is_err(),
            "accepted resealed noncanonical {label}"
        );
    };

    let mut wrong_slot = canonical.clone();
    wrong_slot[slot_offset] ^= 1;
    reject(wrong_slot, "producer/attempt slot");

    let mut wrong_record_identity = canonical.clone();
    wrong_record_identity[record_identity_offset] ^= 1;
    reject(wrong_record_identity, "record/intent identity");

    let mut wrong_output_identity = canonical.clone();
    wrong_output_identity[output_identity_offset] ^= 1;
    reject(wrong_output_identity, "output/receipt identity");

    let mut wrong_receipt_attempt = canonical.clone();
    wrong_receipt_attempt[receipt_offset] ^= 1;
    let receipt_identity = sha256_domain_parts(
        ESCROW_RECEIPT_IDENTITY_DOMAIN,
        &[&wrong_receipt_attempt[receipt_offset..receipt_end]],
    );
    wrong_receipt_attempt[receipt_identity_offset..receipt_offset]
        .copy_from_slice(&receipt_identity);
    reject(wrong_receipt_attempt, "attempt/receipt identity");

    let mut nonprivate_record = canonical.clone();
    let record_mode_offset = record_snapshot_offset + (3 * 8);
    let mut mode = u64::from_le_bytes(
        nonprivate_record[record_mode_offset..record_mode_offset + 8]
            .try_into()
            .unwrap(),
    );
    mode = (mode & !0o777) | 0o644;
    nonprivate_record[record_mode_offset..record_mode_offset + 8]
        .copy_from_slice(&mode.to_le_bytes());
    reject(nonprivate_record, "record mode");

    let mut aliased_files = canonical;
    let output_device_and_inode: [u8; 16] = aliased_files
        [output_snapshot_offset..output_snapshot_offset + 16]
        .try_into()
        .unwrap();
    aliased_files[record_snapshot_offset..record_snapshot_offset + 16]
        .copy_from_slice(&output_device_and_inode);
    reject(aliased_files, "record/output inode alias");
}

#[test]
fn cleanup_escrow_prepare_subprocess_crashes_are_durable_on_retry() {
    for (boundary_index, boundary) in PREPARE_ESCROW_BOUNDARIES.iter().copied().enumerate() {
        for timing in [
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        ] {
            let fixture = CleanupEscrowFixture::new(
                0x30_u8.wrapping_add(boundary_index as u8),
                "prepare-crash",
            );
            let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 { boundary, timing };
            fixture.run_crash_subprocess("prepare", point, None);
            let capsule = fixture.prepare();
            assert_eq!(
                recover_worker_v2_publication_intent_cleanup_escrow_v1(
                    &fixture.output,
                    &fixture.owner,
                    capsule,
                )
                .unwrap(),
                WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared,
                "prepare retry did not recover {point:?}"
            );
            rollback_worker_v2_publication_intent_cleanup_escrow_v1(
                &fixture.output,
                &fixture.owner,
                capsule,
            )
            .unwrap();
            assert!(escrow_entry_names(&fixture.output).is_empty());
        }
    }
}

#[test]
fn cleanup_escrow_commit_subprocess_crashes_retire_all_state_on_retry() {
    for (boundary_index, boundary) in COMMIT_ESCROW_BOUNDARIES.iter().copied().enumerate() {
        for timing in [
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        ] {
            let fixture = CleanupEscrowFixture::new(
                0x50_u8.wrapping_add(boundary_index as u8),
                "commit-crash",
            );
            let capsule = fixture.prepare();
            let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 { boundary, timing };
            fixture.run_crash_subprocess("commit", point, Some(capsule));
            commit_worker_v2_publication_intent_cleanup_escrow_v1(
                &fixture.output,
                &fixture.owner,
                capsule,
            )
            .unwrap();
            assert!(
                escrow_entry_names(&fixture.output).is_empty(),
                "commit retry retained terminal state after {point:?}"
            );
            assert_eq!(
                recover_worker_v2_publication_intent_cleanup_escrow_v1(
                    &fixture.output,
                    &fixture.owner,
                    capsule,
                )
                .unwrap(),
                WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
            );
        }
    }
}

#[test]
fn cleanup_escrow_recover_closes_manifest_rename_and_terminal_unlink_durability() {
    let renamed_fixture = CleanupEscrowFixture::new(0x61, "recover-renamed-manifest");
    let renamed_capsule = renamed_fixture.prepare();
    let renamed_point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 {
        boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1::RenameManifest,
        timing: WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
    };
    renamed_fixture.run_crash_subprocess("commit", renamed_point, Some(renamed_capsule));
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &renamed_fixture.output,
            &renamed_fixture.owner,
            renamed_capsule,
        )
        .unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
    );
    commit_worker_v2_publication_intent_cleanup_escrow_v1(
        &renamed_fixture.output,
        &renamed_fixture.owner,
        renamed_capsule,
    )
    .unwrap();

    let unlinked_fixture = CleanupEscrowFixture::new(0x62, "recover-unlinked-manifest");
    let unlinked_capsule = unlinked_fixture.prepare();
    let unlinked_point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 {
        boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkCommittedManifest,
        timing: WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
    };
    unlinked_fixture.run_crash_subprocess("commit", unlinked_point, Some(unlinked_capsule));
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(
            &unlinked_fixture.output,
            &unlinked_fixture.owner,
            unlinked_capsule,
        )
        .unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Committed
    );
    assert!(escrow_entry_names(&unlinked_fixture.output).is_empty());
}

#[test]
fn cleanup_escrow_rollback_subprocess_crashes_restore_exact_intent_on_retry() {
    for (boundary_index, boundary) in ROLLBACK_ESCROW_BOUNDARIES.iter().copied().enumerate() {
        for timing in [
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
            WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::After,
        ] {
            let fixture = CleanupEscrowFixture::new(
                0x70_u8.wrapping_add(boundary_index as u8),
                "rollback-crash",
            );
            let capsule = fixture.prepare();
            let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 { boundary, timing };
            fixture.run_crash_subprocess("rollback", point, Some(capsule));
            rollback_worker_v2_publication_intent_cleanup_escrow_v1(
                &fixture.output,
                &fixture.owner,
                capsule,
            )
            .unwrap();
            assert!(
                escrow_entry_names(&fixture.output).is_empty(),
                "rollback retry retained terminal state after {point:?}"
            );
            let recovered = recover_worker_v2_publication_intent_v2(
                &fixture.output,
                &fixture.owner,
                fixture.attempt,
                fixture.closure,
            )
            .unwrap();
            assert_eq!(recovered.record().identity(), fixture.intent);
        }
    }
}

#[test]
fn cleanup_escrow_rejects_hardlinks_symlinks_and_replacements() {
    let hardlink_fixture = CleanupEscrowFixture::new(0x91, "hostile-hardlink");
    let hardlink_capsule = hardlink_fixture.prepare();
    let quarantined_output =
        escrow_entry_with_suffix(&hardlink_fixture.output, ".output.quarantine");
    let hostile_link = hardlink_fixture.output.join("hostile-output-hardlink");
    fs::hard_link(&quarantined_output, &hostile_link).unwrap();
    assert!(
        commit_worker_v2_publication_intent_cleanup_escrow_v1(
            &hardlink_fixture.output,
            &hardlink_fixture.owner,
            hardlink_capsule,
        )
        .is_err()
    );
    assert!(quarantined_output.exists());
    assert!(hostile_link.exists());
    fs::remove_file(hostile_link).unwrap();
    commit_worker_v2_publication_intent_cleanup_escrow_v1(
        &hardlink_fixture.output,
        &hardlink_fixture.owner,
        hardlink_capsule,
    )
    .unwrap();

    let symlink_fixture = CleanupEscrowFixture::new(0xa1, "hostile-symlink");
    let symlink_capsule = symlink_fixture.prepare();
    let quarantined_output =
        escrow_entry_with_suffix(&symlink_fixture.output, ".output.quarantine");
    let displaced_output = symlink_fixture.output.join("displaced-output-symlink");
    fs::rename(&quarantined_output, &displaced_output).unwrap();
    std::os::unix::fs::symlink(&displaced_output, &quarantined_output).unwrap();
    assert!(
        commit_worker_v2_publication_intent_cleanup_escrow_v1(
            &symlink_fixture.output,
            &symlink_fixture.owner,
            symlink_capsule,
        )
        .is_err()
    );
    assert!(
        fs::symlink_metadata(&quarantined_output)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(displaced_output.exists());
    fs::remove_file(&quarantined_output).unwrap();
    fs::rename(&displaced_output, &quarantined_output).unwrap();
    commit_worker_v2_publication_intent_cleanup_escrow_v1(
        &symlink_fixture.output,
        &symlink_fixture.owner,
        symlink_capsule,
    )
    .unwrap();

    let replacement_fixture = CleanupEscrowFixture::new(0xb1, "hostile-replacement");
    let replacement_capsule = replacement_fixture.prepare();
    let quarantined_output =
        escrow_entry_with_suffix(&replacement_fixture.output, ".output.quarantine");
    let displaced_output = replacement_fixture
        .output
        .join("displaced-output-replacement");
    fs::rename(&quarantined_output, &displaced_output).unwrap();
    fs::copy(&displaced_output, &quarantined_output).unwrap();
    fs::set_permissions(&quarantined_output, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        commit_worker_v2_publication_intent_cleanup_escrow_v1(
            &replacement_fixture.output,
            &replacement_fixture.owner,
            replacement_capsule,
        )
        .is_err()
    );
    assert!(quarantined_output.exists());
    assert!(displaced_output.exists());
    fs::remove_file(&quarantined_output).unwrap();
    fs::rename(&displaced_output, &quarantined_output).unwrap();
    commit_worker_v2_publication_intent_cleanup_escrow_v1(
        &replacement_fixture.output,
        &replacement_fixture.owner,
        replacement_capsule,
    )
    .unwrap();
}

#[test]
fn cleanup_escrow_terminal_manifest_replacement_is_not_unlinked() {
    let fixture = CleanupEscrowFixture::new(0xc1, "terminal-manifest-replacement");
    let capsule = fixture.prepare();
    let point = WorkerV2PublicationIntentCleanupEscrowFaultPointV1 {
        boundary: WorkerV2PublicationIntentCleanupEscrowBoundaryV1::UnlinkQuarantinedRecord,
        timing: WorkerV2PublicationIntentCleanupEscrowFaultTimingV1::Before,
    };
    assert!(matches!(
        commit_worker_v2_publication_intent_cleanup_escrow_v1_with_options(
            &fixture.output,
            &fixture.owner,
            capsule,
            WorkerV2PublicationIntentCleanupEscrowOptionsV1::inject_crash(point),
        ),
        Err(WorkerV2PublicationIntentCleanupEscrowErrorV1::InjectedCrash { point: actual })
            if actual == point
    ));

    let manifest = escrow_entry_with_suffix(&fixture.output, ".manifest");
    let displaced = fixture.output.join("displaced-terminal-manifest");
    let mut replacement = fs::read(&manifest).unwrap();
    fs::rename(&manifest, &displaced).unwrap();
    replacement[0] ^= 0xff;
    fs::write(&manifest, &replacement).unwrap();
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();

    assert!(
        commit_worker_v2_publication_intent_cleanup_escrow_v1(
            &fixture.output,
            &fixture.owner,
            capsule,
        )
        .is_err()
    );
    assert_eq!(fs::read(&manifest).unwrap(), replacement);
    assert!(displaced.exists());

    fs::remove_file(&manifest).unwrap();
    fs::rename(displaced, manifest).unwrap();
    commit_worker_v2_publication_intent_cleanup_escrow_v1(&fixture.output, &fixture.owner, capsule)
        .unwrap();
    assert!(escrow_entry_names(&fixture.output).is_empty());
}

#[test]
fn cleanup_escrow_names_isolate_attempts_and_newer_commit_cannot_retire_predecessor() {
    let temp = TestDirectory::new();
    let output = temp.output();
    let owner = producer("/src/v2-cleanup-escrow-attempt-isolation.rs");

    let first_attempt = begin(&output, &owner, 0xd1);
    let first_bytes = b"first attempt exact output";
    let first_plan = escrow_plan(&owner, first_attempt, 0xd2, first_bytes);
    let first_evidence = upstream(0xd3);
    let first_closure = compiler_closure(0xd4);
    let first_intent = persist_worker_v2_publication_intent_v2(
        &output,
        &owner,
        first_attempt,
        first_plan,
        first_evidence,
        first_closure,
        first_bytes,
    )
    .unwrap();
    publish_v2(
        &output,
        &owner,
        first_attempt,
        first_plan,
        first_evidence,
        first_closure,
        first_bytes,
    );
    finish_build_attempt(&output, &owner, first_attempt).unwrap();
    let first_capsule = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
        &output,
        &owner,
        first_attempt,
        first_closure,
        first_intent.record().identity(),
        exact_v2_receipt(&output, &owner, first_attempt),
    )
    .unwrap();
    let first_names = escrow_entry_names(&output)
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(first_names.len(), 3);

    let second_attempt = begin(&output, &owner, 0xe1);
    assert!(second_attempt.generation() > first_attempt.generation());
    let second_bytes = b"second attempt exact output";
    let second_plan = escrow_plan(&owner, second_attempt, 0xe2, second_bytes);
    let second_evidence = upstream(0xe3);
    let second_closure = compiler_closure(0xe4);
    let second_intent = persist_worker_v2_publication_intent_v2(
        &output,
        &owner,
        second_attempt,
        second_plan,
        second_evidence,
        second_closure,
        second_bytes,
    )
    .unwrap();
    publish_v2(
        &output,
        &owner,
        second_attempt,
        second_plan,
        second_evidence,
        second_closure,
        second_bytes,
    );
    let second_capsule = prepare_worker_v2_publication_intent_cleanup_escrow_v1(
        &output,
        &owner,
        second_attempt,
        second_closure,
        second_intent.record().identity(),
        exact_v2_receipt(&output, &owner, second_attempt),
    )
    .unwrap();
    let both_names = escrow_entry_names(&output)
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(both_names.len(), 6);
    assert!(first_names.is_subset(&both_names));

    commit_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, second_capsule).unwrap();
    assert_eq!(
        escrow_entry_names(&output)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        first_names
    );
    assert_eq!(
        recover_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, first_capsule,)
            .unwrap(),
        WorkerV2PublicationIntentCleanupEscrowStateV1::Prepared
    );
    rollback_worker_v2_publication_intent_cleanup_escrow_v1(&output, &owner, first_capsule)
        .unwrap();
    assert!(escrow_entry_names(&output).is_empty());
}
