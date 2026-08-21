#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use core::fmt;

use fe2o3_amdhsa_loader::{AdmittedProfile, LoadPlan, PlanError};
use fe2o3_artifact_transaction::{
    CompilerModuleHandoffIdentityV1, ConsumedCompilerModuleHandoffV1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2, CompilerModuleHandoffV2,
    CompilerModuleKindV1, CompilerModuleSymbolManifestErrorV1,
    CompilerModuleSymbolManifestIdentityV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion as InspectedCodeObjectVersion,
    ExplicitArgument, ExplicitValueKind, ExplicitValueType, HiddenArgument, HiddenValueKind,
    KernelBindingError, inspect_and_bind_kernel_descriptors,
};
use fe2o3_hsaco_finalize::{
    CompilerHandoffWorkerRequestV2, ContentIdentityV1, InertDecodedWorkerExchangeV2,
    InertFirstBuildWorkerV2EvidenceV1, InspectedRawWorkerV2HsacoV1, LinkInputKindClosureV1,
    LinkPlanIdentityV1, MultiInputLinkPlanV1, PinnedWorkerV1, WorkerInputKindV1,
    WorkerMeasurementV1, WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1,
    WorkerProtocolError, WorkerRequestConstructionError, WorkerRequestV2,
    construct_worker_request_v2_from_consumed_handoff, inspect_general_gemm_worker_v2_raw_hsaco_v1,
};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallingConventionV2,
    EvidenceV2, ExecutableModuleV2, FunctionAttributeV1, FunctionAttributeV2, FunctionIdV2,
    FunctionKindV2, FunctionParameterV2, FunctionV2, Gfx942HandoffInputV1, Gfx942HandoffV1,
    Gfx942HandoffV2, Gfx942TargetPolicyV1, HandoffDiagnosticV1, HandoffDiagnosticV2,
    HandoffIdentityV2, IdentityV1, InstructionKindV2, InstructionV2, IntegerBinaryOperationV2,
    IntrinsicReferenceV2, IntrinsicV2, KernelEntryV1, KernelParameterV1, KernelValueTypeV1,
    ModuleFlagV1, ModuleMetadataV1, ObligationKindV1, ObligationV1, OriginKindV1, OriginV1,
    ParameterAttributeV1, ReturnTypeV2, ScalarConstantV2, ScalarTypeV1, StageIdentitiesV1,
    TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2, WorkgroupSizeRangeV1,
};
use fe2o3_llvm_text::{LlvmAssemblySha256V2, SerializeErrorV2, serialize_gfx942_handoff_v2};
use fe2o3_llvm_worker_handoff::{
    MeasuredLlvmLldBuildV1, WorkerAdmissionErrorV2, WorkerAdmissionIdentityV2,
    WorkerAdmissionRequestV2,
};
use sha2::{Digest as _, Sha256};

/// Exact compiler and metadata kernel entry.
pub const BF16_GEMM_KERNEL_SYMBOL_V1: &str = "bf16_gemm_16x16x16_v1";
/// Exact AMDHSA descriptor symbol derived for the kernel.
pub const BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1: &str = "bf16_gemm_16x16x16_v1.kd";
/// Only target admitted by this profile.
pub const BF16_GEMM_KERNEL_TARGET_V1: &str = "gfx942:xnack-";
/// Fixed wave64 workgroup width.
pub const BF16_GEMM_KERNEL_WORKGROUP_X_V1: u32 = 64;
/// Fixed row count of A and C.
pub const BF16_GEMM_M_V1: u32 = 16;
/// Fixed column count of B and C.
pub const BF16_GEMM_N_V1: u32 = 16;
/// Fixed reduction count.
pub const BF16_GEMM_K_V1: u32 = 16;
/// Exact bytes read from row-major BF16 A.
pub const BF16_GEMM_A_BYTES_V1: u64 = 16 * 16 * 2;
/// Exact bytes read from row-major BF16 B.
pub const BF16_GEMM_B_BYTES_V1: u64 = 16 * 16 * 2;
/// Exact bytes written to row-major FP32 C.
pub const BF16_GEMM_C_BYTES_V1: u64 = 16 * 16 * 4;
/// Three explicit 64-bit fields followed by the COV6 hidden block.
pub const BF16_GEMM_KERNEL_KERNARG_BYTES_V1: u64 = 24 + 256;
/// Alignment required by the exact explicit ABI.
pub const BF16_GEMM_KERNEL_KERNARG_ALIGNMENT_V1: u64 = 8;

const A_NAME: &str = "a_bf16";
const B_NAME: &str = "b_bf16";
const C_NAME: &str = "c_f32";
const REQUIRED_OBLIGATIONS: [ObligationKindV1; 7] = [
    ObligationKindV1::PreserveKernelAbi,
    ObligationKindV1::PreserveAddressSpaces,
    ObligationKindV1::PreserveTargetFeatures,
    ObligationKindV1::PreserveCallingConvention,
    ObligationKindV1::PreserveFunctionAttributes,
    ObligationKindV1::PreserveModuleMetadata,
    ObligationKindV1::MaintainOriginCoverage,
];

/// One of the three exact GEMM buffers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bf16GemmBufferV1 {
    /// Row-major BF16 A.
    A,
    /// Row-major BF16 B.
    B,
    /// Row-major FP32 C.
    C,
}

/// Exact numerical launch shape for the one-tile slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bf16GemmKernelDispatchShapeV1;

impl Bf16GemmKernelDispatchShapeV1 {
    /// Returns the only admitted launch shape.
    pub const fn exact() -> Self {
        Self
    }

    /// Exact logical matrix dimensions `[M, N, K]`.
    pub const fn matrix_dimensions(self) -> [u32; 3] {
        [BF16_GEMM_M_V1, BF16_GEMM_N_V1, BF16_GEMM_K_V1]
    }

    /// Exact AQL grid in workitems.
    pub const fn grid(self) -> [u32; 3] {
        [BF16_GEMM_KERNEL_WORKGROUP_X_V1, 1, 1]
    }

    /// Exact required workgroup dimensions.
    pub const fn workgroup(self) -> [u32; 3] {
        [BF16_GEMM_KERNEL_WORKGROUP_X_V1, 1, 1]
    }

    /// Block counts expected by `HsaLaunchGeometryV1` before workgroup expansion.
    pub const fn hsa_adapter_block_counts(self) -> [u32; 3] {
        [1, 1, 1]
    }

    /// Checks that adapter block counts expand to the exact AQL workitem grid.
    pub fn hsa_block_counts_expand_to_aql_grid(self, blocks: [u32; 3]) -> bool {
        let workgroup = self.workgroup();
        let Some(x) = blocks[0].checked_mul(workgroup[0]) else {
            return false;
        };
        let Some(y) = blocks[1].checked_mul(workgroup[1]) else {
            return false;
        };
        let Some(z) = blocks[2].checked_mul(workgroup[2]) else {
            return false;
        };
        [x, y, z] == self.grid()
    }

    /// A numerical shape does not prove leases, pack kernargs, or grant launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Numerical buffer-contract rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Bf16GemmBufferContractErrorV1 {
    /// A raw numerical address was zero.
    ZeroAddress(Bf16GemmBufferV1),
    /// The available span differed from the exact closed profile.
    ByteLength(Bf16GemmBufferV1),
    /// The numerical address did not meet the load/store alignment.
    Alignment(Bf16GemmBufferV1),
    /// A half-open address range overflowed `u64`.
    RangeOverflow(Bf16GemmBufferV1),
    /// Two declared buffer ranges overlap.
    Aliasing,
}

impl fmt::Display for Bf16GemmBufferContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "BF16 GEMM numerical buffer contract failed: {self:?}"
        )
    }
}

impl std::error::Error for Bf16GemmBufferContractErrorV1 {}

/// Inert checked numerical address layout for the exact three-buffer ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bf16GemmBufferContractV1 {
    addresses: [u64; 3],
    ends: [u64; 3],
}

