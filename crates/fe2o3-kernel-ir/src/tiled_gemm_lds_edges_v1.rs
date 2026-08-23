//! Canonical bounded LDS-tiled GEMM Slice 4 for one exact gfx942 edge case.
//!
//! This module seals the positive `M=17`, `N=19`, `K=18` representative of
//! the committed Slice 4 edge proof. A `2x2` grid of wave64/workgroup64
//! workgroups executes two K=16 phases. Each phase conditionally loads valid A
//! and B elements and otherwise stages exact BF16 zero through two reused,
//! aligned 512-byte XOR4 LDS tiles. Every load path reconverges before the
//! unconditional publish barrier, and every lane reaches an unconditional
//! reuse barrier before the loop backedge. Four FP32 accumulators are carried
//! by the loop. C input loads and output stores share the exact M/N predicate,
//! with explicit FP32 constants `alpha=2.0` and `beta=-1.0`.
//!
//! Exact graph equality is a closed Kernel IR admission boundary. The core IR
//! verifier checks typed CFG and operation invariants but does not independently
//! prove predicate bounds or convergence; the exhaustive helpers and tests bind
//! this graph to the committed edge model. This module makes no source,
//! lowering, runtime, hardware, IEEE-754, numerical-refinement, or artifact
//! identity claim. Runtime slice lengths remain a later host-binding obligation.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, BasicBlock, BinaryOp, BlockId,
    ComparePredicate, Constant, Convergence, Function, IndexKind, IntrinsicKind,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MatrixElement, MatrixLayout,
    MatrixOperation, MemoryOrdering, Operation, OperationKind, ScalarType, Signature,
    SynchronizationScope, TargetCapability, TensorLayoutContractV1, Terminator, Type, ValueDef,
    ValueId, VerificationErrors, WaveWidth, WorkgroupBarrier, WorkgroupMemory,
    WorkgroupMemoryExtent, WorkgroupSize, gfx942_xnack_minus_target_capability, verify_module,
};

pub const TILED_GEMM_LDS_EDGES_V1_MODULE_ID: &str = "fe2o3::tiled_gemm_lds_edges_v1";
pub const TILED_GEMM_LDS_EDGES_V1_FUNCTION_ID: &str = "__fe2o3_tiled_gemm_lds_edges_v1_impl";
pub const TILED_GEMM_LDS_EDGES_V1_KERNEL_ID: &str = "tiled_gemm_lds_edges_v1";
pub const TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT: u32 = 16;
pub const TILED_GEMM_LDS_EDGES_V1_M: u32 = 17;
pub const TILED_GEMM_LDS_EDGES_V1_N: u32 = 19;
pub const TILED_GEMM_LDS_EDGES_V1_K: u32 = 18;
pub const TILED_GEMM_LDS_EDGES_V1_PHASES: u32 = 2;
pub const TILED_GEMM_LDS_EDGES_V1_PHASE_K: u32 = 16;
pub const TILED_GEMM_LDS_EDGES_V1_LANES: u32 = 64;
pub const TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS: u32 = 4;
pub const TILED_GEMM_LDS_EDGES_V1_TILE_ROWS: u32 = 2;
pub const TILED_GEMM_LDS_EDGES_V1_TILE_COLUMNS: u32 = 2;
pub const TILED_GEMM_LDS_EDGES_V1_WORKGROUPS: u32 = 4;
pub const TILED_GEMM_LDS_EDGES_V1_LAUNCH_EXTENT_X: u32 =
    TILED_GEMM_LDS_EDGES_V1_TILE_COLUMNS * TILED_GEMM_LDS_EDGES_V1_LANES;
pub const TILED_GEMM_LDS_EDGES_V1_LAUNCH_EXTENT_Y: u32 = TILED_GEMM_LDS_EDGES_V1_TILE_ROWS;
pub const TILED_GEMM_LDS_EDGES_V1_A_ELEMENTS: u32 =
    TILED_GEMM_LDS_EDGES_V1_M * TILED_GEMM_LDS_EDGES_V1_K;
pub const TILED_GEMM_LDS_EDGES_V1_B_ELEMENTS: u32 =
    TILED_GEMM_LDS_EDGES_V1_K * TILED_GEMM_LDS_EDGES_V1_N;
pub const TILED_GEMM_LDS_EDGES_V1_C_ELEMENTS: u32 =
    TILED_GEMM_LDS_EDGES_V1_M * TILED_GEMM_LDS_EDGES_V1_N;
