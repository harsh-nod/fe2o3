mod support;

use std::ffi::OsString;

use fe2o3_artifact_transaction::TargetIdentityV1;
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity,
    CodeObjectPayload, CompilerIdentity, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity,
    DigestAlgorithm, DigestBytes, Dimensions, Endianness, IdentityText, KernelEntry,
    LaunchContract, ManifestV1, Mutability, Name, PointerWidth, ScalarType, TargetIdentity,
    ToolIdentity, TypeIdentity,
};
use fe2o3_hsaco_finalize::{
    WorkerInputKindV1, WorkerInputV1, WorkerOptimizationLevelV1, WorkerOptionsV1,
    WorkerOutputConstraintsV1, WorkerOutputV1, WorkerRequestV1, WorkerResponseV1,
    finalize_unfinalized, inspect_unfinalized,
};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DeviceTargetV1, encode_device_descriptor_table_v1,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2, encode_descriptor_v2,
};
use fe2o3_worker_v2_bundle::{
    AlphaZetaSemanticLayoutWitnessesV1, CompilerTransactionCheckpointV1,
    CompilerTransactionRecorderErrorV1, CompilerTransactionRecorderV1, CompilerTransactionStageV1,
    ExactCompilerInvocationV1, ExactCompilerSourceClosureV1, ExactCompilerSourceFileV1,
    ExactCompilerToolV1, ExactSemanticLayoutWitnessV1, Gfx942CompilerTargetV1,
    MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1, SEALED_COMPILER_TRANSACTION_MAGIC_V1,
    SEALED_COMPILER_TRANSACTION_VERSION_V1, SealedCompilerTransactionDecodeErrorV1,
    SealedCompilerTransactionV1,
};
use sha2::{Digest, Sha256};
use support::{HsacoFixture, alpha_zeta_hsaco};

const RUSTC_BYTES: &[u8] = b"exact rustc executable";
const BACKEND_BYTES: &[u8] = b"exact rustc_codegen_fe2o3 executable";
const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/SEALED-COMPILER-TRANSACTION/V1\0";
const HEADER_BYTES: usize = 16;
const MEASUREMENTS_OFFSET: usize = HEADER_BYTES + 32;
const FINAL_CHAIN_OFFSET: usize = MEASUREMENTS_OFFSET + (17 * 32);

struct FinalInputs {
    finalized: Vec<u8>,
    descriptor_source: Vec<u8>,
    finalized_descriptor: Vec<u8>,
    container: Vec<u8>,
}

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn digest(seed: u8) -> DigestBytes {
    DigestBytes::from_bytes([seed; 32])
}

fn source(seed: u8) -> ExactCompilerSourceClosureV1 {
    ExactCompilerSourceClosureV1::new(
        ExactCompilerSourceFileV1::measure(text("src/lib.rs"), &[seed]).unwrap(),
        vec![
            ExactCompilerSourceFileV1::measure(text("src/zeta.rs"), b"zeta").unwrap(),
            ExactCompilerSourceFileV1::measure(text("src/alpha.rs"), b"alpha").unwrap(),
        ],
        vec![text("verify"), text("worker-v2")],
    )
    .unwrap()
}

fn rustc_descriptor(target: &str) -> Option<Vec<u8>> {
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "alpha_zeta".into(),
            "src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .ok()?;
    let environment = CompileEnvironmentV2::from_child_environment([
        (
            OsString::from("CARGO_CFG_TARGET_ARCH"),
            OsString::from("amdgcn"),
        ),
        (
            OsString::from("FE2O3_HSACO_DIR"),
            OsString::from("/workspace/fe2o3/target/fe2o3"),
        ),
        (OsString::from("FE2O3_TARGET"), OsString::from(target)),
        (
            OsString::from("FE2O3_VERIFY_KERNEL_IR"),
            OsString::from("1"),
        ),
    ])
    .ok()?;
    let descriptor = RustcInvocationDescriptorV2::new(
        Sha256::digest(RUSTC_BYTES).into(),
        Sha256::digest(BACKEND_BYTES).into(),
        rustc,
        environment,
    )
    .ok()?;
    encode_descriptor_v2(&descriptor).ok()
}

