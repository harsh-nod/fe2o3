use super::*;

use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::{
    BuildInvocation, begin_build_attempt, consume_compiler_module_handoff_v1, fail_build_attempt,
    finish_build_attempt, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_api::{
    CompileLimitsV1, CompileRequestV1, CompilerProfileIdentityV1, CompilerStageV1,
    KernelInstanceIdentityV1, PipelineSelectorV1, RequestIdentityV1, SnapshotFormatIdentityV1,
    SnapshotIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
};
use fe2o3_general_gemm_compiler::{
    GeneralGemmFrontendSemanticBindingV1, GeneralGemmLoweringLimitsV1, GeneralGemmSymbolicKirV1,
    GeneralGemmSymbolicPlanV1, general_gemm_symbolic_obligation_set_identity_v1,
    general_gemm_symbolic_pipeline_configuration_identity_v1,
};
use serde_json::json;

const MEASURED_WORKER: &str = "/home/harsh/fe2o3-general-gemm-worker-llvm22/fe2o3-llvm-link-worker";
const MEASURED_VERUS: &str = "/home/harsh/.cache/fe2o3-verus-0.2026.08.02/verus-x86-linux/verus";
const MEASURED_WORKER_SHA256: &str =
    "0b4936777b08d7d9d864bf357ab4f14cac33a0bb0a13c479209a26c1da808d35";
const MEASURED_WORKER_BUILD_ID: &str =
    "fe2o3-worker-v1-sha256-1769826adfd0cc9832015371d9df79bb9128093a28be89144aa797d8155151a0";
const MEASURED_LLVM_BUILD_ID: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "rustc-fe2o3-general-gemm-pipeline-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct OpaqueFrontendToken(u8);

fn identity(value: u8) -> [u8; 32] {
    [value; 32]
}

fn frontend() -> GeneralGemmFrontendSemanticBindingV1 {
    frontend_with_provider(0x43)
}

fn frontend_with_provider(provider: u8) -> GeneralGemmFrontendSemanticBindingV1 {
    GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
        identity(0x12),
        identity(0x41),
        identity(0x42),
        identity(provider),
        GeneralGemmSymbolicPlanV1::canonical(),
        GeneralGemmSymbolicKirV1::canonical(),
    )
    .unwrap()
}

fn request(
    frontend: &GeneralGemmFrontendSemanticBindingV1,
    schedule: GeneralGemmScheduleV1,
) -> CompileRequestV1 {
    let input = StageSnapshotV1::new(
        CompilerStageV1::FrontendInput,
        SnapshotIdentityV1::from_untrusted_bytes(identity(0x17)),
        SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x18)),
        b"inert-general-gemm-rustc-pipeline-test".to_vec(),
    )
    .unwrap();
    let obligations = general_gemm_symbolic_obligation_set_identity_v1(&input, frontend);
    CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes(identity(0x11)),
        KernelInstanceIdentityV1::from_untrusted_bytes(identity(0x12)),
        CompilerProfileIdentityV1::from_untrusted_bytes(identity(0x13)),
        TargetProfileIdentityV1::from_untrusted_bytes(identity(0x14)),
        general_gemm_symbolic_pipeline_configuration_identity_v1(schedule),
        obligations,
        PipelineSelectorV1::PlironV1,
        input,
        CompileLimitsV1::new(16, 16, 16, 4096, 16_384, 4096).unwrap(),
    )
    .unwrap()
}

fn unit(schedule: GeneralGemmScheduleV1) -> GeneralGemmSymbolicCompilationUnitV1 {
    let frontend = frontend();
    unit_with_frontend(schedule, frontend)
}

fn unit_with_frontend(
    schedule: GeneralGemmScheduleV1,
    frontend: GeneralGemmFrontendSemanticBindingV1,
) -> GeneralGemmSymbolicCompilationUnitV1 {
    let request = request(&frontend, schedule);
    GeneralGemmSymbolicCompilationUnitV1::checked(
        &request,
        frontend,
        schedule,
        GeneralGemmLoweringLimitsV1::default(),
    )
    .unwrap()
}