pub const TILED_GEMM_LDS_EDGES_V1_A_BYTES: u32 = TILED_GEMM_LDS_EDGES_V1_A_ELEMENTS * 2;
pub const TILED_GEMM_LDS_EDGES_V1_B_BYTES: u32 = TILED_GEMM_LDS_EDGES_V1_B_ELEMENTS * 2;
pub const TILED_GEMM_LDS_EDGES_V1_C_BYTES: u32 = TILED_GEMM_LDS_EDGES_V1_C_ELEMENTS * 4;
pub const TILED_GEMM_LDS_EDGES_V1_TILE_ELEMENTS: u32 = 256;
pub const TILED_GEMM_LDS_EDGES_V1_ALLOCATION_COUNT: u32 = 2;
pub const TILED_GEMM_LDS_EDGES_V1_TILE_BYTES: u32 = 512;
pub const TILED_GEMM_LDS_EDGES_V1_STATIC_LDS_BYTES: u32 = 1_024;
pub const TILED_GEMM_LDS_EDGES_V1_LDS_ALIGNMENT: u32 = 16;
pub const TILED_GEMM_LDS_EDGES_V1_ALPHA_BITS: u32 = 2.0f32.to_bits();
pub const TILED_GEMM_LDS_EDGES_V1_BETA_BITS: u32 = (-1.0f32).to_bits();

/// Closed admission profile for the exact positive Slice 4 representative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TiledGemmLdsEdgesV1Profile {
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub alpha_bits: u32,
    pub beta_bits: u32,
    pub a_elements: u32,
    pub b_elements: u32,
    pub c_elements: u32,
    pub a_bytes: u32,
    pub b_bytes: u32,
    pub c_bytes: u32,
    pub tile_rows: u32,
    pub tile_columns: u32,
    pub depth_tiles: u32,
    pub phase_k: u32,
    pub workgroup_count: u32,
    pub wave_width: WaveWidth,
    pub launch_extent_x: u32,
    pub launch_extent_y: u32,
    pub workgroup_size: WorkgroupSize,
    pub lds_allocations: u32,
    pub lds_elements_per_allocation: u32,
    pub lds_bytes_per_allocation: u32,
    pub static_lds_bytes: u32,
    pub lds_alignment: u32,
    pub lds_layout: MatrixLayout,
    pub output_elements_per_lane: u32,
}

impl TiledGemmLdsEdgesV1Profile {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        Self {
            target: gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            m: TILED_GEMM_LDS_EDGES_V1_M,
            n: TILED_GEMM_LDS_EDGES_V1_N,
            k: TILED_GEMM_LDS_EDGES_V1_K,
            alpha_bits: TILED_GEMM_LDS_EDGES_V1_ALPHA_BITS,
            beta_bits: TILED_GEMM_LDS_EDGES_V1_BETA_BITS,
            a_elements: TILED_GEMM_LDS_EDGES_V1_A_ELEMENTS,
            b_elements: TILED_GEMM_LDS_EDGES_V1_B_ELEMENTS,
            c_elements: TILED_GEMM_LDS_EDGES_V1_C_ELEMENTS,
            a_bytes: TILED_GEMM_LDS_EDGES_V1_A_BYTES,
            b_bytes: TILED_GEMM_LDS_EDGES_V1_B_BYTES,
            c_bytes: TILED_GEMM_LDS_EDGES_V1_C_BYTES,
            tile_rows: TILED_GEMM_LDS_EDGES_V1_TILE_ROWS,
            tile_columns: TILED_GEMM_LDS_EDGES_V1_TILE_COLUMNS,
            depth_tiles: TILED_GEMM_LDS_EDGES_V1_PHASES,
            phase_k: TILED_GEMM_LDS_EDGES_V1_PHASE_K,
            workgroup_count: TILED_GEMM_LDS_EDGES_V1_WORKGROUPS,
            wave_width: WaveWidth::Wave64,
            launch_extent_x: TILED_GEMM_LDS_EDGES_V1_LAUNCH_EXTENT_X,
            launch_extent_y: TILED_GEMM_LDS_EDGES_V1_LAUNCH_EXTENT_Y,
            workgroup_size: WorkgroupSize::new(TILED_GEMM_LDS_EDGES_V1_LANES, 1, 1),
            lds_allocations: TILED_GEMM_LDS_EDGES_V1_ALLOCATION_COUNT,
            lds_elements_per_allocation: TILED_GEMM_LDS_EDGES_V1_TILE_ELEMENTS,
            lds_bytes_per_allocation: TILED_GEMM_LDS_EDGES_V1_TILE_BYTES,
            static_lds_bytes: TILED_GEMM_LDS_EDGES_V1_STATIC_LDS_BYTES,
            lds_alignment: TILED_GEMM_LDS_EDGES_V1_LDS_ALIGNMENT,
            lds_layout: MatrixLayout::RowMajorXor4,
            output_elements_per_lane: TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS,
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TiledGemmLdsEdgesV1Error {
    UnsupportedProfile,
    InvalidKernelIr(VerificationErrors),
    NonCanonicalKernelIr,
}

impl fmt::Display for TiledGemmLdsEdgesV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "tiled GEMM LDS edges V1 requires the exact gfx942:xnack-/COV6 M=17, N=19, K=18 profile, alpha=2.0, beta=-1.0, a 2x2 wave64/workgroup64 grid, two reused aligned 512-byte XOR4 LDS tiles, two predicated K=16 phases, carried FP32 accumulators, unconditional publish/reuse barriers, and predicated C read/write ownership",
            ),
            Self::InvalidKernelIr(error) => error.fmt(formatter),
            Self::NonCanonicalKernelIr => {
                formatter.write_str("kernel IR does not match canonical tiled GEMM LDS edges V1")
            }
        }
    }
}

