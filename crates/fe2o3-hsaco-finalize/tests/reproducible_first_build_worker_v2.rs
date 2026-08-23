#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use fe2o3_artifact_transaction::{
    BuildAttempt, BuildInvocation, BuildSession,
    CompilerModuleHandoffErrorV1 as TransactionHandoffErrorV1,
    CompilerModuleHandoffErrorV2 as TransactionHandoffErrorV2, CompilerModuleHandoffIdentityV1,
    CompilerModuleHandoffIdentityV2 as TransactionHandoffIdentityV2,
    ConsumedCompilerModuleHandoffV1, ConsumedCompilerModuleHandoffV2, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, consume_compiler_module_handoff_v2,
    publish_compiler_module_handoff_v1, publish_compiler_module_handoff_v2,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    CompilerModuleHandoffErrorV1, CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2,
    CompilerModuleKindV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    DeviceTargetV1 as CompilerDeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FirstBuildWorkerV2Error, InertDecodedWorkerExchangeV2, LinkOptionV1,
    LinkPlanError, PinnedWorkerV1, ProtectedFirstBuildWorkerV2Error, WorkerEvidenceClassV2,
    WorkerExecutionErrorKind, WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerInputV1,
    WorkerMeasurementV1, WorkerOutputConstraintsV1, WorkerProtocolError,
    WorkerRequestConstructionError, WorkerStageV1,
    execute_protected_reproducible_first_build_worker_v2,
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
        fe2o3_artifact_transaction::enable_same_mount_namespace_artifact_path_guard_v1();
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

fn compiler_closure(seed: u8) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        [seed.wrapping_add(2); 32],
        [seed.wrapping_add(3); 32],
        [seed.wrapping_add(4); 32],
        [seed.wrapping_add(5); 32],
    )
    .unwrap()
}

fn protected_consumed_bytes(
    directory: &TestDirectory,
    closure: CompilerClosureV2,
    bytes: &[u8],
) -> (
    BuildAttempt,
    TransactionHandoffIdentityV2,
    ConsumedCompilerModuleHandoffV2,
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
        publish_compiler_module_handoff_v2(&directory.0, &producer, attempt, closure, bytes)
            .unwrap();
    let consumed =
        consume_compiler_module_handoff_v2(&directory.0, &producer, attempt, closure).unwrap();
    (attempt, receipt.identity(), consumed)
}