fn manifest(
    directory: &TestDirectory,
    worker_path: &Path,
    verus_path: &Path,
    worker_sha256: [u8; 32],
    worker_length: u64,
    worker_build: &str,
    llvm_build: &str,
) -> (PathBuf, String) {
    let value = json!({
        "candidate_output_max_bytes": fe2o3_hsaco::MAX_HSACO_BYTES,
        "format": CONFIG_FORMAT,
        "general_gemm_v1": {
            "profile": GENERAL_GEMM_QUALIFICATION_PAIR_PROFILE_V1,
            "proof_timeout_seconds": 120,
            "verus_path": verus_path
        },
        "limits": {
            "stderr_bytes": 64 * 1024,
            "stdout_bytes": 2 * 1024 * 1024,
            "timeout_ms": 120_000
        },
        "link_options": [
            {"name": "code-object-version", "value": "6"},
            {"name": "opt-level", "value": "2"},
            {"name": "strip-debug", "value": "true"},
            {"name": "verify-each", "value": "true"}
        ],
        "providers": [],
        "units": [{
            "crate_name": "general_gemm_test",
            "source": "src/lib.rs",
            "working_directory": directory.0
        }],
        "worker": {
            "byte_len": worker_length,
            "llvm_build_identity": llvm_build,
            "path": worker_path,
            "sha256": hex(&worker_sha256),
            "worker_build_identity": worker_build
        }
    });
    let bytes = serde_json::to_vec(&value).unwrap();
    let path = directory.0.join("qualification-pair-v1.json");
    fs::write(&path, &bytes).unwrap();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::from_parts(worker_sha256, worker_length),
        worker_build,
        llvm_build,
    )
    .unwrap();
    let worker = PinnedWorkerV1::open(worker_path, measurement).unwrap();
    (path, hex(&calculate_config_identity(&bytes, &worker).0))
}

fn local_manifest(directory: &TestDirectory) -> (PathBuf, String) {
    let worker = PathBuf::from("/usr/bin/true");
    let bytes = fs::read(&worker).unwrap();
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    manifest(
        directory,
        &worker,
        &worker,
        sha256,
        bytes.len() as u64,
        "test-worker-v1",
        "test-llvm-v1",
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|value| format!("{value:02x}")).collect()
}

fn producer() -> ProducerIdentity {
    ProducerIdentity::from_codegen("general_gemm_test", Some(Path::new("src/lib.rs"))).unwrap()
}

#[test]
fn parser_accepts_only_the_closed_qualification_pair_and_exact_fields() {
    let directory = TestDirectory::new("hostile");
    let (path, expected) = local_manifest(&directory);
    let config = PreparedGeneralGemmPipelineV1::from_manifest(
        &path,
        &expected,
        "general_gemm_test",
        Path::new("src/lib.rs"),
        &directory.0,
    )
    .unwrap();
    assert_eq!(config.verus_path(), Path::new("/usr/bin/true"));
    assert_eq!(config.proof_timeout_seconds(), 120);
    assert_eq!(hex(&config.identity().as_bytes()), expected);

    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["general_gemm_v1"]["profile"] = json!("single-schedule-v1");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        PreparedGeneralGemmPipelineV1::from_manifest(
            &path,
            &expected,
            "general_gemm_test",
            Path::new("src/lib.rs"),
            &directory.0,
        ),
        Err(GeneralGemmPipelineErrorV1::Configuration(reason))
            if reason.contains("unsupported")
    ));
    value["general_gemm_v1"]["extra"] = json!(true);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        PreparedGeneralGemmPipelineV1::from_manifest(
            &path,
            &expected,
            "general_gemm_test",
            Path::new("src/lib.rs"),
            &directory.0,
        ),
        Err(GeneralGemmPipelineErrorV1::Configuration(reason))
            if reason.contains("unknown, missing, or reordered")
    ));
}