impl Error for TiledGemmLdsEdgesV1Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidKernelIr(error) => Some(error),
            _ => None,
        }
    }
}

/// A physical tile coordinate classified against one logical row-major operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TiledGemmLdsEdgesV1Coordinate {
    Valid { row: u32, column: u32, index: u32 },
    Tail { row: u32, column: u32 },
}

impl TiledGemmLdsEdgesV1Coordinate {
    pub const fn row(self) -> u32 {
        match self {
            Self::Valid { row, .. } | Self::Tail { row, .. } => row,
        }
    }

    pub const fn column(self) -> u32 {
        match self {
            Self::Valid { column, .. } | Self::Tail { column, .. } => column,
        }
    }

    pub const fn index(self) -> Option<u32> {
        match self {
            Self::Valid { index, .. } => Some(index),
            Self::Tail { .. } => None,
        }
    }

    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid { .. })
    }
}

/// One offset in the two physical K phases, classified against `K=18`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TiledGemmLdsEdgesV1Depth {
    Valid { depth: u32 },
    Tail { depth: u32 },
}

impl TiledGemmLdsEdgesV1Depth {
    pub const fn depth(self) -> u32 {
        match self {
            Self::Valid { depth } | Self::Tail { depth } => depth,
        }
    }

    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TiledGemmLdsEdgesV1Barrier {
    Publish,
    Reuse,
}

/// Logical tile-local line/depth staged by one physical lane/component.
pub const fn tiled_gemm_lds_edges_v1_staging_coordinate(
    lane: u32,
    component: u32,
) -> Option<(u32, u32)> {
    if lane >= TILED_GEMM_LDS_EDGES_V1_LANES
        || component >= TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS
    {
        return None;
    }
    Some((
        lane % TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT,
        TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS * (lane / TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT)
            + component,
    ))
}

/// Logical tile-local `(row, column)` accumulator owned by a lane/component.
pub const fn tiled_gemm_lds_edges_v1_output_coordinate(
    lane: u32,
    component: u32,
) -> Option<(u32, u32)> {
    let Some((column, row)) = tiled_gemm_lds_edges_v1_staging_coordinate(lane, component) else {
        return None;
    };
    Some((row, column))
}

/// Checked origin of one of the four admitted output workgroups.
pub const fn tiled_gemm_lds_edges_v1_tile_origin(group_x: u32, group_y: u32) -> Option<(u32, u32)> {
    if group_x >= TILED_GEMM_LDS_EDGES_V1_TILE_COLUMNS
        || group_y >= TILED_GEMM_LDS_EDGES_V1_TILE_ROWS
    {
        return None;
    }
    Some((
        group_y * TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT,
        group_x * TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT,
    ))
}

/// Classifies one physical phase offset as a valid K depth or zero-filled tail.
pub const fn tiled_gemm_lds_edges_v1_depth(
    phase: u32,
    tile_depth: u32,
) -> Option<TiledGemmLdsEdgesV1Depth> {
    if phase >= TILED_GEMM_LDS_EDGES_V1_PHASES || tile_depth >= TILED_GEMM_LDS_EDGES_V1_PHASE_K {
        return None;
    }
    let depth = phase * TILED_GEMM_LDS_EDGES_V1_PHASE_K + tile_depth;
    if depth < TILED_GEMM_LDS_EDGES_V1_K {
        Some(TiledGemmLdsEdgesV1Depth::Valid { depth })
    } else {
        Some(TiledGemmLdsEdgesV1Depth::Tail { depth })
    }
}

const fn classify_row_major(
    row: u32,
    column: u32,
    rows: u32,
    columns: u32,
) -> TiledGemmLdsEdgesV1Coordinate {
    if row >= rows || column >= columns {
        return TiledGemmLdsEdgesV1Coordinate::Tail { row, column };
    }
    let index = row * columns + column;
    TiledGemmLdsEdgesV1Coordinate::Valid { row, column, index }
}

/// Exact A `(row, depth)` coordinate for one admitted physical staging slot.
pub const fn tiled_gemm_lds_edges_v1_a_coordinate(
    group_y: u32,
    phase: u32,
    lane: u32,
    component: u32,
) -> Option<TiledGemmLdsEdgesV1Coordinate> {
    if group_y >= TILED_GEMM_LDS_EDGES_V1_TILE_ROWS || phase >= TILED_GEMM_LDS_EDGES_V1_PHASES {
        return None;
    }
    let Some((tile_row, tile_depth)) = tiled_gemm_lds_edges_v1_staging_coordinate(lane, component)
    else {
        return None;
    };
    Some(classify_row_major(
        group_y * TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT + tile_row,
        phase * TILED_GEMM_LDS_EDGES_V1_PHASE_K + tile_depth,
        TILED_GEMM_LDS_EDGES_V1_M,
        TILED_GEMM_LDS_EDGES_V1_K,
    ))
}

/// Exact B `(depth, column)` coordinate for one admitted physical staging slot.
pub const fn tiled_gemm_lds_edges_v1_b_coordinate(
    group_x: u32,
    phase: u32,
    lane: u32,
    component: u32,
) -> Option<TiledGemmLdsEdgesV1Coordinate> {
    if group_x >= TILED_GEMM_LDS_EDGES_V1_TILE_COLUMNS || phase >= TILED_GEMM_LDS_EDGES_V1_PHASES {
        return None;
    }
    let Some((tile_column, tile_depth)) =
        tiled_gemm_lds_edges_v1_staging_coordinate(lane, component)
    else {
        return None;
    };
    Some(classify_row_major(
        phase * TILED_GEMM_LDS_EDGES_V1_PHASE_K + tile_depth,
        group_x * TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT + tile_column,
        TILED_GEMM_LDS_EDGES_V1_K,
        TILED_GEMM_LDS_EDGES_V1_N,
    ))
}

/// Exact C `(row, column)` coordinate shared by input load and output store.
pub const fn tiled_gemm_lds_edges_v1_c_coordinate(
    group_x: u32,
    group_y: u32,
    lane: u32,
    component: u32,
) -> Option<TiledGemmLdsEdgesV1Coordinate> {
    let Some((origin_row, origin_column)) = tiled_gemm_lds_edges_v1_tile_origin(group_x, group_y)
    else {
        return None;
    };
    let Some((tile_row, tile_column)) = tiled_gemm_lds_edges_v1_output_coordinate(lane, component)
    else {
        return None;
    };
    Some(classify_row_major(
        origin_row + tile_row,
        origin_column + tile_column,
        TILED_GEMM_LDS_EDGES_V1_M,
        TILED_GEMM_LDS_EDGES_V1_N,
    ))
}

pub const fn tiled_gemm_lds_edges_v1_a_index(
    group_y: u32,
    phase: u32,
    lane: u32,
    component: u32,
) -> Option<u32> {
    match tiled_gemm_lds_edges_v1_a_coordinate(group_y, phase, lane, component) {
        Some(coordinate) => coordinate.index(),
        None => None,
    }
}

pub const fn tiled_gemm_lds_edges_v1_b_index(
    group_x: u32,
    phase: u32,
    lane: u32,
    component: u32,
) -> Option<u32> {
    match tiled_gemm_lds_edges_v1_b_coordinate(group_x, phase, lane, component) {
        Some(coordinate) => coordinate.index(),
        None => None,
    }
}

pub const fn tiled_gemm_lds_edges_v1_c_index(
    group_x: u32,
    group_y: u32,
    lane: u32,
    component: u32,
) -> Option<u32> {
    match tiled_gemm_lds_edges_v1_c_coordinate(group_x, group_y, lane, component) {
        Some(coordinate) => coordinate.index(),
        None => None,
    }
}

/// Physical index for the exact row-major XOR4 LDS layout.
pub const fn tiled_gemm_lds_edges_v1_xor4_index(row: u32, column: u32) -> Option<u32> {
    if row >= TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT || column >= TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT {
        return None;
    }
    Some(
        row * TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT
            + (column ^ ((row & 3) * TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS)),
    )
}

/// Physical LDS cell always written by a lane/component, with zero for tails.
pub const fn tiled_gemm_lds_edges_v1_lds_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_lds_edges_v1_staging_coordinate(lane, component) {
        Some((row, column)) => tiled_gemm_lds_edges_v1_xor4_index(row, column),
        None => None,
    }
}

