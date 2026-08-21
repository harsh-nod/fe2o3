//! Exact finite Qwen3 GEMM/GEMV profiles over the typed general-GEMM compiler.

use core::fmt;

use fe2o3_amdhsa_loader::{AdmittedProfile, LoadPlan, PlanError};
use fe2o3_artifact_transaction::ConsumedCompilerModuleHandoffV1;
use fe2o3_compiler_api::{
    CompileLimitsV1, CompileRequestV1, CompilerProfileIdentityV1, CompilerStageV1,
    KernelInstanceIdentityV1, PipelineSelectorV1, RequestIdentityV1, SnapshotFormatIdentityV1,
    SnapshotIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
};
use fe2o3_general_gemm_compiler::{
    GeneralGemmCheckedLaunchInstantiationErrorV1, GeneralGemmCheckedLaunchInstantiationV1,
    GeneralGemmFrontendSemanticBindingV1, GeneralGemmLoweringLimitsV1,
    GeneralGemmRuntimeAbiSnapshotV1, GeneralGemmScheduleV1, GeneralGemmStructuralMachineErrorV1,
    GeneralGemmSymbolicCompilationUnitV1, GeneralGemmSymbolicKirV1, GeneralGemmSymbolicPlanV1,
    GeneralGemmSymbolicStructuralMachineV1, general_gemm_symbolic_obligation_set_identity_v1,
    general_gemm_symbolic_pipeline_configuration_identity_v1,
    lower_general_gemm_symbolic_structural_machine_v1,
};
use fe2o3_hsaco_finalize::{
    GeneralGemmPostLinkMachineErrorV1, GeneralGemmWorkerV2ErrorV1,
    InertSymbolicGeneralGemmWorkerV2EvidenceV1, OpaqueGeneralGemmPostLinkMachineObservationV1,
    PinnedWorkerV1, WorkerExecutionLimitsV1, execute_symbolic_general_gemm_worker_v2_v1,
    finalize_symbolic_general_gemm_worker_v2_v1,
};
use fe2o3_kernel_ir::{
    GeneralGemmKirV1, GeneralGemmPlanFieldsV1, GeneralGemmPlanSnapshotErrorV1,
    GeneralGemmPlanSnapshotV1,
};
use sha2::{Digest as _, Sha256};

/// Exact device target required by every profile.
pub const QWEN3_GEMM_TARGET_V1: &str = "gfx942:xnack-";
/// Exact code-object version required by every profile.
pub const QWEN3_GEMM_CODE_OBJECT_VERSION_V1: u8 = 6;
/// Exact wave64 workgroup used by both schedules.
pub const QWEN3_GEMM_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Qwen3 vocabulary size pinned by the M1 model envelope.
pub const QWEN3_VOCABULARY_SIZE_V1: u32 = 151_936;
/// Number of target/draft mode-bucket selections in the finite catalog.
pub const QWEN3_GEMM_BUCKET_COUNT_V1: usize = 22;
/// Number of dense projection operations per bucket.
pub const QWEN3_GEMM_OPERATION_COUNT_V1: usize = 8;
/// Total finite profiles retained by the catalog.
pub const QWEN3_GEMM_PROFILE_COUNT_V1: usize =
    QWEN3_GEMM_BUCKET_COUNT_V1 * QWEN3_GEMM_OPERATION_COUNT_V1;

const CATALOG_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/PROFILE-CATALOG/V1\0";
const PROFILE_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/PROFILE/V1\0";
const SNAPSHOT_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/FRONTEND-SNAPSHOT/V1\0";
const SNAPSHOT_FORMAT_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/FRONTEND-FORMAT/V1\0";
const KERNEL_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/KERNEL-INSTANCE/V1\0";
const REQUEST_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/REQUEST/V1\0";
const COMPILER_PROFILE_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/COMPILER-PROFILE/V1\0";
const TARGET_PROFILE_DOMAIN: &[u8] = b"FE2O3/QWEN3/GEMM/TARGET-PROFILE/V1\0";

/// Target or speculative-draft Qwen3 model role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3GemmModelRoleV1 {
    /// Pinned Qwen3-8B target geometry.
    Target8B = 1,
    /// Pinned Qwen3-0.6B draft geometry.
    Draft06B = 2,
}

impl Qwen3GemmModelRoleV1 {
    /// Exact hidden width.
    pub const fn hidden_size(self) -> u32 {
        match self {
            Self::Target8B => 4_096,
            Self::Draft06B => 1_024,
        }
    }

    /// Exact gated-MLP intermediate width.
    pub const fn intermediate_size(self) -> u32 {
        match self {
            Self::Target8B => 12_288,
            Self::Draft06B => 3_072,
        }
    }

    /// Exact query projection width.
    pub const fn query_width(self) -> u32 {
        match self {
            Self::Target8B => 32 * 128,
            Self::Draft06B => 16 * 128,
        }
    }

    /// Exact key/value projection width.
    pub const fn kv_width(self) -> u32 {
        8 * 128
    }
}

/// One of the eleven exact Ferric M1 bucket shapes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3GemmBucketKindV1 {
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

impl Qwen3GemmBucketKindV1 {
    const fn sequence_and_active_tokens(self, role: Qwen3GemmModelRoleV1) -> [u32; 2] {
        match self {
            Self::PrefillS1T128 => [1, 128],
            Self::PrefillS8T128 => [8, 128],
            Self::PrefillS1T512 => [1, 512],
            Self::PrefillS1T2048 => [1, 2_048],
            Self::DecodeS1C8192 => [1, 1],
            Self::DecodeS8C8192 => [8, 1],
            Self::DecodeS32C8192 => [32, 1],
            Self::SpeculativeS1K4C8192 => match role {
                Qwen3GemmModelRoleV1::Target8B => [1, 5],
                Qwen3GemmModelRoleV1::Draft06B => [1, 4],
            },
            Self::SpeculativeS8K4C8192 => match role {
                Qwen3GemmModelRoleV1::Target8B => [8, 5],
                Qwen3GemmModelRoleV1::Draft06B => [8, 4],
            },
            Self::SpeculativeS1K8C8192 => match role {
                Qwen3GemmModelRoleV1::Target8B => [1, 9],
                Qwen3GemmModelRoleV1::Draft06B => [1, 8],
            },
            Self::SpeculativeS1K16C8192 => match role {
                Qwen3GemmModelRoleV1::Target8B => [1, 17],
                Qwen3GemmModelRoleV1::Draft06B => [1, 16],
            },
        }
    }
}

/// One exact role and mode-bucket selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Qwen3GemmBucketV1 {
    role: Qwen3GemmModelRoleV1,
    kind: Qwen3GemmBucketKindV1,
}

impl Qwen3GemmBucketV1 {
    /// Creates one of the finite target/draft selections.
    pub const fn new(role: Qwen3GemmModelRoleV1, kind: Qwen3GemmBucketKindV1) -> Self {
        Self { role, kind }
    }

    /// Exact model role.
    pub const fn role(self) -> Qwen3GemmModelRoleV1 {
        self.role
    }

    /// Exact mode-bucket kind.
    pub const fn kind(self) -> Qwen3GemmBucketKindV1 {
        self.kind
    }

