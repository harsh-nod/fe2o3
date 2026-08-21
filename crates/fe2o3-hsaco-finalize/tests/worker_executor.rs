#![cfg(target_os = "linux")]

use std::{
    fs::{self, File},
    io::Write,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, consume_compiler_module_handoff_v2,
    publish_compiler_module_handoff_v1, publish_compiler_module_handoff_v2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, InertWorkerExecutionV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1,
    LinkOutputV1, MultiInputLinkPlanV1, PinnedWorkerV1, ProvenanceNodeV1, WorkerExecutionErrorKind,
    WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1, WorkerRequestV1,
    construct_protected_worker_request_v2_from_consumed_handoff,
    construct_worker_request_v2_from_consumed_handoff,
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
    request_with_id([mode; 32], vec![mode; input_bytes], output_bytes)
}

fn request_with_id(request_id: [u8; 32], input: Vec<u8>, output_bytes: u64) -> WorkerRequestV1 {
    WorkerRequestV1::new(
        request_id,
        LLVM_ID,
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CodeObjectVersion::V6,
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
        vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, input).unwrap()],
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

fn compiler_export(symbol: &str) -> CompilerFfiContractV1 {
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    let semantic_identity = [0x53; 32];
    let semantic_text = semantic_identity
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol,
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
            "fixture_crate",
            "fixture_crate::kernel_main",
            [0x35; 16],
            "_RINvNtCs1234_13fixture_crate11kernel_main",
        )
        .unwrap(),
        symbol,
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}

