#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use core::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, HierarchyAttr, HierarchyIdOp, MemoryOrderAttr, MemoryScopeAttr,
    MemorySpaceOp,
};
use dialect_kernel::AlgorithmOp;
use dialect_schedule::PlanOp;
use dialect_tile::MaterializeOp;
use fe2o3_compiler_api::{
    CandidateIdentityV1, CanonicalDiagnosticV1, CompileDispositionV1, CompileOutputV1,
    CompileRequestV1, CompilerStageV1, DiagnosticCodeV1, DiagnosticMessageV1, DiagnosticSeverityV1,
    DiagnosticSubjectIdentityV1, PipelineSelectorV1,
};
use fe2o3_compiler_driver::{
    AdmittedGemmCompilerBackendV1, CompilerBackendFailureV1, GemmSemanticProgramV1,
    ProofRequiredGemmAdmissionV1, analyze_gemm_semantics_v1,
    general_gemm_semantic_obligation_set_identity_v1,
};
use fe2o3_compiler_ffi::CompilerModuleHandoffIdentityV2;
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

/// Schema for the exact compilation-unit binding.
pub const GENERAL_GEMM_COMPILATION_BINDING_SCHEMA_V1: &str =
    "fe2o3.general-gemm.compilation-binding.v1";
/// Fixed kernel symbol in the authenticated safe source profile.
pub const GENERAL_GEMM_KERNEL_SYMBOL_V1: &str = "tiled_gemm_general_v1";
/// Exact target selected by this first lowering route.
pub const GENERAL_GEMM_DEVICE_TARGET_V1: &str = "gfx942:xnack-";
/// Maximum complete structured KIR bytes admitted by this route.
pub const MAX_GENERAL_GEMM_KIR_BYTES_V1: usize = 4 * 1024;
/// Exact number of typed operations in the current Pliron projection.
pub const GENERAL_GEMM_PLIRON_OPERATION_COUNT_V1: usize = 11;
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
const PROJECTION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.pliron-projection.v1\0";
const ARTIFACT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.artifact-binding.v1\0";
const PLIRON_MODULE_SYMBOL: &str = "fe2o3_general_gemm";
const PLIRON_SCHEMA_ATTR: &str = "fe2o3_general_gemm_schema_v1";
const PLIRON_BINDING_ATTR: &str = "fe2o3_general_gemm_binding_identity_v1";
const PLIRON_KIR_ATTR: &str = "fe2o3_general_gemm_kir_v1";
const PLIRON_SCHEDULE_ATTR: &str = "fe2o3_general_gemm_schedule_identity_v1";
const GENERAL_GEMM_LOWERING_BLOCKED_CODE_V1: u32 = 0x4647_0201;
const GENERAL_GEMM_LOWERING_BLOCKED_MESSAGE_V1: &str =
    "general GEMM AMDGPU lowering is unavailable in LLVM Handoff V2; no candidate was produced";

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
    /// Identity of the owner-checked typed Pliron projection.
    GeneralGemmPlironProjectionIdentityV1
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

/// Closed runtime-parameterized checked-plan schema derived from positive MIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmSymbolicPlanV1 {
    identity: GeneralGemmSymbolicPlanIdentityV1,
}

impl GeneralGemmSymbolicPlanV1 {
    /// Returns the one admitted runtime-derived plan expression schema.
    pub fn canonical() -> Self {
        Self {
            identity: GeneralGemmSymbolicPlanIdentityV1(hash_fields(
                SYMBOLIC_PLAN_IDENTITY_DOMAIN_V1,
                &[&encode_symbolic_plan()],
            )),
        }
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
        Self {
            identity: GeneralGemmSymbolicKirIdentityV1(hash_fields(
                SYMBOLIC_KIR_IDENTITY_DOMAIN_V1,
                &[
                    GeneralGemmSymbolicPlanV1::canonical().identity().as_bytes(),
                    b"wave64-grid-xy16",
                    b"guarded-a-b-positive-zero-tail",
                    b"xor4-single-buffer-stage-publish-mfma-reuse",
                    b"carried-f32x4-phase-accumulator",
                    b"guarded-disjoint-c-alpha-acc-plus-beta-c",
                ],
            )),
        }
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
        symbolic_plan: GeneralGemmSymbolicPlanV1,
        symbolic_kir: GeneralGemmSymbolicKirV1,
    ) -> Result<Self, GeneralGemmFrontendSemanticBindingErrorV1> {
        if [kernel_instance, compiled_source, provider_semantics]
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
                symbolic_plan.identity().as_bytes(),
                symbolic_kir.identity().as_bytes(),
            ],
        ));
        Ok(Self {
            kernel_instance,
            compiled_source,
            provider_semantics,
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

/// Complete deterministic binding passed from semantic admission to proof.
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

/// Missing typed machine contracts that prevent honest Handoff V2 emission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmMachineRepresentationGapV1 {
    /// Handoff V2 globals cannot represent a 256-element BF16 LDS allocation.
    WorkgroupBf16Array256,
    /// Handoff V2 has no wave64 BF16 `m16n16k16` MFMA fragment/intrinsic.
    Wave64Bf16MfmaM16N16K16,
    /// Handoff V2 CFG cannot carry the four FP32 accumulators through a loop.
    LoopCarriedF32x4Accumulator,
}

/// Known later contracts that remain unavailable after typed LLVM handoff.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmPostLoweringGapV1 {
    /// Worker V2 preparation is inert and does not execute a measured worker.
    MeasuredWorkerV2Execution,
    /// The finalizer does not consume an exact source-bound general-GEMM handoff.
    SourceBoundGemmHsacoFinalization,
    /// Publication and protected runtime admission do not yet consume this chain.
    TransactionalPublicationAndRuntimeAdmission,
}

/// Exact stage and gaps that stopped the candidate-producing route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmLoweringBlockerV1 {
    stage: CompilerStageV1,
    gaps: [GeneralGemmMachineRepresentationGapV1; 3],
}

impl GeneralGemmLoweringBlockerV1 {
    /// Returns the first stage that cannot represent the checked semantics.
    pub const fn stage(self) -> CompilerStageV1 {
        self.stage
    }

    /// Returns all independently missing Handoff V2 contracts.
    pub const fn gaps(self) -> [GeneralGemmMachineRepresentationGapV1; 3] {
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

    /// Returns the exact missing machine-representation contracts.
    pub const fn blocker(self) -> GeneralGemmLoweringBlockerV1 {
        self.blocker
    }

    /// Returns known later gaps without claiming that the route reached them.
    pub const fn post_lowering_gaps(self) -> [GeneralGemmPostLoweringGapV1; 3] {
        [
            GeneralGemmPostLoweringGapV1::MeasuredWorkerV2Execution,
            GeneralGemmPostLoweringGapV1::SourceBoundGemmHsacoFinalization,
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
    /// returns the honest typed Handoff V2 blocker.
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
                stage: CompilerStageV1::Amdgcn,
                gaps: [
                    GeneralGemmMachineRepresentationGapV1::WorkgroupBf16Array256,
                    GeneralGemmMachineRepresentationGapV1::Wave64Bf16MfmaM16N16K16,
                    GeneralGemmMachineRepresentationGapV1::LoopCarriedF32x4Accumulator,
                ],
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
            Some(CompilerStageV1::Amdgcn),
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

fn encode_symbolic_plan() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(64);
    for expression in symbolic_plan_expressions() {
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