    /// Exact `[sequences, active_tokens]` dimensions.
    pub const fn sequence_and_active_tokens(self) -> [u32; 2] {
        self.kind.sequence_and_active_tokens(self.role)
    }

    /// Flattened dense-projection row count.
    pub const fn flattened_rows(self) -> u32 {
        let dimensions = self.sequence_and_active_tokens();
        dimensions[0] * dimensions[1]
    }
}

/// Dense Qwen3 operations compiled by this lane.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3GemmOperationV1 {
    /// Hidden to all query heads.
    QueryProjection = 1,
    /// Hidden to all key/value heads' keys.
    KeyProjection = 2,
    /// Hidden to all key/value heads' values.
    ValueProjection = 3,
    /// Attention output projection with FP32 residual accumulation.
    AttentionOutputResidual = 4,
    /// Hidden to gated-MLP gate projection.
    GateProjection = 5,
    /// Hidden to gated-MLP up projection.
    UpProjection = 6,
    /// Intermediate to hidden projection with FP32 residual accumulation.
    DownResidual = 7,
    /// Hidden to full-vocabulary FP32 logits.
    LogitsProjection = 8,
}

impl Qwen3GemmOperationV1 {
    const fn dimensions(self, role: Qwen3GemmModelRoleV1, m: u32) -> [u32; 3] {
        let hidden = role.hidden_size();
        match self {
            Self::QueryProjection => [m, role.query_width(), hidden],
            Self::KeyProjection | Self::ValueProjection => [m, role.kv_width(), hidden],
            Self::AttentionOutputResidual => [m, hidden, hidden],
            Self::GateProjection | Self::UpProjection => [m, role.intermediate_size(), hidden],
            Self::DownResidual => [m, hidden, role.intermediate_size()],
            Self::LogitsProjection => [m, QWEN3_VOCABULARY_SIZE_V1, hidden],
        }
    }

    const fn beta_bits(self) -> u32 {
        match self {
            Self::AttentionOutputResidual | Self::DownResidual => 1.0_f32.to_bits(),
            _ => 0.0_f32.to_bits(),
        }
    }
}

/// Whether the selected dynamic shape is the M=1 GEMV case or tiled GEMM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3GemmExecutionClassV1 {
    /// A single flattened input row using the parameterized GEMM body.
    GemvM1,
    /// Two or more flattened rows using the parameterized GEMM body.
    TiledGemm,
}

/// SHA-256 identity of one exact profile record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Qwen3GemmProfileIdentityV1([u8; 32]);

impl Qwen3GemmProfileIdentityV1 {
    /// Returns the domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One finite checked Qwen3 projection profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3GemmProfileV1 {
    bucket: Qwen3GemmBucketV1,
    operation: Qwen3GemmOperationV1,
    schedule: GeneralGemmScheduleV1,
    dimensions: [u32; 3],
    strides: [u32; 3],
    storage_elements: [u64; 3],
    block_counts: [u32; 3],
    aql_grid_work_items: [u32; 3],
    reduction_phases: u32,
    identity: Qwen3GemmProfileIdentityV1,
}

impl Qwen3GemmProfileV1 {
    fn checked(
        bucket: Qwen3GemmBucketV1,
        operation: Qwen3GemmOperationV1,
    ) -> Result<Self, Qwen3GemmCatalogErrorV1> {
        let m = bucket.flattened_rows();
        let dimensions = operation.dimensions(bucket.role, m);
        let [m, n, k] = dimensions;
        let strides = [k, n, n];
        let storage_elements = [
            u64::from(m)
                .checked_mul(u64::from(k))
                .ok_or(Qwen3GemmCatalogErrorV1::ExtentOverflow)?,
            u64::from(k)
                .checked_mul(u64::from(n))
                .ok_or(Qwen3GemmCatalogErrorV1::ExtentOverflow)?,
            u64::from(m)
                .checked_mul(u64::from(n))
                .ok_or(Qwen3GemmCatalogErrorV1::ExtentOverflow)?,
        ];
        let block_x = ceil_div_16(n);
        let block_y = ceil_div_16(m);
        let grid_x = block_x
            .checked_mul(QWEN3_GEMM_WORKGROUP_V1[0])
            .ok_or(Qwen3GemmCatalogErrorV1::GridOverflow)?;
        let block_counts = [block_x, block_y, 1];
        let aql_grid_work_items = [grid_x, block_y, 1];
        let reduction_phases = ceil_div_16(k);
        let schedule = if m < 16 {
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1
        } else {
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
        };
        let mut profile = Self {
            bucket,
            operation,
            schedule,
            dimensions,
            strides,
            storage_elements,
            block_counts,
            aql_grid_work_items,
            reduction_phases,
            identity: Qwen3GemmProfileIdentityV1([0; 32]),
        };
        profile.identity = Qwen3GemmProfileIdentityV1(hash(PROFILE_DOMAIN, &profile.encode()));
        profile.checked_plan()?;
        Ok(profile)
    }

    fn checked_plan(self) -> Result<GeneralGemmPlanFieldsV1, Qwen3GemmCatalogErrorV1> {
        GeneralGemmPlanFieldsV1::checked(self.plan_snapshot())
            .map_err(Qwen3GemmCatalogErrorV1::Plan)
    }

    fn plan_snapshot(self) -> GeneralGemmPlanSnapshotV1 {
        GeneralGemmPlanSnapshotV1 {
            dimensions: self.dimensions,
            strides: self.strides,
            storage_elements: self.storage_elements,
            block_counts: self.block_counts,
            aql_grid_work_items: self.aql_grid_work_items,
            reduction_phases: self.reduction_phases,
            alpha_bits: 1.0_f32.to_bits(),
            beta_bits: self.operation.beta_bits(),
        }
    }

