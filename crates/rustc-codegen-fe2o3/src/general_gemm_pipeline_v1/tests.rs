use super::*;

use std::ffi::OsStr;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifact_transaction::{
    BuildInvocation, begin_build_attempt, consume_compiler_module_handoff_v1, fail_build_attempt,
    publish_compiler_module_handoff_in_slot_v1, publish_compiler_module_handoff_v1,
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
const TEST_RUNTIME_CLOSURE_V2_ROOT: &str = "/opt/fe2o3/verus-runtime-v2/0.2026.08.02-test";
const MEASURED_WORKER_SHA256: &str =
    "0b4936777b08d7d9d864bf357ab4f14cac33a0bb0a13c479209a26c1da808d35";
const MEASURED_WORKER_BUILD_ID: &str =
    "fe2o3-worker-v1-sha256-1769826adfd0cc9832015371d9df79bb9128093a28be89144aa797d8155151a0";
const MEASURED_LLVM_BUILD_ID: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
const TEST_CODEGEN_BACKEND_BUILD_OBSERVATION_V2: [u8; 32] = [0x5a; 32];

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

#[test]
fn manifest_reader_rejects_symlinks_oversized_files_and_lexical_aliases() {
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("manifest-objects");
    let regular = directory.0.join("config.json");
    fs::write(&regular, b"{}").unwrap();

    let symlink_path = directory.0.join("config-link.json");
    symlink(&regular, &symlink_path).unwrap();
    assert!(read_bounded(&symlink_path, MAX_CONFIG_BYTES, "configuration").is_err());

    let oversized = directory.0.join("oversized.json");
    fs::File::create(&oversized)
        .unwrap()
        .set_len((MAX_CONFIG_BYTES + 1) as u64)
        .unwrap();
    assert!(read_bounded(&oversized, MAX_CONFIG_BYTES, "configuration").is_err());

    let lexical_alias = regular
        .parent()
        .unwrap()
        .join(".")
        .join(regular.file_name().unwrap());
    assert!(require_closed_child_manifest_path(&lexical_alias, "configuration").is_err());
}

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
    request_with_identity(frontend, schedule, 0x11)
}

fn request_with_identity(
    frontend: &GeneralGemmFrontendSemanticBindingV1,
    schedule: GeneralGemmScheduleV1,
    request_identity: u8,
) -> CompileRequestV1 {
    request_with_identity_and_compiler(frontend, schedule, request_identity, 0x13)
}

fn request_with_identity_and_compiler(
    frontend: &GeneralGemmFrontendSemanticBindingV1,
    schedule: GeneralGemmScheduleV1,
    request_identity: u8,
    compiler_profile: u8,
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
        RequestIdentityV1::from_untrusted_bytes(identity(request_identity)),
        KernelInstanceIdentityV1::from_untrusted_bytes(identity(0x12)),
        CompilerProfileIdentityV1::from_untrusted_bytes(identity(compiler_profile)),
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

fn unit_with_request_identity(
    schedule: GeneralGemmScheduleV1,
    request_identity: u8,
) -> GeneralGemmSymbolicCompilationUnitV1 {
    let frontend = frontend();
    let request = request_with_identity(&frontend, schedule, request_identity);
    GeneralGemmSymbolicCompilationUnitV1::checked(
        &request,
        frontend,
        schedule,
        GeneralGemmLoweringLimitsV1::default(),
    )
    .unwrap()
}

fn unit_with_compiler_profile(
    schedule: GeneralGemmScheduleV1,
    compiler_profile: u8,
) -> GeneralGemmSymbolicCompilationUnitV1 {
    let frontend = frontend();
    let request = request_with_identity_and_compiler(&frontend, schedule, 0x11, compiler_profile);
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
    runtime_closure_v2_root: &Path,
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
            "runtime_closure_v2_manifest_sha256": hex(&GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256),
            "runtime_closure_v2_root": runtime_closure_v2_root
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
        Path::new(TEST_RUNTIME_CLOSURE_V2_ROOT),
        sha256,
        bytes.len() as u64,
        "test-worker-v1",
        "test-llvm-v1",
    )
}

