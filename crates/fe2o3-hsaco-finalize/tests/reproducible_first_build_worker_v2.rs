#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession, CompilerModuleHandoffIdentityV1,
    ConsumedCompilerModuleHandoffV1, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FirstBuildWorkerV2Error, LinkOptionV1, LinkPlanError, PinnedWorkerV1,
    WorkerEvidenceClassV1, WorkerEvidenceClassV2, WorkerExecutionErrorKind,
    WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1, WorkerProtocolError, WorkerRequestConstructionError, WorkerStageV1,
    execute_reproducible_first_build_worker_v2,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};

const WORKER_ID: &str = "fixture-worker-v1";
const LLVM_ID: &str = "fixture-llvm-v1";
const MODULE: &[u8] = b"define amdgpu_kernel void @workflow_kernel() { ret void }\ndefine i32 @workflow_export(i32 %value) { ret i32 %value }\n";
const PROVIDER: &[u8] = b"exact workflow external provider";
const OUTPUT: &[u8] = b"fixture-output";

fn fixture_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-executor-fixture"))
}

fn pinned() -> PinnedWorkerV1 {
    let bytes = fs::read(fixture_path()).unwrap();
    let measurement =
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, LLVM_ID).unwrap();
    PinnedWorkerV1::open(fixture_path(), measurement).unwrap()
}

fn limits() -> WorkerExecutionLimitsV1 {
    WorkerExecutionLimitsV1::new(Duration::from_secs(2), 16 * 1024, 1024).unwrap()
}

fn options() -> Vec<LinkOptionV1> {
    [
        ("verify-each", "true"),
        ("code-object-version", "6"),
        ("strip-debug", "true"),
        ("opt-level", "2"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).unwrap())
    .collect()
}

fn provider() -> WorkerInputV1 {
    WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, PROVIDER.to_vec()).unwrap()
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-reproducible-first-build-{}-{}",
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

fn compiler_export() -> CompilerFfiContractV1 {
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    let semantic_identity = [0x53; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "workflow_export",
        calling_convention: "C",
        code_object_version: 6,
        target: "gfx942:xnack-",
        physical_abi: ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    };
    CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Export,
        CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerCodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            "workflow_fixture",
            "workflow_fixture::workflow_export",
            [0x35; 16],
            "_RINvNtCs1234_16workflow_fixture15workflow_export",
        )
        .unwrap(),
        "workflow_export",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}

fn canonical_handoff(extra_kernels: &[&str]) -> CompilerModuleHandoffV2 {
    let mut envelope = CompilerFfiEnvelopeBuilderV1::new(
        CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerCodeObjectVersion::V6,
        1,
    )
    .unwrap();
    envelope.push(compiler_export()).unwrap();
    let mut entries = vec![
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            "workflow_kernel".to_owned(),
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "workflow_kernel.kd".to_owned(),
        ),
        (
            CompilerModuleSymbolRoleV1::DeviceFfiExport,
            "workflow_export".to_owned(),
        ),
    ];
    for kernel in extra_kernels {
        entries.push((
            CompilerModuleSymbolRoleV1::KernelEntry,
            (*kernel).to_owned(),
        ));
        entries.push((
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            format!("{kernel}.kd"),
        ));
    }
    entries.sort();
    let manifest = CompilerModuleSymbolManifestV1::new(entries).unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerCodeObjectVersion::V6,
        envelope.finish().unwrap(),
        manifest,
        MODULE,
    )
    .unwrap()
}

fn consumed_handoff(
    directory: &TestDirectory,
) -> (
    BuildAttempt,
    CompilerModuleHandoffIdentityV1,
    ConsumedCompilerModuleHandoffV1,
) {
    consumed_handoff_with_extra(directory, &[])
}

fn consumed_handoff_with_extra(
    directory: &TestDirectory,
    extra_kernels: &[&str],
) -> (
    BuildAttempt,
    CompilerModuleHandoffIdentityV1,
    ConsumedCompilerModuleHandoffV1,
) {
    consumed_bytes(
        directory,
        canonical_handoff(extra_kernels).canonical_bytes(),
    )
}