impl Bf16GemmBufferContractV1 {
    /// Checks exact byte spans, required alignment, overflow, and pairwise disjointness.
    pub fn new(
        a_address: u64,
        a_bytes: u64,
        b_address: u64,
        b_bytes: u64,
        c_address: u64,
        c_bytes: u64,
    ) -> Result<Self, Bf16GemmBufferContractErrorV1> {
        let addresses = [a_address, b_address, c_address];
        let lengths = [a_bytes, b_bytes, c_bytes];
        let exact_lengths = [
            BF16_GEMM_A_BYTES_V1,
            BF16_GEMM_B_BYTES_V1,
            BF16_GEMM_C_BYTES_V1,
        ];
        let alignments = [8, 2, 4];
        let buffers = [
            Bf16GemmBufferV1::A,
            Bf16GemmBufferV1::B,
            Bf16GemmBufferV1::C,
        ];
        let mut ends = [0; 3];
        for index in 0..3 {
            if addresses[index] == 0 {
                return Err(Bf16GemmBufferContractErrorV1::ZeroAddress(buffers[index]));
            }
            if lengths[index] != exact_lengths[index] {
                return Err(Bf16GemmBufferContractErrorV1::ByteLength(buffers[index]));
            }
            if !addresses[index].is_multiple_of(alignments[index]) {
                return Err(Bf16GemmBufferContractErrorV1::Alignment(buffers[index]));
            }
            ends[index] = addresses[index]
                .checked_add(lengths[index])
                .ok_or(Bf16GemmBufferContractErrorV1::RangeOverflow(buffers[index]))?;
        }
        for left in 0..3 {
            for right in left + 1..3 {
                if addresses[left] < ends[right] && addresses[right] < ends[left] {
                    return Err(Bf16GemmBufferContractErrorV1::Aliasing);
                }
            }
        }
        Ok(Self { addresses, ends })
    }

    /// Exact checked starts in ABI order A, B, C.
    pub const fn addresses(self) -> [u64; 3] {
        self.addresses
    }

    /// Exact checked exclusive ends in ABI order A, B, C.
    pub const fn ends(self) -> [u64; 3] {
        self.ends
    }

    /// Integer observations do not prove mappings, leases, or initialized content.
    pub const fn authenticates_device_memory(self) -> bool {
        false
    }

    /// A numerical buffer check cannot grant launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Nonzero identities from the compiler stages preceding this exact profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bf16GemmKernelSourceBindingsV1 {
    source: [u8; 32],
    semantic: [u8; 32],
    schedule: [u8; 32],
    target_plan: [u8; 32],
}

impl Bf16GemmKernelSourceBindingsV1 {
    /// Constructs inert source/stage bindings. Zero identities fail during preparation.
    pub const fn new(
        source: [u8; 32],
        semantic: [u8; 32],
        schedule: [u8; 32],
        target_plan: [u8; 32],
    ) -> Self {
        Self {
            source,
            semantic,
            schedule,
            target_plan,
        }
    }

    /// Caller-supplied identities are inert provenance labels, not authentication.
    pub const fn authenticates_provenance(self) -> bool {
        false
    }
}

/// Failure to construct and admit the exact typed BF16 GEMM semantics.
#[derive(Debug)]
#[non_exhaustive]
pub enum PrepareBf16GemmKernelErrorV1 {
    /// Caller-supplied inert provenance reused one identity for multiple roles.
    NonDistinctSourceBindings,
    /// A V1 policy object rejected the source bindings or ABI.
    HandoffV1(HandoffDiagnosticV1),
    /// The executable graph rejected an instruction, CFG, or evidence edge.
    HandoffV2(HandoffDiagnosticV2),
    /// Exact LLVM/LLD 22.1.8 policy admission failed.
    WorkerAdmission(WorkerAdmissionErrorV2),
    /// Canonical LLVM serialization failed.
    Serialize(SerializeErrorV2),
    /// The serialized bytes did not retain the exact handoff identity marker.
    MissingSourceIdentity,
    /// The compiler-FFI envelope could not be constructed.
    CompilerEnvelope(CompilerFfiEnvelopeError),
    /// The closed two-symbol manifest could not be constructed.
    SymbolManifest(CompilerModuleSymbolManifestErrorV1),
    /// The canonical compiler handoff rejected the prepared module.
    CompilerHandoff(CompilerModuleHandoffErrorV2),
}

impl fmt::Display for PrepareBf16GemmKernelErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonDistinctSourceBindings => formatter.write_str(
                "BF16 GEMM source, semantic, schedule, and target-plan identities must be pairwise distinct",
            ),
            Self::HandoffV1(error) => {
                write!(formatter, "BF16 GEMM policy construction failed: {error}")
            }
            Self::HandoffV2(error) => {
                write!(formatter, "BF16 GEMM graph construction failed: {error}")
            }
            Self::WorkerAdmission(error) => {
                write!(formatter, "BF16 GEMM worker admission failed: {error}")
            }
            Self::Serialize(error) => {
                write!(formatter, "BF16 GEMM LLVM serialization failed: {error}")
            }
            Self::MissingSourceIdentity => {
                formatter.write_str("BF16 GEMM LLVM omitted its exact handoff identity")
            }
            Self::CompilerEnvelope(error) => {
                write!(formatter, "BF16 GEMM compiler envelope failed: {error}")
            }
            Self::SymbolManifest(error) => {
                write!(formatter, "BF16 GEMM symbol manifest failed: {error}")
            }
            Self::CompilerHandoff(error) => {
                write!(formatter, "BF16 GEMM compiler handoff failed: {error}")
            }
        }
    }
}

impl std::error::Error for PrepareBf16GemmKernelErrorV1 {}

/// Inert exact BF16 GEMM semantics and compiler handoff.
#[derive(Debug)]
pub struct PreparedBf16GemmKernelV1 {
    source_identity: HandoffIdentityV2,
    worker_admission_identity: WorkerAdmissionIdentityV2,
    assembly_sha256: LlvmAssemblySha256V2,
    assembly_len: u64,
    compiler_handoff_identity: CompilerModuleHandoffIdentityV2,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    compiler_handoff: CompilerModuleHandoffV2,
}

impl PreparedBf16GemmKernelV1 {
    /// Identity of the complete typed source graph and retained policy.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.source_identity
    }

    /// Identity binding the source graph to exact LLVM/LLD 22.1.8 policy.
    pub const fn worker_admission_identity(&self) -> WorkerAdmissionIdentityV2 {
        self.worker_admission_identity
    }

    /// SHA-256 of the canonical LLVM assembly bytes.
    pub const fn assembly_sha256(&self) -> LlvmAssemblySha256V2 {
        self.assembly_sha256
    }

    /// Exact canonical LLVM assembly byte length.
    pub const fn assembly_len(&self) -> u64 {
        self.assembly_len
    }

    /// Content identity used by Worker V2 link planning.
    pub fn assembly_content_identity(&self) -> ContentIdentityV1 {
        ContentIdentityV1::from_parts(*self.assembly_sha256.as_bytes(), self.assembly_len)
    }

    /// Identity of the canonical compiler-FFI handoff.
    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.compiler_handoff_identity
    }

    /// Identity of the closed kernel-entry/descriptor manifest.
    pub const fn manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.manifest_identity
    }

    /// Borrowed canonical handoff for attempt-scoped publication.
    pub const fn compiler_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.compiler_handoff
    }

    /// Preparation authenticates policy, not a worker executable.
    pub const fn authenticates_worker_executable(&self) -> bool {
        false
    }

    /// Preparation does not authenticate the caller-supplied upstream provenance labels.
    pub const fn authenticates_upstream_provenance(&self) -> bool {
        false
    }

    /// Preparation grants no artifact publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Preparation grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Preparation grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Constructs the only admitted BF16 GEMM graph and binds it to exact LLVM/LLD policy.
