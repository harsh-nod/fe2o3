#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use core::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, GeneralGemmEpilogueAttr, GeneralGemmEpilogueOp,
    GeneralGemmEpochAttr, GeneralGemmEpochOp, GeneralGemmGlobalTransferAttr,
    GeneralGemmGlobalTransferOp, GeneralGemmGridMappingAttr, GeneralGemmGridMappingOp,
    GeneralGemmLdsTransferAttr, GeneralGemmLdsTransferOp, GeneralGemmMfmaAttr, GeneralGemmMfmaOp,
    GeneralGemmPhaseLoopAttr, GeneralGemmPhaseLoopOp, GeneralGemmRuntimeAbiAttr,
    GeneralGemmRuntimeAbiOp, HierarchyAttr, HierarchyIdOp, MemoryOrderAttr, MemoryScopeAttr,
    MemorySpaceOp,
};
use dialect_kernel::{AlgorithmOp, GeneralGemmOp};
use dialect_schedule::{GeneralGemmPlanOp, GeneralGemmScheduleAttr, PlanOp};
use dialect_tile::{GeneralGemmXor4Op, MaterializeOp};
use fe2o3_compiler_api::{
    CandidateIdentityV1, CanonicalDiagnosticV1, CompileDispositionV1, CompileOutputV1,
    CompileRequestV1, CompilerStageV1, DiagnosticCodeV1, DiagnosticMessageV1, DiagnosticSeverityV1,
    DiagnosticSubjectIdentityV1, ObligationSetIdentityV1, PipelineConfigurationIdentityV1,
    PipelineSelectorV1, StageSnapshotV1,
};
use fe2o3_compiler_driver::{
    AdmittedGemmCompilerBackendV1, CompilerBackendFailureV1, GemmSemanticProgramV1,
    ProofRequiredGemmAdmissionV1, analyze_gemm_semantics_v1,
    general_gemm_semantic_obligation_set_identity_v1,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1, CompilerModuleHandoffIdentityV2,
};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CapabilityV1,
    CodeObjectVersion, CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
    DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, LogicalArgumentV1,
    ProducerIdentityV1, ScalarTypeV1 as DescriptorScalarTypeV1, SourceTypeDescriptorV1,
    SourceTypeRecordV1, Text, ValidName, ValidationError as DescriptorValidationError,
};
use fe2o3_kernel_ir::{
    GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1, GENERAL_GEMM_KIR_LDS_ELEMENTS_V1,
    GENERAL_GEMM_KIR_TILE_EXTENT_V1, GENERAL_GEMM_KIR_WAVE_LANES_V1, GeneralGemmKirDiagnosticV1,
    GeneralGemmKirIdentityV1, GeneralGemmKirV1, GeneralGemmPlanFieldsV1,
    verify_general_gemm_kir_v1,
};
use fe2o3_llvm_handoff::{
    GFX942_AMDHSA_DATA_LAYOUT_V1, GFX942_AMDHSA_TARGET_TRIPLE_V1, HandoffIdentityV2,
};
use fe2o3_llvm_text::LlvmAssemblySha256V2;
use fe2o3_pliron::{
    ContextIdentity, ContextIdentityError, PLIRON_REVISION, ensure_context_identity,
    require_context_identity,
};
use fe2o3_verifier::{
    GeneralGemmEvidenceIdentityV1, GeneralGemmProofExecutionErrorV1, GeneralGemmProofRequestV1,
    GeneralGemmProofScheduleV1,
};
use pliron::{
    builtin::{
        attributes::{BytesAttr, IdentifierAttr},
        op_interfaces::{ATTR_KEY_SYM_NAME, SingleBlockRegionInterface},
        ops::ModuleOp,
    },
    context::Context,
    dialect::DialectName,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
};
use sha2::{Digest, Sha256};

mod machine;

pub use machine::*;

/// Schema for the exact compilation-unit binding.
pub const GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1: &str =
    "fe2o3.general-gemm.compilation-binding.v1";
/// Fixed kernel symbol in the authenticated safe source profile.
pub const GENERAL_GEMM_KERNEL_SYMBOL_V1: &str = "tiled_gemm_general_v1";
/// Descriptor symbol paired with the exact kernel entry.
pub const GENERAL_GEMM_KERNEL_DESCRIPTOR_SYMBOL_V1: &str = "tiled_gemm_general_v1.kd";
/// Exact target selected by this first lowering route.
pub const GENERAL_GEMM_DEVICE_TARGET_V1: &str = "gfx942:xnack-";
/// Exact explicit kernarg bytes for the eleven source-level arguments.
pub const GENERAL_GEMM_EXPLICIT_KERNARG_BYTES_V1: u32 = 80;
/// Exact complete kernarg bytes including the gfx942 implicit argument span.
pub const GENERAL_GEMM_TOTAL_KERNARG_BYTES_V1: u32 = 336;
/// Static bytes reserved by the two distinct BF16 LDS tiles.
pub const GENERAL_GEMM_STATIC_LDS_BYTES_V1: u32 = 1024;
/// Maximum complete structured KIR bytes admitted by this route.
pub const MAX_GENERAL_GEMM_KIR_BYTES_V1: usize = 4 * 1024;
/// Exact number of typed operations in the current Pliron projection.
pub const GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1: usize = 11;
/// Exact high-level operations verified before symbolic schedule lowering.
pub const GENERAL_GEMM_SYMBOLIC_SOURCE_OPERATION_COUNT_V1: usize = 6;
/// Exact explicit GPU operations verified after symbolic schedule lowering.
pub const GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1: usize = 15;
/// Hard maximum typed operations accepted by this route.
pub const MAX_GENERAL_GEMM_PLIRON_OPERATIONS_V1: usize = 32;

const SCHEDULE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.schedule.v1\0";
const SYMBOLIC_PLAN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.symbolic-plan.v1\0";
const SYMBOLIC_KIR_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.symbolic-kir.v1\0";
const FRONTEND_SEMANTIC_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.frontend-semantic-binding.v1\0";
const PLAN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.plan-fields.v1\0";
const ABI_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.runtime-abi.v1\0";
const TOOLCHAIN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.toolchain-route.v1\0";
const BINDING_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.compilation-unit.v1\0";
const SYMBOLIC_OBLIGATION_SET_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.symbolic-obligation-set.v1\0";
const SYMBOLIC_COMPILATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.symbolic-compilation-unit.v1\0";
const SYMBOLIC_PIPELINE_CONFIGURATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.symbolic-pipeline-configuration.v1\0";
const CHECKED_LAUNCH_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.checked-launch-instantiation.v1\0";
const PROJECTION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.pliron-projection.v1\0";
const SOURCE_OPERATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.pliron-source-operations.v1\0";
const LOWERED_OPERATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.general-gemm.pliron-lowered-operations.v1\0";
const TRANSFORMATION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.pliron-transformation.v1\0";
const ARTIFACT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.artifact-binding.v1\0";
const PLIRON_MODULE_SYMBOL: &str = "fe2o3_general_gemm";
const PLIRON_SCHEMA_ATTR: &str = "fe2o3_general_gemm_schema_v1";
const PLIRON_BINDING_ATTR: &str = "fe2o3_general_gemm_binding_identity_v1";
const PLIRON_KIR_ATTR: &str = "fe2o3_general_gemm_kir_v1";
const PLIRON_SCHEDULE_ATTR: &str = "fe2o3_general_gemm_schedule_identity_v1";
const GENERAL_GEMM_LOWERING_BLOCKED_CODE_V1: u32 = 0x4647_0201;
const GENERAL_GEMM_LOWERING_BLOCKED_MESSAGE_V1: &str =
    "general GEMM production authority join is incomplete; no candidate was produced";

macro_rules! identity_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Borrows the exact domain-separated identity bytes.
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Returns the exact domain-separated identity bytes.
            pub const fn into_bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

identity_type!(
    /// Identity of one closed general-GEMM schedule.
    GeneralGemmScheduleIdentityV1
);
identity_type!(
    /// Identity of the runtime-parameterized checked host-plan expressions.
    GeneralGemmSymbolicPlanIdentityV1
);
identity_type!(
    /// Identity of the runtime-parameterized source/KIR semantic template.
    GeneralGemmSymbolicKirIdentityV1
);
identity_type!(
    /// Identity of the authenticated frontend observation carried downstream.
    GeneralGemmFrontendSemanticBindingIdentityV1
);
identity_type!(
    /// Identity of all independently checked host-plan fields.
    GeneralGemmPlanIdentityV1
);
identity_type!(
    /// Identity of the exact runtime ABI values paired with a checked plan.
    GeneralGemmRuntimeAbiIdentityV1
);
identity_type!(
    /// Identity of the pinned Pliron, target, LLVM handoff, and worker route.
    GeneralGemmToolchainRouteIdentityV1
);
identity_type!(
    /// Aggregate identity required at the proof-to-lowering boundary.
    GeneralGemmCompilationBindingIdentityV1
);
identity_type!(
    /// Aggregate identity of a runtime-parameterized production compilation.
    GeneralGemmSymbolicCompilationIdentityV1
);
identity_type!(
    /// Identity of an exact source-bound symbolic machine artifact observation.
    GeneralGemmSymbolicArtifactIdentityV1
);
identity_type!(
    /// Aggregate identity of one concrete checked launch instantiation.
    GeneralGemmCheckedLaunchInstantiationIdentityV1
);
identity_type!(
    /// Identity of the owner-checked typed Pliron projection.
    GeneralGemmPlironProjectionIdentityV1
);
identity_type!(
    /// Identity of the verified high-level symbolic operation sequence.
    GeneralGemmPlironSourceOperationIdentityV1
);
identity_type!(
    /// Identity of the verified explicit lowered GPU operation sequence.
    GeneralGemmPlironLoweredOperationIdentityV1
);
identity_type!(
    /// Identity of the exact source-to-GPU lowering transformation.
    GeneralGemmPlironTransformationIdentityV1
);
identity_type!(
    /// Identity of all fields required for an eventual executable candidate.
    GeneralGemmArtifactBindingIdentityV1
);

/// Closed schedule choices for the one general-GEMM algorithm body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmScheduleV1 {
    /// Scalar masked A/B staging with wave64 XOR4 LDS and one buffer.
    ReferenceWave64Xor4V1 = 1,
    /// Aligned A-only BF16 v4 staging, with scalar A-tail and scalar B paths.
    VectorizedAOnlyBf16GlobalTransferV1 = 2,
}

/// Fixed ABI position of one semantic runtime operand.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmAbiArgumentV1 {
    /// `&[u16]` A data and length pair.
    A = 0,
    /// `&[u16]` B data and length pair.
    B = 1,
    /// `DisjointSlice<f32>` C data and length pair.
    C = 2,
    /// M.
    M = 3,
    /// N.
    N = 4,
    /// K.
    K = 5,
    /// lda.
    Lda = 6,
    /// ldb.
    Ldb = 7,
    /// ldc.
    Ldc = 8,
    /// alpha.
    Alpha = 9,
    /// beta.
    Beta = 10,
}

/// Runtime-derived plan expression admitted by the symbolic frontend schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmSymbolicPlanExpressionV1 {
    /// `(rows - 1) * stride + columns` with empty-domain zero and checked u64 arithmetic.
    CheckedRowMajorExtent {
        /// Logical row count argument.
        rows: GeneralGemmAbiArgumentV1,
        /// Logical column count argument.
        columns: GeneralGemmAbiArgumentV1,
        /// Row-stride argument.
        stride: GeneralGemmAbiArgumentV1,
    },
    /// `ceil(value / 16)` without overflowing the u32 domain.
    CeilDiv16(GeneralGemmAbiArgumentV1),
    /// `[ceil(N/16), ceil(M/16), 1]`.
    OutputBlockCounts,
    /// `[block_x * 64, block_y, 1]` with checked multiplication.
    AqlGridWorkItems,
}

/// One independently derived source/KIR behavior record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmDerivedKirBehaviorV1 {
    /// A wave64 maps grid X/Y to 16x16 output tiles and four owners per lane.
    Wave64GridXy16 = 1,
    /// Checked row-major A/B loads are guarded and false tail lanes produce zero.
    GuardedAbCheckedRowMajorZeroTail = 2,
    /// XOR4 single-buffer staging has publish, read, MFMA, and reuse lifecycle events.
    Xor4SingleBufferPublishReadMfmaReuse = 3,
    /// The dynamic phase loop carries four f32 accumulator components.
    CarriedF32x4PhaseAccumulator = 4,
    /// Disjoint guarded C owners compute `alpha * accumulator + beta * C`.
    GuardedDisjointCAlphaAccPlusBetaC = 5,
}

/// A source-derived symbolic schema did not exactly match the closed first slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmDerivedSourceSchemaErrorV1 {
    /// One of the six plan expressions was missing, reordered, or substituted.
    PlanExpressions,
    /// One of the five KIR behavior records was missing, reordered, or substituted.
    KirBehaviors,
    /// Re-encoding a checked schema did not reproduce the closed plan identity.
    PlanIdentity,
    /// Re-encoding a checked schema did not reproduce the closed KIR identity.
    KirIdentity,
}

impl fmt::Display for GeneralGemmDerivedSourceSchemaErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid derived general GEMM source schema: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmDerivedSourceSchemaErrorV1 {}

/// Exact descriptive schema built from independently derived MIR facts.
///
/// This value grants no source, proof, artifact, or runtime authority. The
/// rustc-owned private receipt remains responsible for authenticating each
/// input record before calling [`Self::checked`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmDerivedSourceSchemaV1 {
    plan_expressions: [GeneralGemmSymbolicPlanExpressionV1; 6],
    kir_behaviors: [GeneralGemmDerivedKirBehaviorV1; 5],
}

impl GeneralGemmDerivedSourceSchemaV1 {
    /// Checks exact independently derived facts without selecting a canonical schema first.
    pub fn checked(
        plan_expressions: [GeneralGemmSymbolicPlanExpressionV1; 6],
        kir_behaviors: [GeneralGemmDerivedKirBehaviorV1; 5],
    ) -> Result<Self, GeneralGemmDerivedSourceSchemaErrorV1> {
        if plan_expressions != symbolic_plan_expressions() {
            return Err(GeneralGemmDerivedSourceSchemaErrorV1::PlanExpressions);
        }
        if kir_behaviors != symbolic_kir_behaviors() {
            return Err(GeneralGemmDerivedSourceSchemaErrorV1::KirBehaviors);
        }
        Ok(Self {
            plan_expressions,
            kir_behaviors,
        })
    }

    /// Returns the exact ordered source-derived plan expressions.
    pub const fn plan_expressions(self) -> [GeneralGemmSymbolicPlanExpressionV1; 6] {
        self.plan_expressions
    }

    /// Returns the exact ordered source-derived KIR behavior records.
    pub const fn kir_behaviors(self) -> [GeneralGemmDerivedKirBehaviorV1; 5] {
        self.kir_behaviors
    }

    /// Descriptive source schemas never grant authentication or artifact authority.
    pub const fn grants_authority(self) -> bool {
        false
    }
}