fn consumed_bytes(
    directory: &TestDirectory,
    bytes: &[u8],
) -> (
    BuildAttempt,
    CompilerModuleHandoffIdentityV1,
    ConsumedCompilerModuleHandoffV1,
) {
    let producer = ProducerIdentity::from_codegen(
        "workflow_fixture",
        Some(Path::new("src/workflow_fixture.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0x61; 32]),
        BuildSession::from_bytes([0x62; 16]),
    )
    .unwrap();
    let receipt =
        publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, bytes).unwrap();
    let consumed = consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
    (attempt, receipt.identity(), consumed)
}

#[test]
fn derives_exact_plan_and_returns_only_inert_dual_execution_evidence() {
    let directory = TestDirectory::new();
    let (attempt, handoff_identity, consumed) = consumed_handoff(&directory);
    let worker = pinned();
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed,
        &worker,
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();

    assert_eq!(evidence.attempt(), attempt);
    assert_eq!(evidence.handoff_identity(), handoff_identity);
    assert_eq!(
        evidence.compiler_envelope_identity(),
        evidence.compiler_envelope().identity()
    );
    assert_eq!(
        evidence.manifest_identity(),
        evidence.symbol_manifest().identity()
    );
    assert_eq!(
        evidence
            .symbol_manifest()
            .symbols(CompilerModuleSymbolRoleV1::KernelEntry)
            .collect::<Vec<_>>(),
        ["workflow_kernel"]
    );
    assert_eq!(evidence.worker_measurement(), worker.measurement());
    assert_eq!(
        evidence.candidate().worker_executable(),
        worker.measurement().executable()
    );
    assert_eq!(
        evidence.authorized().worker_executable(),
        worker.measurement().executable()
    );
    assert_eq!(
        evidence.candidate().evidence_class(),
        WorkerEvidenceClassV1::GenericLink
    );
    assert_eq!(
        evidence.authorized().evidence_class(),
        WorkerEvidenceClassV2::CompilerFfiLink
    );
    assert_eq!(evidence.output_bytes(), OUTPUT);
    assert_eq!(
        evidence.output_identity(),
        ContentIdentityV1::calculate(OUTPUT)
    );
    evidence.plan().verify_output_bytes(OUTPUT).unwrap();
    assert_eq!(evidence.plan().inputs().len(), 2);
    assert_eq!(evidence.plan().provenance().len(), 3);
    assert_ne!(evidence.identity().as_bytes(), &[0; 32]);
    assert_ne!(
        evidence.candidate().response().request_id(),
        evidence.authorized().response().request_id()
    );
    assert_eq!(
        evidence.authorized_request_id(),
        evidence.authorized().response().request_id()
    );
    assert_eq!(
        evidence.authorized_request_identity(),
        evidence.authorized().response().request_identity()
    );
    assert!(!evidence.grants_publication_authority());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
}

#[test]
fn identical_first_build_inputs_are_deterministic_across_independent_roots() {
    let first_directory = TestDirectory::new();
    let second_directory = TestDirectory::new();
    let (_, _, first_consumed) = consumed_handoff(&first_directory);
    let (_, _, second_consumed) = consumed_handoff(&second_directory);
    let worker = pinned();
    let first = execute_reproducible_first_build_worker_v2(
        first_consumed,
        &worker,
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();
    let mut reordered_options = options();
    reordered_options.reverse();
    let second = execute_reproducible_first_build_worker_v2(
        second_consumed,
        &worker,
        vec![provider()],
        reordered_options,
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();

    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.plan().identity(), second.plan().identity());
    assert_eq!(
        first.candidate().response().request_id(),
        second.candidate().response().request_id()
    );
    assert_eq!(
        first.candidate().response().request_identity(),
        second.candidate().response().request_identity()
    );
    assert_eq!(
        first.authorized().response().request_identity(),
        second.authorized().response().request_identity()
    );
}

#[test]
fn exact_byte_mismatch_retains_both_inert_executions_and_fails_closed() {
    let directory = TestDirectory::new();
    let (_, _, consumed) = consumed_handoff_with_extra(&directory, &["workflow_mismatch"]);
    let error = execute_reproducible_first_build_worker_v2(
        consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap_err();

    let FirstBuildWorkerV2Error::OutputMismatch {
        candidate,
        authorized,
    } = error
    else {
        panic!("unexpected error: {error:?}");
    };
    assert_eq!(candidate.response().output().unwrap().bytes(), OUTPUT);
    assert_eq!(
        authorized.response().output().unwrap().bytes(),
        b"changed-output"
    );
    assert!(!candidate.grants_load_authority());
    assert!(!authorized.grants_load_authority());
}

#[test]
fn candidate_and_v2_failure_responses_are_distinguished() {
    let candidate_directory = TestDirectory::new();
    let (_, _, candidate_consumed) =
        consumed_handoff_with_extra(&candidate_directory, &["workflow_candidate_failure"]);
    let candidate_error = execute_reproducible_first_build_worker_v2(
        candidate_consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap_err();
    let candidate_diagnostic = candidate_error.to_string();
    let FirstBuildWorkerV2Error::CandidateDidNotProduceOutput(candidate) = candidate_error else {
        panic!("unexpected error: {candidate_error:?}");
    };
    assert_eq!(candidate.response().stage(), WorkerStageV1::Codegen);
    assert!(candidate.response().output().is_none());
    assert!(candidate_diagnostic.contains("Codegen: []"));

    let v2_directory = TestDirectory::new();
    let (_, _, v2_consumed) = consumed_handoff_with_extra(&v2_directory, &["workflow_v2_failure"]);
    let v2_error = execute_reproducible_first_build_worker_v2(
        v2_consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap_err();
    let v2_diagnostic = v2_error.to_string();
    let FirstBuildWorkerV2Error::AuthorizedDidNotProduceOutput {
        candidate,
        authorized,
    } = v2_error
    else {
        panic!("unexpected error: {v2_error:?}");
    };
    assert!(candidate.response().output().is_some());
    assert_eq!(authorized.response().stage(), WorkerStageV1::Codegen);
    assert!(authorized.response().output().is_none());
    assert!(v2_diagnostic.contains("Codegen: []"));
}

#[test]
fn candidate_and_v2_identity_corruption_are_distinguished() {
    let candidate_directory = TestDirectory::new();
    let (_, _, candidate_consumed) =
        consumed_handoff_with_extra(&candidate_directory, &["workflow_candidate_bad_response"]);
    let candidate_error = execute_reproducible_first_build_worker_v2(
        candidate_consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap_err();
    assert!(matches!(
        candidate_error,
        FirstBuildWorkerV2Error::CandidateExecution(ref error)
            if error.kind() == &WorkerExecutionErrorKind::RequestIdentityMismatch
    ));

    let v2_directory = TestDirectory::new();
    let (_, _, v2_consumed) =
        consumed_handoff_with_extra(&v2_directory, &["workflow_v2_bad_response"]);
    let v2_error = execute_reproducible_first_build_worker_v2(
        v2_consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap_err();
    assert!(matches!(
        v2_error,
        FirstBuildWorkerV2Error::AuthorizedExecution(ref error)
            if matches!(
                error.kind(),
                WorkerExecutionErrorKind::DecodeResponse(
                    WorkerProtocolError::RequestIdentityMismatch
                )
            )
    ));
}

#[test]
fn malformed_handoff_and_invalid_options_fail_before_candidate_execution() {
    let malformed_directory = TestDirectory::new();
    let (_, _, malformed) = consumed_bytes(&malformed_directory, b"not-a-canonical-handoff");
    assert!(matches!(
        execute_reproducible_first_build_worker_v2(
            malformed,
            &pinned(),
            vec![provider()],
            options(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        ),
        Err(FirstBuildWorkerV2Error::CompilerModuleHandoff(_))
    ));

    let option_directory = TestDirectory::new();
    let (_, _, consumed) = consumed_handoff(&option_directory);
    let mut invalid_options = options();
    invalid_options.push(LinkOptionV1::new("unbounded-escape-hatch", "true").unwrap());
    assert!(matches!(
        execute_reproducible_first_build_worker_v2(
            consumed,
            &pinned(),
            vec![provider()],
            invalid_options,
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        ),
        Err(FirstBuildWorkerV2Error::RequestConstruction(
            WorkerRequestConstructionError::UnsupportedLinkOption(name)
        )) if name == "unbounded-escape-hatch"
    ));
}

#[test]
fn caller_output_bound_is_enforced_on_the_candidate() {
    let directory = TestDirectory::new();
    let (_, _, consumed) = consumed_handoff(&directory);
    let error = execute_reproducible_first_build_worker_v2(
        consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(1).unwrap(),
        limits(),
    )
    .unwrap_err();
    let is_output_limit = matches!(
        &error,
        FirstBuildWorkerV2Error::CandidateExecution(execution)
            if execution.kind() == &WorkerExecutionErrorKind::OutputLimitExceeded
    );
    assert!(
        is_output_limit,
        "unexpected candidate output-bound error: {error:#?}"
    );
}

#[test]
fn duplicate_content_under_a_different_input_kind_is_rejected_before_execution() {
    let directory = TestDirectory::new();
    let (_, _, consumed) = consumed_handoff(&directory);
    let duplicate =
        WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, MODULE.to_vec()).unwrap();
    let error = execute_reproducible_first_build_worker_v2(
        consumed,
        &pinned(),
        vec![duplicate],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        FirstBuildWorkerV2Error::LinkPlan(LinkPlanError::DuplicateInput(identity))
            if identity == ContentIdentityV1::calculate(MODULE)
    ));
}
