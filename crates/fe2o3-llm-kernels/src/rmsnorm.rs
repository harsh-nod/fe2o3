//! Exact finite Qwen3 RMSNorm and auxiliary residual-fusion profiles.
//!
//! The generic machine ABI validates behavior, supported width, positive rows,
//! epsilon, and exact element counts. Exact bucket rows and the distinction
//! among machine-equivalent pure hidden operations remain host catalog state;
//! joining that state to a Ferric runner plan is outside this compiler slice.

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
    KernelBindingError, MAX_HSACO_BYTES, inspect_and_bind_kernel_descriptors,
};
use fe2o3_hsaco_finalize::{
    FirstBuildWorkerV2Error, InertDecodedWorkerExchangeV2, InertFirstBuildWorkerV2EvidenceV1,
    LinkOptionV1, PinnedWorkerV1, WorkerExecutionLimitsV1, WorkerOutputConstraintsV1,
    WorkerProtocolError, execute_reproducible_first_build_worker_v2,
};
use fe2o3_llvm_handoff::{
    AddressSpaceV1, AxisV2, BasicBlockV2, BinaryOperationV2, BlockIdV2, CallingConventionV2,
    CastOperationV2, ComparePredicateV2, EvidenceV2, ExecutableModuleV2, FloatBinaryOperationV2,
    FunctionAttributeV1, FunctionAttributeV2, FunctionIdV2, FunctionKindV2, FunctionParameterV2,
    FunctionV2, Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1,
    HandoffDiagnosticV1, HandoffDiagnosticV2, HandoffIdentityV2, IdentityV1, InstructionKindV2,
    InstructionV2, IntegerBinaryOperationV2, IntrinsicReferenceV2, IntrinsicV2, KernelEntryV1,
    KernelParameterV1, KernelValueTypeV1, ModuleFlagV1, ModuleMetadataV1, ObligationKindV1,
    ObligationV1, OriginKindV1, OriginV1, ParameterAttributeV1, ReturnTypeV2, ScalarConstantV2,
    ScalarTypeV1, StageIdentitiesV1, TerminatorV2, TypedValueV2, ValueIdV2, ValueTypeV2,
    WorkgroupSizeRangeV1,
};
use fe2o3_llvm_text::{Gfx942LlvmAssemblyV2, SerializeErrorV2, serialize_gfx942_handoff_v2};
use fe2o3_llvm_worker_handoff::{
    MeasuredLlvmLldBuildV1, WorkerAdmissionErrorV2, WorkerAdmissionIdentityV2,
    WorkerAdmissionRequestV2,
};
use sha2::{Digest as _, Sha256};

/// Exact kernel entry emitted by the typed graph.
pub const QWEN3_RMSNORM_KERNEL_SYMBOL_V1: &str = "qwen3_rmsnorm_v1";
/// Exact AMDHSA descriptor symbol for the typed graph.
pub const QWEN3_RMSNORM_KERNEL_DESCRIPTOR_SYMBOL_V1: &str = "qwen3_rmsnorm_v1.kd";
/// Exact device target required by this compiler lane.
pub const QWEN3_RMSNORM_TARGET_V1: &str = "gfx942:xnack-";
/// Exact code-object version required by this compiler lane.
pub const QWEN3_RMSNORM_CODE_OBJECT_VERSION_V1: u8 = 6;
/// One wave64 workgroup is assigned to each active row.
pub const QWEN3_RMSNORM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Exact Qwen3 RMSNorm epsilon, represented as FP32 bits.
pub const QWEN3_RMSNORM_EPSILON_BITS_V1: u32 = 1.0e-6_f32.to_bits();
/// Number of graph and auxiliary operation profiles across both roles and all buckets.
pub const QWEN3_RMSNORM_PROFILE_COUNT_V1: usize = 132;
/// Explicit kernarg bytes before COV6 padding and hidden arguments.
pub const QWEN3_RMSNORM_EXPLICIT_KERNARG_BYTES_V1: u64 = 96;
/// Offset at which the COV6 hidden argument block begins.
pub const QWEN3_RMSNORM_HIDDEN_KERNARG_OFFSET_V1: u64 = 96;
/// Complete explicit, alignment padding, and COV6 hidden kernarg bytes.
pub const QWEN3_RMSNORM_TOTAL_KERNARG_BYTES_V1: u64 = 96 + 256;
/// Exact kernarg alignment required by the closed ABI.
pub const QWEN3_RMSNORM_KERNARG_ALIGNMENT_V1: u64 = 8;

const CATALOG_DOMAIN: &[u8] = b"FE2O3/QWEN3/RMSNORM/PROFILE-CATALOG/V1\0";
const PROFILE_DOMAIN: &[u8] = b"FE2O3/QWEN3/RMSNORM/PROFILE/V1\0";
const SOURCE_DOMAIN: &[u8] = b"FE2O3/QWEN3/RMSNORM/TYPED-SOURCE/V1\0";
const SEMANTIC_DOMAIN: &[u8] = b"FE2O3/QWEN3/RMSNORM/SEMANTIC-STAGE/V1\0";
const SCHEDULE_DOMAIN: &[u8] = b"FE2O3/QWEN3/RMSNORM/SCHEDULE-STAGE/V1\0";
const TARGET_PLAN_DOMAIN: &[u8] = b"FE2O3/QWEN3/RMSNORM/TARGET-PLAN-STAGE/V1\0";

const REQUIRED_OBLIGATIONS: [ObligationKindV1; 7] = [
    ObligationKindV1::PreserveKernelAbi,
    ObligationKindV1::PreserveAddressSpaces,
    ObligationKindV1::PreserveTargetFeatures,
    ObligationKindV1::PreserveCallingConvention,
    ObligationKindV1::PreserveFunctionAttributes,
    ObligationKindV1::PreserveModuleMetadata,
    ObligationKindV1::MaintainOriginCoverage,
];

/// Target or speculative-draft Qwen3 model role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3RmsNormModelRoleV1 {
    /// Pinned Qwen3-8B target geometry.
    Target8B = 1,
    /// Pinned Qwen3-0.6B draft geometry.
    Draft06B = 2,
}

impl Qwen3RmsNormModelRoleV1 {
    /// Exact hidden width.
    pub const fn hidden_size(self) -> u32 {
        match self {
            Self::Target8B => 4_096,
            Self::Draft06B => 1_024,
        }
    }

    /// Exact query-head count.
    pub const fn query_heads(self) -> u32 {
        match self {
            Self::Target8B => 32,
            Self::Draft06B => 16,
        }
    }

    /// Exact key/value-head count.
    pub const fn key_value_heads(self) -> u32 {
        8
    }
}

/// One exact Ferric M1 mode bucket.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3RmsNormBucketKindV1 {
    /// One sequence with 128 active prefill tokens.
    PrefillS1T128 = 1,
    /// Eight sequences with 128 active prefill tokens each.
    PrefillS8T128 = 2,
    /// One sequence with 512 active prefill tokens.
    PrefillS1T512 = 3,
    /// One sequence with 2,048 active prefill tokens.
    PrefillS1T2048 = 4,
    /// One single-token decode sequence.
    DecodeS1C8192 = 5,
    /// Eight single-token decode sequences.
    DecodeS8C8192 = 6,
    /// Thirty-two single-token decode sequences.
    DecodeS32C8192 = 7,
    /// One speculative sequence with K=4.
    SpeculativeS1K4C8192 = 8,
    /// Eight speculative sequences with K=4.
    SpeculativeS8K4C8192 = 9,
    /// One speculative sequence with K=8.
    SpeculativeS1K8C8192 = 10,
    /// One speculative sequence with K=16.
    SpeculativeS1K16C8192 = 11,
}

impl Qwen3RmsNormBucketKindV1 {
    const fn sequence_and_active_tokens(self, role: Qwen3RmsNormModelRoleV1) -> [u32; 2] {
        match self {
            Self::PrefillS1T128 => [1, 128],
            Self::PrefillS8T128 => [8, 128],
            Self::PrefillS1T512 => [1, 512],
            Self::PrefillS1T2048 => [1, 2_048],
            Self::DecodeS1C8192 => [1, 1],
            Self::DecodeS8C8192 => [8, 1],
            Self::DecodeS32C8192 => [32, 1],
            Self::SpeculativeS1K4C8192 => match role {
                Qwen3RmsNormModelRoleV1::Target8B => [1, 5],
                Qwen3RmsNormModelRoleV1::Draft06B => [1, 4],
            },
            Self::SpeculativeS8K4C8192 => match role {
                Qwen3RmsNormModelRoleV1::Target8B => [8, 5],
                Qwen3RmsNormModelRoleV1::Draft06B => [8, 4],
            },
            Self::SpeculativeS1K8C8192 => match role {
                Qwen3RmsNormModelRoleV1::Target8B => [1, 9],
                Qwen3RmsNormModelRoleV1::Draft06B => [1, 8],
            },
            Self::SpeculativeS1K16C8192 => match role {
                Qwen3RmsNormModelRoleV1::Target8B => [1, 17],
                Qwen3RmsNormModelRoleV1::Draft06B => [1, 16],
            },
        }
    }
}

/// One exact role and mode-bucket selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Qwen3RmsNormBucketV1 {
    role: Qwen3RmsNormModelRoleV1,
    kind: Qwen3RmsNormBucketKindV1,
}

impl Qwen3RmsNormBucketV1 {
    /// Creates one finite target/draft selection.
    pub const fn new(role: Qwen3RmsNormModelRoleV1, kind: Qwen3RmsNormBucketKindV1) -> Self {
        Self { role, kind }
    }

    /// Exact model role.
    pub const fn role(self) -> Qwen3RmsNormModelRoleV1 {
        self.role
    }

    /// Exact bucket kind.
    pub const fn kind(self) -> Qwen3RmsNormBucketKindV1 {
        self.kind
    }

    /// Exact `[sequences, active_tokens]` dimensions.
    pub const fn sequence_and_active_tokens(self) -> [u32; 2] {
        self.kind.sequence_and_active_tokens(self.role)
    }

    /// Number of active rows seen by RMSNorm.
    pub const fn flattened_rows(self) -> u32 {
        let dimensions = self.sequence_and_active_tokens();
        dimensions[0] * dimensions[1]
    }
}

/// SHA-256 identity of one exact profile record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Qwen3RmsNormProfileIdentityV1([u8; 32]);

impl Qwen3RmsNormProfileIdentityV1 {
    /// Returns the domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Runtime behavior selected by the closed mode-tagged ABI.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum Qwen3RmsNormBehaviorV1 {
    /// Pure RMSNorm; the residual pointer/output and lengths must be zero.
    Pure = 0,
    /// Add a residual before normalization and write the fused value.
    ResidualFused = 1,
}

/// Exact graph operator or explicitly separate auxiliary fused operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3RmsNormOperationV1 {
    /// Per-layer hidden-width input RMSNorm.
    InputRmsNorm = 1,
    /// Per-query-head RMSNorm over head width 128.
    QueryRmsNorm = 2,
    /// Per-key-head RMSNorm over head width 128.
    KeyRmsNorm = 3,
    /// Per-layer hidden-width post-attention RMSNorm.
    PostAttentionRmsNorm = 4,
    /// Final hidden-width RMSNorm.
    FinalRmsNorm = 5,
    /// Auxiliary hidden-width residual-add plus RMSNorm, not a Ferric graph RMSNorm op.
    ResidualFusedHidden = 6,
}

impl Qwen3RmsNormOperationV1 {
    /// Exact ABI behavior required by this operation.
    pub const fn behavior(self) -> Qwen3RmsNormBehaviorV1 {
        match self {
            Self::ResidualFusedHidden => Qwen3RmsNormBehaviorV1::ResidualFused,
            Self::InputRmsNorm
            | Self::QueryRmsNorm
            | Self::KeyRmsNorm
            | Self::PostAttentionRmsNorm
            | Self::FinalRmsNorm => Qwen3RmsNormBehaviorV1::Pure,
        }
    }

