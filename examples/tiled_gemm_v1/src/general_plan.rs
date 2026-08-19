//! Checked specialization and host plan for the conservative general GEMM.
//!
//! This module derives an inert plan. It grants no compiler, artifact, load,
//! or launch authority. The identity commits to the complete problem,
//! schedule, target, numerical contract, derived geometry, and resources so a
//! later compiler stage cannot silently substitute one of those values.

use core::fmt;

use sha2::{Digest, Sha256};

use crate::contract::{AdmittedTargetV1, TILE_K_V1, TILE_M_V1, TILE_N_V1, WAVE_LANES_V1};
use crate::numerical_contract::{GemmSpec, NumericalOperand, SOURCE_SEMANTICS_ID};

/// Canonical wire domain for the inert general GEMM host plan.
pub const GENERAL_GEMM_PLAN_SCHEMA_V1: &str = "fe2o3-general-lds-gemm-plan-v1";
/// Stable identity of the conservative single-buffered XOR4 schedule.
pub const GENERAL_GEMM_REFERENCE_SCHEDULE_V1: &str =
    "gfx942-wave64-m16n16k16-single-buffer-xor4-v1";
/// Two separate `16x16` BF16 operand tiles.
pub const GENERAL_GEMM_LDS_BYTES_V1: u32 = 2 * 16 * 16 * 2;

/// Untrusted host request for `C = alpha * A*B + beta * C`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmRequestV1 {
    m: u32,
    n: u32,
    k: u32,
    lda: u32,
    ldb: u32,
    ldc: u32,
    alpha_bits: u32,
    beta_bits: u32,
}

impl GeneralGemmRequestV1 {
    /// Creates an untrusted row-major GEMM request. Validation occurs in
    /// [`plan_general_gemm_v1`].
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        m: u32,
        n: u32,
        k: u32,
        lda: u32,
        ldb: u32,
        ldc: u32,
        alpha: f32,
        beta: f32,
    ) -> Self {
        Self {
            m,
            n,
            k,
            lda,
            ldb,
            ldc,
            alpha_bits: alpha.to_bits(),
            beta_bits: beta.to_bits(),
        }
    }

    /// Returns `[M, N, K]`.
    pub const fn dimensions(self) -> [u32; 3] {
        [self.m, self.n, self.k]
    }

    /// Returns `[lda, ldb, ldc]` in elements.
    pub const fn strides(self) -> [u32; 3] {
        [self.lda, self.ldb, self.ldc]
    }

    /// Returns the exact FP32 `alpha` bits.
    pub const fn alpha_bits(self) -> u32 {
        self.alpha_bits
    }

    /// Returns the exact FP32 `beta` bits.
    pub const fn beta_bits(self) -> u32 {
        self.beta_bits
    }
}

/// Resource ceilings observed before a plan can leave host preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLaunchLimitsV1 {
    max_grid_x: u32,
    max_grid_y: u32,
    max_workgroups: u64,
    max_buffer_bytes: u64,
    max_workgroup_size: u32,
    max_lds_bytes: u32,
}

/// A launch limit set is malformed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralLaunchLimitErrorV1 {
    /// Every limit must be nonzero.
    Zero(&'static str),
}

impl fmt::Display for GeneralLaunchLimitErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero(field) => write!(formatter, "general GEMM launch limit `{field}` is zero"),
        }
    }
}

impl std::error::Error for GeneralLaunchLimitErrorV1 {}

impl GeneralLaunchLimitsV1 {
    /// Creates explicit nonzero launch and allocation limits.
    pub fn checked(
        max_grid: [u32; 2],
        max_workgroups: u64,
        max_buffer_bytes: u64,
        max_workgroup_size: u32,
        max_lds_bytes: u32,
    ) -> Result<Self, GeneralLaunchLimitErrorV1> {
        for (field, value) in [
            ("max_grid_x", u64::from(max_grid[0])),
            ("max_grid_y", u64::from(max_grid[1])),
            ("max_workgroups", max_workgroups),
            ("max_buffer_bytes", max_buffer_bytes),
            ("max_workgroup_size", u64::from(max_workgroup_size)),
            ("max_lds_bytes", u64::from(max_lds_bytes)),
        ] {
            if value == 0 {
                return Err(GeneralLaunchLimitErrorV1::Zero(field));
            }
        }
        Ok(Self {
            max_grid_x: max_grid[0],
            max_grid_y: max_grid[1],
            max_workgroups,
            max_buffer_bytes,
            max_workgroup_size,
            max_lds_bytes,
        })
    }

    /// Returns the broad representational limits used by generic host tests.
    /// A runtime backend must replace these with observed device limits.
    pub const fn representable() -> Self {
        Self {
            max_grid_x: u32::MAX,
            max_grid_y: u32::MAX,
            max_workgroups: u32::MAX as u64,
            max_buffer_bytes: u64::MAX,
            max_workgroup_size: 1_024,
            max_lds_bytes: 64 * 1_024,
        }
    }

