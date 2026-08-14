mod support;

use std::{
    ffi::OsString,
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
    consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity,
    CodeObjectPayload, CompilerIdentity, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity,
    DigestAlgorithm, DigestBytes, Dimensions, Endianness, IdentityText, KernelEntry,
    LaunchContract, ManifestV1, Mutability, Name, PointerWidth, ScalarType, TargetIdentity,
    ToolIdentity, TypeIdentity, derive_generated_host_contract_identity_v1,
    derive_manifest_claim_target_identity_v1,
};
use fe2o3_hsaco_finalize::{
    CompilerFfiCodeObjectVersion, CompilerFfiContractV1, CompilerFfiDeviceTargetV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiEnvelopeV1, CompilerFfiLinkRoleV1,
    CompilerFfiSourceOwnerV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, ContentIdentityV1,
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1, MultiInputLinkPlanV1,
    ProvenanceNodeV1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3, WORKER_RESPONSE_MAGIC_V2,
    WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1, WorkerOutputConstraintsV1,
    WorkerRequestV2, construct_worker_request_v2_from_consumed_handoff,
    derive_device_ffi_contract_id_v1, finalize_unfinalized, inspect_unfinalized,
};
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CodeObjectVersion, DeviceDescriptorTableV1,
    KernelDescriptorV1, OwnershipSemantics, PhysicalAbiComponentKind, ScalarTypeV1,
    encode_device_descriptor_table_v1,
};
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2, encode_descriptor_v2,
};
use fe2o3_worker_v2_bundle::{
    AlphaZetaSemanticLayoutWitnessesV1, CompilerTransactionCheckpointV1,
    CompilerTransactionRecorderErrorV1, CompilerTransactionRecorderV1, CompilerTransactionStageV1,
    ExactCompilerInvocationV1, ExactCompilerSourceClosureV1, ExactCompilerSourceFileV1,
    ExactCompilerToolV1, ExactSemanticLayoutWitnessV1, ExactWorkerToolV1, Gfx942CompilerTargetV1,
    MAX_SEALED_COMPILER_TRANSACTION_BYTES_V1, SEALED_COMPILER_TRANSACTION_MAGIC_V1,
    SEALED_COMPILER_TRANSACTION_VERSION_V1, ScalarGemmV1SemanticLayoutWitnessV1,
    SealedCompilerTransactionDecodeErrorV1, SealedCompilerTransactionV1,
};
use sha2::{Digest, Sha256};
use support::{HsacoFixture, alpha_zeta_hsaco};

const RUSTC_BYTES: &[u8] = b"exact rustc executable";
const BACKEND_BYTES: &[u8] = b"exact rustc_codegen_fe2o3 executable";
const WORKER_BYTES: &[u8] = b"exact fe2o3 LLVM API worker executable";
const WORKER_BUILD: &str = "worker-gfx942-test-build";
const LLVM_BUILD: &str = "llvm-gfx942-test-build";
const PRODUCER_NAME: &str = "rustc-codegen-fe2o3-worker-v2";
const PRODUCER_VERSION: &str = "typed-general-gfx942-cov6-v1";
const MODULE: &[u8] = b"target triple = \"amdgcn-amd-amdhsa\"\ndefine amdgpu_kernel void @alpha() { ret void }\ndefine i32 @compiler_helper(i32 %value) { ret i32 %value }\ndefine amdgpu_kernel void @zeta() { ret void }\n";
const SCALAR_MODULE: &[u8] = b"target triple = \"amdgcn-amd-amdhsa\"\ndefine amdgpu_kernel void @scalar_gemm_v1() { ret void }\n";
const RECORD_IDENTITY_DOMAIN: &[u8] = b"FE2O3/SEALED-COMPILER-TRANSACTION/V1\0";
const MEASUREMENTS_OFFSET: usize = 16 + 32;
const FINAL_CHAIN_OFFSET: usize = MEASUREMENTS_OFFSET + (17 * 32);

#[derive(Clone, Copy)]
enum ManifestMutation {
    None,
    Target,
    Capabilities,
    Compiler,
    Producer,
    Evidence,
    Abi,
    Launch,
}

struct FinalInputs {
    finalized: Vec<u8>,
    descriptor_source: Vec<u8>,
    finalized_descriptor: Vec<u8>,
    container: Vec<u8>,
}

struct TransactionFixture {
    hsaco: HsacoFixture,
    handoff: CompilerModuleHandoffV2,
    semantic: AlphaZetaSemanticLayoutWitnessesV1,
    final_inputs: FinalInputs,
}

