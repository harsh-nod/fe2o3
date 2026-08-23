//! Canonical bounded LDS-tiled GEMM Slice 3 for one exact gfx942 grid.
//!
//! This module seals the `M=64`, `N=48`, `K=16`, `lda=33`, `ldb=79`,
//! `ldc=96` representative from the Slice 3 proof model. A `3x4` grid of
//! wave64 workgroups derives tile origins from workgroup X/Y, cooperatively
//! stages exact A and transposed-B fragments through separate aligned 512-byte
//! XOR4 LDS tiles, crosses one converged workgroup barrier, executes one BF16
//! MFMA, and writes four disjoint strided C elements per lane. The fully tiled
//! positive dimensions require no edge predicates.
//!
//! Kernel IR can carry scalar ABI parameters, but its current verifier cannot
//! prove runtime stride arithmetic or relate slice lengths to those scalars.
//! This milestone therefore admits only the exact static representative. A
//! later authenticated host binding must establish the profile's slice
//! lengths. This module claims no arbitrary dimensions, source correspondence,
//! lowering correctness, artifact identity, or execution authority.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, BasicBlock, BinaryOp, BlockId, Constant,
    Convergence, Function, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain,
    LaunchExtent, MatrixElement, MatrixOperation, MemoryOrdering, Operation, OperationKind,
    ScalarType, Signature, SynchronizationScope, TargetCapability, TensorLayoutContractV1,
    Terminator, Type, ValueDef, ValueId, VerificationErrors, WaveWidth, WorkgroupBarrier,
    WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupSize, gfx942_xnack_minus_target_capability,
    verify_module,
};

pub const TILED_GEMM_LDS_GRID_V1_MODULE_ID: &str = "fe2o3::tiled_gemm_lds_grid_v1";
pub const TILED_GEMM_LDS_GRID_V1_FUNCTION_ID: &str = "__fe2o3_tiled_gemm_lds_grid_v1_impl";
pub const TILED_GEMM_LDS_GRID_V1_KERNEL_ID: &str = "tiled_gemm_lds_grid_v1";
pub const TILED_GEMM_LDS_GRID_V1_TILE_EXTENT: u32 = 16;
pub const TILED_GEMM_LDS_GRID_V1_M: u32 = 64;
pub const TILED_GEMM_LDS_GRID_V1_N: u32 = 48;
pub const TILED_GEMM_LDS_GRID_V1_K: u32 = 16;
pub const TILED_GEMM_LDS_GRID_V1_LDA: u32 = 33;
pub const TILED_GEMM_LDS_GRID_V1_LDB: u32 = 79;
pub const TILED_GEMM_LDS_GRID_V1_LDC: u32 = 96;
pub const TILED_GEMM_LDS_GRID_V1_LANES: u32 = 64;
pub const TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS: u32 = 4;
pub const TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS: u32 =
    TILED_GEMM_LDS_GRID_V1_N / TILED_GEMM_LDS_GRID_V1_TILE_EXTENT;
pub const TILED_GEMM_LDS_GRID_V1_TILE_ROWS: u32 =
    TILED_GEMM_LDS_GRID_V1_M / TILED_GEMM_LDS_GRID_V1_TILE_EXTENT;
pub const TILED_GEMM_LDS_GRID_V1_WORKGROUPS: u32 =
    TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS * TILED_GEMM_LDS_GRID_V1_TILE_ROWS;
pub const TILED_GEMM_LDS_GRID_V1_LAUNCH_EXTENT_X: u32 =
    TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS * TILED_GEMM_LDS_GRID_V1_LANES;
pub const TILED_GEMM_LDS_GRID_V1_LAUNCH_EXTENT_Y: u32 = TILED_GEMM_LDS_GRID_V1_TILE_ROWS;
pub const TILED_GEMM_LDS_GRID_V1_A_ELEMENTS: u32 =
    (TILED_GEMM_LDS_GRID_V1_M - 1) * TILED_GEMM_LDS_GRID_V1_LDA + TILED_GEMM_LDS_GRID_V1_K;
