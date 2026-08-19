//! Closed compiler semantics for the authenticated general-GEMM typestate intrinsics.
//!
//! This schema describes what the six production provider terminals mean after
//! their live `DefId`s, optimized MIR, and exact ABIs have been authenticated.
//! It is not source evidence: matching this identity, a provider source hash,
//! or a diagnostic-item spelling cannot establish that a kernel calls the
//! terminals with the required control flow and operands. The frontend owner
//! must join those live facts separately.

#![allow(dead_code)]

use sha2::{Digest as _, Sha256};

const SCHEMA_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.intrinsic-semantics.v1\0";
const TILE_EXTENT_V1: u16 = 16;
const WAVE_LANES_V1: u16 = 64;
const COMPONENTS_PER_LANE_V1: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct GeneralGemmIntrinsicSemanticsIdentityV1([u8; 32]);

impl GeneralGemmIntrinsicSemanticsIdentityV1 {
    pub(crate) const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmIntrinsicRoleV1 {
    Acquire = 1,
    Stage = 2,
    Publish = 3,
    Mfma = 4,
    Reuse = 5,
    Store = 6,
}

const TERMINAL_ROLES_V1: [GeneralGemmIntrinsicRoleV1; 6] = [
    GeneralGemmIntrinsicRoleV1::Acquire,
    GeneralGemmIntrinsicRoleV1::Stage,
    GeneralGemmIntrinsicRoleV1::Publish,
    GeneralGemmIntrinsicRoleV1::Mfma,
    GeneralGemmIntrinsicRoleV1::Reuse,
    GeneralGemmIntrinsicRoleV1::Store,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmIntrinsicValueTypeV1 {
    U32 = 1,
    F32 = 2,
    Bf16Bits4 = 3,
    DisjointF32Slice = 4,
    WaveReady = 5,
    WaveStaged = 6,
    WavePublished = 7,
    WaveConsumed = 8,
    Unit = 9,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmIntrinsicPassingModeV1 {
    Owned = 1,
    ExclusiveBorrow = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmIntrinsicParameterNameV1 {
    K = 1,
    Wave = 2,
    ABits = 3,
    BBits = 4,
    C = 5,
    M = 6,
    N = 7,
    Ldc = 8,
    Alpha = 9,
    Beta = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmIntrinsicParameterV1 {
    name: GeneralGemmIntrinsicParameterNameV1,
    value_type: GeneralGemmIntrinsicValueTypeV1,
    passing: GeneralGemmIntrinsicPassingModeV1,
}

const fn parameter(
    name: GeneralGemmIntrinsicParameterNameV1,
    value_type: GeneralGemmIntrinsicValueTypeV1,
    passing: GeneralGemmIntrinsicPassingModeV1,
) -> GeneralGemmIntrinsicParameterV1 {
    GeneralGemmIntrinsicParameterV1 {
        name,
        value_type,
        passing,
    }
}

const ACQUIRE_PARAMETERS_V1: [GeneralGemmIntrinsicParameterV1; 1] = [parameter(
    GeneralGemmIntrinsicParameterNameV1::K,
    GeneralGemmIntrinsicValueTypeV1::U32,
    GeneralGemmIntrinsicPassingModeV1::Owned,
)];
const STAGE_PARAMETERS_V1: [GeneralGemmIntrinsicParameterV1; 3] = [
    parameter(
        GeneralGemmIntrinsicParameterNameV1::Wave,
        GeneralGemmIntrinsicValueTypeV1::WaveReady,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::ABits,
        GeneralGemmIntrinsicValueTypeV1::Bf16Bits4,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::BBits,
        GeneralGemmIntrinsicValueTypeV1::Bf16Bits4,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
];
const PUBLISH_PARAMETERS_V1: [GeneralGemmIntrinsicParameterV1; 1] = [parameter(
    GeneralGemmIntrinsicParameterNameV1::Wave,
    GeneralGemmIntrinsicValueTypeV1::WaveStaged,
    GeneralGemmIntrinsicPassingModeV1::Owned,
)];
const MFMA_PARAMETERS_V1: [GeneralGemmIntrinsicParameterV1; 1] = [parameter(
    GeneralGemmIntrinsicParameterNameV1::Wave,
    GeneralGemmIntrinsicValueTypeV1::WavePublished,
    GeneralGemmIntrinsicPassingModeV1::Owned,
)];
const REUSE_PARAMETERS_V1: [GeneralGemmIntrinsicParameterV1; 1] = [parameter(
    GeneralGemmIntrinsicParameterNameV1::Wave,
    GeneralGemmIntrinsicValueTypeV1::WaveConsumed,
    GeneralGemmIntrinsicPassingModeV1::Owned,
)];
const STORE_PARAMETERS_V1: [GeneralGemmIntrinsicParameterV1; 7] = [
    parameter(
        GeneralGemmIntrinsicParameterNameV1::Wave,
        GeneralGemmIntrinsicValueTypeV1::WaveReady,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::C,
        GeneralGemmIntrinsicValueTypeV1::DisjointF32Slice,
        GeneralGemmIntrinsicPassingModeV1::ExclusiveBorrow,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::M,
        GeneralGemmIntrinsicValueTypeV1::U32,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::N,
        GeneralGemmIntrinsicValueTypeV1::U32,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::Ldc,
        GeneralGemmIntrinsicValueTypeV1::U32,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::Alpha,
        GeneralGemmIntrinsicValueTypeV1::F32,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
    parameter(
        GeneralGemmIntrinsicParameterNameV1::Beta,
        GeneralGemmIntrinsicValueTypeV1::F32,
        GeneralGemmIntrinsicPassingModeV1::Owned,
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmIntrinsicTerminalV1 {
    role: GeneralGemmIntrinsicRoleV1,
    diagnostic_item: &'static str,
    canonical_def_path: &'static str,
    parameters: &'static [GeneralGemmIntrinsicParameterV1],
    result: GeneralGemmIntrinsicValueTypeV1,
    unsafe_rust_fn: bool,
    rust_call_abi: bool,
    inline_never: bool,
}

const TERMINALS_V1: [GeneralGemmIntrinsicTerminalV1; 6] = [
    GeneralGemmIntrinsicTerminalV1 {
        role: GeneralGemmIntrinsicRoleV1::Acquire,
        diagnostic_item: "fe2o3_device_general_tiled_gemm_wave64_acquire_v1",
        canonical_def_path: "fe2o3_gemm_device_v1::acquire_gfx942_tiled_gemm_wave64_v1",
        parameters: &ACQUIRE_PARAMETERS_V1,
        result: GeneralGemmIntrinsicValueTypeV1::WaveReady,
        unsafe_rust_fn: true,
        rust_call_abi: true,
        inline_never: true,
    },
    GeneralGemmIntrinsicTerminalV1 {
        role: GeneralGemmIntrinsicRoleV1::Stage,
        diagnostic_item: "fe2o3_device_general_tiled_gemm_wave64_stage_v1",
        canonical_def_path: "fe2o3_gemm_device_v1::stage_gfx942_tiled_gemm_wave64_v1",
        parameters: &STAGE_PARAMETERS_V1,
        result: GeneralGemmIntrinsicValueTypeV1::WaveStaged,
        unsafe_rust_fn: true,
        rust_call_abi: true,
        inline_never: true,
    },
    GeneralGemmIntrinsicTerminalV1 {
        role: GeneralGemmIntrinsicRoleV1::Publish,
        diagnostic_item: "fe2o3_device_general_tiled_gemm_wave64_publish_v1",
        canonical_def_path: "fe2o3_gemm_device_v1::publish_gfx942_tiled_gemm_wave64_v1",
        parameters: &PUBLISH_PARAMETERS_V1,
        result: GeneralGemmIntrinsicValueTypeV1::WavePublished,
        unsafe_rust_fn: true,
        rust_call_abi: true,
        inline_never: true,
    },
    GeneralGemmIntrinsicTerminalV1 {
        role: GeneralGemmIntrinsicRoleV1::Mfma,
        diagnostic_item: "fe2o3_device_general_tiled_gemm_wave64_mfma_v1",
        canonical_def_path: "fe2o3_gemm_device_v1::mfma_gfx942_tiled_gemm_wave64_v1",
        parameters: &MFMA_PARAMETERS_V1,
        result: GeneralGemmIntrinsicValueTypeV1::WaveConsumed,
        unsafe_rust_fn: true,
        rust_call_abi: true,
        inline_never: true,
    },
    GeneralGemmIntrinsicTerminalV1 {
        role: GeneralGemmIntrinsicRoleV1::Reuse,
        diagnostic_item: "fe2o3_device_general_tiled_gemm_wave64_reuse_v1",
        canonical_def_path: "fe2o3_gemm_device_v1::reuse_gfx942_tiled_gemm_wave64_v1",
        parameters: &REUSE_PARAMETERS_V1,
        result: GeneralGemmIntrinsicValueTypeV1::WaveReady,
        unsafe_rust_fn: true,
        rust_call_abi: true,
        inline_never: true,
    },
    GeneralGemmIntrinsicTerminalV1 {
        role: GeneralGemmIntrinsicRoleV1::Store,
        diagnostic_item: "fe2o3_device_general_tiled_gemm_wave64_store_v1",
        canonical_def_path: "fe2o3_gemm_device_v1::store_gfx942_tiled_gemm_wave64_v1",
        parameters: &STORE_PARAMETERS_V1,
        result: GeneralGemmIntrinsicValueTypeV1::Unit,
        unsafe_rust_fn: true,
        rust_call_abi: true,
        inline_never: true,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmCoordinateFormulaV1 {
    TileRowOrigin = 1,
    TileColumnOrigin = 2,
    LaneModulo16 = 3,
    PhaseTimes16PlusLaneQuarterTimes4PlusComponent = 4,
    LaneQuarterTimes4PlusComponent = 5,
    CeilKDiv16 = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmLaneTilePhaseSemanticsV1 {
    workgroup: [u16; 3],
    tile: [u16; 3],
    components_per_lane: u8,
    a_row_in_tile: GeneralGemmCoordinateFormulaV1,
    b_column_in_tile: GeneralGemmCoordinateFormulaV1,
    stage_depth: GeneralGemmCoordinateFormulaV1,
    output_row_in_tile: GeneralGemmCoordinateFormulaV1,
    output_column_in_tile: GeneralGemmCoordinateFormulaV1,
    phase_count: GeneralGemmCoordinateFormulaV1,
}

const LANE_TILE_PHASE_V1: GeneralGemmLaneTilePhaseSemanticsV1 =
    GeneralGemmLaneTilePhaseSemanticsV1 {
        workgroup: [WAVE_LANES_V1, 1, 1],
        tile: [TILE_EXTENT_V1, TILE_EXTENT_V1, TILE_EXTENT_V1],
        components_per_lane: COMPONENTS_PER_LANE_V1,
        a_row_in_tile: GeneralGemmCoordinateFormulaV1::LaneModulo16,
        b_column_in_tile: GeneralGemmCoordinateFormulaV1::LaneModulo16,
        stage_depth: GeneralGemmCoordinateFormulaV1::PhaseTimes16PlusLaneQuarterTimes4PlusComponent,
        output_row_in_tile: GeneralGemmCoordinateFormulaV1::LaneQuarterTimes4PlusComponent,
        output_column_in_tile: GeneralGemmCoordinateFormulaV1::LaneModulo16,
        phase_count: GeneralGemmCoordinateFormulaV1::CeilKDiv16,
    };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmMemoryRegionV1 {
    GlobalA = 1,
    GlobalB = 2,
    GlobalC = 3,
    WorkgroupLdsA = 4,
    WorkgroupLdsB = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmGuardPredicateV1 {
    ARowLessMAndDepthLessK = 1,
    BDepthLessKAndColumnLessN = 2,
    CRowLessMAndColumnLessN = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmRowMajorStrideV1 {
    Lda = 1,
    Ldb = 2,
    Ldc = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmOutOfDomainActionV1 {
    PositiveBf16Zero = 1,
    SuppressAccess = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmGuardedRegionSemanticsV1 {
    region: GeneralGemmMemoryRegionV1,
    predicate: GeneralGemmGuardPredicateV1,
    stride: GeneralGemmRowMajorStrideV1,
    checked_row_times_stride_plus_column: bool,
    out_of_domain: GeneralGemmOutOfDomainActionV1,
}

const GUARDED_REGIONS_V1: [GeneralGemmGuardedRegionSemanticsV1; 3] = [
    GeneralGemmGuardedRegionSemanticsV1 {
        region: GeneralGemmMemoryRegionV1::GlobalA,
        predicate: GeneralGemmGuardPredicateV1::ARowLessMAndDepthLessK,
        stride: GeneralGemmRowMajorStrideV1::Lda,
        checked_row_times_stride_plus_column: true,
        out_of_domain: GeneralGemmOutOfDomainActionV1::PositiveBf16Zero,
    },
    GeneralGemmGuardedRegionSemanticsV1 {
        region: GeneralGemmMemoryRegionV1::GlobalB,
        predicate: GeneralGemmGuardPredicateV1::BDepthLessKAndColumnLessN,
        stride: GeneralGemmRowMajorStrideV1::Ldb,
        checked_row_times_stride_plus_column: true,
        out_of_domain: GeneralGemmOutOfDomainActionV1::PositiveBf16Zero,
    },
    GeneralGemmGuardedRegionSemanticsV1 {
        region: GeneralGemmMemoryRegionV1::GlobalC,
        predicate: GeneralGemmGuardPredicateV1::CRowLessMAndColumnLessN,
        stride: GeneralGemmRowMajorStrideV1::Ldc,
        checked_row_times_stride_plus_column: true,
        out_of_domain: GeneralGemmOutOfDomainActionV1::SuppressAccess,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmAllocationSemanticsV1 {
    a_borrow_shared: bool,
    b_borrow_shared: bool,
    c_borrow_exclusive_disjoint: bool,
    a_lds_region: GeneralGemmMemoryRegionV1,
    b_lds_region: GeneralGemmMemoryRegionV1,
    lds_region_bytes: u16,
    lds_region_alignment: u8,
    lds_regions_nonoverlapping: bool,
    wave_capability_unique_linear: bool,
}

const ALLOCATION_V1: GeneralGemmAllocationSemanticsV1 = GeneralGemmAllocationSemanticsV1 {
    a_borrow_shared: true,
    b_borrow_shared: true,
    c_borrow_exclusive_disjoint: true,
    a_lds_region: GeneralGemmMemoryRegionV1::WorkgroupLdsA,
    b_lds_region: GeneralGemmMemoryRegionV1::WorkgroupLdsB,
    lds_region_bytes: 512,
    lds_region_alignment: 16,
    lds_regions_nonoverlapping: true,
    wave_capability_unique_linear: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmExtentSemanticsV1 {
    empty_if_either_dimension_is_zero: bool,
    otherwise_checked_last_row_times_stride_plus_width: bool,
    nonempty_stride_at_least_logical_width: bool,
    a_slice_length_dominates_m_by_k_extent: bool,
    b_slice_length_dominates_k_by_n_extent: bool,
    c_disjoint_slice_length_dominates_m_by_n_extent: bool,
    failure_traps_before_acquire: bool,
}

const EXTENTS_V1: GeneralGemmExtentSemanticsV1 = GeneralGemmExtentSemanticsV1 {
    empty_if_either_dimension_is_zero: true,
    otherwise_checked_last_row_times_stride_plus_width: true,
    nonempty_stride_at_least_logical_width: true,
    a_slice_length_dominates_m_by_k_extent: true,
    b_slice_length_dominates_k_by_n_extent: true,
    c_disjoint_slice_length_dominates_m_by_n_extent: true,
    failure_traps_before_acquire: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmLdsLayoutV1 {
    RowMajorXor4ColumnXorRowLow2Times4 = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmLdsSemanticsV1 {
    tile_rows: u16,
    tile_columns: u16,
    element_bytes: u8,
    layout: GeneralGemmLdsLayoutV1,
    a_logical_axes_row_depth: bool,
    b_logical_axes_column_depth_transposed: bool,
    writes_per_lane_per_tile: u8,
    writes_per_tile_per_phase: u16,
    each_slot_has_one_writer: bool,
    mfma_reads_current_published_epoch_only: bool,
}

const LDS_V1: GeneralGemmLdsSemanticsV1 = GeneralGemmLdsSemanticsV1 {
    tile_rows: TILE_EXTENT_V1,
    tile_columns: TILE_EXTENT_V1,
    element_bytes: 2,
    layout: GeneralGemmLdsLayoutV1::RowMajorXor4ColumnXorRowLow2Times4,
    a_logical_axes_row_depth: true,
    b_logical_axes_column_depth_transposed: true,
    writes_per_lane_per_tile: COMPONENTS_PER_LANE_V1,
    writes_per_tile_per_phase: 256,
    each_slot_has_one_writer: true,
    mfma_reads_current_published_epoch_only: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmPhaseStateV1 {
    Ready = 1,
    Staged = 2,
    Published = 3,
    Consumed = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmPhaseTransitionV1 {
    role: GeneralGemmIntrinsicRoleV1,
    from: GeneralGemmPhaseStateV1,
    to: GeneralGemmPhaseStateV1,
    convergent_wave_barrier: bool,
    epoch_increment: u8,
}

const PHASE_TRANSITIONS_V1: [GeneralGemmPhaseTransitionV1; 4] = [
    GeneralGemmPhaseTransitionV1 {
        role: GeneralGemmIntrinsicRoleV1::Stage,
        from: GeneralGemmPhaseStateV1::Ready,
        to: GeneralGemmPhaseStateV1::Staged,
        convergent_wave_barrier: false,
        epoch_increment: 0,
    },
    GeneralGemmPhaseTransitionV1 {
        role: GeneralGemmIntrinsicRoleV1::Publish,
        from: GeneralGemmPhaseStateV1::Staged,
        to: GeneralGemmPhaseStateV1::Published,
        convergent_wave_barrier: true,
        epoch_increment: 0,
    },
    GeneralGemmPhaseTransitionV1 {
        role: GeneralGemmIntrinsicRoleV1::Mfma,
        from: GeneralGemmPhaseStateV1::Published,
        to: GeneralGemmPhaseStateV1::Consumed,
        convergent_wave_barrier: false,
        epoch_increment: 0,
    },
    GeneralGemmPhaseTransitionV1 {
        role: GeneralGemmIntrinsicRoleV1::Reuse,
        from: GeneralGemmPhaseStateV1::Consumed,
        to: GeneralGemmPhaseStateV1::Ready,
        convergent_wave_barrier: true,
        epoch_increment: 1,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmLdsLifecycleSemanticsV1 {
    transitions: [GeneralGemmPhaseTransitionV1; 4],
    all_lanes_execute_each_live_phase: bool,
    publish_dominates_mfma_reads: bool,
    reuse_postdominates_mfma_reads: bool,
    next_stage_follows_reuse: bool,
    store_requires_ready_at_phase_count: bool,
}

const LIFECYCLE_V1: GeneralGemmLdsLifecycleSemanticsV1 = GeneralGemmLdsLifecycleSemanticsV1 {
    transitions: PHASE_TRANSITIONS_V1,
    all_lanes_execute_each_live_phase: true,
    publish_dominates_mfma_reads: true,
    reuse_postdominates_mfma_reads: true,
    next_stage_follows_reuse: true,
    store_requires_ready_at_phase_count: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmOutputOwnershipSemanticsV1 {
    row_in_tile: GeneralGemmCoordinateFormulaV1,
    column_in_tile: GeneralGemmCoordinateFormulaV1,
    lane_component_injective: bool,
    workgroup_tile_injective: bool,
    valid_output_has_exactly_one_store: bool,
}

const OUTPUT_OWNERSHIP_V1: GeneralGemmOutputOwnershipSemanticsV1 =
    GeneralGemmOutputOwnershipSemanticsV1 {
        row_in_tile: GeneralGemmCoordinateFormulaV1::LaneQuarterTimes4PlusComponent,
        column_in_tile: GeneralGemmCoordinateFormulaV1::LaneModulo16,
        lane_component_injective: true,
        workgroup_tile_injective: true,
        valid_output_has_exactly_one_store: true,
    };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmStoreSemanticsV1 {
    owner: GeneralGemmIntrinsicRoleV1,
    consumes_ready_at_completed_phase_count: bool,
    tile_row_origin: GeneralGemmCoordinateFormulaV1,
    tile_column_origin: GeneralGemmCoordinateFormulaV1,
    row_in_tile: GeneralGemmCoordinateFormulaV1,
    column_in_tile: GeneralGemmCoordinateFormulaV1,
    predicate: GeneralGemmGuardPredicateV1,
    stride: GeneralGemmRowMajorStrideV1,
    checked_row_times_ldc_plus_column: bool,
    index_must_be_within_disjoint_c_extent: bool,
    out_of_domain_suppresses_prior_c_load_and_store: bool,
    prior_c_and_result_use_same_owned_index: bool,
    formula: [GeneralGemmEpilogueOperationV1; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmAccumulatorSemanticsV1 {
    components_per_lane: u8,
    initial_f32_bits: u32,
    initialized_by_acquire: bool,
    updated_by_each_mfma: bool,
    carried_across_reuse: bool,
    reset_between_phases: bool,
}

const ACCUMULATOR_V1: GeneralGemmAccumulatorSemanticsV1 = GeneralGemmAccumulatorSemanticsV1 {
    components_per_lane: COMPONENTS_PER_LANE_V1,
    initial_f32_bits: 0,
    initialized_by_acquire: true,
    updated_by_each_mfma: true,
    carried_across_reuse: true,
    reset_between_phases: false,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmTailSemanticsV1 {
    phase_count: GeneralGemmCoordinateFormulaV1,
    a_oob_bf16_bits: u16,
    b_oob_bf16_bits: u16,
    c_oob_suppresses_load_and_store: bool,
    zero_k_skips_all_phases: bool,
}

const TAILS_V1: GeneralGemmTailSemanticsV1 = GeneralGemmTailSemanticsV1 {
    phase_count: GeneralGemmCoordinateFormulaV1::CeilKDiv16,
    a_oob_bf16_bits: 0,
    b_oob_bf16_bits: 0,
    c_oob_suppresses_load_and_store: true,
    zero_k_skips_all_phases: true,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmEpilogueOperationV1 {
    MultiplyAlphaByAccumulator = 1,
    MultiplyBetaByPriorC = 2,
    AddScaledTerms = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmEpilogueSemanticsV1 {
    operations: [GeneralGemmEpilogueOperationV1; 3],
    runtime_alpha: bool,
    runtime_beta: bool,
    prior_c_same_owned_coordinate: bool,
    separate_fp32_operations: bool,
}

const EPILOGUE_V1: GeneralGemmEpilogueSemanticsV1 = GeneralGemmEpilogueSemanticsV1 {
    operations: [
        GeneralGemmEpilogueOperationV1::MultiplyAlphaByAccumulator,
        GeneralGemmEpilogueOperationV1::MultiplyBetaByPriorC,
        GeneralGemmEpilogueOperationV1::AddScaledTerms,
    ],
    runtime_alpha: true,
    runtime_beta: true,
    prior_c_same_owned_coordinate: true,
    separate_fp32_operations: true,
};

const STORE_V1: GeneralGemmStoreSemanticsV1 = GeneralGemmStoreSemanticsV1 {
    owner: GeneralGemmIntrinsicRoleV1::Store,
    consumes_ready_at_completed_phase_count: true,
    tile_row_origin: GeneralGemmCoordinateFormulaV1::TileRowOrigin,
    tile_column_origin: GeneralGemmCoordinateFormulaV1::TileColumnOrigin,
    row_in_tile: GeneralGemmCoordinateFormulaV1::LaneQuarterTimes4PlusComponent,
    column_in_tile: GeneralGemmCoordinateFormulaV1::LaneModulo16,
    predicate: GeneralGemmGuardPredicateV1::CRowLessMAndColumnLessN,
    stride: GeneralGemmRowMajorStrideV1::Ldc,
    checked_row_times_ldc_plus_column: true,
    index_must_be_within_disjoint_c_extent: true,
    out_of_domain_suppresses_prior_c_load_and_store: true,
    prior_c_and_result_use_same_owned_index: true,
    formula: [
        GeneralGemmEpilogueOperationV1::MultiplyAlphaByAccumulator,
        GeneralGemmEpilogueOperationV1::MultiplyBetaByPriorC,
        GeneralGemmEpilogueOperationV1::AddScaledTerms,
    ],
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmNumericalStepV1 {
    PositiveZeroFillBf16Bits = 1,
    ExactBf16Widening = 2,
    Gfx942VmfmaF32M16N16K16Bf16 = 3,
    CarryFp32AccumulatorToNextIncreasingPhase = 4,
    SeparateFp32AlphaMultiply = 5,
    SeparateFp32BetaMultiply = 6,
    SeparateFp32EpilogueAdd = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmNumericalOrderSemanticsV1 {
    steps: [GeneralGemmNumericalStepV1; 7],
    increasing_phase_order: bool,
    machine_mfma_confirmation_still_required: bool,
}

const NUMERICAL_ORDER_V1: GeneralGemmNumericalOrderSemanticsV1 =
    GeneralGemmNumericalOrderSemanticsV1 {
        steps: [
            GeneralGemmNumericalStepV1::PositiveZeroFillBf16Bits,
            GeneralGemmNumericalStepV1::ExactBf16Widening,
            GeneralGemmNumericalStepV1::Gfx942VmfmaF32M16N16K16Bf16,
            GeneralGemmNumericalStepV1::CarryFp32AccumulatorToNextIncreasingPhase,
            GeneralGemmNumericalStepV1::SeparateFp32AlphaMultiply,
            GeneralGemmNumericalStepV1::SeparateFp32BetaMultiply,
            GeneralGemmNumericalStepV1::SeparateFp32EpilogueAdd,
        ],
        increasing_phase_order: true,
        machine_mfma_confirmation_still_required: true,
    };

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmIntrinsicSourceFactKindV1 {
    AllocationAndProvenance = 1,
    GuardedGlobalAccesses = 2,
    LdsWriteReadInitialization = 3,
    EffectConflictFreedom = 4,
    ControlFlowBarrierConvergence = 5,
    OutputOwnership = 6,
    LdsLifecycle = 7,
    AccumulatorPhase = 8,
    MaskedTail = 9,
    AlphaBetaEpilogue = 10,
    NumericalOperationOrder = 11,
}

const SOURCE_FACT_KINDS_V1: [GeneralGemmIntrinsicSourceFactKindV1; 11] = [
    GeneralGemmIntrinsicSourceFactKindV1::AllocationAndProvenance,
    GeneralGemmIntrinsicSourceFactKindV1::GuardedGlobalAccesses,
    GeneralGemmIntrinsicSourceFactKindV1::LdsWriteReadInitialization,
    GeneralGemmIntrinsicSourceFactKindV1::EffectConflictFreedom,
    GeneralGemmIntrinsicSourceFactKindV1::ControlFlowBarrierConvergence,
    GeneralGemmIntrinsicSourceFactKindV1::OutputOwnership,
    GeneralGemmIntrinsicSourceFactKindV1::LdsLifecycle,
    GeneralGemmIntrinsicSourceFactKindV1::AccumulatorPhase,
    GeneralGemmIntrinsicSourceFactKindV1::MaskedTail,
    GeneralGemmIntrinsicSourceFactKindV1::AlphaBetaEpilogue,
    GeneralGemmIntrinsicSourceFactKindV1::NumericalOperationOrder,
];

const COMPONENT_ALLOCATION: u16 = 1 << 0;
const COMPONENT_LANE_TILE_PHASE: u16 = 1 << 1;
const COMPONENT_GUARDS: u16 = 1 << 2;
const COMPONENT_LDS: u16 = 1 << 3;
const COMPONENT_LIFECYCLE: u16 = 1 << 4;
const COMPONENT_OUTPUT: u16 = 1 << 5;
const COMPONENT_ACCUMULATOR: u16 = 1 << 6;
const COMPONENT_TAILS: u16 = 1 << 7;
const COMPONENT_EPILOGUE: u16 = 1 << 8;
const COMPONENT_NUMERICAL: u16 = 1 << 9;
const COMPONENT_EXTENTS: u16 = 1 << 10;
const COMPONENT_STORE: u16 = 1 << 11;

const fn role_bit(role: GeneralGemmIntrinsicRoleV1) -> u8 {
    1 << ((role as u8) - 1)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmIntrinsicSourceFactV1 {
    kind: GeneralGemmIntrinsicSourceFactKindV1,
    terminal_role_mask: u8,
    semantic_component_mask: u16,
}

const SOURCE_FACTS_V1: [GeneralGemmIntrinsicSourceFactV1; 11] = [
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::AllocationAndProvenance,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Acquire)
            | role_bit(GeneralGemmIntrinsicRoleV1::Stage)
            | role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_ALLOCATION
            | COMPONENT_EXTENTS
            | COMPONENT_LANE_TILE_PHASE
            | COMPONENT_LDS,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::GuardedGlobalAccesses,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Stage)
            | role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_ALLOCATION
            | COMPONENT_EXTENTS
            | COMPONENT_LANE_TILE_PHASE
            | COMPONENT_GUARDS
            | COMPONENT_STORE,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::LdsWriteReadInitialization,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Acquire)
            | role_bit(GeneralGemmIntrinsicRoleV1::Stage)
            | role_bit(GeneralGemmIntrinsicRoleV1::Publish)
            | role_bit(GeneralGemmIntrinsicRoleV1::Mfma),
        semantic_component_mask: COMPONENT_LDS | COMPONENT_LIFECYCLE | COMPONENT_TAILS,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::EffectConflictFreedom,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Stage)
            | role_bit(GeneralGemmIntrinsicRoleV1::Mfma)
            | role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_ALLOCATION
            | COMPONENT_LANE_TILE_PHASE
            | COMPONENT_LDS
            | COMPONENT_LIFECYCLE
            | COMPONENT_OUTPUT
            | COMPONENT_STORE,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::ControlFlowBarrierConvergence,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Publish)
            | role_bit(GeneralGemmIntrinsicRoleV1::Reuse),
        semantic_component_mask: COMPONENT_LANE_TILE_PHASE | COMPONENT_LIFECYCLE,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::OutputOwnership,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Acquire)
            | role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_LANE_TILE_PHASE | COMPONENT_OUTPUT | COMPONENT_STORE,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::LdsLifecycle,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Stage)
            | role_bit(GeneralGemmIntrinsicRoleV1::Publish)
            | role_bit(GeneralGemmIntrinsicRoleV1::Mfma)
            | role_bit(GeneralGemmIntrinsicRoleV1::Reuse),
        semantic_component_mask: COMPONENT_LDS | COMPONENT_LIFECYCLE,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::AccumulatorPhase,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Acquire)
            | role_bit(GeneralGemmIntrinsicRoleV1::Mfma)
            | role_bit(GeneralGemmIntrinsicRoleV1::Reuse)
            | role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_LIFECYCLE | COMPONENT_ACCUMULATOR,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::MaskedTail,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Acquire)
            | role_bit(GeneralGemmIntrinsicRoleV1::Stage)
            | role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_ALLOCATION
            | COMPONENT_EXTENTS
            | COMPONENT_LANE_TILE_PHASE
            | COMPONENT_GUARDS
            | COMPONENT_TAILS
            | COMPONENT_STORE,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::AlphaBetaEpilogue,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_GUARDS
            | COMPONENT_OUTPUT
            | COMPONENT_TAILS
            | COMPONENT_EPILOGUE
            | COMPONENT_STORE,
    },
    GeneralGemmIntrinsicSourceFactV1 {
        kind: GeneralGemmIntrinsicSourceFactKindV1::NumericalOperationOrder,
        terminal_role_mask: role_bit(GeneralGemmIntrinsicRoleV1::Stage)
            | role_bit(GeneralGemmIntrinsicRoleV1::Mfma)
            | role_bit(GeneralGemmIntrinsicRoleV1::Store),
        semantic_component_mask: COMPONENT_LIFECYCLE
            | COMPONENT_ACCUMULATOR
            | COMPONENT_TAILS
            | COMPONENT_EPILOGUE
            | COMPONENT_NUMERICAL
            | COMPONENT_STORE,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmIntrinsicSemanticsV1 {
    version: u16,
    terminals: [GeneralGemmIntrinsicTerminalV1; 6],
    lane_tile_phase: GeneralGemmLaneTilePhaseSemanticsV1,
    allocation: GeneralGemmAllocationSemanticsV1,
    extents: GeneralGemmExtentSemanticsV1,
    guarded_regions: [GeneralGemmGuardedRegionSemanticsV1; 3],
    lds: GeneralGemmLdsSemanticsV1,
    lifecycle: GeneralGemmLdsLifecycleSemanticsV1,
    output_ownership: GeneralGemmOutputOwnershipSemanticsV1,
    store: GeneralGemmStoreSemanticsV1,
    accumulator: GeneralGemmAccumulatorSemanticsV1,
    tails: GeneralGemmTailSemanticsV1,
    epilogue: GeneralGemmEpilogueSemanticsV1,
    numerical_order: GeneralGemmNumericalOrderSemanticsV1,
    source_facts: [GeneralGemmIntrinsicSourceFactV1; 11],
    identity: GeneralGemmIntrinsicSemanticsIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneralGemmIntrinsicSemanticsErrorV1 {
    Version,
    Terminal(usize),
    LaneTilePhase,
    Allocation,
    Extents,
    GuardedRegions,
    Lds,
    Lifecycle,
    OutputOwnership,
    Store,
    Accumulator,
    Tails,
    Epilogue,
    NumericalOrder,
    SourceFact(usize),
    Identity,
}

impl GeneralGemmIntrinsicSemanticsV1 {
    pub(crate) fn canonical() -> Self {
        let mut semantics = Self::canonical_without_identity();
        semantics.identity = semantics.compute_identity();
        semantics
    }

    fn canonical_without_identity() -> Self {
        Self {
            version: 1,
            terminals: TERMINALS_V1,
            lane_tile_phase: LANE_TILE_PHASE_V1,
            allocation: ALLOCATION_V1,
            extents: EXTENTS_V1,
            guarded_regions: GUARDED_REGIONS_V1,
            lds: LDS_V1,
            lifecycle: LIFECYCLE_V1,
            output_ownership: OUTPUT_OWNERSHIP_V1,
            store: STORE_V1,
            accumulator: ACCUMULATOR_V1,
            tails: TAILS_V1,
            epilogue: EPILOGUE_V1,
            numerical_order: NUMERICAL_ORDER_V1,
            source_facts: SOURCE_FACTS_V1,
            identity: GeneralGemmIntrinsicSemanticsIdentityV1([0; 32]),
        }
    }

    pub(crate) const fn identity(&self) -> GeneralGemmIntrinsicSemanticsIdentityV1 {
        self.identity
    }

    pub(crate) const fn terminals(&self) -> &[GeneralGemmIntrinsicTerminalV1; 6] {
        &self.terminals
    }

    pub(crate) const fn source_facts(&self) -> &[GeneralGemmIntrinsicSourceFactV1; 11] {
        &self.source_facts
    }

    pub(crate) fn terminal(
        &self,
        role: GeneralGemmIntrinsicRoleV1,
    ) -> &GeneralGemmIntrinsicTerminalV1 {
        &self.terminals[(role as usize) - 1]
    }

    pub(crate) fn source_fact(
        &self,
        kind: GeneralGemmIntrinsicSourceFactKindV1,
    ) -> &GeneralGemmIntrinsicSourceFactV1 {
        &self.source_facts[(kind as usize) - 1]
    }

    pub(crate) const fn grants_source_correspondence(&self) -> bool {
        false
    }

    pub(crate) const fn grants_proof_or_execution_authority(&self) -> bool {
        false
    }

    pub(crate) fn validate(&self) -> Result<(), GeneralGemmIntrinsicSemanticsErrorV1> {
        let canonical = Self::canonical_without_identity();
        if self.version != canonical.version {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Version);
        }
        for (index, (actual, expected)) in
            self.terminals.iter().zip(canonical.terminals).enumerate()
        {
            if actual != &expected {
                return Err(GeneralGemmIntrinsicSemanticsErrorV1::Terminal(index));
            }
        }
        if self.lane_tile_phase != canonical.lane_tile_phase {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::LaneTilePhase);
        }
        if self.allocation != canonical.allocation {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Allocation);
        }
        if self.extents != canonical.extents {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Extents);
        }
        if self.guarded_regions != canonical.guarded_regions {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::GuardedRegions);
        }
        if self.lds != canonical.lds {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Lds);
        }
        if self.lifecycle != canonical.lifecycle {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Lifecycle);
        }
        if self.output_ownership != canonical.output_ownership {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::OutputOwnership);
        }
        if self.store != canonical.store {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Store);
        }
        if self.accumulator != canonical.accumulator {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Accumulator);
        }
        if self.tails != canonical.tails {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Tails);
        }
        if self.epilogue != canonical.epilogue {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Epilogue);
        }
        if self.numerical_order != canonical.numerical_order {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::NumericalOrder);
        }
        for (index, (actual, expected)) in self
            .source_facts
            .iter()
            .zip(canonical.source_facts)
            .enumerate()
        {
            if actual != &expected {
                return Err(GeneralGemmIntrinsicSemanticsErrorV1::SourceFact(index));
            }
        }
        if self.identity.0 == [0; 32] || self.identity != self.compute_identity() {
            return Err(GeneralGemmIntrinsicSemanticsErrorV1::Identity);
        }
        Ok(())
    }

    fn compute_identity(&self) -> GeneralGemmIntrinsicSemanticsIdentityV1 {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(SCHEMA_DOMAIN_V1);
        push_u16(&mut bytes, self.version);
        for terminal in self.terminals {
            push_u8(&mut bytes, terminal.role as u8);
            push_str(&mut bytes, terminal.diagnostic_item);
            push_str(&mut bytes, terminal.canonical_def_path);
            push_u8(&mut bytes, terminal.parameters.len() as u8);
            for parameter in terminal.parameters {
                push_u8(&mut bytes, parameter.name as u8);
                push_u8(&mut bytes, parameter.value_type as u8);
                push_u8(&mut bytes, parameter.passing as u8);
            }
            push_u8(&mut bytes, terminal.result as u8);
            push_bool(&mut bytes, terminal.unsafe_rust_fn);
            push_bool(&mut bytes, terminal.rust_call_abi);
            push_bool(&mut bytes, terminal.inline_never);
        }
        for extent in self.lane_tile_phase.workgroup {
            push_u16(&mut bytes, extent);
        }
        for extent in self.lane_tile_phase.tile {
            push_u16(&mut bytes, extent);
        }
        push_u8(&mut bytes, self.lane_tile_phase.components_per_lane);
        for formula in [
            self.lane_tile_phase.a_row_in_tile,
            self.lane_tile_phase.b_column_in_tile,
            self.lane_tile_phase.stage_depth,
            self.lane_tile_phase.output_row_in_tile,
            self.lane_tile_phase.output_column_in_tile,
            self.lane_tile_phase.phase_count,
        ] {
            push_u8(&mut bytes, formula as u8);
        }
        for value in [
            self.allocation.a_borrow_shared,
            self.allocation.b_borrow_shared,
            self.allocation.c_borrow_exclusive_disjoint,
            self.allocation.lds_regions_nonoverlapping,
            self.allocation.wave_capability_unique_linear,
        ] {
            push_bool(&mut bytes, value);
        }
        push_u8(&mut bytes, self.allocation.a_lds_region as u8);
        push_u8(&mut bytes, self.allocation.b_lds_region as u8);
        push_u16(&mut bytes, self.allocation.lds_region_bytes);
        push_u8(&mut bytes, self.allocation.lds_region_alignment);
        for value in [
            self.extents.empty_if_either_dimension_is_zero,
            self.extents
                .otherwise_checked_last_row_times_stride_plus_width,
            self.extents.nonempty_stride_at_least_logical_width,
            self.extents.a_slice_length_dominates_m_by_k_extent,
            self.extents.b_slice_length_dominates_k_by_n_extent,
            self.extents.c_disjoint_slice_length_dominates_m_by_n_extent,
            self.extents.failure_traps_before_acquire,
        ] {
            push_bool(&mut bytes, value);
        }
        for region in self.guarded_regions {
            push_u8(&mut bytes, region.region as u8);
            push_u8(&mut bytes, region.predicate as u8);
            push_u8(&mut bytes, region.stride as u8);
            push_bool(&mut bytes, region.checked_row_times_stride_plus_column);
            push_u8(&mut bytes, region.out_of_domain as u8);
        }
        push_u16(&mut bytes, self.lds.tile_rows);
        push_u16(&mut bytes, self.lds.tile_columns);
        push_u8(&mut bytes, self.lds.element_bytes);
        push_u8(&mut bytes, self.lds.layout as u8);
        for value in [
            self.lds.a_logical_axes_row_depth,
            self.lds.b_logical_axes_column_depth_transposed,
            self.lds.each_slot_has_one_writer,
            self.lds.mfma_reads_current_published_epoch_only,
        ] {
            push_bool(&mut bytes, value);
        }
        push_u8(&mut bytes, self.lds.writes_per_lane_per_tile);
        push_u16(&mut bytes, self.lds.writes_per_tile_per_phase);
        for transition in self.lifecycle.transitions {
            push_u8(&mut bytes, transition.role as u8);
            push_u8(&mut bytes, transition.from as u8);
            push_u8(&mut bytes, transition.to as u8);
            push_bool(&mut bytes, transition.convergent_wave_barrier);
            push_u8(&mut bytes, transition.epoch_increment);
        }
        for value in [
            self.lifecycle.all_lanes_execute_each_live_phase,
            self.lifecycle.publish_dominates_mfma_reads,
            self.lifecycle.reuse_postdominates_mfma_reads,
            self.lifecycle.next_stage_follows_reuse,
            self.lifecycle.store_requires_ready_at_phase_count,
        ] {
            push_bool(&mut bytes, value);
        }
        push_u8(&mut bytes, self.output_ownership.row_in_tile as u8);
        push_u8(&mut bytes, self.output_ownership.column_in_tile as u8);
        for value in [
            self.output_ownership.lane_component_injective,
            self.output_ownership.workgroup_tile_injective,
            self.output_ownership.valid_output_has_exactly_one_store,
        ] {
            push_bool(&mut bytes, value);
        }
        push_u8(&mut bytes, self.store.owner as u8);
        push_bool(
            &mut bytes,
            self.store.consumes_ready_at_completed_phase_count,
        );
        push_u8(&mut bytes, self.store.tile_row_origin as u8);
        push_u8(&mut bytes, self.store.tile_column_origin as u8);
        push_u8(&mut bytes, self.store.row_in_tile as u8);
        push_u8(&mut bytes, self.store.column_in_tile as u8);
        push_u8(&mut bytes, self.store.predicate as u8);
        push_u8(&mut bytes, self.store.stride as u8);
        for value in [
            self.store.checked_row_times_ldc_plus_column,
            self.store.index_must_be_within_disjoint_c_extent,
            self.store.out_of_domain_suppresses_prior_c_load_and_store,
            self.store.prior_c_and_result_use_same_owned_index,
        ] {
            push_bool(&mut bytes, value);
        }
        for operation in self.store.formula {
            push_u8(&mut bytes, operation as u8);
        }
        push_u8(&mut bytes, self.accumulator.components_per_lane);
        push_u32(&mut bytes, self.accumulator.initial_f32_bits);
        for value in [
            self.accumulator.initialized_by_acquire,
            self.accumulator.updated_by_each_mfma,
            self.accumulator.carried_across_reuse,
            self.accumulator.reset_between_phases,
        ] {
            push_bool(&mut bytes, value);
        }
        push_u8(&mut bytes, self.tails.phase_count as u8);
        push_u16(&mut bytes, self.tails.a_oob_bf16_bits);
        push_u16(&mut bytes, self.tails.b_oob_bf16_bits);
        push_bool(&mut bytes, self.tails.c_oob_suppresses_load_and_store);
        push_bool(&mut bytes, self.tails.zero_k_skips_all_phases);
        for operation in self.epilogue.operations {
            push_u8(&mut bytes, operation as u8);
        }
        for value in [
            self.epilogue.runtime_alpha,
            self.epilogue.runtime_beta,
            self.epilogue.prior_c_same_owned_coordinate,
            self.epilogue.separate_fp32_operations,
        ] {
            push_bool(&mut bytes, value);
        }
        for step in self.numerical_order.steps {
            push_u8(&mut bytes, step as u8);
        }
        push_bool(&mut bytes, self.numerical_order.increasing_phase_order);
        push_bool(
            &mut bytes,
            self.numerical_order
                .machine_mfma_confirmation_still_required,
        );
        for fact in self.source_facts {
            push_u8(&mut bytes, fact.kind as u8);
            push_u8(&mut bytes, fact.terminal_role_mask);
            push_u16(&mut bytes, fact.semantic_component_mask);
        }
        GeneralGemmIntrinsicSemanticsIdentityV1(Sha256::digest(bytes).into())
    }
}

pub(crate) const fn general_gemm_xor4_index_v1(row: u8, column: u8) -> Option<u16> {
    if row >= 16 || column >= 16 {
        return None;
    }
    Some((row as u16) * 16 + (column ^ ((row & 3) << 2)) as u16)
}

pub(crate) const fn general_gemm_lane_component_coordinates_v1(
    lane: u8,
    component: u8,
) -> Option<GeneralGemmLaneComponentCoordinatesV1> {
    if lane >= 64 || component >= 4 {
        return None;
    }
    let lane_quarter = lane / 16;
    let lane_modulo = lane % 16;
    let depth = lane_quarter * 4 + component;
    Some(GeneralGemmLaneComponentCoordinatesV1 {
        a_row: lane_modulo,
        b_column: lane_modulo,
        phase_depth: depth,
        output_row: lane_quarter * 4 + component,
        output_column: lane_modulo,
        a_lds_slot: match general_gemm_xor4_index_v1(lane_modulo, depth) {
            Some(slot) => slot,
            None => return None,
        },
        b_lds_slot: match general_gemm_xor4_index_v1(lane_modulo, depth) {
            Some(slot) => slot,
            None => return None,
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmLaneComponentCoordinatesV1 {
    pub(crate) a_row: u8,
    pub(crate) b_column: u8,
    pub(crate) phase_depth: u8,
    pub(crate) output_row: u8,
    pub(crate) output_column: u8,
    pub(crate) a_lds_slot: u16,
    pub(crate) b_lds_slot: u16,
}

fn push_bool(bytes: &mut Vec<u8>, value: bool) {
    bytes.push(value as u8);
}

fn push_u8(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_str(bytes: &mut Vec<u8>, value: &str) {
    push_u32(bytes, value.len() as u32);
    bytes.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_after_mutation(
        mut semantics: GeneralGemmIntrinsicSemanticsV1,
    ) -> GeneralGemmIntrinsicSemanticsIdentityV1 {
        semantics.identity = semantics.compute_identity();
        semantics.identity
    }

    #[test]
    fn canonical_schema_is_nonzero_closed_and_non_authoritative() {
        let semantics = GeneralGemmIntrinsicSemanticsV1::canonical();
        assert_eq!(semantics.validate(), Ok(()));
        assert_ne!(semantics.identity().as_bytes(), [0; 32]);
        assert_eq!(
            semantics.terminals().map(|terminal| terminal.role),
            TERMINAL_ROLES_V1
        );
        assert_eq!(
            semantics.source_facts().map(|fact| fact.kind),
            SOURCE_FACT_KINDS_V1
        );
        assert!(!semantics.grants_source_correspondence());
        assert!(!semantics.grants_proof_or_execution_authority());
    }

    #[test]
    fn every_terminal_role_path_and_abi_substitution_is_rejected() {
        let canonical = GeneralGemmIntrinsicSemanticsV1::canonical();
        for index in 0..canonical.terminals.len() {
            let mut changed_role = canonical;
            changed_role.terminals[index].role = GeneralGemmIntrinsicRoleV1::Store;
            if index != 5 {
                assert_eq!(
                    changed_role.validate(),
                    Err(GeneralGemmIntrinsicSemanticsErrorV1::Terminal(index))
                );
                assert_ne!(identity_after_mutation(changed_role), canonical.identity());
            }

            let mut changed_path = canonical;
            changed_path.terminals[index].canonical_def_path = "substituted::terminal";
            assert_eq!(
                changed_path.validate(),
                Err(GeneralGemmIntrinsicSemanticsErrorV1::Terminal(index))
            );
            assert_ne!(identity_after_mutation(changed_path), canonical.identity());

            let mut changed_abi = canonical;
            changed_abi.terminals[index].result = GeneralGemmIntrinsicValueTypeV1::Unit;
            if canonical.terminals[index].result != GeneralGemmIntrinsicValueTypeV1::Unit {
                assert_eq!(
                    changed_abi.validate(),
                    Err(GeneralGemmIntrinsicSemanticsErrorV1::Terminal(index))
                );
                assert_ne!(identity_after_mutation(changed_abi), canonical.identity());
            }
        }

        let mut changed_store_borrow = canonical;
        changed_store_borrow.terminals[5].parameters = &STAGE_PARAMETERS_V1;
        assert_eq!(
            changed_store_borrow.validate(),
            Err(GeneralGemmIntrinsicSemanticsErrorV1::Terminal(5))
        );
        assert_ne!(
            identity_after_mutation(changed_store_borrow),
            canonical.identity()
        );
    }

    #[test]
    fn lane_mapping_covers_each_lds_tile_and_output_exactly_once() {
        let mut a_slots = [false; 256];
        let mut b_slots = [false; 256];
        let mut outputs = [false; 256];
        for lane in 0..64 {
            for component in 0..4 {
                let coordinates =
                    general_gemm_lane_component_coordinates_v1(lane, component).unwrap();
                assert!(!a_slots[usize::from(coordinates.a_lds_slot)]);
                assert!(!b_slots[usize::from(coordinates.b_lds_slot)]);
                a_slots[usize::from(coordinates.a_lds_slot)] = true;
                b_slots[usize::from(coordinates.b_lds_slot)] = true;
                let output = usize::from(coordinates.output_row) * 16
                    + usize::from(coordinates.output_column);
                assert!(!outputs[output]);
                outputs[output] = true;
            }
        }
        assert!(a_slots.into_iter().all(|written| written));
        assert!(b_slots.into_iter().all(|written| written));
        assert!(outputs.into_iter().all(|owned| owned));
        assert_eq!(general_gemm_lane_component_coordinates_v1(64, 0), None);
        assert_eq!(general_gemm_lane_component_coordinates_v1(0, 4), None);
        assert_eq!(general_gemm_xor4_index_v1(16, 0), None);
        assert_eq!(general_gemm_xor4_index_v1(0, 16), None);
    }

    #[test]
    fn source_facts_bind_all_cross_cutting_semantic_dependencies() {
        let semantics = GeneralGemmIntrinsicSemanticsV1::canonical();
        let guarded =
            semantics.source_fact(GeneralGemmIntrinsicSourceFactKindV1::GuardedGlobalAccesses);
        assert_eq!(
            guarded.semantic_component_mask
                & (COMPONENT_ALLOCATION | COMPONENT_EXTENTS | COMPONENT_STORE),
            COMPONENT_ALLOCATION | COMPONENT_EXTENTS | COMPONENT_STORE
        );
        let conflicts =
            semantics.source_fact(GeneralGemmIntrinsicSourceFactKindV1::EffectConflictFreedom);
        assert_eq!(
            conflicts.semantic_component_mask & (COMPONENT_ALLOCATION | COMPONENT_LANE_TILE_PHASE),
            COMPONENT_ALLOCATION | COMPONENT_LANE_TILE_PHASE
        );
        let epilogue =
            semantics.source_fact(GeneralGemmIntrinsicSourceFactKindV1::AlphaBetaEpilogue);
        assert_eq!(
            epilogue.semantic_component_mask & (COMPONENT_GUARDS | COMPONENT_TAILS),
            COMPONENT_GUARDS | COMPONENT_TAILS
        );
        let numerical =
            semantics.source_fact(GeneralGemmIntrinsicSourceFactKindV1::NumericalOperationOrder);
        assert_ne!(numerical.semantic_component_mask & COMPONENT_LIFECYCLE, 0);

        assert_eq!(semantics.store.owner, GeneralGemmIntrinsicRoleV1::Store);
        assert!(semantics.store.checked_row_times_ldc_plus_column);
        assert!(semantics.store.index_must_be_within_disjoint_c_extent);
        assert!(
            semantics
                .store
                .out_of_domain_suppresses_prior_c_load_and_store
        );
        assert_eq!(
            semantics.store.predicate,
            GeneralGemmGuardPredicateV1::CRowLessMAndColumnLessN
        );
        assert_eq!(semantics.store.formula, EPILOGUE_V1.operations);
    }

    #[test]
    fn each_semantic_section_and_property_fact_is_identity_bound_and_rejected() {
        let canonical = GeneralGemmIntrinsicSemanticsV1::canonical();
        let mut mutations: Vec<(
            GeneralGemmIntrinsicSemanticsV1,
            GeneralGemmIntrinsicSemanticsErrorV1,
        )> = Vec::new();

        let mut lane = canonical;
        lane.lane_tile_phase.workgroup[0] = 32;
        mutations.push((lane, GeneralGemmIntrinsicSemanticsErrorV1::LaneTilePhase));
        let mut allocation = canonical;
        allocation.allocation.lds_regions_nonoverlapping = false;
        mutations.push((allocation, GeneralGemmIntrinsicSemanticsErrorV1::Allocation));
        let mut extents = canonical;
        extents.extents.failure_traps_before_acquire = false;
        mutations.push((extents, GeneralGemmIntrinsicSemanticsErrorV1::Extents));
        let mut guard = canonical;
        guard.guarded_regions[0].predicate = GeneralGemmGuardPredicateV1::BDepthLessKAndColumnLessN;
        mutations.push((guard, GeneralGemmIntrinsicSemanticsErrorV1::GuardedRegions));
        let mut lds = canonical;
        lds.lds.each_slot_has_one_writer = false;
        mutations.push((lds, GeneralGemmIntrinsicSemanticsErrorV1::Lds));
        let mut lifecycle = canonical;
        lifecycle.lifecycle.transitions[1].convergent_wave_barrier = false;
        mutations.push((lifecycle, GeneralGemmIntrinsicSemanticsErrorV1::Lifecycle));
        let mut output = canonical;
        output.output_ownership.lane_component_injective = false;
        mutations.push((
            output,
            GeneralGemmIntrinsicSemanticsErrorV1::OutputOwnership,
        ));
        let mut store = canonical;
        store.store.checked_row_times_ldc_plus_column = false;
        mutations.push((store, GeneralGemmIntrinsicSemanticsErrorV1::Store));
        let mut accumulator = canonical;
        accumulator.accumulator.reset_between_phases = true;
        mutations.push((
            accumulator,
            GeneralGemmIntrinsicSemanticsErrorV1::Accumulator,
        ));
        let mut tails = canonical;
        tails.tails.a_oob_bf16_bits = 0x8000;
        mutations.push((tails, GeneralGemmIntrinsicSemanticsErrorV1::Tails));
        let mut epilogue = canonical;
        epilogue.epilogue.operations.swap(0, 1);
        mutations.push((epilogue, GeneralGemmIntrinsicSemanticsErrorV1::Epilogue));
        let mut numerical = canonical;
        numerical.numerical_order.steps.swap(1, 2);
        mutations.push((
            numerical,
            GeneralGemmIntrinsicSemanticsErrorV1::NumericalOrder,
        ));

        for (mutated, expected_error) in mutations {
            assert_eq!(mutated.validate(), Err(expected_error));
            assert_ne!(identity_after_mutation(mutated), canonical.identity());
        }

        for index in 0..canonical.source_facts.len() {
            let mut mutated = canonical;
            mutated.source_facts[index].terminal_role_mask ^=
                role_bit(GeneralGemmIntrinsicRoleV1::Acquire);
            assert_eq!(
                mutated.validate(),
                Err(GeneralGemmIntrinsicSemanticsErrorV1::SourceFact(index))
            );
            assert_ne!(identity_after_mutation(mutated), canonical.identity());
        }
    }

    #[test]
    fn stored_identity_cannot_be_replaced_or_reused_after_mutation() {
        let canonical = GeneralGemmIntrinsicSemanticsV1::canonical();
        let mut zero = canonical;
        zero.identity = GeneralGemmIntrinsicSemanticsIdentityV1([0; 32]);
        assert_eq!(
            zero.validate(),
            Err(GeneralGemmIntrinsicSemanticsErrorV1::Identity)
        );

        let mut semantic_substitution = canonical;
        semantic_substitution.tails.b_oob_bf16_bits = 0x8000;
        assert_eq!(
            semantic_substitution.validate(),
            Err(GeneralGemmIntrinsicSemanticsErrorV1::Tails)
        );
        semantic_substitution.identity = semantic_substitution.compute_identity();
        assert_ne!(semantic_substitution.identity(), canonical.identity());
        assert_eq!(
            semantic_substitution.validate(),
            Err(GeneralGemmIntrinsicSemanticsErrorV1::Tails)
        );
    }
}