/// Closed runtime-parameterized checked-plan schema derived from positive MIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmSymbolicPlanV1 {
    identity: GeneralGemmSymbolicPlanIdentityV1,
}

impl GeneralGemmSymbolicPlanV1 {
    /// Returns the one admitted runtime-derived plan expression schema.
    pub fn canonical() -> Self {
        let schema = GeneralGemmDerivedSourceSchemaV1::checked(
            symbolic_plan_expressions(),
            symbolic_kir_behaviors(),
        )
        .expect("static general GEMM source schema is exact");
        Self::from_derived_source_schema(&schema)
            .expect("static general GEMM plan identity is exact")
    }

    /// Re-encodes a checked independently derived source schema.
    pub fn from_derived_source_schema(
        schema: &GeneralGemmDerivedSourceSchemaV1,
    ) -> Result<Self, GeneralGemmDerivedSourceSchemaErrorV1> {
        let identity = GeneralGemmSymbolicPlanIdentityV1(hash_fields(
            SYMBOLIC_PLAN_IDENTITY_DOMAIN_V1,
            &[&encode_symbolic_plan(schema.plan_expressions)],
        ));
        if identity != expected_symbolic_plan_identity() {
            return Err(GeneralGemmDerivedSourceSchemaErrorV1::PlanIdentity);
        }
        Ok(Self { identity })
    }

    /// Returns the exact symbolic-plan identity.
    pub const fn identity(self) -> GeneralGemmSymbolicPlanIdentityV1 {
        self.identity
    }

    /// Returns the fixed ordered derived-expression inventory.
    pub const fn expressions(self) -> [GeneralGemmSymbolicPlanExpressionV1; 6] {
        symbolic_plan_expressions()
    }
}

/// Closed source/KIR behavior carried from authenticated MIR before concrete launch values exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmSymbolicKirV1 {
    identity: GeneralGemmSymbolicKirIdentityV1,
}

impl GeneralGemmSymbolicKirV1 {
    /// Returns the only source template admitted for launch-time instantiation.
    pub fn canonical() -> Self {
        let schema = GeneralGemmDerivedSourceSchemaV1::checked(
            symbolic_plan_expressions(),
            symbolic_kir_behaviors(),
        )
        .expect("static general GEMM source schema is exact");
        Self::from_derived_source_schema(&schema)
            .expect("static general GEMM KIR identity is exact")
    }

    /// Re-encodes a checked independently derived source schema.
    pub fn from_derived_source_schema(
        schema: &GeneralGemmDerivedSourceSchemaV1,
    ) -> Result<Self, GeneralGemmDerivedSourceSchemaErrorV1> {
        let plan = GeneralGemmSymbolicPlanV1::from_derived_source_schema(schema)?;
        let plan_identity = plan.identity();
        let behavior_fields = encode_symbolic_kir_behavior_fields(schema.kir_behaviors);
        let mut fields = Vec::with_capacity(6);
        fields.push(plan_identity.as_bytes().as_slice());
        fields.extend(behavior_fields.iter().map(Vec::as_slice));
        let identity =
            GeneralGemmSymbolicKirIdentityV1(hash_fields(SYMBOLIC_KIR_IDENTITY_DOMAIN_V1, &fields));
        if identity != expected_symbolic_kir_identity() {
            return Err(GeneralGemmDerivedSourceSchemaErrorV1::KirIdentity);
        }
        Ok(Self { identity })
    }

    /// Returns the exact symbolic semantic-template identity.
    pub const fn identity(self) -> GeneralGemmSymbolicKirIdentityV1 {
        self.identity
    }
}

/// Structurally retained identities from the private authenticated MIR receipt.
///
/// This record is not itself authentication. Production construction must
/// occur only while consuming the non-Clone rustc-owned receipt, and final
/// artifact construction additionally requires verifier-owned evidence that
/// rechecks this exact aggregate binding.
#[derive(Debug, Eq, PartialEq)]
pub struct GeneralGemmFrontendSemanticBindingV1 {
    kernel_instance: [u8; 32],
    compiled_source: [u8; 32],
    provider_semantics: [u8; 32],
    frontend_abi: [u8; 32],
    symbolic_plan: GeneralGemmSymbolicPlanV1,
    symbolic_kir: GeneralGemmSymbolicKirV1,
    identity: GeneralGemmFrontendSemanticBindingIdentityV1,
}

/// A private frontend receipt observation was empty or named another schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmFrontendSemanticBindingErrorV1 {
    /// A required authenticated observation identity was zero.
    ZeroIdentity,
    /// The receipt named a different symbolic plan schema.
    SymbolicPlan,
    /// The receipt named a different symbolic KIR schema.
    SymbolicKir,
}

impl GeneralGemmFrontendSemanticBindingV1 {
    /// Retains the identities extracted while consuming a private frontend receipt.
    ///
    /// This structural constructor deliberately grants no proof or artifact
    /// authority. The rustc integration owns receipt authentication; later
    /// verifier admission must bind [`Self::identity`] before executable work.
    pub fn from_consumed_frontend_receipt_observation(
        kernel_instance: [u8; 32],
        compiled_source: [u8; 32],
        provider_semantics: [u8; 32],
        frontend_abi: [u8; 32],
        symbolic_plan: GeneralGemmSymbolicPlanV1,
        symbolic_kir: GeneralGemmSymbolicKirV1,
    ) -> Result<Self, GeneralGemmFrontendSemanticBindingErrorV1> {
        if [
            kernel_instance,
            compiled_source,
            provider_semantics,
            frontend_abi,
        ]
        .iter()
        .any(is_zero_identity)
        {
            return Err(GeneralGemmFrontendSemanticBindingErrorV1::ZeroIdentity);
        }
        if symbolic_plan != GeneralGemmSymbolicPlanV1::canonical() {
            return Err(GeneralGemmFrontendSemanticBindingErrorV1::SymbolicPlan);
        }
        if symbolic_kir != GeneralGemmSymbolicKirV1::canonical() {
            return Err(GeneralGemmFrontendSemanticBindingErrorV1::SymbolicKir);
        }
        let identity = GeneralGemmFrontendSemanticBindingIdentityV1(hash_fields(
            FRONTEND_SEMANTIC_IDENTITY_DOMAIN_V1,
            &[
                &kernel_instance,
                &compiled_source,
                &provider_semantics,
                &frontend_abi,
                symbolic_plan.identity().as_bytes(),
                symbolic_kir.identity().as_bytes(),
            ],
        ));
        Ok(Self {
            kernel_instance,
            compiled_source,
            provider_semantics,
            frontend_abi,
            symbolic_plan,
            symbolic_kir,
            identity,
        })
    }

    /// Returns the authenticated kernel-instance observation.
    pub const fn kernel_instance_identity(&self) -> &[u8; 32] {
        &self.kernel_instance
    }

    /// Returns the compiled source-file observation.
    pub const fn compiled_source_identity(&self) -> &[u8; 32] {
        &self.compiled_source
    }

    /// Returns the provider semantic-source observation.
    pub const fn provider_semantics_identity(&self) -> &[u8; 32] {
        &self.provider_semantics
    }

    /// Returns the authenticated ordered MIR argument-position and type observation.
    pub const fn frontend_abi_identity(&self) -> &[u8; 32] {
        &self.frontend_abi
    }

    /// Returns the exact symbolic plan schema.
    pub const fn symbolic_plan(&self) -> GeneralGemmSymbolicPlanV1 {
        self.symbolic_plan
    }

    /// Returns the exact symbolic KIR template.
    pub const fn symbolic_kir(&self) -> GeneralGemmSymbolicKirV1 {
        self.symbolic_kir
    }

    /// Returns the aggregate frontend semantic observation identity.
    pub const fn identity(&self) -> GeneralGemmFrontendSemanticBindingIdentityV1 {
        self.identity
    }
}

impl GeneralGemmScheduleV1 {
    /// Returns the exact schedule identity. No proof or artifact evidence is shared.
    pub fn identity(self) -> GeneralGemmScheduleIdentityV1 {
        GeneralGemmScheduleIdentityV1(hash_fields(
            SCHEDULE_IDENTITY_DOMAIN_V1,
            &[&self.encode_canonical()],
        ))
    }

    /// Returns the closed canonical schedule encoding.
    pub fn encode_canonical(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(32);
        bytes.push(self as u8);
        bytes.extend_from_slice(&GENERAL_GEMM_KIR_TILE_EXTENT_V1.to_le_bytes());
        bytes.extend_from_slice(&GENERAL_GEMM_KIR_WAVE_LANES_V1.to_le_bytes());
        bytes.extend_from_slice(&GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1.to_le_bytes());
        bytes.extend_from_slice(&GENERAL_GEMM_KIR_LDS_ELEMENTS_V1.to_le_bytes());
        bytes.push(1); // Single-buffered LDS pipeline.
        match self {
            Self::ReferenceWave64Xor4V1 => {
                bytes.extend_from_slice(&[1, 1, 1, 0]);
            }
            Self::VectorizedAOnlyBf16GlobalTransferV1 => {
                // A: v4 only under 8-byte alignment and a full-vector predicate.
                // A tails and every B component retain scalar masked transfers.
                bytes.extend_from_slice(&[4, 8, 1, 1]);
            }
        }
        bytes
    }

    /// Returns whether this schedule requires post-lowering ISA confirmation.
    pub const fn requires_vectorized_a_isa_confirmation(self) -> bool {
        matches!(self, Self::VectorizedAOnlyBf16GlobalTransferV1)
    }

    /// Returns the BF16 element width of an admitted full A transfer.
    pub const fn a_full_transfer_width_bf16(self) -> u8 {
        match self {
            Self::ReferenceWave64Xor4V1 => 1,
            Self::VectorizedAOnlyBf16GlobalTransferV1 => 4,
        }
    }

    /// Returns the byte alignment required for the full A transfer path.
    pub const fn a_full_transfer_alignment_bytes(self) -> u8 {
        match self {
            Self::ReferenceWave64Xor4V1 => 2,
            Self::VectorizedAOnlyBf16GlobalTransferV1 => 8,
        }
    }

    /// Every out-of-domain or incomplete A vector uses scalar masked fallback.
    pub const fn has_scalar_a_tail_fallback(self) -> bool {
        true
    }

    /// B components vary along K and remain scalar strided transfers.
    pub const fn b_transfer_width_bf16(self) -> u8 {
        1
    }
}

/// Untrusted runtime ABI values before they are checked against the host plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmRuntimeAbiSnapshotV1 {
    /// Runtime A slice length in BF16 elements.
    pub a_elements: u64,
    /// Runtime B slice length in BF16 elements.
    pub b_elements: u64,
    /// Runtime C disjoint-slice length in FP32 elements.
    pub c_elements: u64,
    /// Runtime `[M, N, K]` scalars.
    pub dimensions: [u32; 3],
    /// Runtime `[lda, ldb, ldc]` scalars.
    pub strides: [u32; 3],
    /// Exact runtime alpha bits.
    pub alpha_bits: u32,
    /// Exact runtime beta bits.
    pub beta_bits: u32,
}

/// A runtime ABI field conflicted with the independently checked host plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmRuntimeAbiErrorV1 {
    /// A, B, or C element length differs from the checked accessed extent.
    StorageElements,
    /// M, N, or K differs from the checked dimensions.
    Dimensions,
    /// lda, ldb, or ldc differs from the checked row strides.
    Strides,
    /// Runtime alpha bits differ from the checked plan.
    Alpha,
    /// Runtime beta bits differ from the checked plan.
    Beta,
}

impl fmt::Display for GeneralGemmRuntimeAbiErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "general GEMM runtime ABI substituted {self:?}")
    }
}

impl std::error::Error for GeneralGemmRuntimeAbiErrorV1 {}

/// Exact scalarized runtime ABI values bound to one checked host plan.
///
/// Buffer addresses remain launch-time provenance and are deliberately not a
/// durable compiler identity. Their semantic slots, lengths, pointee types,
/// mutability, and every scalar operand are fixed by this record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmRuntimeAbiV1 {
    snapshot: GeneralGemmRuntimeAbiSnapshotV1,
    identity: GeneralGemmRuntimeAbiIdentityV1,
}

impl GeneralGemmRuntimeAbiV1 {
    /// Checks every dynamic ABI value against the independently checked plan.
    pub fn checked(
        plan: GeneralGemmPlanFieldsV1,
        snapshot: GeneralGemmRuntimeAbiSnapshotV1,
    ) -> Result<Self, GeneralGemmRuntimeAbiErrorV1> {
        if [
            snapshot.a_elements,
            snapshot.b_elements,
            snapshot.c_elements,
        ] != plan.storage_elements()
        {
            return Err(GeneralGemmRuntimeAbiErrorV1::StorageElements);
        }
        if snapshot.dimensions != plan.dimensions() {
            return Err(GeneralGemmRuntimeAbiErrorV1::Dimensions);
        }
        if snapshot.strides != plan.strides() {
            return Err(GeneralGemmRuntimeAbiErrorV1::Strides);
        }
        if snapshot.alpha_bits != plan.alpha_bits() {
            return Err(GeneralGemmRuntimeAbiErrorV1::Alpha);
        }
        if snapshot.beta_bits != plan.beta_bits() {
            return Err(GeneralGemmRuntimeAbiErrorV1::Beta);
        }
        let identity = GeneralGemmRuntimeAbiIdentityV1(hash_fields(
            ABI_IDENTITY_DOMAIN_V1,
            &[&encode_runtime_abi(snapshot)],
        ));
        Ok(Self { snapshot, identity })
    }

    /// Derives the exact ABI values directly from a checked host plan.
    pub fn from_plan(plan: GeneralGemmPlanFieldsV1) -> Self {
        let [a_elements, b_elements, c_elements] = plan.storage_elements();
        Self::checked(
            plan,
            GeneralGemmRuntimeAbiSnapshotV1 {
                a_elements,
                b_elements,
                c_elements,
                dimensions: plan.dimensions(),
                strides: plan.strides(),
                alpha_bits: plan.alpha_bits(),
                beta_bits: plan.beta_bits(),
            },
        )
        .expect("ABI values derived from a checked plan are consistent")
    }

    /// Returns the exact runtime ABI commitment.
    pub const fn identity(self) -> GeneralGemmRuntimeAbiIdentityV1 {
        self.identity
    }

    /// Returns the checked ABI field values.
    pub const fn snapshot(self) -> GeneralGemmRuntimeAbiSnapshotV1 {
        self.snapshot
    }
}

/// Hard-bounded resource policy for the pre-artifact lowering route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmLoweringLimitsV1 {
    max_kir_bytes: usize,
    max_pliron_operations: usize,
}

/// A configured lowering limit is zero or above its hard maximum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmLoweringLimitErrorV1 {
    /// The maximum KIR byte count is invalid.
    KirBytes,
    /// The maximum Pliron operation count is invalid.
    PlironOperations,
}