pub fn prepare_bf16_gemm_kernel_v1(
    bindings: Bf16GemmKernelSourceBindingsV1,
) -> Result<PreparedBf16GemmKernelV1, PrepareBf16GemmKernelErrorV1> {
    let handoff = construct_bf16_gemm_handoff(bindings)?;
    let source_identity = handoff.identity();
    let canonical = handoff.encode_canonical();
    let admitted = WorkerAdmissionRequestV2::new(
        canonical.as_bytes(),
        *source_identity.as_bytes(),
        MeasuredLlvmLldBuildV1::exact(),
    )
    .admit()
    .map_err(PrepareBf16GemmKernelErrorV1::WorkerAdmission)?;
    if admitted.handoff() != &handoff || admitted.handoff_identity() != source_identity {
        return Err(PrepareBf16GemmKernelErrorV1::MissingSourceIdentity);
    }
    let worker_admission_identity = admitted.admission_identity();
    let assembly = serialize_gfx942_handoff_v2(admitted.handoff())
        .map_err(PrepareBf16GemmKernelErrorV1::Serialize)?;
    if assembly.source_identity() != source_identity || !assembly.has_embedded_source_identity() {
        return Err(PrepareBf16GemmKernelErrorV1::MissingSourceIdentity);
    }

    let target = exact_target();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(PrepareBf16GemmKernelErrorV1::CompilerEnvelope)?;
    let manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            BF16_GEMM_KERNEL_SYMBOL_V1,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ),
    ])
    .map_err(PrepareBf16GemmKernelErrorV1::SymbolManifest)?;
    let manifest_identity = manifest.identity();
    let compiler_handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        assembly.as_bytes(),
    )
    .map_err(PrepareBf16GemmKernelErrorV1::CompilerHandoff)?;
    Ok(PreparedBf16GemmKernelV1 {
        source_identity,
        worker_admission_identity,
        assembly_sha256: assembly.sha256(),
        assembly_len: assembly.as_bytes().len() as u64,
        compiler_handoff_identity: compiler_handoff.identity(),
        manifest_identity,
        compiler_handoff,
    })
}

fn construct_bf16_gemm_handoff(
    bindings: Bf16GemmKernelSourceBindingsV1,
) -> Result<Gfx942HandoffV2, PrepareBf16GemmKernelErrorV1> {
    let identities = [
        bindings.source,
        bindings.semantic,
        bindings.schedule,
        bindings.target_plan,
    ];
    if identities
        .iter()
        .enumerate()
        .any(|(index, identity)| identities[index + 1..].contains(identity))
    {
        return Err(PrepareBf16GemmKernelErrorV1::NonDistinctSourceBindings);
    }
    let source =
        IdentityV1::new(bindings.source).map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?;
    let stages = StageIdentitiesV1::new(bindings.semantic, bindings.schedule, bindings.target_plan)
        .map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?;
    let origin = OriginV1::new(OriginKindV1::AmdgcnIr, source, None);
    let bf16_pointer = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::I16,
        address_space: AddressSpaceV1::Global,
    };
    let f32_pointer = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let a_attributes = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(8),
        ParameterAttributeV1::Dereferenceable(BF16_GEMM_A_BYTES_V1 as u32),
    ];
    let b_attributes = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(2),
        ParameterAttributeV1::Dereferenceable(BF16_GEMM_B_BYTES_V1 as u32),
    ];
    let c_attributes = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::WriteOnly,
        ParameterAttributeV1::Align(4),
        ParameterAttributeV1::Dereferenceable(BF16_GEMM_C_BYTES_V1 as u32),
    ];
    let attributes = exact_function_attributes_v1();
    let kernel = KernelEntryV1::new(
        BF16_GEMM_KERNEL_SYMBOL_V1,
        vec![
            KernelParameterV1::new(A_NAME, bf16_pointer, a_attributes.clone())
                .map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?,
            KernelParameterV1::new(B_NAME, bf16_pointer, b_attributes.clone())
                .map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?,
            KernelParameterV1::new(C_NAME, f32_pointer, c_attributes.clone())
                .map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?,
        ],
        attributes.clone(),
        origin.identity(),
    )
    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?;
    let obligations = REQUIRED_OBLIGATIONS
        .into_iter()
        .map(|kind| {
            ObligationV1::new(
                kind,
                match kind {
                    ObligationKindV1::PreserveKernelAbi
                    | ObligationKindV1::MaintainOriginCoverage => stages.semantic(),
                    ObligationKindV1::AuthenticateDeviceLibraries => stages.schedule(),
                    _ => stages.target_plan(),
                },
                origin.identity(),
            )
        })
        .collect();
    let module_metadata = ModuleMetadataV1::new(
        vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2],
        vec![],
        vec![],
    )
    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?;
    let base = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: stages,
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module: module_metadata,
        origins: vec![origin],
        obligations,
    })
    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV1)?;
    let evidence = EvidenceV2::new(
        base.origins()[0].identity(),
        base.obligations()
            .iter()
            .map(|item| item.identity())
            .collect(),
    )
    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?;
    let global_i16 = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::I16,
        address_space: AddressSpaceV1::Global,
    };
    let global_f32 = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::F32,
        address_space: AddressSpaceV1::Global,
    };
    let i32_type = ValueTypeV2::Scalar(ScalarTypeV1::I32);
    let i16_type = ValueTypeV2::Scalar(ScalarTypeV1::I16);
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let i16x4_type = ValueTypeV2::fixed_vector(ScalarTypeV1::I16);
    let f32x4_type = ValueTypeV2::fixed_vector(ScalarTypeV1::F32);
    let parameters = vec![
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(0), global_i16),
            A_NAME,
            a_attributes,
        )
        .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?,
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(1), global_i16),
            B_NAME,
            b_attributes,
        )
        .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?,
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(2), global_f32),
            C_NAME,
            c_attributes,
        )
        .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?,
    ];
    let instruction = |result, kind| {
        InstructionV2::new(result, kind, evidence.clone())
            .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)
    };
    let mut body = vec![
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(3), i32_type)),
            InstructionKindV2::Call {
                target: fe2o3_llvm_handoff::CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuWorkitemId(
                    AxisV2::X,
                )),
                arguments: vec![],
            },
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(4), i32_type)),
            InstructionKindV2::Constant(
                ScalarConstantV2::new(ScalarTypeV1::I32, 15)
                    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?,
            ),
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(5), i32_type)),
            InstructionKindV2::Constant(
                ScalarConstantV2::new(ScalarTypeV1::I32, 4)
                    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?,
            ),
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(6), i32_type)),
            InstructionKindV2::Constant(
                ScalarConstantV2::new(ScalarTypeV1::I32, 2)
                    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?,
            ),
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(7), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::And),
                left: ValueIdV2::new(3),
                right: ValueIdV2::new(4),
            },
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(8), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::LogicalShiftRight),
                left: ValueIdV2::new(3),
                right: ValueIdV2::new(5),
            },
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(9), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::ShiftLeft),
                left: ValueIdV2::new(8),
                right: ValueIdV2::new(6),
            },
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(10), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::ShiftLeft),
                left: ValueIdV2::new(7),
                right: ValueIdV2::new(5),
            },
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(11), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                left: ValueIdV2::new(10),
                right: ValueIdV2::new(9),
            },
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(12), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::ShiftLeft),
                left: ValueIdV2::new(9),
                right: ValueIdV2::new(5),
            },
        )?,
        instruction(
            Some(TypedValueV2::new(ValueIdV2::new(13), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                left: ValueIdV2::new(12),
                right: ValueIdV2::new(7),
            },
        )?,
    ];
    for (id, value) in [
        (14, 0),
        (15, 1),
        (16, 2),
        (17, 3),
        (18, 16),
        (19, 32),
        (20, 48),
    ] {
        body.push(instruction(
            Some(TypedValueV2::new(ValueIdV2::new(id), i32_type)),
            InstructionKindV2::Constant(
                ScalarConstantV2::new(ScalarTypeV1::I32, value)
                    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?,
            ),
        )?);
    }
    for (id, offset_id) in [(21, 18), (22, 19), (23, 20)] {
        body.push(instruction(
            Some(TypedValueV2::new(ValueIdV2::new(id), i32_type)),
            InstructionKindV2::Binary {
                operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                left: ValueIdV2::new(13),
                right: ValueIdV2::new(offset_id),
            },
        )?);
    }
    body.push(instruction(
        Some(TypedValueV2::new(ValueIdV2::new(24), global_i16)),
        InstructionKindV2::GetElementPtr {
            base: ValueIdV2::new(0),
            indices: vec![ValueIdV2::new(11)],
        },
    )?);
    body.push(instruction(
        Some(TypedValueV2::new(ValueIdV2::new(25), i16x4_type)),
        InstructionKindV2::VectorLoad4 {
            pointer: ValueIdV2::new(24),
            element_type: ScalarTypeV1::I16,
            alignment: 8,
        },
    )?);
    for (pointer_id, load_id, index_id) in [(26, 30, 13), (27, 31, 21), (28, 32, 22), (29, 33, 23)]
    {
        body.push(instruction(
            Some(TypedValueV2::new(ValueIdV2::new(pointer_id), global_i16)),
            InstructionKindV2::GetElementPtr {
                base: ValueIdV2::new(1),
                indices: vec![ValueIdV2::new(index_id)],
            },
        )?);
        body.push(instruction(
            Some(TypedValueV2::new(ValueIdV2::new(load_id), i16_type)),
            InstructionKindV2::Load {
                pointer: ValueIdV2::new(pointer_id),
                value_type: ScalarTypeV1::I16,
                alignment: 2,
            },
        )?);
    }
    body.push(instruction(
        Some(TypedValueV2::new(ValueIdV2::new(34), i16x4_type)),
        InstructionKindV2::VectorZero {
            element_type: ScalarTypeV1::I16,
        },
    )?);
    for (result_id, vector_id, element_id, index_id) in [
        (35, 34, 30, 14),
        (36, 35, 31, 15),
        (37, 36, 32, 16),
        (38, 37, 33, 17),
    ] {
        body.push(instruction(
            Some(TypedValueV2::new(ValueIdV2::new(result_id), i16x4_type)),
            InstructionKindV2::InsertElement {
                vector: ValueIdV2::new(vector_id),
                element: ValueIdV2::new(element_id),
                index: ValueIdV2::new(index_id),
            },
        )?);
    }
    body.push(instruction(
        Some(TypedValueV2::new(ValueIdV2::new(39), f32x4_type)),
        InstructionKindV2::VectorZero {
            element_type: ScalarTypeV1::F32,
        },
    )?);
    body.push(instruction(
        Some(TypedValueV2::new(ValueIdV2::new(40), f32x4_type)),
        InstructionKindV2::Call {
            target: fe2o3_llvm_handoff::CallTargetV2::Intrinsic(
                IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k,
            ),
            arguments: vec![
                ValueIdV2::new(25),
                ValueIdV2::new(38),
                ValueIdV2::new(39),
                ValueIdV2::new(14),
                ValueIdV2::new(14),
                ValueIdV2::new(14),
            ],
        },
    )?);
    for (pointer_id, value_id, index_id, component_id) in [
        (41, 45, 13, 14),
        (42, 46, 21, 15),
        (43, 47, 22, 16),
        (44, 48, 23, 17),
    ] {
        body.push(instruction(
            Some(TypedValueV2::new(ValueIdV2::new(pointer_id), global_f32)),
            InstructionKindV2::GetElementPtr {
                base: ValueIdV2::new(2),
                indices: vec![ValueIdV2::new(index_id)],
            },
        )?);
        body.push(instruction(
            Some(TypedValueV2::new(ValueIdV2::new(value_id), f32_type)),
            InstructionKindV2::ExtractElement {
                vector: ValueIdV2::new(40),
                index: ValueIdV2::new(component_id),
            },
        )?);
        body.push(instruction(
            None,
            InstructionKindV2::Store {
                pointer: ValueIdV2::new(pointer_id),
                value: ValueIdV2::new(value_id),
                value_type: ScalarTypeV1::F32,
                alignment: 4,
            },
        )?);
    }
    let entry = BasicBlockV2::new(BlockIdV2::new(0), body, TerminatorV2::Return(None));
    let mut executable_attributes = attributes
        .into_iter()
        .map(FunctionAttributeV2::from)
        .collect::<Vec<_>>();
    executable_attributes.push(FunctionAttributeV2::RequiredWorkgroupSize([
        BF16_GEMM_KERNEL_WORKGROUP_X_V1 as u16,
        1,
        1,
    ]));
    let function = FunctionV2::new(
        FunctionIdV2::new(0),
        BF16_GEMM_KERNEL_SYMBOL_V1,
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        parameters,
        executable_attributes,
        BlockIdV2::new(0),
        vec![entry],
        evidence.clone(),
    )
    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?;
    let module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![],
        vec![
            IntrinsicReferenceV2::new(IntrinsicV2::AmdGpuWorkitemId(AxisV2::X), evidence.clone()),
            IntrinsicReferenceV2::new(IntrinsicV2::AmdGpuMfmaF32_16x16x16Bf16_1k, evidence),
        ],
        vec![function],
    )
    .map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)?;
    Gfx942HandoffV2::new(base, module).map_err(PrepareBf16GemmKernelErrorV1::HandoffV2)
}