    fn encode(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(96);
        bytes.push(self.bucket.role as u8);
        bytes.push(self.bucket.kind as u8);
        bytes.push(self.operation as u8);
        bytes.push(schedule_tag(self.schedule));
        for value in self.dimensions {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.strides {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.storage_elements {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.block_counts {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.aql_grid_work_items {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&self.reduction_phases.to_le_bytes());
        bytes.extend_from_slice(&1.0_f32.to_bits().to_le_bytes());
        bytes.extend_from_slice(&self.operation.beta_bits().to_le_bytes());
        bytes
    }

    /// Exact role and bucket selection.
    pub const fn bucket(self) -> Qwen3GemmBucketV1 {
        self.bucket
    }

    /// Exact graph operation.
    pub const fn operation(self) -> Qwen3GemmOperationV1 {
        self.operation
    }

    /// Closed compiler schedule selected by the row count.
    pub const fn schedule(self) -> GeneralGemmScheduleV1 {
        self.schedule
    }

    /// Exact `[M, N, K]` dimensions.
    pub const fn dimensions(self) -> [u32; 3] {
        self.dimensions
    }

    /// Exact row-major `[lda, ldb, ldc]` strides in elements.
    pub const fn strides(self) -> [u32; 3] {
        self.strides
    }

    /// Exact accessed `[A BF16, B BF16, C FP32]` element extents.
    pub const fn storage_elements(self) -> [u64; 3] {
        self.storage_elements
    }

    /// HSA-adapter block counts before workgroup expansion.
    pub const fn hsa_adapter_block_counts(self) -> [u32; 3] {
        self.block_counts
    }

    /// Exact AQL total-workitem grid.
    pub const fn aql_grid_work_items(self) -> [u32; 3] {
        self.aql_grid_work_items
    }

    /// Exact number of K/16 reduction phases.
    pub const fn reduction_phases(self) -> u32 {
        self.reduction_phases
    }

    /// Exact alpha bits, always FP32 one.
    pub const fn alpha_bits(self) -> u32 {
        1.0_f32.to_bits()
    }

    /// Exact beta bits, FP32 one only for the two residual projections.
    pub const fn beta_bits(self) -> u32 {
        self.operation.beta_bits()
    }

    /// GEMV for M=1 and tiled GEMM otherwise.
    pub const fn execution_class(self) -> Qwen3GemmExecutionClassV1 {
        if self.dimensions[0] == 1 {
            Qwen3GemmExecutionClassV1::GemvM1
        } else {
            Qwen3GemmExecutionClassV1::TiledGemm
        }
    }

    /// Exact domain-separated profile identity.
    pub const fn identity(self) -> Qwen3GemmProfileIdentityV1 {
        self.identity
    }

    /// Profile geometry is inert and grants no launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Failure while deriving the immutable finite catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3GemmCatalogErrorV1 {
    /// A matrix or byte extent overflowed its checked integer domain.
    ExtentOverflow,
    /// Workgroup expansion overflowed the AQL grid domain.
    GridOverflow,
    /// The upstream checked general-GEMM planner rejected a derived field.
    Plan(GeneralGemmPlanSnapshotErrorV1),
}

impl fmt::Display for Qwen3GemmCatalogErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 GEMM profile catalog failed: {self:?}")
    }
}

impl std::error::Error for Qwen3GemmCatalogErrorV1 {}

/// SHA-256 identity of the complete finite catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Qwen3GemmProfileCatalogIdentityV1([u8; 32]);

impl Qwen3GemmProfileCatalogIdentityV1 {
    /// Returns the exact catalog identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Complete finite target/draft GEMM/GEMV profile catalog.
#[derive(Debug, Eq, PartialEq)]
pub struct Qwen3GemmProfileCatalogV1 {
    profiles: Box<[Qwen3GemmProfileV1]>,
    canonical_bytes: Box<[u8]>,
    identity: Qwen3GemmProfileCatalogIdentityV1,
}

impl Qwen3GemmProfileCatalogV1 {
    /// Constructs all 176 exact profiles in stable role/bucket/operator order.
    pub fn canonical() -> Result<Self, Qwen3GemmCatalogErrorV1> {
        let mut profiles = Vec::with_capacity(QWEN3_GEMM_PROFILE_COUNT_V1);
        for role in QWEN3_GEMM_ROLES_V1 {
            for kind in QWEN3_GEMM_BUCKET_KINDS_V1 {
                let bucket = Qwen3GemmBucketV1::new(role, kind);
                for operation in QWEN3_GEMM_OPERATIONS_V1 {
                    profiles.push(Qwen3GemmProfileV1::checked(bucket, operation)?);
                }
            }
        }
        let mut canonical_bytes = Vec::with_capacity(20_000);
        canonical_bytes.extend_from_slice(&(profiles.len() as u32).to_le_bytes());
        canonical_bytes.extend_from_slice(QWEN3_GEMM_TARGET_V1.as_bytes());
        canonical_bytes.push(QWEN3_GEMM_CODE_OBJECT_VERSION_V1);
        for profile in &profiles {
            let encoded = profile.encode();
            canonical_bytes.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            canonical_bytes.extend_from_slice(&encoded);
            canonical_bytes.extend_from_slice(profile.identity.as_bytes());
        }
        let identity = Qwen3GemmProfileCatalogIdentityV1(hash(CATALOG_DOMAIN, &canonical_bytes));
        Ok(Self {
            profiles: profiles.into_boxed_slice(),
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            identity,
        })
    }

    /// Exact stable profile roster.
    pub fn profiles(&self) -> &[Qwen3GemmProfileV1] {
        &self.profiles
    }

    /// Finds the exact profile for one role/bucket/operation tuple.
    pub fn profile(
        &self,
        bucket: Qwen3GemmBucketV1,
        operation: Qwen3GemmOperationV1,
    ) -> Option<Qwen3GemmProfileV1> {
        self.profiles
            .iter()
            .copied()
            .find(|profile| profile.bucket == bucket && profile.operation == operation)
    }

    /// Canonical bytes retaining every checked dimension, stride, extent, and grid.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Exact catalog identity.
    pub const fn identity(&self) -> Qwen3GemmProfileCatalogIdentityV1 {
        self.identity
    }

    /// The catalog is a structural input, not source or artifact authentication.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

const QWEN3_GEMM_ROLES_V1: [Qwen3GemmModelRoleV1; 2] = [
    Qwen3GemmModelRoleV1::Target8B,
    Qwen3GemmModelRoleV1::Draft06B,
];

const QWEN3_GEMM_BUCKET_KINDS_V1: [Qwen3GemmBucketKindV1; 11] = [
    Qwen3GemmBucketKindV1::PrefillS1T128,
    Qwen3GemmBucketKindV1::PrefillS8T128,
    Qwen3GemmBucketKindV1::PrefillS1T512,
    Qwen3GemmBucketKindV1::PrefillS1T2048,
    Qwen3GemmBucketKindV1::DecodeS1C8192,
    Qwen3GemmBucketKindV1::DecodeS8C8192,
    Qwen3GemmBucketKindV1::DecodeS32C8192,
    Qwen3GemmBucketKindV1::SpeculativeS1K4C8192,
    Qwen3GemmBucketKindV1::SpeculativeS8K4C8192,
    Qwen3GemmBucketKindV1::SpeculativeS1K8C8192,
    Qwen3GemmBucketKindV1::SpeculativeS1K16C8192,
];

const QWEN3_GEMM_OPERATIONS_V1: [Qwen3GemmOperationV1; 8] = [
    Qwen3GemmOperationV1::QueryProjection,
    Qwen3GemmOperationV1::KeyProjection,
    Qwen3GemmOperationV1::ValueProjection,
    Qwen3GemmOperationV1::AttentionOutputResidual,
    Qwen3GemmOperationV1::GateProjection,
    Qwen3GemmOperationV1::UpProjection,
    Qwen3GemmOperationV1::DownResidual,
    Qwen3GemmOperationV1::LogitsProjection,
];

/// One numerical buffer role in the dynamic ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3GemmBufferV1 {
    /// Row-major BF16 activation matrix A.
    A,
    /// Row-major BF16 weight matrix B.
    B,
    /// Row-major FP32 accumulator/output matrix C.
    C,
}

/// Numerical buffer-contract rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qwen3GemmBufferContractErrorV1 {
    /// A numerical address was zero.
    ZeroAddress(Qwen3GemmBufferV1),
    /// A supplied byte span differed from the exact profile extent.
    ByteLength(Qwen3GemmBufferV1),
    /// A numerical address did not meet the declared alignment.
    Alignment(Qwen3GemmBufferV1),
    /// A half-open numerical address range overflowed.
    RangeOverflow(Qwen3GemmBufferV1),
    /// Two numerical half-open ranges overlap.
    Aliasing,
    /// Converting an element extent to bytes overflowed.
    ExtentOverflow,
}

impl fmt::Display for Qwen3GemmBufferContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 GEMM numerical buffers failed: {self:?}")
    }
}

