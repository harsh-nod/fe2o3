#![cfg(target_os = "linux")]

use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use fe2o3_hsaco_finalize::{
    CompilerFfiCodeObjectVersion, CompilerFfiContractV1, CompilerFfiDeviceTargetV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    ContentIdentityV1, InertWorkerExecutionV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1,
    LinkOutputV1, LinkSymbolClosureV1, MultiInputLinkPlanV1, PinnedWorkerV1, ProvenanceNodeV1,
    WorkerEvidenceClassV2, WorkerExecutionErrorKind, WorkerExecutionLimitsV1, WorkerInputKindV1,
    WorkerInputV1, WorkerMeasurementV1, WorkerOptimizationLevelV1, WorkerOptionsV1,
    WorkerOutputConstraintsV1, WorkerRequestV1, WorkerRequestV2, construct_worker_request_v2,
    stage_compiler_ffi_envelope_v1, stage_exact_compiler_module_artifact_v1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};

const WORKER_ID: &str = "fixture-worker-v1";
const LLVM_ID: &str = "fixture-llvm-v1";

fn fixture_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-executor-fixture"))
}

fn pinned() -> PinnedWorkerV1 {
    let bytes = fs::read(fixture_path()).unwrap();
    let measurement =
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, LLVM_ID).unwrap();
    PinnedWorkerV1::open(fixture_path(), measurement).unwrap()
}

fn request(mode: u8, input_bytes: usize, output_bytes: u64) -> WorkerRequestV1 {
    WorkerRequestV1::new(
        [mode; 32],
        LLVM_ID,
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CodeObjectVersion::V6,
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
        vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![mode; input_bytes]).unwrap()],
        vec![],
        vec![],
        WorkerOutputConstraintsV1::new(output_bytes).unwrap(),
    )
    .unwrap()
}

fn limits() -> WorkerExecutionLimitsV1 {
    WorkerExecutionLimitsV1::new(Duration::from_millis(300), 4096, 1024).unwrap()
}

fn execute(mode: u8) -> Result<InertWorkerExecutionV1, fe2o3_hsaco_finalize::WorkerExecutionError> {
    pinned().execute(&request(mode, 1, 1024), limits())
}

fn v2_request(pinned: &PinnedWorkerV1) -> WorkerRequestV2 {
    let module_bytes = b"fixture compiler module".to_vec();
    let module = WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, module_bytes.clone()).unwrap();
    let output_identity = ContentIdentityV1::calculate(b"fixture-output");
    let link_input = LinkInputV1::new(module.identity(), request_target());
    let plan = MultiInputLinkPlanV1::canonicalized(
        request_target(),
        vec![link_input],
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
        LinkOutputV1::new(output_identity, request_target()),
        vec![
            ProvenanceNodeV1::new(module.identity(), vec![]).unwrap(),
            ProvenanceNodeV1::new(output_identity, vec![module.identity()]).unwrap(),
        ],
    )
    .unwrap();
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, vec![WorkerInputKindV1::LlvmBitcode]).unwrap();
    let mut envelope_builder = CompilerFfiEnvelopeBuilderV1::new(
        CompilerFfiDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerFfiCodeObjectVersion::V6,
        1,
    )
    .unwrap();
    envelope_builder.push(v2_export_contract()).unwrap();
    let envelope = envelope_builder.finish().unwrap();
    construct_worker_request_v2(
        &plan,
        pinned.measurement(),
        request_target(),
        CodeObjectVersion::V6,
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
        stage_compiler_ffi_envelope_v1(envelope),
        stage_exact_compiler_module_artifact_v1(WorkerInputKindV1::LlvmBitcode, module_bytes)
            .unwrap(),
        vec![],
        &input_kinds,
        &LinkSymbolClosureV1::new(
            vec!["fixture_symbol".to_owned(), "rust_helper".to_owned()],
            vec![],
            vec!["rust_helper".to_owned()],
        )
        .unwrap(),
        WorkerOutputConstraintsV1::new(output_identity.byte_len()).unwrap(),
    )
    .unwrap()
}