fn exact_function_attributes_v1() -> Vec<FunctionAttributeV1> {
    FunctionAttributeV1::gfx942_kernel_defaults(
        WorkgroupSizeRangeV1::new(
            BF16_GEMM_KERNEL_WORKGROUP_X_V1 as u16,
            BF16_GEMM_KERNEL_WORKGROUP_X_V1 as u16,
        )
        .expect("the fixed BF16 GEMM workgroup bound is valid"),
    )
}

fn exact_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(BF16_GEMM_KERNEL_TARGET_V1)
        .expect("the fixed BF16 GEMM target is canonical")
}

/// Sealed Worker V2 request retaining the exact prepared BF16 GEMM graph.
#[derive(Debug)]
pub struct InertBf16GemmKernelWorkerRequestV1 {
    prepared: PreparedBf16GemmKernelV1,
    transaction_handoff_identity: CompilerModuleHandoffIdentityV1,
    plan_identity: LinkPlanIdentityV1,
    worker_measurement: WorkerMeasurementV1,
    request: CompilerHandoffWorkerRequestV2,
}

impl InertBf16GemmKernelWorkerRequestV1 {
    /// Exact source semantics retained by this request.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.prepared.source_identity
    }

    /// Attempt-scoped transaction handoff identity.
    pub const fn transaction_handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.transaction_handoff_identity
    }

    /// Link plan identity rechecked during construction.
    pub const fn plan_identity(&self) -> LinkPlanIdentityV1 {
        self.plan_identity
    }

    /// Measured executable/build fields sealed into the request.
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker_measurement
    }

    /// Exact sealed request passed to the executor.
    pub const fn sealed_request(&self) -> &WorkerRequestV2 {
        self.request.sealed_request()
    }

    /// Complete compiler-handoff request retaining attempt ownership.
    pub const fn compiler_handoff_request(&self) -> &CompilerHandoffWorkerRequestV2 {
        &self.request
    }

    /// This request does not establish that the worker process executed.
    pub const fn authenticates_worker_execution(&self) -> bool {
        false
    }

    /// A replay plan's predeclared output identity is not an independent artifact approval.
    pub const fn has_independent_output_approval(&self) -> bool {
        false
    }

    /// This request grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// This request grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// This request grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Field rejected while composing the exact BF16 GEMM request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Bf16GemmKernelRequestFieldV1 {
    /// Consumed attempt bytes or transaction identity changed.
    ConsumedHandoff,
    /// Link plan target, inputs, options, or provenance changed.
    LinkPlan,
    /// Input-kind closure changed.
    InputKinds,
    /// Output byte bound changed.
    Output,
    /// Compiler module, symbols, target, or COV changed after sealing.
    SealedRequest,
    /// Worker measurement changed after sealing.
    WorkerMeasurement,
}

/// Failure to construct the exact sealed BF16 GEMM request.
#[derive(Debug)]
#[non_exhaustive]
pub enum ConstructBf16GemmKernelRequestErrorV1 {
    /// A local exact-profile binding changed.
    Binding(Bf16GemmKernelRequestFieldV1),
    /// The shared sealed request constructor rejected the handoff.
    Request(WorkerRequestConstructionError),
}

impl fmt::Display for ConstructBf16GemmKernelRequestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binding(field) => {
                write!(formatter, "BF16 GEMM request substituted {field:?}")
            }
            Self::Request(error) => {
                write!(formatter, "BF16 GEMM request construction failed: {error}")
            }
        }
    }
}

impl std::error::Error for ConstructBf16GemmKernelRequestErrorV1 {}