pub const TILED_GEMM_LDS_GRID_V1_B_ELEMENTS: u32 =
    (TILED_GEMM_LDS_GRID_V1_K - 1) * TILED_GEMM_LDS_GRID_V1_LDB + TILED_GEMM_LDS_GRID_V1_N;
pub const TILED_GEMM_LDS_GRID_V1_C_ELEMENTS: u32 =
    (TILED_GEMM_LDS_GRID_V1_M - 1) * TILED_GEMM_LDS_GRID_V1_LDC + TILED_GEMM_LDS_GRID_V1_N;
pub const TILED_GEMM_LDS_GRID_V1_A_BYTES: u32 = TILED_GEMM_LDS_GRID_V1_A_ELEMENTS * 2;
pub const TILED_GEMM_LDS_GRID_V1_B_BYTES: u32 = TILED_GEMM_LDS_GRID_V1_B_ELEMENTS * 2;
pub const TILED_GEMM_LDS_GRID_V1_C_BYTES: u32 = TILED_GEMM_LDS_GRID_V1_C_ELEMENTS * 4;
pub const TILED_GEMM_LDS_GRID_V1_TILE_ELEMENTS: u32 = 256;
pub const TILED_GEMM_LDS_GRID_V1_ALLOCATION_COUNT: u32 = 2;
pub const TILED_GEMM_LDS_GRID_V1_TILE_BYTES: u32 = 512;
pub const TILED_GEMM_LDS_GRID_V1_STATIC_LDS_BYTES: u32 = 1_024;
pub const TILED_GEMM_LDS_GRID_V1_LDS_ALIGNMENT: u32 = 16;

/// Closed admission profile for the exact padded-stride Slice 3 representative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TiledGemmLdsGridV1Profile {
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub lda: u32,
    pub ldb: u32,
    pub ldc: u32,
    pub a_elements: u32,
    pub b_elements: u32,
    pub c_elements: u32,
    pub a_bytes: u32,
    pub b_bytes: u32,
    pub c_bytes: u32,
    pub tile_rows: u32,
    pub tile_columns: u32,
    pub depth_tiles: u32,
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
    pub output_elements_per_lane: u32,
}

impl TiledGemmLdsGridV1Profile {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        Self {
            target: gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            m: TILED_GEMM_LDS_GRID_V1_M,
            n: TILED_GEMM_LDS_GRID_V1_N,
            k: TILED_GEMM_LDS_GRID_V1_K,
            lda: TILED_GEMM_LDS_GRID_V1_LDA,
            ldb: TILED_GEMM_LDS_GRID_V1_LDB,
            ldc: TILED_GEMM_LDS_GRID_V1_LDC,
            a_elements: TILED_GEMM_LDS_GRID_V1_A_ELEMENTS,
            b_elements: TILED_GEMM_LDS_GRID_V1_B_ELEMENTS,
            c_elements: TILED_GEMM_LDS_GRID_V1_C_ELEMENTS,
            a_bytes: TILED_GEMM_LDS_GRID_V1_A_BYTES,
            b_bytes: TILED_GEMM_LDS_GRID_V1_B_BYTES,
            c_bytes: TILED_GEMM_LDS_GRID_V1_C_BYTES,
            tile_rows: TILED_GEMM_LDS_GRID_V1_TILE_ROWS,
            tile_columns: TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS,
            depth_tiles: 1,
            workgroup_count: TILED_GEMM_LDS_GRID_V1_WORKGROUPS,
            wave_width: WaveWidth::Wave64,
            launch_extent_x: TILED_GEMM_LDS_GRID_V1_LAUNCH_EXTENT_X,
            launch_extent_y: TILED_GEMM_LDS_GRID_V1_LAUNCH_EXTENT_Y,
            workgroup_size: WorkgroupSize::new(TILED_GEMM_LDS_GRID_V1_LANES, 1, 1),
            lds_allocations: TILED_GEMM_LDS_GRID_V1_ALLOCATION_COUNT,
            lds_elements_per_allocation: TILED_GEMM_LDS_GRID_V1_TILE_ELEMENTS,
            lds_bytes_per_allocation: TILED_GEMM_LDS_GRID_V1_TILE_BYTES,
            static_lds_bytes: TILED_GEMM_LDS_GRID_V1_STATIC_LDS_BYTES,
            lds_alignment: TILED_GEMM_LDS_GRID_V1_LDS_ALIGNMENT,
            output_elements_per_lane: TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS,
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TiledGemmLdsGridV1Error {
    UnsupportedProfile,
    InvalidKernelIr(VerificationErrors),
    NonCanonicalKernelIr,
}

impl fmt::Display for TiledGemmLdsGridV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "tiled GEMM LDS grid V1 requires the exact gfx942:xnack-/COV6 M=64, N=48, K=16, lda=33, ldb=79, ldc=96 profile, a 3x4 wave64/workgroup64 grid, two separate aligned 512-byte XOR4 LDS tiles, one converged workgroup barrier, one BF16 MFMA, and four disjoint strided C stores per lane",
            ),
            Self::InvalidKernelIr(error) => error.fmt(formatter),
            Self::NonCanonicalKernelIr => {
                formatter.write_str("kernel IR does not match canonical tiled GEMM LDS grid V1")
            }
        }
    }
}

