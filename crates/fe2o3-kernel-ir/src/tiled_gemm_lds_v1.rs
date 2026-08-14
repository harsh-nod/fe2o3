//! Canonical bounded LDS-tiled GEMM Slice 1 for the exact gfx942 profile.
//!
//! This module seals one `16x16x16` BF16 x BF16 -> F32 graph. One wave64
//! cooperatively loads complete A and B tiles, writes them to two distinct
//! static XOR4 LDS allocations, crosses one workgroup barrier, reads
//! authenticated four-element fragments from each allocation, performs one
//! matrix multiply-accumulate from zero, and writes all 256 C elements with
//! one lane/component owner per element.
//!
//! This is a Kernel IR admission boundary. It does not claim source
//! correspondence, lowering correctness, artifact identity, or execution.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, BasicBlock, BinaryOp, BlockId, Constant,
    Convergence, Function, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain,
    LaunchExtent, MatrixElement, MatrixOperation, MemoryOrdering, Operation, OperationKind,
    ScalarType, Signature, SynchronizationScope, TargetCapability, Terminator, Type, ValueDef,
    ValueId, VerificationErrors, WaveWidth, WorkgroupBarrier, WorkgroupMemory,
    WorkgroupMemoryExtent, WorkgroupSize, gfx942_xnack_minus_target_capability, verify_module,
};

pub const TILED_GEMM_LDS_V1_MODULE_ID: &str = "fe2o3::tiled_gemm_lds_v1";
pub const TILED_GEMM_LDS_V1_FUNCTION_ID: &str = "__fe2o3_tiled_gemm_lds_v1_impl";
pub const TILED_GEMM_LDS_V1_KERNEL_ID: &str = "tiled_gemm_lds_v1";
pub const TILED_GEMM_LDS_V1_TILE_EXTENT: u32 = 16;
pub const TILED_GEMM_LDS_V1_ELEMENTS: u32 = 256;
pub const TILED_GEMM_LDS_V1_LANES: u32 = 64;
pub const TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS: u32 = 4;
pub const TILED_GEMM_LDS_V1_ALLOCATION_COUNT: u32 = 2;
pub const TILED_GEMM_LDS_V1_TILE_BYTES: u32 = 512;
pub const TILED_GEMM_LDS_V1_STATIC_LDS_BYTES: u32 = 1024;
pub const TILED_GEMM_LDS_V1_LDS_ALIGNMENT: u32 = 16;

/// Closed admission profile for bounded LDS-tiled GEMM Slice 1.
///
/// Slice lengths and disjoint output ownership are profile facts checked by
/// this module's exact graph and exhaustive lane maps. Kernel IR slice types do
/// not encode static lengths, so a later authenticated host binding must still
/// establish the runtime extents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TiledGemmLdsV1Profile {
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub a_elements: u32,
    pub b_elements: u32,
    pub c_elements: u32,
    pub tile_rows: u32,
    pub tile_columns: u32,
    pub depth_tiles: u32,
    pub wave_width: WaveWidth,
    pub launch_extent_x: u32,
    pub workgroup_size: WorkgroupSize,
    pub lds_allocations: u32,
    pub lds_elements_per_allocation: u32,
    pub lds_bytes_per_allocation: u32,
    pub static_lds_bytes: u32,
    pub lds_alignment: u32,
    pub output_elements_per_lane: u32,
}