    /// Returns the maximum admitted AQL grid x dimension.
    pub const fn max_grid_x(self) -> u32 {
        self.max_grid_x
    }

    /// Returns the maximum admitted AQL grid y dimension.
    pub const fn max_grid_y(self) -> u32 {
        self.max_grid_y
    }

    /// Returns the maximum admitted workgroup count.
    pub const fn max_workgroups(self) -> u64 {
        self.max_workgroups
    }

    /// Returns the maximum bytes in any one operand allocation.
    pub const fn max_buffer_bytes(self) -> u64 {
        self.max_buffer_bytes
    }

    /// Returns the maximum admitted work-items per workgroup.
    pub const fn max_workgroup_size(self) -> u32 {
        self.max_workgroup_size
    }

    /// Returns the maximum admitted LDS bytes per workgroup.
    pub const fn max_lds_bytes(self) -> u32 {
        self.max_lds_bytes
    }
}

/// Exact checked storage extents for the host-visible matrices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralStorageExtentsV1 {
    elements: [usize; 3],
    bytes: [u64; 3],
}

impl GeneralStorageExtentsV1 {
    /// Returns `[A, B, C]` accessed lengths in elements.
    pub const fn elements(self) -> [usize; 3] {
        self.elements
    }

    /// Returns `[A, B, C]` accessed lengths in bytes.
    pub const fn bytes(self) -> [u64; 3] {
        self.bytes
    }
}

/// Limit category named by a deterministic plan rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralPlanLimitV1 {
    /// AQL grid x work-items.
    GridX,
    /// AQL grid y work-items.
    GridY,
    /// Complete output workgroup count.
    Workgroups,
    /// Per-workgroup thread count.
    WorkgroupSize,
    /// Per-workgroup LDS bytes.
    LdsBytes,
    /// One operand's allocation bytes.
    BufferBytes(NumericalOperand),
}

/// Checked planning failed before any allocation or compiler publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralPlanErrorV1 {
    /// A nonempty matrix row has an undersized stride.
    StrideTooSmall {
        /// Matrix whose stride is invalid.
        operand: NumericalOperand,
        /// Smallest valid stride in elements.
        minimum: u32,
        /// Rejected stride in elements.
        actual: u32,
    },
    /// An accessed storage element extent overflowed the host domain.
    StorageExtentOverflow(NumericalOperand),
    /// An accessed byte extent overflowed `u64`.
    StorageByteCountOverflow(NumericalOperand),
    /// `ceil(N/16) * 64` overflowed the AQL `u32` grid domain.
    GridXOverflow,
    /// A derived value exceeds an explicit host/device limit.
    LimitExceeded {
        /// Limit category.
        limit: GeneralPlanLimitV1,
        /// Derived value.
        actual: u64,
        /// Maximum admitted value.
        maximum: u64,
    },
    /// The shared numerical contract unexpectedly rejected checked values.
    NumericalContractMismatch,
}

impl fmt::Display for GeneralPlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StrideTooSmall {
                operand,
                minimum,
                actual,
            } => write!(
                formatter,
                "{operand} row stride requires at least {minimum} elements, got {actual}"
            ),
            Self::StorageExtentOverflow(operand) => {
                write!(formatter, "{operand} storage extent overflows usize")
            }
            Self::StorageByteCountOverflow(operand) => {
                write!(formatter, "{operand} storage byte count overflows u64")
            }
            Self::GridXOverflow => formatter.write_str("general GEMM AQL grid x overflows u32"),
            Self::LimitExceeded {
                limit,
                actual,
                maximum,
            } => write!(
                formatter,
                "general GEMM {limit:?} value {actual} exceeds limit {maximum}"
            ),
            Self::NumericalContractMismatch => formatter.write_str(
                "general GEMM checked plan disagrees with the shared numerical contract",
            ),
        }
    }
}

impl std::error::Error for GeneralPlanErrorV1 {}

/// SHA-256 commitment to one complete canonical inert plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmPlanIdentityV1([u8; 32]);

impl GeneralGemmPlanIdentityV1 {
    /// Returns the exact commitment bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Privately constructed conservative reference specialization and host plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralGemmPlanV1 {
    target: AdmittedTargetV1,
    request: GeneralGemmRequestV1,
    storage: GeneralStorageExtentsV1,
    block_counts: [u32; 3],
    aql_grid_work_items: [u32; 3],
    reduction_phases: u32,
    total_workgroups: u64,
    identity: GeneralGemmPlanIdentityV1,
}

impl GeneralGemmPlanV1 {
    /// Returns the exact admitted gfx942 target.
    pub const fn target(&self) -> AdmittedTargetV1 {
        self.target
    }