/// Consumes the exact published compiler handoff into one sealed Worker V2 request.
pub fn construct_bf16_gemm_kernel_worker_request_v1(
    prepared: PreparedBf16GemmKernelV1,
    plan: &MultiInputLinkPlanV1,
    worker: &PinnedWorkerV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    input_kinds: &LinkInputKindClosureV1,
    output: WorkerOutputConstraintsV1,
) -> Result<InertBf16GemmKernelWorkerRequestV1, ConstructBf16GemmKernelRequestErrorV1> {
    let transaction_identity = CompilerModuleHandoffIdentityV1::from_bytes(
        Sha256::digest(prepared.compiler_handoff.canonical_bytes()).into(),
    );
    if consumed.bytes() != prepared.compiler_handoff.canonical_bytes()
        || consumed.identity() != transaction_identity
    {
        return Err(ConstructBf16GemmKernelRequestErrorV1::Binding(
            Bf16GemmKernelRequestFieldV1::ConsumedHandoff,
        ));
    }
    validate_plan(&prepared, plan, input_kinds, &output)
        .map_err(ConstructBf16GemmKernelRequestErrorV1::Binding)?;
    let measurement = worker.measurement().clone();
    let request = construct_worker_request_v2_from_consumed_handoff(
        plan,
        &measurement,
        consumed,
        vec![],
        input_kinds,
        output.clone(),
    )
    .map_err(ConstructBf16GemmKernelRequestErrorV1::Request)?;
    let sealed = request.sealed_request();
    let module = sealed.compiler_module();
    if module.kind() != WorkerInputKindV1::LlvmTextIr
        || module.bytes() != prepared.compiler_handoff.module_bytes()
        || module.identity() != prepared.assembly_content_identity()
        || !sealed.external_providers().is_empty()
        || !sealed.import_symbols().is_empty()
        || !sealed.export_symbols().is_empty()
        || !sealed.final_symbols().iter().map(String::as_str).eq([
            BF16_GEMM_KERNEL_SYMBOL_V1,
            BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ])
        || sealed.target() != exact_target()
        || sealed.code_object_version() != CodeObjectVersion::V6
        || sealed.options() != WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
    {
        return Err(ConstructBf16GemmKernelRequestErrorV1::Binding(
            Bf16GemmKernelRequestFieldV1::SealedRequest,
        ));
    }
    if sealed.worker_executable() != measurement.executable()
        || sealed.worker_build_identity() != measurement.worker_build_identity()
        || sealed.llvm_build_identity() != measurement.llvm_build_identity()
    {
        return Err(ConstructBf16GemmKernelRequestErrorV1::Binding(
            Bf16GemmKernelRequestFieldV1::WorkerMeasurement,
        ));
    }
    Ok(InertBf16GemmKernelWorkerRequestV1 {
        prepared,
        transaction_handoff_identity: transaction_identity,
        plan_identity: plan.identity(),
        worker_measurement: measurement,
        request,
    })
}

fn validate_plan(
    prepared: &PreparedBf16GemmKernelV1,
    plan: &MultiInputLinkPlanV1,
    kinds: &LinkInputKindClosureV1,
    output: &WorkerOutputConstraintsV1,
) -> Result<(), Bf16GemmKernelRequestFieldV1> {
    if plan.target() != exact_target() || plan.output().target() != exact_target() {
        return Err(Bf16GemmKernelRequestFieldV1::LinkPlan);
    }
    let [input] = plan.inputs() else {
        return Err(Bf16GemmKernelRequestFieldV1::LinkPlan);
    };
    let input_identity = prepared.assembly_content_identity();
    if input.identity() != input_identity || input.target() != exact_target() {
        return Err(Bf16GemmKernelRequestFieldV1::LinkPlan);
    }
    let options = plan.options();
    if options.len() != 4
        || options[0].name() != "code-object-version"
        || options[0].value() != "6"
        || options[1].name() != "opt-level"
        || options[1].value() != "2"
        || options[2].name() != "strip-debug"
        || options[2].value() != "true"
        || options[3].name() != "verify-each"
        || options[3].value() != "true"
        || output.max_bytes() != plan.output().identity().byte_len()
    {
        return Err(Bf16GemmKernelRequestFieldV1::Output);
    }
    if kinds.plan_identity() != plan.identity() || kinds.kinds() != [WorkerInputKindV1::LlvmTextIr]
    {
        return Err(Bf16GemmKernelRequestFieldV1::InputKinds);
    }
    let output_identity = plan.output().identity();
    if plan.provenance().len() != 2
        || !plan
            .provenance()
            .iter()
            .any(|node| node.identity() == input_identity && node.parents().is_empty())
        || !plan
            .provenance()
            .iter()
            .any(|node| node.identity() == output_identity && node.parents() == [input_identity])
    {
        return Err(Bf16GemmKernelRequestFieldV1::LinkPlan);
    }
    Ok(())
}

/// Exact post-worker COV6 closure retained with the sealed response lineage.
#[derive(Debug)]
pub struct AdmittedBf16GemmKernelArtifactV1 {
    source_identity: HandoffIdentityV2,
    assembly_identity: ContentIdentityV1,
    compiler_handoff_identity: CompilerModuleHandoffIdentityV2,
    transaction_handoff_identity: CompilerModuleHandoffIdentityV1,
    link_plan_identity: LinkPlanIdentityV1,
    sealed_request_identity: [u8; 32],
    worker_measurement: WorkerMeasurementV1,
    inspected: InspectedRawWorkerV2HsacoV1,
    loader_plan: LoadPlan,
}

impl AdmittedBf16GemmKernelArtifactV1 {
    /// Exact typed BF16 GEMM source identity retained by this artifact lineage.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.source_identity
    }

    /// Exact canonical LLVM input identity retained by this artifact lineage.
    pub const fn assembly_identity(&self) -> ContentIdentityV1 {
        self.assembly_identity
    }

    /// Exact compiler handoff identity consumed by the reproducible first build.
    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.compiler_handoff_identity
    }

    /// Attempt-transaction identity re-established from the same canonical compiler handoff.
    pub const fn transaction_handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.transaction_handoff_identity
    }

    /// Exact observed link-plan identity retaining the output identity and its input provenance.
    pub const fn link_plan_identity(&self) -> LinkPlanIdentityV1 {
        self.link_plan_identity
    }

    /// Exact replay request identity echoed by the sealed Worker V2 response.
    pub const fn sealed_request_identity(&self) -> &[u8; 32] {
        &self.sealed_request_identity
    }

    /// Measured worker executable and compiler-build identity retained by first-build evidence.
    pub const fn worker_measurement(&self) -> &WorkerMeasurementV1 {
        &self.worker_measurement
    }

    /// Exact Worker V2 output identity of the inspected artifact.
    pub const fn artifact_identity(&self) -> ContentIdentityV1 {
        self.inspected.linked_output_identity()
    }

    /// Strict pure-Rust loader envelope derived from the same exact bytes.
    pub const fn loader_plan(&self) -> &LoadPlan {
        &self.loader_plan
    }

    /// Exact bytes retained by sealed Worker V2 evidence.
    pub fn exact_bytes(&self) -> &[u8] {
        self.inspected.exact_bytes()
    }

    /// The output identity is observed, not an independently approved deployment pin.
    pub const fn has_independent_deployment_pin(&self) -> bool {
        false
    }

    /// Admission grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Admission grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Exact post-worker BF16 GEMM artifact rejection.
#[derive(Debug)]
#[non_exhaustive]
pub enum AdmitBf16GemmKernelArtifactErrorV1 {
    /// A retained Worker V2 request/response transcript failed canonical decoding.
    Protocol(WorkerProtocolError),
    /// First-build handoff, module, plan, or response lineage differs from the typed source.
    SourceLineage,
    /// Generic sealed Worker V2/raw-HSACO inspection failed.
    Raw(fe2o3_hsaco_finalize::WorkerV2RawHsacoInspectionError),
    /// The measured worker or exact LLVM identity differs from the request.
    WorkerMeasurement,
    /// Kernel symbols or metadata ABI differ from the exact BF16 GEMM profile.
    GemmProfile,
    /// AMDHSA parser or descriptor binding failed.
    Hsaco(KernelBindingError),
    /// Strict allocation-free loader envelope rejected the bytes.
    Loader(PlanError),
}