impl GeneralGemmLoweringLimitsV1 {
    /// Creates caller limits constrained by immutable implementation ceilings.
    pub const fn new(
        max_kir_bytes: usize,
        max_pliron_operations: usize,
    ) -> Result<Self, GeneralGemmLoweringLimitErrorV1> {
        if max_kir_bytes == 0 || max_kir_bytes > MAX_GENERAL_GEMM_KIR_BYTES_V1 {
            return Err(GeneralGemmLoweringLimitErrorV1::KirBytes);
        }
        if max_pliron_operations == 0
            || max_pliron_operations > MAX_GENERAL_GEMM_PLIRON_OPERATIONS_V1
        {
            return Err(GeneralGemmLoweringLimitErrorV1::PlironOperations);
        }
        Ok(Self {
            max_kir_bytes,
            max_pliron_operations,
        })
    }

    /// Returns the active complete-KIR byte limit.
    pub const fn max_kir_bytes(self) -> usize {
        self.max_kir_bytes
    }

    /// Returns the active typed-operation limit.
    pub const fn max_pliron_operations(self) -> usize {
        self.max_pliron_operations
    }
}

impl Default for GeneralGemmLoweringLimitsV1 {
    fn default() -> Self {
        Self {
            max_kir_bytes: MAX_GENERAL_GEMM_KIR_BYTES_V1,
            max_pliron_operations: MAX_GENERAL_GEMM_PLIRON_OPERATIONS_V1,
        }
    }
}

/// Exact pinned route metadata retained in every compilation binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmToolchainRouteV1 {
    identity: GeneralGemmToolchainRouteIdentityV1,
}

impl GeneralGemmToolchainRouteV1 {
    /// Returns the route fixed by this crate's reviewed dependency stack.
    pub fn reviewed_v1() -> Self {
        let identity = GeneralGemmToolchainRouteIdentityV1(hash_fields(
            TOOLCHAIN_IDENTITY_DOMAIN_V1,
            &[
                PLIRON_REVISION.as_bytes(),
                GENERAL_GEMM_DEVICE_TARGET_V1.as_bytes(),
                GFX942_AMDHSA_TARGET_TRIPLE_V1.as_bytes(),
                GFX942_AMDHSA_DATA_LAYOUT_V1.as_bytes(),
                b"fe2o3-llvm-handoff-v2",
                b"fe2o3-llvm-text-v2",
                b"fe2o3-compiler-module-handoff-v2",
                b"fe2o3-worker-v2",
            ],
        ));
        Self { identity }
    }

    /// Returns the exact route identity.
    pub const fn identity(self) -> GeneralGemmToolchainRouteIdentityV1 {
        self.identity
    }
}

/// A compile request, plan, KIR, schedule, ABI, or limit failed closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmCompilationBindingErrorV1 {
    /// Only the explicit candidate-producing Pliron V1 route is accepted.
    PipelineSelector,
    /// One required request commitment is the all-zero untrusted sentinel.
    ZeroRequestCommitment,
    /// The request kernel differs from the consumed frontend receipt.
    FrontendKernelSubstitution,
    /// The KIR exceeded the active byte limit.
    KirBytesLimit {
        /// Observed complete KIR byte count.
        actual: usize,
        /// Active caller-selected maximum.
        maximum: usize,
    },
    /// The fixed projection exceeds the active operation limit.
    PlironOperationsLimit {
        /// Operations required by the closed projection.
        required: usize,
        /// Active caller-selected maximum.
        maximum: usize,
    },
    /// The KIR plan differs from the independently supplied checked host plan.
    PlanSubstitution,
    /// The KIR is not the exact verified conservative semantic schedule.
    NonCanonicalKir,
    /// Structured KIR verification found an exact counterexample.
    SemanticKir(GeneralGemmKirDiagnosticV1),
    /// KIR and compiler-driver property vocabularies did not agree.
    SemanticSchema,
    /// The runtime ABI differs from the checked host plan.
    RuntimeAbiSubstitution,
    /// The request's exact source/KIR obligation commitment differs.
    ObligationSetSubstitution,
}

impl fmt::Display for GeneralGemmCompilationBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid general GEMM compilation binding: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmCompilationBindingErrorV1 {}

/// Derives the exact symbolic obligation commitment for a runtime-parameterized GEMM source.
///
/// The commitment binds the authenticated frontend snapshot and canonical
/// symbolic plan/KIR schemas. It contains no witness launch dimensions,
/// strides, storage lengths, alpha, or beta.
pub fn general_gemm_symbolic_obligation_set_identity_v1(
    input: &StageSnapshotV1,
    frontend: &GeneralGemmFrontendSemanticBindingV1,
) -> ObligationSetIdentityV1 {
    ObligationSetIdentityV1::from_untrusted_bytes(hash_fields(
        SYMBOLIC_OBLIGATION_SET_IDENTITY_DOMAIN_V1,
        &[
            input.identity().as_bytes(),
            input.format_identity().as_bytes(),
            frontend.identity().as_bytes(),
            frontend.symbolic_plan().identity().as_bytes(),
            frontend.symbolic_kir().identity().as_bytes(),
            &encode_pliron_operation_schema(),
        ],
    ))
}

/// Derives the exact request configuration for one closed symbolic schedule.
///
/// Cargo/rustc must place this value in the compile request. The symbolic unit
/// re-derives it from the selected schedule, preventing a caller from relabeling
/// reference and A-vectorized machine evidence after source collection.
pub fn general_gemm_symbolic_pipeline_configuration_identity_v1(
    schedule: GeneralGemmScheduleV1,
) -> PipelineConfigurationIdentityV1 {
    PipelineConfigurationIdentityV1::from_untrusted_bytes(hash_fields(
        SYMBOLIC_PIPELINE_CONFIGURATION_IDENTITY_DOMAIN_V1,
        &[
            schedule.identity().as_bytes(),
            &schedule.encode_canonical(),
            GENERAL_GEMM_DEVICE_TARGET_V1.as_bytes(),
            GFX942_AMDHSA_TARGET_TRIPLE_V1.as_bytes(),
        ],
    ))
}

/// A production symbolic compilation input failed closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmSymbolicCompilationErrorV1 {
    /// Only the explicit Pliron V1 route is accepted.
    PipelineSelector,
    /// One required request commitment is the all-zero sentinel.
    ZeroRequestCommitment,
    /// The request kernel differs from the consumed frontend receipt.
    FrontendKernelSubstitution,
    /// The request configuration does not name the selected closed schedule.
    ScheduleSelectionSubstitution,
    /// The request retained a concrete or unrelated obligation commitment.
    SymbolicObligationSetSubstitution,
    /// The authenticated template names another symbolic plan schema.
    SymbolicPlanSubstitution,
    /// The authenticated template names another symbolic KIR schema.
    SymbolicKirSubstitution,
    /// The symbolic template exceeds the active KIR-byte limit.
    KirBytesLimit {
        /// Observed canonical symbolic template bytes.
        actual: usize,
        /// Active maximum.
        maximum: usize,
    },
    /// The fixed projection exceeds the active operation limit.
    PlironOperationsLimit {
        /// Operations required by the closed projection.
        required: usize,
        /// Active maximum.
        maximum: usize,
    },
}

impl fmt::Display for GeneralGemmSymbolicCompilationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid symbolic general GEMM compilation: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmSymbolicCompilationErrorV1 {}

/// Runtime-parameterized production compilation unit derived from positive MIR.
///
/// This unit deliberately contains no concrete launch dimensions, strides,
/// storage lengths, alpha, or beta. The rustc integration must retain its
/// private non-Clone frontend correspondence while synchronously lowering and
/// inspecting this structural unit. The public record is descriptive and does
/// not itself grant source, proof, artifact, publication, load, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct GeneralGemmSymbolicCompilationUnitV1 {
    request: CompileRequestV1,
    frontend_semantics: GeneralGemmFrontendSemanticBindingV1,
    schedule: GeneralGemmScheduleV1,
    toolchain: GeneralGemmToolchainRouteV1,
    identity: GeneralGemmSymbolicCompilationIdentityV1,
    limits: GeneralGemmLoweringLimitsV1,
}

impl GeneralGemmSymbolicCompilationUnitV1 {
    /// Checks the authenticated symbolic source/template and exact compilation route.
    pub fn checked(
        request: &CompileRequestV1,
        frontend_semantics: GeneralGemmFrontendSemanticBindingV1,
        schedule: GeneralGemmScheduleV1,
        limits: GeneralGemmLoweringLimitsV1,
    ) -> Result<Self, GeneralGemmSymbolicCompilationErrorV1> {
        if request.selector() != PipelineSelectorV1::PlironV1 {
            return Err(GeneralGemmSymbolicCompilationErrorV1::PipelineSelector);
        }
        if request_commitments(request).iter().any(is_zero_identity) {
            return Err(GeneralGemmSymbolicCompilationErrorV1::ZeroRequestCommitment);
        }
        if request.kernel_instance_identity().as_bytes()
            != frontend_semantics.kernel_instance_identity()
        {
            return Err(GeneralGemmSymbolicCompilationErrorV1::FrontendKernelSubstitution);
        }
        if request.pipeline_configuration_identity()
            != general_gemm_symbolic_pipeline_configuration_identity_v1(schedule)
        {
            return Err(GeneralGemmSymbolicCompilationErrorV1::ScheduleSelectionSubstitution);
        }
        if frontend_semantics.symbolic_plan() != GeneralGemmSymbolicPlanV1::canonical() {
            return Err(GeneralGemmSymbolicCompilationErrorV1::SymbolicPlanSubstitution);
        }
        if frontend_semantics.symbolic_kir() != GeneralGemmSymbolicKirV1::canonical() {
            return Err(GeneralGemmSymbolicCompilationErrorV1::SymbolicKirSubstitution);
        }
        if request.input_obligations_identity()
            != general_gemm_symbolic_obligation_set_identity_v1(
                request.input(),
                &frontend_semantics,
            )
        {
            return Err(GeneralGemmSymbolicCompilationErrorV1::SymbolicObligationSetSubstitution);
        }
        let symbolic_bytes = encode_symbolic_kir_template(&frontend_semantics);
        if symbolic_bytes.len() > limits.max_kir_bytes {
            return Err(GeneralGemmSymbolicCompilationErrorV1::KirBytesLimit {
                actual: symbolic_bytes.len(),
                maximum: limits.max_kir_bytes,
            });
        }
        if GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1 > limits.max_pliron_operations {
            return Err(
                GeneralGemmSymbolicCompilationErrorV1::PlironOperationsLimit {
                    required: GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1,
                    maximum: limits.max_pliron_operations,
                },
            );
        }
        let toolchain = GeneralGemmToolchainRouteV1::reviewed_v1();
        let identity = GeneralGemmSymbolicCompilationIdentityV1(hash_fields(
            SYMBOLIC_COMPILATION_IDENTITY_DOMAIN_V1,
            &[
                GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1.as_bytes(),
                request.identity().as_bytes(),
                request.kernel_instance_identity().as_bytes(),
                request.input().identity().as_bytes(),
                request.input().format_identity().as_bytes(),
                request.input_obligations_identity().as_bytes(),
                request.compiler_profile_identity().as_bytes(),
                request.target_profile_identity().as_bytes(),
                request.pipeline_configuration_identity().as_bytes(),
                frontend_semantics.identity().as_bytes(),
                frontend_semantics.symbolic_plan().identity().as_bytes(),
                frontend_semantics.symbolic_kir().identity().as_bytes(),
                schedule.identity().as_bytes(),
                toolchain.identity().as_bytes(),
            ],
        ));
        Ok(Self {
            request: request.clone(),
            frontend_semantics,
            schedule,
            toolchain,
            identity,
            limits,
        })
    }

    /// Returns the exact symbolic compile request.
    pub const fn request(&self) -> &CompileRequestV1 {
        &self.request
    }

    /// Returns identities retained from the consumed authenticated frontend receipt.
    pub const fn frontend_semantics(&self) -> &GeneralGemmFrontendSemanticBindingV1 {
        &self.frontend_semantics
    }

    /// Returns the aggregate frontend semantic observation identity.
    pub const fn frontend_semantic_binding_identity(
        &self,
    ) -> GeneralGemmFrontendSemanticBindingIdentityV1 {
        self.frontend_semantics.identity()
    }

    /// Returns the canonical symbolic plan identity.
    pub const fn symbolic_plan_identity(&self) -> GeneralGemmSymbolicPlanIdentityV1 {
        self.frontend_semantics.symbolic_plan().identity()
    }

    /// Returns the canonical symbolic KIR identity.
    pub const fn symbolic_kir_identity(&self) -> GeneralGemmSymbolicKirIdentityV1 {
        self.frontend_semantics.symbolic_kir().identity()
    }

    /// Returns the independently identified machine schedule.
    pub const fn schedule(&self) -> GeneralGemmScheduleV1 {
        self.schedule
    }

    /// Returns the independently identified machine schedule identity.
    pub fn schedule_identity(&self) -> GeneralGemmScheduleIdentityV1 {
        self.schedule.identity()
    }

    /// Returns the exact target/toolchain route identity.
    pub const fn toolchain_route_identity(&self) -> GeneralGemmToolchainRouteIdentityV1 {
        self.toolchain.identity()
    }

    /// Returns the aggregate symbolic compilation identity.
    pub const fn identity(&self) -> GeneralGemmSymbolicCompilationIdentityV1 {
        self.identity
    }

    /// Returns the bounded structural lowering limits.
    pub const fn limits(&self) -> GeneralGemmLoweringLimitsV1 {
        self.limits
    }

    /// Public structural data grants no production authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    /// Derives the parameterized schedule-proof request from this symbolic unit.
    ///
    /// Concrete launch dimensions, strides, coefficients, plan, KIR, and ABI
    /// values cannot enter this mapping. No caller-supplied identity is accepted.
    pub fn symbolic_schedule_proof_request(
        &self,
    ) -> Result<GeneralGemmProofRequestV1, GeneralGemmProofExecutionErrorV1> {
        let schedule = match self.schedule {
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
                GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1
            }
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
                GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
            }
        };
        GeneralGemmProofRequestV1::checked(
            schedule,
            proof_identity(self.schedule_identity().into_bytes()),
            proof_identity(self.symbolic_plan_identity().into_bytes()),
            proof_identity(self.symbolic_kir_identity().into_bytes()),
            proof_identity(self.identity.into_bytes()),
            proof_identity(self.request.identity().into_bytes()),
            proof_identity(self.request.input_obligations_identity().into_bytes()),
            proof_identity(self.request.compiler_profile_identity().into_bytes()),
            proof_identity(self.request.target_profile_identity().into_bytes()),
            proof_identity(self.toolchain.identity().into_bytes()),
            proof_identity(self.frontend_semantics.identity().into_bytes()),
            proof_identity(*self.frontend_semantics.provider_semantics_identity()),
        )
    }
}