fn exact_tool(name: &str, bytes: &[u8], seed: u8) -> ExactCompilerToolV1 {
    ExactCompilerToolV1::measure(text(name), text("test"), bytes, &[seed]).unwrap()
}

fn invocation_for(
    target: &str,
) -> Result<ExactCompilerInvocationV1, CompilerTransactionRecorderErrorV1> {
    ExactCompilerInvocationV1::measure(
        &rustc_descriptor(target).expect("target must pass the rustc descriptor codec"),
        exact_tool("rustc", RUSTC_BYTES, 0x11),
        exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x12),
        b"canonical backend invocation",
    )
}

fn witnesses() -> AlphaZetaSemanticLayoutWitnessesV1 {
    AlphaZetaSemanticLayoutWitnessesV1::new(vec![
        ExactSemanticLayoutWitnessV1::measure(text("zeta"), b"zeta-layout").unwrap(),
        ExactSemanticLayoutWitnessV1::measure(text("alpha"), b"alpha-layout").unwrap(),
    ])
    .unwrap()
}

fn recorder_at_ir(
    freshness: u8,
) -> (
    CompilerTransactionRecorderV1,
    CompilerTransactionCheckpointV1,
) {
    let (mut recorder, source_checkpoint) =
        CompilerTransactionRecorderV1::begin([freshness; 32], source(freshness)).unwrap();
    let invocation = invocation_for("gfx942:xnack-").unwrap();
    let target = Gfx942CompilerTargetV1::for_invocation(&invocation).unwrap();
    let compiler = recorder
        .record_compiler(source_checkpoint, invocation)
        .unwrap();
    let target_checkpoint = recorder.record_target(compiler, target).unwrap();
    let semantic = recorder
        .record_semantic_layouts(target_checkpoint, witnesses())
        .unwrap();
    let ir = recorder
        .record_kernel_ir(semantic, b"canonical alpha/zeta Kernel IR")
        .unwrap();
    (recorder, ir)
}

fn worker_exchange(
    raw_hsaco: &[u8],
    request_seed: u8,
    target: &str,
    version: CodeObjectVersion,
) -> (WorkerRequestV1, WorkerResponseV1) {
    worker_exchange_with_symbols(
        raw_hsaco,
        request_seed,
        target,
        version,
        vec!["alpha".into(), "zeta".into()],
    )
}

fn worker_exchange_with_symbols(
    raw_hsaco: &[u8],
    request_seed: u8,
    target: &str,
    version: CodeObjectVersion,
    expected_defined_symbols: Vec<String>,
) -> (WorkerRequestV1, WorkerResponseV1) {
    let request = WorkerRequestV1::new(
        [request_seed; 32],
        "llvm-gfx942-test-build",
        DeviceTargetV1::parse(target).unwrap(),
        version,
        WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true),
        vec![WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, vec![request_seed, 0x42]).unwrap()],
        vec![],
        expected_defined_symbols,
        WorkerOutputConstraintsV1::new(1024 * 1024).unwrap(),
    )
    .unwrap();
    let response = WorkerResponseV1::success(
        &request,
        "worker-gfx942-test-build",
        vec![],
        WorkerOutputV1::new(raw_hsaco.to_vec()).unwrap(),
    )
    .unwrap();
    (request, response)
}