#[test]
fn protected_publish_consume_executes_with_exact_inert_closure_binding() {
    let directory = TestDirectory::new();
    let closure = compiler_closure(0x20);
    let (attempt, handoff_identity, consumed) = protected_consumed_bytes(
        &directory,
        closure,
        canonical_handoff(&[]).canonical_bytes(),
    );
    let evidence = execute_protected_reproducible_first_build_worker_v2(
        consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();

    assert_eq!(evidence.attempt(), attempt);
    assert_eq!(evidence.handoff_identity(), handoff_identity);
    assert_eq!(evidence.compiler_closure(), closure);
    assert_eq!(evidence.bootstrap().handoff_identity(), handoff_identity);
    assert_eq!(evidence.exact_replay().handoff_identity(), handoff_identity);
    assert_eq!(evidence.bootstrap().compiler_closure(), closure);
    assert_eq!(evidence.exact_replay().compiler_closure(), closure);
    assert_eq!(evidence.output_bytes(), OUTPUT);
    assert!(!evidence.grants_compiler_authority());
    assert!(!evidence.grants_link_authority());
    assert!(!evidence.grants_publication_authority());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
}

#[test]
fn closure_substitution_is_rejected_and_different_closures_change_all_protected_identities() {
    let rejected_directory = TestDirectory::new();
    let producer = ProducerIdentity::from_codegen(
        "workflow_fixture",
        Some(Path::new("src/workflow_fixture.rs")),
    )
    .unwrap();
    let attempt = begin_build_attempt(
        &rejected_directory.0,
        &producer,
        BuildInvocation::from_bytes([0x61; 32]),
        BuildSession::from_bytes([0x62; 16]),
    )
    .unwrap();
    let expected = compiler_closure(0x30);
    let substituted = compiler_closure(0x40);
    publish_compiler_module_handoff_v2(
        &rejected_directory.0,
        &producer,
        attempt,
        expected,
        canonical_handoff(&[]).canonical_bytes(),
    )
    .unwrap();
    assert!(matches!(
        consume_compiler_module_handoff_v2(&rejected_directory.0, &producer, attempt, substituted,),
        Err(TransactionHandoffErrorV2::WrongCompilerClosure)
    ));

    let first_directory = TestDirectory::new();
    let second_directory = TestDirectory::new();
    let (_, first_handoff, first_consumed) = protected_consumed_bytes(
        &first_directory,
        expected,
        canonical_handoff(&[]).canonical_bytes(),
    );
    let (_, second_handoff, second_consumed) = protected_consumed_bytes(
        &second_directory,
        substituted,
        canonical_handoff(&[]).canonical_bytes(),
    );
    let worker = pinned();
    let first = execute_protected_reproducible_first_build_worker_v2(
        first_consumed,
        &worker,
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();
    let second = execute_protected_reproducible_first_build_worker_v2(
        second_consumed,
        &worker,
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();

    assert_ne!(first_handoff, second_handoff);
    assert_ne!(first.identity(), second.identity());
    assert_ne!(
        first.bootstrap().response().request_id(),
        second.bootstrap().response().request_id()
    );
    assert_ne!(
        first.exact_replay().response().request_id(),
        second.exact_replay().response().request_id()
    );
    assert_eq!(first.plan().identity(), second.plan().identity());
    assert_eq!(first.output_bytes(), second.output_bytes());
}

#[test]
fn transaction_schema_cross_use_and_compiler_handoff_downgrade_are_rejected() {
    let closure = compiler_closure(0x50);
    let producer = ProducerIdentity::from_codegen(
        "workflow_fixture",
        Some(Path::new("src/workflow_fixture.rs")),
    )
    .unwrap();

    let v1_directory = TestDirectory::new();
    let v1_attempt = begin_build_attempt(
        &v1_directory.0,
        &producer,
        BuildInvocation::from_bytes([0x71; 32]),
        BuildSession::from_bytes([0x72; 16]),
    )
    .unwrap();
    publish_compiler_module_handoff_v1(
        &v1_directory.0,
        &producer,
        v1_attempt,
        canonical_handoff(&[]).canonical_bytes(),
    )
    .unwrap();
    assert!(matches!(
        consume_compiler_module_handoff_v2(&v1_directory.0, &producer, v1_attempt, closure,),
        Err(TransactionHandoffErrorV2::NotPublished)
    ));

    let v2_directory = TestDirectory::new();
    let v2_attempt = begin_build_attempt(
        &v2_directory.0,
        &producer,
        BuildInvocation::from_bytes([0x73; 32]),
        BuildSession::from_bytes([0x74; 16]),
    )
    .unwrap();
    publish_compiler_module_handoff_v2(
        &v2_directory.0,
        &producer,
        v2_attempt,
        closure,
        canonical_handoff(&[]).canonical_bytes(),
    )
    .unwrap();
    assert!(matches!(
        consume_compiler_module_handoff_v1(&v2_directory.0, &producer, v2_attempt),
        Err(TransactionHandoffErrorV1::NotPublished)
    ));

    let downgrade_directory = TestDirectory::new();
    let mut downgraded = canonical_handoff(&[]).canonical_bytes().to_vec();
    let version = downgraded
        .windows(3)
        .position(|window| window == b"/V2")
        .unwrap();
    downgraded[version + 2] = b'1';
    let (_, _, consumed) = protected_consumed_bytes(&downgrade_directory, closure, &downgraded);
    assert!(matches!(
        execute_protected_reproducible_first_build_worker_v2(
            consumed,
            &pinned(),
            vec![provider()],
            options(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        ),
        Err(ProtectedFirstBuildWorkerV2Error::CompilerModuleHandoff(
            CompilerModuleHandoffErrorV2::InvalidMagic
        ))
    ));
}

#[test]
fn protected_decode_rejects_target_cov_manifest_and_module_substitution() {
    const DOMAIN: &[u8] = b"FE2O3/COMPILER-MODULE-HANDOFF/V2\0";
    const TARGET_TEXT: &[u8] = b"gfx942:xnack-";

    let canonical = canonical_handoff(&[]).canonical_bytes().to_vec();
    let target_offset = DOMAIN.len() + 4;
    let cov_offset = target_offset + TARGET_TEXT.len();
    let manifest_digest_offset = cov_offset + 1 + 1 + 32 + 8 + 4;
    let mut cases = Vec::new();

    let mut target = canonical.clone();
    target[target_offset..target_offset + 6].copy_from_slice(b"gfx90a");
    cases.push(("target", target));

    let mut cov = canonical.clone();
    cov[cov_offset] = 5;
    cases.push(("code object version", cov));

    let mut manifest = canonical.clone();
    manifest[manifest_digest_offset] ^= 1;
    cases.push(("manifest", manifest));

    let mut module = canonical;
    *module.last_mut().unwrap() ^= 1;
    cases.push(("module", module));

    for (index, (field, bytes)) in cases.into_iter().enumerate() {
        let directory = TestDirectory::new();
        let closure = compiler_closure(0x60 + index as u8);
        let (_, _, consumed) = protected_consumed_bytes(&directory, closure, &bytes);
        let error = execute_protected_reproducible_first_build_worker_v2(
            consumed,
            &pinned(),
            vec![provider()],
            options(),
            WorkerOutputConstraintsV1::new(4096).unwrap(),
            limits(),
        )
        .unwrap_err();
        let ProtectedFirstBuildWorkerV2Error::CompilerModuleHandoff(error) = error else {
            panic!("unexpected {field} substitution result: {error:?}");
        };
        let expected = match field {
            "target" => matches!(
                error,
                CompilerModuleHandoffErrorV2::Handoff(CompilerModuleHandoffErrorV1::TargetMismatch)
            ),
            "code object version" => matches!(
                error,
                CompilerModuleHandoffErrorV2::Handoff(
                    CompilerModuleHandoffErrorV1::CodeObjectVersionMismatch
                )
            ),
            "manifest" => matches!(
                error,
                CompilerModuleHandoffErrorV2::ManifestIdentityMismatch
            ),
            "module" => matches!(
                error,
                CompilerModuleHandoffErrorV2::Handoff(
                    CompilerModuleHandoffErrorV1::ModuleIdentityMismatch
                )
            ),
            _ => false,
        };
        assert!(expected, "unexpected {field} substitution error: {error:?}");
    }
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
        WorkerEvidenceClassV2::CompilerFfiLink
    );
    assert_eq!(
        evidence.authorized().evidence_class(),
        WorkerEvidenceClassV2::CompilerFfiLink
    );
    assert!(
        evidence
            .bootstrap()
            .response()
            .response_identity()
            .is_none()
    );
    assert!(
        evidence
            .bootstrap()
            .response()
            .device_library_provider()
            .is_none()
    );
    assert!(
        evidence
            .exact_replay()
            .response()
            .response_identity()
            .is_none()
    );
    assert!(
        evidence
            .exact_replay()
            .response()
            .device_library_provider()
            .is_none()
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

    let authorized_request_identity = *evidence.authorized_request_identity();
    let authorized = evidence.into_authorized_execution();
    assert_eq!(
        authorized.response().request_identity(),
        &authorized_request_identity
    );
    assert_eq!(authorized.response().output().unwrap().bytes(), OUTPUT);
    assert!(!authorized.grants_publication_authority());
    assert!(!authorized.grants_load_authority());
    assert!(!authorized.grants_launch_authority());
}

#[test]
fn compiler_aware_v2_bootstrap_precedes_the_exact_v2_replay() {
    let directory = TestDirectory::new();
    let (_, _, consumed) = consumed_handoff_with_extra(&directory, &["workflow_phase_trace"]);
    let evidence = execute_reproducible_first_build_worker_v2(
        consumed,
        &pinned(),
        vec![provider()],
        options(),
        WorkerOutputConstraintsV1::new(4096).unwrap(),
        limits(),
    )
    .unwrap();

    assert!(evidence.bootstrap_request_bytes().starts_with(b"F3LREQ02"));
    assert!(evidence.authorized_request_bytes().starts_with(b"F3LREQ02"));
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        evidence.bootstrap_request_bytes(),
        evidence.bootstrap().response().canonical_bytes(),
    )
    .unwrap();
    let replay = InertDecodedWorkerExchangeV2::decode(
        evidence.authorized_request_bytes(),
        evidence.exact_replay().response().canonical_bytes(),
    )
    .unwrap();

    assert_eq!(bootstrap.request().output_constraints().max_bytes(), 4096);
    assert_eq!(
        replay.request().output_constraints().max_bytes(),
        OUTPUT.len() as u64
    );
    assert_eq!(
        bootstrap.response().diagnostics(),
        ["fixture.phase=v2-bootstrap"]
    );
    assert_eq!(
        replay.response().diagnostics(),
        ["fixture.phase=v2-exact-replay"]
    );
    assert_eq!(
        bootstrap.request().compiler_module(),
        replay.request().compiler_module()
    );
    assert_eq!(bootstrap.request().target(), replay.request().target());
    assert_eq!(
        bootstrap.request().code_object_version(),
        replay.request().code_object_version()
    );
    assert_eq!(bootstrap.request().options(), replay.request().options());
    assert_eq!(
        bootstrap.request().compiler_envelope_identity(),
        replay.request().compiler_envelope_identity()
    );
    assert_eq!(
        bootstrap.request().worker_executable(),
        replay.request().worker_executable()
    );
    assert_eq!(
        bootstrap.request().worker_build_identity(),
        replay.request().worker_build_identity()
    );
    assert_eq!(
        bootstrap.request().llvm_build_identity(),
        replay.request().llvm_build_identity()
    );
    assert_eq!(
        bootstrap.request().external_providers(),
        replay.request().external_providers()
    );
    assert_eq!(
        bootstrap.request().import_symbols(),
        replay.request().import_symbols()
    );
    assert_eq!(
        bootstrap.request().export_symbols(),
        replay.request().export_symbols()
    );
    assert_eq!(
        bootstrap.request().final_symbols(),
        replay.request().final_symbols()
    );
    assert_ne!(
        bootstrap.request().request_id(),
        replay.request().request_id()
    );
    assert_eq!(
        bootstrap.response().output().unwrap().bytes(),
        replay.response().output().unwrap().bytes()
    );
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
            if error.kind()
                == &WorkerExecutionErrorKind::DecodeResponse(
                    WorkerProtocolError::RequestIdentityMismatch
                )
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
    let FirstBuildWorkerV2Error::AuthorizedExecution(error) = v2_error else {
        panic!("unexpected error: {v2_error:?}");
    };
    assert!(matches!(
        error.kind(),
        WorkerExecutionErrorKind::DecodeResponse(WorkerProtocolError::RequestIdentityMismatch)
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
            if execution.kind()
                == &WorkerExecutionErrorKind::DecodeResponse(
                    WorkerProtocolError::InvalidOutputBound
                )
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