struct TestDirectory(std::path::PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-transaction-{}-{}",
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

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
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

fn exact_worker() -> ExactWorkerToolV1 {
    ExactWorkerToolV1::measure(
        WORKER_BYTES,
        text(WORKER_BUILD),
        text(LLVM_BUILD),
        text(PRODUCER_NAME),
        text(PRODUCER_VERSION),
    )
    .unwrap()
}

fn invocation_for(
    target: &str,
) -> Result<ExactCompilerInvocationV1, CompilerTransactionRecorderErrorV1> {
    ExactCompilerInvocationV1::measure(
        &rustc_descriptor(target).expect("target must pass the rustc descriptor codec"),
        exact_tool("rustc", RUSTC_BYTES, 0x11),
        exact_tool("rustc-codegen-fe2o3", BACKEND_BYTES, 0x12),
        exact_worker(),
        b"canonical backend invocation",
    )
}

fn compiler_handoff(
    module: &[u8],
    target: &str,
    version: CompilerFfiCodeObjectVersion,
) -> CompilerModuleHandoffV2 {
    let target = CompilerFfiDeviceTargetV1::parse(target).unwrap();
    let mut envelope = CompilerFfiEnvelopeBuilderV1::new(target, version, 1).unwrap();
    envelope
        .push(compiler_helper_contract(target, version))
        .unwrap();
    let envelope = envelope.finish().unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "alpha"),
        (CompilerModuleSymbolRoleV1::KernelEntry, "zeta"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "alpha.kd"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "zeta.kd"),
        (
            CompilerModuleSymbolRoleV1::DeviceFfiExport,
            "compiler_helper",
        ),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        version,
        envelope,
        manifest,
        module,
    )
    .unwrap()
}

fn scalar_compiler_handoff() -> CompilerModuleHandoffV2 {
    let target = CompilerFfiDeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
        target,
        CompilerFfiCodeObjectVersion::V6,
    )
    .unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "scalar_gemm_v1"),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            "scalar_gemm_v1.kd",
        ),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CompilerFfiCodeObjectVersion::V6,
        envelope,
        manifest,
        SCALAR_MODULE,
    )
    .unwrap()
}

fn compiler_helper_contract(
    target: CompilerFfiDeviceTargetV1,
    version: CompilerFfiCodeObjectVersion,
) -> CompilerFfiContractV1 {
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    let semantic = [0x53; 32];
    let semantic_text = semantic
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "compiler_helper",
        calling_convention: "C",
        code_object_version: match version {
            CompilerFfiCodeObjectVersion::V4 => 4,
            CompilerFfiCodeObjectVersion::V5 => 5,
            CompilerFfiCodeObjectVersion::V6 => 6,
        },
        target: &target.to_string(),
        physical_abi: ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    };
    CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Export,
        CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        target,
        version,
        CompilerFfiSourceOwnerV1::new(
            "alpha_zeta",
            "alpha_zeta::compiler_helper",
            [0x35; 16],
            "_RINvNtCs1234_10alpha_zeta15compiler_helper",
        )
        .unwrap(),
        "compiler_helper",
        ABI,
        "none",
        semantic,
    )
    .unwrap()
}

fn worker_measurement(
    executable: &[u8],
    worker_build: &str,
    llvm_build: &str,
) -> WorkerMeasurementV1 {
    WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(executable),
        worker_build,
        llvm_build,
    )
    .unwrap()
}

fn worker_exchange(
    handoff: &CompilerModuleHandoffV2,
    raw_hsaco: &[u8],
    seed: u8,
    measurement: WorkerMeasurementV1,
    response_worker_build: &str,
) -> (Vec<u8>, Vec<u8>) {
    let directory = TestDirectory::new();
    let producer =
        ProducerIdentity::from_codegen("alpha_zeta", Some(Path::new("src/lib.rs"))).unwrap();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([seed; 32]),
        BuildSession::from_bytes([seed.wrapping_add(1); 16]),
    )
    .unwrap();
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, handoff.canonical_bytes())
        .unwrap();
    let consumed = consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
    let module = WorkerInputV1::new(
        WorkerInputKindV1::LlvmTextIr,
        handoff.module_bytes().to_vec(),
    )
    .unwrap();
    let target = fe2o3_kernel_descriptor::DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let input = LinkInputV1::new(module.identity(), target);
    let output_identity = ContentIdentityV1::calculate(raw_hsaco);
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
    let kinds = LinkInputKindClosureV1::new(&plan, vec![WorkerInputKindV1::LlvmTextIr]).unwrap();
    let request = construct_worker_request_v2_from_consumed_handoff(
        &plan,
        &measurement,
        consumed,
        vec![],
        &kinds,
        WorkerOutputConstraintsV1::new(raw_hsaco.len() as u64).unwrap(),
    )
    .unwrap();
    let request = request.sealed_request();
    (
        request.canonical_bytes().to_vec(),
        encode_worker_response(request, raw_hsaco, response_worker_build),
    )
}

fn push_field(bytes: &mut Vec<u8>, tag: u16, value: &[u8]) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}

