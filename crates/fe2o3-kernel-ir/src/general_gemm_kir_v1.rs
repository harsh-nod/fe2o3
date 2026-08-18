//! Structured semantic IR for the conservative general tiled GEMM schedule.
//!
//! This model is deliberately independent of source text and of the frozen
//! `Module` wire encodings. A frontend can translate compiler-owned semantics
//! into these regions and events, then run the verifier before requesting a
//! proof. Successful verification is not a proof report, artifact authority,
//! or permission to lower or launch a kernel.

use core::fmt;

use sha2::{Digest, Sha256};

/// Schema identity for the first structured general GEMM semantic model.
pub const GENERAL_GEMM_KIR_SCHEMA_V1: &str = "fe2o3-general-gemm-kir-v1";
/// Fixed output and reduction tile extent.
pub const GENERAL_GEMM_KIR_TILE_EXTENT_V1: u32 = 16;
/// Fixed wave size admitted by this schedule.
pub const GENERAL_GEMM_KIR_WAVE_LANES_V1: u32 = 64;
/// Number of accumulator and staging components owned by each lane.
pub const GENERAL_GEMM_KIR_COMPONENTS_PER_LANE_V1: u32 = 4;
/// Elements in either single-buffered LDS tile.
pub const GENERAL_GEMM_KIR_LDS_ELEMENTS_V1: u64 = 256;
/// Hard bound on events in the symbolic phase body.
pub const MAX_GENERAL_GEMM_PHASE_EVENTS_V1: usize = 32;
const GENERAL_GEMM_KIR_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-kir-identity-v1\0";

/// Untrusted projection of the fields produced by the checked host planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmPlanSnapshotV1 {
    /// `[M, N, K]`.
    pub dimensions: [u32; 3],
    /// Row-major `[lda, ldb, ldc]` in elements.
    pub strides: [u32; 3],
    /// Exact accessed `[A, B, C]` allocation extents in elements.
    pub storage_elements: [u64; 3],
    /// `[ceil(N/16), ceil(M/16), 1]`, or zeros for an empty output.
    pub block_counts: [u32; 3],
    /// `[block_count_x * 64, block_count_y, 1]`, or zeros for an empty output.
    pub aql_grid_work_items: [u32; 3],
    /// `ceil(K/16)`, or zero for an empty output.
    pub reduction_phases: u32,
    /// Exact runtime `alpha` bits.
    pub alpha_bits: u32,
    /// Exact runtime `beta` bits.
    pub beta_bits: u32,
}

/// A checked planner projection is inconsistent with the general schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmPlanSnapshotErrorV1 {
    /// A nonempty logical row has an undersized row stride.
    StrideTooSmall {
        /// A, B, or C.
        operand: GemmOperandV1,
        /// Smallest admitted stride.
        minimum: u32,
        /// Rejected stride.
        actual: u32,
    },
    /// Computing a derived extent overflowed the semantic model.
    DerivedExtentOverflow,
    /// A planner-derived field differs from its independently derived value.
    FieldMismatch {
        /// Frozen field spelling.
        field: &'static str,
    },
}

impl fmt::Display for GeneralGemmPlanSnapshotErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StrideTooSmall {
                operand,
                minimum,
                actual,
            } => write!(
                formatter,
                "general GEMM {operand:?} stride requires at least {minimum}, got {actual}"
            ),
            Self::DerivedExtentOverflow => {
                formatter.write_str("general GEMM derived plan extent overflowed")
            }
            Self::FieldMismatch { field } => {
                write!(
                    formatter,
                    "general GEMM planner field `{field}` is inconsistent"
                )
            }
        }
    }
}

impl std::error::Error for GeneralGemmPlanSnapshotErrorV1 {}

/// Live extents of the last M, N, and K tiles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmTailShapeV1 {
    /// Live rows in the last output tile, from zero through sixteen.
    pub m: u8,
    /// Live columns in the last output tile, from zero through sixteen.
    pub n: u8,
    /// Live depths in the last reduction tile, from zero through sixteen.
    pub k: u8,
}

/// Independently checked fields consumed by the semantic KIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmPlanFieldsV1 {
    snapshot: GeneralGemmPlanSnapshotV1,
    tails: GeneralGemmTailShapeV1,
}

impl GeneralGemmPlanFieldsV1 {
    /// Checks every derived geometry and storage field before admitting it.
    pub fn checked(
        snapshot: GeneralGemmPlanSnapshotV1,
    ) -> Result<Self, GeneralGemmPlanSnapshotErrorV1> {
        let [m, n, k] = snapshot.dimensions;
        let [lda, ldb, ldc] = snapshot.strides;
        let empty_output = m == 0 || n == 0;

        let expected_storage = if empty_output {
            [0, 0, 0]
        } else {
            [
                row_major_extent(GemmOperandV1::A, m, k, lda)?,
                row_major_extent(GemmOperandV1::B, k, n, ldb)?,
                row_major_extent(GemmOperandV1::C, m, n, ldc)?,
            ]
        };
        if snapshot.storage_elements != expected_storage {
            return Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
                field: "storage_elements",
            });
        }

        let (block_counts, grid, phases) = if empty_output {
            ([0, 0, 0], [0, 0, 0], 0)
        } else {
            let block_x = ceil_div_16(n);
            let block_y = ceil_div_16(m);
            let grid_x = block_x
                .checked_mul(GENERAL_GEMM_KIR_WAVE_LANES_V1)
                .ok_or(GeneralGemmPlanSnapshotErrorV1::DerivedExtentOverflow)?;
            ([block_x, block_y, 1], [grid_x, block_y, 1], ceil_div_16(k))
        };
        if snapshot.block_counts != block_counts {
            return Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
                field: "block_counts",
            });
        }
        if snapshot.aql_grid_work_items != grid {
            return Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
                field: "aql_grid_work_items",
            });
        }
        if snapshot.reduction_phases != phases {
            return Err(GeneralGemmPlanSnapshotErrorV1::FieldMismatch {
                field: "reduction_phases",
            });
        }

        Ok(Self {
            snapshot,
            tails: GeneralGemmTailShapeV1 {
                m: last_tile_extent(m),
                n: last_tile_extent(n),
                k: last_tile_extent(k),
            },
        })
    }

    /// Returns `[M, N, K]`.
    pub const fn dimensions(self) -> [u32; 3] {
        self.snapshot.dimensions
    }

    /// Returns row-major `[lda, ldb, ldc]`.
    pub const fn strides(self) -> [u32; 3] {
        self.snapshot.strides
    }

    /// Returns exact accessed `[A, B, C]` extents.
    pub const fn storage_elements(self) -> [u64; 3] {
        self.snapshot.storage_elements
    }

    /// Returns checked output tile counts.
    pub const fn block_counts(self) -> [u32; 3] {
        self.snapshot.block_counts
    }

    /// Returns checked AQL grid work-item counts.
    pub const fn aql_grid_work_items(self) -> [u32; 3] {
        self.snapshot.aql_grid_work_items
    }

    /// Returns the checked reduction phase count.
    pub const fn reduction_phases(self) -> u32 {
        self.snapshot.reduction_phases
    }

    /// Returns exact runtime `alpha` bits.
    pub const fn alpha_bits(self) -> u32 {
        self.snapshot.alpha_bits
    }

    /// Returns exact runtime `beta` bits.
    pub const fn beta_bits(self) -> u32 {
        self.snapshot.beta_bits
    }

    /// Returns live extents in the last logical tiles.
    pub const fn tails(self) -> GeneralGemmTailShapeV1 {
        self.tails
    }

    /// Returns whether a GPU dispatch is required.
    pub const fn requires_dispatch(self) -> bool {
        self.snapshot.block_counts[0] != 0 && self.snapshot.block_counts[1] != 0
    }
}