impl TiledGemmLdsV1Profile {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        Self {
            target: gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            m: TILED_GEMM_LDS_V1_TILE_EXTENT,
            n: TILED_GEMM_LDS_V1_TILE_EXTENT,
            k: TILED_GEMM_LDS_V1_TILE_EXTENT,
            a_elements: TILED_GEMM_LDS_V1_ELEMENTS,
            b_elements: TILED_GEMM_LDS_V1_ELEMENTS,
            c_elements: TILED_GEMM_LDS_V1_ELEMENTS,
            tile_rows: 1,
            tile_columns: 1,
            depth_tiles: 1,
            wave_width: WaveWidth::Wave64,
            launch_extent_x: TILED_GEMM_LDS_V1_LANES,
            workgroup_size: WorkgroupSize::new(TILED_GEMM_LDS_V1_LANES, 1, 1),
            lds_allocations: TILED_GEMM_LDS_V1_ALLOCATION_COUNT,
            lds_elements_per_allocation: TILED_GEMM_LDS_V1_ELEMENTS,
            lds_bytes_per_allocation: TILED_GEMM_LDS_V1_TILE_BYTES,
            static_lds_bytes: TILED_GEMM_LDS_V1_STATIC_LDS_BYTES,
            lds_alignment: TILED_GEMM_LDS_V1_LDS_ALIGNMENT,
            output_elements_per_lane: TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS,
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TiledGemmLdsV1Error {
    UnsupportedProfile,
    InvalidKernelIr(VerificationErrors),
    NonCanonicalKernelIr,
}

impl fmt::Display for TiledGemmLdsV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "tiled GEMM LDS V1 requires gfx942:xnack-, COV6, one 16x16x16 BF16/BF16-to-F32 tile, wave64/workgroup64, two separate 256-element static XOR4 LDS allocations, one workgroup barrier, and four disjoint output elements per lane",
            ),
            Self::InvalidKernelIr(error) => error.fmt(formatter),
            Self::NonCanonicalKernelIr => {
                formatter.write_str("kernel IR does not match canonical tiled GEMM LDS V1")
            }
        }
    }
}

impl Error for TiledGemmLdsV1Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidKernelIr(error) => Some(error),
            _ => None,
        }
    }
}

/// Logical `(row, column)` owned by a lane/component in either XOR4 LDS tile.
pub const fn tiled_gemm_lds_v1_fragment_coordinate(
    lane: u32,
    component: u32,
) -> Option<(u32, u32)> {
    if lane >= TILED_GEMM_LDS_V1_LANES || component >= TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS {
        return None;
    }
    Some((
        lane % TILED_GEMM_LDS_V1_TILE_EXTENT,
        TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS * (lane / TILED_GEMM_LDS_V1_TILE_EXTENT) + component,
    ))
}

/// Row-major A input index staged at this lane/component's logical LDS cell.
pub const fn tiled_gemm_lds_v1_a_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_lds_v1_fragment_coordinate(lane, component) {
        Some((row, depth)) => Some(row * TILED_GEMM_LDS_V1_TILE_EXTENT + depth),
        None => None,
    }
}

/// Row-major B input index staged transposed into this logical LDS cell.
pub const fn tiled_gemm_lds_v1_b_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_lds_v1_fragment_coordinate(lane, component) {
        Some((column, depth)) => Some(depth * TILED_GEMM_LDS_V1_TILE_EXTENT + column),
        None => None,
    }
}

/// Row-major C output index uniquely owned by this MFMA lane/component.
pub const fn tiled_gemm_lds_v1_c_index(lane: u32, component: u32) -> Option<u32> {
    tiled_gemm_lds_v1_b_index(lane, component)
}

/// Physical index for the exact row-major XOR4 LDS layout.
pub const fn tiled_gemm_lds_v1_xor4_index(row: u32, column: u32) -> Option<u32> {
    if row >= TILED_GEMM_LDS_V1_TILE_EXTENT || column >= TILED_GEMM_LDS_V1_TILE_EXTENT {
        return None;
    }
    Some(
        row * TILED_GEMM_LDS_V1_TILE_EXTENT
            + (column ^ ((row & 3) * TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS)),
    )
}

/// Physical LDS index written and later read by a lane/component.
pub const fn tiled_gemm_lds_v1_lds_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_lds_v1_fragment_coordinate(lane, component) {
        Some((row, column)) => tiled_gemm_lds_v1_xor4_index(row, column),
        None => None,
    }
}