fn encode_worker_response(request: &WorkerRequestV2, output: &[u8], worker_build: &str) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(WORKER_RESPONSE_MAGIC_V2);
    push_field(&mut encoded, 1, request.request_id());
    push_field(&mut encoded, 2, request.identity());
    let envelope = request.compiler_envelope_identity().as_bytes();
    push_field(&mut encoded, 3, &envelope);
    push_field(&mut encoded, 4, worker_build.as_bytes());
    push_field(&mut encoded, 5, &[9]);
    push_field(&mut encoded, 6, &0_u32.to_le_bytes());
    let mut output_field = Vec::with_capacity(41 + output.len());
    output_field.push(1);
    output_field.extend_from_slice(&Sha256::digest(output));
    output_field.extend_from_slice(&(output.len() as u64).to_le_bytes());
    output_field.extend_from_slice(output);
    push_field(&mut encoded, 7, &output_field);
    encoded
}

fn manifest_scalar(value: ScalarTypeV1) -> ScalarType {
    match value {
        ScalarTypeV1::I8 => ScalarType::I8,
        ScalarTypeV1::U8 => ScalarType::U8,
        ScalarTypeV1::I16 => ScalarType::I16,
        ScalarTypeV1::U16 => ScalarType::U16,
        ScalarTypeV1::I32 => ScalarType::I32,
        ScalarTypeV1::U32 => ScalarType::U32,
        ScalarTypeV1::I64 => ScalarType::I64,
        ScalarTypeV1::U64 => ScalarType::U64,
        ScalarTypeV1::F16 => ScalarType::F16,
        ScalarTypeV1::F32 => ScalarType::F32,
        ScalarTypeV1::F64 => ScalarType::F64,
    }
}

fn manifest_access(value: AccessMode) -> Access {
    match value {
        AccessMode::ByValue => Access::ByValue,
        AccessMode::ReadOnly => Access::ReadOnly,
        AccessMode::WriteOnly => Access::WriteOnly,
        AccessMode::ReadWrite => Access::ReadWrite,
    }
}

fn manifest_abi(
    table: &DeviceDescriptorTableV1,
    kernel: &KernelDescriptorV1,
    mutation: ManifestMutation,
) -> AbiLayout {
    let mut fields = Vec::new();
    for (index, argument) in kernel.arguments().iter().enumerate() {
        let source = table
            .type_records()
            .iter()
            .find(|record| record.identity() == argument.source_type())
            .unwrap();
        let scalar = source.descriptor().scalar_type();
        let components = argument.physical_components().collect::<Vec<_>>();
        let (offset, size, alignment, kind, address_space) = match components.as_slice() {
            [(PhysicalAbiComponentKind::ScalarByValue(_), offset, size, alignment)] => (
                *offset,
                u64::from(*size),
                u32::from(*alignment),
                AbiKind::Scalar(manifest_scalar(scalar)),
                AddressSpace::Value,
            ),
            [
                (PhysicalAbiComponentKind::GlobalPointer, offset, 8, 8),
                (PhysicalAbiComponentKind::SliceLengthU64, _, 8, 8),
            ] => (
                *offset,
                16,
                8,
                AbiKind::Slice {
                    element_size: u64::from(scalar.size_bytes()),
                    element_alignment: u32::from(scalar.alignment_bytes()),
                },
                AddressSpace::Global,
            ),
            _ => panic!("unsupported descriptor fixture shape"),
        };
        let access = manifest_access(argument.access());
        let layout_identity = if matches!(mutation, ManifestMutation::Abi)
            && kernel.entry_name().as_str() == "alpha"
            && index == 0
        {
            DigestBytes::from_bytes([0xee; 32])
        } else {
            DigestBytes::from_bytes(*argument.device_layout().as_bytes())
        };
        fields.push(
            AbiField::new(
                name(argument.name().as_str()),
                u64::from(offset),
                size,
                alignment,
                kind,
                if matches!(access, Access::WriteOnly | Access::ReadWrite) {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                },
                access,
                address_space,
                TypeIdentity::new(
                    DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                        *argument.source_type().as_bytes(),
                    )),
                    DeclaredRustLayoutIdentity::from_untrusted_bytes(layout_identity),
                ),
                match argument.ownership() {
                    OwnershipSemantics::ByValue => ArgumentOwnership::ByValue,
                    OwnershipSemantics::SharedBorrow => ArgumentOwnership::SharedBorrow,
                    OwnershipSemantics::UniqueBorrow => ArgumentOwnership::UniqueBorrow,
                },
                match argument.alias() {
                    AliasSemantics::Value => AliasClass::Value,
                    AliasSemantics::SharedReadOnly => AliasClass::SharedReadOnly,
                    AliasSemantics::Exclusive => AliasClass::Exclusive,
                },
            )
            .unwrap(),
        );
    }
    let layout = kernel.abi_layout();
    AbiLayout::new(
        u64::from(layout.explicit_argument_size()),
        layout.kernarg_segment_alignment(),
        PointerWidth::Bits64,
        fields,
    )
    .unwrap()
}