fn v2_export_contract() -> CompilerFfiContractV1 {
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    let semantic_identity = [0x44; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "rust_helper",
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
        CompilerFfiDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerFfiCodeObjectVersion::V6,
        CompilerFfiSourceOwnerV1::new(
            "fixture",
            "fixture::rust_helper",
            [0x44; 16],
            "_RINvNtCs1234_7fixturerust_helper",
        )
        .unwrap(),
        "rust_helper",
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}

fn request_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").unwrap()
}

#[test]
fn v2_uses_the_same_supervisor_and_retains_sealed_evidence() {
    let pinned = pinned();
    let request = v2_request(&pinned);
    let execution = pinned.execute_v2(&request, limits()).unwrap();
    assert!(execution.response().binds_request(&request));
    assert_eq!(
        execution.evidence_class(),
        WorkerEvidenceClassV2::CompilerFfiLink
    );
    let output = execution.response().output().unwrap();
    assert_eq!(output.bytes(), b"fixture-output");
    assert_eq!(output.request_identity(), request.identity());
    assert_eq!(
        output.compiler_envelope_identity(),
        request.compiler_envelope_identity()
    );
    assert!(!execution.grants_publication_authority());
    assert!(!execution.grants_load_authority());
    assert!(!execution.grants_launch_authority());
}

#[test]
fn returns_only_inert_identity_bound_output() {
    let pinned = pinned();
    let request = request(1, 1, 1024);
    let result = pinned.execute(&request, limits()).unwrap();
    assert!(result.response().binds_request(&request));
    assert_eq!(result.response().worker_build_identity(), WORKER_ID);
    assert_eq!(
        result.response().output().unwrap().bytes(),
        b"fixture-output"
    );
    assert_eq!(
        result.worker_executable(),
        pinned.measurement().executable()
    );
    assert!(!result.grants_publication_authority());
    assert!(!result.grants_load_authority());
    assert!(!result.grants_launch_authority());
}

#[test]
fn pathname_substitution_after_pinning_does_not_change_execution() {
    let pinned = pinned();
    let request = request(1, 1, 1024);
    let copied =
        std::env::temp_dir().join(format!("fe2o3-worker-substitute-{}", std::process::id()));
    fs::copy(fixture_path(), &copied).unwrap();
    let bytes = fs::read(&copied).unwrap();
    let other = PinnedWorkerV1::open(
        &copied,
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, LLVM_ID).unwrap(),
    )
    .unwrap();
    fs::write(&copied, b"replaced").unwrap();
    assert_eq!(
        other
            .execute(&request, limits())
            .unwrap()
            .response()
            .output()
            .unwrap()
            .bytes(),
        b"fixture-output"
    );
    let _ = fs::remove_file(copied);
    assert_eq!(
        pinned
            .execute(&request, limits())
            .unwrap()
            .response()
            .output()
            .unwrap()
            .bytes(),
        b"fixture-output"
    );
}

#[test]
fn hangs_and_blocked_stdin_are_timed_out() {
    let started = Instant::now();
    let error = pinned()
        .execute(&request(2, 2 * 1024 * 1024, 1024), limits())
        .unwrap_err();
    assert_eq!(error.kind(), &WorkerExecutionErrorKind::Timeout);
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn stdout_and_stderr_floods_are_bounded() {
    let stdout = execute(3).unwrap_err();
    assert_eq!(
        stdout.kind(),
        &WorkerExecutionErrorKind::StdoutLimitExceeded
    );
    assert_eq!(stdout.stdout().len(), 4096);

    let stderr = execute(4).unwrap_err();
    assert_eq!(
        stderr.kind(),
        &WorkerExecutionErrorKind::StderrLimitExceeded
    );
    assert_eq!(stderr.stderr().len(), 1024);
}

#[test]
fn malformed_and_short_responses_fail_decoding() {
    for mode in [5, 6] {
        assert!(matches!(
            execute(mode).unwrap_err().kind(),
            WorkerExecutionErrorKind::DecodeResponse(_)
        ));
    }
}

#[test]
fn nonzero_and_early_exits_are_deterministic() {
    assert!(matches!(
        execute(7).unwrap_err().kind(),
        WorkerExecutionErrorKind::ExitFailure(_)
    ));
    let error = pinned()
        .execute(&request(9, 2 * 1024 * 1024, 1024), limits())
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &WorkerExecutionErrorKind::RequestWriteIncomplete
    );
}