const fn ceil_div_16(value: u32) -> u32 {
    value / 16 + if value.is_multiple_of(16) { 0 } else { 1 }
}

const fn last_tile_extent(value: u32) -> u8 {
    if value == 0 {
        0
    } else {
        (((value - 1) % GENERAL_GEMM_KIR_TILE_EXTENT_V1) + 1) as u8
    }
}

fn row_major_extent(
    operand: GemmOperandV1,
    rows: u32,
    columns: u32,
    stride: u32,
) -> Result<u64, GeneralGemmPlanSnapshotErrorV1> {
    if rows == 0 || columns == 0 {
        return Ok(0);
    }
    if stride < columns {
        return Err(GeneralGemmPlanSnapshotErrorV1::StrideTooSmall {
            operand,
            minimum: columns,
            actual: stride,
        });
    }
    u64::from(rows - 1)
        .checked_mul(u64::from(stride))
        .and_then(|prefix| prefix.checked_add(u64::from(columns)))
        .ok_or(GeneralGemmPlanSnapshotErrorV1::DerivedExtentOverflow)
}

/// Logical GEMM operand.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GemmOperandV1 {
    /// Left BF16 operand.
    A,
    /// Right BF16 operand.
    B,
    /// FP32 input/output operand.
    C,
}

/// Addressable region in the structured schedule.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmRegionV1 {
    /// Host-bound A allocation.
    GlobalA,
    /// Host-bound B allocation.
    GlobalB,
    /// Host-bound C allocation.
    GlobalC,
    /// Workgroup-local A tile.
    LdsA,
    /// Workgroup-local B tile.
    LdsB,
}

/// Element representation of a schedule region.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmElementV1 {
    /// IEEE BF16 bits.
    Bf16,
    /// IEEE FP32 bits.
    F32,
}

/// Scope of a region.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmRegionScopeV1 {
    /// One runtime allocation shared by the dispatch.
    Dispatch,
    /// One allocation private to a workgroup.
    Workgroup,
}

/// Region descriptor derived only from checked plan fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmRegionDescriptorV1 {
    /// Region identity.
    pub region: GeneralGemmRegionV1,
    /// Element representation.
    pub element: GeneralGemmElementV1,
    /// Allocation scope.
    pub scope: GeneralGemmRegionScopeV1,
    /// Exact accessed or allocated element count.
    pub elements: u64,
    /// Row stride for global matrices; sixteen for XOR4 LDS tiles.
    pub row_stride: u32,
}

/// Exact predicate attached to a global access family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmBoundsGuardV1 {
    /// `row < M && depth < K`.
    ARowAndDepth,
    /// `depth < K && column < N`.
    BDepthAndColumn,
    /// `row < M && column < N`.
    CRowAndColumn,
    /// No predicate protects the access.
    Unguarded,
}

/// Physical mapping from wave64 lane/components to one LDS tile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmLdsMappingV1 {
    /// The schedule's injective `16x16` row-major XOR4 mapping.
    Wave64Xor4,
    /// Two or more writers name the same physical cell in an epoch.
    Aliased,
}

/// Static completeness of a phase's LDS writes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmWriteCoverageV1 {
    /// Every one of the 256 physical cells is written.
    Complete,
    /// At least one subsequently readable cell is not written.
    Incomplete,
}

/// Value staged for an out-of-domain K-tail coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmTailValueV1 {
    /// Exact BF16 zero.
    Zero,
    /// A nonzero or source-derived tail value.
    NonZero,
}

/// Completion state of one stage operation when it is issued.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmStageCompletionV1 {
    /// Synchronous staging is ready when the event completes.
    Ready,
    /// Asynchronous staging requires a later matching wait.
    PendingAsync,
}

/// A complete global-to-LDS staging family for one operand and phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmStageEventV1 {
    /// A or B.
    pub operand: GemmOperandV1,
    /// Global source region.
    pub source: GeneralGemmRegionV1,
    /// LDS destination region.
    pub destination: GeneralGemmRegionV1,
    /// Exact global tail predicate.
    pub guard: GeneralGemmBoundsGuardV1,
    /// Physical wave-to-LDS mapping.
    pub mapping: GeneralGemmLdsMappingV1,
    /// Whether all cells are initialized.
    pub coverage: GeneralGemmWriteCoverageV1,
    /// Value written for a K-tail slot.
    pub tail_value: GeneralGemmTailValueV1,
    /// Readiness after this event.
    pub completion: GeneralGemmStageCompletionV1,
}

/// A wait that completes asynchronous staging for one operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmAsyncWaitEventV1 {
    /// A or B.
    pub operand: GemmOperandV1,
}

/// Workgroup synchronization role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmBarrierRoleV1 {
    /// Publishes complete LDS writes before reads.
    Publish,
    /// Orders all reads before the next phase overwrites LDS.
    Reuse,
}

/// Participation domain for a workgroup barrier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmBarrierParticipationV1 {
    /// All 64 lanes execute the barrier in every active phase.
    UniformWave64,
    /// A lane-varying predicate controls barrier execution.
    LaneConditional,
}

/// One workgroup barrier in the phase loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmBarrierEventV1 {
    /// Publish or reuse role.
    pub role: GeneralGemmBarrierRoleV1,
    /// Static participation domain.
    pub participation: GeneralGemmBarrierParticipationV1,
}

/// Phase epoch named by an LDS access.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmPhaseEpochV1 {
    /// The current reduction phase.
    Current,
    /// An expired preceding reduction phase.
    Previous,
}

/// A complete MFMA input read family from one LDS tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmLdsReadEventV1 {
    /// A or B.
    pub operand: GemmOperandV1,
    /// Region being consumed.
    pub source: GeneralGemmRegionV1,
    /// Epoch being named.
    pub epoch: GeneralGemmPhaseEpochV1,
}

/// Accumulator value entering a phase update.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmAccumulatorInputV1 {
    /// The exact accumulator produced by the preceding phase, or zero at phase zero.
    Carried,
    /// The accumulator is reset before a later phase contribution.
    Reset,
}

/// One complete `m16n16k16` phase update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmAccumulateEventV1 {
    /// Accumulator flow across the symbolic phase loop.
    pub input: GeneralGemmAccumulatorInputV1,
}

/// Event in the symbolic body repeated for every checked K phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmPhaseEventV1 {
    /// Global-to-LDS writes.
    Stage(GeneralGemmStageEventV1),
    /// Completion of a prior asynchronous stage.
    AsyncWait(GeneralGemmAsyncWaitEventV1),
    /// Workgroup synchronization.
    Barrier(GeneralGemmBarrierEventV1),
    /// MFMA reads from LDS.
    LdsRead(GeneralGemmLdsReadEventV1),
    /// FP32 accumulator update.
    Accumulate(GeneralGemmAccumulateEventV1),
}

/// Per-lane output coordinate mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmLaneOutputMappingV1 {
    /// `(4*(lane/16)+component, lane%16)`.
    Wave64FourRows,
    /// Multiple lane/components own the same logical output.
    Aliased,
}

/// Workgroup-to-output-tile mapping.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmWorkgroupOutputMappingV1 {
    /// `(group_y*16, group_x*16)`.
    GridXY16,
    /// Multiple workgroups own the same logical output tile.
    Overlapping,
}

/// Algebra computed by the output event.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmEpilogueExpressionV1 {
    /// `alpha * accumulator + beta * initial_c` using the plan's exact bits.
    AlphaAccumulatorPlusBetaC,
    /// Any other expression or coefficient association.
    Other,
}