fn manifest_abi() -> AbiLayout {
    AbiLayout::new(
        4,
        4,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                name("value"),
                0,
                4,
                4,
                AbiKind::Scalar(ScalarType::F32),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                TypeIdentity::new(
                    DeclaredRustTypeIdentity::from_untrusted_bytes(digest(0xc1)),
                    DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(0xc2)),
                ),
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn manifest_launch() -> LaunchContract {
    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(u32::MAX, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap()
}

fn artifact_container(
    finalized: &[u8],
    architecture: &str,
    target_capabilities: Vec<Capability>,
) -> Vec<u8> {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, finalized.to_vec()).unwrap();
    let code_object_digest = payload.digest().bytes();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc-codegen-fe2o3"), text("test")),
        ToolIdentity::new(text("rustc-codegen-fe2o3-worker-v2"), text("test")),
        TargetIdentity::new(
            text("amdgcn-amd-amdhsa"),
            text(architecture),
            PointerWidth::Bits64,
            Endianness::Little,
            target_capabilities,
        )
        .unwrap(),
        vec![
            CodeObjectIdentity::new(
                code_object_digest,
                CodeObjectFormat::NativeExecutable,
                finalized.len() as u64,
            )
            .unwrap(),
        ],
        [("alpha", 0xa1_u8), ("zeta", 0xb2_u8)]
            .into_iter()
            .map(|(kernel_name, id)| {
                KernelEntry::new(
                    digest(id),
                    name(kernel_name),
                    name(kernel_name),
                    digest(id.wrapping_add(1)),
                    digest(id.wrapping_add(2)),
                    code_object_digest,
                    vec![Capability::AmdWave],
                    manifest_launch(),
                    manifest_abi(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload])
        .unwrap()
        .to_bytes()
}

fn final_inputs(fixture: &HsacoFixture) -> FinalInputs {
    let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
    let finalized_descriptor =
        encode_device_descriptor_table_v1(finalized.inspection().descriptor_table()).unwrap();
    let finalized = finalized.into_bytes();
    let container = artifact_container(&finalized, "gfx942:xnack-", vec![Capability::AmdWave]);
    FinalInputs {
        finalized,
        descriptor_source: fixture.descriptor_source.clone(),
        finalized_descriptor,
        container,
    }
}

fn seal_transaction(
    freshness: u8,
    canonical_target: TargetIdentityV1,
    fixture: &HsacoFixture,
) -> SealedCompilerTransactionV1 {
    let inputs = final_inputs(fixture);
    let (mut recorder, ir) = recorder_at_ir(freshness);
    let (request, response) = worker_exchange(
        &fixture.bytes,
        freshness,
        "gfx942:xnack-",
        CodeObjectVersion::V6,
    );
    let worker = recorder
        .record_worker_exchange(ir, request.canonical_bytes(), response.canonical_bytes())
        .unwrap();
    let raw = recorder.record_raw_hsaco(worker, &fixture.bytes).unwrap();
    let finalized = recorder
        .record_finalized_artifact(
            raw,
            &inputs.finalized,
            &inputs.descriptor_source,
            &inputs.finalized_descriptor,
            &inputs.container,
            canonical_target,
        )
        .unwrap();
    recorder.seal(finalized).unwrap()
}

fn resign_record(bytes: &mut [u8]) {
    let prefix_len = bytes.len() - 32;
    let mut digest = Sha256::new();
    digest.update(RECORD_IDENTITY_DOMAIN);
    digest.update(&bytes[..prefix_len]);
    bytes[prefix_len..].copy_from_slice(&digest.finalize());
}

#[test]
fn genuine_cov6_alpha_zeta_transaction_round_trips_canonically() {
    let fixture = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let inspection = inspect_unfinalized(&fixture.bytes).unwrap();
    assert_eq!(
        inspection.descriptor_table().code_object_version(),
        CodeObjectVersion::V6
    );
    assert_eq!(
        inspection.descriptor_table().device_target().to_string(),
        "gfx942:xnack-"
    );
    assert_eq!(inspection.descriptor_table().kernels().len(), 2);

    let canonical_target = TargetIdentityV1::from_bytes([0xd1; 32]);
    let sealed = seal_transaction(0x80, canonical_target, &fixture);
    let bytes = sealed.to_bytes();
    assert_eq!(
        SealedCompilerTransactionV1::from_bytes(&bytes).unwrap(),
        sealed
    );
    assert_eq!(&bytes[..8], &SEALED_COMPILER_TRANSACTION_MAGIC_V1);
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        SEALED_COMPILER_TRANSACTION_VERSION_V1
    );
    assert!(bytes.len() <= MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1);
    assert_eq!(sealed.evidence_capsule().target(), canonical_target);
    assert_ne!(
        sealed.measurements().target_profile().as_bytes(),
        canonical_target.as_bytes()
    );

    let other_target = seal_transaction(0x80, TargetIdentityV1::from_bytes([0xd2; 32]), &fixture);
    assert_eq!(
        other_target.measurements().target_profile(),
        sealed.measurements().target_profile()
    );
    assert_ne!(other_target.identity(), sealed.identity());
}

#[test]
fn compiler_target_is_exact_canonical_gfx942_xnack_minus() {
    for wrong in ["gfx942", "gfx942:xnack+", "gfx942:sramecc+:xnack-"] {
        assert!(matches!(
            invocation_for(wrong),
            Err(CompilerTransactionRecorderErrorV1::UnsupportedTarget)
        ));
    }
    assert!(rustc_descriptor("gfx942:unknown+").is_none());
    assert!(rustc_descriptor("gfx942:xnack-:sramecc+").is_none());

    let invocation = invocation_for("gfx942:xnack-").unwrap();
    let target = Gfx942CompilerTargetV1::for_invocation(&invocation).unwrap();
    assert_eq!(target.amd_target(), "gfx942:xnack-");
    assert_eq!(target.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(target.capabilities(), &[Capability::AmdWave]);
}

#[test]
fn cov5_and_wrong_worker_targets_are_rejected_before_recording() {
    let fixture = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let (mut recorder, ir) = recorder_at_ir(0x81);
    let (cov5_request, cov5_response) =
        worker_exchange(&fixture.bytes, 0x41, "gfx942:xnack-", CodeObjectVersion::V5);
    assert!(matches!(
        recorder.record_worker_exchange(
            ir,
            cov5_request.canonical_bytes(),
            cov5_response.canonical_bytes()
        ),
        Err(CompilerTransactionRecorderErrorV1::WorkerCodeObjectVersionMismatch)
    ));

    let (wrong_request, wrong_response) =
        worker_exchange(&fixture.bytes, 0x42, "gfx942:xnack+", CodeObjectVersion::V6);
    assert!(matches!(
        recorder.record_worker_exchange(
            ir,
            wrong_request.canonical_bytes(),
            wrong_response.canonical_bytes()
        ),
        Err(CompilerTransactionRecorderErrorV1::WorkerTargetMismatch)
    ));
}

#[test]
fn mixed_response_and_mixed_worker_payload_are_rejected() {
    let first = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let second = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x41, 0xa5);
    let (request, _) = worker_exchange(&first.bytes, 0x51, "gfx942:xnack-", CodeObjectVersion::V6);
    let (_, mixed_response) =
        worker_exchange(&first.bytes, 0x52, "gfx942:xnack-", CodeObjectVersion::V6);
    let (mut recorder, ir) = recorder_at_ir(0x82);
    assert!(matches!(
        recorder.record_worker_exchange(
            ir,
            request.canonical_bytes(),
            mixed_response.canonical_bytes()
        ),
        Err(CompilerTransactionRecorderErrorV1::WorkerResponseMismatch)
    ));

    let (_, response) = worker_exchange(&first.bytes, 0x51, "gfx942:xnack-", CodeObjectVersion::V6);
    let worker = recorder
        .record_worker_exchange(ir, request.canonical_bytes(), response.canonical_bytes())
        .unwrap();
    assert!(matches!(
        recorder.record_raw_hsaco(worker, &second.bytes),
        Err(CompilerTransactionRecorderErrorV1::WorkerOutputMismatch)
    ));
}

#[test]
fn mixed_worker_symbol_closure_is_rejected() {
    let fixture = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let (request, response) = worker_exchange_with_symbols(
        &fixture.bytes,
        0x58,
        "gfx942:xnack-",
        CodeObjectVersion::V6,
        vec!["unrelated".into()],
    );
    let (mut recorder, ir) = recorder_at_ir(0x88);
    assert!(matches!(
        recorder.record_worker_exchange(ir, request.canonical_bytes(), response.canonical_bytes()),
        Err(CompilerTransactionRecorderErrorV1::WorkerKernelSetMismatch)
    ));
}

#[test]
fn cov5_hsaco_and_noncanonical_worker_bytes_fail_closed() {
    let cov5 = alpha_zeta_hsaco(CodeObjectVersion::V5, "gfx942:xnack-", 0x31, 0xbf);
    let (mut recorder, ir) = recorder_at_ir(0x83);
    let (request, response) =
        worker_exchange(&cov5.bytes, 0x53, "gfx942:xnack-", CodeObjectVersion::V6);
    let worker = recorder
        .record_worker_exchange(ir, request.canonical_bytes(), response.canonical_bytes())
        .unwrap();
    assert!(matches!(
        recorder.record_raw_hsaco(worker, &cov5.bytes),
        Err(CompilerTransactionRecorderErrorV1::InvalidRawHsaco)
            | Err(CompilerTransactionRecorderErrorV1::DescriptorCodeObjectVersionMismatch)
    ));

    let fixture = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let (mut recorder, ir) = recorder_at_ir(0x84);
    let (request, response) =
        worker_exchange(&fixture.bytes, 0x54, "gfx942:xnack-", CodeObjectVersion::V6);
    let mut malformed = request.canonical_bytes().to_vec();
    malformed.push(0);
    assert!(matches!(
        recorder.record_worker_exchange(ir, &malformed, response.canonical_bytes()),
        Err(CompilerTransactionRecorderErrorV1::InvalidWorkerRequest)
    ));
}

#[test]
fn descriptor_final_payload_and_container_substitutions_cannot_seal() {
    let first = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let second = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x61, 0xa5);
    let first_inputs = final_inputs(&first);
    let second_inputs = final_inputs(&second);

    let prepare = || {
        let (mut recorder, ir) = recorder_at_ir(0x85);
        let (request, response) =
            worker_exchange(&first.bytes, 0x55, "gfx942:xnack-", CodeObjectVersion::V6);
        let worker = recorder
            .record_worker_exchange(ir, request.canonical_bytes(), response.canonical_bytes())
            .unwrap();
        let raw = recorder.record_raw_hsaco(worker, &first.bytes).unwrap();
        (recorder, raw)
    };

    let (mut recorder, raw) = prepare();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &first_inputs.finalized,
            &second_inputs.descriptor_source,
            &first_inputs.finalized_descriptor,
            &first_inputs.container,
            TargetIdentityV1::from_bytes([0xd1; 32])
        ),
        Err(CompilerTransactionRecorderErrorV1::DescriptorSourceMismatch)
    ));

    let (mut recorder, raw) = prepare();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &second_inputs.finalized,
            &first_inputs.descriptor_source,
            &second_inputs.finalized_descriptor,
            &second_inputs.container,
            TargetIdentityV1::from_bytes([0xd1; 32])
        ),
        Err(CompilerTransactionRecorderErrorV1::FinalizedHsacoMismatch)
    ));

    let (mut recorder, raw) = prepare();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &first_inputs.finalized,
            &first_inputs.descriptor_source,
            &second_inputs.finalized_descriptor,
            &first_inputs.container,
            TargetIdentityV1::from_bytes([0xd1; 32])
        ),
        Err(CompilerTransactionRecorderErrorV1::FinalizedDescriptorMismatch)
    ));

    let (mut recorder, raw) = prepare();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &first_inputs.finalized,
            &first_inputs.descriptor_source,
            &first_inputs.finalized_descriptor,
            &second_inputs.container,
            TargetIdentityV1::from_bytes([0xd1; 32])
        ),
        Err(CompilerTransactionRecorderErrorV1::ArtifactPayloadMismatch)
    ));
}