impl std::error::Error for Qwen3GemmBufferContractErrorV1 {}

/// Checked numerical ranges for A, B, and C in ABI order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3GemmBufferContractV1 {
    starts: [u64; 3],
    ends: [u64; 3],
    byte_lengths: [u64; 3],
}

impl Qwen3GemmBufferContractV1 {
    /// Checks exact byte spans, alignments, overflow, and pairwise disjointness.
    pub fn checked(
        profile: Qwen3GemmProfileV1,
        addresses: [u64; 3],
        byte_lengths: [u64; 3],
    ) -> Result<Self, Qwen3GemmBufferContractErrorV1> {
        let elements = profile.storage_elements;
        let expected = [
            elements[0]
                .checked_mul(2)
                .ok_or(Qwen3GemmBufferContractErrorV1::ExtentOverflow)?,
            elements[1]
                .checked_mul(2)
                .ok_or(Qwen3GemmBufferContractErrorV1::ExtentOverflow)?,
            elements[2]
                .checked_mul(4)
                .ok_or(Qwen3GemmBufferContractErrorV1::ExtentOverflow)?,
        ];
        let buffers = [
            Qwen3GemmBufferV1::A,
            Qwen3GemmBufferV1::B,
            Qwen3GemmBufferV1::C,
        ];
        let alignments = [8_u64, 2, 4];
        let mut ends = [0; 3];
        for index in 0..3 {
            if addresses[index] == 0 {
                return Err(Qwen3GemmBufferContractErrorV1::ZeroAddress(buffers[index]));
            }
            if byte_lengths[index] != expected[index] {
                return Err(Qwen3GemmBufferContractErrorV1::ByteLength(buffers[index]));
            }
            if !addresses[index].is_multiple_of(alignments[index]) {
                return Err(Qwen3GemmBufferContractErrorV1::Alignment(buffers[index]));
            }
            ends[index] = addresses[index].checked_add(byte_lengths[index]).ok_or(
                Qwen3GemmBufferContractErrorV1::RangeOverflow(buffers[index]),
            )?;
        }
        for left in 0..3 {
            for right in left + 1..3 {
                if addresses[left] < ends[right] && addresses[right] < ends[left] {
                    return Err(Qwen3GemmBufferContractErrorV1::Aliasing);
                }
            }
        }
        Ok(Self {
            starts: addresses,
            ends,
            byte_lengths,
        })
    }

    /// Exact checked numerical starts in A, B, C order.
    pub const fn starts(self) -> [u64; 3] {
        self.starts
    }

    /// Exact checked numerical exclusive ends in A, B, C order.
    pub const fn ends(self) -> [u64; 3] {
        self.ends
    }

    /// Exact checked byte lengths in A, B, C order.
    pub const fn byte_lengths(self) -> [u64; 3] {
        self.byte_lengths
    }

    /// Integer ranges do not authenticate KFD mappings or initialized content.
    pub const fn authenticates_device_memory(self) -> bool {
        false
    }

    /// Numerical ranges grant no launch authority.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

/// Caller-supplied inert source observations retained by both schedule lanes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3GemmSourceBindingsV1 {
    compiled_source: [u8; 32],
    provider_semantics: [u8; 32],
    frontend_abi: [u8; 32],
    target_plan: [u8; 32],
}

impl Qwen3GemmSourceBindingsV1 {
    /// Constructs inert labels. Preparation requires all four to be nonzero and distinct.
    pub const fn new(
        compiled_source: [u8; 32],
        provider_semantics: [u8; 32],
        frontend_abi: [u8; 32],
        target_plan: [u8; 32],
    ) -> Self {
        Self {
            compiled_source,
            provider_semantics,
            frontend_abi,
            target_plan,
        }
    }

    /// Caller labels do not authenticate source, producer, compiler, or target provenance.
    pub const fn authenticates_provenance(self) -> bool {
        false
    }
}

/// Failure while preparing the exact two-schedule compiler source.
#[derive(Debug)]
pub enum PrepareQwen3GemmKernelSetErrorV1 {
    /// A source label was zero or reused for another role.
    SourceBindings,
    /// The canonical finite profile catalog failed.
    Catalog(Qwen3GemmCatalogErrorV1),
    /// An exact frontend/request/symbolic unit could not be constructed.
    CompilerInput,
}

impl fmt::Display for PrepareQwen3GemmKernelSetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 GEMM source preparation failed: {self:?}")
    }
}

impl std::error::Error for PrepareQwen3GemmKernelSetErrorV1 {}

/// Linear prepared source for both exact schedule variants.
#[derive(Debug)]
pub struct PreparedQwen3GemmKernelSetV1 {
    catalog: Qwen3GemmProfileCatalogV1,
    reference: GeneralGemmSymbolicCompilationUnitV1,
    vectorized: GeneralGemmSymbolicCompilationUnitV1,
}

impl PreparedQwen3GemmKernelSetV1 {
    /// Complete finite profile catalog retained by this source owner.
    pub const fn catalog(&self) -> &Qwen3GemmProfileCatalogV1 {
        &self.catalog
    }

