use super::*;

use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_compiler_ffi::{
    CodeObjectVersion as FfiCodeObjectVersion, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestErrorV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1,
};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallTargetV2,
    CallingConventionV2, CastOperationV2, ComparePredicateV2, DeviceLibraryInputV1, EvidenceV2,
    ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV1, FunctionAttributeV2,
    FunctionIdV2, FunctionKindV2, FunctionParameterV2, FunctionV2, GENERAL_GEMM_BINDING_SECTION_V2,
    Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1, GlobalIdV2,
    GlobalV2, HandoffDiagnosticV1, HandoffDiagnosticV2, IdentityV1, InstructionKindV2,
    InstructionV2, IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2,
    KERNEL_DESCRIPTOR_SECTION_V2, KernelEntryV1, KernelParameterV1, KernelValueTypeV1,
    ModuleFlagV1, ModuleMetadataV1, NamedMetadataV1, ObligationKindV1, ObligationV1, OriginKindV1,
    OriginV1, ParameterAttributeV1, ReturnTypeV2, ScalarConstantV2, ScalarTypeV1,
    StageIdentitiesV1, TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2, WorkgroupSizeRangeV1,
};
use fe2o3_llvm_text::{Gfx942LlvmAssemblyV2, SerializeErrorV2, serialize_gfx942_handoff_v2};

/// Canonical source-binding section schema retained in every general-GEMM object.
pub const GENERAL_GEMM_MACHINE_BINDING_SCHEMA_V1: &str = "fe2o3.general-gemm.machine-binding.v1";

const MACHINE_BINDING_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.machine-binding.identity.v1\0";
const LDS_A_SYMBOL_V1: &str = "general_gemm_a_lds";
const LDS_B_SYMBOL_V1: &str = "general_gemm_b_lds";
const DESCRIPTOR_SOURCE_SYMBOL_V1: &str = "general_gemm_descriptor_source";
const MACHINE_BINDING_SYMBOL_V1: &str = "general_gemm_compilation_binding";

const GLOBAL_LDS_A: u32 = 1;
const GLOBAL_LDS_B: u32 = 2;
const GLOBAL_DESCRIPTOR: u32 = 3;
const GLOBAL_BINDING: u32 = 4;
const KERNEL_FUNCTION: u32 = 1;

/// SHA-256 identity of the exact retained general-GEMM machine-binding section.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmMachineBindingIdentityV1([u8; 32]);

impl GeneralGemmMachineBindingIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Bounded canonical bytes retained in `.fe2o3.general-gemm.binding.v1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralGemmMachineBindingSectionV1 {
    bytes: Vec<u8>,
    identity: GeneralGemmMachineBindingIdentityV1,
}

impl GeneralGemmMachineBindingSectionV1 {
    /// Returns the exact retained section bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the SHA-256 identity of the exact section bytes.
    pub const fn identity(&self) -> GeneralGemmMachineBindingIdentityV1 {
        self.identity
    }
}

/// A complete structural machine route that grants no artifact or runtime authority.
#[derive(Debug)]
pub struct GeneralGemmStructuralMachineV1 {
    projection: GeneralGemmPlironProjectionV1,
    descriptor_source: CompilerDescriptorSourceV1,
    binding_section: GeneralGemmMachineBindingSectionV1,
    handoff: Gfx942HandoffV2,
    assembly: Gfx942LlvmAssemblyV2,
    compiler_handoff: CompilerModuleHandoffV2,
}

/// Complete symbolic machine route produced while concrete launch values remain dynamic.
#[derive(Debug)]
pub struct GeneralGemmSymbolicStructuralMachineV1 {
    projection: GeneralGemmSymbolicPlironProjectionV1,
    descriptor_source: CompilerDescriptorSourceV1,
    binding_section: GeneralGemmMachineBindingSectionV1,
    handoff: Gfx942HandoffV2,
    assembly: Gfx942LlvmAssemblyV2,
    compiler_handoff: CompilerModuleHandoffV2,
    artifact_identity: GeneralGemmSymbolicArtifactIdentityV1,
}

impl GeneralGemmSymbolicStructuralMachineV1 {
    /// Returns the owner-checked symbolic Pliron projection.
    pub const fn projection(&self) -> GeneralGemmSymbolicPlironProjectionV1 {
        self.projection
    }

    /// Returns the exact compiler-owned descriptor source.
    pub const fn descriptor_source(&self) -> &CompilerDescriptorSourceV1 {
        &self.descriptor_source
    }

    /// Returns the exact retained symbolic source-binding section.
    pub const fn binding_section(&self) -> &GeneralGemmMachineBindingSectionV1 {
        &self.binding_section
    }

    /// Returns the complete typed dynamic gfx942 Handoff V2 graph.
    pub const fn handoff(&self) -> &Gfx942HandoffV2 {
        &self.handoff
    }

    /// Returns the deterministic dynamic LLVM assembly.
    pub const fn assembly(&self) -> &Gfx942LlvmAssemblyV2 {
        &self.assembly
    }

    /// Returns the inert exact compiler-worker handoff.
    pub const fn compiler_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.compiler_handoff
    }

    /// Returns the descriptive symbolic artifact identity for launch binding.
    pub const fn artifact_identity(&self) -> GeneralGemmSymbolicArtifactIdentityV1 {
        self.artifact_identity
    }

    /// Structural machine data grants no artifact, load, or launch authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

impl GeneralGemmStructuralMachineV1 {
    /// Returns the owner-checked structural Pliron projection.
    pub const fn projection(&self) -> GeneralGemmPlironProjectionV1 {
        self.projection
    }

    /// Returns the exact compiler-owned descriptor source.
    pub const fn descriptor_source(&self) -> &CompilerDescriptorSourceV1 {
        &self.descriptor_source
    }

    /// Returns the exact retained source-binding section.
    pub const fn binding_section(&self) -> &GeneralGemmMachineBindingSectionV1 {
        &self.binding_section
    }

    /// Returns the complete typed gfx942 Handoff V2 graph.
    pub const fn handoff(&self) -> &Gfx942HandoffV2 {
        &self.handoff
    }

    /// Returns the deterministic LLVM assembly derived from the typed graph.
    pub const fn assembly(&self) -> &Gfx942LlvmAssemblyV2 {
        &self.assembly
    }

    /// Returns the inert exact compiler-worker handoff.
    pub const fn compiler_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.compiler_handoff
    }

    /// Structural lowering authenticates no proof, artifact, worker, load, or launch authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

/// Failure while deriving the closed general-GEMM machine route.
#[derive(Debug)]
pub enum GeneralGemmStructuralMachineErrorV1 {
    /// Owner-bound Pliron projection construction or validation failed.
    PlironProjection,
    /// Compiler-owned descriptor construction failed.
    Descriptor(GeneralGemmDescriptorSourceErrorV1),
    /// Canonical Handoff V1 construction failed.
    HandoffV1(HandoffDiagnosticV1),
    /// Typed Handoff V2 construction failed.
    HandoffV2(HandoffDiagnosticV2),
    /// LLVM assembly serialization failed.
    Serialize(SerializeErrorV2),
    /// The serializer did not retain the exact Handoff V2 identity.
    SourceIdentity,
    /// The compiler FFI envelope failed closed.
    CompilerEnvelope(CompilerFfiEnvelopeError),
    /// The exact symbol manifest failed closed.
    SymbolManifest(CompilerModuleSymbolManifestErrorV1),
    /// Compiler-worker handoff construction failed closed.
    CompilerHandoff(CompilerModuleHandoffErrorV2),
    /// A closed internal builder invariant failed without producing data.
    Construction,
}

impl fmt::Display for GeneralGemmStructuralMachineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM structural machine lowering failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmStructuralMachineErrorV1 {}

/// Lowers one checked unit through Pliron, typed Handoff V2, LLVM text, and an inert worker handoff.
///
/// This function accepts descriptive structural data only. It intentionally
/// cannot create a candidate or artifact and grants no publication, loading,
/// or launch authority. The eventual authority join must additionally consume
/// the rustc-owned frontend correspondence and verifier-owned machine evidence.
pub fn lower_general_gemm_structural_machine_v1(
    unit: &GeneralGemmCompilationUnitV1,
) -> Result<GeneralGemmStructuralMachineV1, GeneralGemmStructuralMachineErrorV1> {
    catch_unwind(AssertUnwindSafe(|| lower_structural_inner(unit)))
        .unwrap_or(Err(GeneralGemmStructuralMachineErrorV1::Construction))
}

/// Lowers an authenticated symbolic template through Pliron and typed LLVM handoff.
///
/// The dynamic machine body consumes runtime ABI operands and contains no
/// witness launch values. This public structural result remains non-authoritative;
/// rustc must synchronously retain its private frontend correspondence while a
/// measured worker and opaque finalizer observation are joined.
pub fn lower_general_gemm_symbolic_structural_machine_v1(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
) -> Result<GeneralGemmSymbolicStructuralMachineV1, GeneralGemmStructuralMachineErrorV1> {
    catch_unwind(AssertUnwindSafe(|| lower_symbolic_structural_inner(unit)))
        .unwrap_or(Err(GeneralGemmStructuralMachineErrorV1::Construction))
}

fn lower_symbolic_structural_inner(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
) -> Result<GeneralGemmSymbolicStructuralMachineV1, GeneralGemmStructuralMachineErrorV1> {
    let envelope = project_symbolic_to_pliron(unit)
        .map_err(|_| GeneralGemmStructuralMachineErrorV1::PlironProjection)?;
    let lowered = envelope
        .into_verified_lowered(unit)
        .map_err(|_| GeneralGemmStructuralMachineErrorV1::PlironProjection)?;
    let projection = lowered.projection();
    let descriptor_source = derive_symbolic_descriptor_source(&lowered)
        .map_err(GeneralGemmStructuralMachineErrorV1::Descriptor)?;
    let binding_section = symbolic_machine_binding_section(&lowered, &descriptor_source);
    let handoff = build_symbolic_machine_handoff(&lowered, &descriptor_source, &binding_section)?;
    let source_identity = handoff.identity();
    let assembly = serialize_gfx942_handoff_v2(&handoff)
        .map_err(GeneralGemmStructuralMachineErrorV1::Serialize)?;
    if assembly.source_identity() != source_identity || !assembly.has_embedded_source_identity() {
        return Err(GeneralGemmStructuralMachineErrorV1::SourceIdentity);
    }
    let target = descriptor_source.table().device_target();
    let compiler_envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, FfiCodeObjectVersion::V6)
            .map_err(GeneralGemmStructuralMachineErrorV1::CompilerEnvelope)?;
    let manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            GENERAL_GEMM_KERNEL_SYMBOL_V1,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            GENERAL_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ),
    ])
    .map_err(GeneralGemmStructuralMachineErrorV1::SymbolManifest)?;
    let compiler_handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        FfiCodeObjectVersion::V6,
        compiler_envelope,
        manifest,
        assembly.as_bytes(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::CompilerHandoff)?;
    let artifact_identity = GeneralGemmSymbolicArtifactIdentityV1(hash_fields(
        ARTIFACT_IDENTITY_DOMAIN_V1,
        &[
            lowered.compilation_identity().as_bytes(),
            projection.identity().as_bytes(),
            source_identity.as_bytes(),
            assembly.sha256().as_bytes(),
            compiler_handoff.identity().sha256(),
            binding_section.identity().as_bytes(),
            descriptor_source.identity().sha256(),
        ],
    ));
    Ok(GeneralGemmSymbolicStructuralMachineV1 {
        projection,
        descriptor_source,
        binding_section,
        handoff,
        assembly,
        compiler_handoff,
        artifact_identity,
    })
}

fn lower_structural_inner(
    unit: &GeneralGemmCompilationUnitV1,
) -> Result<GeneralGemmStructuralMachineV1, GeneralGemmStructuralMachineErrorV1> {
    let envelope = project_to_pliron(unit)
        .map_err(|_| GeneralGemmStructuralMachineErrorV1::PlironProjection)?;
    envelope
        .validate_exact(unit)
        .map_err(|_| GeneralGemmStructuralMachineErrorV1::PlironProjection)?;
    let projection = envelope.receipt;
    let descriptor_source = derive_general_gemm_descriptor_source_v1(unit, projection)
        .map_err(GeneralGemmStructuralMachineErrorV1::Descriptor)?;
    let binding_section = machine_binding_section(unit, projection, &descriptor_source);
    let handoff = build_machine_handoff(unit, projection, &descriptor_source, &binding_section)?;
    let source_identity = handoff.identity();
    let assembly = serialize_gfx942_handoff_v2(&handoff)
        .map_err(GeneralGemmStructuralMachineErrorV1::Serialize)?;
    if assembly.source_identity() != source_identity || !assembly.has_embedded_source_identity() {
        return Err(GeneralGemmStructuralMachineErrorV1::SourceIdentity);
    }
    let target = descriptor_source.table().device_target();
    let compiler_envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, FfiCodeObjectVersion::V6)
            .map_err(GeneralGemmStructuralMachineErrorV1::CompilerEnvelope)?;
    let manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            GENERAL_GEMM_KERNEL_SYMBOL_V1,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            GENERAL_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ),
    ])
    .map_err(GeneralGemmStructuralMachineErrorV1::SymbolManifest)?;
    let compiler_handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        FfiCodeObjectVersion::V6,
        compiler_envelope,
        manifest,
        assembly.as_bytes(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::CompilerHandoff)?;
    Ok(GeneralGemmStructuralMachineV1 {
        projection,
        descriptor_source,
        binding_section,
        handoff,
        assembly,
        compiler_handoff,
    })
}

fn machine_binding_section(
    unit: &GeneralGemmCompilationUnitV1,
    projection: GeneralGemmPlironProjectionV1,
    descriptor: &CompilerDescriptorSourceV1,
) -> GeneralGemmMachineBindingSectionV1 {
    let mut bytes = Vec::with_capacity(768);
    append_field(
        &mut bytes,
        GENERAL_GEMM_MACHINE_BINDING_SCHEMA_V1.as_bytes(),
    );
    for identity in [
        unit.identity().into_bytes(),
        unit.request().identity().into_bytes(),
        unit.request().kernel_instance_identity().into_bytes(),
        unit.frontend_semantic_binding_identity().into_bytes(),
        *unit.frontend_semantics().compiled_source_identity(),
        *unit.frontend_semantics().provider_semantics_identity(),
        *unit.frontend_semantics().frontend_abi_identity(),
        unit.frontend_semantics()
            .symbolic_plan()
            .identity()
            .into_bytes(),
        unit.frontend_semantics()
            .symbolic_kir()
            .identity()
            .into_bytes(),
        unit.plan_identity().into_bytes(),
        *unit.kir_identity().as_bytes(),
        unit.schedule_identity().into_bytes(),
        unit.runtime_abi_identity().into_bytes(),
        unit.toolchain_route_identity().into_bytes(),
        projection.identity().into_bytes(),
        *descriptor.identity().sha256(),
    ] {
        append_field(&mut bytes, &identity);
    }
    append_field(&mut bytes, &unit.schedule().encode_canonical());
    append_field(&mut bytes, &unit.kir().encode_canonical());
    let identity = GeneralGemmMachineBindingIdentityV1(hash_fields(
        MACHINE_BINDING_IDENTITY_DOMAIN_V1,
        &[&bytes],
    ));
    GeneralGemmMachineBindingSectionV1 { bytes, identity }
}

fn symbolic_machine_binding_section(
    lowered: &GeneralGemmVerifiedLoweredGpuReceiptV1,
    descriptor: &CompilerDescriptorSourceV1,
) -> GeneralGemmMachineBindingSectionV1 {
    let mut bytes = Vec::with_capacity(768);
    append_field(
        &mut bytes,
        GENERAL_GEMM_MACHINE_BINDING_SCHEMA_V1.as_bytes(),
    );
    for identity in [
        lowered.compilation_identity().into_bytes(),
        *lowered.request_identity(),
        *lowered.kernel_instance_identity(),
        lowered.frontend_semantic_binding_identity().into_bytes(),
        *lowered.compiled_source_identity(),
        *lowered.provider_semantics_identity(),
        *lowered.frontend_abi_identity(),
        lowered.symbolic_plan_identity().into_bytes(),
        lowered.symbolic_kir_identity().into_bytes(),
        lowered.schedule_identity().into_bytes(),
        lowered.toolchain_route_identity().into_bytes(),
        lowered.projection().identity().into_bytes(),
        *descriptor.identity().sha256(),
    ] {
        append_field(&mut bytes, &identity);
    }
    append_field(&mut bytes, &lowered.schedule().encode_canonical());
    append_field(&mut bytes, lowered.symbolic_kir_template());
    let identity = GeneralGemmMachineBindingIdentityV1(hash_fields(
        MACHINE_BINDING_IDENTITY_DOMAIN_V1,
        &[&bytes],
    ));
    GeneralGemmMachineBindingSectionV1 { bytes, identity }
}

fn append_field(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
    bytes.extend_from_slice(field);
}

fn derive_symbolic_descriptor_source(
    lowered: &GeneralGemmVerifiedLoweredGpuReceiptV1,
) -> Result<CompilerDescriptorSourceV1, GeneralGemmDescriptorSourceErrorV1> {
    let projection = lowered.projection();

    let bf16_slice_type = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(
        DescriptorScalarTypeV1::U16,
    ));
    let c_slice_type = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(
        DescriptorScalarTypeV1::F32,
    ));
    let u32_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(DescriptorScalarTypeV1::U32));
    let f32_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(DescriptorScalarTypeV1::F32));
    let bf16_slice_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(
        DescriptorScalarTypeV1::U16,
    ));
    let c_slice_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(
        DescriptorScalarTypeV1::F32,
    ));
    let u32_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(
        DescriptorScalarTypeV1::U32,
    ));
    let f32_layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(
        DescriptorScalarTypeV1::F32,
    ));
    let name = |value: &'static str| ValidName::new(value);
    let arguments = vec![
        LogicalArgumentV1::shared_slice(0, name("a")?, &bf16_slice_type, &bf16_slice_layout, 0)?,
        LogicalArgumentV1::shared_slice(1, name("b")?, &bf16_slice_type, &bf16_slice_layout, 16)?,
        LogicalArgumentV1::disjoint_slice(
            2,
            name("c")?,
            &c_slice_type,
            &c_slice_layout,
            AccessMode::ReadWrite,
            32,
        )?,
        LogicalArgumentV1::scalar(3, name("m")?, &u32_type, &u32_layout, 48)?,
        LogicalArgumentV1::scalar(4, name("n")?, &u32_type, &u32_layout, 52)?,
        LogicalArgumentV1::scalar(5, name("k")?, &u32_type, &u32_layout, 56)?,
        LogicalArgumentV1::scalar(6, name("lda")?, &u32_type, &u32_layout, 60)?,
        LogicalArgumentV1::scalar(7, name("ldb")?, &u32_type, &u32_layout, 64)?,
        LogicalArgumentV1::scalar(8, name("ldc")?, &u32_type, &u32_layout, 68)?,
        LogicalArgumentV1::scalar(9, name("alpha")?, &f32_type, &f32_layout, 72)?,
        LogicalArgumentV1::scalar(10, name("beta")?, &f32_type, &f32_layout, 76)?,
    ];
    let source_evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(
            lowered.frontend_semantic_binding_identity().into_bytes(),
        ),
        EvidenceDigest::from_sha256_bytes(*lowered.compiled_source_identity()),
    );
    let executable_ir_evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(projection.identity().into_bytes()),
        EvidenceDigest::from_sha256_bytes(lowered.symbolic_kir_identity().into_bytes()),
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(lowered.compilation_identity().into_bytes()),
        name(GENERAL_GEMM_KERNEL_SYMBOL_V1)?,
        name(GENERAL_GEMM_KERNEL_SYMBOL_V1)?,
        name(GENERAL_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1)?,
        source_evidence,
        executable_ir_evidence,
        vec![
            CapabilityV1::WorkgroupMemory,
            CapabilityV1::MatrixMultiply,
            CapabilityV1::AmdWave,
            CapabilityV1::AmdMfma,
        ],
        KernelAbiLayoutV1::new(
            GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1,
            GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1,
            8,
        )?,
        LaunchConstraintsV1::new(
            2,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1)?),
            DimensionsV1::new(u32::MAX, u32::MAX, 1)?,
            64,
            GENERAL_GEMM_STATIC_LDS_BYTES_V1,
            0,
        )?,
        arguments,
    )?;
    let target = DeviceTargetV1::parse(GENERAL_GEMM_DEVICE_TARGET_V1)?;
    let table = DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("fe2o3-general-gemm-compiler")?,
            Text::new(env!("CARGO_PKG_VERSION"))?,
            [0; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("fe2o3-general-gemm-compiler")?,
            Text::new("general-gemm-symbolic-v1")?,
        ),
        target,
        vec![bf16_slice_type, c_slice_type, u32_type, f32_type],
        vec![bf16_slice_layout, c_slice_layout, u32_layout, f32_layout],
        vec![kernel],
    )?;
    CompilerDescriptorSourceV1::new(table).map_err(Into::into)
}

fn build_symbolic_machine_handoff(
    lowered: &GeneralGemmVerifiedLoweredGpuReceiptV1,
    descriptor: &CompilerDescriptorSourceV1,
    binding: &GeneralGemmMachineBindingSectionV1,
) -> Result<Gfx942HandoffV2, GeneralGemmStructuralMachineErrorV1> {
    let base = build_symbolic_base_handoff(lowered)?;
    let evidence = EvidenceV2::new(
        base.origins()[0].identity(),
        base.obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)?;
    let globals = vec![
        GlobalV2::new_lds_bf16_array_256(
            GlobalIdV2::new(GLOBAL_LDS_A),
            LDS_A_SYMBOL_V1,
            evidence.clone(),
        ),
        GlobalV2::new_lds_bf16_array_256(
            GlobalIdV2::new(GLOBAL_LDS_B),
            LDS_B_SYMBOL_V1,
            evidence.clone(),
        ),
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(GLOBAL_DESCRIPTOR),
            DESCRIPTOR_SOURCE_SYMBOL_V1,
            KERNEL_DESCRIPTOR_SECTION_V2,
            descriptor.canonical_bytes().to_vec(),
            8,
            evidence.clone(),
        ),
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(GLOBAL_BINDING),
            MACHINE_BINDING_SYMBOL_V1,
            GENERAL_GEMM_BINDING_SECTION_V2,
            binding.canonical_bytes().to_vec(),
            16,
            evidence.clone(),
        ),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)?;
    let intrinsics = [
        IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::X),
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Y),
        IntrinsicV2::AmdGpuBarrier,
        IntrinsicV2::Trap,
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
    ]
    .into_iter()
    .map(|intrinsic| IntrinsicReferenceV2::new(intrinsic, evidence.clone()))
    .collect();
    let function = build_kernel_function(&base, lowered.schedule(), evidence.clone())?;
    let module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        globals,
        intrinsics,
        vec![function],
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)?;
    Gfx942HandoffV2::new(base, module).map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)
}

fn build_symbolic_base_handoff(
    lowered: &GeneralGemmVerifiedLoweredGpuReceiptV1,
) -> Result<Gfx942HandoffV1, GeneralGemmStructuralMachineErrorV1> {
    let projection = lowered.projection();
    let origin_source = IdentityV1::new(projection.identity().into_bytes())
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let origin = OriginV1::new(OriginKindV1::KernelIr, origin_source, None);
    let kernel = KernelEntryV1::new(
        GENERAL_GEMM_KERNEL_SYMBOL_V1,
        kernel_parameters_v1().map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?,
        FunctionAttributeV1::gfx942_kernel_defaults(
            WorkgroupSizeRangeV1::new(64, 64)
                .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?,
        ),
        origin.identity(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let semantic = IdentityV1::new(lowered.compilation_identity().into_bytes())
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let target = IdentityV1::new(lowered.toolchain_route_identity().into_bytes())
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let obligations = [
        ObligationKindV1::PreserveKernelAbi,
        ObligationKindV1::PreserveAddressSpaces,
        ObligationKindV1::PreserveTargetFeatures,
        ObligationKindV1::PreserveCallingConvention,
        ObligationKindV1::PreserveFunctionAttributes,
        ObligationKindV1::PreserveModuleMetadata,
        ObligationKindV1::AuthenticateDeviceLibraries,
        ObligationKindV1::MaintainOriginCoverage,
    ]
    .into_iter()
    .map(|kind| {
        let subject = match kind {
            ObligationKindV1::PreserveKernelAbi | ObligationKindV1::MaintainOriginCoverage => {
                semantic
            }
            _ => target,
        };
        ObligationV1::new(kind, subject, origin.identity())
    })
    .collect();
    let module = ModuleMetadataV1::new(
        vec![
            ModuleFlagV1::CodeObjectVersion6,
            ModuleFlagV1::PicLevel2,
            ModuleFlagV1::WcharSize4,
        ],
        vec![
            NamedMetadataV1::OpenClVersion2_0,
            NamedMetadataV1::ProducerIdentity(target),
        ],
        Vec::<DeviceLibraryInputV1>::new(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new(
            lowered.compilation_identity().into_bytes(),
            lowered.schedule_identity().into_bytes(),
            lowered.toolchain_route_identity().into_bytes(),
        )
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?,
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module,
        origins: vec![origin],
        obligations,
    })
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)
}

fn build_machine_handoff(
    unit: &GeneralGemmCompilationUnitV1,
    projection: GeneralGemmPlironProjectionV1,
    descriptor: &CompilerDescriptorSourceV1,
    binding: &GeneralGemmMachineBindingSectionV1,
) -> Result<Gfx942HandoffV2, GeneralGemmStructuralMachineErrorV1> {
    let base = build_base_handoff(unit, projection)?;
    let evidence = EvidenceV2::new(
        base.origins()[0].identity(),
        base.obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)?;
    let globals = vec![
        GlobalV2::new_lds_bf16_array_256(
            GlobalIdV2::new(GLOBAL_LDS_A),
            LDS_A_SYMBOL_V1,
            evidence.clone(),
        ),
        GlobalV2::new_lds_bf16_array_256(
            GlobalIdV2::new(GLOBAL_LDS_B),
            LDS_B_SYMBOL_V1,
            evidence.clone(),
        ),
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(GLOBAL_DESCRIPTOR),
            DESCRIPTOR_SOURCE_SYMBOL_V1,
            KERNEL_DESCRIPTOR_SECTION_V2,
            descriptor.canonical_bytes().to_vec(),
            8,
            evidence.clone(),
        ),
        GlobalV2::new_private_constant_bytes(
            GlobalIdV2::new(GLOBAL_BINDING),
            MACHINE_BINDING_SYMBOL_V1,
            GENERAL_GEMM_BINDING_SECTION_V2,
            binding.canonical_bytes().to_vec(),
            16,
            evidence.clone(),
        ),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)?;
    let intrinsics = [
        IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::X),
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Y),
        IntrinsicV2::AmdGpuBarrier,
        IntrinsicV2::Trap,
        IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
    ]
    .into_iter()
    .map(|intrinsic| IntrinsicReferenceV2::new(intrinsic, evidence.clone()))
    .collect();
    let function = build_kernel_function(&base, unit.schedule(), evidence.clone())?;
    let module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        globals,
        intrinsics,
        vec![function],
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)?;
    Gfx942HandoffV2::new(base, module).map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)
}

fn build_base_handoff(
    unit: &GeneralGemmCompilationUnitV1,
    projection: GeneralGemmPlironProjectionV1,
) -> Result<Gfx942HandoffV1, GeneralGemmStructuralMachineErrorV1> {
    let origin_source = IdentityV1::new(projection.identity().into_bytes())
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let origin = OriginV1::new(OriginKindV1::KernelIr, origin_source, None);
    let kernel = KernelEntryV1::new(
        GENERAL_GEMM_KERNEL_SYMBOL_V1,
        kernel_parameters_v1().map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?,
        FunctionAttributeV1::gfx942_kernel_defaults(
            WorkgroupSizeRangeV1::new(64, 64)
                .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?,
        ),
        origin.identity(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let semantic = IdentityV1::new(unit.identity().into_bytes())
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let target = IdentityV1::new(unit.toolchain_route_identity().into_bytes())
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    let obligations = [
        ObligationKindV1::PreserveKernelAbi,
        ObligationKindV1::PreserveAddressSpaces,
        ObligationKindV1::PreserveTargetFeatures,
        ObligationKindV1::PreserveCallingConvention,
        ObligationKindV1::PreserveFunctionAttributes,
        ObligationKindV1::PreserveModuleMetadata,
        ObligationKindV1::AuthenticateDeviceLibraries,
        ObligationKindV1::MaintainOriginCoverage,
    ]
    .into_iter()
    .map(|kind| {
        let subject = match kind {
            ObligationKindV1::PreserveKernelAbi | ObligationKindV1::MaintainOriginCoverage => {
                semantic
            }
            _ => target,
        };
        ObligationV1::new(kind, subject, origin.identity())
    })
    .collect();
    let module = ModuleMetadataV1::new(
        vec![
            ModuleFlagV1::CodeObjectVersion6,
            ModuleFlagV1::PicLevel2,
            ModuleFlagV1::WcharSize4,
        ],
        vec![
            NamedMetadataV1::OpenClVersion2_0,
            NamedMetadataV1::ProducerIdentity(target),
        ],
        Vec::<DeviceLibraryInputV1>::new(),
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?;
    Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new(
            unit.identity().into_bytes(),
            unit.schedule_identity().into_bytes(),
            unit.toolchain_route_identity().into_bytes(),
        )
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)?,
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module,
        origins: vec![origin],
        obligations,
    })
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV1)
}

fn kernel_parameters_v1() -> Result<Vec<KernelParameterV1>, HandoffDiagnosticV1> {
    let global_i16 = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::I16,
        address_space: AddressSpaceV1::Global,
    };
    let global_f32 = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let readonly_i16 = vec![
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(2),
    ];
    let disjoint_f32 = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::Align(4),
    ];
    [
        ("a", global_i16, readonly_i16.clone()),
        (
            "a_len",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("b", global_i16, readonly_i16),
        (
            "b_len",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("c", global_f32, disjoint_f32),
        (
            "c_len",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("m", KernelValueTypeV1::Scalar(ScalarTypeV1::I32), vec![]),
        ("n", KernelValueTypeV1::Scalar(ScalarTypeV1::I32), vec![]),
        ("k", KernelValueTypeV1::Scalar(ScalarTypeV1::I32), vec![]),
        ("lda", KernelValueTypeV1::Scalar(ScalarTypeV1::I32), vec![]),
        ("ldb", KernelValueTypeV1::Scalar(ScalarTypeV1::I32), vec![]),
        ("ldc", KernelValueTypeV1::Scalar(ScalarTypeV1::I32), vec![]),
        (
            "alpha",
            KernelValueTypeV1::Scalar(ScalarTypeV1::F32),
            vec![],
        ),
        ("beta", KernelValueTypeV1::Scalar(ScalarTypeV1::F32), vec![]),
    ]
    .into_iter()
    .map(|(name, value_type, attributes)| KernelParameterV1::new(name, value_type, attributes))
    .collect()
}

#[derive(Clone, Copy)]
struct KernelValues {
    a: ValueIdV2,
    a_len: ValueIdV2,
    b: ValueIdV2,
    b_len: ValueIdV2,
    c: ValueIdV2,
    c_len: ValueIdV2,
    m: ValueIdV2,
    n: ValueIdV2,
    k: ValueIdV2,
    lda: ValueIdV2,
    ldb: ValueIdV2,
    ldc: ValueIdV2,
    alpha: ValueIdV2,
    beta: ValueIdV2,
}

impl KernelValues {
    const fn fixed() -> Self {
        Self {
            a: ValueIdV2::new(1),
            a_len: ValueIdV2::new(2),
            b: ValueIdV2::new(3),
            b_len: ValueIdV2::new(4),
            c: ValueIdV2::new(5),
            c_len: ValueIdV2::new(6),
            m: ValueIdV2::new(7),
            n: ValueIdV2::new(8),
            k: ValueIdV2::new(9),
            lda: ValueIdV2::new(10),
            ldb: ValueIdV2::new(11),
            ldc: ValueIdV2::new(12),
            alpha: ValueIdV2::new(13),
            beta: ValueIdV2::new(14),
        }
    }
}

struct MachineFunctionBuilder {
    evidence: EvidenceV2,
    current_id: BlockIdV2,
    current: Vec<InstructionV2>,
    blocks: Vec<BasicBlockV2>,
    next_block: u32,
    next_value: u32,
}

impl MachineFunctionBuilder {
    fn new(evidence: EvidenceV2) -> Self {
        Self {
            evidence,
            current_id: BlockIdV2::new(0),
            current: Vec::new(),
            blocks: Vec::new(),
            next_block: 1,
            next_value: 15,
        }
    }

    fn block(&mut self) -> BlockIdV2 {
        let id = BlockIdV2::new(self.next_block);
        self.next_block += 1;
        id
    }

    fn reserve(&mut self) -> ValueIdV2 {
        let id = ValueIdV2::new(self.next_value);
        self.next_value += 1;
        id
    }

    fn instruction(&mut self, value_type: ValueTypeV2, kind: InstructionKindV2) -> ValueIdV2 {
        let id = self.reserve();
        self.instruction_with(id, value_type, kind);
        id
    }

    fn instruction_with(
        &mut self,
        id: ValueIdV2,
        value_type: ValueTypeV2,
        kind: InstructionKindV2,
    ) {
        self.current.push(
            InstructionV2::new(
                Some(TypedValueV2::new(id, value_type)),
                kind,
                self.evidence.clone(),
            )
            .expect("closed general GEMM instruction shape is valid"),
        );
    }

    fn void(&mut self, kind: InstructionKindV2) {
        self.current.push(
            InstructionV2::new(None, kind, self.evidence.clone())
                .expect("closed general GEMM void instruction shape is valid"),
        );
    }

    fn finish(&mut self, terminator: TerminatorV2) {
        self.blocks.push(BasicBlockV2::new(
            self.current_id,
            core::mem::take(&mut self.current),
            terminator,
        ));
    }

    fn start(&mut self, block: BlockIdV2) {
        self.current_id = block;
    }

    fn constant(&mut self, scalar_type: ScalarTypeV1, bits: u64) -> ValueIdV2 {
        self.instruction(
            ValueTypeV2::Scalar(scalar_type),
            InstructionKindV2::Constant(
                ScalarConstantV2::new(scalar_type, bits)
                    .expect("closed general GEMM constants fit their scalar type"),
            ),
        )
    }

    fn integer(
        &mut self,
        operation: IntegerBinaryOperationV2,
        left: ValueIdV2,
        right: ValueIdV2,
        scalar_type: ScalarTypeV1,
    ) -> ValueIdV2 {
        self.instruction(
            ValueTypeV2::Scalar(scalar_type),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(operation),
                left,
                right,
            },
        )
    }

    fn float(
        &mut self,
        operation: FloatBinaryOperationV2,
        left: ValueIdV2,
        right: ValueIdV2,
    ) -> ValueIdV2 {
        self.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Float(operation),
                left,
                right,
            },
        )
    }

    fn compare(
        &mut self,
        predicate: ComparePredicateV2,
        left: ValueIdV2,
        right: ValueIdV2,
    ) -> ValueIdV2 {
        self.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::I1),
            InstructionKindV2::Compare {
                predicate,
                left,
                right,
            },
        )
    }

    fn cast(&mut self, operation: CastOperationV2, value: ValueIdV2, to: ValueTypeV2) -> ValueIdV2 {
        self.instruction(
            to,
            InstructionKindV2::Cast {
                operation,
                value,
                to,
            },
        )
    }

    fn and(&mut self, left: ValueIdV2, right: ValueIdV2) -> ValueIdV2 {
        self.integer(IntegerBinaryOperationV2::And, left, right, ScalarTypeV1::I1)
    }

    fn or(&mut self, left: ValueIdV2, right: ValueIdV2) -> ValueIdV2 {
        self.integer(IntegerBinaryOperationV2::Or, left, right, ScalarTypeV1::I1)
    }

    fn extent(
        &mut self,
        rows: ValueIdV2,
        columns: ValueIdV2,
        stride: ValueIdV2,
        zero_i64: ValueIdV2,
        one_i64: ValueIdV2,
    ) -> ValueIdV2 {
        let rows_zero = self.compare(ComparePredicateV2::IntegerEqual, rows, zero_i64);
        let columns_zero = self.compare(ComparePredicateV2::IntegerEqual, columns, zero_i64);
        let empty = self.or(rows_zero, columns_zero);
        let zero_block = self.block();
        let calculated_block = self.block();
        let merge = self.block();
        self.finish(TerminatorV2::ConditionalBranch {
            condition: empty,
            then_block: zero_block,
            else_block: calculated_block,
        });
        self.start(zero_block);
        let zero = self.constant(ScalarTypeV1::I64, 0);
        self.finish(TerminatorV2::Branch(merge));
        self.start(calculated_block);
        let rows_minus_one = self.integer(
            IntegerBinaryOperationV2::Subtract,
            rows,
            one_i64,
            ScalarTypeV1::I64,
        );
        let row_base = self.integer(
            IntegerBinaryOperationV2::Multiply,
            rows_minus_one,
            stride,
            ScalarTypeV1::I64,
        );
        let calculated = self.integer(
            IntegerBinaryOperationV2::Add,
            row_base,
            columns,
            ScalarTypeV1::I64,
        );
        self.finish(TerminatorV2::Branch(merge));
        self.start(merge);
        self.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::I64),
            InstructionKindV2::Phi {
                incoming: vec![(zero, zero_block), (calculated, calculated_block)],
            },
        )
    }

    fn guarded_i16_load(
        &mut self,
        base: ValueIdV2,
        index: ValueIdV2,
        predicate: ValueIdV2,
    ) -> ValueIdV2 {
        let load_block = self.block();
        let zero_block = self.block();
        let merge = self.block();
        self.finish(TerminatorV2::ConditionalBranch {
            condition: predicate,
            then_block: load_block,
            else_block: zero_block,
        });
        self.start(load_block);
        let pointer = self.instruction(
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::I16,
                address_space: AddressSpaceV1::Global,
            },
            InstructionKindV2::GetElementPtr {
                base,
                indices: vec![index],
            },
        );
        let loaded = self.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::I16),
            InstructionKindV2::Load {
                pointer,
                value_type: ScalarTypeV1::I16,
                alignment: 2,
            },
        );
        self.finish(TerminatorV2::Branch(merge));
        self.start(zero_block);
        let zero = self.constant(ScalarTypeV1::I16, 0);
        self.finish(TerminatorV2::Branch(merge));
        self.start(merge);
        self.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::I16),
            InstructionKindV2::Phi {
                incoming: vec![(loaded, load_block), (zero, zero_block)],
            },
        )
    }

    fn vector_from_i16(&mut self, values: [ValueIdV2; 4], indices: [ValueIdV2; 4]) -> ValueIdV2 {
        let vector_type = ValueTypeV2::fixed_vector(ScalarTypeV1::I16);
        let mut vector = self.instruction(
            vector_type,
            InstructionKindV2::VectorZero {
                element_type: ScalarTypeV1::I16,
            },
        );
        for (value, index) in values.into_iter().zip(indices) {
            vector = self.instruction(
                vector_type,
                InstructionKindV2::InsertElement {
                    vector,
                    element: value,
                    index,
                },
            );
        }
        vector
    }
}

fn build_kernel_function(
    base: &Gfx942HandoffV1,
    schedule: GeneralGemmScheduleV1,
    evidence: EvidenceV2,
) -> Result<FunctionV2, GeneralGemmStructuralMachineErrorV1> {
    let values = KernelValues::fixed();
    let mut builder = MachineFunctionBuilder::new(evidence.clone());
    let i32_type = ValueTypeV2::Scalar(ScalarTypeV1::I32);
    let i64_type = ValueTypeV2::Scalar(ScalarTypeV1::I64);
    let i16_type = ValueTypeV2::Scalar(ScalarTypeV1::I16);
    let f32x4_type = ValueTypeV2::fixed_vector(ScalarTypeV1::F32);
    let i16x4_type = ValueTypeV2::fixed_vector(ScalarTypeV1::I16);

    let zero_i32 = builder.constant(ScalarTypeV1::I32, 0);
    let one_i32 = builder.constant(ScalarTypeV1::I32, 1);
    let two_i32 = builder.constant(ScalarTypeV1::I32, 2);
    let three_i32 = builder.constant(ScalarTypeV1::I32, 3);
    let four_i32 = builder.constant(ScalarTypeV1::I32, 4);
    let seven_i64 = builder.constant(ScalarTypeV1::I64, 7);
    let fifteen_i32 = builder.constant(ScalarTypeV1::I32, 15);
    let sixteen_i32 = builder.constant(ScalarTypeV1::I32, 16);
    let sixteen_i64 = builder.constant(ScalarTypeV1::I64, 16);
    let zero_i64 = builder.constant(ScalarTypeV1::I64, 0);
    let one_i64 = builder.constant(ScalarTypeV1::I64, 1);
    let component_indices = [zero_i32, one_i32, two_i32, three_i32];

    let m64 = builder.cast(CastOperationV2::ZeroExtend, values.m, i64_type);
    let n64 = builder.cast(CastOperationV2::ZeroExtend, values.n, i64_type);
    let k64 = builder.cast(CastOperationV2::ZeroExtend, values.k, i64_type);
    let lda64 = builder.cast(CastOperationV2::ZeroExtend, values.lda, i64_type);
    let ldb64 = builder.cast(CastOperationV2::ZeroExtend, values.ldb, i64_type);
    let ldc64 = builder.cast(CastOperationV2::ZeroExtend, values.ldc, i64_type);
    let a_extent = builder.extent(m64, k64, lda64, zero_i64, one_i64);
    let b_extent = builder.extent(k64, n64, ldb64, zero_i64, one_i64);
    let c_extent = builder.extent(m64, n64, ldc64, zero_i64, one_i64);

    let m_nonzero = builder.compare(ComparePredicateV2::IntegerNotEqual, values.m, zero_i32);
    let n_nonzero = builder.compare(ComparePredicateV2::IntegerNotEqual, values.n, zero_i32);
    let k_nonzero = builder.compare(ComparePredicateV2::IntegerNotEqual, values.k, zero_i32);
    let lda_short = builder.compare(ComparePredicateV2::UnsignedLessThan, values.lda, values.k);
    let ldb_short = builder.compare(ComparePredicateV2::UnsignedLessThan, values.ldb, values.n);
    let ldc_short = builder.compare(ComparePredicateV2::UnsignedLessThan, values.ldc, values.n);
    let mk = builder.and(m_nonzero, k_nonzero);
    let kn = builder.and(k_nonzero, n_nonzero);
    let mn = builder.and(m_nonzero, n_nonzero);
    let bad_lda = builder.and(mk, lda_short);
    let bad_ldb = builder.and(kn, ldb_short);
    let bad_ldc = builder.and(mn, ldc_short);
    let bad_strides_ab = builder.or(bad_lda, bad_ldb);
    let bad_strides = builder.or(bad_strides_ab, bad_ldc);
    let a_short = builder.compare(ComparePredicateV2::UnsignedLessThan, values.a_len, a_extent);
    let b_short = builder.compare(ComparePredicateV2::UnsignedLessThan, values.b_len, b_extent);
    let c_short = builder.compare(ComparePredicateV2::UnsignedLessThan, values.c_len, c_extent);
    let bad_lengths_ab = builder.or(a_short, b_short);
    let bad_lengths = builder.or(bad_lengths_ab, c_short);
    let invalid = builder.or(bad_strides, bad_lengths);
    let trap_block = builder.block();
    let initialize = builder.block();
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: invalid,
        then_block: trap_block,
        else_block: initialize,
    });
    builder.start(trap_block);
    builder.void(InstructionKindV2::Call {
        target: CallTargetV2::Intrinsic(IntrinsicV2::Trap),
        arguments: vec![],
    });
    builder.finish(TerminatorV2::Unreachable);

    builder.start(initialize);
    let lane = builder.instruction(
        i32_type,
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuWorkitemId(AxisV2::X)),
            arguments: vec![],
        },
    );
    let group_x = builder.instruction(
        i32_type,
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuWorkgroupId(AxisV2::X)),
            arguments: vec![],
        },
    );
    let group_y = builder.instruction(
        i32_type,
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuWorkgroupId(AxisV2::Y)),
            arguments: vec![],
        },
    );
    let lane_column = builder.integer(
        IntegerBinaryOperationV2::And,
        lane,
        fifteen_i32,
        ScalarTypeV1::I32,
    );
    let lane_quad = builder.integer(
        IntegerBinaryOperationV2::LogicalShiftRight,
        lane,
        four_i32,
        ScalarTypeV1::I32,
    );
    let lane_depth = builder.integer(
        IntegerBinaryOperationV2::ShiftLeft,
        lane_quad,
        two_i32,
        ScalarTypeV1::I32,
    );
    let lane_column64 = builder.cast(CastOperationV2::ZeroExtend, lane_column, i64_type);
    let lane_depth64 = builder.cast(CastOperationV2::ZeroExtend, lane_depth, i64_type);
    let group_x64 = builder.cast(CastOperationV2::ZeroExtend, group_x, i64_type);
    let group_y64 = builder.cast(CastOperationV2::ZeroExtend, group_y, i64_type);
    let tile_column = builder.integer(
        IntegerBinaryOperationV2::Multiply,
        group_x64,
        sixteen_i64,
        ScalarTypeV1::I64,
    );
    let tile_row = builder.integer(
        IntegerBinaryOperationV2::Multiply,
        group_y64,
        sixteen_i64,
        ScalarTypeV1::I64,
    );
    let a_row = builder.integer(
        IntegerBinaryOperationV2::Add,
        tile_row,
        lane_column64,
        ScalarTypeV1::I64,
    );
    let b_column = builder.integer(
        IntegerBinaryOperationV2::Add,
        tile_column,
        lane_column64,
        ScalarTypeV1::I64,
    );
    let a_row_base = builder.integer(
        IntegerBinaryOperationV2::Multiply,
        a_row,
        lda64,
        ScalarTypeV1::I64,
    );
    let phase_floor = builder.integer(
        IntegerBinaryOperationV2::LogicalShiftRight,
        values.k,
        four_i32,
        ScalarTypeV1::I32,
    );
    let phase_remainder = builder.integer(
        IntegerBinaryOperationV2::And,
        values.k,
        fifteen_i32,
        ScalarTypeV1::I32,
    );
    let has_remainder = builder.compare(
        ComparePredicateV2::IntegerNotEqual,
        phase_remainder,
        zero_i32,
    );
    let remainder_increment = builder.cast(CastOperationV2::ZeroExtend, has_remainder, i32_type);
    let phases = builder.integer(
        IntegerBinaryOperationV2::Add,
        phase_floor,
        remainder_increment,
        ScalarTypeV1::I32,
    );
    let lds_a = builder.instruction(
        ValueTypeV2::ArrayPointer {
            element: ScalarTypeV1::I16,
            elements: GENERAL_GEMM_KIR_LDS_ELEMENTS_V1 as u16,
            address_space: AddressSpaceV1::Local,
        },
        InstructionKindV2::GlobalAddress(GlobalIdV2::new(GLOBAL_LDS_A)),
    );
    let lds_b = builder.instruction(
        ValueTypeV2::ArrayPointer {
            element: ScalarTypeV1::I16,
            elements: GENERAL_GEMM_KIR_LDS_ELEMENTS_V1 as u16,
            address_space: AddressSpaceV1::Local,
        },
        InstructionKindV2::GlobalAddress(GlobalIdV2::new(GLOBAL_LDS_B)),
    );
    let mut lds_indices = Vec::with_capacity(4);
    let mut lds_pointers_a = Vec::with_capacity(4);
    let mut lds_pointers_b = Vec::with_capacity(4);
    for component in component_indices {
        let column = builder.integer(
            IntegerBinaryOperationV2::Add,
            lane_depth,
            component,
            ScalarTypeV1::I32,
        );
        let row_low = builder.integer(
            IntegerBinaryOperationV2::And,
            lane_column,
            three_i32,
            ScalarTypeV1::I32,
        );
        let xor_shift = builder.integer(
            IntegerBinaryOperationV2::ShiftLeft,
            row_low,
            two_i32,
            ScalarTypeV1::I32,
        );
        let swizzled = builder.integer(
            IntegerBinaryOperationV2::Xor,
            column,
            xor_shift,
            ScalarTypeV1::I32,
        );
        let row_base = builder.integer(
            IntegerBinaryOperationV2::Multiply,
            lane_column,
            sixteen_i32,
            ScalarTypeV1::I32,
        );
        let index = builder.integer(
            IntegerBinaryOperationV2::Add,
            row_base,
            swizzled,
            ScalarTypeV1::I32,
        );
        let index64 = builder.cast(CastOperationV2::ZeroExtend, index, i64_type);
        let pointer_a = builder.instruction(
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::I16,
                address_space: AddressSpaceV1::Local,
            },
            InstructionKindV2::GetElementPtr {
                base: lds_a,
                indices: vec![zero_i64, index64],
            },
        );
        let pointer_b = builder.instruction(
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::I16,
                address_space: AddressSpaceV1::Local,
            },
            InstructionKindV2::GetElementPtr {
                base: lds_b,
                indices: vec![zero_i64, index64],
            },
        );
        lds_indices.push(index64);
        lds_pointers_a.push(pointer_a);
        lds_pointers_b.push(pointer_b);
    }
    let accumulator_zero = builder.instruction(
        f32x4_type,
        InstructionKindV2::VectorZero {
            element_type: ScalarTypeV1::F32,
        },
    );
    let initial_block = builder.current_id;
    let header = builder.block();
    let phase_body = builder.block();
    let epilogue = builder.block();
    let backedge = builder.block();
    let phase_next_reserved = builder.reserve();
    let accumulator_next_reserved = builder.reserve();
    builder.finish(TerminatorV2::Branch(header));

    builder.start(header);
    let phase = builder.instruction(
        i32_type,
        InstructionKindV2::Phi {
            incoming: vec![(zero_i32, initial_block), (phase_next_reserved, backedge)],
        },
    );
    let accumulator = builder.instruction(
        f32x4_type,
        InstructionKindV2::Phi {
            incoming: vec![
                (accumulator_zero, initial_block),
                (accumulator_next_reserved, backedge),
            ],
        },
    );
    let phase_active = builder.compare(ComparePredicateV2::UnsignedLessThan, phase, phases);
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: phase_active,
        then_block: phase_body,
        else_block: epilogue,
    });

    builder.start(phase_body);
    let phase64 = builder.cast(CastOperationV2::ZeroExtend, phase, i64_type);
    let phase_depth = builder.integer(
        IntegerBinaryOperationV2::Multiply,
        phase64,
        sixteen_i64,
        ScalarTypeV1::I64,
    );
    let depth_base = builder.integer(
        IntegerBinaryOperationV2::Add,
        phase_depth,
        lane_depth64,
        ScalarTypeV1::I64,
    );
    let a_row_valid = builder.compare(ComparePredicateV2::UnsignedLessThan, a_row, m64);
    let b_column_valid = builder.compare(ComparePredicateV2::UnsignedLessThan, b_column, n64);
    let mut a_indices = Vec::with_capacity(4);
    let mut b_indices = Vec::with_capacity(4);
    let mut a_predicates = Vec::with_capacity(4);
    let mut b_predicates = Vec::with_capacity(4);
    for component in component_indices {
        let component64 = builder.cast(CastOperationV2::ZeroExtend, component, i64_type);
        let depth = builder.integer(
            IntegerBinaryOperationV2::Add,
            depth_base,
            component64,
            ScalarTypeV1::I64,
        );
        let depth_valid = builder.compare(ComparePredicateV2::UnsignedLessThan, depth, k64);
        let a_predicate = builder.and(a_row_valid, depth_valid);
        let b_predicate = builder.and(depth_valid, b_column_valid);
        let a_index = builder.integer(
            IntegerBinaryOperationV2::Add,
            a_row_base,
            depth,
            ScalarTypeV1::I64,
        );
        let b_row_base = builder.integer(
            IntegerBinaryOperationV2::Multiply,
            depth,
            ldb64,
            ScalarTypeV1::I64,
        );
        let b_index = builder.integer(
            IntegerBinaryOperationV2::Add,
            b_row_base,
            b_column,
            ScalarTypeV1::I64,
        );
        a_indices.push(a_index);
        b_indices.push(b_index);
        a_predicates.push(a_predicate);
        b_predicates.push(b_predicate);
    }
    let a_indices: [ValueIdV2; 4] = a_indices.try_into().expect("four A indices");
    let b_indices: [ValueIdV2; 4] = b_indices.try_into().expect("four B indices");
    let a_predicates: [ValueIdV2; 4] = a_predicates.try_into().expect("four A predicates");
    let b_predicates: [ValueIdV2; 4] = b_predicates.try_into().expect("four B predicates");

    let a_vector = match schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
            let a_values = a_indices
                .into_iter()
                .zip(a_predicates)
                .map(|(index, predicate)| builder.guarded_i16_load(values.a, index, predicate))
                .collect::<Vec<_>>()
                .try_into()
                .expect("four scalar A values");
            builder.vector_from_i16(a_values, component_indices)
        }
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
            let first_pointer = builder.instruction(
                ValueTypeV2::Pointer {
                    pointee: ScalarTypeV1::I16,
                    address_space: AddressSpaceV1::Global,
                },
                InstructionKindV2::GetElementPtr {
                    base: values.a,
                    indices: vec![a_indices[0]],
                },
            );
            let address = builder.cast(CastOperationV2::PointerToInt, first_pointer, i64_type);
            let low_bits = builder.integer(
                IntegerBinaryOperationV2::And,
                address,
                seven_i64,
                ScalarTypeV1::I64,
            );
            let aligned = builder.compare(ComparePredicateV2::IntegerEqual, low_bits, zero_i64);
            let full_01 = builder.and(a_predicates[0], a_predicates[1]);
            let full_23 = builder.and(a_predicates[2], a_predicates[3]);
            let full = builder.and(full_01, full_23);
            let vectorized = builder.and(full, aligned);
            let vector_block = builder.block();
            let scalar_block = builder.block();
            let vector_merge = builder.block();
            builder.finish(TerminatorV2::ConditionalBranch {
                condition: vectorized,
                then_block: vector_block,
                else_block: scalar_block,
            });
            builder.start(vector_block);
            let loaded = builder.instruction(
                i16x4_type,
                InstructionKindV2::VectorLoad4 {
                    pointer: first_pointer,
                    element_type: ScalarTypeV1::I16,
                    alignment: 8,
                },
            );
            builder.finish(TerminatorV2::Branch(vector_merge));
            builder.start(scalar_block);
            let scalar_values = a_indices
                .into_iter()
                .zip(a_predicates)
                .map(|(index, predicate)| builder.guarded_i16_load(values.a, index, predicate))
                .collect::<Vec<_>>()
                .try_into()
                .expect("four scalar A fallback values");
            let scalar_vector = builder.vector_from_i16(scalar_values, component_indices);
            let scalar_predecessor = builder.current_id;
            builder.finish(TerminatorV2::Branch(vector_merge));
            builder.start(vector_merge);
            builder.instruction(
                i16x4_type,
                InstructionKindV2::Phi {
                    incoming: vec![(loaded, vector_block), (scalar_vector, scalar_predecessor)],
                },
            )
        }
    };
    let b_values = b_indices
        .into_iter()
        .zip(b_predicates)
        .map(|(index, predicate)| builder.guarded_i16_load(values.b, index, predicate))
        .collect::<Vec<_>>()
        .try_into()
        .expect("four scalar B values");
    let b_vector = builder.vector_from_i16(b_values, component_indices);

    for (component, (pointer_a, pointer_b)) in component_indices
        .into_iter()
        .zip(lds_pointers_a.into_iter().zip(lds_pointers_b))
    {
        let a_value = builder.instruction(
            i16_type,
            InstructionKindV2::ExtractElement {
                vector: a_vector,
                index: component,
            },
        );
        let b_value = builder.instruction(
            i16_type,
            InstructionKindV2::ExtractElement {
                vector: b_vector,
                index: component,
            },
        );
        builder.void(InstructionKindV2::Store {
            pointer: pointer_a,
            value: a_value,
            value_type: ScalarTypeV1::I16,
            alignment: 2,
        });
        builder.void(InstructionKindV2::Store {
            pointer: pointer_b,
            value: b_value,
            value_type: ScalarTypeV1::I16,
            alignment: 2,
        });
    }
    builder.void(InstructionKindV2::Call {
        target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuBarrier),
        arguments: vec![],
    });
    let mut staged_a = Vec::with_capacity(4);
    let mut staged_b = Vec::with_capacity(4);
    for index in lds_indices {
        let pointer_type = ValueTypeV2::Pointer {
            pointee: ScalarTypeV1::I16,
            address_space: AddressSpaceV1::Local,
        };
        let pointer_a = builder.instruction(
            pointer_type,
            InstructionKindV2::GetElementPtr {
                base: lds_a,
                indices: vec![zero_i64, index],
            },
        );
        let pointer_b = builder.instruction(
            pointer_type,
            InstructionKindV2::GetElementPtr {
                base: lds_b,
                indices: vec![zero_i64, index],
            },
        );
        staged_a.push(builder.instruction(
            i16_type,
            InstructionKindV2::Load {
                pointer: pointer_a,
                value_type: ScalarTypeV1::I16,
                alignment: 2,
            },
        ));
        staged_b.push(builder.instruction(
            i16_type,
            InstructionKindV2::Load {
                pointer: pointer_b,
                value_type: ScalarTypeV1::I16,
                alignment: 2,
            },
        ));
    }
    let staged_a = builder.vector_from_i16(
        staged_a.try_into().expect("four staged A values"),
        component_indices,
    );
    let staged_b = builder.vector_from_i16(
        staged_b.try_into().expect("four staged B values"),
        component_indices,
    );
    builder.instruction_with(
        accumulator_next_reserved,
        f32x4_type,
        InstructionKindV2::Call {
            target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k),
            arguments: vec![
                staged_a,
                staged_b,
                accumulator,
                zero_i32,
                zero_i32,
                zero_i32,
            ],
        },
    );
    builder.void(InstructionKindV2::Call {
        target: CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuBarrier),
        arguments: vec![],
    });
    builder.instruction_with(
        phase_next_reserved,
        i32_type,
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
            left: phase,
            right: one_i32,
        },
    );
    builder.finish(TerminatorV2::Branch(backedge));
    builder.start(backedge);
    builder.finish(TerminatorV2::Branch(header));

    builder.start(epilogue);
    for component in component_indices {
        let component64 = builder.cast(CastOperationV2::ZeroExtend, component, i64_type);
        let row_offset = builder.integer(
            IntegerBinaryOperationV2::Add,
            lane_depth64,
            component64,
            ScalarTypeV1::I64,
        );
        let row = builder.integer(
            IntegerBinaryOperationV2::Add,
            tile_row,
            row_offset,
            ScalarTypeV1::I64,
        );
        let row_valid = builder.compare(ComparePredicateV2::UnsignedLessThan, row, m64);
        let column_valid = builder.compare(ComparePredicateV2::UnsignedLessThan, b_column, n64);
        let valid = builder.and(row_valid, column_valid);
        let row_base = builder.integer(
            IntegerBinaryOperationV2::Multiply,
            row,
            ldc64,
            ScalarTypeV1::I64,
        );
        let index = builder.integer(
            IntegerBinaryOperationV2::Add,
            row_base,
            b_column,
            ScalarTypeV1::I64,
        );
        let store_block = builder.block();
        let skip_block = builder.block();
        let merge = builder.block();
        builder.finish(TerminatorV2::ConditionalBranch {
            condition: valid,
            then_block: store_block,
            else_block: skip_block,
        });
        builder.start(store_block);
        let pointer = builder.instruction(
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: AddressSpaceV1::Global,
            },
            InstructionKindV2::GetElementPtr {
                base: values.c,
                indices: vec![index],
            },
        );
        let prior = builder.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
            InstructionKindV2::Load {
                pointer,
                value_type: ScalarTypeV1::F32,
                alignment: 4,
            },
        );
        let accumulated = builder.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::F32),
            InstructionKindV2::ExtractElement {
                vector: accumulator,
                index: component,
            },
        );
        let scaled_accumulator =
            builder.float(FloatBinaryOperationV2::Multiply, values.alpha, accumulated);
        let scaled_prior = builder.float(FloatBinaryOperationV2::Multiply, values.beta, prior);
        let output = builder.float(
            FloatBinaryOperationV2::Add,
            scaled_accumulator,
            scaled_prior,
        );
        builder.void(InstructionKindV2::Store {
            pointer,
            value: output,
            value_type: ScalarTypeV1::F32,
            alignment: 4,
        });
        builder.finish(TerminatorV2::Branch(merge));
        builder.start(skip_block);
        builder.finish(TerminatorV2::Branch(merge));
        builder.start(merge);
    }
    builder.finish(TerminatorV2::Return(None));

    let kernel = &base.kernels()[0];
    let parameters = kernel
        .parameters()
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            FunctionParameterV2::new(
                TypedValueV2::new(
                    ValueIdV2::new(index as u32 + 1),
                    parameter.value_type().into(),
                ),
                parameter.name(),
                parameter.attributes().to_vec(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)?;
    let mut attributes = kernel
        .function_attributes()
        .iter()
        .copied()
        .map(FunctionAttributeV2::from)
        .collect::<Vec<_>>();
    attributes.push(FunctionAttributeV2::RequiredWorkgroupSize([64, 1, 1]));
    FunctionV2::new(
        FunctionIdV2::new(KERNEL_FUNCTION),
        GENERAL_GEMM_KERNEL_SYMBOL_V1,
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        parameters,
        attributes,
        BlockIdV2::new(0),
        builder.blocks,
        evidence,
    )
    .map_err(GeneralGemmStructuralMachineErrorV1::HandoffV2)
}