/// Both physical barriers are reached by every admitted lane in every phase.
pub const fn tiled_gemm_lds_edges_v1_lane_reaches_barrier(
    phase: u32,
    lane: u32,
    _barrier: TiledGemmLdsEdgesV1Barrier,
) -> bool {
    phase < TILED_GEMM_LDS_EDGES_V1_PHASES && lane < TILED_GEMM_LDS_EDGES_V1_LANES
}

/// Constructs the only Kernel IR graph admitted by bounded LDS GEMM Slice 4.
pub fn tiled_gemm_lds_edges_v1_module() -> crate::Module {
    const PREDICATED_LOADS: usize = 8;
    const OUTPUT_BLOCK: BlockId = BlockId(19);

    let bf16 = Type::Scalar(ScalarType::Bf16);
    let read_bf16_slice = Type::slice(bf16.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let read_write_f32_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let read_bf16_pointer = Type::pointer(bf16.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let read_write_f32_pointer =
        Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);

    let mut graph = GraphBuilder::new(3);
    let a_base = graph.value(
        read_bf16_pointer.clone(),
        OperationKind::SliceData { slice: ValueId(0) },
    );
    let b_base = graph.value(
        read_bf16_pointer,
        OperationKind::SliceData { slice: ValueId(1) },
    );
    let c_base = graph.value(
        read_write_f32_pointer,
        OperationKind::SliceData { slice: ValueId(2) },
    );
    let a_lds = graph.static_bf16_lds_tile();
    let b_lds = graph.static_bf16_lds_tile();
    let lane = graph.invocation_index(IndexKind::Local, Axis::X);
    let group_x = graph.invocation_index(IndexKind::Workgroup, Axis::X);
    let group_y = graph.invocation_index(IndexKind::Workgroup, Axis::Y);

    let tile_extent = graph.index_constant(TILED_GEMM_LDS_EDGES_V1_TILE_EXTENT);
    let fragment_elements = graph.index_constant(TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS);
    let m = graph.index_constant(TILED_GEMM_LDS_EDGES_V1_M);
    let n = graph.index_constant(TILED_GEMM_LDS_EDGES_V1_N);
    let k = graph.index_constant(TILED_GEMM_LDS_EDGES_V1_K);
    let phase_zero = graph.index_constant(0);
    let phase_count = graph.index_constant(TILED_GEMM_LDS_EDGES_V1_PHASES);
    let phase_step = graph.index_constant(1);
    let component_constants = [
        graph.index_constant(0),
        graph.index_constant(1),
        graph.index_constant(2),
        graph.index_constant(3),
    ];
    let bf16_zero = graph.value(bf16.clone(), OperationKind::Constant(Constant::Bf16Bits(0)));
    let f32_zero = graph.value(Type::F32, OperationKind::Constant(Constant::F32Bits(0)));
    let alpha = graph.value(
        Type::F32,
        OperationKind::Constant(Constant::F32Bits(TILED_GEMM_LDS_EDGES_V1_ALPHA_BITS)),
    );
    let beta = graph.value(
        Type::F32,
        OperationKind::Constant(Constant::F32Bits(TILED_GEMM_LDS_EDGES_V1_BETA_BITS)),
    );

    let lane_line = graph.index_binary(BinaryOp::Remainder, lane, tile_extent);
    let lane_quad = graph.index_binary(BinaryOp::Divide, lane, tile_extent);
    let local_depth_base = graph.index_binary(BinaryOp::Multiply, lane_quad, fragment_elements);
    let origin_row = graph.index_binary(BinaryOp::Multiply, group_y, tile_extent);
    let origin_column = graph.index_binary(BinaryOp::Multiply, group_x, tile_extent);
    let a_row = graph.index_binary(BinaryOp::Add, origin_row, lane_line);
    let b_column = graph.index_binary(BinaryOp::Add, origin_column, lane_line);
    let a_row_base = graph.index_binary(BinaryOp::Multiply, a_row, k);
    let a_row_valid = graph.compare(ComparePredicate::LessThan, a_row, m);
    let b_column_valid = graph.compare(ComparePredicate::LessThan, b_column, n);

    let mut local_depths = Vec::with_capacity(TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS as usize);
    let mut c_indices = Vec::with_capacity(TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS as usize);
    let mut c_enabled = Vec::with_capacity(TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS as usize);
    for component in component_constants {
        let local_depth = graph.index_binary(BinaryOp::Add, local_depth_base, component);
        let c_row = graph.index_binary(BinaryOp::Add, origin_row, local_depth);
        let c_row_base = graph.index_binary(BinaryOp::Multiply, c_row, n);
        let c_index = graph.index_binary(BinaryOp::Add, c_row_base, b_column);
        let c_row_valid = graph.compare(ComparePredicate::LessThan, c_row, m);
        local_depths.push(local_depth);
        c_indices.push(c_index);
        c_enabled.push(graph.bool_and(c_row_valid, b_column_valid));
    }
    let local_depths: [ValueId; 4] = local_depths.try_into().expect("four local depths");
    let c_indices: [ValueId; 4] = c_indices.try_into().expect("four C indices");
    let c_enabled: [ValueId; 4] = c_enabled.try_into().expect("four C predicates");

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = graph.take_operations();
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![phase_zero, f32_zero, f32_zero, f32_zero, f32_zero],
    });

    let phase = graph.block_parameter(Type::INDEX);
    let accumulators = [
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
    ];
    let accumulator_ids = [
        accumulators[0].id,
        accumulators[1].id,
        accumulators[2].id,
        accumulators[3].id,
    ];
    let phase_active = graph.compare(ComparePredicate::LessThan, phase.id, phase_count);
    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = std::iter::once(phase.clone())
        .chain(accumulators.iter().cloned())
        .collect();
    header.operations = graph.take_operations();
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: phase_active,
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: OUTPUT_BLOCK,
        else_arguments: accumulators.iter().map(|value| value.id).collect(),
    });

    let phase_depth_base = graph.index_binary(BinaryOp::Multiply, phase.id, tile_extent);
    let mut load_specs = Vec::with_capacity(PREDICATED_LOADS);
    for local_depth in local_depths {
        let depth = graph.index_binary(BinaryOp::Add, phase_depth_base, local_depth);
        let depth_valid = graph.compare(ComparePredicate::LessThan, depth, k);
        let a_index = graph.index_binary(BinaryOp::Add, a_row_base, depth);
        let b_row_base = graph.index_binary(BinaryOp::Multiply, depth, n);
        let b_index = graph.index_binary(BinaryOp::Add, b_row_base, b_column);
        load_specs.push(PredicatedLoad {
            condition: graph.bool_and(a_row_valid, depth_valid),
            base: a_base,
            offset: a_index,
        });
        load_specs.push(PredicatedLoad {
            condition: graph.bool_and(depth_valid, b_column_valid),
            base: b_base,
            offset: b_index,
        });
    }
    let load_specs = [
        load_specs[0],
        load_specs[2],
        load_specs[4],
        load_specs[6],
        load_specs[1],
        load_specs[3],
        load_specs[5],
        load_specs[7],
    ];

    let mut blocks = vec![entry, header];
    let mut first_test = BasicBlock::new(BlockId(2));
    first_test.operations = graph.take_operations();
    let mut test_block = Some(first_test);
    let mut staged = Vec::with_capacity(PREDICATED_LOADS);

    for (index, spec) in load_specs.into_iter().enumerate() {
        let valid_id = BlockId(3 + 2 * index as u32);
        let merge_id = BlockId(4 + 2 * index as u32);
        let mut test = test_block.take().expect("predicated load test block");
        test.terminator = Some(Terminator::ConditionalBranch {
            condition: spec.condition,
            then_target: valid_id,
            then_arguments: vec![],
            else_target: merge_id,
            else_arguments: vec![bf16_zero],
        });
        blocks.push(test);

        let loaded = graph.global_load(
            spec.base,
            spec.offset,
            bf16.clone(),
            AccessMode::ReadOnly,
            2,
        );
        let mut valid = BasicBlock::new(valid_id);
        valid.operations = graph.take_operations();
        valid.terminator = Some(Terminator::Branch {
            target: merge_id,
            arguments: vec![loaded],
        });
        blocks.push(valid);

        let selected = graph.block_parameter(bf16.clone());
        staged.push(selected.id);
        let mut merge = BasicBlock::new(merge_id);
        merge.parameters.push(selected);
        if index + 1 == PREDICATED_LOADS {
            let a_values = [staged[0], staged[1], staged[2], staged[3]];
            let b_values = [staged[4], staged[5], staged[6], staged[7]];
            graph.matrix(MatrixOperation::lds_store(
                a_lds,
                a_values,
                MatrixElement::Bf16,
            ));
            graph.matrix(MatrixOperation::lds_store(
                b_lds,
                b_values,
                MatrixElement::Bf16,
            ));
            graph.workgroup_lds_barrier();
            let lhs = graph
                .matrix(MatrixOperation::lds_load(a_lds, MatrixElement::Bf16))
                .try_into()
                .expect("four A LDS fragment values");
            let rhs = graph
                .matrix(MatrixOperation::lds_load(b_lds, MatrixElement::Bf16))
                .try_into()
                .expect("four B LDS fragment values");
            let results: [ValueId; 4] = graph
                .matrix(
                    MatrixOperation::multiply_accumulate(lhs, rhs, accumulator_ids)
                        .with_declared_tensor_layout(
                            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64_lds_xor4(
                            )
                            .with_zero_filled_predicate_inputs(),
                        ),
                )
                .try_into()
                .expect("four carried accumulator values");
            graph.workgroup_lds_barrier();
            let next_phase = graph.index_binary(BinaryOp::Add, phase.id, phase_step);
            merge.operations = graph.take_operations();
            merge.terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: std::iter::once(next_phase).chain(results).collect(),
            });
            blocks.push(merge);
        } else {
            test_block = Some(merge);
        }
    }

    let output_values = [
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
    ];
    let mut first_output_test = BasicBlock::new(OUTPUT_BLOCK);
    first_output_test.parameters = output_values.to_vec();
    let mut output_test = Some(first_output_test);

    for component in 0..TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS as usize {
        let valid_id = BlockId(20 + 2 * component as u32);
        let next_id = BlockId(21 + 2 * component as u32);
        let mut test = output_test.take().expect("predicated C test block");
        test.terminator = Some(Terminator::ConditionalBranch {
            condition: c_enabled[component],
            then_target: valid_id,
            then_arguments: vec![],
            else_target: next_id,
            else_arguments: vec![],
        });
        blocks.push(test);

        let c_input = graph.global_load(
            c_base,
            c_indices[component],
            Type::F32,
            AccessMode::ReadWrite,
            4,
        );
        let scaled_product = graph.binary(
            Type::F32,
            BinaryOp::Multiply,
            alpha,
            output_values[component].id,
        );
        let scaled_input = graph.binary(Type::F32, BinaryOp::Multiply, beta, c_input);
        let output = graph.binary(Type::F32, BinaryOp::Add, scaled_product, scaled_input);
        graph.global_store(c_base, c_indices[component], output, 4);
        let mut valid = BasicBlock::new(valid_id);
        valid.operations = graph.take_operations();
        valid.terminator = Some(Terminator::Branch {
            target: next_id,
            arguments: vec![],
        });
        blocks.push(valid);

        let mut next = BasicBlock::new(next_id);
        if component + 1 == TILED_GEMM_LDS_EDGES_V1_FRAGMENT_ELEMENTS as usize {
            next.terminator = Some(Terminator::Return { values: vec![] });
            blocks.push(next);
        } else {
            output_test = Some(next);
        }
    }

    let mut function = Function::kernel_entry(
        TILED_GEMM_LDS_EDGES_V1_FUNCTION_ID,
        Signature::new(
            vec![
                read_bf16_slice.clone(),
                read_bf16_slice,
                read_write_f32_slice,
            ],
            vec![],
        ),
        (0..3).map(ValueId).collect(),
        blocks,
    );
    function.required_capabilities = exact_capabilities();

    let mut kernel = Kernel::new(
        TILED_GEMM_LDS_EDGES_V1_KERNEL_ID,
        TILED_GEMM_LDS_EDGES_V1_FUNCTION_ID,
        LaunchDomain::D2 {
            x: LaunchExtent::Static(TILED_GEMM_LDS_EDGES_V1_LAUNCH_EXTENT_X),
            y: LaunchExtent::Static(TILED_GEMM_LDS_EDGES_V1_LAUNCH_EXTENT_Y),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(TILED_GEMM_LDS_EDGES_V1_LANES, 1, 1));
    kernel.required_capabilities = exact_capabilities();

    let mut module = crate::Module::new(TILED_GEMM_LDS_EDGES_V1_MODULE_ID);
    module.required_capabilities = exact_capabilities();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

/// Verifies generic IR invariants, the closed profile, and exact graph identity.
pub fn verify_tiled_gemm_lds_edges_v1_module(
    module: &crate::Module,
    profile: &TiledGemmLdsEdgesV1Profile,
) -> Result<(), TiledGemmLdsEdgesV1Error> {
    if !profile.is_exact() {
        return Err(TiledGemmLdsEdgesV1Error::UnsupportedProfile);
    }
    verify_module(module).map_err(TiledGemmLdsEdgesV1Error::InvalidKernelIr)?;
    if module != &tiled_gemm_lds_edges_v1_module() {
        return Err(TiledGemmLdsEdgesV1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

fn exact_capabilities() -> BTreeSet<TargetCapability> {
    let mut capabilities =
        MatrixOperation::multiply_accumulate([ValueId(0); 4], [ValueId(0); 4], [ValueId(0); 4])
            .required_capabilities();
    capabilities
        .extend(MatrixOperation::lds_load(ValueId(0), MatrixElement::Bf16).required_capabilities());
    capabilities.insert(TargetCapability::WorkgroupBarrier);
    capabilities.insert(gfx942_xnack_minus_target_capability());
    capabilities
}

#[derive(Clone, Copy)]
struct PredicatedLoad {
    condition: ValueId,
    base: ValueId,
    offset: ValueId,
}

struct GraphBuilder {
    next_id: u32,
    operations: Vec<Operation>,
}

impl GraphBuilder {
    fn new(next_id: u32) -> Self {
        Self {
            next_id,
            operations: Vec::new(),
        }
    }

    fn next_value(&mut self, ty: Type) -> ValueDef {
        let value = ValueDef::new(ValueId(self.next_id), ty);
        self.next_id += 1;
        value
    }

    fn block_parameter(&mut self, ty: Type) -> ValueDef {
        self.next_value(ty)
    }

    fn value(&mut self, ty: Type, kind: OperationKind) -> ValueId {
        let result = self.next_value(ty);
        let id = result.id;
        self.operations.push(Operation::effect_free(result, kind));
        id
    }

    fn invocation_index(&mut self, kind: IndexKind, axis: Axis) -> ValueId {
        self.value(
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex { kind, axis },
                Type::INDEX,
            )),
        )
    }

    fn index_constant(&mut self, value: u32) -> ValueId {
        self.value(
            Type::INDEX,
            OperationKind::Constant(Constant::Index(u64::from(value))),
        )
    }

    fn binary(&mut self, ty: Type, op: BinaryOp, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.value(ty, OperationKind::Binary { op, lhs, rhs })
    }

    fn index_binary(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.binary(Type::INDEX, op, lhs, rhs)
    }

    fn bool_and(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.binary(Type::BOOL, BinaryOp::BitAnd, lhs, rhs)
    }

    fn compare(&mut self, predicate: ComparePredicate, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.value(
            Type::BOOL,
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            },
        )
    }

    fn static_bf16_lds_tile(&mut self) -> ValueId {
        let element = Type::Scalar(ScalarType::Bf16);
        self.value(
            Type::pointer(
                element.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element,
                extent: WorkgroupMemoryExtent::Static(TILED_GEMM_LDS_EDGES_V1_TILE_ELEMENTS),
                alignment: TILED_GEMM_LDS_EDGES_V1_LDS_ALIGNMENT,
            }),
        )
    }

    fn global_load(
        &mut self,
        base: ValueId,
        offset: ValueId,
        ty: Type,
        access_mode: AccessMode,
        alignment: u32,
    ) -> ValueId {
        let pointer = self.value(
            Type::pointer(ty.clone(), AddressSpace::Global, access_mode),
            OperationKind::GetElementPointer { base, offset },
        );
        self.value(
            ty,
            OperationKind::Load {
                pointer,
                access: crate::MemoryAccess::new(AddressSpace::Global, alignment),
            },
        )
    }

    fn global_store(&mut self, base: ValueId, offset: ValueId, value: ValueId, alignment: u32) {
        let pointer = self.value(
            Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
            OperationKind::GetElementPointer { base, offset },
        );
        self.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer,
                value,
                access: crate::MemoryAccess::new(AddressSpace::Global, alignment),
            },
        ));
    }

    fn matrix(&mut self, matrix: MatrixOperation) -> Vec<ValueId> {
        let results = matrix
            .result_types()
            .into_iter()
            .map(|ty| self.next_value(ty))
            .collect::<Vec<_>>();
        let ids = results.iter().map(|result| result.id).collect();
        self.operations
            .push(Operation::new(results, OperationKind::Matrix(matrix)));
        ids
    }

    fn workgroup_lds_barrier(&mut self) {
        self.operations.push(Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ));
    }

    fn take_operations(&mut self) -> Vec<Operation> {
        std::mem::take(&mut self.operations)
    }
}