/// Complete C load/store family after the reduction loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmEpilogueEventV1 {
    /// Predicate on the initial C load.
    pub load_guard: GeneralGemmBoundsGuardV1,
    /// Predicate on the final C store.
    pub store_guard: GeneralGemmBoundsGuardV1,
    /// Per-lane/component ownership.
    pub lane_mapping: GeneralGemmLaneOutputMappingV1,
    /// Per-workgroup ownership.
    pub workgroup_mapping: GeneralGemmWorkgroupOutputMappingV1,
    /// Exact alpha/beta algebra.
    pub expression: GeneralGemmEpilogueExpressionV1,
}

/// Structured symbolic schedule for arbitrary checked M/N/K and row strides.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralGemmKirV1 {
    plan: GeneralGemmPlanFieldsV1,
    regions: [GeneralGemmRegionDescriptorV1; 5],
    phase_events: Vec<GeneralGemmPhaseEventV1>,
    epilogue: GeneralGemmEpilogueEventV1,
}

impl GeneralGemmKirV1 {
    /// Constructs the canonical single-buffered wave64/XOR4 event schedule.
    pub fn canonical(plan: GeneralGemmPlanFieldsV1) -> Self {
        let phase_events = vec![
            GeneralGemmPhaseEventV1::Stage(canonical_stage(GemmOperandV1::A)),
            GeneralGemmPhaseEventV1::Stage(canonical_stage(GemmOperandV1::B)),
            GeneralGemmPhaseEventV1::Barrier(GeneralGemmBarrierEventV1 {
                role: GeneralGemmBarrierRoleV1::Publish,
                participation: GeneralGemmBarrierParticipationV1::UniformWave64,
            }),
            GeneralGemmPhaseEventV1::LdsRead(canonical_read(GemmOperandV1::A)),
            GeneralGemmPhaseEventV1::LdsRead(canonical_read(GemmOperandV1::B)),
            GeneralGemmPhaseEventV1::Accumulate(GeneralGemmAccumulateEventV1 {
                input: GeneralGemmAccumulatorInputV1::Carried,
            }),
            GeneralGemmPhaseEventV1::Barrier(GeneralGemmBarrierEventV1 {
                role: GeneralGemmBarrierRoleV1::Reuse,
                participation: GeneralGemmBarrierParticipationV1::UniformWave64,
            }),
        ];
        Self::checked_from_parts(
            plan,
            phase_events,
            GeneralGemmEpilogueEventV1 {
                load_guard: GeneralGemmBoundsGuardV1::CRowAndColumn,
                store_guard: GeneralGemmBoundsGuardV1::CRowAndColumn,
                lane_mapping: GeneralGemmLaneOutputMappingV1::Wave64FourRows,
                workgroup_mapping: GeneralGemmWorkgroupOutputMappingV1::GridXY16,
                expression: GeneralGemmEpilogueExpressionV1::AlphaAccumulatorPlusBetaC,
            },
        )
        .expect("canonical phase event count is bounded")
    }

    /// Constructs a bounded, unverified event graph for compiler-owned
    /// translation.
    pub fn checked_from_parts(
        plan: GeneralGemmPlanFieldsV1,
        phase_events: Vec<GeneralGemmPhaseEventV1>,
        epilogue: GeneralGemmEpilogueEventV1,
    ) -> Result<Self, GeneralGemmKirBuildErrorV1> {
        if phase_events.len() > MAX_GENERAL_GEMM_PHASE_EVENTS_V1 {
            return Err(GeneralGemmKirBuildErrorV1::TooManyPhaseEvents {
                actual: phase_events.len(),
                maximum: MAX_GENERAL_GEMM_PHASE_EVENTS_V1,
            });
        }
        let [lda, ldb, ldc] = plan.strides();
        let [a_elements, b_elements, c_elements] = plan.storage_elements();
        Ok(Self {
            plan,
            regions: [
                region(
                    GeneralGemmRegionV1::GlobalA,
                    GeneralGemmElementV1::Bf16,
                    GeneralGemmRegionScopeV1::Dispatch,
                    a_elements,
                    lda,
                ),
                region(
                    GeneralGemmRegionV1::GlobalB,
                    GeneralGemmElementV1::Bf16,
                    GeneralGemmRegionScopeV1::Dispatch,
                    b_elements,
                    ldb,
                ),
                region(
                    GeneralGemmRegionV1::GlobalC,
                    GeneralGemmElementV1::F32,
                    GeneralGemmRegionScopeV1::Dispatch,
                    c_elements,
                    ldc,
                ),
                region(
                    GeneralGemmRegionV1::LdsA,
                    GeneralGemmElementV1::Bf16,
                    GeneralGemmRegionScopeV1::Workgroup,
                    GENERAL_GEMM_KIR_LDS_ELEMENTS_V1,
                    GENERAL_GEMM_KIR_TILE_EXTENT_V1,
                ),
                region(
                    GeneralGemmRegionV1::LdsB,
                    GeneralGemmElementV1::Bf16,
                    GeneralGemmRegionScopeV1::Workgroup,
                    GENERAL_GEMM_KIR_LDS_ELEMENTS_V1,
                    GENERAL_GEMM_KIR_TILE_EXTENT_V1,
                ),
            ],
            phase_events,
            epilogue,
        })
    }

    /// Returns the checked plan projection.
    pub const fn plan(&self) -> GeneralGemmPlanFieldsV1 {
        self.plan
    }

    /// Returns the exact derived region table.
    pub const fn regions(&self) -> &[GeneralGemmRegionDescriptorV1; 5] {
        &self.regions
    }

    /// Returns the symbolic phase body.
    pub fn phase_events(&self) -> &[GeneralGemmPhaseEventV1] {
        &self.phase_events
    }

    /// Returns the output event.
    pub const fn epilogue(&self) -> &GeneralGemmEpilogueEventV1 {
        &self.epilogue
    }

    /// Returns the deterministic canonical encoding of every semantic field.
    pub fn encode_canonical(&self) -> Vec<u8> {
        encode_general_gemm_kir_canonical_v1(self)
    }

    /// Returns the domain-separated SHA-256 semantic graph identity.
    pub fn identity(&self) -> GeneralGemmKirIdentityV1 {
        general_gemm_kir_identity_v1(self)
    }
}

/// A structured program exceeded a deterministic verifier resource bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmKirBuildErrorV1 {
    /// The symbolic phase body contains too many events.
    TooManyPhaseEvents {
        /// Submitted event count.
        actual: usize,
        /// Maximum admitted event count.
        maximum: usize,
    },
}