impl Error for TiledGemmLdsGridV1Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidKernelIr(error) => Some(error),
            _ => None,
        }
    }
}

/// Logical tile-local `(row-or-column, depth)` staged by one lane/component.
pub const fn tiled_gemm_lds_grid_v1_fragment_coordinate(
    lane: u32,
    component: u32,
) -> Option<(u32, u32)> {
    if lane >= TILED_GEMM_LDS_GRID_V1_LANES || component >= TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS
    {
        return None;
    }
    Some((
        lane % TILED_GEMM_LDS_GRID_V1_TILE_EXTENT,
        TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS * (lane / TILED_GEMM_LDS_GRID_V1_TILE_EXTENT)
            + component,
    ))
}

/// Checked `(row, column)` origin for one admitted workgroup.
pub const fn tiled_gemm_lds_grid_v1_tile_origin(group_x: u32, group_y: u32) -> Option<(u32, u32)> {
    if group_x >= TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS || group_y >= TILED_GEMM_LDS_GRID_V1_TILE_ROWS
    {
        return None;
    }
    Some((
        group_y * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT,
        group_x * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT,
    ))
}

const fn bounded_strided_index(row: u32, column: u32, stride: u32, elements: u32) -> Option<u32> {
    let Some(row_base) = row.checked_mul(stride) else {
        return None;
    };
    let Some(index) = row_base.checked_add(column) else {
        return None;
    };
    if index >= elements {
        return None;
    }
    Some(index)
}

/// Checked padded-row-major A index for one workgroup row and lane/component.
pub const fn tiled_gemm_lds_grid_v1_a_index(
    group_y: u32,
    lane: u32,
    component: u32,
) -> Option<u32> {
    let Some((row, depth)) = tiled_gemm_lds_grid_v1_fragment_coordinate(lane, component) else {
        return None;
    };
    if group_y >= TILED_GEMM_LDS_GRID_V1_TILE_ROWS {
        return None;
    }
    bounded_strided_index(
        group_y * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT + row,
        depth,
        TILED_GEMM_LDS_GRID_V1_LDA,
        TILED_GEMM_LDS_GRID_V1_A_ELEMENTS,
    )
}

/// Checked padded-row-major B index staged transposed for one workgroup column.
pub const fn tiled_gemm_lds_grid_v1_b_index(
    group_x: u32,
    lane: u32,
    component: u32,
) -> Option<u32> {
    let Some((column, depth)) = tiled_gemm_lds_grid_v1_fragment_coordinate(lane, component) else {
        return None;
    };
    if group_x >= TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS {
        return None;
    }
    bounded_strided_index(
        depth,
        group_x * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT + column,
        TILED_GEMM_LDS_GRID_V1_LDB,
        TILED_GEMM_LDS_GRID_V1_B_ELEMENTS,
    )
}