    /// Preparation creates no artifact authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

/// Constructs the finite catalog and two exact symbolic compilation units.
pub fn prepare_qwen3_gemm_kernel_set_v1(
    bindings: Qwen3GemmSourceBindingsV1,
) -> Result<PreparedQwen3GemmKernelSetV1, PrepareQwen3GemmKernelSetErrorV1> {
    validate_source_bindings(bindings)?;
    let catalog = Qwen3GemmProfileCatalogV1::canonical()
        .map_err(PrepareQwen3GemmKernelSetErrorV1::Catalog)?;
    let reference = prepare_unit(
        &catalog,
        bindings,
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
    )?;
    let vectorized = prepare_unit(
        &catalog,
        bindings,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
    )?;
    Ok(PreparedQwen3GemmKernelSetV1 {
        catalog,
        reference,
        vectorized,
    })
}

fn validate_source_bindings(
    bindings: Qwen3GemmSourceBindingsV1,
) -> Result<(), PrepareQwen3GemmKernelSetErrorV1> {
    let identities = [
        bindings.compiled_source,
        bindings.provider_semantics,
        bindings.frontend_abi,
        bindings.target_plan,
    ];
    for (index, identity) in identities.iter().enumerate() {
        if identity == &[0; 32] || identities[index + 1..].contains(identity) {
            return Err(PrepareQwen3GemmKernelSetErrorV1::SourceBindings);
        }
    }
    Ok(())
}

fn prepare_unit(
    catalog: &Qwen3GemmProfileCatalogV1,
    bindings: Qwen3GemmSourceBindingsV1,
    schedule: GeneralGemmScheduleV1,
) -> Result<GeneralGemmSymbolicCompilationUnitV1, PrepareQwen3GemmKernelSetErrorV1> {
    let kernel_instance = hash(KERNEL_DOMAIN, catalog.identity.as_bytes());
    let frontend =
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            kernel_instance,
            bindings.compiled_source,
            bindings.provider_semantics,
            bindings.frontend_abi,
            GeneralGemmSymbolicPlanV1::canonical(),
            GeneralGemmSymbolicKirV1::canonical(),
        )
        .map_err(|_| PrepareQwen3GemmKernelSetErrorV1::CompilerInput)?;
    let mut snapshot_bytes = catalog.canonical_bytes.to_vec();
    snapshot_bytes.extend_from_slice(&bindings.target_plan);
    let snapshot_identity = hash(SNAPSHOT_DOMAIN, &snapshot_bytes);
    let input = StageSnapshotV1::new(
        CompilerStageV1::FrontendInput,
        SnapshotIdentityV1::from_untrusted_bytes(snapshot_identity),
        SnapshotFormatIdentityV1::from_untrusted_bytes(hash(
            SNAPSHOT_FORMAT_DOMAIN,
            b"qwen3-gemm-profile-catalog-v1",
        )),
        snapshot_bytes,
    )
    .map_err(|_| PrepareQwen3GemmKernelSetErrorV1::CompilerInput)?;
    let obligations = general_gemm_symbolic_obligation_set_identity_v1(&input, &frontend);
    let request = CompileRequestV1::new(
        RequestIdentityV1::from_untrusted_bytes(hash(REQUEST_DOMAIN, catalog.identity.as_bytes())),
        KernelInstanceIdentityV1::from_untrusted_bytes(kernel_instance),
        CompilerProfileIdentityV1::from_untrusted_bytes(hash(
            COMPILER_PROFILE_DOMAIN,
            b"typed-general-gemm-pliron-handoff-v2-worker-v2",
        )),
        TargetProfileIdentityV1::from_untrusted_bytes(hash(
            TARGET_PROFILE_DOMAIN,
            b"gfx942:xnack-/cov6/wave64",
        )),
        general_gemm_symbolic_pipeline_configuration_identity_v1(schedule),
        obligations,
        PipelineSelectorV1::PlironV1,
        input,
        CompileLimitsV1::new(16, 16, 16, 64 * 1024, 64 * 1024, 4_096)
            .map_err(|_| PrepareQwen3GemmKernelSetErrorV1::CompilerInput)?,
    )
    .map_err(|_| PrepareQwen3GemmKernelSetErrorV1::CompilerInput)?;
    GeneralGemmSymbolicCompilationUnitV1::checked(
        &request,
        frontend,
        schedule,
        GeneralGemmLoweringLimitsV1::default(),
    )
    .map_err(|_| PrepareQwen3GemmKernelSetErrorV1::CompilerInput)
}

fn unit_retains_catalog(
    unit: &GeneralGemmSymbolicCompilationUnitV1,
    catalog: &Qwen3GemmProfileCatalogV1,
) -> bool {
    let input = unit.request().input();
    let bytes = input.canonical_bytes();
    bytes.len() == catalog.canonical_bytes.len() + 32
        && bytes.starts_with(&catalog.canonical_bytes)
        && input.identity().as_bytes() == &hash(SNAPSHOT_DOMAIN, bytes)
        && input.format_identity().as_bytes()
            == &hash(SNAPSHOT_FORMAT_DOMAIN, b"qwen3-gemm-profile-catalog-v1")
        && unit.request().kernel_instance_identity().as_bytes()
            == &hash(KERNEL_DOMAIN, catalog.identity.as_bytes())
        && unit.request().identity().as_bytes()
            == &hash(REQUEST_DOMAIN, catalog.identity.as_bytes())
}

struct Qwen3GemmCompilerLaneV1 {
    unit: GeneralGemmSymbolicCompilationUnitV1,
    machine: GeneralGemmSymbolicStructuralMachineV1,
}

/// Failure while lowering both prepared sources into typed Worker V2 handoffs.
#[derive(Debug)]
pub enum LowerQwen3GemmKernelSetErrorV1 {
    /// The reference schedule failed structural machine lowering.
    Reference(GeneralGemmStructuralMachineErrorV1),
    /// The vectorized-A schedule failed structural machine lowering.
    Vectorized(GeneralGemmStructuralMachineErrorV1),
    /// A structural machine no longer named its exact source unit.
    SourceSubstitution,
}

impl fmt::Display for LowerQwen3GemmKernelSetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 GEMM typed lowering failed: {self:?}")
    }
}

impl std::error::Error for LowerQwen3GemmKernelSetErrorV1 {}

/// Linear pair of exact typed compiler handoffs awaiting transaction publication.
pub struct InertQwen3GemmKernelWorkerRequestV1 {
    catalog: Qwen3GemmProfileCatalogV1,
    reference: Qwen3GemmCompilerLaneV1,
    vectorized: Qwen3GemmCompilerLaneV1,
}

impl fmt::Debug for InertQwen3GemmKernelWorkerRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertQwen3GemmKernelWorkerRequestV1")
            .field("catalog", &self.catalog.identity)
            .field("reference", &self.reference.unit.identity())
            .field("vectorized", &self.vectorized.unit.identity())
            .finish_non_exhaustive()
    }
}

impl InertQwen3GemmKernelWorkerRequestV1 {
    /// Complete finite catalog retained with both source owners.
    pub const fn catalog(&self) -> &Qwen3GemmProfileCatalogV1 {
        &self.catalog
    }

    /// Exact compiler handoff for one closed schedule.
    pub const fn compiler_handoff(
        &self,
        schedule: GeneralGemmScheduleV1,
    ) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        match schedule {
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
                self.reference.machine.compiler_handoff()
            }
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
                self.vectorized.machine.compiler_handoff()
            }
        }
    }

    /// Exact typed source identity for one closed schedule.
    pub fn source_identity(
        &self,
        schedule: GeneralGemmScheduleV1,
    ) -> fe2o3_llvm_handoff::HandoffIdentityV2 {
        match schedule {
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
                self.reference.machine.graph_handoff().identity()
            }
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
                self.vectorized.machine.graph_handoff().identity()
            }
        }
    }

    /// A typed Worker V2 handoff grants no artifact authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