impl fmt::Display for GeneralGemmKirBuildErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyPhaseEvents { actual, maximum } => write!(
                formatter,
                "general GEMM phase contains {actual} events, maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for GeneralGemmKirBuildErrorV1 {}

/// SHA-256 commitment to one complete bounded general GEMM semantic graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneralGemmKirIdentityV1([u8; 32]);

impl GeneralGemmKirIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Encodes every checked plan, derived region, ordered event, and epilogue
/// field under the V1 schema domain.
pub fn encode_general_gemm_kir_canonical_v1(kir: &GeneralGemmKirV1) -> Vec<u8> {
    fn text(output: &mut Vec<u8>, value: &str) {
        u32_value(output, value.len() as u32);
        output.extend_from_slice(value.as_bytes());
    }

    fn u32_value(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn u64_value(output: &mut Vec<u8>, value: u64) {
        output.extend_from_slice(&value.to_le_bytes());
    }

    let mut output = Vec::with_capacity(384);
    text(&mut output, GENERAL_GEMM_KIR_SCHEMA_V1);

    for value in kir.plan.dimensions() {
        u32_value(&mut output, value);
    }
    for value in kir.plan.strides() {
        u32_value(&mut output, value);
    }
    for value in kir.plan.storage_elements() {
        u64_value(&mut output, value);
    }
    for value in kir.plan.block_counts() {
        u32_value(&mut output, value);
    }
    for value in kir.plan.aql_grid_work_items() {
        u32_value(&mut output, value);
    }
    u32_value(&mut output, kir.plan.reduction_phases());
    u32_value(&mut output, kir.plan.alpha_bits());
    u32_value(&mut output, kir.plan.beta_bits());
    let tails = kir.plan.tails();
    output.extend_from_slice(&[tails.m, tails.n, tails.k]);
    output.push(u8::from(kir.plan.requires_dispatch()));

    output.push(kir.regions.len() as u8);
    for descriptor in kir.regions {
        output.push(region_tag(descriptor.region));
        output.push(element_tag(descriptor.element));
        output.push(scope_tag(descriptor.scope));
        u64_value(&mut output, descriptor.elements);
        u32_value(&mut output, descriptor.row_stride);
    }

    output.push(kir.phase_events.len() as u8);
    for event in &kir.phase_events {
        match *event {
            GeneralGemmPhaseEventV1::Stage(stage) => {
                output.push(1);
                output.push(operand_tag(stage.operand));
                output.push(region_tag(stage.source));
                output.push(region_tag(stage.destination));
                output.push(guard_tag(stage.guard));
                output.push(lds_mapping_tag(stage.mapping));
                output.push(coverage_tag(stage.coverage));
                output.push(tail_value_tag(stage.tail_value));
                output.push(stage_completion_tag(stage.completion));
            }
            GeneralGemmPhaseEventV1::AsyncWait(wait) => {
                output.extend_from_slice(&[2, operand_tag(wait.operand)]);
            }
            GeneralGemmPhaseEventV1::Barrier(barrier) => {
                output.extend_from_slice(&[
                    3,
                    barrier_role_tag(barrier.role),
                    barrier_participation_tag(barrier.participation),
                ]);
            }
            GeneralGemmPhaseEventV1::LdsRead(read) => {
                output.extend_from_slice(&[
                    4,
                    operand_tag(read.operand),
                    region_tag(read.source),
                    epoch_tag(read.epoch),
                ]);
            }
            GeneralGemmPhaseEventV1::Accumulate(accumulate) => {
                output.extend_from_slice(&[5, accumulator_input_tag(accumulate.input)]);
            }
        }
    }

    output.extend_from_slice(&[
        guard_tag(kir.epilogue.load_guard),
        guard_tag(kir.epilogue.store_guard),
        lane_output_tag(kir.epilogue.lane_mapping),
        workgroup_output_tag(kir.epilogue.workgroup_mapping),
        epilogue_expression_tag(kir.epilogue.expression),
    ]);
    output
}

/// Computes the domain-separated identity of a bounded general GEMM KIR.
pub fn general_gemm_kir_identity_v1(kir: &GeneralGemmKirV1) -> GeneralGemmKirIdentityV1 {
    let canonical = encode_general_gemm_kir_canonical_v1(kir);
    let mut hasher = Sha256::new();
    hasher.update(GENERAL_GEMM_KIR_IDENTITY_DOMAIN_V1);
    hasher.update((canonical.len() as u64).to_le_bytes());
    hasher.update(canonical);
    GeneralGemmKirIdentityV1(hasher.finalize().into())
}

const fn operand_tag(value: GemmOperandV1) -> u8 {
    match value {
        GemmOperandV1::A => 1,
        GemmOperandV1::B => 2,
        GemmOperandV1::C => 3,
    }
}

const fn region_tag(value: GeneralGemmRegionV1) -> u8 {
    match value {
        GeneralGemmRegionV1::GlobalA => 1,
        GeneralGemmRegionV1::GlobalB => 2,
        GeneralGemmRegionV1::GlobalC => 3,
        GeneralGemmRegionV1::LdsA => 4,
        GeneralGemmRegionV1::LdsB => 5,
    }
}

const fn element_tag(value: GeneralGemmElementV1) -> u8 {
    match value {
        GeneralGemmElementV1::Bf16 => 1,
        GeneralGemmElementV1::F32 => 2,
    }
}

const fn scope_tag(value: GeneralGemmRegionScopeV1) -> u8 {
    match value {
        GeneralGemmRegionScopeV1::Dispatch => 1,
        GeneralGemmRegionScopeV1::Workgroup => 2,
    }
}

const fn guard_tag(value: GeneralGemmBoundsGuardV1) -> u8 {
    match value {
        GeneralGemmBoundsGuardV1::ARowAndDepth => 1,
        GeneralGemmBoundsGuardV1::BDepthAndColumn => 2,
        GeneralGemmBoundsGuardV1::CRowAndColumn => 3,
        GeneralGemmBoundsGuardV1::Unguarded => 4,
    }
}

const fn lds_mapping_tag(value: GeneralGemmLdsMappingV1) -> u8 {
    match value {
        GeneralGemmLdsMappingV1::Wave64Xor4 => 1,
        GeneralGemmLdsMappingV1::Aliased => 2,
    }
}

const fn coverage_tag(value: GeneralGemmWriteCoverageV1) -> u8 {
    match value {
        GeneralGemmWriteCoverageV1::Complete => 1,
        GeneralGemmWriteCoverageV1::Incomplete => 2,
    }
}

const fn tail_value_tag(value: GeneralGemmTailValueV1) -> u8 {
    match value {
        GeneralGemmTailValueV1::Zero => 1,
        GeneralGemmTailValueV1::NonZero => 2,
    }
}

const fn stage_completion_tag(value: GeneralGemmStageCompletionV1) -> u8 {
    match value {
        GeneralGemmStageCompletionV1::Ready => 1,
        GeneralGemmStageCompletionV1::PendingAsync => 2,
    }
}

const fn barrier_role_tag(value: GeneralGemmBarrierRoleV1) -> u8 {
    match value {
        GeneralGemmBarrierRoleV1::Publish => 1,
        GeneralGemmBarrierRoleV1::Reuse => 2,
    }
}

const fn barrier_participation_tag(value: GeneralGemmBarrierParticipationV1) -> u8 {
    match value {
        GeneralGemmBarrierParticipationV1::UniformWave64 => 1,
        GeneralGemmBarrierParticipationV1::LaneConditional => 2,
    }
}

const fn epoch_tag(value: GeneralGemmPhaseEpochV1) -> u8 {
    match value {
        GeneralGemmPhaseEpochV1::Current => 1,
        GeneralGemmPhaseEpochV1::Previous => 2,
    }
}

const fn accumulator_input_tag(value: GeneralGemmAccumulatorInputV1) -> u8 {
    match value {
        GeneralGemmAccumulatorInputV1::Carried => 1,
        GeneralGemmAccumulatorInputV1::Reset => 2,
    }
}

const fn lane_output_tag(value: GeneralGemmLaneOutputMappingV1) -> u8 {
    match value {
        GeneralGemmLaneOutputMappingV1::Wave64FourRows => 1,
        GeneralGemmLaneOutputMappingV1::Aliased => 2,
    }
}

const fn workgroup_output_tag(value: GeneralGemmWorkgroupOutputMappingV1) -> u8 {
    match value {
        GeneralGemmWorkgroupOutputMappingV1::GridXY16 => 1,
        GeneralGemmWorkgroupOutputMappingV1::Overlapping => 2,
    }
}

const fn epilogue_expression_tag(value: GeneralGemmEpilogueExpressionV1) -> u8 {
    match value {
        GeneralGemmEpilogueExpressionV1::AlphaAccumulatorPlusBetaC => 1,
        GeneralGemmEpilogueExpressionV1::Other => 2,
    }
}

const fn region(
    region: GeneralGemmRegionV1,
    element: GeneralGemmElementV1,
    scope: GeneralGemmRegionScopeV1,
    elements: u64,
    row_stride: u32,
) -> GeneralGemmRegionDescriptorV1 {
    GeneralGemmRegionDescriptorV1 {
        region,
        element,
        scope,
        elements,
        row_stride,
    }
}

const fn canonical_stage(operand: GemmOperandV1) -> GeneralGemmStageEventV1 {
    let (source, destination, guard) = match operand {
        GemmOperandV1::A => (
            GeneralGemmRegionV1::GlobalA,
            GeneralGemmRegionV1::LdsA,
            GeneralGemmBoundsGuardV1::ARowAndDepth,
        ),
        GemmOperandV1::B => (
            GeneralGemmRegionV1::GlobalB,
            GeneralGemmRegionV1::LdsB,
            GeneralGemmBoundsGuardV1::BDepthAndColumn,
        ),
        GemmOperandV1::C => (
            GeneralGemmRegionV1::GlobalC,
            GeneralGemmRegionV1::LdsA,
            GeneralGemmBoundsGuardV1::CRowAndColumn,
        ),
    };
    GeneralGemmStageEventV1 {
        operand,
        source,
        destination,
        guard,
        mapping: GeneralGemmLdsMappingV1::Wave64Xor4,
        coverage: GeneralGemmWriteCoverageV1::Complete,
        tail_value: GeneralGemmTailValueV1::Zero,
        completion: GeneralGemmStageCompletionV1::Ready,
    }
}

const fn canonical_read(operand: GemmOperandV1) -> GeneralGemmLdsReadEventV1 {
    GeneralGemmLdsReadEventV1 {
        operand,
        source: match operand {
            GemmOperandV1::A => GeneralGemmRegionV1::LdsA,
            GemmOperandV1::B => GeneralGemmRegionV1::LdsB,
            GemmOperandV1::C => GeneralGemmRegionV1::GlobalC,
        },
        epoch: GeneralGemmPhaseEpochV1::Current,
    }
}

/// Proof-required property named by a semantic KIR diagnostic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmPropertyV1 {
    /// Valid allocations and region provenance.
    MemorySafe,
    /// Guarded in-bounds accesses.
    BoundsSafe,
    /// Reads observe complete initialized values.
    Initialized,
    /// Conflicting parallel effects are absent.
    RaceFree,
    /// Barrier participation is convergent.
    BarrierConvergent,
    /// C ownership is injective.
    OutputRegionInjective,
    /// LDS epochs publish reads and order reuse.
    LdsEpochCorrect,
    /// Accumulators carry every phase contribution.
    AccumulatorPhaseRefinement,
    /// K-tail slots are zero and accesses are masked.
    TailRefinement,
    /// The alpha/beta output algebra is exact.
    EpilogueRefinement,
    /// BF16 inputs and FP32 accumulation match the schedule contract.
    NumericalContract,
    /// Evidence covers the selected source-to-machine boundary.
    MachineRefinementBoundary,
}

impl GeneralGemmPropertyV1 {
    /// Returns the frozen proof-required property spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MemorySafe => "memory_safe",
            Self::BoundsSafe => "bounds_safe",
            Self::Initialized => "initialized",
            Self::RaceFree => "race_free",
            Self::BarrierConvergent => "barrier_convergent",
            Self::OutputRegionInjective => "output_region_injective",
            Self::LdsEpochCorrect => "lds_epoch_correct",
            Self::AccumulatorPhaseRefinement => "accumulator_phase_refinement",
            Self::TailRefinement => "tail_refinement",
            Self::EpilogueRefinement => "epilogue_refinement",
            Self::NumericalContract => "numerical_contract",
            Self::MachineRefinementBoundary => "machine_refinement_boundary",
        }
    }

    /// Returns the stable compiler diagnostic code.
    pub const fn diagnostic_code(self) -> u32 {
        match self {
            Self::MemorySafe => 0x4647_0101,
            Self::BoundsSafe => 0x4647_0102,
            Self::Initialized => 0x4647_0103,
            Self::RaceFree => 0x4647_0104,
            Self::BarrierConvergent => 0x4647_0105,
            Self::OutputRegionInjective => 0x4647_0106,
            Self::LdsEpochCorrect => 0x4647_0107,
            Self::AccumulatorPhaseRefinement => 0x4647_0108,
            Self::TailRefinement => 0x4647_0109,
            Self::EpilogueRefinement => 0x4647_010a,
            Self::NumericalContract => 0x4647_010b,
            Self::MachineRefinementBoundary => 0x4647_010c,
        }
    }

    /// Returns the earliest compiler stage that owns this obligation.
    pub const fn verification_stage(self) -> GeneralGemmVerificationStageV1 {
        match self {
            Self::MemorySafe | Self::Initialized | Self::RaceFree => {
                GeneralGemmVerificationStageV1::Gpu
            }
            Self::BoundsSafe | Self::OutputRegionInjective => GeneralGemmVerificationStageV1::Tile,
            Self::BarrierConvergent | Self::LdsEpochCorrect => GeneralGemmVerificationStageV1::Gpu,
            Self::AccumulatorPhaseRefinement
            | Self::TailRefinement
            | Self::EpilogueRefinement
            | Self::NumericalContract => GeneralGemmVerificationStageV1::Kernel,
            Self::MachineRefinementBoundary => GeneralGemmVerificationStageV1::Amdgcn,
        }
    }
}