    const fn rows_and_width(self, bucket: Qwen3RmsNormBucketV1) -> Option<[u32; 2]> {
        let base_rows = bucket.flattened_rows();
        match self {
            Self::QueryRmsNorm => match base_rows.checked_mul(bucket.role.query_heads()) {
                Some(rows) => Some([rows, 128]),
                None => None,
            },
            Self::KeyRmsNorm => match base_rows.checked_mul(bucket.role.key_value_heads()) {
                Some(rows) => Some([rows, 128]),
                None => None,
            },
            Self::InputRmsNorm
            | Self::PostAttentionRmsNorm
            | Self::FinalRmsNorm
            | Self::ResidualFusedHidden => Some([base_rows, bucket.role.hidden_size()]),
        }
    }
}

const fn behavior_accepts_width(behavior: Qwen3RmsNormBehaviorV1, width: u32) -> bool {
    match behavior {
        Qwen3RmsNormBehaviorV1::Pure => matches!(width, 128 | 1_024 | 4_096),
        Qwen3RmsNormBehaviorV1::ResidualFused => matches!(width, 1_024 | 4_096),
    }
}

/// One finite checked Qwen3 RMSNorm operation profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3RmsNormProfileV1 {
    bucket: Qwen3RmsNormBucketV1,
    operation: Qwen3RmsNormOperationV1,
    rows: u32,
    width: u32,
    row_elements: u64,
    block_counts: [u32; 3],
    aql_grid_work_items: [u32; 3],
    identity: Qwen3RmsNormProfileIdentityV1,
}

impl Qwen3RmsNormProfileV1 {
    fn checked(
        bucket: Qwen3RmsNormBucketV1,
        operation: Qwen3RmsNormOperationV1,
    ) -> Result<Self, Qwen3RmsNormCatalogErrorV1> {
        let [rows, width] = operation
            .rows_and_width(bucket)
            .ok_or(Qwen3RmsNormCatalogErrorV1::ExtentOverflow)?;
        if !behavior_accepts_width(operation.behavior(), width) {
            return Err(Qwen3RmsNormCatalogErrorV1::OperationGeometry);
        }
        let row_elements = u64::from(rows)
            .checked_mul(u64::from(width))
            .ok_or(Qwen3RmsNormCatalogErrorV1::ExtentOverflow)?;
        let grid_x = rows
            .checked_mul(QWEN3_RMSNORM_WORKGROUP_V1[0])
            .ok_or(Qwen3RmsNormCatalogErrorV1::GridOverflow)?;
        let mut profile = Self {
            bucket,
            operation,
            rows,
            width,
            row_elements,
            block_counts: [rows, 1, 1],
            aql_grid_work_items: [grid_x, 1, 1],
            identity: Qwen3RmsNormProfileIdentityV1([0; 32]),
        };
        profile.identity = Qwen3RmsNormProfileIdentityV1(hash(PROFILE_DOMAIN, &profile.encode()));
        Ok(profile)
    }

    fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(80);
        bytes.push(self.bucket.role as u8);
        bytes.push(self.bucket.kind as u8);
        bytes.push(self.operation as u8);
        bytes.extend_from_slice(&(self.operation.behavior() as u32).to_le_bytes());
        bytes.extend_from_slice(&self.rows.to_le_bytes());
        bytes.extend_from_slice(&self.width.to_le_bytes());
        bytes.extend_from_slice(&self.row_elements.to_le_bytes());
        for value in self.block_counts {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.aql_grid_work_items {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&QWEN3_RMSNORM_EPSILON_BITS_V1.to_le_bytes());
        bytes
    }

    /// Exact role and bucket selection.
    pub const fn bucket(self) -> Qwen3RmsNormBucketV1 {
        self.bucket
    }

    /// Exact graph or auxiliary operation variant.
    pub const fn operation(self) -> Qwen3RmsNormOperationV1 {
        self.operation
    }

    /// Exact mode-tagged ABI behavior.
    pub const fn behavior(self) -> Qwen3RmsNormBehaviorV1 {
        self.operation.behavior()
    }

    /// Exact active row count.
    pub const fn rows(self) -> u32 {
        self.rows
    }

    /// Exact normalization width: hidden width or per-head width 128.
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Elements in each row-shaped input or output buffer.
    pub const fn row_elements(self) -> u64 {
        self.row_elements
    }

    /// Elements in the one-dimensional RMSNorm weight buffer.
    pub const fn weight_elements(self) -> u64 {
        self.width as u64
    }

    /// Exact Qwen3 epsilon bits.
    pub const fn epsilon_bits(self) -> u32 {
        QWEN3_RMSNORM_EPSILON_BITS_V1
    }

    /// HSA-adapter block counts before workgroup expansion.
    pub const fn hsa_adapter_block_counts(self) -> [u32; 3] {
        self.block_counts
    }

    /// Exact AQL total-workitem grid.
    pub const fn aql_grid_work_items(self) -> [u32; 3] {
        self.aql_grid_work_items
    }

    /// Exact domain-separated profile identity.
    pub const fn identity(self) -> Qwen3RmsNormProfileIdentityV1 {
        self.identity
    }

    /// Geometry and numerical declarations grant no launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Failure while deriving the finite catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3RmsNormCatalogErrorV1 {
    /// An operation selected a width outside its exact behavior contract.
    OperationGeometry,
    /// A row extent overflowed `u64`.
    ExtentOverflow,
    /// Workgroup expansion overflowed the AQL grid domain.
    GridOverflow,
}

impl fmt::Display for Qwen3RmsNormCatalogErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 RMSNorm catalog failed: {self:?}")
    }
}

impl std::error::Error for Qwen3RmsNormCatalogErrorV1 {}

/// SHA-256 identity of the complete finite catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Qwen3RmsNormProfileCatalogIdentityV1([u8; 32]);

impl Qwen3RmsNormProfileCatalogIdentityV1 {
    /// Returns the exact catalog identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Complete finite target/draft RMSNorm profile catalog.
#[derive(Debug, Eq, PartialEq)]
pub struct Qwen3RmsNormProfileCatalogV1 {
    profiles: Box<[Qwen3RmsNormProfileV1]>,
    canonical_bytes: Box<[u8]>,
    identity: Qwen3RmsNormProfileCatalogIdentityV1,
}

impl Qwen3RmsNormProfileCatalogV1 {
    /// Constructs all 132 profiles in stable role/bucket/operation order.
    pub fn canonical() -> Result<Self, Qwen3RmsNormCatalogErrorV1> {
        let mut profiles = Vec::with_capacity(QWEN3_RMSNORM_PROFILE_COUNT_V1);
        for role in QWEN3_RMSNORM_ROLES_V1 {
            for kind in QWEN3_RMSNORM_BUCKET_KINDS_V1 {
                for operation in QWEN3_RMSNORM_OPERATIONS_V1 {
                    profiles.push(Qwen3RmsNormProfileV1::checked(
                        Qwen3RmsNormBucketV1::new(role, kind),
                        operation,
                    )?);
                }
            }
        }
        let mut canonical_bytes = Vec::with_capacity(2_048);
        canonical_bytes.extend_from_slice(&(profiles.len() as u32).to_le_bytes());
        canonical_bytes.extend_from_slice(QWEN3_RMSNORM_TARGET_V1.as_bytes());
        canonical_bytes.push(QWEN3_RMSNORM_CODE_OBJECT_VERSION_V1);
        for profile in &profiles {
            let encoded = profile.encode();
            canonical_bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            canonical_bytes.extend_from_slice(&encoded);
            canonical_bytes.extend_from_slice(profile.identity.as_bytes());
        }
        let identity = Qwen3RmsNormProfileCatalogIdentityV1(hash(CATALOG_DOMAIN, &canonical_bytes));
        Ok(Self {
            profiles: profiles.into_boxed_slice(),
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            identity,
        })
    }

    /// Exact stable profile roster.
    pub fn profiles(&self) -> &[Qwen3RmsNormProfileV1] {
        &self.profiles
    }

    /// Finds one exact role/bucket/operation profile.
    pub fn profile(
        &self,
        bucket: Qwen3RmsNormBucketV1,
        operation: Qwen3RmsNormOperationV1,
    ) -> Option<Qwen3RmsNormProfileV1> {
        self.profiles
            .iter()
            .copied()
            .find(|profile| profile.bucket == bucket && profile.operation == operation)
    }

    /// Canonical bytes retaining every checked shape and grid.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Exact catalog identity.
    pub const fn identity(&self) -> Qwen3RmsNormProfileCatalogIdentityV1 {
        self.identity
    }

    /// The catalog is structural and authenticates no source or artifact.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

const QWEN3_RMSNORM_ROLES_V1: [Qwen3RmsNormModelRoleV1; 2] = [
    Qwen3RmsNormModelRoleV1::Target8B,
    Qwen3RmsNormModelRoleV1::Draft06B,
];

const QWEN3_RMSNORM_BUCKET_KINDS_V1: [Qwen3RmsNormBucketKindV1; 11] = [
    Qwen3RmsNormBucketKindV1::PrefillS1T128,
    Qwen3RmsNormBucketKindV1::PrefillS8T128,
    Qwen3RmsNormBucketKindV1::PrefillS1T512,
    Qwen3RmsNormBucketKindV1::PrefillS1T2048,
    Qwen3RmsNormBucketKindV1::DecodeS1C8192,
    Qwen3RmsNormBucketKindV1::DecodeS8C8192,
    Qwen3RmsNormBucketKindV1::DecodeS32C8192,
    Qwen3RmsNormBucketKindV1::SpeculativeS1K4C8192,
    Qwen3RmsNormBucketKindV1::SpeculativeS8K4C8192,
    Qwen3RmsNormBucketKindV1::SpeculativeS1K8C8192,
    Qwen3RmsNormBucketKindV1::SpeculativeS1K16C8192,
];

const QWEN3_RMSNORM_OPERATIONS_V1: [Qwen3RmsNormOperationV1; 6] = [
    Qwen3RmsNormOperationV1::InputRmsNorm,
    Qwen3RmsNormOperationV1::QueryRmsNorm,
    Qwen3RmsNormOperationV1::KeyRmsNorm,
    Qwen3RmsNormOperationV1::PostAttentionRmsNorm,
    Qwen3RmsNormOperationV1::FinalRmsNorm,
    Qwen3RmsNormOperationV1::ResidualFusedHidden,
];

/// One exact buffer role in the fused residual and RMSNorm ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Qwen3RmsNormBufferV1 {
    /// BF16 layer input.
    Input = 0,
    /// BF16 residual input.
    Residual = 1,
    /// BF16 per-normalization-width weight.
    Weight = 2,
    /// BF16 fused input-plus-residual output.
    FusedResidualOutput = 3,
    /// BF16 normalized and weighted output.
    NormalizedOutput = 4,
}

/// Numerical buffer-contract rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3RmsNormBufferContractErrorV1 {
    /// A pure profile did not use exact zero for an inactive pointer.
    InactiveAddress(Qwen3RmsNormBufferV1),
    /// A numerical address was zero.
    ZeroAddress(Qwen3RmsNormBufferV1),
    /// The available byte span differed from the exact selected profile.
    ByteLength(Qwen3RmsNormBufferV1),
    /// A numerical address was not BF16 aligned.
    Alignment(Qwen3RmsNormBufferV1),
    /// A half-open range overflowed `u64`.
    RangeOverflow(Qwen3RmsNormBufferV1),
    /// Two declared buffer ranges overlap.
    Aliasing,
}

impl fmt::Display for Qwen3RmsNormBufferContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 RMSNorm buffer contract failed: {self:?}")
    }
}

impl std::error::Error for Qwen3RmsNormBufferContractErrorV1 {}

/// Inert checked numerical layout for the mode-tagged five-buffer ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3RmsNormBufferContractV1 {
    behavior: Qwen3RmsNormBehaviorV1,
    addresses: [u64; 5],
    ends: [u64; 5],
    lengths: [u64; 5],
}