/// Consumes the prepared source pair into real typed LLVM/Worker V2 handoffs.
pub fn lower_qwen3_gemm_kernel_set_v1(
    prepared: PreparedQwen3GemmKernelSetV1,
) -> Result<InertQwen3GemmKernelWorkerRequestV1, LowerQwen3GemmKernelSetErrorV1> {
    let PreparedQwen3GemmKernelSetV1 {
        catalog,
        reference,
        vectorized,
    } = prepared;
    let reference_machine = lower_general_gemm_symbolic_structural_machine_v1(&reference)
        .map_err(LowerQwen3GemmKernelSetErrorV1::Reference)?;
    let vectorized_machine = lower_general_gemm_symbolic_structural_machine_v1(&vectorized)
        .map_err(LowerQwen3GemmKernelSetErrorV1::Vectorized)?;
    if !unit_retains_catalog(&reference, &catalog)
        || !unit_retains_catalog(&vectorized, &catalog)
        || reference_machine.projection().compilation_identity() != reference.identity()
        || vectorized_machine.projection().compilation_identity() != vectorized.identity()
        || reference_machine.projection().schedule_identity() != reference.schedule_identity()
        || vectorized_machine.projection().schedule_identity() != vectorized.schedule_identity()
    {
        return Err(LowerQwen3GemmKernelSetErrorV1::SourceSubstitution);
    }
    Ok(InertQwen3GemmKernelWorkerRequestV1 {
        catalog,
        reference: Qwen3GemmCompilerLaneV1 {
            unit: reference,
            machine: reference_machine,
        },
        vectorized: Qwen3GemmCompilerLaneV1 {
            unit: vectorized,
            machine: vectorized_machine,
        },
    })
}

struct Qwen3GemmWorkerLaneEvidenceV1 {
    unit: GeneralGemmSymbolicCompilationUnitV1,
    evidence: InertSymbolicGeneralGemmWorkerV2EvidenceV1,
}

/// Owning pair of exact Worker V2 executions awaiting post-link inspection.
pub struct InertQwen3GemmKernelWorkerEvidenceV1 {
    catalog: Qwen3GemmProfileCatalogV1,
    reference: Qwen3GemmWorkerLaneEvidenceV1,
    vectorized: Qwen3GemmWorkerLaneEvidenceV1,
}

impl fmt::Debug for InertQwen3GemmKernelWorkerEvidenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertQwen3GemmKernelWorkerEvidenceV1")
            .field("catalog", &self.catalog.identity)
            .field("reference", &self.reference.evidence.identity())
            .field("vectorized", &self.vectorized.evidence.identity())
            .finish_non_exhaustive()
    }
}

impl InertQwen3GemmKernelWorkerEvidenceV1 {
    /// The measured results remain non-authoritative pending post-link inspection.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}

/// Failure while executing the two exact Worker V2 requests.
#[derive(Debug)]
pub enum ExecuteQwen3GemmKernelSetErrorV1 {
    /// The reference schedule Worker V2 execution failed.
    Reference(GeneralGemmWorkerV2ErrorV1),
    /// The vectorized-A schedule Worker V2 execution failed.
    Vectorized(GeneralGemmWorkerV2ErrorV1),
}

impl fmt::Display for ExecuteQwen3GemmKernelSetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 GEMM Worker V2 execution failed: {self:?}")
    }
}

impl std::error::Error for ExecuteQwen3GemmKernelSetErrorV1 {}

/// Consumes both transaction handoffs through the same measured Worker V2 executable.
pub fn execute_qwen3_gemm_kernel_set_worker_v2_v1(
    request: InertQwen3GemmKernelWorkerRequestV1,
    reference_consumed: ConsumedCompilerModuleHandoffV1,
    vectorized_consumed: ConsumedCompilerModuleHandoffV1,
    worker: &PinnedWorkerV1,
    limits: WorkerExecutionLimitsV1,
) -> Result<InertQwen3GemmKernelWorkerEvidenceV1, ExecuteQwen3GemmKernelSetErrorV1> {
    let InertQwen3GemmKernelWorkerRequestV1 {
        catalog,
        reference,
        vectorized,
    } = request;
    let reference_evidence = execute_symbolic_general_gemm_worker_v2_v1(
        reference.machine,
        reference_consumed,
        worker,
        limits,
    )
    .map_err(ExecuteQwen3GemmKernelSetErrorV1::Reference)?;
    let vectorized_evidence = execute_symbolic_general_gemm_worker_v2_v1(
        vectorized.machine,
        vectorized_consumed,
        worker,
        limits,
    )
    .map_err(ExecuteQwen3GemmKernelSetErrorV1::Vectorized)?;
    Ok(InertQwen3GemmKernelWorkerEvidenceV1 {
        catalog,
        reference: Qwen3GemmWorkerLaneEvidenceV1 {
            unit: reference.unit,
            evidence: reference_evidence,
        },
        vectorized: Qwen3GemmWorkerLaneEvidenceV1 {
            unit: vectorized.unit,
            evidence: vectorized_evidence,
        },
    })
}

struct Qwen3GemmInspectedLaneV1 {
    unit: GeneralGemmSymbolicCompilationUnitV1,
    observation: OpaqueGeneralGemmPostLinkMachineObservationV1,
    loader_plan: LoadPlan,
}

/// Inert post-worker artifacts for both schedules and all finite profiles.
pub struct InspectedQwen3GemmKernelSetV1 {
    catalog: Qwen3GemmProfileCatalogV1,
    reference: Qwen3GemmInspectedLaneV1,
    vectorized: Qwen3GemmInspectedLaneV1,
}

impl fmt::Debug for InspectedQwen3GemmKernelSetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InspectedQwen3GemmKernelSetV1")
            .field("catalog", &self.catalog.identity)
            .field("reference", &self.reference.observation.identity())
            .field("vectorized", &self.vectorized.observation.identity())
            .finish_non_exhaustive()
    }
}

impl InspectedQwen3GemmKernelSetV1 {
    /// Complete profile catalog retained with both post-link owners.
    pub const fn catalog(&self) -> &Qwen3GemmProfileCatalogV1 {
        &self.catalog
    }

    /// Strict pure-Rust loader plan for one exact finalized schedule artifact.
    pub const fn loader_plan(&self, schedule: GeneralGemmScheduleV1) -> &LoadPlan {
        match schedule {
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => &self.reference.loader_plan,
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
                &self.vectorized.loader_plan
            }
        }
    }

    /// Exact finalized bytes retained by one post-link owner.
    pub fn exact_finalized_bytes(&self, schedule: GeneralGemmScheduleV1) -> &[u8] {
        match schedule {
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => {
                self.reference.observation.exact_finalized_bytes()
            }
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => {
                self.vectorized.observation.exact_finalized_bytes()
            }
        }
    }

    /// Observed exact bytes are not an independently approved deployment pin.
    pub const fn has_independent_deployment_pin(&self) -> bool {
        false
    }

    /// Inspection grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Inspection grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Binds one exact profile and numerical buffer layout to the retained compiler owner.
    pub fn bind_checked_launch(
        &self,
        bucket: Qwen3GemmBucketV1,
        operation: Qwen3GemmOperationV1,
        addresses: [u64; 3],
        byte_lengths: [u64; 3],
    ) -> Result<CheckedQwen3GemmLaunchV1, BindQwen3GemmLaunchErrorV1> {
        let profile = self
            .catalog
            .profile(bucket, operation)
            .ok_or(BindQwen3GemmLaunchErrorV1::Profile)?;
        let buffers = Qwen3GemmBufferContractV1::checked(profile, addresses, byte_lengths)
            .map_err(BindQwen3GemmLaunchErrorV1::Buffers)?;
        let plan = profile
            .checked_plan()
            .map_err(BindQwen3GemmLaunchErrorV1::Catalog)?;
        let kir = GeneralGemmKirV1::canonical(plan);
        let snapshot = profile.plan_snapshot();
        let (lane, artifact_identity) = match profile.schedule {
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => (
                &self.reference,
                self.reference.observation.symbolic_artifact_identity(),
            ),
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => (
                &self.vectorized,
                self.vectorized.observation.symbolic_artifact_identity(),
            ),
        };
        let launch = GeneralGemmCheckedLaunchInstantiationV1::checked(
            &lane.unit,
            artifact_identity,
            plan,
            kir,
            GeneralGemmRuntimeAbiSnapshotV1 {
                a_elements: snapshot.storage_elements[0],
                b_elements: snapshot.storage_elements[1],
                c_elements: snapshot.storage_elements[2],
                dimensions: snapshot.dimensions,
                strides: snapshot.strides,
                alpha_bits: snapshot.alpha_bits,
                beta_bits: snapshot.beta_bits,
            },
        )
        .map_err(BindQwen3GemmLaunchErrorV1::Launch)?;
        Ok(CheckedQwen3GemmLaunchV1 {
            profile,
            buffers,
            launch,
        })
    }
}