fn test_compile_unit(directory: &TestDirectory) -> GeneralGemmManifestCompileUnitV1<'_> {
    GeneralGemmManifestCompileUnitV1 {
        codegen_backend_build_observation_v2: TEST_CODEGEN_BACKEND_BUILD_OBSERVATION_V2,
        crate_name: "general_gemm_test",
        source: Path::new("src/lib.rs"),
        working_directory: &directory.0,
    }
}

fn parse_test_manifest(
    path: &Path,
    expected_identity: &str,
    directory: &TestDirectory,
) -> Result<ParsedGeneralGemmPipelineV1, GeneralGemmPipelineErrorV1> {
    parse_general_gemm_manifest_v1(
        path,
        expected_identity,
        Path::new(TEST_RUNTIME_CLOSURE_V2_ROOT),
        &hex(&GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256),
        test_compile_unit(directory),
    )
}

fn prepare_test_manifest(
    path: &Path,
    expected_identity: &str,
    directory: &TestDirectory,
) -> Result<PreparedGeneralGemmPipelineV1, GeneralGemmPipelineErrorV1> {
    PreparedGeneralGemmPipelineV1::from_manifest(
        path,
        expected_identity,
        Path::new(TEST_RUNTIME_CLOSURE_V2_ROOT),
        &hex(&GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256),
        test_compile_unit(directory),
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
    let config = parse_test_manifest(&path, &expected, &directory).unwrap();
    assert_eq!(
        config.runtime_closure_v2_root,
        Path::new(TEST_RUNTIME_CLOSURE_V2_ROOT)
    );
    assert_eq!(
        config.runtime_closure_v2_manifest_sha256,
        GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256
    );
    assert_eq!(
        config.codegen_backend_build_observation_v2,
        TEST_CODEGEN_BACKEND_BUILD_OBSERVATION_V2
    );
    assert_eq!(config.proof_timeout_seconds, 120);
    assert_eq!(hex(&config.identity.as_bytes()), expected);

    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["general_gemm_v1"]["profile"] = json!("single-schedule-v1");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        parse_test_manifest(&path, &expected, &directory),
        Err(GeneralGemmPipelineErrorV1::Configuration(reason))
            if reason.contains("unsupported")
    ));
    value["general_gemm_v1"]["extra"] = json!(true);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        parse_test_manifest(&path, &expected, &directory),
        Err(GeneralGemmPipelineErrorV1::Configuration(reason))
            if reason.contains("unknown, missing, or reordered")
    ));
}

#[test]
fn backend_build_observation_requires_exact_nonzero_lowercase_sha256() {
    let zero = "0".repeat(64);
    let uppercase = "AA".repeat(32);
    let short = "5a".repeat(31);
    let valid = "5a".repeat(32);
    for value in [
        None,
        Some(OsStr::new("")),
        Some(OsStr::new(&zero)),
        Some(OsStr::new(&uppercase)),
        Some(OsStr::new(&short)),
    ] {
        assert!(parse_codegen_backend_build_observation_v2(value).is_err());
    }
    assert_eq!(
        parse_codegen_backend_build_observation_v2(Some(OsStr::new(&valid))).unwrap(),
        TEST_CODEGEN_BACKEND_BUILD_OBSERVATION_V2
    );
}

#[test]
fn runtime_pair_retention_uses_one_lease_and_exact_boundaries() {
    assert_eq!(
        RUNTIME_CLOSURE_V2_PAIR_BOUNDARIES,
        [
            "post-admission before the qualification pair",
            "before the reference schedule",
            "between schedule proof evaluations",
            "after schedule proof evaluations",
            "between schedule machine evaluations",
            "after the qualification pair",
        ]
    );
    let source = include_str!("../general_gemm_pipeline_v1.rs");
    assert_eq!(
        source
            .matches("runtime_closure_v2: GeneralGemmVerusRuntimeClosureLeaseV2")
            .count(),
        1
    );
    assert!(!source.contains("runtime_closure_v2.clone()"));
    assert_eq!(
        source
            .matches("GeneralGemmVerusRuntimeClosureLeaseV2::open(")
            .count(),
        1
    );
}