impl Qwen3RmsNormBufferContractV1 {
    /// Checks exact spans, BF16 alignment, overflow, and pairwise disjointness.
    pub fn checked(
        profile: Qwen3RmsNormProfileV1,
        addresses: [u64; 5],
        lengths: [u64; 5],
    ) -> Result<Self, Qwen3RmsNormBufferContractErrorV1> {
        let row_bytes = profile.row_elements.checked_mul(2).ok_or(
            Qwen3RmsNormBufferContractErrorV1::ByteLength(Qwen3RmsNormBufferV1::Input),
        )?;
        let weight_bytes = profile.weight_elements().checked_mul(2).ok_or(
            Qwen3RmsNormBufferContractErrorV1::ByteLength(Qwen3RmsNormBufferV1::Weight),
        )?;
        let expected = match profile.behavior() {
            Qwen3RmsNormBehaviorV1::Pure => [row_bytes, 0, weight_bytes, 0, row_bytes],
            Qwen3RmsNormBehaviorV1::ResidualFused => {
                [row_bytes, row_bytes, weight_bytes, row_bytes, row_bytes]
            }
        };
        let roles = [
            Qwen3RmsNormBufferV1::Input,
            Qwen3RmsNormBufferV1::Residual,
            Qwen3RmsNormBufferV1::Weight,
            Qwen3RmsNormBufferV1::FusedResidualOutput,
            Qwen3RmsNormBufferV1::NormalizedOutput,
        ];
        let mut ends = [0; 5];
        for index in 0..5 {
            if expected[index] == 0 && addresses[index] != 0 {
                return Err(Qwen3RmsNormBufferContractErrorV1::InactiveAddress(
                    roles[index],
                ));
            }
            if expected[index] != 0 && addresses[index] == 0 {
                return Err(Qwen3RmsNormBufferContractErrorV1::ZeroAddress(roles[index]));
            }
            if lengths[index] != expected[index] {
                return Err(Qwen3RmsNormBufferContractErrorV1::ByteLength(roles[index]));
            }
            if expected[index] != 0 && !addresses[index].is_multiple_of(2) {
                return Err(Qwen3RmsNormBufferContractErrorV1::Alignment(roles[index]));
            }
            ends[index] = if expected[index] == 0 {
                0
            } else {
                addresses[index].checked_add(lengths[index]).ok_or(
                    Qwen3RmsNormBufferContractErrorV1::RangeOverflow(roles[index]),
                )?
            };
        }
        for left in 0..5 {
            for right in left + 1..5 {
                if expected[left] != 0
                    && expected[right] != 0
                    && addresses[left] < ends[right]
                    && addresses[right] < ends[left]
                {
                    return Err(Qwen3RmsNormBufferContractErrorV1::Aliasing);
                }
            }
        }
        Ok(Self {
            behavior: profile.behavior(),
            addresses,
            ends,
            lengths,
        })
    }

    /// Exact mode tag carried by the ABI.
    pub const fn behavior(self) -> Qwen3RmsNormBehaviorV1 {
        self.behavior
    }

    /// Exact starts in ABI role order.
    pub const fn addresses(self) -> [u64; 5] {
        self.addresses
    }

    /// Exact exclusive ends in ABI role order.
    pub const fn ends(self) -> [u64; 5] {
        self.ends
    }

    /// Exact byte lengths in ABI role order.
    pub const fn byte_lengths(self) -> [u64; 5] {
        self.lengths
    }

    /// Integer checks do not authenticate mappings, leases, or contents.
    pub const fn authenticates_device_memory(self) -> bool {
        false
    }

    /// A numerical layout grants no launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Four inert identities labeling compiler stages preceding this bounded graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3RmsNormSourceBindingsV1 {
    source: [u8; 32],
    semantic: [u8; 32],
    schedule: [u8; 32],
    target_plan: [u8; 32],
}

impl Qwen3RmsNormSourceBindingsV1 {
    /// Constructs inert labels. Preparation requires all four to be nonzero and distinct.
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

    /// Caller labels authenticate no source, producer, compiler, or target provenance.
    pub const fn authenticates_provenance(self) -> bool {
        false
    }
}

/// Failure while preparing the typed BF16 RMSNorm compiler handoff.
#[derive(Debug)]
pub enum PrepareQwen3RmsNormKernelErrorV1 {
    /// A source label was zero or reused for another role.
    SourceBindings,
    /// The finite profile catalog failed closed.
    Catalog(Qwen3RmsNormCatalogErrorV1),
    /// The Handoff V1 policy object rejected a field.
    HandoffV1(HandoffDiagnosticV1),
    /// The executable Handoff V2 graph rejected a field.
    HandoffV2(HandoffDiagnosticV2),
    /// Canonical LLVM serialization failed.
    Serialize(SerializeErrorV2),
    /// The typed graph did not survive exact serializer identity binding.
    SourceIdentity,
    /// Exact LLVM/LLD Worker policy admission failed.
    WorkerAdmission(WorkerAdmissionErrorV2),
    /// The compiler-FFI envelope failed closed.
    CompilerEnvelope(CompilerFfiEnvelopeError),
    /// The closed two-symbol manifest failed closed.
    SymbolManifest(CompilerModuleSymbolManifestErrorV1),
    /// The Handoff V2 compiler module failed closed.
    CompilerHandoff(CompilerModuleHandoffErrorV2),
}

impl fmt::Display for PrepareQwen3RmsNormKernelErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 RMSNorm preparation failed: {self:?}")
    }
}

impl std::error::Error for PrepareQwen3RmsNormKernelErrorV1 {}

/// Linear prepared typed graph and canonical Handoff V2 compiler module.
pub struct PreparedQwen3RmsNormKernelV1 {
    catalog: Qwen3RmsNormProfileCatalogV1,
    source_identity: HandoffIdentityV2,
    worker_admission_identity: WorkerAdmissionIdentityV2,
    assembly: Gfx942LlvmAssemblyV2,
    compiler_handoff_identity: CompilerModuleHandoffIdentityV2,
    manifest_identity: CompilerModuleSymbolManifestIdentityV1,
    compiler_handoff: CompilerModuleHandoffV2,
}

impl fmt::Debug for PreparedQwen3RmsNormKernelV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedQwen3RmsNormKernelV1")
            .field("catalog", &self.catalog.identity)
            .field("source_identity", &self.source_identity)
            .field("worker_admission", &self.worker_admission_identity)
            .field("assembly_sha256", &self.assembly.sha256())
            .field("compiler_handoff", &self.compiler_handoff_identity)
            .finish_non_exhaustive()
    }
}

impl PreparedQwen3RmsNormKernelV1 {
    /// Complete exact profile catalog retained with this graph owner.
    pub const fn catalog(&self) -> &Qwen3RmsNormProfileCatalogV1 {
        &self.catalog
    }

    /// Identity of the complete typed Handoff V2 graph.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.source_identity
    }

    /// Identity binding the typed graph to exact LLVM/LLD policy admission.
    pub const fn worker_admission_identity(&self) -> WorkerAdmissionIdentityV2 {
        self.worker_admission_identity
    }

    /// SHA-256 of the exact canonical LLVM assembly.
    pub const fn assembly_sha256(&self) -> fe2o3_llvm_text::LlvmAssemblySha256V2 {
        self.assembly.sha256()
    }

    /// Exact canonical LLVM assembly byte length.
    pub fn assembly_len(&self) -> u64 {
        self.assembly.as_bytes().len() as u64
    }

    /// Complete canonical compiler handoff identity.
    pub const fn compiler_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.compiler_handoff_identity
    }

    /// Closed entry/descriptor symbol manifest identity.
    pub const fn manifest_identity(&self) -> CompilerModuleSymbolManifestIdentityV1 {
        self.manifest_identity
    }

    /// Borrows the exact Handoff V2 compiler module for attempt-scoped publication.
    pub const fn compiler_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.compiler_handoff
    }

    /// Current Pliron lowering cannot represent scalar BF16 and was not used.
    pub const fn uses_pliron_lowering(&self) -> bool {
        false
    }

    /// Typed graph construction is not a source-to-LLVM refinement proof.
    pub const fn proves_machine_refinement(&self) -> bool {
        false
    }

    /// Preparation does not establish RMSNorm numerical correctness.
    pub const fn proves_numerical_contract(&self) -> bool {
        false
    }

    /// Preparation does not establish that Worker V2 executed.
    pub const fn authenticates_worker_execution(&self) -> bool {
        false
    }

    /// Preparation grants no artifact, load, or launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Constructs the exact catalog and one runtime-parameterized typed BF16 graph.
pub fn prepare_qwen3_rmsnorm_kernel_v1(
    bindings: Qwen3RmsNormSourceBindingsV1,
) -> Result<PreparedQwen3RmsNormKernelV1, PrepareQwen3RmsNormKernelErrorV1> {
    validate_source_bindings(bindings)?;
    let catalog = Qwen3RmsNormProfileCatalogV1::canonical()
        .map_err(PrepareQwen3RmsNormKernelErrorV1::Catalog)?;
    let handoff = construct_typed_handoff(&catalog, bindings)?;
    let source_identity = handoff.identity();
    let canonical = handoff.encode_canonical();
    let worker_admission = WorkerAdmissionRequestV2::new(
        canonical.as_bytes(),
        *source_identity.as_bytes(),
        MeasuredLlvmLldBuildV1::exact(),
    )
    .admit()
    .map_err(PrepareQwen3RmsNormKernelErrorV1::WorkerAdmission)?;
    if worker_admission.handoff() != &handoff
        || worker_admission.handoff_identity() != source_identity
    {
        return Err(PrepareQwen3RmsNormKernelErrorV1::SourceIdentity);
    }
    let worker_admission_identity = worker_admission.admission_identity();
    let assembly = serialize_gfx942_handoff_v2(worker_admission.handoff())
        .map_err(PrepareQwen3RmsNormKernelErrorV1::Serialize)?;
    if assembly.source_identity() != source_identity || !assembly.has_embedded_source_identity() {
        return Err(PrepareQwen3RmsNormKernelErrorV1::SourceIdentity);
    }
    let target = exact_target();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(PrepareQwen3RmsNormKernelErrorV1::CompilerEnvelope)?;
    let manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            QWEN3_RMSNORM_KERNEL_SYMBOL_V1,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            QWEN3_RMSNORM_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ),
    ])
    .map_err(PrepareQwen3RmsNormKernelErrorV1::SymbolManifest)?;
    let manifest_identity = manifest.identity();
    let compiler_handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        assembly.as_bytes(),
    )
    .map_err(PrepareQwen3RmsNormKernelErrorV1::CompilerHandoff)?;
    let compiler_handoff_identity = compiler_handoff.identity();
    Ok(PreparedQwen3RmsNormKernelV1 {
        catalog,
        source_identity,
        worker_admission_identity,
        assembly,
        compiler_handoff_identity,
        manifest_identity,
        compiler_handoff,
    })
}

fn validate_source_bindings(
    bindings: Qwen3RmsNormSourceBindingsV1,
) -> Result<(), PrepareQwen3RmsNormKernelErrorV1> {
    let identities = [
        bindings.source,
        bindings.semantic,
        bindings.schedule,
        bindings.target_plan,
    ];
    for (index, identity) in identities.iter().enumerate() {
        if identity == &[0; 32] || identities[index + 1..].contains(identity) {
            return Err(PrepareQwen3RmsNormKernelErrorV1::SourceBindings);
        }
    }
    Ok(())
}