/// A concrete launch failed to instantiate the exact symbolic artifact schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmCheckedLaunchInstantiationErrorV1 {
    /// No inspected symbolic artifact was supplied.
    ZeroArtifactIdentity,
    /// The concrete KIR names another checked host plan.
    PlanSubstitution,
    /// The concrete KIR is not the canonical instantiation of the plan.
    NonCanonicalKir,
    /// Concrete semantic KIR verification rejected the instantiation.
    SemanticKir(GeneralGemmKirDiagnosticV1),
    /// Runtime values differ from the concrete checked host plan.
    RuntimeAbi(GeneralGemmRuntimeAbiErrorV1),
}

impl fmt::Display for GeneralGemmCheckedLaunchInstantiationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid general GEMM symbolic-artifact launch instantiation: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmCheckedLaunchInstantiationErrorV1 {}

/// Concrete checked launch values bound to one symbolic machine artifact.
///
/// This record rechecks the complete plan/KIR/runtime snapshot relation at
/// launch time. It is still descriptive: protected runtime admission must
/// additionally consume the opaque post-link artifact token and compare the
/// exact symbolic artifact identity before loading or launching code.
#[derive(Debug, Eq, PartialEq)]
pub struct GeneralGemmCheckedLaunchInstantiationV1 {
    symbolic_compilation: GeneralGemmSymbolicCompilationIdentityV1,
    symbolic_artifact: GeneralGemmSymbolicArtifactIdentityV1,
    symbolic_plan: GeneralGemmSymbolicPlanIdentityV1,
    symbolic_kir: GeneralGemmSymbolicKirIdentityV1,
    plan: GeneralGemmPlanFieldsV1,
    plan_identity: GeneralGemmPlanIdentityV1,
    kir: GeneralGemmKirV1,
    abi: GeneralGemmRuntimeAbiV1,
    identity: GeneralGemmCheckedLaunchInstantiationIdentityV1,
}

impl GeneralGemmCheckedLaunchInstantiationV1 {
    /// Rechecks and binds one concrete runtime snapshot to a symbolic artifact.
    pub fn checked(
        unit: &GeneralGemmSymbolicCompilationUnitV1,
        symbolic_artifact: GeneralGemmSymbolicArtifactIdentityV1,
        plan: GeneralGemmPlanFieldsV1,
        kir: GeneralGemmKirV1,
        snapshot: GeneralGemmRuntimeAbiSnapshotV1,
    ) -> Result<Self, GeneralGemmCheckedLaunchInstantiationErrorV1> {
        if is_zero_identity(symbolic_artifact.as_bytes()) {
            return Err(GeneralGemmCheckedLaunchInstantiationErrorV1::ZeroArtifactIdentity);
        }
        if kir.plan() != plan {
            return Err(GeneralGemmCheckedLaunchInstantiationErrorV1::PlanSubstitution);
        }
        verify_general_gemm_kir_v1(&kir)
            .map_err(GeneralGemmCheckedLaunchInstantiationErrorV1::SemanticKir)?;
        if kir != GeneralGemmKirV1::canonical(plan) {
            return Err(GeneralGemmCheckedLaunchInstantiationErrorV1::NonCanonicalKir);
        }
        let abi = GeneralGemmRuntimeAbiV1::checked(plan, snapshot)
            .map_err(GeneralGemmCheckedLaunchInstantiationErrorV1::RuntimeAbi)?;
        let plan_identity = plan_identity(plan);
        let identity = GeneralGemmCheckedLaunchInstantiationIdentityV1(hash_fields(
            CHECKED_LAUNCH_IDENTITY_DOMAIN_V1,
            &[
                unit.identity().as_bytes(),
                symbolic_artifact.as_bytes(),
                unit.symbolic_plan_identity().as_bytes(),
                unit.symbolic_kir_identity().as_bytes(),
                plan_identity.as_bytes(),
                kir.identity().as_bytes(),
                abi.identity().as_bytes(),
            ],
        ));
        Ok(Self {
            symbolic_compilation: unit.identity(),
            symbolic_artifact,
            symbolic_plan: unit.symbolic_plan_identity(),
            symbolic_kir: unit.symbolic_kir_identity(),
            plan,
            plan_identity,
            kir,
            abi,
            identity,
        })
    }

    /// Returns the symbolic compilation identity.
    pub const fn symbolic_compilation_identity(&self) -> GeneralGemmSymbolicCompilationIdentityV1 {
        self.symbolic_compilation
    }

    /// Returns the exact inspected symbolic artifact identity.
    pub const fn symbolic_artifact_identity(&self) -> GeneralGemmSymbolicArtifactIdentityV1 {
        self.symbolic_artifact
    }

    /// Returns the symbolic checked-plan schema identity.
    pub const fn symbolic_plan_identity(&self) -> GeneralGemmSymbolicPlanIdentityV1 {
        self.symbolic_plan
    }

    /// Returns the symbolic source/KIR schema identity.
    pub const fn symbolic_kir_identity(&self) -> GeneralGemmSymbolicKirIdentityV1 {
        self.symbolic_kir
    }

    /// Returns the concrete checked host plan.
    pub const fn plan(&self) -> GeneralGemmPlanFieldsV1 {
        self.plan
    }

    /// Returns the concrete checked host-plan identity.
    pub const fn plan_identity(&self) -> GeneralGemmPlanIdentityV1 {
        self.plan_identity
    }

    /// Returns the canonical concrete semantic KIR.
    pub const fn kir(&self) -> &GeneralGemmKirV1 {
        &self.kir
    }

    /// Returns the concrete semantic KIR identity.
    pub fn kir_identity(&self) -> GeneralGemmKirIdentityV1 {
        self.kir.identity()
    }

    /// Returns the exact checked runtime ABI.
    pub const fn runtime_abi(&self) -> GeneralGemmRuntimeAbiV1 {
        self.abi
    }

    /// Returns the aggregate launch-instantiation identity.
    pub const fn identity(&self) -> GeneralGemmCheckedLaunchInstantiationIdentityV1 {
        self.identity
    }

    /// Structural launch instantiation grants no load or launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Concrete deterministic model binding retained for tests and launch instantiation checks.
///
/// This legacy concrete record is not a production source-correspondence or
/// artifact authority. Production symbolic lowering uses
/// [`GeneralGemmSymbolicCompilationUnitV1`].
#[derive(Debug, Eq, PartialEq)]
pub struct GeneralGemmCompilationUnitV1 {
    request: CompileRequestV1,
    frontend_semantics: GeneralGemmFrontendSemanticBindingV1,
    plan: GeneralGemmPlanFieldsV1,
    kir: GeneralGemmKirV1,
    schedule: GeneralGemmScheduleV1,
    abi: GeneralGemmRuntimeAbiV1,
    plan_identity: GeneralGemmPlanIdentityV1,
    toolchain: GeneralGemmToolchainRouteV1,
    identity: GeneralGemmCompilationBindingIdentityV1,
    limits: GeneralGemmLoweringLimitsV1,
}

impl GeneralGemmCompilationUnitV1 {
    /// Checks and binds all semantic, proof, schedule, ABI, and route inputs.
    pub fn checked(
        request: &CompileRequestV1,
        frontend_semantics: GeneralGemmFrontendSemanticBindingV1,
        plan: GeneralGemmPlanFieldsV1,
        kir: GeneralGemmKirV1,
        schedule: GeneralGemmScheduleV1,
        abi: GeneralGemmRuntimeAbiV1,
        limits: GeneralGemmLoweringLimitsV1,
    ) -> Result<Self, GeneralGemmCompilationBindingErrorV1> {
        if request.selector() != PipelineSelectorV1::PlironV1 {
            return Err(GeneralGemmCompilationBindingErrorV1::PipelineSelector);
        }
        if request_commitments(request).iter().any(is_zero_identity) {
            return Err(GeneralGemmCompilationBindingErrorV1::ZeroRequestCommitment);
        }
        if request.kernel_instance_identity().as_bytes()
            != frontend_semantics.kernel_instance_identity()
        {
            return Err(GeneralGemmCompilationBindingErrorV1::FrontendKernelSubstitution);
        }
        let kir_bytes = kir.encode_canonical();
        if kir_bytes.len() > limits.max_kir_bytes {
            return Err(GeneralGemmCompilationBindingErrorV1::KirBytesLimit {
                actual: kir_bytes.len(),
                maximum: limits.max_kir_bytes,
            });
        }
        if GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1 > limits.max_pliron_operations {
            return Err(
                GeneralGemmCompilationBindingErrorV1::PlironOperationsLimit {
                    required: GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1,
                    maximum: limits.max_pliron_operations,
                },
            );
        }
        if kir.plan() != plan {
            return Err(GeneralGemmCompilationBindingErrorV1::PlanSubstitution);
        }
        verify_general_gemm_kir_v1(&kir)
            .map_err(GeneralGemmCompilationBindingErrorV1::SemanticKir)?;
        if kir != GeneralGemmKirV1::canonical(plan) {
            return Err(GeneralGemmCompilationBindingErrorV1::NonCanonicalKir);
        }
        let program = GemmSemanticProgramV1::new(request, kir.clone())
            .map_err(|_| GeneralGemmCompilationBindingErrorV1::ObligationSetSubstitution)?;
        analyze_gemm_semantics_v1(&program)
            .map_err(|_| GeneralGemmCompilationBindingErrorV1::SemanticSchema)?;
        if abi != GeneralGemmRuntimeAbiV1::from_plan(plan) {
            return Err(GeneralGemmCompilationBindingErrorV1::RuntimeAbiSubstitution);
        }
        let expected_obligations =
            general_gemm_semantic_obligation_set_identity_v1(request.input().identity(), &kir);
        if request.input_obligations_identity() != expected_obligations {
            return Err(GeneralGemmCompilationBindingErrorV1::ObligationSetSubstitution);
        }

        let plan_identity = plan_identity(plan);
        let toolchain = GeneralGemmToolchainRouteV1::reviewed_v1();
        let identity = GeneralGemmCompilationBindingIdentityV1(hash_fields(
            BINDING_IDENTITY_DOMAIN_V1,
            &[
                GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1.as_bytes(),
                request.identity().as_bytes(),
                request.kernel_instance_identity().as_bytes(),
                request.input().identity().as_bytes(),
                request.input().format_identity().as_bytes(),
                request.input_obligations_identity().as_bytes(),
                request.compiler_profile_identity().as_bytes(),
                request.target_profile_identity().as_bytes(),
                request.pipeline_configuration_identity().as_bytes(),
                frontend_semantics.identity().as_bytes(),
                plan_identity.as_bytes(),
                kir.identity().as_bytes(),
                schedule.identity().as_bytes(),
                abi.identity().as_bytes(),
                toolchain.identity().as_bytes(),
            ],
        ));
        Ok(Self {
            request: request.clone(),
            frontend_semantics,
            plan,
            kir,
            schedule,
            abi,
            plan_identity,
            toolchain,
            identity,
            limits,
        })
    }

    /// Returns the exact compile request.
    pub const fn request(&self) -> &CompileRequestV1 {
        &self.request
    }

    /// Returns identities retained from the consumed authenticated frontend receipt.
    pub const fn frontend_semantics(&self) -> &GeneralGemmFrontendSemanticBindingV1 {
        &self.frontend_semantics
    }

    /// Returns the aggregate authenticated frontend semantic observation identity.
    pub const fn frontend_semantic_binding_identity(
        &self,
    ) -> GeneralGemmFrontendSemanticBindingIdentityV1 {
        self.frontend_semantics.identity()
    }

    /// Returns the checked host-plan fields.
    pub const fn plan(&self) -> GeneralGemmPlanFieldsV1 {
        self.plan
    }

    /// Returns the complete structured semantic KIR.
    pub const fn kir(&self) -> &GeneralGemmKirV1 {
        &self.kir
    }

    /// Returns the separately identified schedule.
    pub const fn schedule(&self) -> GeneralGemmScheduleV1 {
        self.schedule
    }

    /// Returns the checked runtime ABI.
    pub const fn runtime_abi(&self) -> GeneralGemmRuntimeAbiV1 {
        self.abi
    }

    /// Returns the exact plan identity.
    pub const fn plan_identity(&self) -> GeneralGemmPlanIdentityV1 {
        self.plan_identity
    }

    /// Returns the exact KIR identity.
    pub fn kir_identity(&self) -> GeneralGemmKirIdentityV1 {
        self.kir.identity()
    }

    /// Returns the exact schedule identity.
    pub fn schedule_identity(&self) -> GeneralGemmScheduleIdentityV1 {
        self.schedule.identity()
    }

    /// Returns the exact ABI identity.
    pub const fn runtime_abi_identity(&self) -> GeneralGemmRuntimeAbiIdentityV1 {
        self.abi.identity()
    }

    /// Returns the exact target/toolchain route identity.
    pub const fn toolchain_route_identity(&self) -> GeneralGemmToolchainRouteIdentityV1 {
        self.toolchain.identity()
    }

    /// Returns the aggregate proof-to-lowering binding identity.
    pub const fn identity(&self) -> GeneralGemmCompilationBindingIdentityV1 {
        self.identity
    }
}

/// Missing production integration that prevents candidate authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmProductionGapV1 {
    /// The production selector does not yet consume the complete Rust, live
    /// compiler-graph, late-machine verifier, and final-artifact identity join.
    AuthorityJoin,
}

/// Known later contracts that remain unavailable after typed LLVM handoff.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmPostLoweringGapV1 {
    /// The production route has not joined exact graph, request, response, and ISA identities.
    LateMachineVerifierIdentityJoin,
    /// The rustc-owned final join does not yet consume the complete identity chain.
    RustcOwnedFinalArtifactJoin,
    /// Publication and protected runtime admission do not yet consume this chain.
    TransactionalPublicationAndRuntimeAdmission,
}

/// Exact stage and gaps that stopped the candidate-producing route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmLoweringBlockerV1 {
    stage: CompilerStageV1,
    gaps: [GeneralGemmProductionGapV1; 1],
}

impl GeneralGemmLoweringBlockerV1 {
    /// Returns the first stage not connected to the production authority join.
    pub const fn stage(self) -> CompilerStageV1 {
        self.stage
    }

    /// Returns the fail-closed production integration gap.
    pub const fn gaps(self) -> [GeneralGemmProductionGapV1; 1] {
        self.gaps
    }
}

/// Successful owner-bound target-neutral Pliron projection before AMDGPU lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmPlironProjectionV1 {
    identity: GeneralGemmPlironProjectionIdentityV1,
    compilation_binding_identity: GeneralGemmCompilationBindingIdentityV1,
    schedule_identity: GeneralGemmScheduleIdentityV1,
    kir_identity: GeneralGemmKirIdentityV1,
    operation_count: usize,
}

/// Owner-checked symbolic Pliron projection used by production lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmSymbolicPlironProjectionV1 {
    identity: GeneralGemmPlironProjectionIdentityV1,
    source_operation_identity: GeneralGemmPlironSourceOperationIdentityV1,
    lowered_operation_identity: GeneralGemmPlironLoweredOperationIdentityV1,
    transformation_identity: GeneralGemmPlironTransformationIdentityV1,
    compilation_identity: GeneralGemmSymbolicCompilationIdentityV1,
    schedule_identity: GeneralGemmScheduleIdentityV1,
    symbolic_plan_identity: GeneralGemmSymbolicPlanIdentityV1,
    symbolic_kir_identity: GeneralGemmSymbolicKirIdentityV1,
    operation_count: usize,
}