/// Exact post-link inspection failure.
#[derive(Debug)]
pub enum InspectQwen3GemmKernelSetErrorV1 {
    /// Reference-schedule post-link inspection failed.
    Reference(GeneralGemmPostLinkMachineErrorV1),
    /// Vectorized-schedule post-link inspection failed.
    Vectorized(GeneralGemmPostLinkMachineErrorV1),
    /// A post-link owner no longer named the retained source unit or schedule.
    SourceSubstitution,
    /// The strict COV6 loader rejected finalized bytes.
    Loader(PlanError),
}

impl fmt::Display for InspectQwen3GemmKernelSetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Qwen3 GEMM post-link inspection failed: {self:?}"
        )
    }
}

impl std::error::Error for InspectQwen3GemmKernelSetErrorV1 {}

/// Consumes both Worker V2 owners through exact post-link and strict loader validation.
pub fn inspect_qwen3_gemm_kernel_set_v1(
    evidence: InertQwen3GemmKernelWorkerEvidenceV1,
) -> Result<InspectedQwen3GemmKernelSetV1, InspectQwen3GemmKernelSetErrorV1> {
    let InertQwen3GemmKernelWorkerEvidenceV1 {
        catalog,
        reference,
        vectorized,
    } = evidence;
    let reference_observation = finalize_symbolic_general_gemm_worker_v2_v1(reference.evidence)
        .map_err(InspectQwen3GemmKernelSetErrorV1::Reference)?;
    let vectorized_observation = finalize_symbolic_general_gemm_worker_v2_v1(vectorized.evidence)
        .map_err(InspectQwen3GemmKernelSetErrorV1::Vectorized)?;
    if reference_observation.symbolic_compilation_identity() != reference.unit.identity()
        || vectorized_observation.symbolic_compilation_identity() != vectorized.unit.identity()
        || !unit_retains_catalog(&reference.unit, &catalog)
        || !unit_retains_catalog(&vectorized.unit, &catalog)
        || reference_observation.schedule() != GeneralGemmScheduleV1::ReferenceWave64Xor4V1
        || vectorized_observation.schedule()
            != GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
    {
        return Err(InspectQwen3GemmKernelSetErrorV1::SourceSubstitution);
    }
    let reference_loader = fe2o3_amdhsa_loader::validate(
        reference_observation.exact_finalized_bytes(),
        AdmittedProfile::Gfx942XnackOffCov6,
    )
    .map_err(InspectQwen3GemmKernelSetErrorV1::Loader)?;
    let vectorized_loader = fe2o3_amdhsa_loader::validate(
        vectorized_observation.exact_finalized_bytes(),
        AdmittedProfile::Gfx942XnackOffCov6,
    )
    .map_err(InspectQwen3GemmKernelSetErrorV1::Loader)?;
    let reference_loader_plan = *reference_loader.plan();
    let vectorized_loader_plan = *vectorized_loader.plan();
    Ok(InspectedQwen3GemmKernelSetV1 {
        catalog,
        reference: Qwen3GemmInspectedLaneV1 {
            unit: reference.unit,
            observation: reference_observation,
            loader_plan: reference_loader_plan,
        },
        vectorized: Qwen3GemmInspectedLaneV1 {
            unit: vectorized.unit,
            observation: vectorized_observation,
            loader_plan: vectorized_loader_plan,
        },
    })
}

/// Failure while binding one finite profile to exact runtime values.
#[derive(Debug)]
pub enum BindQwen3GemmLaunchErrorV1 {
    /// The requested role/bucket/operation is absent from the finite catalog.
    Profile,
    /// The numerical buffer contract failed.
    Buffers(Qwen3GemmBufferContractErrorV1),
    /// Reconstructing the exact checked plan failed.
    Catalog(Qwen3GemmCatalogErrorV1),
    /// The upstream symbolic artifact launch binding failed.
    Launch(GeneralGemmCheckedLaunchInstantiationErrorV1),
}

impl fmt::Display for BindQwen3GemmLaunchErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 GEMM launch binding failed: {self:?}")
    }
}

impl std::error::Error for BindQwen3GemmLaunchErrorV1 {}

/// Exact inert profile/runtime binding retained for a future protected launcher.
#[derive(Debug)]
pub struct CheckedQwen3GemmLaunchV1 {
    profile: Qwen3GemmProfileV1,
    buffers: Qwen3GemmBufferContractV1,
    launch: GeneralGemmCheckedLaunchInstantiationV1,
}

impl CheckedQwen3GemmLaunchV1 {
    /// Exact finite profile.
    pub const fn profile(&self) -> Qwen3GemmProfileV1 {
        self.profile
    }

    /// Exact checked numerical buffer ranges.
    pub const fn buffers(&self) -> Qwen3GemmBufferContractV1 {
        self.buffers
    }

    /// Upstream exact symbolic-artifact launch instantiation.
    pub const fn general_gemm_launch(&self) -> &GeneralGemmCheckedLaunchInstantiationV1 {
        &self.launch
    }

    /// This binding grants no allocation, load, or launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

const fn ceil_div_16(value: u32) -> u32 {
    value / 16 + if value.is_multiple_of(16) { 0 } else { 1 }
}

const fn schedule_tag(schedule: GeneralGemmScheduleV1) -> u8 {
    match schedule {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => 1,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => 2,
    }
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

    fn bindings(seed: u8) -> Qwen3GemmSourceBindingsV1 {
        Qwen3GemmSourceBindingsV1::new(
            [seed; 32],
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
        )
    }

    #[test]
    fn catalog_is_exact_complete_and_deterministic() {
        let first = Qwen3GemmProfileCatalogV1::canonical().unwrap();
        let second = Qwen3GemmProfileCatalogV1::canonical().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.profiles().len(), QWEN3_GEMM_PROFILE_COUNT_V1);
        assert_ne!(first.identity().as_bytes(), &[0; 32]);
        let identities = first
            .profiles()
            .iter()
            .map(|profile| *profile.identity().as_bytes())
            .collect::<BTreeSet<_>>();
        assert_eq!(identities.len(), QWEN3_GEMM_PROFILE_COUNT_V1);
        assert!(!first.grants_authority());
    }

