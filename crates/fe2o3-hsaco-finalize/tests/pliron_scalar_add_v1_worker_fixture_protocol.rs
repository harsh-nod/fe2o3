use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffV2,
    CompilerModuleKindV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FirstBuildWorkerV2Error, LinkOptionV1,
    PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY, PinnedWorkerV1, WorkerExecutionErrorKind,
    WorkerExecutionLimitsV1, WorkerMeasurementV1, WorkerOutputConstraintsV1, WorkerProtocolError,
    execute_reproducible_first_build_worker_v2,
};

const WORKER_BUILD_IDENTITY: &str = "fixture-worker-v2-hsaco-v1";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-pliron-scalar-worker-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create fixture transaction directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exact_worker_fixture_binds_source_request_plan_and_output_identities() {
    let first = execute(0x00).expect("exact worker fixture execution");
    let replay = execute(0x00).expect("exact worker fixture replay");

    assert_eq!(first.output_identity(), replay.output_identity());
    assert_eq!(first.output_bytes(), replay.output_bytes());
    assert_eq!(first.link_plan_identity(), replay.link_plan_identity());
    assert_eq!(
        first.authorized_request_identity(),
        replay.authorized_request_identity()
    );
    assert_ne!(first.output_identity().sha256(), &[0; 32]);
    assert!(!first.grants_publication_authority());
    assert!(!first.grants_load_authority());
    assert!(!first.grants_launch_authority());
}

#[test]
fn source_and_assembly_substitution_changes_every_derived_identity() {
    let exact_module = scalar_module(0x00);
    let changed_module = scalar_module(0x11);
    assert_ne!(source_identity(0x00), source_identity(0x11));
    assert_ne!(exact_module, changed_module);
    assert_ne!(
        ContentIdentityV1::calculate(exact_module.as_bytes()),
        ContentIdentityV1::calculate(changed_module.as_bytes())
    );
    assert_ne!(
        compiler_handoff(0x00).identity(),
        compiler_handoff(0x11).identity()
    );

    let exact = execute(0x00).expect("exact worker fixture execution");
    let changed = execute(0x11).expect("machine-substitution worker fixture execution");

    assert_ne!(exact.output_identity(), changed.output_identity());
    assert_ne!(exact.output_bytes(), changed.output_bytes());
    assert_ne!(exact.link_plan_identity(), changed.link_plan_identity());
    assert_ne!(
        exact.authorized_request_identity(),
        changed.authorized_request_identity()
    );
    assert_ne!(
        exact.authorized_request_bytes(),
        changed.authorized_request_bytes()
    );
    assert_ne!(
        exact.authorized().response().canonical_bytes(),
        changed.authorized().response().canonical_bytes()
    );
}

#[test]
fn measured_worker_build_substitution_is_not_self_approval() {
    let error = execute_with_worker_identity(0x00, "fixture-observed-but-unapproved-v1")
        .expect_err("worker build substitution must fail closed");
    assert_candidate_kind(error, WorkerExecutionErrorKind::WorkerBuildIdentityMismatch);
}

#[test]
fn response_state_and_request_bindings_fail_with_exact_protocol_categories() {
    let cases = [
        (
            0x21,
            WorkerExecutionErrorKind::DecodeResponse(WorkerProtocolError::InvalidResponseState),
        ),
        (
            0x23,
            WorkerExecutionErrorKind::DecodeResponse(WorkerProtocolError::RequestIdentityMismatch),
        ),
        (
            0x24,
            WorkerExecutionErrorKind::DecodeResponse(WorkerProtocolError::RequestIdentityMismatch),
        ),
        (
            0x25,
            WorkerExecutionErrorKind::DecodeResponse(WorkerProtocolError::ContentIdentityMismatch),
        ),
    ];

    for (selector, expected) in cases {
        let error = execute(selector).expect_err("substituted response must fail closed");
        assert_candidate_kind(error, expected);
    }
}

#[test]
fn response_worker_identity_substitution_fails_with_exact_executor_category() {
    let error = execute(0x20).expect_err("response worker substitution must fail closed");
    assert_candidate_kind(error, WorkerExecutionErrorKind::WorkerBuildIdentityMismatch);
}