#[test]
fn artifact_target_capabilities_and_canonical_target_identity_are_checked() {
    let fixture = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let inputs = final_inputs(&fixture);
    let prepare = || {
        let (mut recorder, ir) = recorder_at_ir(0x86);
        let (request, response) =
            worker_exchange(&fixture.bytes, 0x56, "gfx942:xnack-", CodeObjectVersion::V6);
        let worker = recorder
            .record_worker_exchange(ir, request.canonical_bytes(), response.canonical_bytes())
            .unwrap();
        let raw = recorder.record_raw_hsaco(worker, &fixture.bytes).unwrap();
        (recorder, raw)
    };

    let wrong_target = artifact_container(&inputs.finalized, "gfx942", vec![Capability::AmdWave]);
    let (mut recorder, raw) = prepare();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &inputs.finalized,
            &inputs.descriptor_source,
            &inputs.finalized_descriptor,
            &wrong_target,
            TargetIdentityV1::from_bytes([0xd1; 32])
        ),
        Err(CompilerTransactionRecorderErrorV1::ArtifactTargetMismatch)
    ));

    let wrong_capabilities = artifact_container(
        &inputs.finalized,
        "gfx942:xnack-",
        vec![Capability::AmdWave, Capability::Atomics],
    );
    let (mut recorder, raw) = prepare();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &inputs.finalized,
            &inputs.descriptor_source,
            &inputs.finalized_descriptor,
            &wrong_capabilities,
            TargetIdentityV1::from_bytes([0xd1; 32])
        ),
        Err(CompilerTransactionRecorderErrorV1::ArtifactCapabilityMismatch)
    ));

    let (mut recorder, raw) = prepare();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &inputs.finalized,
            &inputs.descriptor_source,
            &inputs.finalized_descriptor,
            &inputs.container,
            TargetIdentityV1::from_bytes([0; 32])
        ),
        Err(CompilerTransactionRecorderErrorV1::ReservedZeroCanonicalTarget)
    ));
}