fn manifest_launch(kernel: &KernelDescriptorV1, mutation: ManifestMutation) -> LaunchContract {
    let launch = kernel.launch();
    let grid = launch.max_grid();
    let block = match launch.block_size() {
        BlockSizeV1::Any => BlockSize::Any,
        BlockSizeV1::Exact(value) => {
            BlockSize::Exact(Dimensions::new(value.x(), value.y(), value.z()).unwrap())
        }
        BlockSizeV1::AtMost(value) => {
            BlockSize::AtMost(Dimensions::new(value.x(), value.y(), value.z()).unwrap())
        }
    };
    let grid_x = if matches!(mutation, ManifestMutation::Launch)
        && kernel.entry_name().as_str() == "alpha"
    {
        grid.x() - 1
    } else {
        grid.x()
    };
    LaunchContract::new(
        launch.rank(),
        block,
        Dimensions::new(grid_x, grid.y(), grid.z()).unwrap(),
        launch.static_shared_memory_bytes(),
        launch.max_dynamic_shared_memory_bytes(),
    )
    .unwrap()
}

fn artifact_container(
    finalized: &[u8],
    table: &DeviceDescriptorTableV1,
    mutation: ManifestMutation,
) -> Vec<u8> {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, finalized.to_vec()).unwrap();
    let code_object_digest = payload.digest().bytes();
    let target_arch = if matches!(mutation, ManifestMutation::Target) {
        "gfx942"
    } else {
        "gfx942:xnack-"
    };
    let target_capabilities = if matches!(mutation, ManifestMutation::Capabilities) {
        vec![Capability::AmdWave, Capability::Atomics]
    } else {
        vec![Capability::AmdWave]
    };
    let kernels = table
        .kernels()
        .iter()
        .map(|kernel| {
            let source_digest = if matches!(mutation, ManifestMutation::Evidence)
                && kernel.entry_name().as_str() == "alpha"
            {
                DigestBytes::from_bytes([0xed; 32])
            } else {
                DigestBytes::from_bytes(*kernel.source_evidence().digest().as_bytes())
            };
            KernelEntry::new(
                DigestBytes::from_bytes(*kernel.kernel_id().as_bytes()),
                name(kernel.logical_name().as_str()),
                name(kernel.entry_name().as_str()),
                source_digest,
                DigestBytes::from_bytes(*kernel.executable_ir_evidence().digest().as_bytes()),
                code_object_digest,
                vec![Capability::AmdWave],
                manifest_launch(kernel, mutation),
                manifest_abi(table, kernel, mutation),
            )
            .unwrap()
        })
        .collect();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(
            text(if matches!(mutation, ManifestMutation::Compiler) {
                "substituted-backend"
            } else {
                table.compiler().name().as_str()
            }),
            text(table.compiler().release().as_str()),
        ),
        ToolIdentity::new(
            text(table.producer().name().as_str()),
            text(if matches!(mutation, ManifestMutation::Producer) {
                "substituted-worker"
            } else {
                table.producer().version().as_str()
            }),
        ),
        TargetIdentity::new(
            text("amdgcn-amd-amdhsa"),
            text(target_arch),
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
        kernels,
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload])
        .unwrap()
        .to_bytes()
}

fn encode_semantic_witness(kernel_binding: [u8; 32], host_contract: [u8; 32]) -> Vec<u8> {
    let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
    let length = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1 + profile.len();
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes());
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes());
    bytes.extend_from_slice(&(length as u32).to_le_bytes());
    bytes.extend_from_slice(&kernel_binding);
    bytes.extend_from_slice(&host_contract);
    bytes.extend_from_slice(&(profile.len() as u16).to_le_bytes());
    bytes.extend_from_slice(profile);
    bytes
}