#[test]
fn reordered_schedule_pair_is_rejected_before_any_handoff() {
    let directory = TestDirectory::new("schedule-reorder");
    let (path, expected) = local_manifest(&directory);
    let config = PreparedGeneralGemmPipelineV1::from_manifest(
        &path,
        &expected,
        "general_gemm_test",
        Path::new("src/lib.rs"),
        &directory.0,
    )
    .unwrap();
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x72; 32]),
        BuildSession::from_bytes([0x71; 16]),
    )
    .unwrap();
    assert!(matches!(
        execute_general_gemm_pipeline_v1(
            OpaqueFrontendToken(1),
            [
                unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1),
                unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
            ],
            config,
            &directory.0,
            &producer,
            attempt,
        ),
        Err(GeneralGemmPipelineErrorV1::ScheduleSubstitution)
    ));
    for slot in [
        CompilerModuleHandoffSlotV1::GeneralGemmReference,
        CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly,
    ] {
        assert!(
            consume_compiler_module_handoff_in_slot_v1(&directory.0, &producer, attempt, slot)
                .is_err()
        );
    }
    fail_build_attempt(&directory.0, &producer, attempt).unwrap();
}

#[test]
fn frontend_binding_substitution_is_rejected_before_proof_or_handoff() {
    let directory = TestDirectory::new("frontend-substitution");
    let (path, expected) = local_manifest(&directory);
    let config = PreparedGeneralGemmPipelineV1::from_manifest(
        &path,
        &expected,
        "general_gemm_test",
        Path::new("src/lib.rs"),
        &directory.0,
    )
    .unwrap();
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x78; 32]),
        BuildSession::from_bytes([0x77; 16]),
    )
    .unwrap();
    assert!(matches!(
        execute_general_gemm_pipeline_v1(
            OpaqueFrontendToken(2),
            [
                unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
                unit_with_frontend(
                    GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                    frontend_with_provider(0x44),
                ),
            ],
            config,
            &directory.0,
            &producer,
            attempt,
        ),
        Err(GeneralGemmPipelineErrorV1::FrontendBindingSubstitution)
    ));
    fail_build_attempt(&directory.0, &producer, attempt).unwrap();
}

#[test]
fn unreviewed_verus_is_rejected_before_any_handoff() {
    let directory = TestDirectory::new("verus-substitution");
    let (path, expected) = local_manifest(&directory);
    let config = PreparedGeneralGemmPipelineV1::from_manifest(
        &path,
        &expected,
        "general_gemm_test",
        Path::new("src/lib.rs"),
        &directory.0,
    )
    .unwrap();
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x7a; 32]),
        BuildSession::from_bytes([0x79; 16]),
    )
    .unwrap();
    assert!(matches!(
        execute_general_gemm_pipeline_v1(
            OpaqueFrontendToken(3),
            [
                unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
                unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1),
            ],
            config,
            &directory.0,
            &producer,
            attempt,
        ),
        Err(GeneralGemmPipelineErrorV1::Verifier(reason))
            if reason.contains("VerusDigestMismatch")
    ));
    for slot in [
        CompilerModuleHandoffSlotV1::GeneralGemmReference,
        CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly,
    ] {
        assert!(
            consume_compiler_module_handoff_in_slot_v1(&directory.0, &producer, attempt, slot,)
                .is_err()
        );
    }
    fail_build_attempt(&directory.0, &producer, attempt).unwrap();
}

#[test]
fn worker_rejection_after_consumption_is_removed_with_the_failed_generation() {
    let directory = TestDirectory::new("post-consumption-failure");
    let (path, expected) = local_manifest(&directory);
    let config = PreparedGeneralGemmPipelineV1::from_manifest(
        &path,
        &expected,
        "general_gemm_test",
        Path::new("src/lib.rs"),
        &directory.0,
    )
    .unwrap();
    let producer = producer();
    let session = BuildSession::from_bytes([0x74; 16]);
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x75; 32]),
        session,
    )
    .unwrap();
    assert!(matches!(
        execute_general_gemm_schedule_machine_core_v1(
            unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
            &config,
            &directory.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV1::GeneralGemmReference,
        ),
        Err(GeneralGemmPipelineErrorV1::Worker(_))
    ));
    assert!(
        consume_compiler_module_handoff_in_slot_v1(
            &directory.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV1::GeneralGemmReference,
        )
        .is_err()
    );
    fail_build_attempt(&directory.0, &producer, attempt).unwrap();
    assert!(
        publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, b"stale").is_err(),
        "failed generation remained claimable"
    );

    let replacement = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x76; 32]),
        session,
    )
    .unwrap();
    publish_compiler_module_handoff_v1(&directory.0, &producer, replacement, b"replacement")
        .unwrap();
    let handoff_parent = fs::read_dir(&directory.0)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".fe2o3-compiler-module-handoff-v1-")
        })
        .unwrap();
    assert_eq!(
        fs::read_dir(handoff_parent).unwrap().count(),
        1,
        "replacement publication did not reap the failed generation's stale slot"
    );
    assert_eq!(
        consume_compiler_module_handoff_v1(&directory.0, &producer, replacement)
            .unwrap()
            .bytes(),
        b"replacement"
    );
    fail_build_attempt(&directory.0, &producer, replacement).unwrap();
}