#[test]
fn reordered_duplicate_stale_and_mixed_stages_fail_closed() {
    let (mut first, first_source) =
        CompilerTransactionRecorderV1::begin([0x91; 32], source(1)).unwrap();
    let (_, second_source) = CompilerTransactionRecorderV1::begin([0x92; 32], source(1)).unwrap();
    let invocation = invocation_for("gfx942:xnack-").unwrap();
    let early_target = Gfx942CompilerTargetV1::for_invocation(&invocation).unwrap();
    assert!(matches!(
        first.seal(first_source),
        Err(CompilerTransactionRecorderErrorV1::UnexpectedStage {
            expected: CompilerTransactionStageV1::FinalizedArtifact,
            actual: CompilerTransactionStageV1::Source
        })
    ));
    assert!(matches!(
        first.record_target(first_source, early_target),
        Err(CompilerTransactionRecorderErrorV1::UnexpectedStage {
            expected: CompilerTransactionStageV1::Compiler,
            actual: CompilerTransactionStageV1::Source
        })
    ));
    assert!(matches!(
        first.record_compiler(second_source, invocation_for("gfx942:xnack-").unwrap()),
        Err(CompilerTransactionRecorderErrorV1::MixedTransaction)
    ));
    let compiler = first.record_compiler(first_source, invocation).unwrap();
    assert!(matches!(
        first.record_compiler(first_source, invocation_for("gfx942:xnack-").unwrap()),
        Err(CompilerTransactionRecorderErrorV1::UnexpectedStage { .. })
    ));
    assert!(matches!(
        first.record_target(
            first_source,
            Gfx942CompilerTargetV1::for_invocation(&invocation_for("gfx942:xnack-").unwrap())
                .unwrap()
        ),
        Err(CompilerTransactionRecorderErrorV1::StaleCheckpoint)
    ));
    assert_eq!(compiler.stage(), CompilerTransactionStageV1::Compiler);
}