#[test]
fn descendants_are_killed_on_timeout() {
    let error = execute(8).unwrap_err();
    assert_eq!(error.kind(), &WorkerExecutionErrorKind::Timeout);
    let text = std::str::from_utf8(error.stderr()).unwrap();
    let pid: u32 = text
        .trim()
        .strip_prefix("descendant=")
        .unwrap()
        .parse()
        .unwrap();
    for _ in 0..100 {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("descendant {pid} survived worker timeout");
}

#[test]
fn request_worker_toolchain_and_output_identities_fail_closed() {
    assert_eq!(
        execute(10).unwrap_err().kind(),
        &WorkerExecutionErrorKind::RequestIdentityMismatch
    );
    assert_eq!(
        execute(11).unwrap_err().kind(),
        &WorkerExecutionErrorKind::WorkerBuildIdentityMismatch
    );
    assert_eq!(
        pinned()
            .execute(&request(12, 1, 1), limits())
            .unwrap_err()
            .kind(),
        &WorkerExecutionErrorKind::OutputLimitExceeded,
    );

    let mut wrong_llvm = request(1, 1, 1024);
    let bytes = wrong_llvm.canonical_bytes().to_vec();
    // Reconstruction is the only supported way to alter a request; this keeps the test canonical.
    wrong_llvm = WorkerRequestV1::new(
        *wrong_llvm.request_id(),
        "other-llvm",
        wrong_llvm.target(),
        wrong_llvm.code_object_version(),
        wrong_llvm.options(),
        wrong_llvm.inputs().to_vec(),
        wrong_llvm.required_symbols().to_vec(),
        wrong_llvm.expected_defined_symbols().to_vec(),
        wrong_llvm.output_constraints().clone(),
    )
    .unwrap();
    assert_ne!(bytes, wrong_llvm.canonical_bytes());
    assert_eq!(
        pinned().execute(&wrong_llvm, limits()).unwrap_err().kind(),
        &WorkerExecutionErrorKind::LlvmBuildIdentityMismatch
    );
}

#[test]
fn valid_failure_response_remains_inert_evidence() {
    let result = execute(13).unwrap();
    assert!(result.response().output().is_none());
    assert!(!result.grants_publication_authority());
}

#[test]
fn child_environment_is_fixed_and_unbound_stderr_is_rejected() {
    assert!(execute(14).is_ok());
    let error = execute(15).unwrap_err();
    assert_eq!(error.kind(), &WorkerExecutionErrorKind::UnexpectedStderr);
    assert_eq!(error.stderr(), b"unbound diagnostic");
}

#[test]
fn output_content_identity_is_redecoded() {
    assert!(matches!(
        execute(16).unwrap_err().kind(),
        WorkerExecutionErrorKind::DecodeResponse(_)
    ));
}

#[test]
fn rejects_symlinks_and_wrong_executable_measurements() {
    use std::os::unix::fs::symlink;
    let link = std::env::temp_dir().join(format!("fe2o3-worker-link-{}", std::process::id()));
    symlink(fixture_path(), &link).unwrap();
    let bytes = fs::read(fixture_path()).unwrap();
    let measurement =
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, LLVM_ID).unwrap();
    assert_eq!(
        PinnedWorkerV1::open(&link, measurement.clone())
            .unwrap_err()
            .kind(),
        &WorkerExecutionErrorKind::OpenWorker
    );
    let wrong = WorkerMeasurementV1::new(
        ContentIdentityV1::from_parts([7; 32], bytes.len() as u64),
        WORKER_ID,
        LLVM_ID,
    )
    .unwrap();
    assert!(matches!(
        PinnedWorkerV1::open(fixture_path(), wrong)
            .unwrap_err()
            .kind(),
        WorkerExecutionErrorKind::WorkerIdentityMismatch { .. }
    ));
    let _ = fs::remove_file(link);
}