fn semantic_witnesses(
    container: &[u8],
    table: &DeviceDescriptorTableV1,
) -> AlphaZetaSemanticLayoutWitnessesV1 {
    let container = ArtifactContainerV1::from_bytes(container).unwrap();
    AlphaZetaSemanticLayoutWitnessesV1::new(
        table
            .kernels()
            .iter()
            .map(|descriptor| {
                let kernel = container
                    .manifest()
                    .kernels()
                    .iter()
                    .find(|kernel| {
                        kernel.kernel_id().as_bytes() == descriptor.kernel_id().as_bytes()
                    })
                    .unwrap();
                let host_contract = derive_generated_host_contract_identity_v1(
                    TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
                    *descriptor.kernel_id().as_bytes(),
                    kernel.name().as_str(),
                    kernel.symbol().as_str(),
                    kernel.abi(),
                    kernel.launch(),
                );
                ExactSemanticLayoutWitnessV1::decode(
                    text(descriptor.logical_name().as_str()),
                    &encode_semantic_witness(
                        *descriptor.kernel_id().as_bytes(),
                        *host_contract.as_bytes(),
                    ),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn fixture(evidence_seed: u8, code_fill: u8) -> TransactionFixture {
    let hsaco = alpha_zeta_hsaco(
        CodeObjectVersion::V6,
        "gfx942:xnack-",
        evidence_seed,
        code_fill,
    );
    let finalized_result = finalize_unfinalized(&hsaco.bytes).unwrap();
    let table = finalized_result.inspection().descriptor_table().clone();
    let finalized_descriptor = encode_device_descriptor_table_v1(&table).unwrap();
    let finalized = finalized_result.into_bytes();
    let container = artifact_container(&finalized, &table, ManifestMutation::None);
    let semantic = semantic_witnesses(&container, &table);
    TransactionFixture {
        handoff: compiler_handoff(MODULE, "gfx942:xnack-", CompilerFfiCodeObjectVersion::V6),
        semantic,
        final_inputs: FinalInputs {
            finalized,
            descriptor_source: hsaco.descriptor_source.clone(),
            finalized_descriptor,
            container,
        },
        hsaco,
    }
}

fn recorder_at_ir(
    freshness: u8,
    handoff: &CompilerModuleHandoffV2,
    semantic: AlphaZetaSemanticLayoutWitnessesV1,
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
    let target = recorder.record_target(compiler, target).unwrap();
    let semantic = recorder.record_semantic_layouts(target, semantic).unwrap();
    let ir = recorder
        .record_kernel_ir(semantic, handoff.canonical_bytes())
        .unwrap();
    (recorder, ir)
}

fn recorder_at_raw(
    freshness: u8,
    fixture: &TransactionFixture,
) -> (
    CompilerTransactionRecorderV1,
    CompilerTransactionCheckpointV1,
) {
    let (mut recorder, ir) = recorder_at_ir(freshness, &fixture.handoff, fixture.semantic.clone());
    let (request, response) = worker_exchange(
        &fixture.handoff,
        &fixture.hsaco.bytes,
        freshness,
        worker_measurement(WORKER_BYTES, WORKER_BUILD, LLVM_BUILD),
        WORKER_BUILD,
    );
    let worker = recorder
        .record_worker_exchange(ir, &request, &response)
        .unwrap();
    let raw = recorder
        .record_raw_hsaco(worker, &fixture.hsaco.bytes)
        .unwrap();
    (recorder, raw)
}

fn seal_transaction(freshness: u8, fixture: &TransactionFixture) -> SealedCompilerTransactionV1 {
    let (mut recorder, raw) = recorder_at_raw(freshness, fixture);
    let finalized = recorder
        .record_finalized_artifact(
            raw,
            &fixture.final_inputs.finalized,
            &fixture.final_inputs.descriptor_source,
            &fixture.final_inputs.finalized_descriptor,
            &fixture.final_inputs.container,
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
fn canonical_cov6_alpha_zeta_transaction_round_trips() {
    let fixture = fixture(0x31, 0xbf);
    let sealed = seal_transaction(0x80, &fixture);
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
    let container = ArtifactContainerV1::from_bytes(&fixture.final_inputs.container).unwrap();
    let expected_target =
        derive_manifest_claim_target_identity_v1(&container).descriptive_identity();
    assert_eq!(sealed.evidence_capsule().target(), expected_target);
    assert!(sealed.requires_authenticated_execution_receipt());
    assert!(!sealed.authenticates_producer());
    assert!(!sealed.grants_launch_authority());
}

#[test]
fn scalar_gemm_v1_profile_requires_retained_exact_worker_evidence() {
    let witness = ExactSemanticLayoutWitnessV1::decode(
        text("scalar_gemm_v1"),
        &encode_semantic_witness([0x78; 32], [0x55; 32]),
    )
    .unwrap();
    let scalar = ScalarGemmV1SemanticLayoutWitnessV1::new(witness).unwrap();
    let (mut recorder, source_checkpoint) =
        CompilerTransactionRecorderV1::begin([0x7a; 32], source(0x7a)).unwrap();
    let invocation = invocation_for("gfx942:xnack-").unwrap();
    let target = Gfx942CompilerTargetV1::for_invocation(&invocation).unwrap();
    let compiler = recorder
        .record_compiler(source_checkpoint, invocation)
        .unwrap();
    let target = recorder.record_target(compiler, target).unwrap();
    let semantic = recorder
        .record_scalar_gemm_v1_semantic_layout(target, scalar)
        .unwrap();
    let ir = recorder
        .record_kernel_ir(semantic, scalar_compiler_handoff().canonical_bytes())
        .unwrap();

    assert!(matches!(
        recorder.record_worker_exchange(
            ir,
            b"caller-supplied-request",
            b"caller-supplied-response"
        ),
        Err(CompilerTransactionRecorderErrorV1::ScalarGemmV1WorkerEvidenceRequired)
    ));
}

#[test]
fn scalar_gemm_v1_witness_is_exact_and_domain_separated() {
    let bytes = encode_semantic_witness([0x78; 32], [0x55; 32]);
    let scalar_witness =
        ExactSemanticLayoutWitnessV1::decode(text("scalar_gemm_v1"), &bytes).unwrap();
    let scalar = ScalarGemmV1SemanticLayoutWitnessV1::new(scalar_witness.clone()).unwrap();
    let alpha = ExactSemanticLayoutWitnessV1::decode(text("alpha"), &bytes).unwrap();
    let zeta = ExactSemanticLayoutWitnessV1::decode(
        text("zeta"),
        &encode_semantic_witness([0x79; 32], [0x56; 32]),
    )
    .unwrap();
    let alpha_zeta = AlphaZetaSemanticLayoutWitnessesV1::new(vec![alpha.clone(), zeta]).unwrap();

    assert_eq!(scalar.witness(), &scalar_witness);
    assert_ne!(scalar.identity(), alpha_zeta.identity());
    assert!(matches!(
        ScalarGemmV1SemanticLayoutWitnessV1::new(alpha),
        Err(CompilerTransactionRecorderErrorV1::MissingScalarGemmV1Witness)
    ));
}

#[test]
fn scalar_gemm_v1_profile_rejects_alpha_zeta_symbol_closure() {
    let witness = ExactSemanticLayoutWitnessV1::decode(
        text("scalar_gemm_v1"),
        &encode_semantic_witness([0x78; 32], [0x55; 32]),
    )
    .unwrap();
    let scalar = ScalarGemmV1SemanticLayoutWitnessV1::new(witness).unwrap();
    let (mut recorder, source_checkpoint) =
        CompilerTransactionRecorderV1::begin([0x7b; 32], source(0x7b)).unwrap();
    let invocation = invocation_for("gfx942:xnack-").unwrap();
    let target = Gfx942CompilerTargetV1::for_invocation(&invocation).unwrap();
    let compiler = recorder
        .record_compiler(source_checkpoint, invocation)
        .unwrap();
    let target = recorder.record_target(compiler, target).unwrap();
    let semantic = recorder
        .record_scalar_gemm_v1_semantic_layout(target, scalar)
        .unwrap();

    assert!(matches!(
        recorder.record_kernel_ir(
            semantic,
            compiler_handoff(MODULE, "gfx942:xnack-", CompilerFfiCodeObjectVersion::V6,)
                .canonical_bytes()
        ),
        Err(CompilerTransactionRecorderErrorV1::WorkerKernelSetMismatch)
    ));
}

#[test]
fn target_and_cov6_are_exact_at_compiler_and_handoff_boundaries() {
    for wrong in ["gfx942", "gfx942:xnack+", "gfx942:sramecc+:xnack-"] {
        assert!(matches!(
            invocation_for(wrong),
            Err(CompilerTransactionRecorderErrorV1::UnsupportedTarget)
        ));
    }
    assert!(rustc_descriptor("gfx942:unknown+").is_none());
    assert!(rustc_descriptor("gfx942:xnack-:sramecc+").is_none());

    let fixture = fixture(0x31, 0xbf);
    for handoff in [
        compiler_handoff(MODULE, "gfx942:xnack-", CompilerFfiCodeObjectVersion::V5),
        compiler_handoff(MODULE, "gfx942:xnack+", CompilerFfiCodeObjectVersion::V6),
    ] {
        let (mut recorder, source) =
            CompilerTransactionRecorderV1::begin([0x81; 32], source(0x81)).unwrap();
        let invocation = invocation_for("gfx942:xnack-").unwrap();
        let target = Gfx942CompilerTargetV1::for_invocation(&invocation).unwrap();
        let compiler = recorder.record_compiler(source, invocation).unwrap();
        let target = recorder.record_target(compiler, target).unwrap();
        let semantic = recorder
            .record_semantic_layouts(target, fixture.semantic.clone())
            .unwrap();
        assert!(matches!(
            recorder.record_kernel_ir(semantic, handoff.canonical_bytes()),
            Err(CompilerTransactionRecorderErrorV1::WorkerCodeObjectVersionMismatch)
                | Err(CompilerTransactionRecorderErrorV1::WorkerTargetMismatch)
        ));
    }
}

#[test]
fn kernel_ir_worker_input_envelope_and_tool_forks_are_rejected() {
    let fixture = fixture(0x31, 0xbf);
    let fork = compiler_handoff(
        b"define amdgpu_kernel void @alpha() { ret void }\ndefine amdgpu_kernel void @zeta() { ret void }\n",
        "gfx942:xnack-",
        CompilerFfiCodeObjectVersion::V6,
    );
    let (request, response) = worker_exchange(
        &fork,
        &fixture.hsaco.bytes,
        0x82,
        worker_measurement(WORKER_BYTES, WORKER_BUILD, LLVM_BUILD),
        WORKER_BUILD,
    );
    let (mut recorder, ir) = recorder_at_ir(0x82, &fixture.handoff, fixture.semantic.clone());
    assert!(matches!(
        recorder.record_worker_exchange(ir, &request, &response),
        Err(CompilerTransactionRecorderErrorV1::WorkerInputMismatch)
    ));

    for measurement in [
        worker_measurement(b"substituted worker", WORKER_BUILD, LLVM_BUILD),
        worker_measurement(WORKER_BYTES, "wrong-worker-build", LLVM_BUILD),
        worker_measurement(WORKER_BYTES, WORKER_BUILD, "wrong-llvm-build"),
    ] {
        let (request, response) = worker_exchange(
            &fixture.handoff,
            &fixture.hsaco.bytes,
            0x83,
            measurement,
            WORKER_BUILD,
        );
        let (mut recorder, ir) = recorder_at_ir(0x83, &fixture.handoff, fixture.semantic.clone());
        assert!(matches!(
            recorder.record_worker_exchange(ir, &request, &response),
            Err(CompilerTransactionRecorderErrorV1::WorkerIdentityMismatch)
        ));
    }
}

#[test]
fn mixed_response_payload_and_noncanonical_exchange_are_rejected() {
    let first = fixture(0x31, 0xbf);
    let second = fixture(0x61, 0xa5);
    let (request, _) = worker_exchange(
        &first.handoff,
        &first.hsaco.bytes,
        0x84,
        worker_measurement(WORKER_BYTES, WORKER_BUILD, LLVM_BUILD),
        WORKER_BUILD,
    );
    let fork = compiler_handoff(
        b"forked canonical compiler module",
        "gfx942:xnack-",
        CompilerFfiCodeObjectVersion::V6,
    );
    let (_, mixed_response) = worker_exchange(
        &fork,
        &first.hsaco.bytes,
        0x85,
        worker_measurement(WORKER_BYTES, WORKER_BUILD, LLVM_BUILD),
        WORKER_BUILD,
    );
    let (mut recorder, ir) = recorder_at_ir(0x84, &first.handoff, first.semantic.clone());
    assert!(matches!(
        recorder.record_worker_exchange(ir, &request, &mixed_response),
        Err(CompilerTransactionRecorderErrorV1::InvalidWorkerResponse)
    ));

    let (request, response) = worker_exchange(
        &first.handoff,
        &first.hsaco.bytes,
        0x86,
        worker_measurement(WORKER_BYTES, WORKER_BUILD, LLVM_BUILD),
        WORKER_BUILD,
    );
    let (mut recorder, ir) = recorder_at_ir(0x86, &first.handoff, first.semantic.clone());
    let worker = recorder
        .record_worker_exchange(ir, &request, &response)
        .unwrap();
    assert!(matches!(
        recorder.record_raw_hsaco(worker, &second.hsaco.bytes),
        Err(CompilerTransactionRecorderErrorV1::WorkerOutputMismatch)
    ));

    let mut malformed = request;
    malformed.push(0);
    let (mut recorder, ir) = recorder_at_ir(0x87, &first.handoff, first.semantic.clone());
    assert!(matches!(
        recorder.record_worker_exchange(ir, &malformed, &response),
        Err(CompilerTransactionRecorderErrorV1::InvalidWorkerResponse)
    ));
}

#[test]
fn descriptor_payload_and_container_substitutions_cannot_seal() {
    let first = fixture(0x31, 0xbf);
    let second = fixture(0x61, 0xa5);

    let (mut recorder, raw) = recorder_at_raw(0x88, &first);
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &first.final_inputs.finalized,
            &second.final_inputs.descriptor_source,
            &first.final_inputs.finalized_descriptor,
            &first.final_inputs.container,
        ),
        Err(CompilerTransactionRecorderErrorV1::DescriptorSourceMismatch)
    ));

    let (mut recorder, raw) = recorder_at_raw(0x89, &first);
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &second.final_inputs.finalized,
            &first.final_inputs.descriptor_source,
            &second.final_inputs.finalized_descriptor,
            &second.final_inputs.container,
        ),
        Err(CompilerTransactionRecorderErrorV1::FinalizedHsacoMismatch)
    ));

    let (mut recorder, raw) = recorder_at_raw(0x8a, &first);
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &first.final_inputs.finalized,
            &first.final_inputs.descriptor_source,
            &second.final_inputs.finalized_descriptor,
            &first.final_inputs.container,
        ),
        Err(CompilerTransactionRecorderErrorV1::FinalizedDescriptorMismatch)
    ));

    let (mut recorder, raw) = recorder_at_raw(0x8b, &first);
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &first.final_inputs.finalized,
            &first.final_inputs.descriptor_source,
            &first.final_inputs.finalized_descriptor,
            &second.final_inputs.container,
        ),
        Err(CompilerTransactionRecorderErrorV1::ArtifactPayloadMismatch)
    ));
}