#[test]
fn diagnostics_substitution_is_observed_but_does_not_become_policy() {
    let evidence = execute(0x22).expect("generic executor records bounded diagnostics");
    assert!(
        evidence
            .authorized()
            .response()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic == "post_link.check=substituted status=ok")
    );
    assert!(!evidence.grants_publication_authority());
}

fn execute(
    selector: u8,
) -> Result<fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1, FirstBuildWorkerV2Error> {
    execute_with_worker_identity(selector, WORKER_BUILD_IDENTITY)
}

fn execute_with_worker_identity(
    selector: u8,
    worker_build_identity: &str,
) -> Result<fe2o3_hsaco_finalize::InertFirstBuildWorkerV2EvidenceV1, FirstBuildWorkerV2Error> {
    let directory = TestDirectory::new();
    let worker = pinned_worker(worker_build_identity);
    let handoff = compiler_handoff(selector);
    let producer = ProducerIdentity::from_codegen(
        "pliron_scalar_add_v1_worker_fixture_protocol",
        Some(Path::new(
            "tests/pliron_scalar_add_v1_worker_fixture_protocol.rs",
        )),
    )
    .expect("fixture producer");
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([selector.wrapping_add(1); 32]),
        BuildSession::from_bytes([selector.wrapping_add(2); 16]),
    )
    .expect("begin fixture attempt");
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .expect("publish fixture handoff");
    let consumed = consume_compiler_module_handoff_v1(&directory.0, &producer, attempt)
        .expect("consume fixture handoff");

    execute_reproducible_first_build_worker_v2(
        consumed,
        &worker,
        Vec::new(),
        exact_link_options(),
        WorkerOutputConstraintsV1::new(64 * 1024).expect("bounded fixture output"),
        WorkerExecutionLimitsV1::default(),
    )
}

fn pinned_worker(worker_build_identity: &str) -> PinnedWorkerV1 {
    let path = Path::new(env!("CARGO_BIN_EXE_fe2o3-worker-v2-hsaco-fixture"));
    let executable = fs::read(path).expect("read fixture worker");
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&executable),
        worker_build_identity,
        PLIRON_SCALAR_ADD_V1_LLVM_BUILD_IDENTITY,
    )
    .expect("fixture measurement");
    PinnedWorkerV1::open(path, measurement).expect("pin fixture worker")
}

fn compiler_handoff(selector: u8) -> CompilerModuleHandoffV2 {
    let target = CompilerDeviceTargetV1::parse("gfx942:xnack-").expect("fixed target");
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CompilerCodeObjectVersion::V6)
            .expect("scalar compiler envelope");
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "scalar_add"),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "scalar_add.kd",
        ),
    ])
    .expect("scalar symbol manifest");
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CompilerCodeObjectVersion::V6,
        envelope,
        manifest,
        scalar_module(selector).as_bytes(),
    )
    .expect("scalar compiler handoff")
}

fn scalar_module(selector: u8) -> String {
    let source_identity = source_identity(selector)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        concat!(
            "target triple = \"amdgcn-amd-amdhsa\"\n",
            "define amdgpu_kernel void @scalar_add(ptr addrspace(1) %input, ",
            "ptr addrspace(1) %output, float %addend) {{\n",
            "  ret void\n",
            "}}\n",
            "!fe2o3.handoff.identity = !{{!0}}\n",
            "!0 = !{{!\"sha256:{source_identity}\"}}\n",
        ),
        source_identity = source_identity
    )
}

fn source_identity(selector: u8) -> [u8; 32] {
    let mut identity = [0x41_u8; 32];
    identity[31] = selector;
    identity
}

fn exact_link_options() -> Vec<LinkOptionV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| LinkOptionV1::new(name, value).expect("fixed link option"))
    .collect()
}

fn assert_candidate_kind(error: FirstBuildWorkerV2Error, expected: WorkerExecutionErrorKind) {
    match error {
        FirstBuildWorkerV2Error::CandidateExecution(error) => {
            assert_eq!(error.kind(), &expected);
        }
        other => panic!("unexpected first-build error: {other:?}"),
    }
}