fn construct_typed_handoff(
    catalog: &Qwen3RmsNormProfileCatalogV1,
    bindings: Qwen3RmsNormSourceBindingsV1,
) -> Result<Gfx942HandoffV2, PrepareQwen3RmsNormKernelErrorV1> {
    let source_bytes = bound_stage_identity(SOURCE_DOMAIN, bindings.source, catalog.identity);
    let semantic_bytes = bound_stage_identity(SEMANTIC_DOMAIN, bindings.semantic, catalog.identity);
    let schedule_bytes = bound_stage_identity(SCHEDULE_DOMAIN, bindings.schedule, catalog.identity);
    let target_plan_bytes =
        bound_stage_identity(TARGET_PLAN_DOMAIN, bindings.target_plan, catalog.identity);
    let source =
        IdentityV1::new(source_bytes).map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV1)?;
    let stages = StageIdentitiesV1::new(semantic_bytes, schedule_bytes, target_plan_bytes)
        .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV1)?;
    let origin = OriginV1::new(OriginKindV1::KernelIr, source, None);
    let kernel_attributes = exact_function_attributes();
    let kernel = KernelEntryV1::new(
        QWEN3_RMSNORM_KERNEL_SYMBOL_V1,
        kernel_parameters_v1().map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV1)?,
        kernel_attributes.clone(),
        origin.identity(),
    )
    .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV1)?;
    let obligations = REQUIRED_OBLIGATIONS
        .into_iter()
        .map(|kind| {
            let subject = match kind {
                ObligationKindV1::PreserveKernelAbi | ObligationKindV1::MaintainOriginCoverage => {
                    stages.semantic()
                }
                _ => stages.target_plan(),
            };
            ObligationV1::new(kind, subject, origin.identity())
        })
        .collect();
    let module_metadata = ModuleMetadataV1::new(
        vec![
            ModuleFlagV1::CodeObjectVersion6,
            ModuleFlagV1::PicLevel2,
            ModuleFlagV1::WcharSize4,
        ],
        vec![],
        vec![],
    )
    .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV1)?;
    let base = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: stages,
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module: module_metadata,
        origins: vec![origin],
        obligations,
    })
    .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV1)?;
    let evidence = EvidenceV2::new(
        base.origins()[0].identity(),
        base.obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect(),
    )
    .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV2)?;
    let function = build_kernel_function(kernel_attributes, evidence.clone())?;
    let intrinsics = [
        IntrinsicV2::AmdGpuWorkitemId(AxisV2::X),
        IntrinsicV2::AmdGpuWorkgroupId(AxisV2::X),
        IntrinsicV2::SqrtF32,
        IntrinsicV2::Trap,
    ]
    .into_iter()
    .map(|intrinsic| IntrinsicReferenceV2::new(intrinsic, evidence.clone()))
    .collect();
    let module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![],
        intrinsics,
        vec![function],
    )
    .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV2)?;
    Gfx942HandoffV2::new(base, module).map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV2)
}

fn kernel_parameters_v1() -> Result<Vec<KernelParameterV1>, HandoffDiagnosticV1> {
    let global_bf16 = KernelValueTypeV1::Pointer {
        pointee: ScalarTypeV1::Bf16,
        address_space: AddressSpaceV1::Global,
    };
    let required_readonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(2),
    ];
    let optional_readonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(2),
    ];
    let required_writeonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::WriteOnly,
        ParameterAttributeV1::Align(2),
    ];
    let optional_writeonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::WriteOnly,
        ParameterAttributeV1::Align(2),
    ];
    [
        ("input_bf16", global_bf16, required_readonly.clone()),
        (
            "input_elements",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("residual_bf16", global_bf16, optional_readonly),
        (
            "residual_elements",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("weight_bf16", global_bf16, required_readonly),
        (
            "weight_elements",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("fused_residual_bf16", global_bf16, optional_writeonly),
        (
            "fused_residual_elements",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("normalized_bf16", global_bf16, required_writeonly),
        (
            "normalized_elements",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I64),
            vec![],
        ),
        ("rows", KernelValueTypeV1::Scalar(ScalarTypeV1::I32), vec![]),
        (
            "width",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I32),
            vec![],
        ),
        (
            "epsilon",
            KernelValueTypeV1::Scalar(ScalarTypeV1::F32),
            vec![],
        ),
        (
            "behavior",
            KernelValueTypeV1::Scalar(ScalarTypeV1::I32),
            vec![],
        ),
    ]
    .into_iter()
    .map(|(name, value_type, attributes)| KernelParameterV1::new(name, value_type, attributes))
    .collect()
}

fn exact_function_attributes() -> Vec<FunctionAttributeV1> {
    FunctionAttributeV1::gfx942_kernel_defaults(
        WorkgroupSizeRangeV1::new(64, 64).expect("the fixed wave64 bound is valid"),
    )
}

#[derive(Clone, Copy)]
struct KernelValues {
    input: ValueIdV2,
    input_len: ValueIdV2,
    residual: ValueIdV2,
    residual_len: ValueIdV2,
    weight: ValueIdV2,
    weight_len: ValueIdV2,
    fused_output: ValueIdV2,
    fused_output_len: ValueIdV2,
    normalized_output: ValueIdV2,
    normalized_output_len: ValueIdV2,
    rows: ValueIdV2,
    width: ValueIdV2,
    epsilon: ValueIdV2,
    behavior: ValueIdV2,
}

impl KernelValues {
    const fn fixed() -> Self {
        Self {
            input: ValueIdV2::new(1),
            input_len: ValueIdV2::new(2),
            residual: ValueIdV2::new(3),
            residual_len: ValueIdV2::new(4),
            weight: ValueIdV2::new(5),
            weight_len: ValueIdV2::new(6),
            fused_output: ValueIdV2::new(7),
            fused_output_len: ValueIdV2::new(8),
            normalized_output: ValueIdV2::new(9),
            normalized_output_len: ValueIdV2::new(10),
            rows: ValueIdV2::new(11),
            width: ValueIdV2::new(12),
            epsilon: ValueIdV2::new(13),
            behavior: ValueIdV2::new(14),
        }
    }
}

struct TypedFunctionBuilder {
    evidence: EvidenceV2,
    current_id: BlockIdV2,
    current: Vec<InstructionV2>,
    blocks: Vec<BasicBlockV2>,
    next_block: u32,
    next_value: u32,
}

impl TypedFunctionBuilder {
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
            .expect("closed RMSNorm instruction shape is valid"),
        );
    }

    fn void(&mut self, kind: InstructionKindV2) {
        self.current.push(
            InstructionV2::new(None, kind, self.evidence.clone())
                .expect("closed RMSNorm void instruction shape is valid"),
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
                    .expect("closed RMSNorm constants fit their scalar type"),
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

    fn bf16_load(&mut self, base: ValueIdV2, index: ValueIdV2) -> ValueIdV2 {
        let pointer = self.instruction(
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::Bf16,
                address_space: AddressSpaceV1::Global,
            },
            InstructionKindV2::GetElementPtr {
                base,
                indices: vec![index],
            },
        );
        self.instruction(
            ValueTypeV2::Scalar(ScalarTypeV1::Bf16),
            InstructionKindV2::Load {
                pointer,
                value_type: ScalarTypeV1::Bf16,
                alignment: 2,
            },
        )
    }

    fn bf16_store(&mut self, base: ValueIdV2, index: ValueIdV2, value: ValueIdV2) {
        let pointer = self.instruction(
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::Bf16,
                address_space: AddressSpaceV1::Global,
            },
            InstructionKindV2::GetElementPtr {
                base,
                indices: vec![index],
            },
        );
        self.void(InstructionKindV2::Store {
            pointer,
            value,
            value_type: ScalarTypeV1::Bf16,
            alignment: 2,
        });
    }
}