fn handoff_plan(module: &WorkerInputV1) -> (MultiInputLinkPlanV1, LinkInputKindClosureV1) {
    let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let input = LinkInputV1::new(module.identity(), target);
    let output_identity = ContentIdentityV1::calculate(b"fixture-output");
    let plan = MultiInputLinkPlanV1::canonicalized(
        target,
        vec![input],
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
        LinkOutputV1::new(output_identity, target),
        vec![
            ProvenanceNodeV1::new(input.identity(), vec![]).unwrap(),
            ProvenanceNodeV1::new(output_identity, vec![input.identity()]).unwrap(),
        ],
    )
    .unwrap();
    let kinds = LinkInputKindClosureV1::new(&plan, vec![module.kind()]).unwrap();
    (plan, kinds)
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
fn consumed_compiler_handoff_executes_v2_without_gaining_authority() {
    let directory =
        std::env::temp_dir().join(format!("fe2o3-worker-v2-handoff-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir(&directory).unwrap();
    let producer =
        ProducerIdentity::from_codegen("fixture_crate", Some(Path::new("src/lib.rs"))).unwrap();
    let attempt = begin_build_attempt(
        &directory,
        &producer,
        BuildInvocation::from_bytes([0x61; 32]),
        BuildSession::from_bytes([0x62; 16]),
    )
    .unwrap();
    let mut envelope = CompilerFfiEnvelopeBuilderV1::new(
        CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerCodeObjectVersion::V6,
        1,
    )
    .unwrap();
    envelope.push(compiler_export("kernel_export")).unwrap();
    let module_bytes = b"define amdgpu_kernel void @kernel_main() { ret void }\n\
define i32 @kernel_export(i32 %value) { ret i32 %value }\n";
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "kernel_main"),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "kernel_main.kd",
        ),
        (CompilerModuleSymbolRoleV1::DeviceFfiExport, "kernel_export"),
    ])
    .unwrap();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerCodeObjectVersion::V6,
        envelope.finish().unwrap(),
        manifest,
        module_bytes,
    )
    .unwrap();
    let receipt = publish_compiler_module_handoff_v1(
        &directory,
        &producer,
        attempt,
        handoff.canonical_bytes(),
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_v1(&directory, &producer, attempt).unwrap();
    let module = WorkerInputV1::new(WorkerInputKindV1::LlvmTextIr, module_bytes.to_vec()).unwrap();
    let (plan, kinds) = handoff_plan(&module);
    let pinned = pinned();
    let request = construct_worker_request_v2_from_consumed_handoff(
        &plan,
        pinned.measurement(),
        consumed,
        vec![],
        &kinds,
        WorkerOutputConstraintsV1::new(b"fixture-output".len() as u64).unwrap(),
    )
    .unwrap();
    let execution = pinned
        .execute_compiler_handoff_v2(&request, limits())
        .unwrap();

    assert_eq!(execution.attempt(), attempt);
    assert_eq!(execution.handoff_identity(), receipt.identity());
    assert_eq!(
        execution.response().output().unwrap().bytes(),
        b"fixture-output"
    );
    assert!(execution.response().binds_request(request.sealed_request()));
    assert!(!execution.grants_publication_authority());
    assert!(!execution.grants_load_authority());
    assert!(!execution.grants_launch_authority());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn protected_consumed_handoff_executes_without_losing_v2_identity_or_closure() {
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-protected-worker-v2-handoff-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir(&directory).unwrap();
    let producer =
        ProducerIdentity::from_codegen("fixture_crate", Some(Path::new("src/lib.rs"))).unwrap();
    let attempt = begin_build_attempt(
        &directory,
        &producer,
        BuildInvocation::from_bytes([0x71; 32]),
        BuildSession::from_bytes([0x72; 16]),
    )
    .unwrap();
    let closure =
        CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap();
    let mut envelope = CompilerFfiEnvelopeBuilderV1::new(
        CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerCodeObjectVersion::V6,
        1,
    )
    .unwrap();
    envelope.push(compiler_export("kernel_export")).unwrap();
    let module_bytes = b"define amdgpu_kernel void @kernel_main() { ret void }\n\
define i32 @kernel_export(i32 %value) { ret i32 %value }\n";
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "kernel_main"),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "kernel_main.kd",
        ),
        (CompilerModuleSymbolRoleV1::DeviceFfiExport, "kernel_export"),
    ])
    .unwrap();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        CompilerDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerCodeObjectVersion::V6,
        envelope.finish().unwrap(),
        manifest,
        module_bytes,
    )
    .unwrap();
    let receipt = publish_compiler_module_handoff_v2(
        &directory,
        &producer,
        attempt,
        closure,
        handoff.canonical_bytes(),
    )
    .unwrap();
    let consumed =
        consume_compiler_module_handoff_v2(&directory, &producer, attempt, closure).unwrap();
    let module = WorkerInputV1::new(WorkerInputKindV1::LlvmTextIr, module_bytes.to_vec()).unwrap();
    let (plan, kinds) = handoff_plan(&module);
    let pinned = pinned();
    let request = construct_protected_worker_request_v2_from_consumed_handoff(
        &plan,
        pinned.measurement(),
        consumed,
        vec![],
        &kinds,
        WorkerOutputConstraintsV1::new(b"fixture-output".len() as u64).unwrap(),
    )
    .unwrap();
    assert_eq!(request.handoff_identity(), receipt.identity());
    assert_eq!(request.compiler_closure(), closure);
    assert!(!request.grants_compiler_authority());
    assert!(!request.grants_link_authority());

    let execution = pinned
        .execute_protected_compiler_handoff_v2(&request, limits())
        .unwrap();
    assert_eq!(execution.handoff_identity(), receipt.identity());
    assert_eq!(execution.compiler_closure(), closure);
    assert_eq!(
        execution.response().output().unwrap().bytes(),
        b"fixture-output"
    );
    assert!(execution.response().binds_request(request.sealed_request()));
    assert!(!execution.grants_compiler_authority());
    assert!(!execution.grants_link_authority());
    assert!(!execution.grants_publication_authority());
    assert!(!execution.grants_load_authority());
    assert!(!execution.grants_launch_authority());
    fs::remove_dir_all(directory).unwrap();
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
    assert!(format!("{error:?}").contains("request_written="));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn workflow_request_id_prefixes_do_not_alias_legacy_control_modes() {
    for prefix in [2, 9] {
        let mut request_id = [0x5a; 32];
        request_id[0] = prefix;
        let request = request_with_id(request_id, b"workflow_kernel".to_vec(), 1024);
        assert_eq!(
            pinned()
                .execute(&request, limits())
                .unwrap()
                .response()
                .output()
                .unwrap()
                .bytes(),
            b"fixture-output"
        );
    }
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

fn retained_worker_image(sealed: bool) -> (File, PathBuf, WorkerMeasurementV1) {
    use rustix::fs::{MemfdFlags, SealFlags};

    let bytes = fs::read(fixture_path()).unwrap();
    let fd =
        rustix::fs::memfd_create("fe2o3-retained-worker-test", MemfdFlags::ALLOW_SEALING).unwrap();
    let mut image = File::from(fd);
    image.write_all(&bytes).unwrap();
    image
        .set_permissions(fs::Permissions::from_mode(0o555))
        .unwrap();
    if sealed {
        rustix::fs::fcntl_add_seals(
            &image,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
        )
        .and_then(|()| rustix::fs::fcntl_add_seals(&image, SealFlags::SEAL))
        .unwrap();
    }
    let path = PathBuf::from(format!("/proc/self/fd/{}", image.as_raw_fd()));
    let measurement =
        WorkerMeasurementV1::new(ContentIdentityV1::calculate(&bytes), WORKER_ID, LLVM_ID).unwrap();
    (image, path, measurement)
}

#[test]
fn accepts_only_fully_sealed_inherited_worker_descriptors() {
    let (_image, path, measurement) = retained_worker_image(true);
    assert!(PinnedWorkerV1::open(&path, measurement).is_ok());

    let (_image, path, measurement) = retained_worker_image(false);
    assert_eq!(
        PinnedWorkerV1::open(&path, measurement).unwrap_err().kind(),
        &WorkerExecutionErrorKind::PreparePinnedImage
    );
}