    /// Returns the complete checked host request.
    pub const fn request(&self) -> GeneralGemmRequestV1 {
        self.request
    }

    /// Returns exact accessed element and byte extents.
    pub const fn storage(&self) -> GeneralStorageExtentsV1 {
        self.storage
    }

    /// Returns `[ceil(N/16), ceil(M/16), 1]`.
    pub const fn block_counts(&self) -> [u32; 3] {
        self.block_counts
    }

    /// Returns `[block_count_x * 64, block_count_y, 1]`.
    pub const fn aql_grid_work_items(&self) -> [u32; 3] {
        self.aql_grid_work_items
    }

    /// Returns the fixed wave64 workgroup shape.
    pub const fn workgroup_dimensions(&self) -> [u32; 3] {
        [WAVE_LANES_V1, 1, 1]
    }

    /// Returns `ceil(K/16)`, including zero for `K=0`.
    pub const fn reduction_phases(&self) -> u32 {
        self.reduction_phases
    }

    /// Returns the complete output tile count.
    pub const fn total_workgroups(&self) -> u64 {
        self.total_workgroups
    }

    /// Returns whether this nonempty output requires GPU dispatch.
    pub const fn requires_dispatch(&self) -> bool {
        self.block_counts[0] != 0 && self.block_counts[1] != 0
    }

    /// Returns the exact per-workgroup LDS requirement.
    pub const fn lds_bytes(&self) -> u32 {
        GENERAL_GEMM_LDS_BYTES_V1
    }

    /// Returns the exact plan identity.
    pub const fn identity(&self) -> GeneralGemmPlanIdentityV1 {
        self.identity
    }

    /// Returns the shared numerical spec for a nonempty output.
    /// Empty output intentionally has no operand-storage contract.
    pub fn numerical_spec(&self) -> Option<GemmSpec> {
        if !self.requires_dispatch() {
            return None;
        }
        let [m, n, k] = self.request.dimensions();
        let [lda, ldb, ldc] = self.request.strides();
        GemmSpec::checked(
            m as usize,
            n as usize,
            k as usize,
            lda as usize,
            ldb as usize,
            ldc as usize,
        )
        .ok()
    }

    /// Returns the canonical identity preimage.
    pub fn encode_canonical(&self) -> Vec<u8> {
        canonical_plan_bytes(
            self.target,
            self.request,
            self.storage,
            self.block_counts,
            self.aql_grid_work_items,
            self.reduction_phases,
            self.total_workgroups,
        )
    }
}

fn ceil_div_16(value: u32) -> u32 {
    value / 16 + u32::from(!value.is_multiple_of(16))
}

fn checked_extent(
    operand: NumericalOperand,
    rows: u32,
    columns: u32,
    stride: u32,
) -> Result<u64, GeneralPlanErrorV1> {
    if rows == 0 || columns == 0 {
        return Ok(0);
    }
    if stride < columns {
        return Err(GeneralPlanErrorV1::StrideTooSmall {
            operand,
            minimum: columns,
            actual: stride,
        });
    }
    u64::from(rows - 1)
        .checked_mul(u64::from(stride))
        .and_then(|prefix| prefix.checked_add(u64::from(columns)))
        .ok_or(GeneralPlanErrorV1::StorageExtentOverflow(operand))
}