#[test]
fn manifest_profile_mutation_cannot_reuse_the_managed_config_identity() {
    let directory = TestDirectory::new("manifest-substitution");
    let (path, expected) = local_manifest(&directory);
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["general_gemm_v1"]["proof_timeout_seconds"] = json!(121);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        PreparedGeneralGemmPipelineV1::from_manifest(
            &path,
            &expected,
            "general_gemm_test",
            Path::new("src/lib.rs"),
            &directory.0,
        ),
        Err(GeneralGemmPipelineErrorV1::Configuration(reason))
            if reason.contains("expected configuration identity differs")
    ));
}

#[test]
#[ignore = "requires the measured upstream LLVM 22.1.8 worker"]
fn measured_worker_runs_both_schedules_inside_the_live_attempt() {
    let worker_path = PathBuf::from(MEASURED_WORKER);
    let worker_length = fs::metadata(&worker_path).unwrap().len();
    let worker_sha256 = decode_sha256(MEASURED_WORKER_SHA256, "measured worker").unwrap();
    let directory = TestDirectory::new("measured-pair");
    let (path, expected) = manifest(
        &directory,
        &worker_path,
        Path::new(MEASURED_VERUS),
        worker_sha256,
        worker_length,
        MEASURED_WORKER_BUILD_ID,
        MEASURED_LLVM_BUILD_ID,
    );
    let config = PreparedGeneralGemmPipelineV1::from_manifest(
        &path,
        &expected,
        "general_gemm_test",
        Path::new("src/lib.rs"),
        &directory.0,
    )
    .unwrap();
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x81; 32]),
        BuildSession::from_bytes([0x82; 16]),
    )
    .unwrap();
    let result = execute_general_gemm_pipeline_v1(
        OpaqueFrontendToken(0x91),
        [
            unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
            unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1),
        ],
        config,
        &directory.0,
        &producer,
        attempt,
    )
    .unwrap();
    let (frontend, config, qualifications) = result.into_join_inputs();
    assert_eq!(frontend.0, 0x91);
    assert_eq!(config.proof_timeout_seconds(), 120);
    for (qualification, schedule, slot) in [
        (
            &qualifications[0],
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            CompilerModuleHandoffSlotV1::GeneralGemmReference,
        ),
        (
            &qualifications[1],
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
            CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly,
        ),
    ] {
        assert_eq!(qualification.unit.schedule(), schedule);
        assert_eq!(qualification.verifier.schedule(), schedule);
        assert!(
            !qualification
                .verifier
                .closure()
                .can_enter_compiler_proof_gate()
        );
        assert_eq!(qualification.managed.output_directory, directory.0);
        assert_eq!(qualification.managed.producer, producer);
        assert_eq!(qualification.managed.attempt, attempt);
        assert_eq!(qualification.managed.slot, slot);
        assert_eq!(qualification.managed.handoff_receipt.attempt(), attempt);
        assert_eq!(qualification.managed.handoff_receipt.slot(), slot);
        assert_eq!(
            qualification.managed.handoff_receipt.identity(),
            qualification.managed.consumed_handoff
        );
        assert_eq!(qualification.observation.schedule(), schedule);
        assert_eq!(
            qualification.observation.vector_global_load_count(),
            u32::from(schedule == GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1)
        );
        assert!(!qualification.observation.grants_artifact_authority());
        assert!(!qualification.observation.grants_publication_authority());
        assert!(!qualification.observation.grants_load_authority());
        assert!(!qualification.observation.grants_launch_authority());
    }
    let error = finish_build_attempt(&directory.0, &producer, attempt).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("without an authorized device backend")
    );
    fail_build_attempt(&directory.0, &producer, attempt).unwrap();
}