/// Checked padded-row-major C index uniquely owned by one grid invocation.
pub const fn tiled_gemm_lds_grid_v1_c_index(
    group_x: u32,
    group_y: u32,
    lane: u32,
    component: u32,
) -> Option<u32> {
    let Some((column, row)) = tiled_gemm_lds_grid_v1_fragment_coordinate(lane, component) else {
        return None;
    };
    if group_x >= TILED_GEMM_LDS_GRID_V1_TILE_COLUMNS || group_y >= TILED_GEMM_LDS_GRID_V1_TILE_ROWS
    {
        return None;
    }
    bounded_strided_index(
        group_y * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT + row,
        group_x * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT + column,
        TILED_GEMM_LDS_GRID_V1_LDC,
        TILED_GEMM_LDS_GRID_V1_C_ELEMENTS,
    )
}

/// Physical index for the exact admitted row-major XOR4 LDS layout.
pub const fn tiled_gemm_lds_grid_v1_xor4_index(row: u32, column: u32) -> Option<u32> {
    if row >= TILED_GEMM_LDS_GRID_V1_TILE_EXTENT || column >= TILED_GEMM_LDS_GRID_V1_TILE_EXTENT {
        return None;
    }
    Some(
        row * TILED_GEMM_LDS_GRID_V1_TILE_EXTENT
            + (column ^ ((row & 3) * TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS)),
    )
}

/// Physical XOR4 LDS cell written and read by one lane/component.
pub const fn tiled_gemm_lds_grid_v1_lds_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_lds_grid_v1_fragment_coordinate(lane, component) {
        Some((row, column)) => tiled_gemm_lds_grid_v1_xor4_index(row, column),
        None => None,
    }
}