    #[test]
    fn finite_rows_match_all_target_and_draft_buckets() {
        let catalog = Qwen3GemmProfileCatalogV1::canonical().unwrap();
        let rows = |role| {
            catalog
                .profiles()
                .iter()
                .filter(|profile| profile.bucket().role() == role)
                .map(|profile| profile.dimensions()[0])
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            rows(Qwen3GemmModelRoleV1::Target8B),
            BTreeSet::from([1, 5, 8, 9, 17, 32, 40, 128, 512, 1_024, 2_048])
        );
        assert_eq!(
            rows(Qwen3GemmModelRoleV1::Draft06B),
            BTreeSet::from([1, 4, 8, 16, 32, 128, 512, 1_024, 2_048])
        );
    }

    #[test]
    fn exact_projection_geometry_includes_draft_query_and_logits() {
        let catalog = Qwen3GemmProfileCatalogV1::canonical().unwrap();
        let draft = Qwen3GemmBucketV1::new(
            Qwen3GemmModelRoleV1::Draft06B,
            Qwen3GemmBucketKindV1::DecodeS1C8192,
        );
        assert_eq!(
            catalog
                .profile(draft, Qwen3GemmOperationV1::QueryProjection)
                .unwrap()
                .dimensions(),
            [1, 2_048, 1_024]
        );
        assert_eq!(
            catalog
                .profile(draft, Qwen3GemmOperationV1::LogitsProjection)
                .unwrap()
                .dimensions(),
            [1, 151_936, 1_024]
        );
    }

    #[test]
    fn adapter_blocks_and_aql_workitems_cannot_be_confused() {
        let catalog = Qwen3GemmProfileCatalogV1::canonical().unwrap();
        for profile in catalog.profiles() {
            let blocks = profile.hsa_adapter_block_counts();
            let grid = profile.aql_grid_work_items();
            assert_eq!(blocks[0].checked_mul(64), Some(grid[0]));
            assert_eq!(blocks[1], grid[1]);
            assert_ne!(blocks, grid);
        }
    }

    #[test]
    fn residual_coefficients_and_schedule_split_are_exact() {
        let catalog = Qwen3GemmProfileCatalogV1::canonical().unwrap();
        for profile in catalog.profiles() {
            assert_eq!(profile.alpha_bits(), 1.0_f32.to_bits());
            let residual = matches!(
                profile.operation(),
                Qwen3GemmOperationV1::AttentionOutputResidual | Qwen3GemmOperationV1::DownResidual
            );
            assert_eq!(
                profile.beta_bits(),
                if residual { 1.0_f32 } else { 0.0 }.to_bits()
            );
            assert_eq!(
                profile.schedule(),
                if profile.dimensions()[0] < 16 {
                    GeneralGemmScheduleV1::ReferenceWave64Xor4V1
                } else {
                    GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1
                }
            );
        }
    }

    #[test]
    fn buffer_contract_rejects_substitution_aliasing_and_overflow() {
        let catalog = Qwen3GemmProfileCatalogV1::canonical().unwrap();
        let profile = catalog.profiles()[0];
        let elements = profile.storage_elements();
        let lengths = [elements[0] * 2, elements[1] * 2, elements[2] * 4];
        let addresses = [0x1_0000, 0x1000_0000, 0x1_0000_0000];
        let checked = Qwen3GemmBufferContractV1::checked(profile, addresses, lengths).unwrap();
        assert_eq!(checked.byte_lengths(), lengths);
        assert!(!checked.authenticates_device_memory());
        let mut short = lengths;
        short[1] -= 2;
        assert_eq!(
            Qwen3GemmBufferContractV1::checked(profile, addresses, short),
            Err(Qwen3GemmBufferContractErrorV1::ByteLength(
                Qwen3GemmBufferV1::B
            ))
        );
        assert_eq!(
            Qwen3GemmBufferContractV1::checked(
                profile,
                [addresses[0], addresses[0] + 8, addresses[2]],
                lengths,
            ),
            Err(Qwen3GemmBufferContractErrorV1::Aliasing)
        );
        assert!(matches!(
            Qwen3GemmBufferContractV1::checked(
                profile,
                [u64::MAX - 7, addresses[1], addresses[2]],
                lengths,
            ),
            Err(Qwen3GemmBufferContractErrorV1::RangeOverflow(
                Qwen3GemmBufferV1::A
            )) | Err(Qwen3GemmBufferContractErrorV1::Alignment(
                Qwen3GemmBufferV1::A
            ))
        ));
    }

    #[test]
    fn source_bindings_reject_zero_and_all_six_repeated_role_pairs() {
        let valid = bindings(0x31);
        assert!(prepare_qwen3_gemm_kernel_set_v1(valid).is_ok());
        let mut identities = [[0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32]];
        for first in 0..4 {
            for second in first + 1..4 {
                identities[second] = identities[first];
                let rejected = Qwen3GemmSourceBindingsV1::new(
                    identities[0],
                    identities[1],
                    identities[2],
                    identities[3],
                );
                assert!(matches!(
                    prepare_qwen3_gemm_kernel_set_v1(rejected),
                    Err(PrepareQwen3GemmKernelSetErrorV1::SourceBindings)
                ));
                identities = [[0x41; 32], [0x42; 32], [0x43; 32], [0x44; 32]];
            }
        }
        let zero = Qwen3GemmSourceBindingsV1::new([0; 32], [2; 32], [3; 32], [4; 32]);
        assert!(matches!(
            prepare_qwen3_gemm_kernel_set_v1(zero),
            Err(PrepareQwen3GemmKernelSetErrorV1::SourceBindings)
        ));
    }

    #[test]
    fn lowering_is_real_dynamic_typed_llvm_for_both_schedules() {
        let prepared = prepare_qwen3_gemm_kernel_set_v1(bindings(0x61)).unwrap();
        let request = lower_qwen3_gemm_kernel_set_v1(prepared).unwrap();
        for schedule in [
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
        ] {
            let handoff = request.compiler_handoff(schedule);
            let llvm = std::str::from_utf8(handoff.module_bytes()).unwrap();
            assert!(llvm.contains("llvm.amdgcn.workgroup.id.x"));
            assert!(llvm.contains("llvm.amdgcn.workgroup.id.y"));
            assert!(llvm.contains("llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
            assert!(llvm.contains("i32 %m"));
            assert!(llvm.contains("i32 %n"));
            assert!(llvm.contains("i32 %k"));
            assert_ne!(request.source_identity(schedule).as_bytes(), &[0; 32]);
        }
        assert_ne!(
            request.source_identity(GeneralGemmScheduleV1::ReferenceWave64Xor4V1),
            request.source_identity(GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1)
        );
        assert!(!request.grants_artifact_authority());
    }

    #[test]
    fn compiler_products_make_all_runtime_and_hardware_nonclaims() {
        let prepared = prepare_qwen3_gemm_kernel_set_v1(bindings(0x71)).unwrap();
        assert!(!prepared.catalog().grants_authority());
        assert!(!prepared.grants_artifact_authority());
        assert!(!bindings(0x71).authenticates_provenance());
    }
}