impl fmt::Display for AdmitBf16GemmKernelArtifactErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => {
                write!(formatter, "BF16 GEMM Worker V2 transcript failed: {error}")
            }
            Self::SourceLineage => {
                formatter.write_str("BF16 GEMM first-build lineage differs from exact typed source")
            }
            Self::Raw(error) => {
                write!(formatter, "BF16 GEMM raw-HSACO inspection failed: {error}")
            }
            Self::WorkerMeasurement => formatter.write_str("BF16 GEMM worker measurement changed"),
            Self::GemmProfile => {
                formatter.write_str("BF16 GEMM kernel symbol, ABI, or resources changed")
            }
            Self::Hsaco(error) => {
                write!(
                    formatter,
                    "BF16 GEMM HSACO descriptor binding failed: {error}"
                )
            }
            Self::Loader(error) => {
                write!(
                    formatter,
                    "BF16 GEMM strict loader rejected COV6: {error:?}"
                )
            }
        }
    }
}

impl std::error::Error for AdmitBf16GemmKernelArtifactErrorV1 {}

/// Consumes typed source authority and matching sealed first-build evidence into one inert artifact.
///
/// The expected worker is a measured [`PinnedWorkerV1`], not a caller-provided identity string.
/// The link plan's output identity is independently re-observed from the retained bytes; it is not
/// accepted as a deployment approval or fixed artifact pin.
pub fn admit_bf16_gemm_kernel_artifact_v1(
    prepared: PreparedBf16GemmKernelV1,
    expected_worker: &PinnedWorkerV1,
    source: InertFirstBuildWorkerV2EvidenceV1,
) -> Result<AdmittedBf16GemmKernelArtifactV1, AdmitBf16GemmKernelArtifactErrorV1> {
    validate_first_build_lineage(&prepared, expected_worker, &source)?;
    let source_identity = prepared.source_identity;
    let assembly_identity = prepared.assembly_content_identity();
    let compiler_handoff_identity = prepared.compiler_handoff_identity;
    let transaction_handoff_identity = CompilerModuleHandoffIdentityV1::from_bytes(
        Sha256::digest(prepared.compiler_handoff.canonical_bytes()).into(),
    );
    let link_plan_identity = source.link_plan_identity();
    let sealed_request_identity = *source.authorized_request_identity();
    let worker_measurement = source.worker_measurement().clone();
    let inspected = inspect_general_gemm_worker_v2_raw_hsaco_v1(source)
        .map_err(AdmitBf16GemmKernelArtifactErrorV1::Raw)?;
    if inspected.worker_measurement() != &worker_measurement
        || inspected.handoff_identity() != transaction_handoff_identity
        || inspected.link_plan_identity() != link_plan_identity
        || inspected.sealed_request_identity() != &sealed_request_identity
        || inspected.worker_measurement().llvm_build_identity()
            != fe2o3_llvm_worker_handoff::EXACT_LLVM_BUILD_IDENTITY_V1
    {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::WorkerMeasurement);
    }
    if inspected.policy().observed_kernels().len() != 1
        || inspected.policy().observed_kernels()[0].entry() != BF16_GEMM_KERNEL_SYMBOL_V1
        || inspected.policy().observed_kernels()[0].descriptor()
            != BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1
    {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::GemmProfile);
    }
    let bytes = inspected.exact_bytes();
    if !inspected.linked_output_identity().matches(bytes) {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::GemmProfile);
    }
    let bound = inspect_and_bind_kernel_descriptors(bytes)
        .map_err(AdmitBf16GemmKernelArtifactErrorV1::Hsaco)?;
    let [kernel] = bound.inspection().kernels() else {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::GemmProfile);
    };
    let [binding] = bound.bindings() else {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::GemmProfile);
    };
    if bound.inspection().code_object_version() != InspectedCodeObjectVersion::V6
        || bound.inspection().target().to_string() != BF16_GEMM_KERNEL_TARGET_V1
        || kernel.name() != BF16_GEMM_KERNEL_SYMBOL_V1
        || kernel.symbol() != BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1
        || kernel.kernarg_segment_size() != BF16_GEMM_KERNEL_KERNARG_BYTES_V1
        || kernel.kernarg_segment_alignment() != BF16_GEMM_KERNEL_KERNARG_ALIGNMENT_V1
        || kernel.required_workgroup_size() != Some([BF16_GEMM_KERNEL_WORKGROUP_X_V1, 1, 1])
        || kernel.max_flat_workgroup_size() != BF16_GEMM_KERNEL_WORKGROUP_X_V1
        || kernel.wavefront_size() != 64
        || kernel.group_segment_fixed_size() != 0
        || kernel.private_segment_fixed_size() != 0
        || kernel.sgpr_spill_count().unwrap_or(0) != 0
        || kernel.vgpr_spill_count().unwrap_or(0) != 0
        || kernel.uses_dynamic_stack()
        || kernel.implicit_argument_offset() != Some(24)
        || kernel.implicit_argument_size() != 256
        || bound.inspection().has_printf_metadata()
        || binding.kernel_index() != 0
        || binding.descriptor().group_segment_fixed_size() != 0
        || binding.descriptor().private_segment_fixed_size() != 0
        || binding.descriptor().wavefront_size() != 64
        || binding.descriptor().uses_dynamic_stack()
        || !exact_explicit_arguments(kernel.explicit_arguments())
        || !exact_hidden_arguments(kernel.hidden_arguments())
    {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::GemmProfile);
    }
    let loader = fe2o3_amdhsa_loader::validate(bytes, AdmittedProfile::Gfx942XnackOffCov6)
        .map_err(AdmitBf16GemmKernelArtifactErrorV1::Loader)?;
    let loader_plan = *loader.plan();
    Ok(AdmittedBf16GemmKernelArtifactV1 {
        source_identity,
        assembly_identity,
        compiler_handoff_identity,
        transaction_handoff_identity,
        link_plan_identity,
        sealed_request_identity,
        worker_measurement,
        inspected,
        loader_plan,
    })
}

fn validate_first_build_lineage(
    prepared: &PreparedBf16GemmKernelV1,
    expected_worker: &PinnedWorkerV1,
    source: &InertFirstBuildWorkerV2EvidenceV1,
) -> Result<(), AdmitBf16GemmKernelArtifactErrorV1> {
    let transaction_identity = CompilerModuleHandoffIdentityV1::from_bytes(
        Sha256::digest(prepared.compiler_handoff.canonical_bytes()).into(),
    );
    if source.handoff_identity() != transaction_identity
        || source.compiler_envelope() != prepared.compiler_handoff.envelope()
        || source.symbol_manifest() != prepared.compiler_handoff.symbol_manifest()
        || source.worker_measurement() != expected_worker.measurement()
        || source.worker_measurement().llvm_build_identity()
            != fe2o3_llvm_worker_handoff::EXACT_LLVM_BUILD_IDENTITY_V1
    {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::SourceLineage);
    }
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        source.bootstrap_request_bytes(),
        source.bootstrap().response().canonical_bytes(),
    )
    .map_err(AdmitBf16GemmKernelArtifactErrorV1::Protocol)?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        source.authorized_request_bytes(),
        source.authorized().response().canonical_bytes(),
    )
    .map_err(AdmitBf16GemmKernelArtifactErrorV1::Protocol)?;
    if !exact_exchange_request(prepared, expected_worker, bootstrap.request())
        || !exact_exchange_request(prepared, expected_worker, replay.request())
        || replay.request().identity() != source.authorized_request_identity()
        || replay.response().request_identity() != replay.request().identity()
    {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::SourceLineage);
    }
    let bytes = source.output_bytes();
    let output_identity = source.output_identity();
    let Some(bootstrap_output) = bootstrap.response().output() else {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::SourceLineage);
    };
    let Some(replay_output) = replay.response().output() else {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::SourceLineage);
    };
    if bootstrap_output.bytes() != bytes
        || replay_output.bytes() != bytes
        || bootstrap_output.identity() != output_identity
        || replay_output.identity() != output_identity
        || !output_identity.matches(bytes)
        || replay.request().output_constraints().max_bytes() != output_identity.byte_len()
        || !exact_observed_plan(prepared, source.plan(), output_identity)
    {
        return Err(AdmitBf16GemmKernelArtifactErrorV1::SourceLineage);
    }
    Ok(())
}