#[test]
fn sealed_mutations_stale_identity_and_inert_boundary_fail_closed() {
    let fixture = alpha_zeta_hsaco(CodeObjectVersion::V6, "gfx942:xnack-", 0x31, 0xbf);
    let sealed = seal_transaction(0x93, TargetIdentityV1::from_bytes([0xd1; 32]), &fixture);
    let bytes = sealed.to_bytes();
    for length in 0..bytes.len() {
        assert!(SealedCompilerTransactionV1::from_bytes(&bytes[..length]).is_err());
    }

    let mut mutation = bytes.clone();
    mutation[MEASUREMENTS_OFFSET] ^= 1;
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&mutation),
        Err(SealedCompilerTransactionDecodeErrorV1::RecordIdentityMismatch)
    ));
    let mut chain = bytes.clone();
    chain[FINAL_CHAIN_OFFSET] ^= 1;
    resign_record(&mut chain);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes(&chain),
        Err(SealedCompilerTransactionDecodeErrorV1::CheckpointMismatch)
    ));

    let stale = seal_transaction(0x94, TargetIdentityV1::from_bytes([0xd1; 32]), &fixture);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes_for_identity(&stale.to_bytes(), sealed.identity()),
        Err(SealedCompilerTransactionDecodeErrorV1::UnexpectedRecordIdentity)
    ));
    assert!(!sealed.authenticates_producer());
    assert!(!sealed.grants_publication_authority());
    assert!(!sealed.grants_load_authority());
    assert!(!sealed.grants_launch_authority());
    assert!(!sealed.evidence_capsule().authenticates_producer());
    assert!(!sealed.evidence_capsule().grants_publication_authority());
    assert!(!sealed.evidence_capsule().grants_load_authority());
    assert!(!sealed.evidence_capsule().grants_launch_authority());
}