impl GeneralGemmSymbolicPlironProjectionV1 {
    /// Returns the exact owner-checked projection identity.
    pub const fn identity(self) -> GeneralGemmPlironProjectionIdentityV1 {
        self.identity
    }

    /// Returns the verified high-level structured operation identity.
    pub const fn source_operation_identity(self) -> GeneralGemmPlironSourceOperationIdentityV1 {
        self.source_operation_identity
    }

    /// Returns the verified explicit GPU operation identity.
    pub const fn lowered_operation_identity(self) -> GeneralGemmPlironLoweredOperationIdentityV1 {
        self.lowered_operation_identity
    }

    /// Returns the exact verified source-to-GPU transformation identity.
    pub const fn transformation_identity(self) -> GeneralGemmPlironTransformationIdentityV1 {
        self.transformation_identity
    }

    /// Returns the symbolic compilation consumed by projection.
    pub const fn compilation_identity(self) -> GeneralGemmSymbolicCompilationIdentityV1 {
        self.compilation_identity
    }

    /// Returns the selected closed schedule identity.
    pub const fn schedule_identity(self) -> GeneralGemmScheduleIdentityV1 {
        self.schedule_identity
    }

    /// Returns the symbolic checked-plan schema identity.
    pub const fn symbolic_plan_identity(self) -> GeneralGemmSymbolicPlanIdentityV1 {
        self.symbolic_plan_identity
    }

    /// Returns the symbolic source/KIR schema identity.
    pub const fn symbolic_kir_identity(self) -> GeneralGemmSymbolicKirIdentityV1 {
        self.symbolic_kir_identity
    }

    /// Returns the exact verified operation count.
    pub const fn operation_count(self) -> usize {
        self.operation_count
    }

    /// Structural projection grants no artifact authority.
    pub const fn grants_artifact_authority(self) -> bool {
        false
    }
}

impl GeneralGemmPlironProjectionV1 {
    /// Returns the exact projection identity.
    pub const fn identity(self) -> GeneralGemmPlironProjectionIdentityV1 {
        self.identity
    }

    /// Returns the complete compilation binding consumed by projection.
    pub const fn compilation_binding_identity(self) -> GeneralGemmCompilationBindingIdentityV1 {
        self.compilation_binding_identity
    }

    /// Returns the schedule retained by projection.
    pub const fn schedule_identity(self) -> GeneralGemmScheduleIdentityV1 {
        self.schedule_identity
    }

    /// Returns the complete semantic KIR retained by projection.
    pub const fn kir_identity(self) -> GeneralGemmKirIdentityV1 {
        self.kir_identity
    }

    /// Returns the exact verified operation count.
    pub const fn operation_count(self) -> usize {
        self.operation_count
    }

    /// The projection is inert and grants no artifact or runtime authority.
    pub const fn grants_artifact_authority(self) -> bool {
        false
    }
}

/// Failure while deriving the exact compiler-owned descriptor source.
#[derive(Debug)]
pub enum GeneralGemmDescriptorSourceErrorV1 {
    /// A typed descriptor field was invalid.
    Descriptor(DescriptorValidationError),
    /// Canonical zero-digest descriptor-source encoding failed.
    Source(CompilerDescriptorSourceErrorV1),
}

impl fmt::Display for GeneralGemmDescriptorSourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Descriptor(error) => {
                write!(formatter, "invalid general GEMM descriptor: {error}")
            }
            Self::Source(error) => {
                write!(formatter, "invalid general GEMM descriptor source: {error}")
            }
        }
    }
}

impl std::error::Error for GeneralGemmDescriptorSourceErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Descriptor(error) => Some(error),
            Self::Source(error) => Some(error),
        }
    }
}

impl From<DescriptorValidationError> for GeneralGemmDescriptorSourceErrorV1 {
    fn from(value: DescriptorValidationError) -> Self {
        Self::Descriptor(value)
    }
}

impl From<CompilerDescriptorSourceErrorV1> for GeneralGemmDescriptorSourceErrorV1 {
    fn from(value: CompilerDescriptorSourceErrorV1) -> Self {
        Self::Source(value)
    }
}

/// Derives the canonical zero-digest descriptor source from one exact projection.
///
/// The result is structural compiler data. It authenticates no producer and
/// grants no worker, publication, loading, or launch authority.
pub fn derive_general_gemm_descriptor_source_v1(
    unit: &GeneralGemmCompilationUnitV1,
    projection: GeneralGemmPlironProjectionV1,
) -> Result<CompilerDescriptorSourceV1, GeneralGemmDescriptorSourceErrorV1> {
    if projection.compilation_binding_identity() != unit.identity()
        || projection.schedule_identity() != unit.schedule_identity()
        || projection.kir_identity() != unit.kir_identity()
    {
        return Err(GeneralGemmDescriptorSourceErrorV1::Descriptor(
            DescriptorValidationError::IdentityMismatch {
                field: "general GEMM Pliron projection",
            },
        ));
    }

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
        EvidenceIdentity::from_opaque_bytes(unit.frontend_semantic_binding_identity().into_bytes()),
        EvidenceDigest::from_sha256_bytes(*unit.frontend_semantics().compiled_source_identity()),
    );
    let executable_ir_evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(projection.identity().into_bytes()),
        EvidenceDigest::from_sha256_bytes(*unit.kir_identity().as_bytes()),
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(unit.identity().into_bytes()),
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
            Text::new("general-gemm-v1")?,
        ),
        target,
        vec![bf16_slice_type, c_slice_type, u32_type, f32_type],
        vec![bf16_slice_layout, c_slice_layout, u32_layout, f32_layout],
        vec![kernel],
    )?;
    CompilerDescriptorSourceV1::new(table).map_err(Into::into)
}

/// Fail-closed result after consuming the existing proof admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmLoweringObservationV1 {
    projection: GeneralGemmPlironProjectionV1,
    blocker: GeneralGemmLoweringBlockerV1,
}

impl GeneralGemmLoweringObservationV1 {
    /// Returns the verified owner-bound Pliron projection receipt.
    pub const fn projection(self) -> GeneralGemmPlironProjectionV1 {
        self.projection
    }

    /// Returns the exact missing production integration contract.
    pub const fn blocker(self) -> GeneralGemmLoweringBlockerV1 {
        self.blocker
    }

    /// Returns known later gaps without claiming that the route reached them.
    pub const fn post_lowering_gaps(self) -> [GeneralGemmPostLoweringGapV1; 3] {
        [
            GeneralGemmPostLoweringGapV1::LateMachineVerifierIdentityJoin,
            GeneralGemmPostLoweringGapV1::RustcOwnedFinalArtifactJoin,
            GeneralGemmPostLoweringGapV1::TransactionalPublicationAndRuntimeAdmission,
        ]
    }

    /// No Handoff V2 was produced.
    pub const fn handoff_v2_identity(self) -> Option<HandoffIdentityV2> {
        None
    }

    /// No LLVM serializer bytes were produced.
    pub const fn llvm_assembly_identity(self) -> Option<LlvmAssemblySha256V2> {
        None
    }

    /// No compiler-worker handoff was produced.
    pub const fn compiler_handoff_identity(self) -> Option<CompilerModuleHandoffIdentityV2> {
        None
    }

    /// No executable candidate was produced.
    pub const fn candidate_identity(self) -> Option<CandidateIdentityV1> {
        None
    }
}

/// Eventual all-stage artifact binding. This crate intentionally exposes no
/// constructor until the missing typed Handoff V2 machine contracts exist.
#[derive(Debug)]
pub struct GeneralGemmArtifactBindingV1 {
    identity: GeneralGemmArtifactBindingIdentityV1,
    compilation_binding_identity: GeneralGemmCompilationBindingIdentityV1,
    projection_identity: GeneralGemmPlironProjectionIdentityV1,
    handoff_identity: HandoffIdentityV2,
    assembly_identity: LlvmAssemblySha256V2,
    assembly_len: u64,
    compiler_handoff_identity: CompilerModuleHandoffIdentityV2,
    candidate_identity: CandidateIdentityV1,
}

impl GeneralGemmArtifactBindingV1 {
    /// Returns the all-stage artifact binding identity.
    pub const fn identity(&self) -> GeneralGemmArtifactBindingIdentityV1 {
        self.identity
    }

    /// Returns the proof-gated compilation binding.
    pub const fn compilation_binding_identity(&self) -> GeneralGemmCompilationBindingIdentityV1 {
        self.compilation_binding_identity
    }

    /// Returns the owner-checked Pliron projection identity.
    pub const fn projection_identity(&self) -> GeneralGemmPlironProjectionIdentityV1 {
        self.projection_identity
    }

    /// Returns the canonical typed LLVM handoff identity.
    pub const fn handoff_identity(&self) -> HandoffIdentityV2 {
        self.handoff_identity
    }

    /// Returns the exact LLVM assembly content digest.
    pub const fn assembly_identity(&self) -> LlvmAssemblySha256V2 {
        self.assembly_identity
    }

    /// Returns the exact LLVM assembly byte length.
    pub const fn assembly_len(&self) -> u64 {
        self.assembly_len
    }

    /// Returns the canonical compiler-worker handoff identity.
    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.compiler_handoff_identity
    }

    /// Returns the transactional executable-candidate identity.
    pub const fn candidate_identity(&self) -> CandidateIdentityV1 {
        self.candidate_identity
    }
}

/// Proof-gated backend failures before a transactional rejection is available.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmAdmittedLoweringErrorV1 {
    /// The supplied request differs from the compilation unit.
    RequestSubstitution,
    /// The proof gate names another request.
    AdmissionRequestSubstitution,
    /// The proof gate names another obligation set.
    AdmissionObligationSubstitution,
    /// The one-shot compilation unit has already been consumed.
    Replay,
    /// Pliron construction, ownership, or typed verification failed.
    PlironProjection,
}

impl fmt::Display for GeneralGemmAdmittedLoweringErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "general GEMM admitted lowering failed: {self:?}")
    }
}

impl std::error::Error for GeneralGemmAdmittedLoweringErrorV1 {}

/// One-shot backend that begins projection only after proof admission.
#[derive(Debug)]
pub struct GeneralGemmAdmittedBackendV1 {
    unit: Option<GeneralGemmCompilationUnitV1>,
}

impl GeneralGemmAdmittedBackendV1 {
    /// Installs one exact checked compilation unit.
    pub const fn new(unit: GeneralGemmCompilationUnitV1) -> Self {
        Self { unit: Some(unit) }
    }

    /// Consumes proof admission, constructs the real Pliron projection, and
    /// returns the honest production authority-join blocker.
    pub fn lower_admitted(
        &mut self,
        request: &CompileRequestV1,
        admission: ProofRequiredGemmAdmissionV1,
    ) -> Result<GeneralGemmLoweringObservationV1, GeneralGemmAdmittedLoweringErrorV1> {
        let unit = self
            .unit
            .as_ref()
            .ok_or(GeneralGemmAdmittedLoweringErrorV1::Replay)?;
        if unit.request != *request {
            return Err(GeneralGemmAdmittedLoweringErrorV1::RequestSubstitution);
        }
        if admission.request_identity() != unit.request.identity() {
            return Err(GeneralGemmAdmittedLoweringErrorV1::AdmissionRequestSubstitution);
        }
        if admission.obligation_set_identity() != unit.request.input_obligations_identity() {
            return Err(GeneralGemmAdmittedLoweringErrorV1::AdmissionObligationSubstitution);
        }
        let unit = self.unit.take().expect("checked above");
        let envelope = project_to_pliron(&unit)
            .map_err(|_| GeneralGemmAdmittedLoweringErrorV1::PlironProjection)?;
        envelope
            .validate_exact(&unit)
            .map_err(|_| GeneralGemmAdmittedLoweringErrorV1::PlironProjection)?;
        let projection = envelope.receipt;
        Ok(GeneralGemmLoweringObservationV1 {
            projection,
            blocker: GeneralGemmLoweringBlockerV1 {
                stage: CompilerStageV1::Llvm,
                gaps: [GeneralGemmProductionGapV1::AuthorityJoin],
            },
        })
    }
}

impl AdmittedGemmCompilerBackendV1 for GeneralGemmAdmittedBackendV1 {
    fn compile_admitted(
        &mut self,
        request: &CompileRequestV1,
        admission: ProofRequiredGemmAdmissionV1,
    ) -> Result<CompileOutputV1, CompilerBackendFailureV1> {
        self.lower_admitted(request, admission)
            .map_err(|error| match error {
                GeneralGemmAdmittedLoweringErrorV1::Replay => CompilerBackendFailureV1::Internal,
                GeneralGemmAdmittedLoweringErrorV1::PlironProjection => {
                    CompilerBackendFailureV1::Internal
                }
                GeneralGemmAdmittedLoweringErrorV1::RequestSubstitution
                | GeneralGemmAdmittedLoweringErrorV1::AdmissionRequestSubstitution
                | GeneralGemmAdmittedLoweringErrorV1::AdmissionObligationSubstitution => {
                    CompilerBackendFailureV1::UnsupportedRequest
                }
            })?;
        let diagnostic = CanonicalDiagnosticV1::new(
            0,
            DiagnosticCodeV1::new(GENERAL_GEMM_LOWERING_BLOCKED_CODE_V1)
                .expect("static diagnostic code is nonzero"),
            DiagnosticSeverityV1::Error,
            Some(CompilerStageV1::Llvm),
            Some(DiagnosticSubjectIdentityV1::from_untrusted_bytes(
                request.kernel_instance_identity().into_bytes(),
            )),
            DiagnosticMessageV1::new(GENERAL_GEMM_LOWERING_BLOCKED_MESSAGE_V1)
                .expect("static diagnostic message is canonical"),
        );
        CompileOutputV1::new(
            request,
            CompileDispositionV1::Rejected,
            Vec::new(),
            Vec::new(),
            vec![diagnostic],
            None,
        )
        .map_err(|_| CompilerBackendFailureV1::Internal)
    }
}

struct GeneralGemmPlironEnvelope {
    context: Context,
    context_identity: ContextIdentity,
    module: ModuleOp,
    receipt: GeneralGemmPlironProjectionV1,
}

impl GeneralGemmPlironEnvelope {
    fn validate_owner(&self) -> Result<(), ContextIdentityError> {
        self.validate_owner_in(&self.context)
    }

    fn validate_owner_in(&self, context: &Context) -> Result<(), ContextIdentityError> {
        let current = require_context_identity(context)?;
        if current != self.context_identity
            || verify_operation(self.module.get_operation(), context).is_err()
        {
            return Err(ContextIdentityError::CorruptMarker);
        }
        Ok(())
    }