fn check_limit(
    limit: GeneralPlanLimitV1,
    actual: u64,
    maximum: u64,
) -> Result<(), GeneralPlanErrorV1> {
    if actual > maximum {
        return Err(GeneralPlanErrorV1::LimitExceeded {
            limit,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn canonical_plan_bytes(
    target: AdmittedTargetV1,
    request: GeneralGemmRequestV1,
    storage: GeneralStorageExtentsV1,
    block_counts: [u32; 3],
    aql_grid_work_items: [u32; 3],
    reduction_phases: u32,
    total_workgroups: u64,
) -> Vec<u8> {
    fn text(output: &mut Vec<u8>, value: &str) {
        output.extend_from_slice(&(value.len() as u32).to_le_bytes());
        output.extend_from_slice(value.as_bytes());
    }
    fn u32s(output: &mut Vec<u8>, values: impl IntoIterator<Item = u32>) {
        for value in values {
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
    fn u64s(output: &mut Vec<u8>, values: impl IntoIterator<Item = u64>) {
        for value in values {
            output.extend_from_slice(&value.to_le_bytes());
        }
    }

    let mut output = Vec::with_capacity(256);
    text(&mut output, GENERAL_GEMM_PLAN_SCHEMA_V1);
    text(&mut output, &target.target_id().to_string());
    text(&mut output, GENERAL_GEMM_REFERENCE_SCHEDULE_V1);
    text(&mut output, SOURCE_SEMANTICS_ID);
    u32s(&mut output, request.dimensions());
    u32s(&mut output, request.strides());
    u32s(&mut output, [request.alpha_bits(), request.beta_bits()]);
    u64s(&mut output, storage.elements.map(|value| value as u64));
    u64s(&mut output, storage.bytes);
    u32s(&mut output, block_counts);
    u32s(&mut output, aql_grid_work_items);
    u32s(
        &mut output,
        [
            WAVE_LANES_V1,
            TILE_M_V1,
            TILE_N_V1,
            TILE_K_V1,
            reduction_phases,
            GENERAL_GEMM_LDS_BYTES_V1,
        ],
    );
    u64s(&mut output, [total_workgroups]);
    output.push(u8::from(block_counts[0] != 0 && block_counts[1] != 0));
    output
}

/// Checks a general GEMM request and derives its complete inert plan.
pub fn plan_general_gemm_v1(
    target: AdmittedTargetV1,
    request: GeneralGemmRequestV1,
    limits: GeneralLaunchLimitsV1,
) -> Result<GeneralGemmPlanV1, GeneralPlanErrorV1> {
    let [m, n, k] = request.dimensions();
    let [lda, ldb, ldc] = request.strides();

    let empty_output = m == 0 || n == 0;
    if !empty_output {
        check_limit(
            GeneralPlanLimitV1::WorkgroupSize,
            u64::from(WAVE_LANES_V1),
            u64::from(limits.max_workgroup_size),
        )?;
        check_limit(
            GeneralPlanLimitV1::LdsBytes,
            u64::from(GENERAL_GEMM_LDS_BYTES_V1),
            u64::from(limits.max_lds_bytes),
        )?;
    }
    let element_extents_u64 = if empty_output {
        [0, 0, 0]
    } else {
        [
            checked_extent(NumericalOperand::A, m, k, lda)?,
            checked_extent(NumericalOperand::B, k, n, ldb)?,
            checked_extent(NumericalOperand::C, m, n, ldc)?,
        ]
    };
    let operands = [
        NumericalOperand::A,
        NumericalOperand::B,
        NumericalOperand::C,
    ];
    let element_bytes = [2_u64, 2, 4];
    let mut elements = [0_usize; 3];
    let mut bytes = [0_u64; 3];
    for index in 0..3 {
        elements[index] = usize::try_from(element_extents_u64[index])
            .map_err(|_| GeneralPlanErrorV1::StorageExtentOverflow(operands[index]))?;
        bytes[index] = element_extents_u64[index]
            .checked_mul(element_bytes[index])
            .ok_or(GeneralPlanErrorV1::StorageByteCountOverflow(
                operands[index],
            ))?;
        check_limit(
            GeneralPlanLimitV1::BufferBytes(operands[index]),
            bytes[index],
            limits.max_buffer_bytes,
        )?;
    }
    let storage = GeneralStorageExtentsV1 { elements, bytes };

    let (block_counts, aql_grid_work_items, reduction_phases, total_workgroups) = if empty_output {
        ([0, 0, 0], [0, 0, 0], 0, 0)
    } else {
        let tile_columns = ceil_div_16(n);
        let tile_rows = ceil_div_16(m);
        let reduction_phases = ceil_div_16(k);
        let grid_x = tile_columns
            .checked_mul(WAVE_LANES_V1)
            .ok_or(GeneralPlanErrorV1::GridXOverflow)?;
        let total_workgroups = u64::from(tile_columns) * u64::from(tile_rows);
        check_limit(
            GeneralPlanLimitV1::GridX,
            u64::from(grid_x),
            u64::from(limits.max_grid_x),
        )?;
        check_limit(
            GeneralPlanLimitV1::GridY,
            u64::from(tile_rows),
            u64::from(limits.max_grid_y),
        )?;
        check_limit(
            GeneralPlanLimitV1::Workgroups,
            total_workgroups,
            limits.max_workgroups,
        )?;
        (
            [tile_columns, tile_rows, 1],
            [grid_x, tile_rows, 1],
            reduction_phases,
            total_workgroups,
        )
    };

    if !empty_output
        && GemmSpec::checked(
            m as usize,
            n as usize,
            k as usize,
            lda as usize,
            ldb as usize,
            ldc as usize,
        )
        .is_err()
    {
        return Err(GeneralPlanErrorV1::NumericalContractMismatch);
    }

    let canonical = canonical_plan_bytes(
        target,
        request,
        storage,
        block_counts,
        aql_grid_work_items,
        reduction_phases,
        total_workgroups,
    );
    let identity = GeneralGemmPlanIdentityV1(Sha256::digest(canonical).into());
    Ok(GeneralGemmPlanV1 {
        target,
        request,
        storage,
        block_counts,
        aql_grid_work_items,
        reduction_phases,
        total_workgroups,
        identity,
    })
}