/// Constructs the only Kernel IR graph admitted by bounded LDS GEMM Slice 1.
pub fn tiled_gemm_lds_v1_module() -> crate::Module {
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
    let lane = graph.value(
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    );
    let sixteen = graph.index_constant(TILED_GEMM_LDS_V1_TILE_EXTENT);
    let four = graph.index_constant(TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS);
    let lane_column = graph.binary(BinaryOp::Remainder, lane, sixteen);
    let lane_quad = graph.binary(BinaryOp::Divide, lane, sixteen);
    let depth_base = graph.binary(BinaryOp::Multiply, lane_quad, four);
    let a_row_base = graph.binary(BinaryOp::Multiply, lane_column, sixteen);

    let mut a_values = Vec::with_capacity(TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS as usize);
    let mut b_values = Vec::with_capacity(TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS as usize);
    let mut c_indices = Vec::with_capacity(TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS as usize);

    for component in 0..TILED_GEMM_LDS_V1_FRAGMENT_ELEMENTS {
        let component = graph.index_constant(component);
        let depth = graph.binary(BinaryOp::Add, depth_base, component);
        let a_index = graph.binary(BinaryOp::Add, a_row_base, depth);
        let c_row_base = graph.binary(BinaryOp::Multiply, depth, sixteen);
        let b_c_index = graph.binary(BinaryOp::Add, c_row_base, lane_column);

        a_values.push(graph.global_load(a_base, a_index, bf16.clone(), 2));
        b_values.push(graph.global_load(b_base, b_c_index, bf16.clone(), 2));
        c_indices.push(b_c_index);
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
    let results = graph.matrix(MatrixOperation::multiply_accumulate(lhs, rhs, [zero; 4]));
    let result_ids: [ValueId; 4] = results.try_into().expect("fixed four-value C fragment");

    for (index, value) in c_indices.into_iter().zip(result_ids) {
        graph.global_store(c_base, index, value, 4);
    }

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = graph.operations;
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        TILED_GEMM_LDS_V1_FUNCTION_ID,
        Signature::new(
            vec![read_bf16_slice.clone(), read_bf16_slice, write_f32_slice],
            vec![],
        ),
        (0..3).map(ValueId).collect(),
        vec![block],
    );
    function.required_capabilities = exact_capabilities();

    let mut kernel = Kernel::new(
        TILED_GEMM_LDS_V1_KERNEL_ID,
        TILED_GEMM_LDS_V1_FUNCTION_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(TILED_GEMM_LDS_V1_LANES),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(TILED_GEMM_LDS_V1_LANES, 1, 1));
    kernel.required_capabilities = exact_capabilities();

    let mut module = crate::Module::new(TILED_GEMM_LDS_V1_MODULE_ID);
    module.required_capabilities = exact_capabilities();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

/// Verifies generic IR invariants, the closed profile, and exact graph identity.
///
/// Generic matrix verification authenticates each LDS fragment operation back
/// to the direct workgroup-memory allocation result. Exact graph equality then
/// seals separate A/B allocations and store/barrier/read ordering. This creates
/// no source, artifact, proof, load, or execution authority.
pub fn verify_tiled_gemm_lds_v1_module(
    module: &crate::Module,
    profile: &TiledGemmLdsV1Profile,
) -> Result<(), TiledGemmLdsV1Error> {
    if !profile.is_exact() {
        return Err(TiledGemmLdsV1Error::UnsupportedProfile);
    }
    verify_module(module).map_err(TiledGemmLdsV1Error::InvalidKernelIr)?;
    if module != &tiled_gemm_lds_v1_module() {
        return Err(TiledGemmLdsV1Error::NonCanonicalKernelIr);
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
                extent: WorkgroupMemoryExtent::Static(TILED_GEMM_LDS_V1_ELEMENTS),
                alignment: TILED_GEMM_LDS_V1_LDS_ALIGNMENT,
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