fn exact_exchange_request(
    prepared: &PreparedBf16GemmKernelV1,
    expected_worker: &PinnedWorkerV1,
    request: &WorkerRequestV2,
) -> bool {
    let measurement = expected_worker.measurement();
    request.target() == exact_target()
        && request.code_object_version() == CodeObjectVersion::V6
        && request.options() == WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
        && request.compiler_module().kind() == WorkerInputKindV1::LlvmTextIr
        && request.compiler_module().bytes() == prepared.compiler_handoff.module_bytes()
        && request.compiler_module().identity() == prepared.assembly_content_identity()
        && request.compiler_envelope_identity().as_bytes()
            == prepared.compiler_handoff.envelope().identity().as_bytes()
        && request.external_providers().is_empty()
        && request.import_symbols().is_empty()
        && request.export_symbols().is_empty()
        && request.final_symbols().iter().map(String::as_str).eq([
            BF16_GEMM_KERNEL_SYMBOL_V1,
            BF16_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ])
        && request.worker_executable() == measurement.executable()
        && request.worker_build_identity() == measurement.worker_build_identity()
        && request.llvm_build_identity() == measurement.llvm_build_identity()
}

fn exact_observed_plan(
    prepared: &PreparedBf16GemmKernelV1,
    plan: &MultiInputLinkPlanV1,
    output_identity: ContentIdentityV1,
) -> bool {
    if plan.target() != exact_target()
        || plan.output().target() != exact_target()
        || plan.output().identity() != output_identity
        || output_identity.byte_len() == 0
    {
        return false;
    }
    let [input] = plan.inputs() else {
        return false;
    };
    let input_identity = prepared.assembly_content_identity();
    if input.identity() != input_identity || input.target() != exact_target() {
        return false;
    }
    let options = plan.options();
    options.len() == 4
        && options[0].name() == "code-object-version"
        && options[0].value() == "6"
        && options[1].name() == "opt-level"
        && options[1].value() == "2"
        && options[2].name() == "strip-debug"
        && options[2].value() == "true"
        && options[3].name() == "verify-each"
        && options[3].value() == "true"
        && plan.provenance().len() == 2
        && plan
            .provenance()
            .iter()
            .any(|node| node.identity() == input_identity && node.parents().is_empty())
        && plan
            .provenance()
            .iter()
            .any(|node| node.identity() == output_identity && node.parents() == [input_identity])
}

fn exact_explicit_arguments(arguments: &[ExplicitArgument]) -> bool {
    let expected = [
        (
            A_NAME,
            0,
            8,
            ExplicitValueType::I16,
            ExplicitValueKind::GlobalBuffer,
            Some(ArgumentAddressSpace::Global),
            Some(ArgumentAccess::ReadOnly),
        ),
        (
            B_NAME,
            8,
            2,
            ExplicitValueType::I16,
            ExplicitValueKind::GlobalBuffer,
            Some(ArgumentAddressSpace::Global),
            Some(ArgumentAccess::ReadOnly),
        ),
        (
            C_NAME,
            16,
            4,
            ExplicitValueType::F32,
            ExplicitValueKind::GlobalBuffer,
            Some(ArgumentAddressSpace::Global),
            Some(ArgumentAccess::WriteOnly),
        ),
    ];
    arguments.len() == expected.len()
        && arguments.iter().zip(expected).all(|(actual, expected)| {
            actual.name() == Some(expected.0)
                && actual.offset() == expected.1
                && actual.size() == 8
                && actual.alignment().is_none_or(|alignment| alignment == 8)
                && actual
                    .pointee_alignment()
                    .is_none_or(|alignment| alignment == expected.2)
                && actual
                    .value_type()
                    .is_none_or(|value_type| value_type == expected.3)
                && actual.value_kind() == expected.4
                && actual.address_space() == expected.5
                && actual.access() == expected.6
        })
}