#[test]
fn reordered_schedule_pair_is_rejected_before_any_handoff() {
    let directory = TestDirectory::new("schedule-reorder");
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x72; 32]),
        BuildSession::from_bytes([0x71; 16]),
    )
    .unwrap();
    assert!(matches!(
        validate_general_gemm_pair_inputs_v1(&[
            unit(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1),
            unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
        ]),
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
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x78; 32]),
        BuildSession::from_bytes([0x77; 16]),
    )
    .unwrap();
    assert!(matches!(
        validate_general_gemm_pair_inputs_v1(&[
            unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
            unit_with_frontend(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_with_provider(0x44),
            ),
        ]),
        Err(GeneralGemmPipelineErrorV1::FrontendBindingSubstitution)
    ));
    fail_build_attempt(&directory.0, &producer, attempt).unwrap();
}

#[test]
fn pair_request_substitution_is_rejected_before_proof_or_handoff() {
    let directory = TestDirectory::new("pair-request-substitution");
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x88; 32]),
        BuildSession::from_bytes([0x87; 16]),
    )
    .unwrap();
    assert!(matches!(
        validate_general_gemm_pair_inputs_v1(&[
            unit_with_request_identity(GeneralGemmScheduleV1::ReferenceWave64Xor4V1, 0x11),
            unit_with_request_identity(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                0x12,
            ),
        ]),
        Err(GeneralGemmPipelineErrorV1::PairRequestSubstitution)
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
fn backend_compiler_profile_substitution_is_rejected_before_handoff() {
    assert!(matches!(
        validate_general_gemm_pair_inputs_v1(&[
            unit_with_compiler_profile(GeneralGemmScheduleV1::ReferenceWave64Xor4V1, 0x13),
            unit_with_compiler_profile(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                0x14,
            ),
        ]),
        Err(GeneralGemmPipelineErrorV1::PairRequestSubstitution)
    ));
}

#[test]
fn positive_analysis_never_creates_frontend_correspondence() {
    let error = consume_general_gemm_production_import_v1(Some(
        GeneralGemmMirImportV1::PositiveAnalysisBlocked,
    ))
    .expect_err("positive analysis must remain non-executable");
    assert!(
        error
            .to_string()
            .contains("production frontend correspondence is disabled")
    );
}

#[test]
fn mutation_oracle_never_creates_a_handoff_and_failed_attempt_is_unclaimable() {
    let directory = TestDirectory::new("mutation-oracle-route");
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x8a; 32]),
        BuildSession::from_bytes([0x89; 16]),
    )
    .unwrap();
    let error = consume_general_gemm_production_import_v1(Some(
        GeneralGemmMirImportV1::VerifiedMutationOracle,
    ))
    .expect_err("mutation oracle must remain non-executable");
    assert!(
        error
            .to_string()
            .contains("mutation oracle is non-executable")
    );
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
    assert!(
        publish_compiler_module_handoff_in_slot_v1(
            &directory.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV1::GeneralGemmReference,
            b"stale",
        )
        .is_err(),
        "failed mutation-oracle attempt remained claimable"
    );
}

#[test]
fn runtime_closure_open_failure_is_rejected_before_any_handoff() {
    let directory = TestDirectory::new("runtime-closure-open-failure");
    let (path, expected) = local_manifest(&directory);
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x7a; 32]),
        BuildSession::from_bytes([0x79; 16]),
    )
    .unwrap();
    let error = match prepare_test_manifest(&path, &expected, &directory) {
        Ok(_) => panic!("missing runtime closure was admitted"),
        Err(error) => error,
    };
    // A missing child reports ObjectType on one filesystem; a separately
    // mounted or insufficiently protected /opt is rejected before child lookup
    // as SymlinkOrTraversal or Protection.
    assert!(
        matches!(
            &error,
            GeneralGemmPipelineErrorV1::RuntimeClosure { boundary, error }
                if boundary.contains("before the qualification pair")
                    && matches!(
                        error.kind(),
                        fe2o3_verifier::GeneralGemmRuntimeClosureErrorKindV2::ObjectType
                            | fe2o3_verifier::GeneralGemmRuntimeClosureErrorKindV2::Protection
                            | fe2o3_verifier::GeneralGemmRuntimeClosureErrorKindV2::SymlinkOrTraversal
                    )
        ),
        "unexpected runtime-closure admission failure: {error:?}"
    );
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
    let config = parse_test_manifest(&path, &expected, &directory).unwrap();
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
        parse_test_manifest(&path, &expected, &directory),
        Err(GeneralGemmPipelineErrorV1::Configuration(reason))
            if reason.contains("expected configuration identity differs")
    ));
}