/// Compiler stage attached to a structured semantic rejection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmVerificationStageV1 {
    /// Algorithm and numerical semantics.
    Kernel = 3,
    /// Distributed regions, masks, and tile mappings.
    Tile = 5,
    /// Target-neutral executable SIMT semantics.
    Gpu = 6,
    /// Target-selected machine refinement.
    Amdgcn = 7,
}

impl GeneralGemmVerificationStageV1 {
    /// Returns the compiler-facing stage spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Tile => "tile",
            Self::Gpu => "gpu",
            Self::Amdgcn => "amdgcn",
        }
    }
}

/// Stable issue #138 adversarial mutation identity.
///
/// These identities generate structured event graphs for verifier tests. The
/// verifier itself never receives or branches on a mutation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmSemanticMutationV1 {
    /// Remove the A row/depth tail guard.
    UnguardedATailLoad,
    /// Remove the B depth/column tail guard.
    UnguardedBTailLoad,
    /// Remove the C row/column store guard.
    UnguardedCTailStore,
    /// Alias output ownership across lane/components.
    DuplicateLaneCWrite,
    /// Alias output tiles across workgroups.
    OverlappingWorkgroupCTile,
    /// Alias physical LDS writers in one epoch.
    DuplicateLdsWrite,
    /// Leave a readable LDS cell unwritten.
    LdsReadBeforeInitialization,
    /// Remove the barrier that publishes LDS writes.
    MissingPublishBarrier,
    /// Make a publish barrier lane conditional.
    DivergentBarrier,
    /// Remove the barrier that orders LDS reuse.
    MissingReuseBarrier,
    /// Read an LDS cell from the preceding phase.
    ExpiredLdsEpoch,
    /// Issue asynchronous staging without a completion wait.
    StagedReadBeforeWait,
    /// Reset accumulators between reduction phases.
    AccumulatorReset,
    /// Stage a nonzero value for a K-tail coordinate.
    IncorrectKTailZeroFill,
    /// Replace the declared alpha/beta expression.
    IncorrectAlphaBetaEpilogue,
}