fn exact_hidden_arguments(arguments: &[HiddenArgument]) -> bool {
    const EXPECTED: [(u64, u64, HiddenValueKind); 19] = [
        (24, 4, HiddenValueKind::BlockCountX),
        (28, 4, HiddenValueKind::BlockCountY),
        (32, 4, HiddenValueKind::BlockCountZ),
        (36, 2, HiddenValueKind::GroupSizeX),
        (38, 2, HiddenValueKind::GroupSizeY),
        (40, 2, HiddenValueKind::GroupSizeZ),
        (42, 2, HiddenValueKind::RemainderX),
        (44, 2, HiddenValueKind::RemainderY),
        (46, 2, HiddenValueKind::RemainderZ),
        (64, 8, HiddenValueKind::GlobalOffsetX),
        (72, 8, HiddenValueKind::GlobalOffsetY),
        (80, 8, HiddenValueKind::GlobalOffsetZ),
        (88, 2, HiddenValueKind::GridDimensions),
        (104, 8, HiddenValueKind::HostcallBuffer),
        (112, 8, HiddenValueKind::MultigridSyncArgument),
        (120, 8, HiddenValueKind::HeapV1),
        (128, 8, HiddenValueKind::DefaultQueue),
        (136, 8, HiddenValueKind::CompletionAction),
        (224, 8, HiddenValueKind::QueuePointer),
    ];
    arguments.len() == EXPECTED.len()
        && arguments.iter().zip(EXPECTED).all(|(actual, expected)| {
            actual.offset() == expected.0
                && actual.size() == expected.1
                && actual.value_kind() == expected.2
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_hsaco_finalize::{LinkInputV1, LinkOptionV1, LinkOutputV1, ProvenanceNodeV1};

    fn bindings(seed: u8) -> Bf16GemmKernelSourceBindingsV1 {
        Bf16GemmKernelSourceBindingsV1::new(
            [seed; 32],
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
        )
    }

    #[test]
    fn exact_bf16_gemm_semantics_is_deterministic_and_worker_policy_admitted() {
        let first = prepare_bf16_gemm_kernel_v1(bindings(0x31)).unwrap();
        let repeat = prepare_bf16_gemm_kernel_v1(bindings(0x31)).unwrap();
        assert_eq!(first.source_identity(), repeat.source_identity());
        assert_eq!(
            first.worker_admission_identity(),
            repeat.worker_admission_identity()
        );
        assert_eq!(first.assembly_sha256(), repeat.assembly_sha256());
        assert_eq!(first.assembly_len(), repeat.assembly_len());
        assert_eq!(
            first.compiler_handoff_identity(),
            repeat.compiler_handoff_identity()
        );
        assert_eq!(first.manifest_identity(), repeat.manifest_identity());
        assert_eq!(
            first.compiler_handoff().module_bytes(),
            repeat.compiler_handoff().module_bytes()
        );
        assert!(!first.authenticates_worker_executable());
        assert!(!first.authenticates_upstream_provenance());
        assert!(!first.grants_publication_authority());
        assert!(!first.grants_load_authority());
        assert!(!first.grants_launch_authority());
    }

    #[test]
    fn source_or_stage_mutation_rebinds_every_downstream_identity() {
        let baseline = prepare_bf16_gemm_kernel_v1(bindings(0x41)).unwrap();
        for mutated in [
            Bf16GemmKernelSourceBindingsV1::new([0x51; 32], [0x42; 32], [0x43; 32], [0x44; 32]),
            Bf16GemmKernelSourceBindingsV1::new([0x41; 32], [0x52; 32], [0x43; 32], [0x44; 32]),
            Bf16GemmKernelSourceBindingsV1::new([0x41; 32], [0x42; 32], [0x53; 32], [0x44; 32]),
            Bf16GemmKernelSourceBindingsV1::new([0x41; 32], [0x42; 32], [0x43; 32], [0x54; 32]),
        ] {
            let changed = prepare_bf16_gemm_kernel_v1(mutated).unwrap();
            assert_ne!(baseline.source_identity(), changed.source_identity());
            assert_ne!(
                baseline.worker_admission_identity(),
                changed.worker_admission_identity()
            );
            assert_ne!(baseline.assembly_sha256(), changed.assembly_sha256());
            assert_ne!(
                baseline.compiler_handoff_identity(),
                changed.compiler_handoff_identity()
            );
        }
    }

    #[test]
    fn zero_source_and_stage_identities_fail_closed() {
        for rejected in [
            Bf16GemmKernelSourceBindingsV1::new([0; 32], [2; 32], [3; 32], [4; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [0; 32], [3; 32], [4; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [2; 32], [0; 32], [4; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [2; 32], [3; 32], [0; 32]),
        ] {
            assert!(matches!(
                prepare_bf16_gemm_kernel_v1(rejected),
                Err(PrepareBf16GemmKernelErrorV1::HandoffV1(_))
            ));
        }
    }

    #[test]
    fn repeated_source_role_identities_fail_closed() {
        for rejected in [
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [1; 32], [3; 32], [4; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [2; 32], [1; 32], [4; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [2; 32], [3; 32], [1; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [2; 32], [2; 32], [4; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [2; 32], [3; 32], [2; 32]),
            Bf16GemmKernelSourceBindingsV1::new([1; 32], [2; 32], [3; 32], [3; 32]),
        ] {
            assert!(!rejected.authenticates_provenance());
            assert!(matches!(
                prepare_bf16_gemm_kernel_v1(rejected),
                Err(PrepareBf16GemmKernelErrorV1::NonDistinctSourceBindings)
            ));
        }
    }

    #[test]
    fn emitted_llvm_has_exact_direct_global_mfma_effect_and_closed_abi() {
        let prepared = prepare_bf16_gemm_kernel_v1(bindings(0x61)).unwrap();
        let llvm = std::str::from_utf8(prepared.compiler_handoff().module_bytes()).unwrap();
        for required in [
            "define amdgpu_kernel void @bf16_gemm_16x16x16_v1",
            "%a_bf16",
            "%b_bf16",
            "%c_f32",
            "dereferenceable(512)",
            "dereferenceable(1024)",
            "!reqd_work_group_size",
            "!{i32 64, i32 1, i32 1}",
            "\"amdgpu-flat-work-group-size\"=\"64,64\"",
            "@llvm.amdgcn.workitem.id.x()",
            "@llvm.amdgcn.mfma.f32.16x16x16bf16.1k",
            "load <4 x i16>",
            "load i16",
            "insertelement <4 x i16>",
            "extractelement <4 x float>",
            "getelementptr i16",
            "getelementptr float",
            "store float",
            "and i32",
            "lshr i32",
            "shl i32",
            "add i32",
        ] {
            assert!(
                llvm.contains(required),
                "missing exact LLVM fragment: {required}\n{llvm}"
            );
        }
        for forbidden in [
            "atomic",
            "volatile",
            "addrspacecast",
            "call void",
            "@llvm.memcpy",
            "@llvm.amdgcn.s.barrier",
            "phi ",
            "br ",
        ] {
            assert!(
                !llvm.contains(forbidden),
                "forbidden LLVM fragment: {forbidden}"
            );
        }
        assert_eq!(llvm.matches("load <4 x i16>").count(), 1);
        assert_eq!(llvm.matches("load i16").count(), 4);
        assert_eq!(llvm.matches("store float").count(), 4);
        assert_eq!(
            llvm.matches("call <4 x float> @llvm.amdgcn.mfma").count(),
            1
        );
    }

    #[test]
    fn invalid_hsaco_never_reaches_gemm_profile_or_loader_authority() {
        let error = inspect_and_bind_kernel_descriptors(b"not an ELF").unwrap_err();
        assert!(matches!(error, KernelBindingError::Inspection(_)));
        assert!(
            fe2o3_amdhsa_loader::validate(b"not an ELF", AdmittedProfile::Gfx942XnackOffCov6)
                .is_err()
        );
    }

    fn observed_plan(
        prepared: &PreparedBf16GemmKernelV1,
        target: DeviceTargetV1,
        input_identity: ContentIdentityV1,
        output_identity: ContentIdentityV1,
        optimization: &str,
    ) -> MultiInputLinkPlanV1 {
        MultiInputLinkPlanV1::canonicalized(
            target,
            vec![LinkInputV1::new(input_identity, target)],
            vec![
                LinkOptionV1::new("code-object-version", "6").unwrap(),
                LinkOptionV1::new("opt-level", optimization).unwrap(),
                LinkOptionV1::new("strip-debug", "true").unwrap(),
                LinkOptionV1::new("verify-each", "true").unwrap(),
            ],
            LinkOutputV1::new(output_identity, target),
            vec![
                ProvenanceNodeV1::new(input_identity, vec![]).unwrap(),
                ProvenanceNodeV1::new(output_identity, vec![input_identity]).unwrap(),
            ],
        )
        .unwrap_or_else(|error| {
            panic!(
                "valid test plan for {} failed: {error}",
                prepared.source_identity()
            )
        })
    }

    #[test]
    fn observed_plan_rejects_target_input_option_and_output_mutations() {
        let prepared = prepare_bf16_gemm_kernel_v1(bindings(0x71)).unwrap();
        let input = prepared.assembly_content_identity();
        let output = ContentIdentityV1::calculate(b"observed exact COV6 bytes");
        let exact = observed_plan(&prepared, exact_target(), input, output, "2");
        assert!(exact_observed_plan(&prepared, &exact, output));

        let other_target = DeviceTargetV1::parse("gfx950:xnack-").unwrap();
        let wrong_target = observed_plan(&prepared, other_target, input, output, "2");
        let wrong_input = observed_plan(
            &prepared,
            exact_target(),
            ContentIdentityV1::calculate(b"mutated LLVM input"),
            output,
            "2",
        );
        let wrong_option = observed_plan(&prepared, exact_target(), input, output, "0");
        let changed_output = ContentIdentityV1::calculate(b"other observed COV6 bytes");
        let wrong_output = observed_plan(&prepared, exact_target(), input, changed_output, "2");

        assert!(!exact_observed_plan(&prepared, &wrong_target, output));
        assert!(!exact_observed_plan(&prepared, &wrong_input, output));
        assert!(!exact_observed_plan(&prepared, &wrong_option, output));
        assert!(!exact_observed_plan(&prepared, &wrong_output, output));
    }

    #[test]
    fn exact_shape_and_buffer_contract_reject_hostile_mutations() {
        let shape = Bf16GemmKernelDispatchShapeV1::exact();
        assert_eq!(shape.matrix_dimensions(), [16, 16, 16]);
        assert_eq!(shape.grid(), [64, 1, 1]);
        assert_eq!(shape.workgroup(), [64, 1, 1]);
        assert_eq!(shape.hsa_adapter_block_counts(), [1, 1, 1]);
        assert!(shape.hsa_block_counts_expand_to_aql_grid([1, 1, 1]));
        assert!(!shape.hsa_block_counts_expand_to_aql_grid([64, 1, 1]));
        assert!(!shape.grants_launch_authority());

        let exact = Bf16GemmBufferContractV1::new(
            0x1000,
            BF16_GEMM_A_BYTES_V1,
            0x2002,
            BF16_GEMM_B_BYTES_V1,
            0x3004,
            BF16_GEMM_C_BYTES_V1,
        )
        .unwrap();
        assert_eq!(exact.addresses(), [0x1000, 0x2002, 0x3004]);
        assert_eq!(exact.ends(), [0x1200, 0x2202, 0x3404]);
        assert!(!exact.authenticates_device_memory());
        assert!(!exact.grants_launch_authority());

        for (result, expected) in [
            (
                Bf16GemmBufferContractV1::new(0, 512, 0x2002, 512, 0x3004, 1024),
                Bf16GemmBufferContractErrorV1::ZeroAddress(Bf16GemmBufferV1::A),
            ),
            (
                Bf16GemmBufferContractV1::new(0x1000, 510, 0x2002, 512, 0x3004, 1024),
                Bf16GemmBufferContractErrorV1::ByteLength(Bf16GemmBufferV1::A),
            ),
            (
                Bf16GemmBufferContractV1::new(0x1002, 512, 0x2002, 512, 0x3004, 1024),
                Bf16GemmBufferContractErrorV1::Alignment(Bf16GemmBufferV1::A),
            ),
            (
                Bf16GemmBufferContractV1::new(u64::MAX - 255, 512, 0x2002, 512, 0x3004, 1024),
                Bf16GemmBufferContractErrorV1::RangeOverflow(Bf16GemmBufferV1::A),
            ),
            (
                Bf16GemmBufferContractV1::new(0x1000, 512, 0x1100, 512, 0x3004, 1024),
                Bf16GemmBufferContractErrorV1::Aliasing,
            ),
        ] {
            assert_eq!(result, Err(expected));
        }
    }
}