    fn validate_exact(&self, unit: &GeneralGemmCompilationUnitV1) -> Result<(), ()> {
        self.validate_owner().map_err(|_| ())?;
        let module_binding = self.module.get_operation();
        let module = module_binding.deref(&self.context);
        if module.attributes.0.len() != 5
            || !metadata_matches(
                &module.attributes,
                PLIRON_SCHEMA_ATTR,
                GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1.as_bytes(),
            )
            || !metadata_matches(
                &module.attributes,
                PLIRON_BINDING_ATTR,
                unit.identity.as_bytes(),
            )
            || !metadata_matches(
                &module.attributes,
                PLIRON_KIR_ATTR,
                &unit.kir.encode_canonical(),
            )
            || !metadata_matches(
                &module.attributes,
                PLIRON_SCHEDULE_ATTR,
                unit.schedule.identity().as_bytes(),
            )
            || module
                .attributes
                .0
                .get(&*ATTR_KEY_SYM_NAME)
                .and_then(|attribute| attribute.downcast_ref::<IdentifierAttr>())
                .map(AsRef::as_ref)
                .is_none_or(|symbol| symbol.as_ref() != PLIRON_MODULE_SYMBOL)
        {
            return Err(());
        }
        drop(module);

        let body = self.module.get_body(&self.context, 0);
        let body = body.deref(&self.context);
        let mut operations = body.iter(&self.context);
        let algorithm = typed_next::<AlgorithmOp>(&mut operations, &self.context)?;
        if algorithm
            .iteration_domain(&self.context)
            .is_none_or(|domain| domain.rank() != 3)
        {
            return Err(());
        }
        let schedule = typed_next::<PlanOp>(&mut operations, &self.context)?;
        if schedule.parameters(&self.context).is_none_or(|parameters| {
            parameters.rank() != 3
                || parameters.tile_extent() != GENERAL_GEMM_KIR_TILE_EXTENT_V1
                || parameters.pipeline_stages() != 1
        }) {
            return Err(());
        }
        let tile = typed_next::<MaterializeOp>(&mut operations, &self.context)?;
        if tile.distribution(&self.context).is_none_or(|distribution| {
            distribution.rank() != 2
                || distribution.lanes() != GENERAL_GEMM_KIR_WAVE_LANES_V1
                || distribution.elements_per_lane() != GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1
        }) {
            return Err(());
        }
        for expected in [
            HierarchyAttr::Grid,
            HierarchyAttr::Workgroup,
            HierarchyAttr::Subgroup,
            HierarchyAttr::Lane,
        ] {
            let operation = typed_next::<HierarchyIdOp>(&mut operations, &self.context)?;
            if operation
                .get_attr_gpu_hierarchy_id_hierarchy(&self.context)
                .is_none_or(|actual| *actual != expected)
            {
                return Err(());
            }
        }
        for expected in [AddressSpaceAttr::Global, AddressSpaceAttr::Workgroup] {
            let operation = typed_next::<MemorySpaceOp>(&mut operations, &self.context)?;
            if operation
                .get_attr_gpu_memory_space_address_space(&self.context)
                .is_none_or(|actual| *actual != expected)
            {
                return Err(());
            }
        }
        for _ in 0..2 {
            let barrier = typed_next::<BarrierOp>(&mut operations, &self.context)?;
            if barrier
                .get_attr_gpu_barrier_execution_scope(&self.context)
                .is_none_or(|value| *value != HierarchyAttr::Workgroup)
                || barrier
                    .get_attr_gpu_barrier_memory_scope(&self.context)
                    .is_none_or(|value| *value != MemoryScopeAttr::Workgroup)
                || barrier
                    .get_attr_gpu_barrier_address_space(&self.context)
                    .is_none_or(|value| *value != AddressSpaceAttr::Workgroup)
                || barrier
                    .get_attr_gpu_barrier_order(&self.context)
                    .is_none_or(|value| *value != MemoryOrderAttr::AcquireRelease)
            {
                return Err(());
            }
        }
        if operations.next().is_some() {
            return Err(());
        }
        Ok(())
    }
}

fn project_to_pliron(unit: &GeneralGemmCompilationUnitV1) -> Result<GeneralGemmPlironEnvelope, ()> {
    catch_unwind(AssertUnwindSafe(|| project_to_pliron_inner(unit))).unwrap_or(Err(()))
}

fn project_to_pliron_inner(
    unit: &GeneralGemmCompilationUnitV1,
) -> Result<GeneralGemmPlironEnvelope, ()> {
    let mut context = Context::new();
    let context_identity = ensure_context_identity(&mut context).map_err(|_| ())?;
    register_dialects(&mut context)?;
    let symbol: Identifier = PLIRON_MODULE_SYMBOL.try_into().map_err(|_| ())?;
    let module = ModuleOp::new(&mut context, symbol);
    install_projection_metadata(&context, &module, unit);

    let algorithm = AlgorithmOp::new(&mut context, 3).map_err(|_| ())?;
    module.append_operation(&mut context, algorithm.get_operation(), 0);
    let schedule =
        PlanOp::new(&mut context, 3, GENERAL_GEMM_KIR_TILE_EXTENT_V1, 1).map_err(|_| ())?;
    module.append_operation(&mut context, schedule.get_operation(), 0);
    let tile = MaterializeOp::new(
        &mut context,
        2,
        GENERAL_GEMM_KIR_WAVE_LANES_V1,
        GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1,
    )
    .map_err(|_| ())?;
    module.append_operation(&mut context, tile.get_operation(), 0);
    for hierarchy in [
        HierarchyAttr::Grid,
        HierarchyAttr::Workgroup,
        HierarchyAttr::Subgroup,
        HierarchyAttr::Lane,
    ] {
        let operation = HierarchyIdOp::new(&mut context, hierarchy);
        module.append_operation(&mut context, operation.get_operation(), 0);
    }
    for address_space in [AddressSpaceAttr::Global, AddressSpaceAttr::Workgroup] {
        let operation = MemorySpaceOp::new(&mut context, address_space);
        module.append_operation(&mut context, operation.get_operation(), 0);
    }
    for _ in 0..2 {
        let barrier = BarrierOp::new(
            &mut context,
            HierarchyAttr::Workgroup,
            MemoryScopeAttr::Workgroup,
            AddressSpaceAttr::Workgroup,
            MemoryOrderAttr::AcquireRelease,
        );
        module.append_operation(&mut context, barrier.get_operation(), 0);
    }
    verify_operation(module.get_operation(), &context).map_err(|_| ())?;
    let operation_count = module
        .get_body(&context, 0)
        .deref(&context)
        .iter(&context)
        .take(unit.limits.max_pliron_operations + 1)
        .count();
    if operation_count != GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1 {
        return Err(());
    }
    let operation_schema = encode_pliron_operation_schema();
    let identity = GeneralGemmPlironProjectionIdentityV1(hash_fields(
        PROJECTION_IDENTITY_DOMAIN_V1,
        &[
            unit.identity.as_bytes(),
            unit.kir.identity().as_bytes(),
            unit.schedule.identity().as_bytes(),
            &operation_schema,
        ],
    ));
    Ok(GeneralGemmPlironEnvelope {
        context,
        context_identity,
        module,
        receipt: GeneralGemmPlironProjectionV1 {
            identity,
            compilation_binding_identity: unit.identity,
            schedule_identity: unit.schedule.identity(),
            kir_identity: unit.kir.identity(),
            operation_count,
        },
    })
}

struct GeneralGemmSymbolicPlironEnvelope {
    context: Context,
    context_identity: ContextIdentity,
    module: ModuleOp,
    receipt: GeneralGemmSymbolicPlironProjectionV1,
}

#[derive(Debug)]
pub(crate) struct GeneralGemmVerifiedLoweredGpuReceiptV1 {
    projection: GeneralGemmSymbolicPlironProjectionV1,
    schedule: GeneralGemmScheduleV1,
    request_identity: [u8; 32],
    kernel_instance_identity: [u8; 32],
    frontend_semantic_binding_identity: GeneralGemmFrontendSemanticBindingIdentityV1,
    compiled_source_identity: [u8; 32],
    provider_semantics_identity: [u8; 32],
    frontend_abi_identity: [u8; 32],
    toolchain_route_identity: GeneralGemmToolchainRouteIdentityV1,
    symbolic_kir_template: Vec<u8>,
}

impl GeneralGemmVerifiedLoweredGpuReceiptV1 {
    pub(crate) const fn projection(&self) -> GeneralGemmSymbolicPlironProjectionV1 {
        self.projection
    }

    pub(crate) const fn schedule(&self) -> GeneralGemmScheduleV1 {
        self.schedule
    }

    pub(crate) const fn compilation_identity(&self) -> GeneralGemmSymbolicCompilationIdentityV1 {
        self.projection.compilation_identity
    }

    pub(crate) const fn request_identity(&self) -> &[u8; 32] {
        &self.request_identity
    }

    pub(crate) const fn kernel_instance_identity(&self) -> &[u8; 32] {
        &self.kernel_instance_identity
    }

    pub(crate) const fn frontend_semantic_binding_identity(
        &self,
    ) -> GeneralGemmFrontendSemanticBindingIdentityV1 {
        self.frontend_semantic_binding_identity
    }

    pub(crate) const fn compiled_source_identity(&self) -> &[u8; 32] {
        &self.compiled_source_identity
    }

    pub(crate) const fn provider_semantics_identity(&self) -> &[u8; 32] {
        &self.provider_semantics_identity
    }

    pub(crate) const fn frontend_abi_identity(&self) -> &[u8; 32] {
        &self.frontend_abi_identity
    }

    pub(crate) const fn symbolic_plan_identity(&self) -> GeneralGemmSymbolicPlanIdentityV1 {
        self.projection.symbolic_plan_identity
    }

    pub(crate) const fn symbolic_kir_identity(&self) -> GeneralGemmSymbolicKirIdentityV1 {
        self.projection.symbolic_kir_identity
    }

    pub(crate) const fn schedule_identity(&self) -> GeneralGemmScheduleIdentityV1 {
        self.projection.schedule_identity
    }

    pub(crate) const fn toolchain_route_identity(&self) -> GeneralGemmToolchainRouteIdentityV1 {
        self.toolchain_route_identity
    }

    pub(crate) fn symbolic_kir_template(&self) -> &[u8] {
        &self.symbolic_kir_template
    }
}

impl GeneralGemmSymbolicPlironEnvelope {
    fn into_verified_lowered(
        self,
        unit: &GeneralGemmSymbolicCompilationUnitV1,
    ) -> Result<GeneralGemmVerifiedLoweredGpuReceiptV1, ()> {
        let current = require_context_identity(&self.context).map_err(|_| ())?;
        if current != self.context_identity
            || verify_operation(self.module.get_operation(), &self.context).is_err()
        {
            return Err(());
        }
        let module_binding = self.module.get_operation();
        let module = module_binding.deref(&self.context);
        if module.attributes.0.len() != 5
            || !metadata_matches(
                &module.attributes,
                PLIRON_SCHEMA_ATTR,
                GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1.as_bytes(),
            )
            || !metadata_matches(
                &module.attributes,
                PLIRON_BINDING_ATTR,
                unit.identity().as_bytes(),
            )
            || !metadata_matches(
                &module.attributes,
                PLIRON_KIR_ATTR,
                &encode_symbolic_kir_template(unit.frontend_semantics()),
            )
            || !metadata_matches(
                &module.attributes,
                PLIRON_SCHEDULE_ATTR,
                unit.schedule_identity().as_bytes(),
            )
            || module
                .attributes
                .0
                .get(&*ATTR_KEY_SYM_NAME)
                .and_then(|attribute| attribute.downcast_ref::<IdentifierAttr>())
                .map(AsRef::as_ref)
                .is_none_or(|symbol| symbol.as_ref() != PLIRON_MODULE_SYMBOL)
        {
            return Err(());
        }
        drop(module);
        validate_symbolic_lowered_operations(&self.context, &self.module, unit.schedule())?;
        if self.receipt.operation_count != GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1
            || self.receipt.compilation_identity != unit.identity()
            || self.receipt.schedule_identity != unit.schedule_identity()
            || self.receipt.symbolic_plan_identity != unit.symbolic_plan_identity()
            || self.receipt.symbolic_kir_identity != unit.symbolic_kir_identity()
        {
            return Err(());
        }
        Ok(GeneralGemmVerifiedLoweredGpuReceiptV1 {
            projection: self.receipt,
            schedule: unit.schedule(),
            request_identity: unit.request().identity().into_bytes(),
            kernel_instance_identity: unit.request().kernel_instance_identity().into_bytes(),
            frontend_semantic_binding_identity: unit.frontend_semantic_binding_identity(),
            compiled_source_identity: *unit.frontend_semantics().compiled_source_identity(),
            provider_semantics_identity: *unit.frontend_semantics().provider_semantics_identity(),
            frontend_abi_identity: *unit.frontend_semantics().frontend_abi_identity(),
            toolchain_route_identity: unit.toolchain_route_identity(),
            symbolic_kir_template: encode_symbolic_kir_template(unit.frontend_semantics()),
        })
    }
}

fn validate_symbolic_source_operations(
    context: &Context,
    module: &ModuleOp,
    expected_schedule: GeneralGemmScheduleV1,
) -> Result<(), ()> {
    let body = module.get_body(context, 0);
    let body = body.deref(context);
    let mut operations = body.iter(context);
    let algorithm = typed_next::<AlgorithmOp>(&mut operations, context)?;
    if algorithm
        .iteration_domain(context)
        .is_none_or(|domain| domain.rank() != 3)
    {
        return Err(());
    }
    typed_next::<GeneralGemmOp>(&mut operations, context)?;
    let schedule = typed_next::<PlanOp>(&mut operations, context)?;
    if schedule.parameters(context).is_none_or(|parameters| {
        parameters.rank() != 3
            || parameters.tile_extent() != GENERAL_GEMM_KIR_TILE_EXTENT_V1
            || parameters.pipeline_stages() != 1
    }) {
        return Err(());
    }
    let schedule = typed_next::<GeneralGemmPlanOp>(&mut operations, context)?;
    if schedule
        .get_attr_general_gemm_kind(context)
        .is_none_or(|actual| *actual != schedule_attr(expected_schedule))
    {
        return Err(());
    }
    let tile = typed_next::<MaterializeOp>(&mut operations, context)?;
    if tile.distribution(context).is_none_or(|distribution| {
        distribution.rank() != 2
            || distribution.lanes() != GENERAL_GEMM_KIR_WAVE_LANES_V1
            || distribution.elements_per_lane() != GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1
    }) {
        return Err(());
    }
    typed_next::<GeneralGemmXor4Op>(&mut operations, context)?;
    if operations.next().is_some() {
        return Err(());
    }
    Ok(())
}