impl GeneralGemmSemanticMutationV1 {
    /// Returns the stable mutation spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnguardedATailLoad => "unguarded_a_tail_load",
            Self::UnguardedBTailLoad => "unguarded_b_tail_load",
            Self::UnguardedCTailStore => "unguarded_c_tail_store",
            Self::DuplicateLaneCWrite => "duplicate_lane_c_write",
            Self::OverlappingWorkgroupCTile => "overlapping_workgroup_c_tile",
            Self::DuplicateLdsWrite => "duplicate_lds_write",
            Self::LdsReadBeforeInitialization => "lds_read_before_initialization",
            Self::MissingPublishBarrier => "missing_publish_barrier",
            Self::DivergentBarrier => "divergent_barrier",
            Self::MissingReuseBarrier => "missing_reuse_barrier",
            Self::ExpiredLdsEpoch => "expired_lds_epoch",
            Self::StagedReadBeforeWait => "staged_read_before_wait",
            Self::AccumulatorReset => "accumulator_reset",
            Self::IncorrectKTailZeroFill => "incorrect_k_tail_zero_fill",
            Self::IncorrectAlphaBetaEpilogue => "incorrect_alpha_beta_epilogue",
        }
    }

    /// Returns the frozen expected verifier finding for this hostile graph.
    pub const fn expectation(self) -> GeneralGemmMutationExpectationV1 {
        let property = match self {
            Self::UnguardedATailLoad | Self::UnguardedBTailLoad | Self::UnguardedCTailStore => {
                GeneralGemmPropertyV1::BoundsSafe
            }
            Self::DuplicateLaneCWrite | Self::OverlappingWorkgroupCTile => {
                GeneralGemmPropertyV1::OutputRegionInjective
            }
            Self::DuplicateLdsWrite => GeneralGemmPropertyV1::RaceFree,
            Self::LdsReadBeforeInitialization
            | Self::MissingPublishBarrier
            | Self::StagedReadBeforeWait => GeneralGemmPropertyV1::Initialized,
            Self::DivergentBarrier => GeneralGemmPropertyV1::BarrierConvergent,
            Self::MissingReuseBarrier | Self::ExpiredLdsEpoch => {
                GeneralGemmPropertyV1::LdsEpochCorrect
            }
            Self::AccumulatorReset => GeneralGemmPropertyV1::AccumulatorPhaseRefinement,
            Self::IncorrectKTailZeroFill => GeneralGemmPropertyV1::TailRefinement,
            Self::IncorrectAlphaBetaEpilogue => GeneralGemmPropertyV1::EpilogueRefinement,
        };
        GeneralGemmMutationExpectationV1 {
            mutation: self,
            property,
            stage: property.verification_stage(),
            code: property.diagnostic_code(),
        }
    }
}

/// Exact required diagnostic for one structured hostile mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmMutationExpectationV1 {
    /// Mutation identity.
    pub mutation: GeneralGemmSemanticMutationV1,
    /// Independently failed property.
    pub property: GeneralGemmPropertyV1,
    /// Earliest owning stage.
    pub stage: GeneralGemmVerificationStageV1,
    /// Stable diagnostic code.
    pub code: u32,
}

/// Complete stable mutation order shared by verifier and driver tests.
pub const GENERAL_GEMM_SEMANTIC_MUTATIONS_V1: [GeneralGemmSemanticMutationV1; 15] = [
    GeneralGemmSemanticMutationV1::UnguardedATailLoad,
    GeneralGemmSemanticMutationV1::UnguardedBTailLoad,
    GeneralGemmSemanticMutationV1::UnguardedCTailStore,
    GeneralGemmSemanticMutationV1::DuplicateLaneCWrite,
    GeneralGemmSemanticMutationV1::OverlappingWorkgroupCTile,
    GeneralGemmSemanticMutationV1::DuplicateLdsWrite,
    GeneralGemmSemanticMutationV1::LdsReadBeforeInitialization,
    GeneralGemmSemanticMutationV1::MissingPublishBarrier,
    GeneralGemmSemanticMutationV1::DivergentBarrier,
    GeneralGemmSemanticMutationV1::MissingReuseBarrier,
    GeneralGemmSemanticMutationV1::ExpiredLdsEpoch,
    GeneralGemmSemanticMutationV1::StagedReadBeforeWait,
    GeneralGemmSemanticMutationV1::AccumulatorReset,
    GeneralGemmSemanticMutationV1::IncorrectKTailZeroFill,
    GeneralGemmSemanticMutationV1::IncorrectAlphaBetaEpilogue,
];

/// Complete frozen property/stage/code metadata in issue order.
pub const GENERAL_GEMM_MUTATION_EXPECTATIONS_V1: [GeneralGemmMutationExpectationV1; 15] = [
    GeneralGemmSemanticMutationV1::UnguardedATailLoad.expectation(),
    GeneralGemmSemanticMutationV1::UnguardedBTailLoad.expectation(),
    GeneralGemmSemanticMutationV1::UnguardedCTailStore.expectation(),
    GeneralGemmSemanticMutationV1::DuplicateLaneCWrite.expectation(),
    GeneralGemmSemanticMutationV1::OverlappingWorkgroupCTile.expectation(),
    GeneralGemmSemanticMutationV1::DuplicateLdsWrite.expectation(),
    GeneralGemmSemanticMutationV1::LdsReadBeforeInitialization.expectation(),
    GeneralGemmSemanticMutationV1::MissingPublishBarrier.expectation(),
    GeneralGemmSemanticMutationV1::DivergentBarrier.expectation(),
    GeneralGemmSemanticMutationV1::MissingReuseBarrier.expectation(),
    GeneralGemmSemanticMutationV1::ExpiredLdsEpoch.expectation(),
    GeneralGemmSemanticMutationV1::StagedReadBeforeWait.expectation(),
    GeneralGemmSemanticMutationV1::AccumulatorReset.expectation(),
    GeneralGemmSemanticMutationV1::IncorrectKTailZeroFill.expectation(),
    GeneralGemmSemanticMutationV1::IncorrectAlphaBetaEpilogue.expectation(),
];

/// Builds one bounded hostile event graph from the canonical structured
/// schedule. This helper exists for adversarial verifier tests and grants no
/// compiler or execution authority.
pub fn general_gemm_semantic_mutation_kir_v1(
    plan: GeneralGemmPlanFieldsV1,
    mutation: GeneralGemmSemanticMutationV1,
) -> GeneralGemmKirV1 {
    let canonical = GeneralGemmKirV1::canonical(plan);
    let mut events = canonical.phase_events().to_vec();
    let mut epilogue = *canonical.epilogue();
    apply_structured_mutation(&mut events, &mut epilogue, mutation);
    GeneralGemmKirV1::checked_from_parts(plan, events, epilogue)
        .expect("a single semantic mutation preserves the event bound")
}