#[test]
fn artifact_identity_abi_launch_evidence_and_target_substitutions_fail() {
    let fixture = fixture(0x31, 0xbf);
    let finalized = inspect_unfinalized(&fixture.hsaco.bytes)
        .unwrap()
        .descriptor_table()
        .clone();
    for (mutation, expected) in [
        (
            ManifestMutation::Target,
            CompilerTransactionRecorderErrorV1::ArtifactTargetMismatch,
        ),
        (
            ManifestMutation::Capabilities,
            CompilerTransactionRecorderErrorV1::ArtifactCapabilityMismatch,
        ),
        (
            ManifestMutation::Compiler,
            CompilerTransactionRecorderErrorV1::ArtifactCompilerMismatch,
        ),
        (
            ManifestMutation::Producer,
            CompilerTransactionRecorderErrorV1::ArtifactProducerMismatch,
        ),
        (
            ManifestMutation::Evidence,
            CompilerTransactionRecorderErrorV1::ArtifactKernelSetMismatch,
        ),
        (
            ManifestMutation::Abi,
            CompilerTransactionRecorderErrorV1::ArtifactAbiMismatch,
        ),
        (
            ManifestMutation::Launch,
            CompilerTransactionRecorderErrorV1::ArtifactLaunchMismatch,
        ),
    ] {
        let container = artifact_container(&fixture.final_inputs.finalized, &finalized, mutation);
        let (mut recorder, raw) = recorder_at_raw(0x8c, &fixture);
        let error = recorder
            .record_finalized_artifact(
                raw,
                &fixture.final_inputs.finalized,
                &fixture.final_inputs.descriptor_source,
                &fixture.final_inputs.finalized_descriptor,
                &container,
            )
            .unwrap_err();
        assert_eq!(error, expected);
    }
}