fn build_kernel_function(
    attributes: Vec<FunctionAttributeV1>,
    evidence: EvidenceV2,
) -> Result<FunctionV2, PrepareQwen3RmsNormKernelErrorV1> {
    let values = KernelValues::fixed();
    let mut builder = TypedFunctionBuilder::new(evidence.clone());
    let i32_type = ValueTypeV2::Scalar(ScalarTypeV1::I32);
    let i64_type = ValueTypeV2::Scalar(ScalarTypeV1::I64);
    let f32_type = ValueTypeV2::Scalar(ScalarTypeV1::F32);
    let bf16_type = ValueTypeV2::Scalar(ScalarTypeV1::Bf16);
    let zero_i1 = builder.constant(ScalarTypeV1::I1, 0);
    let zero_i32 = builder.constant(ScalarTypeV1::I32, 0);
    let fused_mode_i32 = builder.constant(ScalarTypeV1::I32, 1);
    let one_i64 = builder.constant(ScalarTypeV1::I64, 1);
    let zero_i64 = builder.constant(ScalarTypeV1::I64, 0);
    let zero_f32 = builder.constant(ScalarTypeV1::F32, 0.0_f32.to_bits().into());
    let one_f32 = builder.constant(ScalarTypeV1::F32, 1.0_f32.to_bits().into());
    let expected_epsilon =
        builder.constant(ScalarTypeV1::F32, QWEN3_RMSNORM_EPSILON_BITS_V1.into());
    let target_width_value = builder.constant(ScalarTypeV1::I32, 4_096);
    let draft_width_value = builder.constant(ScalarTypeV1::I32, 1_024);
    let head_width_value = builder.constant(ScalarTypeV1::I32, 128);
    let rows_nonzero = builder.compare(ComparePredicateV2::IntegerNotEqual, values.rows, zero_i32);
    let target_width = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.width,
        target_width_value,
    );
    let draft_width = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.width,
        draft_width_value,
    );
    let head_width = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.width,
        head_width_value,
    );
    let hidden_width = builder.or(target_width, draft_width);
    let known_width = builder.or(hidden_width, head_width);
    let pure_mode = builder.compare(ComparePredicateV2::IntegerEqual, values.behavior, zero_i32);
    let fused_mode = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.behavior,
        fused_mode_i32,
    );
    let epsilon_exact = builder.compare(
        ComparePredicateV2::OrderedEqual,
        values.epsilon,
        expected_epsilon,
    );
    let rows64 = builder.cast(CastOperationV2::ZeroExtend, values.rows, i64_type);
    let width64 = builder.cast(CastOperationV2::ZeroExtend, values.width, i64_type);
    let row_elements = builder.integer(
        IntegerBinaryOperationV2::Multiply,
        rows64,
        width64,
        ScalarTypeV1::I64,
    );
    let input_exact = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.input_len,
        row_elements,
    );
    let residual_exact = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.residual_len,
        row_elements,
    );
    let weight_exact =
        builder.compare(ComparePredicateV2::IntegerEqual, values.weight_len, width64);
    let fused_output_exact = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.fused_output_len,
        row_elements,
    );
    let normalized_output_exact = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.normalized_output_len,
        row_elements,
    );
    let residual_zero = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.residual_len,
        zero_i64,
    );
    let fused_output_zero = builder.compare(
        ComparePredicateV2::IntegerEqual,
        values.fused_output_len,
        zero_i64,
    );
    let input_address = builder.cast(CastOperationV2::PointerToInt, values.input, i64_type);
    let residual_address = builder.cast(CastOperationV2::PointerToInt, values.residual, i64_type);
    let weight_address = builder.cast(CastOperationV2::PointerToInt, values.weight, i64_type);
    let fused_output_address =
        builder.cast(CastOperationV2::PointerToInt, values.fused_output, i64_type);
    let normalized_address = builder.cast(
        CastOperationV2::PointerToInt,
        values.normalized_output,
        i64_type,
    );
    let input_nonzero =
        builder.compare(ComparePredicateV2::IntegerNotEqual, input_address, zero_i64);
    let weight_nonzero = builder.compare(
        ComparePredicateV2::IntegerNotEqual,
        weight_address,
        zero_i64,
    );
    let normalized_nonzero = builder.compare(
        ComparePredicateV2::IntegerNotEqual,
        normalized_address,
        zero_i64,
    );
    let residual_address_zero =
        builder.compare(ComparePredicateV2::IntegerEqual, residual_address, zero_i64);
    let fused_address_zero = builder.compare(
        ComparePredicateV2::IntegerEqual,
        fused_output_address,
        zero_i64,
    );
    let residual_address_nonzero = builder.compare(
        ComparePredicateV2::IntegerNotEqual,
        residual_address,
        zero_i64,
    );
    let fused_address_nonzero = builder.compare(
        ComparePredicateV2::IntegerNotEqual,
        fused_output_address,
        zero_i64,
    );
    let pure_lengths = builder.and(residual_zero, fused_output_zero);
    let pure_addresses = builder.and(residual_address_zero, fused_address_zero);
    let pure_auxiliary = builder.and(pure_lengths, pure_addresses);
    let pure_buffers = builder.and(pure_mode, pure_auxiliary);
    let fused_lengths = builder.and(residual_exact, fused_output_exact);
    let fused_addresses = builder.and(residual_address_nonzero, fused_address_nonzero);
    let fused_auxiliary = builder.and(fused_lengths, fused_addresses);
    let fused_buffers = builder.and(fused_mode, fused_auxiliary);
    let mode_buffers_valid = builder.or(pure_buffers, fused_buffers);
    let required_lengths = builder.and(input_exact, weight_exact);
    let required_lengths = builder.and(required_lengths, normalized_output_exact);
    let required_addresses = builder.and(input_nonzero, weight_nonzero);
    let required_addresses = builder.and(required_addresses, normalized_nonzero);
    let required_buffers = builder.and(required_lengths, required_addresses);
    let pure_shape = builder.and(pure_mode, known_width);
    let fused_shape = builder.and(fused_mode, hidden_width);
    let mode_shape = builder.or(pure_shape, fused_shape);
    let shape_and_mode = builder.and(rows_nonzero, mode_shape);
    let shape_mode_epsilon = builder.and(shape_and_mode, epsilon_exact);
    let all_buffers_valid = builder.and(required_buffers, mode_buffers_valid);
    let profile_valid = builder.and(shape_mode_epsilon, all_buffers_valid);
    let invalid = builder.compare(ComparePredicateV2::IntegerEqual, profile_valid, zero_i1);
    let trap_block = builder.block();
    let dispatch_block = builder.block();
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: invalid,
        then_block: trap_block,
        else_block: dispatch_block,
    });

    builder.start(trap_block);
    builder.void(InstructionKindV2::Call {
        target: fe2o3_llvm_handoff::CallTargetV2::Intrinsic(IntrinsicV2::Trap),
        arguments: vec![],
    });
    builder.finish(TerminatorV2::Unreachable);

    builder.start(dispatch_block);
    let lane = builder.instruction(
        i32_type,
        InstructionKindV2::Call {
            target: fe2o3_llvm_handoff::CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuWorkitemId(
                AxisV2::X,
            )),
            arguments: vec![],
        },
    );
    let row = builder.instruction(
        i32_type,
        InstructionKindV2::Call {
            target: fe2o3_llvm_handoff::CallTargetV2::Intrinsic(IntrinsicV2::AmdGpuWorkgroupId(
                AxisV2::X,
            )),
            arguments: vec![],
        },
    );
    let lane_zero = builder.compare(ComparePredicateV2::IntegerEqual, lane, zero_i32);
    let row_valid = builder.compare(ComparePredicateV2::UnsignedLessThan, row, values.rows);
    let active = builder.and(lane_zero, row_valid);
    let compute_block = builder.block();
    let return_block = builder.block();
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: active,
        then_block: compute_block,
        else_block: return_block,
    });
    builder.start(return_block);
    builder.finish(TerminatorV2::Return(None));

    builder.start(compute_block);
    let row64 = builder.cast(CastOperationV2::ZeroExtend, row, i64_type);
    let row_base = builder.integer(
        IntegerBinaryOperationV2::Multiply,
        row64,
        width64,
        ScalarTypeV1::I64,
    );
    let reduction_initial = builder.current_id;
    let reduction_header = builder.block();
    let reduction_body = builder.block();
    let reduction_backedge = builder.block();
    let reduction_complete = builder.block();
    let next_reduction_index = builder.reserve();
    let next_sum = builder.reserve();
    builder.finish(TerminatorV2::Branch(reduction_header));

    builder.start(reduction_header);
    let reduction_index = builder.instruction(
        i64_type,
        InstructionKindV2::Phi {
            incoming: vec![
                (zero_i64, reduction_initial),
                (next_reduction_index, reduction_backedge),
            ],
        },
    );
    let sum = builder.instruction(
        f32_type,
        InstructionKindV2::Phi {
            incoming: vec![
                (zero_f32, reduction_initial),
                (next_sum, reduction_backedge),
            ],
        },
    );
    let reduction_active = builder.compare(
        ComparePredicateV2::UnsignedLessThan,
        reduction_index,
        width64,
    );
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: reduction_active,
        then_block: reduction_body,
        else_block: reduction_complete,
    });

    builder.start(reduction_body);
    let element_index = builder.integer(
        IntegerBinaryOperationV2::Add,
        row_base,
        reduction_index,
        ScalarTypeV1::I64,
    );
    let input_bf16 = builder.bf16_load(values.input, element_index);
    let input_f32 = builder.cast(CastOperationV2::FloatExtend, input_bf16, f32_type);
    let reduction_fused = builder.block();
    let reduction_pure = builder.block();
    let reduction_join = builder.block();
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: fused_mode,
        then_block: reduction_fused,
        else_block: reduction_pure,
    });
    builder.start(reduction_fused);
    let residual_bf16 = builder.bf16_load(values.residual, element_index);
    let residual_f32 = builder.cast(CastOperationV2::FloatExtend, residual_bf16, f32_type);
    let fused_input = builder.float(FloatBinaryOperationV2::Add, input_f32, residual_f32);
    builder.finish(TerminatorV2::Branch(reduction_join));
    builder.start(reduction_pure);
    builder.finish(TerminatorV2::Branch(reduction_join));
    builder.start(reduction_join);
    let normalized_input = builder.instruction(
        f32_type,
        InstructionKindV2::Phi {
            incoming: vec![(fused_input, reduction_fused), (input_f32, reduction_pure)],
        },
    );
    let square = builder.float(
        FloatBinaryOperationV2::Multiply,
        normalized_input,
        normalized_input,
    );
    builder.instruction_with(
        next_sum,
        f32_type,
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Add),
            left: sum,
            right: square,
        },
    );
    builder.instruction_with(
        next_reduction_index,
        i64_type,
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
            left: reduction_index,
            right: one_i64,
        },
    );
    builder.finish(TerminatorV2::Branch(reduction_backedge));
    builder.start(reduction_backedge);
    builder.finish(TerminatorV2::Branch(reduction_header));

    builder.start(reduction_complete);
    let width_f32 = builder.cast(CastOperationV2::UnsignedIntToFloat, values.width, f32_type);
    let mean_square = builder.float(FloatBinaryOperationV2::Divide, sum, width_f32);
    let stabilized = builder.float(FloatBinaryOperationV2::Add, mean_square, values.epsilon);
    let denominator = builder.instruction(
        f32_type,
        InstructionKindV2::Call {
            target: fe2o3_llvm_handoff::CallTargetV2::Intrinsic(IntrinsicV2::SqrtF32),
            arguments: vec![stabilized],
        },
    );
    let inverse_rms = builder.float(FloatBinaryOperationV2::Divide, one_f32, denominator);
    let write_initial = builder.current_id;
    let write_header = builder.block();
    let write_body = builder.block();
    let write_backedge = builder.block();
    let write_complete = builder.block();
    let next_write_index = builder.reserve();
    builder.finish(TerminatorV2::Branch(write_header));

    builder.start(write_header);
    let write_index = builder.instruction(
        i64_type,
        InstructionKindV2::Phi {
            incoming: vec![
                (zero_i64, write_initial),
                (next_write_index, write_backedge),
            ],
        },
    );
    let write_active = builder.compare(ComparePredicateV2::UnsignedLessThan, write_index, width64);
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: write_active,
        then_block: write_body,
        else_block: write_complete,
    });

    builder.start(write_body);
    let output_index = builder.integer(
        IntegerBinaryOperationV2::Add,
        row_base,
        write_index,
        ScalarTypeV1::I64,
    );
    let input_bf16 = builder.bf16_load(values.input, output_index);
    let weight_bf16 = builder.bf16_load(values.weight, write_index);
    let input_f32 = builder.cast(CastOperationV2::FloatExtend, input_bf16, f32_type);
    let weight_f32 = builder.cast(CastOperationV2::FloatExtend, weight_bf16, f32_type);
    let write_fused = builder.block();
    let write_pure = builder.block();
    let write_join = builder.block();
    builder.finish(TerminatorV2::ConditionalBranch {
        condition: fused_mode,
        then_block: write_fused,
        else_block: write_pure,
    });
    builder.start(write_fused);
    let residual_bf16 = builder.bf16_load(values.residual, output_index);
    let residual_f32 = builder.cast(CastOperationV2::FloatExtend, residual_bf16, f32_type);
    let fused_input = builder.float(FloatBinaryOperationV2::Add, input_f32, residual_f32);
    let fused_bf16 = builder.cast(CastOperationV2::FloatTruncate, fused_input, bf16_type);
    builder.bf16_store(values.fused_output, output_index, fused_bf16);
    builder.finish(TerminatorV2::Branch(write_join));
    builder.start(write_pure);
    builder.finish(TerminatorV2::Branch(write_join));
    builder.start(write_join);
    let normalized_input = builder.instruction(
        f32_type,
        InstructionKindV2::Phi {
            incoming: vec![(fused_input, write_fused), (input_f32, write_pure)],
        },
    );
    let normalized = builder.float(
        FloatBinaryOperationV2::Multiply,
        normalized_input,
        inverse_rms,
    );
    let weighted = builder.float(FloatBinaryOperationV2::Multiply, normalized, weight_f32);
    let weighted_bf16 = builder.cast(CastOperationV2::FloatTruncate, weighted, bf16_type);
    builder.bf16_store(values.normalized_output, output_index, weighted_bf16);
    builder.instruction_with(
        next_write_index,
        i64_type,
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
            left: write_index,
            right: one_i64,
        },
    );
    builder.finish(TerminatorV2::Branch(write_backedge));
    builder.start(write_backedge);
    builder.finish(TerminatorV2::Branch(write_header));
    builder.start(write_complete);
    builder.finish(TerminatorV2::Return(None));

    let pointer = ValueTypeV2::Pointer {
        pointee: ScalarTypeV1::Bf16,
        address_space: AddressSpaceV1::Global,
    };
    let required_readonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(2),
    ];
    let optional_readonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::ReadOnly,
        ParameterAttributeV1::Align(2),
    ];
    let required_writeonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::NonNull,
        ParameterAttributeV1::WriteOnly,
        ParameterAttributeV1::Align(2),
    ];
    let optional_writeonly = vec![
        ParameterAttributeV1::NoAlias,
        ParameterAttributeV1::NoCapture,
        ParameterAttributeV1::WriteOnly,
        ParameterAttributeV1::Align(2),
    ];
    let parameters = [
        (
            ValueIdV2::new(1),
            pointer,
            "input_bf16",
            required_readonly.clone(),
        ),
        (ValueIdV2::new(2), i64_type, "input_elements", vec![]),
        (
            ValueIdV2::new(3),
            pointer,
            "residual_bf16",
            optional_readonly,
        ),
        (ValueIdV2::new(4), i64_type, "residual_elements", vec![]),
        (ValueIdV2::new(5), pointer, "weight_bf16", required_readonly),
        (ValueIdV2::new(6), i64_type, "weight_elements", vec![]),
        (
            ValueIdV2::new(7),
            pointer,
            "fused_residual_bf16",
            optional_writeonly,
        ),
        (
            ValueIdV2::new(8),
            i64_type,
            "fused_residual_elements",
            vec![],
        ),
        (
            ValueIdV2::new(9),
            pointer,
            "normalized_bf16",
            required_writeonly,
        ),
        (ValueIdV2::new(10), i64_type, "normalized_elements", vec![]),
        (ValueIdV2::new(11), i32_type, "rows", vec![]),
        (ValueIdV2::new(12), i32_type, "width", vec![]),
        (ValueIdV2::new(13), f32_type, "epsilon", vec![]),
        (ValueIdV2::new(14), i32_type, "behavior", vec![]),
    ]
    .into_iter()
    .map(|(id, value_type, name, attributes)| {
        FunctionParameterV2::new(TypedValueV2::new(id, value_type), name, attributes)
    })
    .collect::<Result<Vec<_>, _>>()
    .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV2)?;
    let mut executable_attributes = attributes
        .into_iter()
        .map(FunctionAttributeV2::from)
        .collect::<Vec<_>>();
    executable_attributes.push(FunctionAttributeV2::RequiredWorkgroupSize([64, 1, 1]));
    FunctionV2::new(
        FunctionIdV2::new(0),
        QWEN3_RMSNORM_KERNEL_SYMBOL_V1,
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        parameters,
        executable_attributes,
        BlockIdV2::new(0),
        builder.blocks,
        evidence,
    )
    .map_err(PrepareQwen3RmsNormKernelErrorV1::HandoffV2)
}