/// Constructs the only Kernel IR graph admitted by bounded LDS GEMM Slice 3.
pub fn tiled_gemm_lds_grid_v1_module() -> crate::Module {
    let bf16 = Type::Scalar(ScalarType::Bf16);
    let read_bf16_slice = Type::slice(bf16.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let write_f32_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let read_bf16_pointer = Type::pointer(bf16.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let write_f32_pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);

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
        write_f32_pointer,
        OperationKind::SliceData { slice: ValueId(2) },
    );
    let a_lds = graph.static_bf16_lds_tile();
    let b_lds = graph.static_bf16_lds_tile();
    let lane = graph.invocation_index(IndexKind::Local, Axis::X);
    let group_x = graph.invocation_index(IndexKind::Workgroup, Axis::X);
    let group_y = graph.invocation_index(IndexKind::Workgroup, Axis::Y);

    let tile_extent = graph.index_constant(TILED_GEMM_LDS_GRID_V1_TILE_EXTENT);
    let fragment_elements = graph.index_constant(TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS);
    let lda = graph.index_constant(TILED_GEMM_LDS_GRID_V1_LDA);
    let ldb = graph.index_constant(TILED_GEMM_LDS_GRID_V1_LDB);
    let ldc = graph.index_constant(TILED_GEMM_LDS_GRID_V1_LDC);
    let tile_origin_column = graph.binary(BinaryOp::Multiply, group_x, tile_extent);
    let tile_origin_row = graph.binary(BinaryOp::Multiply, group_y, tile_extent);
    let lane_column = graph.binary(BinaryOp::Remainder, lane, tile_extent);
    let lane_quad = graph.binary(BinaryOp::Divide, lane, tile_extent);
    let depth_base = graph.binary(BinaryOp::Multiply, lane_quad, fragment_elements);
    let a_row = graph.binary(BinaryOp::Add, tile_origin_row, lane_column);
    let a_row_base = graph.binary(BinaryOp::Multiply, a_row, lda);
    let b_column = graph.binary(BinaryOp::Add, tile_origin_column, lane_column);

    let mut a_values = Vec::with_capacity(TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS as usize);
    let mut b_values = Vec::with_capacity(TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS as usize);
    let mut c_indices = Vec::with_capacity(TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS as usize);
    for component in 0..TILED_GEMM_LDS_GRID_V1_FRAGMENT_ELEMENTS {
        let component = graph.index_constant(component);
        let depth = graph.binary(BinaryOp::Add, depth_base, component);
        let a_index = graph.binary(BinaryOp::Add, a_row_base, depth);
        let b_row_base = graph.binary(BinaryOp::Multiply, depth, ldb);
        let b_index = graph.binary(BinaryOp::Add, b_row_base, b_column);
        let c_row = graph.binary(BinaryOp::Add, tile_origin_row, depth);
        let c_row_base = graph.binary(BinaryOp::Multiply, c_row, ldc);
        let c_index = graph.binary(BinaryOp::Add, c_row_base, b_column);

        a_values.push(graph.global_load(a_base, a_index, bf16.clone(), 2));
        b_values.push(graph.global_load(b_base, b_index, bf16.clone(), 2));
        c_indices.push(c_index);
    }

    let a_values: [ValueId; 4] = a_values.try_into().expect("fixed four-value A fragment");
    let b_values: [ValueId; 4] = b_values.try_into().expect("fixed four-value B fragment");
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
        .expect("fixed four-value authenticated A LDS fragment");
    let rhs = graph
        .matrix(MatrixOperation::lds_load(b_lds, MatrixElement::Bf16))
        .try_into()
        .expect("fixed four-value authenticated B LDS fragment");
    let zero = graph.value(
        Type::F32,
        OperationKind::Constant(Constant::F32Bits(0.0f32.to_bits())),
    );
    let results = graph.matrix(
        MatrixOperation::multiply_accumulate(lhs, rhs, [zero; 4]).with_declared_tensor_layout(
            TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64_lds_xor4(),
        ),
    );
    let result_ids: [ValueId; 4] = results.try_into().expect("fixed four-value C fragment");

    for (index, value) in c_indices.into_iter().zip(result_ids) {
        graph.global_store(c_base, index, value, 4);
    }

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = graph.operations;
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        TILED_GEMM_LDS_GRID_V1_FUNCTION_ID,
        Signature::new(
            vec![read_bf16_slice.clone(), read_bf16_slice, write_f32_slice],
            vec![],
        ),
        (0..3).map(ValueId).collect(),
        vec![block],
    );
    function.required_capabilities = exact_capabilities();

    let mut kernel = Kernel::new(
        TILED_GEMM_LDS_GRID_V1_KERNEL_ID,
        TILED_GEMM_LDS_GRID_V1_FUNCTION_ID,
        LaunchDomain::D2 {
            x: LaunchExtent::Static(TILED_GEMM_LDS_GRID_V1_LAUNCH_EXTENT_X),
            y: LaunchExtent::Static(TILED_GEMM_LDS_GRID_V1_LAUNCH_EXTENT_Y),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(TILED_GEMM_LDS_GRID_V1_LANES, 1, 1));
    kernel.required_capabilities = exact_capabilities();

    let mut module = crate::Module::new(TILED_GEMM_LDS_GRID_V1_MODULE_ID);
    module.required_capabilities = exact_capabilities();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

/// Verifies generic IR invariants, the closed profile, and exact graph identity.
///
/// Exact graph equality seals target capability, grid mapping, padded strides,
/// address arithmetic, separate LDS resources, barrier semantics, XOR4 layout,
/// MFMA shape, and all C owners. It creates no source, backend, artifact, load,
/// or execution authority.
pub fn verify_tiled_gemm_lds_grid_v1_module(
    module: &crate::Module,
    profile: &TiledGemmLdsGridV1Profile,
) -> Result<(), TiledGemmLdsGridV1Error> {
    if !profile.is_exact() {
        return Err(TiledGemmLdsGridV1Error::UnsupportedProfile);
    }
    verify_module(module).map_err(TiledGemmLdsGridV1Error::InvalidKernelIr)?;
    if module != &tiled_gemm_lds_grid_v1_module() {
        return Err(TiledGemmLdsGridV1Error::NonCanonicalKernelIr);
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

    fn binary(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.value(Type::INDEX, OperationKind::Binary { op, lhs, rhs })
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
                extent: WorkgroupMemoryExtent::Static(TILED_GEMM_LDS_GRID_V1_TILE_ELEMENTS),
                alignment: TILED_GEMM_LDS_GRID_V1_LDS_ALIGNMENT,
            }),
        )
    }

    fn global_load(&mut self, base: ValueId, offset: ValueId, ty: Type, alignment: u32) -> ValueId {
        let pointer = self.value(
            Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadOnly),
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
}