fn validate_symbolic_lowered_operations(
    context: &Context,
    module: &ModuleOp,
    expected_schedule: GeneralGemmScheduleV1,
) -> Result<(), ()> {
    let body = module.get_body(context, 0);
    let body = body.deref(context);
    let mut operations = body.iter(context);
    exact_attr(
        typed_next::<GeneralGemmRuntimeAbiOp>(&mut operations, context)?
            .get_attr_general_gemm_runtime_abi(context),
        GeneralGemmRuntimeAbiAttr::DynamicElevenArgumentBf16F32V1,
    )?;
    exact_attr(
        typed_next::<GeneralGemmGridMappingOp>(&mut operations, context)?
            .get_attr_general_gemm_grid_mapping(context),
        GeneralGemmGridMappingAttr::GridXy16Wave64FourComponentsV1,
    )?;
    exact_attr(
        typed_next::<GeneralGemmPhaseLoopOp>(&mut operations, context)?
            .get_attr_general_gemm_phase_loop(context),
        GeneralGemmPhaseLoopAttr::CheckedCeilDivK16InductionV1,
    )?;
    let expected_a = match expected_schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
            GeneralGemmGlobalTransferAttr::AScalarMaskedZeroFillV1
        }
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
            GeneralGemmGlobalTransferAttr::AVector4AlignedFullScalarFallbackZeroFillV1
        }
    };
    exact_attr(
        typed_next::<GeneralGemmGlobalTransferOp>(&mut operations, context)?
            .get_attr_general_gemm_global_transfer(context),
        expected_a,
    )?;
    exact_attr(
        typed_next::<GeneralGemmGlobalTransferOp>(&mut operations, context)?
            .get_attr_general_gemm_global_transfer(context),
        GeneralGemmGlobalTransferAttr::BScalarMaskedZeroFillV1,
    )?;
    for expected in [
        GeneralGemmLdsTransferAttr::AWriteFourXor4V1,
        GeneralGemmLdsTransferAttr::BWriteFourXor4V1,
    ] {
        exact_attr(
            typed_next::<GeneralGemmLdsTransferOp>(&mut operations, context)?
                .get_attr_general_gemm_lds_transfer(context),
            expected,
        )?;
    }
    exact_attr(
        typed_next::<GeneralGemmEpochOp>(&mut operations, context)?
            .get_attr_general_gemm_epoch(context),
        GeneralGemmEpochAttr::PublishWorkgroupAcquireReleaseV1,
    )?;
    for expected in [
        GeneralGemmLdsTransferAttr::AReadFourXor4V1,
        GeneralGemmLdsTransferAttr::BReadFourXor4V1,
    ] {
        exact_attr(
            typed_next::<GeneralGemmLdsTransferOp>(&mut operations, context)?
                .get_attr_general_gemm_lds_transfer(context),
            expected,
        )?;
    }
    exact_attr(
        typed_next::<GeneralGemmMfmaOp>(&mut operations, context)?
            .get_attr_general_gemm_mfma(context),
        GeneralGemmMfmaAttr::Bf16F32Wave64CarriedF32x4V1,
    )?;
    exact_attr(
        typed_next::<GeneralGemmEpochOp>(&mut operations, context)?
            .get_attr_general_gemm_epoch(context),
        GeneralGemmEpochAttr::ReuseWorkgroupAcquireReleaseV1,
    )?;
    exact_attr(
        typed_next::<GeneralGemmGlobalTransferOp>(&mut operations, context)?
            .get_attr_general_gemm_global_transfer(context),
        GeneralGemmGlobalTransferAttr::CGuardedDisjointLoadV1,
    )?;
    exact_attr(
        typed_next::<GeneralGemmEpilogueOp>(&mut operations, context)?
            .get_attr_general_gemm_epilogue(context),
        GeneralGemmEpilogueAttr::GuardedDisjointAlphaAccPlusBetaCV1,
    )?;
    exact_attr(
        typed_next::<GeneralGemmGlobalTransferOp>(&mut operations, context)?
            .get_attr_general_gemm_global_transfer(context),
        GeneralGemmGlobalTransferAttr::CGuardedDisjointStoreV1,
    )?;
    if operations.next().is_some() {
        return Err(());
    }
    Ok(())
}

fn exact_attr<T: Copy + Eq>(actual: Option<std::cell::Ref<'_, T>>, expected: T) -> Result<(), ()> {
    if actual.is_none_or(|value| *value != expected) {
        Err(())
    } else {
        Ok(())
    }
}

fn project_symbolic_to_pliron(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
) -> Result<GeneralGemmSymbolicPlironEnvelope, ()> {
    catch_unwind(AssertUnwindSafe(|| project_symbolic_to_pliron_inner(unit))).unwrap_or(Err(()))
}

fn project_symbolic_to_pliron_inner(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
) -> Result<GeneralGemmSymbolicPlironEnvelope, ()> {
    let source_operation_schema = build_and_verify_symbolic_source_module(unit)?;
    let mut context = Context::new();
    let context_identity = ensure_context_identity(&mut context).map_err(|_| ())?;
    register_dialects(&mut context)?;
    let symbol: Identifier = PLIRON_MODULE_SYMBOL.try_into().map_err(|_| ())?;
    let module = ModuleOp::new(&mut context, symbol);
    install_symbolic_projection_metadata(&context, &module, unit);

    append_symbolic_lowered_operations(&mut context, &module, unit.schedule());
    verify_operation(module.get_operation(), &context).map_err(|_| ())?;
    validate_symbolic_lowered_operations(&context, &module, unit.schedule())?;
    let operation_count = module
        .get_body(&context, 0)
        .deref(&context)
        .iter(&context)
        .take(unit.limits().max_pliron_operations + 1)
        .count();
    if operation_count != GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1 {
        return Err(());
    }
    let lowered_operation_schema = encode_symbolic_lowered_operation_schema(unit.schedule());
    let source_operation_identity = GeneralGemmPlironSourceOperationIdentityV1(hash_fields(
        SOURCE_OPERATION_IDENTITY_DOMAIN_V1,
        &[&source_operation_schema],
    ));
    let lowered_operation_identity = GeneralGemmPlironLoweredOperationIdentityV1(hash_fields(
        LOWERED_OPERATION_IDENTITY_DOMAIN_V1,
        &[&lowered_operation_schema],
    ));
    let transformation_identity = GeneralGemmPlironTransformationIdentityV1(hash_fields(
        TRANSFORMATION_IDENTITY_DOMAIN_V1,
        &[
            source_operation_identity.as_bytes(),
            lowered_operation_identity.as_bytes(),
            unit.symbolic_plan_identity().as_bytes(),
            unit.symbolic_kir_identity().as_bytes(),
            unit.schedule_identity().as_bytes(),
        ],
    ));
    let identity = GeneralGemmPlironProjectionIdentityV1(hash_fields(
        PROJECTION_IDENTITY_DOMAIN_V1,
        &[
            unit.identity().as_bytes(),
            unit.symbolic_plan_identity().as_bytes(),
            unit.symbolic_kir_identity().as_bytes(),
            unit.schedule_identity().as_bytes(),
            source_operation_identity.as_bytes(),
            lowered_operation_identity.as_bytes(),
            transformation_identity.as_bytes(),
        ],
    ));
    Ok(GeneralGemmSymbolicPlironEnvelope {
        context,
        context_identity,
        module,
        receipt: GeneralGemmSymbolicPlironProjectionV1 {
            identity,
            source_operation_identity,
            lowered_operation_identity,
            transformation_identity,
            compilation_identity: unit.identity(),
            schedule_identity: unit.schedule_identity(),
            symbolic_plan_identity: unit.symbolic_plan_identity(),
            symbolic_kir_identity: unit.symbolic_kir_identity(),
            operation_count,
        },
    })
}

fn build_and_verify_symbolic_source_module(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
) -> Result<Vec<u8>, ()> {
    let (context, module) = build_symbolic_source_module(unit)?;
    verify_operation(module.get_operation(), &context).map_err(|_| ())?;
    let operation_count = module
        .get_body(&context, 0)
        .deref(&context)
        .iter(&context)
        .count();
    if operation_count != GENERAL_GEMM_SYMBOLIC_SOURCE_OPERATION_COUNT_V1 {
        return Err(());
    }
    validate_symbolic_source_operations(&context, &module, unit.schedule())?;
    Ok(encode_symbolic_source_operation_schema(unit.schedule()))
}

fn build_symbolic_source_module(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
) -> Result<(Context, ModuleOp), ()> {
    let mut context = Context::new();
    ensure_context_identity(&mut context).map_err(|_| ())?;
    register_dialects(&mut context)?;
    let symbol: Identifier = PLIRON_MODULE_SYMBOL.try_into().map_err(|_| ())?;
    let module = ModuleOp::new(&mut context, symbol);
    install_symbolic_projection_metadata(&context, &module, unit);
    let algorithm = AlgorithmOp::new(&mut context, 3).map_err(|_| ())?;
    module.append_operation(&mut context, algorithm.get_operation(), 0);
    let gemm = GeneralGemmOp::canonical(&mut context);
    module.append_operation(&mut context, gemm.get_operation(), 0);
    let schedule =
        PlanOp::new(&mut context, 3, GENERAL_GEMM_KIR_TILE_EXTENT_V1, 1).map_err(|_| ())?;
    module.append_operation(&mut context, schedule.get_operation(), 0);
    let gemm_schedule = GeneralGemmPlanOp::new(&mut context, schedule_attr(unit.schedule()));
    module.append_operation(&mut context, gemm_schedule.get_operation(), 0);
    let tile = MaterializeOp::new(
        &mut context,
        2,
        GENERAL_GEMM_KIR_WAVE_LANES_V1,
        GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1,
    )
    .map_err(|_| ())?;
    module.append_operation(&mut context, tile.get_operation(), 0);
    let mapping = GeneralGemmXor4Op::canonical(&mut context);
    module.append_operation(&mut context, mapping.get_operation(), 0);
    Ok((context, module))
}

fn append_symbolic_lowered_operations(
    context: &mut Context,
    module: &ModuleOp,
    schedule: GeneralGemmScheduleV1,
) {
    macro_rules! append {
        ($op:expr) => {{
            let operation = $op;
            module.append_operation(context, operation.get_operation(), 0);
        }};
    }
    for operation in canonical_lowered_operations(schedule) {
        match operation {
            GeneralGemmLoweredOperationV1::RuntimeAbi(attribute) => {
                append!(GeneralGemmRuntimeAbiOp::new(context, attribute));
            }
            GeneralGemmLoweredOperationV1::GridMapping(attribute) => {
                append!(GeneralGemmGridMappingOp::new(context, attribute));
            }
            GeneralGemmLoweredOperationV1::PhaseLoop(attribute) => {
                append!(GeneralGemmPhaseLoopOp::new(context, attribute));
            }
            GeneralGemmLoweredOperationV1::GlobalTransfer(attribute) => {
                append!(GeneralGemmGlobalTransferOp::new(context, attribute));
            }
            GeneralGemmLoweredOperationV1::LdsTransfer(attribute) => {
                append!(GeneralGemmLdsTransferOp::new(context, attribute));
            }
            GeneralGemmLoweredOperationV1::Epoch(attribute) => {
                append!(GeneralGemmEpochOp::new(context, attribute));
            }
            GeneralGemmLoweredOperationV1::Mfma(attribute) => {
                append!(GeneralGemmMfmaOp::new(context, attribute));
            }
            GeneralGemmLoweredOperationV1::Epilogue(attribute) => {
                append!(GeneralGemmEpilogueOp::new(context, attribute));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralGemmLoweredOperationV1 {
    RuntimeAbi(GeneralGemmRuntimeAbiAttr),
    GridMapping(GeneralGemmGridMappingAttr),
    PhaseLoop(GeneralGemmPhaseLoopAttr),
    GlobalTransfer(GeneralGemmGlobalTransferAttr),
    LdsTransfer(GeneralGemmLdsTransferAttr),
    Epoch(GeneralGemmEpochAttr),
    Mfma(GeneralGemmMfmaAttr),
    Epilogue(GeneralGemmEpilogueAttr),
}

fn canonical_lowered_operations(
    schedule: GeneralGemmScheduleV1,
) -> [GeneralGemmLoweredOperationV1; GENERAL_GEMM_SYMBOLIC_LOWERED_OPERATION_COUNT_V1] {
    let a_transfer = match schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
            GeneralGemmGlobalTransferAttr::AScalarMaskedZeroFillV1
        }
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
            GeneralGemmGlobalTransferAttr::AVector4AlignedFullScalarFallbackZeroFillV1
        }
    };
    [
        GeneralGemmLoweredOperationV1::RuntimeAbi(
            GeneralGemmRuntimeAbiAttr::DynamicElevenArgumentBf16F32V1,
        ),
        GeneralGemmLoweredOperationV1::GridMapping(
            GeneralGemmGridMappingAttr::GridXy16Wave64FourComponentsV1,
        ),
        GeneralGemmLoweredOperationV1::PhaseLoop(
            GeneralGemmPhaseLoopAttr::CheckedCeilDivK16InductionV1,
        ),
        GeneralGemmLoweredOperationV1::GlobalTransfer(a_transfer),
        GeneralGemmLoweredOperationV1::GlobalTransfer(
            GeneralGemmGlobalTransferAttr::BScalarMaskedZeroFillV1,
        ),
        GeneralGemmLoweredOperationV1::LdsTransfer(GeneralGemmLdsTransferAttr::AWriteFourXor4V1),
        GeneralGemmLoweredOperationV1::LdsTransfer(GeneralGemmLdsTransferAttr::BWriteFourXor4V1),
        GeneralGemmLoweredOperationV1::Epoch(
            GeneralGemmEpochAttr::PublishWorkgroupAcquireReleaseV1,
        ),
        GeneralGemmLoweredOperationV1::LdsTransfer(GeneralGemmLdsTransferAttr::AReadFourXor4V1),
        GeneralGemmLoweredOperationV1::LdsTransfer(GeneralGemmLdsTransferAttr::BReadFourXor4V1),
        GeneralGemmLoweredOperationV1::Mfma(GeneralGemmMfmaAttr::Bf16F32Wave64CarriedF32x4V1),
        GeneralGemmLoweredOperationV1::Epoch(GeneralGemmEpochAttr::ReuseWorkgroupAcquireReleaseV1),
        GeneralGemmLoweredOperationV1::GlobalTransfer(
            GeneralGemmGlobalTransferAttr::CGuardedDisjointLoadV1,
        ),
        GeneralGemmLoweredOperationV1::Epilogue(
            GeneralGemmEpilogueAttr::GuardedDisjointAlphaAccPlusBetaCV1,
        ),
        GeneralGemmLoweredOperationV1::GlobalTransfer(
            GeneralGemmGlobalTransferAttr::CGuardedDisjointStoreV1,
        ),
    ]
}

const fn schedule_attr(schedule: GeneralGemmScheduleV1) -> GeneralGemmScheduleAttr {
    match schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
            GeneralGemmScheduleAttr::ReferenceWave64Xor4V1
        }
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
            GeneralGemmScheduleAttr::VectorizedAOnlyBf16GlobalTransferV1
        }
    }
}