#[test]
fn semantic_witness_mutations_and_substitution_fail_closed() {
    let fixture = fixture(0x31, 0xbf);
    let mut malformed = encode_semantic_witness([0xa1; 32], [0x55; 32]);
    malformed.push(0);
    assert!(matches!(
        ExactSemanticLayoutWitnessV1::decode(text("alpha"), &malformed),
        Err(CompilerTransactionRecorderErrorV1::InvalidSemanticWitness)
    ));

    let wrong_semantic = AlphaZetaSemanticLayoutWitnessesV1::new(vec![
        ExactSemanticLayoutWitnessV1::decode(
            text("alpha"),
            &encode_semantic_witness([0xa1; 32], [0xee; 32]),
        )
        .unwrap(),
        fixture.semantic.witnesses()[1].clone(),
    ])
    .unwrap();
    let (mut recorder, ir) = recorder_at_ir(0x8d, &fixture.handoff, wrong_semantic);
    let (request, response) = worker_exchange(
        &fixture.handoff,
        &fixture.hsaco.bytes,
        0x8d,
        worker_measurement(WORKER_BYTES, WORKER_BUILD, LLVM_BUILD),
        WORKER_BUILD,
    );
    let worker = recorder
        .record_worker_exchange(ir, &request, &response)
        .unwrap();
    let raw = recorder
        .record_raw_hsaco(worker, &fixture.hsaco.bytes)
        .unwrap();
    assert!(matches!(
        recorder.record_finalized_artifact(
            raw,
            &fixture.final_inputs.finalized,
            &fixture.final_inputs.descriptor_source,
            &fixture.final_inputs.finalized_descriptor,
            &fixture.final_inputs.container,
        ),
        Err(CompilerTransactionRecorderErrorV1::ArtifactSemanticWitnessMismatch)
    ));
}