/// Linear exact typed compiler handoff awaiting attempt-scoped Worker V2 execution.
pub struct InertQwen3RmsNormWorkerRequestV1 {
    prepared: PreparedQwen3RmsNormKernelV1,
}

impl fmt::Debug for InertQwen3RmsNormWorkerRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertQwen3RmsNormWorkerRequestV1")
            .field("catalog", &self.prepared.catalog.identity)
            .field("source", &self.prepared.source_identity)
            .field("handoff", &self.prepared.compiler_handoff_identity)
            .finish_non_exhaustive()
    }
}

impl InertQwen3RmsNormWorkerRequestV1 {
    /// Complete profile catalog retained by this request.
    pub const fn catalog(&self) -> &Qwen3RmsNormProfileCatalogV1 {
        &self.prepared.catalog
    }

    /// Exact compiler handoff for attempt-scoped transaction publication.
    pub const fn compiler_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.prepared.compiler_handoff
    }

    /// Typed graph identity retained by the compiler handoff.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.prepared.source_identity
    }

    /// A request value does not establish Worker V2 execution or artifact existence.
    pub const fn authenticates_worker_execution(&self) -> bool {
        false
    }

    /// A compiler handoff grants no artifact, load, or launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Consumes a prepared owner into the exact Worker V2 request stage.
pub const fn lower_qwen3_rmsnorm_kernel_v1(
    prepared: PreparedQwen3RmsNormKernelV1,
) -> InertQwen3RmsNormWorkerRequestV1 {
    InertQwen3RmsNormWorkerRequestV1 { prepared }
}

/// Failure while executing the exact Handoff V2 module through Worker V2.
#[derive(Debug)]
pub enum ExecuteQwen3RmsNormWorkerErrorV1 {
    /// Consumed attempt bytes differ from the exact prepared handoff.
    HandoffSubstitution,
    /// A fixed link option could not be represented.
    FixedLinkOption,
    /// The fixed HSACO output ceiling could not be represented.
    OutputConstraint(WorkerProtocolError),
    /// Reproducible bootstrap and exact replay failed.
    FirstBuild(FirstBuildWorkerV2Error),
}

impl fmt::Display for ExecuteQwen3RmsNormWorkerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Qwen3 RMSNorm Worker V2 execution failed: {self:?}"
        )
    }
}

impl std::error::Error for ExecuteQwen3RmsNormWorkerErrorV1 {}

/// Linear exact Worker V2 bootstrap/replay evidence awaiting structural inspection.
pub struct InertQwen3RmsNormWorkerEvidenceV1 {
    prepared: PreparedQwen3RmsNormKernelV1,
    transaction_handoff: CompilerModuleHandoffIdentityV1,
    worker: InertFirstBuildWorkerV2EvidenceV1,
}

impl fmt::Debug for InertQwen3RmsNormWorkerEvidenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertQwen3RmsNormWorkerEvidenceV1")
            .field("source", &self.prepared.source_identity)
            .field("transaction_handoff", &self.transaction_handoff)
            .field("worker", &self.worker.identity())
            .finish_non_exhaustive()
    }
}

impl InertQwen3RmsNormWorkerEvidenceV1 {
    /// Reproducible execution remains inert until exact structural inspection.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    /// Worker execution alone does not prove numerical or hardware behavior.
    pub const fn proves_numerical_contract(&self) -> bool {
        false
    }
}

/// Executes the exact transaction handoff through Worker V2 bootstrap and replay.
pub fn execute_qwen3_rmsnorm_worker_v2_v1(
    request: InertQwen3RmsNormWorkerRequestV1,
    consumed: ConsumedCompilerModuleHandoffV1,
    worker: &PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertQwen3RmsNormWorkerEvidenceV1, ExecuteQwen3RmsNormWorkerErrorV1> {
    let InertQwen3RmsNormWorkerRequestV1 { prepared } = request;
    if consumed.bytes() != prepared.compiler_handoff.canonical_bytes() {
        return Err(ExecuteQwen3RmsNormWorkerErrorV1::HandoffSubstitution);
    }
    let transaction_handoff = consumed.identity();
    let worker_evidence = execute_reproducible_first_build_worker_v2(
        consumed,
        worker,
        Vec::new(),
        fixed_link_options()?,
        WorkerOutputConstraintsV1::new(MAX_HSACO_BYTES as u64)
            .map_err(ExecuteQwen3RmsNormWorkerErrorV1::OutputConstraint)?,
        limits,
    )
    .map_err(ExecuteQwen3RmsNormWorkerErrorV1::FirstBuild)?;
    Ok(InertQwen3RmsNormWorkerEvidenceV1 {
        prepared,
        transaction_handoff,
        worker: worker_evidence,
    })
}

/// Exact post-worker structural rejection.
#[derive(Debug)]
pub enum InspectQwen3RmsNormKernelErrorV1 {
    /// Worker request or response canonical bytes failed decoding.
    Protocol(WorkerProtocolError),
    /// Transaction, compiler module, manifest, worker, or output lineage drifted.
    SourceLineage,
    /// AMDHSA metadata or descriptor binding failed.
    Hsaco(KernelBindingError),
    /// Kernel inventory, ABI, or resource facts differ from the exact profile.
    KernelProfile,
    /// Strict allocation-free COV6 loader validation failed.
    Loader(PlanError),
}

impl fmt::Display for InspectQwen3RmsNormKernelErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Qwen3 RMSNorm structural inspection failed: {self:?}"
        )
    }
}

impl std::error::Error for InspectQwen3RmsNormKernelErrorV1 {}

/// Linear exact Worker V2 output after strict ABI/resource and loader inspection.
pub struct InspectedQwen3RmsNormKernelV1 {
    catalog: Qwen3RmsNormProfileCatalogV1,
    source_identity: HandoffIdentityV2,
    compiler_handoff_identity: CompilerModuleHandoffIdentityV2,
    transaction_handoff: CompilerModuleHandoffIdentityV1,
    loader_plan: LoadPlan,
    worker: InertFirstBuildWorkerV2EvidenceV1,
}

impl fmt::Debug for InspectedQwen3RmsNormKernelV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InspectedQwen3RmsNormKernelV1")
            .field("catalog", &self.catalog.identity)
            .field("source", &self.source_identity)
            .field("compiler_handoff", &self.compiler_handoff_identity)
            .field("transaction_handoff", &self.transaction_handoff)
            .field("worker", &self.worker.identity())
            .finish_non_exhaustive()
    }
}

impl InspectedQwen3RmsNormKernelV1 {
    /// Exact profile catalog retained with the inspected output owner.
    pub const fn catalog(&self) -> &Qwen3RmsNormProfileCatalogV1 {
        &self.catalog
    }

    /// Exact strict pure-Rust loader plan over the same Worker output bytes.
    pub const fn loader_plan(&self) -> &LoadPlan {
        &self.loader_plan
    }

    /// Exact bytes retained by sealed Worker V2 evidence.
    pub fn exact_worker_output_bytes(&self) -> &[u8] {
        self.worker.output_bytes()
    }

    /// Observed output bytes are not an independently approved deployment pin.
    pub const fn has_independent_deployment_pin(&self) -> bool {
        false
    }

    /// Structural inspection does not prove typed-source or machine refinement.
    pub const fn proves_machine_refinement(&self) -> bool {
        false
    }

    /// Structural inspection does not prove RMSNorm numerical behavior.
    pub const fn proves_numerical_contract(&self) -> bool {
        false
    }

    /// Structural inspection does not prove hardware execution.
    pub const fn proves_hardware_execution(&self) -> bool {
        false
    }

    /// Structural inspection grants no load or launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Binds one exact operation profile to its mode-specific numerical buffers.
    pub fn bind_checked_profile(
        &self,
        bucket: Qwen3RmsNormBucketV1,
        operation: Qwen3RmsNormOperationV1,
        addresses: [u64; 5],
        byte_lengths: [u64; 5],
    ) -> Result<CheckedQwen3RmsNormLaunchV1, BindQwen3RmsNormLaunchErrorV1> {
        let profile = self
            .catalog
            .profile(bucket, operation)
            .ok_or(BindQwen3RmsNormLaunchErrorV1::Profile)?;
        let buffers = Qwen3RmsNormBufferContractV1::checked(profile, addresses, byte_lengths)
            .map_err(BindQwen3RmsNormLaunchErrorV1::Buffers)?;
        Ok(CheckedQwen3RmsNormLaunchV1 { profile, buffers })
    }
}

/// Consumes Worker V2 evidence through exact transcript, HSACO, ABI, and loader checks.
pub fn inspect_qwen3_rmsnorm_kernel_v1(
    evidence: InertQwen3RmsNormWorkerEvidenceV1,
) -> Result<InspectedQwen3RmsNormKernelV1, InspectQwen3RmsNormKernelErrorV1> {
    let InertQwen3RmsNormWorkerEvidenceV1 {
        prepared,
        transaction_handoff,
        worker,
    } = evidence;
    validate_worker_lineage(&prepared, transaction_handoff, &worker)?;
    let bytes = worker.output_bytes();
    if !worker.output_identity().matches(bytes) {
        return Err(InspectQwen3RmsNormKernelErrorV1::SourceLineage);
    }
    let bound = inspect_and_bind_kernel_descriptors(bytes)
        .map_err(InspectQwen3RmsNormKernelErrorV1::Hsaco)?;
    let [kernel] = bound.inspection().kernels() else {
        return Err(InspectQwen3RmsNormKernelErrorV1::KernelProfile);
    };
    let [binding] = bound.bindings() else {
        return Err(InspectQwen3RmsNormKernelErrorV1::KernelProfile);
    };
    if bound.inspection().code_object_version() != InspectedCodeObjectVersion::V6
        || bound.inspection().target().to_string() != QWEN3_RMSNORM_TARGET_V1
        || bound.inspection().has_printf_metadata()
        || kernel.name() != QWEN3_RMSNORM_KERNEL_SYMBOL_V1
        || kernel.symbol() != QWEN3_RMSNORM_KERNEL_DESCRIPTOR_SYMBOL_V1
        || kernel.kernarg_segment_size() != QWEN3_RMSNORM_TOTAL_KERNARG_BYTES_V1
        || kernel.kernarg_segment_alignment() != QWEN3_RMSNORM_KERNARG_ALIGNMENT_V1
        || kernel.implicit_argument_offset() != Some(QWEN3_RMSNORM_HIDDEN_KERNARG_OFFSET_V1)
        || kernel.implicit_argument_size() != 256
        || kernel.required_workgroup_size() != Some(QWEN3_RMSNORM_WORKGROUP_V1)
        || kernel.max_flat_workgroup_size() != QWEN3_RMSNORM_WORKGROUP_V1[0]
        || kernel.wavefront_size() != 64
        || kernel.group_segment_fixed_size() != 0
        || kernel.private_segment_fixed_size() != 0
        || kernel.sgpr_spill_count().unwrap_or(0) != 0
        || kernel.vgpr_spill_count().unwrap_or(0) != 0
        || kernel.uses_dynamic_stack()
        || binding.kernel_index() != 0
        || binding.descriptor().group_segment_fixed_size() != 0
        || binding.descriptor().private_segment_fixed_size() != 0
        || binding.descriptor().wavefront_size() != 64
        || binding.descriptor().uses_dynamic_stack()
        || !exact_explicit_arguments(kernel.explicit_arguments())
        || !exact_hidden_arguments(kernel.hidden_arguments())
    {
        return Err(InspectQwen3RmsNormKernelErrorV1::KernelProfile);
    }
    let loader = fe2o3_amdhsa_loader::validate(bytes, AdmittedProfile::Gfx942XnackOffCov6)
        .map_err(InspectQwen3RmsNormKernelErrorV1::Loader)?;
    let loader_plan = *loader.plan();
    Ok(InspectedQwen3RmsNormKernelV1 {
        catalog: prepared.catalog,
        source_identity: prepared.source_identity,
        compiler_handoff_identity: prepared.compiler_handoff_identity,
        transaction_handoff,
        loader_plan,
        worker,
    })
}