fn apply_structured_mutation(
    events: &mut Vec<GeneralGemmPhaseEventV1>,
    epilogue: &mut GeneralGemmEpilogueEventV1,
    mutation: GeneralGemmSemanticMutationV1,
) {
    match mutation {
        GeneralGemmSemanticMutationV1::UnguardedATailLoad => {
            stage_event_mut(events, GemmOperandV1::A).guard = GeneralGemmBoundsGuardV1::Unguarded;
        }
        GeneralGemmSemanticMutationV1::UnguardedBTailLoad => {
            stage_event_mut(events, GemmOperandV1::B).guard = GeneralGemmBoundsGuardV1::Unguarded;
        }
        GeneralGemmSemanticMutationV1::UnguardedCTailStore => {
            epilogue.store_guard = GeneralGemmBoundsGuardV1::Unguarded;
        }
        GeneralGemmSemanticMutationV1::DuplicateLaneCWrite => {
            epilogue.lane_mapping = GeneralGemmLaneOutputMappingV1::Aliased;
        }
        GeneralGemmSemanticMutationV1::OverlappingWorkgroupCTile => {
            epilogue.workgroup_mapping = GeneralGemmWorkgroupOutputMappingV1::Overlapping;
        }
        GeneralGemmSemanticMutationV1::DuplicateLdsWrite => {
            stage_event_mut(events, GemmOperandV1::A).mapping = GeneralGemmLdsMappingV1::Aliased;
        }
        GeneralGemmSemanticMutationV1::LdsReadBeforeInitialization => {
            stage_event_mut(events, GemmOperandV1::B).coverage =
                GeneralGemmWriteCoverageV1::Incomplete;
        }
        GeneralGemmSemanticMutationV1::MissingPublishBarrier => {
            remove_barrier_event(events, GeneralGemmBarrierRoleV1::Publish);
        }
        GeneralGemmSemanticMutationV1::DivergentBarrier => {
            barrier_event_mut(events, GeneralGemmBarrierRoleV1::Publish).participation =
                GeneralGemmBarrierParticipationV1::LaneConditional;
        }
        GeneralGemmSemanticMutationV1::MissingReuseBarrier => {
            remove_barrier_event(events, GeneralGemmBarrierRoleV1::Reuse);
        }
        GeneralGemmSemanticMutationV1::ExpiredLdsEpoch => {
            lds_read_event_mut(events, GemmOperandV1::A).epoch = GeneralGemmPhaseEpochV1::Previous;
        }
        GeneralGemmSemanticMutationV1::StagedReadBeforeWait => {
            stage_event_mut(events, GemmOperandV1::A).completion =
                GeneralGemmStageCompletionV1::PendingAsync;
        }
        GeneralGemmSemanticMutationV1::AccumulatorReset => {
            accumulate_event_mut(events).input = GeneralGemmAccumulatorInputV1::Reset;
        }
        GeneralGemmSemanticMutationV1::IncorrectKTailZeroFill => {
            stage_event_mut(events, GemmOperandV1::A).tail_value = GeneralGemmTailValueV1::NonZero;
        }
        GeneralGemmSemanticMutationV1::IncorrectAlphaBetaEpilogue => {
            epilogue.expression = GeneralGemmEpilogueExpressionV1::Other;
        }
    }
}

fn stage_event_mut(
    events: &mut [GeneralGemmPhaseEventV1],
    operand: GemmOperandV1,
) -> &mut GeneralGemmStageEventV1 {
    events
        .iter_mut()
        .find_map(|event| match event {
            GeneralGemmPhaseEventV1::Stage(stage) if stage.operand == operand => Some(stage),
            _ => None,
        })
        .expect("canonical stage event is present")
}

fn lds_read_event_mut(
    events: &mut [GeneralGemmPhaseEventV1],
    operand: GemmOperandV1,
) -> &mut GeneralGemmLdsReadEventV1 {
    events
        .iter_mut()
        .find_map(|event| match event {
            GeneralGemmPhaseEventV1::LdsRead(read) if read.operand == operand => Some(read),
            _ => None,
        })
        .expect("canonical LDS read event is present")
}

fn barrier_event_mut(
    events: &mut [GeneralGemmPhaseEventV1],
    role: GeneralGemmBarrierRoleV1,
) -> &mut GeneralGemmBarrierEventV1 {
    events
        .iter_mut()
        .find_map(|event| match event {
            GeneralGemmPhaseEventV1::Barrier(barrier) if barrier.role == role => Some(barrier),
            _ => None,
        })
        .expect("canonical barrier event is present")
}

fn accumulate_event_mut(
    events: &mut [GeneralGemmPhaseEventV1],
) -> &mut GeneralGemmAccumulateEventV1 {
    events
        .iter_mut()
        .find_map(|event| match event {
            GeneralGemmPhaseEventV1::Accumulate(accumulate) => Some(accumulate),
            _ => None,
        })
        .expect("canonical accumulate event is present")
}

fn remove_barrier_event(events: &mut Vec<GeneralGemmPhaseEventV1>, role: GeneralGemmBarrierRoleV1) {
    let index = events
        .iter()
        .position(|event| {
            matches!(event, GeneralGemmPhaseEventV1::Barrier(barrier) if barrier.role == role)
        })
        .expect("canonical barrier event is present");
    events.remove(index);
}

/// Exact semantic counterexample produced by the KIR verifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmKirDiagnosticV1 {
    /// Independently failed property.
    pub property: GeneralGemmPropertyV1,
    /// Earliest owning compiler stage.
    pub stage: GeneralGemmVerificationStageV1,
    /// Stable fe2o3 diagnostic code.
    pub code: u32,
    /// Phase-event index, or `None` for plan/epilogue failures.
    pub event_index: Option<usize>,
}

impl fmt::Display for GeneralGemmKirDiagnosticV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM {} counterexample at {} (0x{:08x})",
            self.property.as_str(),
            self.stage.as_str(),
            self.code
        )
    }
}

impl std::error::Error for GeneralGemmKirDiagnosticV1 {}

/// Deterministic first-finding type returned by the verifier.
pub type GeneralGemmKirFindingV1 = GeneralGemmKirDiagnosticV1;

/// A structurally verified semantic schedule.
///
/// This wrapper intentionally exposes no artifact or launch capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGeneralGemmKirV1 {
    kir: GeneralGemmKirV1,
}

/// Deterministic structured verifier result consumed by compiler admission.
pub type GeneralGemmKirVerificationResultV1 =
    Result<VerifiedGeneralGemmKirV1, GeneralGemmKirFindingV1>;