fn register_dialects(context: &mut Context) -> Result<(), ()> {
    let kernel = DialectName::try_new(dialect_kernel::DIALECT_NAME).map_err(|_| ())?;
    dialect_kernel::register_dialect(context, &kernel).map_err(|_| ())?;
    let schedule = DialectName::try_new(dialect_schedule::DIALECT_NAME).map_err(|_| ())?;
    dialect_schedule::register_dialect(context, &schedule).map_err(|_| ())?;
    let tile = DialectName::try_new(dialect_tile::DIALECT_NAME).map_err(|_| ())?;
    dialect_tile::register_dialect(context, &tile).map_err(|_| ())?;
    dialect_gpu::register_dialect(context).map_err(|_| ())?;
    Ok(())
}

fn install_projection_metadata(
    context: &Context,
    module: &ModuleOp,
    unit: &GeneralGemmCompilationUnitV1,
) {
    let binding = module.get_operation();
    let mut operation = binding.deref_mut(context);
    operation.attributes.set(
        metadata_key(PLIRON_SCHEMA_ATTR),
        BytesAttr::new(
            GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1
                .as_bytes()
                .to_vec(),
        ),
    );
    operation.attributes.set(
        metadata_key(PLIRON_BINDING_ATTR),
        BytesAttr::new(unit.identity.into_bytes().to_vec()),
    );
    operation.attributes.set(
        metadata_key(PLIRON_KIR_ATTR),
        BytesAttr::new(unit.kir.encode_canonical()),
    );
    operation.attributes.set(
        metadata_key(PLIRON_SCHEDULE_ATTR),
        BytesAttr::new(unit.schedule.identity().into_bytes().to_vec()),
    );
}

fn install_symbolic_projection_metadata(
    context: &Context,
    module: &ModuleOp,
    unit: &GeneralGemmSymbolicCompilationUnitV1,
) {
    let binding = module.get_operation();
    let mut operation = binding.deref_mut(context);
    operation.attributes.set(
        metadata_key(PLIRON_SCHEMA_ATTR),
        BytesAttr::new(
            GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1
                .as_bytes()
                .to_vec(),
        ),
    );
    operation.attributes.set(
        metadata_key(PLIRON_BINDING_ATTR),
        BytesAttr::new(unit.identity().into_bytes().to_vec()),
    );
    operation.attributes.set(
        metadata_key(PLIRON_KIR_ATTR),
        BytesAttr::new(encode_symbolic_kir_template(unit.frontend_semantics())),
    );
    operation.attributes.set(
        metadata_key(PLIRON_SCHEDULE_ATTR),
        BytesAttr::new(unit.schedule_identity().into_bytes().to_vec()),
    );
}

fn metadata_key(value: &'static str) -> Identifier {
    value
        .try_into()
        .expect("fixed general GEMM metadata key is valid")
}

fn metadata_matches(
    attributes: &pliron::attribute::AttributeDict,
    key: &'static str,
    expected: &[u8],
) -> bool {
    attributes
        .0
        .get(&metadata_key(key))
        .and_then(|attribute| attribute.downcast_ref::<BytesAttr>())
        .is_some_and(|actual| actual.as_ref().as_slice() == expected)
}

fn typed_next<T>(
    operations: &mut impl Iterator<Item = pliron::context::Ptr<Operation>>,
    context: &Context,
) -> Result<T, ()>
where
    T: Op + 'static,
{
    let operation = operations.next().ok_or(())?;
    Operation::get_op::<T>(operation, context).ok_or(())
}

fn encode_pliron_operation_schema() -> Vec<u8> {
    // kernel, schedule, tile, four hierarchy IDs, two memory spaces, two barriers.
    vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10]
}

fn encode_symbolic_source_operation_schema(schedule: GeneralGemmScheduleV1) -> Vec<u8> {
    // algorithm, GEMM ABI/epilogue, generic plan, GEMM plan, tile, XOR4 mapping.
    vec![1, 11, 2, 20 + schedule as u8, 3, 12]
}

fn encode_symbolic_lowered_operation_schema(schedule: GeneralGemmScheduleV1) -> Vec<u8> {
    canonical_lowered_operations(schedule)
        .into_iter()
        .map(|operation| match operation {
            GeneralGemmLoweredOperationV1::RuntimeAbi(_) => 21,
            GeneralGemmLoweredOperationV1::GridMapping(_) => 22,
            GeneralGemmLoweredOperationV1::PhaseLoop(_) => 23,
            GeneralGemmLoweredOperationV1::GlobalTransfer(attribute) => match attribute {
                GeneralGemmGlobalTransferAttr::AScalarMaskedZeroFillV1 => 31,
                GeneralGemmGlobalTransferAttr::AVector4AlignedFullScalarFallbackZeroFillV1 => 32,
                GeneralGemmGlobalTransferAttr::BScalarMaskedZeroFillV1 => 33,
                GeneralGemmGlobalTransferAttr::CGuardedDisjointLoadV1 => 34,
                GeneralGemmGlobalTransferAttr::CGuardedDisjointStoreV1 => 35,
            },
            GeneralGemmLoweredOperationV1::LdsTransfer(attribute) => match attribute {
                GeneralGemmLdsTransferAttr::AWriteFourXor4V1 => 41,
                GeneralGemmLdsTransferAttr::BWriteFourXor4V1 => 42,
                GeneralGemmLdsTransferAttr::AReadFourXor4V1 => 43,
                GeneralGemmLdsTransferAttr::BReadFourXor4V1 => 44,
            },
            GeneralGemmLoweredOperationV1::Epoch(attribute) => match attribute {
                GeneralGemmEpochAttr::PublishWorkgroupAcquireReleaseV1 => 51,
                GeneralGemmEpochAttr::ReuseWorkgroupAcquireReleaseV1 => 52,
            },
            GeneralGemmLoweredOperationV1::Mfma(_) => 61,
            GeneralGemmLoweredOperationV1::Epilogue(_) => 71,
        })
        .collect()
}

fn encode_symbolic_plan(expressions: [GeneralGemmSymbolicPlanExpressionV1; 6]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    for expression in expressions {
        match expression {
            GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
                rows,
                columns,
                stride,
            } => bytes.extend_from_slice(&[1, rows as u8, columns as u8, stride as u8]),
            GeneralGemmSymbolicPlanExpressionV1::CeilDiv16(value) => {
                bytes.extend_from_slice(&[2, value as u8])
            }
            GeneralGemmSymbolicPlanExpressionV1::OutputBlockCounts => bytes.push(3),
            GeneralGemmSymbolicPlanExpressionV1::AqlGridWorkItems => bytes.push(4),
        }
    }
    bytes.extend_from_slice(&GENERAL_GEMM_KIR_TILE_EXTENT_V1.to_le_bytes());
    bytes.extend_from_slice(&GENERAL_GEMM_KIR_WAVE_LANES_V1.to_le_bytes());
    bytes
}

fn encode_symbolic_kir_behavior_fields(
    behaviors: [GeneralGemmDerivedKirBehaviorV1; 5],
) -> [Vec<u8>; 5] {
    behaviors.map(|behavior| match behavior {
        GeneralGemmDerivedKirBehaviorV1::Wave64GridXy16 => b"wave64-grid-xy16".to_vec(),
        GeneralGemmDerivedKirBehaviorV1::GuardedAbCheckedRowMajorZeroTail => {
            b"guarded-a-b-positive-zero-tail".to_vec()
        }
        GeneralGemmDerivedKirBehaviorV1::Xor4SingleBufferPublishReadMfmaReuse => {
            b"xor4-single-buffer-stage-publish-mfma-reuse".to_vec()
        }
        GeneralGemmDerivedKirBehaviorV1::CarriedF32x4PhaseAccumulator => {
            b"carried-f32x4-phase-accumulator".to_vec()
        }
        GeneralGemmDerivedKirBehaviorV1::GuardedDisjointCAlphaAccPlusBetaC => {
            b"guarded-disjoint-c-alpha-acc-plus-beta-c".to_vec()
        }
    })
}

fn expected_symbolic_plan_identity() -> GeneralGemmSymbolicPlanIdentityV1 {
    GeneralGemmSymbolicPlanIdentityV1(hash_fields(
        SYMBOLIC_PLAN_IDENTITY_DOMAIN_V1,
        &[&encode_symbolic_plan(symbolic_plan_expressions())],
    ))
}

fn expected_symbolic_kir_identity() -> GeneralGemmSymbolicKirIdentityV1 {
    let plan = expected_symbolic_plan_identity();
    let behavior_fields = encode_symbolic_kir_behavior_fields(symbolic_kir_behaviors());
    let mut fields = Vec::with_capacity(6);
    fields.push(plan.as_bytes().as_slice());
    fields.extend(behavior_fields.iter().map(Vec::as_slice));
    GeneralGemmSymbolicKirIdentityV1(hash_fields(SYMBOLIC_KIR_IDENTITY_DOMAIN_V1, &fields))
}

fn encode_symbolic_kir_template(frontend: &GeneralGemmFrontendSemanticBindingV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(256);
    append_identity_field(&mut bytes, frontend.symbolic_plan().identity().as_bytes());
    append_identity_field(&mut bytes, frontend.symbolic_kir().identity().as_bytes());
    append_identity_field(&mut bytes, frontend.frontend_abi_identity());
    for field in [
        b"wave64-grid-xy16".as_slice(),
        b"guarded-a-b-positive-zero-tail".as_slice(),
        b"xor4-single-buffer-stage-publish-mfma-reuse".as_slice(),
        b"carried-f32x4-phase-accumulator".as_slice(),
        b"guarded-disjoint-c-alpha-acc-plus-beta-c".as_slice(),
    ] {
        append_identity_field(&mut bytes, field);
    }
    bytes
}

fn append_identity_field(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
    bytes.extend_from_slice(field);
}

const fn symbolic_plan_expressions() -> [GeneralGemmSymbolicPlanExpressionV1; 6] {
    [
        GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
            rows: GeneralGemmAbiArgumentV1::M,
            columns: GeneralGemmAbiArgumentV1::K,
            stride: GeneralGemmAbiArgumentV1::Lda,
        },
        GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
            rows: GeneralGemmAbiArgumentV1::K,
            columns: GeneralGemmAbiArgumentV1::N,
            stride: GeneralGemmAbiArgumentV1::Ldb,
        },
        GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
            rows: GeneralGemmAbiArgumentV1::M,
            columns: GeneralGemmAbiArgumentV1::N,
            stride: GeneralGemmAbiArgumentV1::Ldc,
        },
        GeneralGemmSymbolicPlanExpressionV1::CeilDiv16(GeneralGemmAbiArgumentV1::K),
        GeneralGemmSymbolicPlanExpressionV1::OutputBlockCounts,
        GeneralGemmSymbolicPlanExpressionV1::AqlGridWorkItems,
    ]
}

const fn symbolic_kir_behaviors() -> [GeneralGemmDerivedKirBehaviorV1; 5] {
    [
        GeneralGemmDerivedKirBehaviorV1::Wave64GridXy16,
        GeneralGemmDerivedKirBehaviorV1::GuardedAbCheckedRowMajorZeroTail,
        GeneralGemmDerivedKirBehaviorV1::Xor4SingleBufferPublishReadMfmaReuse,
        GeneralGemmDerivedKirBehaviorV1::CarriedF32x4PhaseAccumulator,
        GeneralGemmDerivedKirBehaviorV1::GuardedDisjointCAlphaAccPlusBetaC,
    ]
}

fn plan_identity(plan: GeneralGemmPlanFieldsV1) -> GeneralGemmPlanIdentityV1 {
    GeneralGemmPlanIdentityV1(hash_fields(PLAN_IDENTITY_DOMAIN_V1, &[&encode_plan(plan)]))
}

fn encode_plan(plan: GeneralGemmPlanFieldsV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(112);
    for value in plan.dimensions() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in plan.strides() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in plan.storage_elements() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in plan.block_counts() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in plan.aql_grid_work_items() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&plan.reduction_phases().to_le_bytes());
    bytes.extend_from_slice(&plan.alpha_bits().to_le_bytes());
    bytes.extend_from_slice(&plan.beta_bits().to_le_bytes());
    let tails = plan.tails();
    bytes.extend_from_slice(&[
        tails.m,
        tails.n,
        tails.k,
        u8::from(plan.requires_dispatch()),
    ]);
    bytes
}

fn encode_runtime_abi(snapshot: GeneralGemmRuntimeAbiSnapshotV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(96);
    bytes.extend_from_slice(GENERAL_GEMM_KERNEL_SYMBOL_V1.as_bytes());
    // Three scalarized pointer slots: const BF16 A, const BF16 B, mutable disjoint FP32 C.
    bytes.extend_from_slice(&[1, 1, 2]);
    for value in [
        snapshot.a_elements,
        snapshot.b_elements,
        snapshot.c_elements,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in snapshot.dimensions {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in snapshot.strides {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&snapshot.alpha_bits.to_le_bytes());
    bytes.extend_from_slice(&snapshot.beta_bits.to_le_bytes());
    bytes
}

fn request_commitments(request: &CompileRequestV1) -> [[u8; 32]; 9] {
    [
        request.identity().into_bytes(),
        request.kernel_instance_identity().into_bytes(),
        request.compiler_profile_identity().into_bytes(),
        request.target_profile_identity().into_bytes(),
        request.pipeline_configuration_identity().into_bytes(),
        request.input_obligations_identity().into_bytes(),
        request.input().identity().into_bytes(),
        request.input().format_identity().into_bytes(),
        hash_fields(
            b"fe2o3.general-gemm.frontend-bytes.v1\0",
            &[request.input().canonical_bytes()],
        ),
    ]
}

fn is_zero_identity(identity: &[u8; 32]) -> bool {
    identity == &[0; 32]
}

const fn proof_identity(identity: [u8; 32]) -> GeneralGemmEvidenceIdentityV1 {
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(identity)
}

fn hash_fields(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_le_bytes());
    hasher.update(domain);
    for field in fields {
        hasher.update((field.len() as u64).to_le_bytes());
        hasher.update(field);
    }
    hasher.finalize().into()
}

#[allow(dead_code)]
fn artifact_binding_identity(
    compilation: GeneralGemmCompilationBindingIdentityV1,
    projection: GeneralGemmPlironProjectionIdentityV1,
    handoff: HandoffIdentityV2,
    assembly: LlvmAssemblySha256V2,
    assembly_len: u64,
    compiler_handoff: CompilerModuleHandoffIdentityV2,
    candidate: CandidateIdentityV1,
) -> GeneralGemmArtifactBindingIdentityV1 {
    GeneralGemmArtifactBindingIdentityV1(hash_fields(
        ARTIFACT_IDENTITY_DOMAIN_V1,
        &[
            compilation.as_bytes(),
            projection.as_bytes(),
            handoff.as_bytes(),
            assembly.as_bytes(),
            &assembly_len.to_le_bytes(),
            compiler_handoff.sha256(),
            &compiler_handoff.byte_len().to_le_bytes(),
            candidate.as_bytes(),
        ],
    ))
}

#[cfg(test)]
mod tests;