fn validate_worker_lineage(
    prepared: &PreparedQwen3RmsNormKernelV1,
    transaction_handoff: CompilerModuleHandoffIdentityV1,
    worker: &InertFirstBuildWorkerV2EvidenceV1,
) -> Result<(), InspectQwen3RmsNormKernelErrorV1> {
    let expected_transaction = CompilerModuleHandoffIdentityV1::from_bytes(
        Sha256::digest(prepared.compiler_handoff.canonical_bytes()).into(),
    );
    if transaction_handoff != expected_transaction
        || worker.handoff_identity() != expected_transaction
        || worker.compiler_envelope() != prepared.compiler_handoff.envelope()
        || worker.symbol_manifest() != prepared.compiler_handoff.symbol_manifest()
        || worker.worker_measurement().llvm_build_identity()
            != fe2o3_llvm_worker_handoff::EXACT_LLVM_BUILD_IDENTITY_V1
    {
        return Err(InspectQwen3RmsNormKernelErrorV1::SourceLineage);
    }
    let bootstrap = InertDecodedWorkerExchangeV2::decode(
        worker.bootstrap_request_bytes(),
        worker.bootstrap().response().canonical_bytes(),
    )
    .map_err(InspectQwen3RmsNormKernelErrorV1::Protocol)?;
    let replay = InertDecodedWorkerExchangeV2::decode(
        worker.authorized_request_bytes(),
        worker.authorized().response().canonical_bytes(),
    )
    .map_err(InspectQwen3RmsNormKernelErrorV1::Protocol)?;
    for exchange in [&bootstrap, &replay] {
        let request = exchange.request();
        if request.target() != exact_target()
            || request.code_object_version() != CodeObjectVersion::V6
            || request.compiler_module().bytes() != prepared.compiler_handoff.module_bytes()
            || !request.external_providers().is_empty()
            || !request.import_symbols().is_empty()
            || !request.export_symbols().is_empty()
            || !request.final_symbols().iter().map(String::as_str).eq([
                QWEN3_RMSNORM_KERNEL_SYMBOL_V1,
                QWEN3_RMSNORM_KERNEL_DESCRIPTOR_SYMBOL_V1,
            ])
            || exchange.response().request_identity() != request.identity()
        {
            return Err(InspectQwen3RmsNormKernelErrorV1::SourceLineage);
        }
    }
    Ok(())
}

/// Failure while binding an inspected output to one finite runtime profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindQwen3RmsNormLaunchErrorV1 {
    /// The requested role/bucket/operation tuple is absent from the finite catalog.
    Profile,
    /// Numerical buffer address or extent validation failed.
    Buffers(Qwen3RmsNormBufferContractErrorV1),
}

impl fmt::Display for BindQwen3RmsNormLaunchErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 RMSNorm launch binding failed: {self:?}")
    }
}

impl std::error::Error for BindQwen3RmsNormLaunchErrorV1 {}

/// Inert exact profile and numerical-buffer binding for a future protected launcher.
#[derive(Debug)]
pub struct CheckedQwen3RmsNormLaunchV1 {
    profile: Qwen3RmsNormProfileV1,
    buffers: Qwen3RmsNormBufferContractV1,
}

impl CheckedQwen3RmsNormLaunchV1 {
    /// Exact finite profile.
    pub const fn profile(&self) -> Qwen3RmsNormProfileV1 {
        self.profile
    }

    /// Exact checked numerical buffer ranges.
    pub const fn buffers(&self) -> Qwen3RmsNormBufferContractV1 {
        self.buffers
    }

    /// This binding grants no allocation, load, or launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn exact_explicit_arguments(arguments: &[ExplicitArgument]) -> bool {
    if arguments.len() != 14 {
        return false;
    }
    let pointers = [
        (0, "input_bf16", ArgumentAccess::ReadOnly),
        (2, "residual_bf16", ArgumentAccess::ReadOnly),
        (4, "weight_bf16", ArgumentAccess::ReadOnly),
        (6, "fused_residual_bf16", ArgumentAccess::WriteOnly),
        (8, "normalized_bf16", ArgumentAccess::WriteOnly),
    ];
    for (index, name, access) in pointers {
        let argument = &arguments[index];
        if argument.name() != Some(name)
            || argument.offset() != (index as u64 / 2) * 16
            || argument.size() != 8
            || argument.alignment().is_some_and(|alignment| alignment != 8)
            || argument
                .pointee_alignment()
                .is_some_and(|alignment| alignment != 2)
            || argument.value_kind() != ExplicitValueKind::GlobalBuffer
            || !argument.value_type().is_none_or(is_bf16_metadata_carrier)
            || argument.address_space() != Some(ArgumentAddressSpace::Global)
            || argument.access() != Some(access)
        {
            return false;
        }
    }
    let lengths = [
        (1, "input_elements"),
        (3, "residual_elements"),
        (5, "weight_elements"),
        (7, "fused_residual_elements"),
        (9, "normalized_elements"),
    ];
    for (index, name) in lengths {
        let argument = &arguments[index];
        if argument.name() != Some(name)
            || argument.offset() != ((index - 1) as u64 / 2) * 16 + 8
            || argument.size() != 8
            || argument.value_kind() != ExplicitValueKind::ByValue
            || argument
                .value_type()
                .is_some_and(|value_type| value_type != ExplicitValueType::U64)
            || argument.address_space().is_some()
            || argument.access().is_some()
        {
            return false;
        }
    }
    exact_scalar_argument(&arguments[10], "rows", 80, ExplicitValueType::U32)
        && exact_scalar_argument(&arguments[11], "width", 84, ExplicitValueType::U32)
        && exact_scalar_argument(&arguments[12], "epsilon", 88, ExplicitValueType::F32)
        && exact_scalar_argument(&arguments[13], "behavior", 92, ExplicitValueType::U32)
}

fn exact_scalar_argument(
    argument: &ExplicitArgument,
    name: &str,
    offset: u64,
    value_type: ExplicitValueType,
) -> bool {
    argument.name() == Some(name)
        && argument.offset() == offset
        && argument.size() == 4
        && argument.value_kind() == ExplicitValueKind::ByValue
        && argument
            .value_type()
            .is_none_or(|actual| actual == value_type)
        && argument.address_space().is_none()
        && argument.access().is_none()
}

const fn is_bf16_metadata_carrier(value_type: ExplicitValueType) -> bool {
    matches!(
        value_type,
        ExplicitValueType::I16 | ExplicitValueType::U16 | ExplicitValueType::F16
    )
}

fn exact_hidden_arguments(arguments: &[HiddenArgument]) -> bool {
    const RELATIVE: [(u64, u64, HiddenValueKind); 19] = [
        (0, 4, HiddenValueKind::BlockCountX),
        (4, 4, HiddenValueKind::BlockCountY),
        (8, 4, HiddenValueKind::BlockCountZ),
        (12, 2, HiddenValueKind::GroupSizeX),
        (14, 2, HiddenValueKind::GroupSizeY),
        (16, 2, HiddenValueKind::GroupSizeZ),
        (18, 2, HiddenValueKind::RemainderX),
        (20, 2, HiddenValueKind::RemainderY),
        (22, 2, HiddenValueKind::RemainderZ),
        (40, 8, HiddenValueKind::GlobalOffsetX),
        (48, 8, HiddenValueKind::GlobalOffsetY),
        (56, 8, HiddenValueKind::GlobalOffsetZ),
        (64, 2, HiddenValueKind::GridDimensions),
        (80, 8, HiddenValueKind::HostcallBuffer),
        (88, 8, HiddenValueKind::MultigridSyncArgument),
        (96, 8, HiddenValueKind::HeapV1),
        (104, 8, HiddenValueKind::DefaultQueue),
        (112, 8, HiddenValueKind::CompletionAction),
        (200, 8, HiddenValueKind::QueuePointer),
    ];
    arguments.len() == RELATIVE.len()
        && arguments.iter().zip(RELATIVE).all(|(actual, expected)| {
            actual.offset() == QWEN3_RMSNORM_HIDDEN_KERNARG_OFFSET_V1 + expected.0
                && actual.size() == expected.1
                && actual.value_kind() == expected.2
        })
}

fn fixed_link_options() -> Result<Vec<LinkOptionV1>, ExecuteQwen3RmsNormWorkerErrorV1> {
    [
        ("code-object-version", "6"),
        ("opt-level", "2"),
        ("strip-debug", "true"),
        ("verify-each", "true"),
    ]
    .into_iter()
    .map(|(name, value)| {
        LinkOptionV1::new(name, value)
            .map_err(|_| ExecuteQwen3RmsNormWorkerErrorV1::FixedLinkOption)
    })
    .collect()
}

fn bound_stage_identity(
    domain: &[u8],
    caller_label: [u8; 32],
    catalog: Qwen3RmsNormProfileCatalogIdentityV1,
) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(&caller_label);
    bytes.extend_from_slice(catalog.as_bytes());
    hash(domain, &bytes)
}

fn exact_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(QWEN3_RMSNORM_TARGET_V1)
        .expect("the fixed Qwen3 RMSNorm target is canonical")
}

fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn bindings(seed: u8) -> Qwen3RmsNormSourceBindingsV1 {
        Qwen3RmsNormSourceBindingsV1::new(
            [seed; 32],
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
        )
    }

    #[test]
    fn catalog_is_exact_complete_and_deterministic() {
        let first = Qwen3RmsNormProfileCatalogV1::canonical().unwrap();
        let second = Qwen3RmsNormProfileCatalogV1::canonical().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.profiles().len(), QWEN3_RMSNORM_PROFILE_COUNT_V1);
        let identities = first
            .profiles()
            .iter()
            .map(|profile| *profile.identity().as_bytes())
            .collect::<BTreeSet<_>>();
        assert_eq!(identities.len(), QWEN3_RMSNORM_PROFILE_COUNT_V1);
        assert_ne!(first.identity().as_bytes(), &[0; 32]);
        assert!(!first.grants_authority());
    }

    #[test]
    fn all_graph_operations_match_exact_qwen3_rows_and_widths() {
        let catalog = Qwen3RmsNormProfileCatalogV1::canonical().unwrap();
        for role in QWEN3_RMSNORM_ROLES_V1 {
            for kind in QWEN3_RMSNORM_BUCKET_KINDS_V1 {
                let bucket = Qwen3RmsNormBucketV1::new(role, kind);
                let base_rows = bucket.flattened_rows();
                for operation in QWEN3_RMSNORM_OPERATIONS_V1 {
                    let profile = catalog.profile(bucket, operation).unwrap();
                    let [expected_rows, expected_width] = match operation {
                        Qwen3RmsNormOperationV1::QueryRmsNorm => {
                            [base_rows * role.query_heads(), 128]
                        }
                        Qwen3RmsNormOperationV1::KeyRmsNorm => {
                            [base_rows * role.key_value_heads(), 128]
                        }
                        _ => [base_rows, role.hidden_size()],
                    };
                    assert_eq!(profile.rows(), expected_rows);
                    assert_eq!(profile.width(), expected_width);
                    assert_eq!(profile.operation(), operation);
                    assert_eq!(profile.behavior(), operation.behavior());
                }
            }
        }
        assert_eq!(Qwen3RmsNormModelRoleV1::Target8B.query_heads(), 32);
        assert_eq!(Qwen3RmsNormModelRoleV1::Draft06B.query_heads(), 16);
        assert_eq!(Qwen3RmsNormModelRoleV1::Target8B.key_value_heads(), 8);
        assert_eq!(Qwen3RmsNormModelRoleV1::Draft06B.key_value_heads(), 8);
        let max_rows = |role, operation| {
            catalog
                .profiles()
                .iter()
                .filter(|profile| {
                    profile.bucket().role() == role && profile.operation() == operation
                })
                .map(|profile| profile.rows())
                .max()
                .unwrap()
        };
        assert_eq!(
            max_rows(
                Qwen3RmsNormModelRoleV1::Target8B,
                Qwen3RmsNormOperationV1::QueryRmsNorm
            ),
            65_536
        );
        assert_eq!(
            max_rows(
                Qwen3RmsNormModelRoleV1::Draft06B,
                Qwen3RmsNormOperationV1::QueryRmsNorm
            ),
            32_768
        );
        for role in QWEN3_RMSNORM_ROLES_V1 {
            assert_eq!(max_rows(role, Qwen3RmsNormOperationV1::KeyRmsNorm), 16_384);
            assert_eq!(max_rows(role, Qwen3RmsNormOperationV1::InputRmsNorm), 2_048);
        }
    }

    #[test]
    fn profile_geometry_and_fp32_contract_are_exact() {
        let catalog = Qwen3RmsNormProfileCatalogV1::canonical().unwrap();
        for profile in catalog.profiles() {
            assert_eq!(
                profile.hsa_adapter_block_counts()[0].checked_mul(64),
                Some(profile.aql_grid_work_items()[0])
            );
            assert_eq!(profile.epsilon_bits(), 1.0e-6_f32.to_bits());
            assert_eq!(
                profile.row_elements(),
                u64::from(profile.rows()) * u64::from(profile.width())
            );
            assert!(!profile.grants_launch_authority());
        }
    }

    #[test]
    fn behavior_width_compatibility_rejects_fused_head_width() {
        assert!(behavior_accepts_width(Qwen3RmsNormBehaviorV1::Pure, 128));
        assert!(behavior_accepts_width(Qwen3RmsNormBehaviorV1::Pure, 1_024));
        assert!(behavior_accepts_width(
            Qwen3RmsNormBehaviorV1::ResidualFused,
            1_024
        ));
        assert!(!behavior_accepts_width(
            Qwen3RmsNormBehaviorV1::ResidualFused,
            128
        ));
        assert!(!behavior_accepts_width(Qwen3RmsNormBehaviorV1::Pure, 256));
    }

    #[test]
    fn buffer_contract_rejects_short_aliasing_unaligned_and_overflow() {
        let catalog = Qwen3RmsNormProfileCatalogV1::canonical().unwrap();
        let bucket = Qwen3RmsNormBucketV1::new(
            Qwen3RmsNormModelRoleV1::Target8B,
            Qwen3RmsNormBucketKindV1::PrefillS1T128,
        );
        let profile = catalog
            .profile(bucket, Qwen3RmsNormOperationV1::ResidualFusedHidden)
            .unwrap();
        let row_bytes = profile.row_elements() * 2;
        let weight_bytes = profile.weight_elements() * 2;
        let lengths = [row_bytes, row_bytes, weight_bytes, row_bytes, row_bytes];
        let addresses = [0x1_0000, 0x1000_0000, 0x2000_0000, 0x3000_0000, 0x4000_0000];
        let checked = Qwen3RmsNormBufferContractV1::checked(profile, addresses, lengths).unwrap();
        assert_eq!(checked.byte_lengths(), lengths);
        assert!(!checked.authenticates_device_memory());
        let mut short = lengths;
        short[2] -= 2;
        assert_eq!(
            Qwen3RmsNormBufferContractV1::checked(profile, addresses, short),
            Err(Qwen3RmsNormBufferContractErrorV1::ByteLength(
                Qwen3RmsNormBufferV1::Weight
            ))
        );
        let mut aliased = addresses;
        aliased[4] = addresses[3];
        assert_eq!(
            Qwen3RmsNormBufferContractV1::checked(profile, aliased, lengths),
            Err(Qwen3RmsNormBufferContractErrorV1::Aliasing)
        );
        let mut unaligned = addresses;
        unaligned[0] += 1;
        assert_eq!(
            Qwen3RmsNormBufferContractV1::checked(profile, unaligned, lengths),
            Err(Qwen3RmsNormBufferContractErrorV1::Alignment(
                Qwen3RmsNormBufferV1::Input
            ))
        );
        let mut overflow = addresses;
        overflow[0] = u64::MAX - 1;
        assert_eq!(
            Qwen3RmsNormBufferContractV1::checked(profile, overflow, lengths),
            Err(Qwen3RmsNormBufferContractErrorV1::RangeOverflow(
                Qwen3RmsNormBufferV1::Input
            ))
        );
    }

    #[test]
    fn pure_and_fused_buffer_modes_reject_cross_variant_bindings() {
        let catalog = Qwen3RmsNormProfileCatalogV1::canonical().unwrap();
        let bucket = Qwen3RmsNormBucketV1::new(
            Qwen3RmsNormModelRoleV1::Draft06B,
            Qwen3RmsNormBucketKindV1::DecodeS8C8192,
        );
        let pure = catalog
            .profile(bucket, Qwen3RmsNormOperationV1::InputRmsNorm)
            .unwrap();
        let fused = catalog
            .profile(bucket, Qwen3RmsNormOperationV1::ResidualFusedHidden)
            .unwrap();
        let pure_row_bytes = pure.row_elements() * 2;
        let pure_weight_bytes = pure.weight_elements() * 2;
        let pure_addresses = [0x10_0000, 0, 0x20_0000, 0, 0x30_0000];
        let pure_lengths = [pure_row_bytes, 0, pure_weight_bytes, 0, pure_row_bytes];
        let checked =
            Qwen3RmsNormBufferContractV1::checked(pure, pure_addresses, pure_lengths).unwrap();
        assert_eq!(checked.behavior(), Qwen3RmsNormBehaviorV1::Pure);

        let fused_row_bytes = fused.row_elements() * 2;
        let fused_weight_bytes = fused.weight_elements() * 2;
        let fused_addresses = [
            0x1000_0000,
            0x2000_0000,
            0x3000_0000,
            0x4000_0000,
            0x5000_0000,
        ];
        let fused_lengths = [
            fused_row_bytes,
            fused_row_bytes,
            fused_weight_bytes,
            fused_row_bytes,
            fused_row_bytes,
        ];
        let checked =
            Qwen3RmsNormBufferContractV1::checked(fused, fused_addresses, fused_lengths).unwrap();
        assert_eq!(checked.behavior(), Qwen3RmsNormBehaviorV1::ResidualFused);

        assert_eq!(
            Qwen3RmsNormBufferContractV1::checked(pure, fused_addresses, pure_lengths),
            Err(Qwen3RmsNormBufferContractErrorV1::InactiveAddress(
                Qwen3RmsNormBufferV1::Residual
            ))
        );
        assert_eq!(
            Qwen3RmsNormBufferContractV1::checked(fused, pure_addresses, pure_lengths),
            Err(Qwen3RmsNormBufferContractErrorV1::ZeroAddress(
                Qwen3RmsNormBufferV1::Residual
            ))
        );
        let mut nonzero_inactive = pure_addresses;
        nonzero_inactive[1] = 0x40_0000;
        assert_eq!(
            Qwen3RmsNormBufferContractV1::checked(pure, nonzero_inactive, pure_lengths),
            Err(Qwen3RmsNormBufferContractErrorV1::InactiveAddress(
                Qwen3RmsNormBufferV1::Residual
            ))
        );
    }

    #[test]
    fn source_bindings_reject_zero_and_every_repeated_role_pair() {
        assert!(prepare_qwen3_rmsnorm_kernel_v1(bindings(0x31)).is_ok());
        let mut identities = [[0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32]];
        for first in 0..4 {
            for second in first + 1..4 {
                identities[second] = identities[first];
                let rejected = Qwen3RmsNormSourceBindingsV1::new(
                    identities[0],
                    identities[1],
                    identities[2],
                    identities[3],
                );
                assert!(matches!(
                    prepare_qwen3_rmsnorm_kernel_v1(rejected),
                    Err(PrepareQwen3RmsNormKernelErrorV1::SourceBindings)
                ));
                identities = [[0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32]];
            }
        }
        assert!(matches!(
            prepare_qwen3_rmsnorm_kernel_v1(Qwen3RmsNormSourceBindingsV1::new(
                [0; 32], [2; 32], [3; 32], [4; 32]
            )),
            Err(PrepareQwen3RmsNormKernelErrorV1::SourceBindings)
        ));
    }

    #[test]
    fn typed_bf16_graph_is_deterministic_and_structurally_exact() {
        let first = prepare_qwen3_rmsnorm_kernel_v1(bindings(0x61)).unwrap();
        let second = prepare_qwen3_rmsnorm_kernel_v1(bindings(0x61)).unwrap();
        assert_eq!(first.source_identity(), second.source_identity());
        assert_eq!(first.assembly_sha256(), second.assembly_sha256());
        assert_eq!(first.assembly_len(), second.assembly_len());
        assert_eq!(
            first.compiler_handoff_identity(),
            second.compiler_handoff_identity()
        );
        let llvm = std::str::from_utf8(first.compiler_handoff().module_bytes()).unwrap();
        for required in [
            "define amdgpu_kernel void @qwen3_rmsnorm_v1",
            "%input_bf16",
            "%residual_bf16",
            "%weight_bf16",
            "%fused_residual_bf16",
            "%normalized_bf16",
            "load bfloat",
            "fpext bfloat",
            "fptrunc float",
            "fmul float",
            "fadd float",
            "fdiv float",
            "@llvm.sqrt.f32",
            "phi i64",
            "phi float",
            "ptrtoint",
            "%behavior",
            "!reqd_work_group_size",
            "!{i32 64, i32 1, i32 1}",
        ] {
            assert!(
                llvm.contains(required),
                "missing LLVM fragment: {required}\n{llvm}"
            );
        }
        let definition = llvm
            .lines()
            .find(|line| line.starts_with("define amdgpu_kernel"))
            .unwrap();
        assert!(
            definition.contains(
                "ptr addrspace(1) noalias captures(none) readonly align 2 %residual_bf16"
            )
        );
        assert!(definition.contains(
            "ptr addrspace(1) noalias captures(none) writeonly align 2 %fused_residual_bf16"
        ));
        assert!(!definition.contains("nonnull readonly align 2 %residual_bf16"));
        assert!(!definition.contains("nonnull writeonly align 2 %fused_residual_bf16"));
        assert!(!definition.contains("dereferenceable"));
        assert!(!first.uses_pliron_lowering());
        assert!(!first.proves_machine_refinement());
        assert!(!first.proves_numerical_contract());
        assert!(!first.authenticates_worker_execution());
        assert!(!first.grants_launch_authority());
    }

    #[test]
    fn source_stage_mutation_rebinds_typed_and_compiler_identities() {
        let baseline = prepare_qwen3_rmsnorm_kernel_v1(bindings(0x71)).unwrap();
        for changed in [
            Qwen3RmsNormSourceBindingsV1::new([0x78; 32], [0x72; 32], [0x73; 32], [0x74; 32]),
            Qwen3RmsNormSourceBindingsV1::new([0x71; 32], [0x75; 32], [0x73; 32], [0x74; 32]),
            Qwen3RmsNormSourceBindingsV1::new([0x71; 32], [0x72; 32], [0x76; 32], [0x74; 32]),
            Qwen3RmsNormSourceBindingsV1::new([0x71; 32], [0x72; 32], [0x73; 32], [0x77; 32]),
        ] {
            let changed = prepare_qwen3_rmsnorm_kernel_v1(changed).unwrap();
            assert_ne!(baseline.source_identity(), changed.source_identity());
            assert_ne!(
                baseline.compiler_handoff_identity(),
                changed.compiler_handoff_identity()
            );
        }
    }

    #[test]
    fn malformed_bytes_never_reach_hsaco_or_loader_profile() {
        assert!(matches!(
            inspect_and_bind_kernel_descriptors(b"not an ELF"),
            Err(KernelBindingError::Inspection(_))
        ));
        assert!(
            fe2o3_amdhsa_loader::validate(b"not an ELF", AdmittedProfile::Gfx942XnackOffCov6)
                .is_err()
        );
    }
}