impl VerifiedGeneralGemmKirV1 {
    /// Returns the verified semantic graph for proof-request binding.
    pub const fn kir(&self) -> &GeneralGemmKirV1 {
        &self.kir
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct OperandState {
    present: bool,
    complete: bool,
    ready: bool,
    published: bool,
    read: bool,
}

/// Verifies the structured general GEMM schedule without consulting source
/// names, source bytes, fixture paths, or digests.
pub fn verify_general_gemm_kir_v1(kir: &GeneralGemmKirV1) -> GeneralGemmKirVerificationResultV1 {
    verify_bounds_and_regions(kir)?;
    verify_output_ownership(kir)?;
    verify_lds_mapping(kir)?;
    verify_barrier_convergence(kir)?;
    verify_phase_state(kir)?;
    verify_refinements(kir)?;
    Ok(VerifiedGeneralGemmKirV1 { kir: kir.clone() })
}

fn diagnostic(
    property: GeneralGemmPropertyV1,
    event_index: Option<usize>,
) -> GeneralGemmKirDiagnosticV1 {
    GeneralGemmKirDiagnosticV1 {
        property,
        stage: property.verification_stage(),
        code: property.diagnostic_code(),
        event_index,
    }
}

fn verify_bounds_and_regions(kir: &GeneralGemmKirV1) -> Result<(), GeneralGemmKirDiagnosticV1> {
    for (index, event) in kir.phase_events.iter().enumerate() {
        let GeneralGemmPhaseEventV1::Stage(stage) = event else {
            continue;
        };
        let expected = match stage.operand {
            GemmOperandV1::A => (
                GeneralGemmRegionV1::GlobalA,
                GeneralGemmRegionV1::LdsA,
                GeneralGemmBoundsGuardV1::ARowAndDepth,
            ),
            GemmOperandV1::B => (
                GeneralGemmRegionV1::GlobalB,
                GeneralGemmRegionV1::LdsB,
                GeneralGemmBoundsGuardV1::BDepthAndColumn,
            ),
            GemmOperandV1::C => {
                return Err(diagnostic(GeneralGemmPropertyV1::MemorySafe, Some(index)));
            }
        };
        if (stage.source, stage.destination) != (expected.0, expected.1) {
            return Err(diagnostic(GeneralGemmPropertyV1::MemorySafe, Some(index)));
        }
        if stage.guard != expected.2 {
            return Err(diagnostic(GeneralGemmPropertyV1::BoundsSafe, Some(index)));
        }
    }
    if kir.epilogue.load_guard != GeneralGemmBoundsGuardV1::CRowAndColumn
        || kir.epilogue.store_guard != GeneralGemmBoundsGuardV1::CRowAndColumn
    {
        return Err(diagnostic(GeneralGemmPropertyV1::BoundsSafe, None));
    }
    Ok(())
}

fn verify_output_ownership(kir: &GeneralGemmKirV1) -> Result<(), GeneralGemmKirDiagnosticV1> {
    if kir.epilogue.lane_mapping != GeneralGemmLaneOutputMappingV1::Wave64FourRows
        || kir.epilogue.workgroup_mapping != GeneralGemmWorkgroupOutputMappingV1::GridXY16
    {
        return Err(diagnostic(
            GeneralGemmPropertyV1::OutputRegionInjective,
            None,
        ));
    }
    Ok(())
}

fn verify_lds_mapping(kir: &GeneralGemmKirV1) -> Result<(), GeneralGemmKirDiagnosticV1> {
    for (index, event) in kir.phase_events.iter().enumerate() {
        if let GeneralGemmPhaseEventV1::Stage(stage) = event
            && stage.mapping != GeneralGemmLdsMappingV1::Wave64Xor4
        {
            return Err(diagnostic(GeneralGemmPropertyV1::RaceFree, Some(index)));
        }
    }
    Ok(())
}

fn verify_barrier_convergence(kir: &GeneralGemmKirV1) -> Result<(), GeneralGemmKirDiagnosticV1> {
    for (index, event) in kir.phase_events.iter().enumerate() {
        if let GeneralGemmPhaseEventV1::Barrier(barrier) = event
            && barrier.participation != GeneralGemmBarrierParticipationV1::UniformWave64
        {
            return Err(diagnostic(
                GeneralGemmPropertyV1::BarrierConvergent,
                Some(index),
            ));
        }
    }
    Ok(())
}

fn verify_phase_state(kir: &GeneralGemmKirV1) -> Result<(), GeneralGemmKirDiagnosticV1> {
    let mut a = OperandState::default();
    let mut b = OperandState::default();
    let mut publish_seen = false;
    let mut reuse_seen = false;
    let mut accumulate_seen = false;

    for (index, event) in kir.phase_events.iter().enumerate() {
        match *event {
            GeneralGemmPhaseEventV1::Stage(stage) => {
                let state = operand_state_mut(stage.operand, &mut a, &mut b)
                    .ok_or_else(|| diagnostic(GeneralGemmPropertyV1::MemorySafe, Some(index)))?;
                if state.present {
                    return Err(diagnostic(GeneralGemmPropertyV1::RaceFree, Some(index)));
                }
                state.present = true;
                state.complete = stage.coverage == GeneralGemmWriteCoverageV1::Complete;
                state.ready = stage.completion == GeneralGemmStageCompletionV1::Ready;
            }
            GeneralGemmPhaseEventV1::AsyncWait(wait) => {
                let state = operand_state_mut(wait.operand, &mut a, &mut b)
                    .ok_or_else(|| diagnostic(GeneralGemmPropertyV1::MemorySafe, Some(index)))?;
                if !state.present {
                    return Err(diagnostic(GeneralGemmPropertyV1::Initialized, Some(index)));
                }
                state.ready = true;
            }
            GeneralGemmPhaseEventV1::Barrier(barrier) => match barrier.role {
                GeneralGemmBarrierRoleV1::Publish => {
                    if !a.present
                        || !b.present
                        || !a.complete
                        || !b.complete
                        || !a.ready
                        || !b.ready
                    {
                        return Err(diagnostic(GeneralGemmPropertyV1::Initialized, Some(index)));
                    }
                    publish_seen = true;
                    a.published = true;
                    b.published = true;
                }
                GeneralGemmBarrierRoleV1::Reuse => {
                    if !a.read || !b.read || !accumulate_seen {
                        return Err(diagnostic(
                            GeneralGemmPropertyV1::LdsEpochCorrect,
                            Some(index),
                        ));
                    }
                    reuse_seen = true;
                }
            },
            GeneralGemmPhaseEventV1::LdsRead(read) => {
                if read.epoch != GeneralGemmPhaseEpochV1::Current {
                    return Err(diagnostic(
                        GeneralGemmPropertyV1::LdsEpochCorrect,
                        Some(index),
                    ));
                }
                let expected_source = match read.operand {
                    GemmOperandV1::A => GeneralGemmRegionV1::LdsA,
                    GemmOperandV1::B => GeneralGemmRegionV1::LdsB,
                    GemmOperandV1::C => {
                        return Err(diagnostic(GeneralGemmPropertyV1::MemorySafe, Some(index)));
                    }
                };
                if read.source != expected_source {
                    return Err(diagnostic(GeneralGemmPropertyV1::MemorySafe, Some(index)));
                }
                let state = operand_state_mut(read.operand, &mut a, &mut b)
                    .expect("A and B were matched above");
                if !state.present || !state.complete || !state.ready || !state.published {
                    return Err(diagnostic(GeneralGemmPropertyV1::Initialized, Some(index)));
                }
                state.read = true;
            }
            GeneralGemmPhaseEventV1::Accumulate(_) => {
                if !a.read || !b.read {
                    return Err(diagnostic(GeneralGemmPropertyV1::Initialized, Some(index)));
                }
                accumulate_seen = true;
            }
        }
    }

    if !a.present || !b.present || !a.complete || !b.complete {
        return Err(diagnostic(GeneralGemmPropertyV1::Initialized, None));
    }
    if !publish_seen || !a.read || !b.read || !accumulate_seen {
        return Err(diagnostic(GeneralGemmPropertyV1::Initialized, None));
    }
    if !reuse_seen {
        return Err(diagnostic(GeneralGemmPropertyV1::LdsEpochCorrect, None));
    }
    Ok(())
}

fn operand_state_mut<'a>(
    operand: GemmOperandV1,
    a: &'a mut OperandState,
    b: &'a mut OperandState,
) -> Option<&'a mut OperandState> {
    match operand {
        GemmOperandV1::A => Some(a),
        GemmOperandV1::B => Some(b),
        GemmOperandV1::C => None,
    }
}

fn verify_refinements(kir: &GeneralGemmKirV1) -> Result<(), GeneralGemmKirDiagnosticV1> {
    let mut accumulator_count = 0;
    for (index, event) in kir.phase_events.iter().enumerate() {
        match event {
            GeneralGemmPhaseEventV1::Stage(stage)
                if stage.tail_value != GeneralGemmTailValueV1::Zero =>
            {
                return Err(diagnostic(
                    GeneralGemmPropertyV1::TailRefinement,
                    Some(index),
                ));
            }
            GeneralGemmPhaseEventV1::Accumulate(accumulate) => {
                accumulator_count += 1;
                if accumulate.input != GeneralGemmAccumulatorInputV1::Carried {
                    return Err(diagnostic(
                        GeneralGemmPropertyV1::AccumulatorPhaseRefinement,
                        Some(index),
                    ));
                }
            }
            _ => {}
        }
    }
    if accumulator_count != 1 {
        return Err(diagnostic(
            GeneralGemmPropertyV1::AccumulatorPhaseRefinement,
            None,
        ));
    }
    if kir.epilogue.expression != GeneralGemmEpilogueExpressionV1::AlphaAccumulatorPlusBetaC {
        return Err(diagnostic(GeneralGemmPropertyV1::EpilogueRefinement, None));
    }
    Ok(())
}