#[test]
fn runtime_root_path_environment_and_manifest_substitution_are_rejected() {
    let directory = TestDirectory::new("runtime-pin-substitution");
    let (path, expected) = local_manifest(&directory);
    let manifest_sha256 = hex(&GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256);

    for (expected_root, expected_manifest, reason) in [
        (
            Path::new("/opt/fe2o3/verus-runtime-v2/substituted"),
            manifest_sha256.as_str(),
            "parent-authenticated child environment",
        ),
        (
            Path::new(TEST_RUNTIME_CLOSURE_V2_ROOT),
            "77",
            "64 lowercase hexadecimal",
        ),
        (
            Path::new(TEST_RUNTIME_CLOSURE_V2_ROOT),
            "77f16c7b1b2c68b3fa5a16f8efdfc48b98022165c7829a567118a380f916c213",
            "compiled-in reviewed manifest",
        ),
    ] {
        let error = match parse_general_gemm_manifest_v1(
            &path,
            &expected,
            expected_root,
            expected_manifest,
            test_compile_unit(&directory),
        ) {
            Err(error) => error,
            Ok(_) => panic!("substituted runtime pin was admitted"),
        };
        assert!(error.to_string().contains(reason), "{error}");
    }

    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["general_gemm_v1"]["runtime_closure_v2_root"] =
        json!("/opt/fe2o3/verus-runtime-v2/../substituted");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = match parse_test_manifest(&path, &expected, &directory) {
        Err(error) => error,
        Ok(_) => panic!("lexical runtime-root alias was admitted"),
    };
    assert!(error.to_string().contains("canonical absolute UTF-8"));

    value["general_gemm_v1"]["runtime_closure_v2_root"] = json!(TEST_RUNTIME_CLOSURE_V2_ROOT);
    value["general_gemm_v1"]["runtime_closure_v2_manifest_sha256"] = json!("77".repeat(32));
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let error = match parse_test_manifest(&path, &expected, &directory) {
        Err(error) => error,
        Ok(_) => panic!("substituted runtime manifest was admitted"),
    };
    assert!(error.to_string().contains("compiled-in reviewed manifest"));
}

#[test]
#[ignore = "requires the measured worker and provisioned root-owned runtime closure V2"]
fn measured_pair_retains_one_runtime_generation_and_proof_execution_stays_closed() {
    let worker_path = PathBuf::from(MEASURED_WORKER);
    let worker_length = fs::metadata(&worker_path).unwrap().len();
    let worker_sha256 = decode_sha256(MEASURED_WORKER_SHA256, "measured worker").unwrap();
    let directory = TestDirectory::new("measured-pair");
    let (path, expected) = manifest(
        &directory,
        &worker_path,
        Path::new("/opt/fe2o3/verus-runtime-v2/0.2026.08.02"),
        worker_sha256,
        worker_length,
        MEASURED_WORKER_BUILD_ID,
        MEASURED_LLVM_BUILD_ID,
    );
    let config = PreparedGeneralGemmPipelineV1::from_manifest(
        &path,
        &expected,
        Path::new("/opt/fe2o3/verus-runtime-v2/0.2026.08.02"),
        &hex(&GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256),
        test_compile_unit(&directory),
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
    let result = execute_general_gemm_verifier_closure_v1(
        &unit(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
        &config,
    );
    assert!(matches!(
        result,
        Err(GeneralGemmPipelineErrorV1::Verifier(reason))
            if reason.contains("AuthenticatedRuntimeClosureUnavailable")
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