#[test]
fn cov5_hsaco_is_rejected_as_raw_output() {
    let fixture = fixture(0x31, 0xbf);
    let cov5 = alpha_zeta_hsaco(CodeObjectVersion::V5, "gfx942:xnack-", 0x31, 0xbf);
    let (mut recorder, ir) = recorder_at_ir(0x8e, &fixture.handoff, fixture.semantic.clone());
    let (request, response) = worker_exchange(
        &fixture.handoff,
        &cov5.bytes,
        0x8e,
        worker_measurement(WORKER_BYTES, WORKER_BUILD, LLVM_BUILD),
        WORKER_BUILD,
    );
    let worker = recorder
        .record_worker_exchange(ir, &request, &response)
        .unwrap();
    assert!(matches!(
        recorder.record_raw_hsaco(worker, &cov5.bytes),
        Err(CompilerTransactionRecorderErrorV1::InvalidRawHsaco)
            | Err(CompilerTransactionRecorderErrorV1::DescriptorCodeObjectVersionMismatch)
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
        Err(CompilerTransactionRecorderErrorV1::UnexpectedStage { .. })
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
    assert_eq!(compiler.stage(), CompilerTransactionStageV1::Compiler);
}

#[test]
fn sealed_mutations_stale_identity_and_decode_remain_inert() {
    let fixture = fixture(0x31, 0xbf);
    let sealed = seal_transaction(0x93, &fixture);
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

    let stale = seal_transaction(0x94, &fixture);
    assert!(matches!(
        SealedCompilerTransactionV1::from_bytes_for_identity(&stale.to_bytes(), sealed.identity()),
        Err(SealedCompilerTransactionDecodeErrorV1::UnexpectedRecordIdentity)
    ));
    let decoded = SealedCompilerTransactionV1::from_bytes(&bytes).unwrap();
    assert!(decoded.requires_authenticated_execution_receipt());
    assert!(!decoded.authenticates_producer());
    assert!(!decoded.grants_publication_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
}
