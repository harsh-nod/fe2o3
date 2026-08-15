//! Canonical two-phase LDS-tiled GEMM Slice 2 for the exact gfx942 profile.
//!
//! This module seals one `16x16x32` BF16 x BF16 -> F32 graph. A real bounded
//! SSA loop executes two K=16 phases. Both phases cooperatively stage A and B
//! through the same two 512-byte XOR4 LDS tiles, synchronize before LDS reads,
//! carry four FP32 accumulator components through the loop header, and
//! synchronize after the MFMA before the next phase may overwrite LDS.
//!
//! This is an exact K=32 Kernel IR admission boundary, not an unrolled graph.
//! It does not claim arbitrary K, source correspondence, lowering correctness,
//! artifact identity, numerical refinement, or execution authority.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, BasicBlock, BinaryOp, BlockId,
    ComparePredicate, Constant, Convergence, Function, IndexKind, IntrinsicKind,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MatrixElement, MatrixOperation,
    MemoryOrdering, Operation, OperationKind, ScalarType, Signature, SynchronizationScope,
    TargetCapability, Terminator, Type, ValueDef, ValueId, VerificationErrors, WaveWidth,
    WorkgroupBarrier, WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupSize,
    gfx942_xnack_minus_target_capability, verify_module,
};

pub const TILED_GEMM_LDS_K32_V2_MODULE_ID: &str = "fe2o3::tiled_gemm_lds_k32_v2";
pub const TILED_GEMM_LDS_K32_V2_FUNCTION_ID: &str = "__fe2o3_tiled_gemm_lds_k32_v2_impl";
pub const TILED_GEMM_LDS_K32_V2_KERNEL_ID: &str = "tiled_gemm_lds_k32_v2";
pub const TILED_GEMM_LDS_K32_V2_TILE_EXTENT: u32 = 16;
pub const TILED_GEMM_LDS_K32_V2_K: u32 = 32;
pub const TILED_GEMM_LDS_K32_V2_PHASES: u32 = 2;
pub const TILED_GEMM_LDS_K32_V2_PHASE_K: u32 = 16;
pub const TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS: u32 = 256;
pub const TILED_GEMM_LDS_K32_V2_INPUT_ELEMENTS: u32 = 512;
pub const TILED_GEMM_LDS_K32_V2_LANES: u32 = 64;
pub const TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS: u32 = 4;
pub const TILED_GEMM_LDS_K32_V2_ALLOCATION_COUNT: u32 = 2;
pub const TILED_GEMM_LDS_K32_V2_TILE_BYTES: u32 = 512;
pub const TILED_GEMM_LDS_K32_V2_STATIC_LDS_BYTES: u32 = 1024;
pub const TILED_GEMM_LDS_K32_V2_LDS_ALIGNMENT: u32 = 16;

/// Closed admission profile for the exact two-phase K=32 increment.
///
/// Slice lengths and output ownership are profile facts because Kernel IR
/// slice types do not encode static lengths. A later host binding must still
/// establish these exact runtime extents.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TiledGemmLdsK32V2Profile {
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
    pub phase_k: u32,
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

impl TiledGemmLdsK32V2Profile {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        Self {
            target: gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            m: TILED_GEMM_LDS_K32_V2_TILE_EXTENT,
            n: TILED_GEMM_LDS_K32_V2_TILE_EXTENT,
            k: TILED_GEMM_LDS_K32_V2_K,
            a_elements: TILED_GEMM_LDS_K32_V2_INPUT_ELEMENTS,
            b_elements: TILED_GEMM_LDS_K32_V2_INPUT_ELEMENTS,
            c_elements: TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS,
            tile_rows: 1,
            tile_columns: 1,
            depth_tiles: TILED_GEMM_LDS_K32_V2_PHASES,
            phase_k: TILED_GEMM_LDS_K32_V2_PHASE_K,
            wave_width: WaveWidth::Wave64,
            launch_extent_x: TILED_GEMM_LDS_K32_V2_LANES,
            workgroup_size: WorkgroupSize::new(TILED_GEMM_LDS_K32_V2_LANES, 1, 1),
            lds_allocations: TILED_GEMM_LDS_K32_V2_ALLOCATION_COUNT,
            lds_elements_per_allocation: TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS,
            lds_bytes_per_allocation: TILED_GEMM_LDS_K32_V2_TILE_BYTES,
            static_lds_bytes: TILED_GEMM_LDS_K32_V2_STATIC_LDS_BYTES,
            lds_alignment: TILED_GEMM_LDS_K32_V2_LDS_ALIGNMENT,
            output_elements_per_lane: TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS,
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TiledGemmLdsK32V2Error {
    UnsupportedProfile,
    InvalidKernelIr(VerificationErrors),
    NonCanonicalKernelIr,
}

impl fmt::Display for TiledGemmLdsK32V2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "tiled GEMM LDS K32 V2 requires gfx942:xnack-, COV6, one 16x16x32 BF16/BF16-to-F32 tile, an exact two-iteration K=16 loop, wave64/workgroup64, two reused 256-element static XOR4 LDS allocations, pre-read and pre-reuse workgroup barriers, carried FP32 accumulators, and four disjoint output elements per lane",
            ),
            Self::InvalidKernelIr(error) => error.fmt(formatter),
            Self::NonCanonicalKernelIr => {
                formatter.write_str("kernel IR does not match canonical tiled GEMM LDS K32 V2")
            }
        }
    }
}

impl Error for TiledGemmLdsK32V2Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidKernelIr(error) => Some(error),
            _ => None,
        }
    }
}

/// Logical tile coordinate owned by one lane/component.
pub const fn tiled_gemm_lds_k32_v2_fragment_coordinate(
    lane: u32,
    component: u32,
) -> Option<(u32, u32)> {
    if lane >= TILED_GEMM_LDS_K32_V2_LANES || component >= TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS {
        return None;
    }
    Some((
        lane % TILED_GEMM_LDS_K32_V2_TILE_EXTENT,
        TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS * (lane / TILED_GEMM_LDS_K32_V2_TILE_EXTENT)
            + component,
    ))
}

/// Row-major A input index staged by a lane/component in one loop phase.
pub const fn tiled_gemm_lds_k32_v2_a_index(phase: u32, lane: u32, component: u32) -> Option<u32> {
    if phase >= TILED_GEMM_LDS_K32_V2_PHASES {
        return None;
    }
    match tiled_gemm_lds_k32_v2_fragment_coordinate(lane, component) {
        Some((row, local_depth)) => Some(
            row * TILED_GEMM_LDS_K32_V2_K + phase * TILED_GEMM_LDS_K32_V2_PHASE_K + local_depth,
        ),
        None => None,
    }
}

/// Row-major B input index staged transposed in one loop phase.
pub const fn tiled_gemm_lds_k32_v2_b_index(phase: u32, lane: u32, component: u32) -> Option<u32> {
    if phase >= TILED_GEMM_LDS_K32_V2_PHASES {
        return None;
    }
    match tiled_gemm_lds_k32_v2_fragment_coordinate(lane, component) {
        Some((column, local_depth)) => Some(
            (phase * TILED_GEMM_LDS_K32_V2_PHASE_K + local_depth)
                * TILED_GEMM_LDS_K32_V2_TILE_EXTENT
                + column,
        ),
        None => None,
    }
}

/// Row-major C output index uniquely owned by this lane/component.
pub const fn tiled_gemm_lds_k32_v2_c_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_lds_k32_v2_fragment_coordinate(lane, component) {
        Some((column, row)) => Some(row * TILED_GEMM_LDS_K32_V2_TILE_EXTENT + column),
        None => None,
    }
}

/// Physical index for the exact row-major XOR4 LDS layout.
pub const fn tiled_gemm_lds_k32_v2_xor4_index(row: u32, column: u32) -> Option<u32> {
    if row >= TILED_GEMM_LDS_K32_V2_TILE_EXTENT || column >= TILED_GEMM_LDS_K32_V2_TILE_EXTENT {
        return None;
    }
    Some(
        row * TILED_GEMM_LDS_K32_V2_TILE_EXTENT
            + (column ^ ((row & 3) * TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS)),
    )
}

/// Physical LDS index reused by both K phases for a lane/component.
pub const fn tiled_gemm_lds_k32_v2_lds_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_lds_k32_v2_fragment_coordinate(lane, component) {
        Some((row, column)) => tiled_gemm_lds_k32_v2_xor4_index(row, column),
        None => None,
    }
}

/// Constructs the only Kernel IR graph admitted by LDS GEMM K32 Slice 2.
pub fn tiled_gemm_lds_k32_v2_module() -> crate::Module {
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
    let tile_extent = graph.index_constant(TILED_GEMM_LDS_K32_V2_TILE_EXTENT);
    let full_k = graph.index_constant(TILED_GEMM_LDS_K32_V2_K);
    let fragment_elements = graph.index_constant(TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS);
    let phase_zero = graph.index_constant(0);
    let phase_count = graph.index_constant(TILED_GEMM_LDS_K32_V2_PHASES);
    let phase_step = graph.index_constant(1);
    let lane_column = graph.binary(BinaryOp::Remainder, lane, tile_extent);
    let lane_quad = graph.binary(BinaryOp::Divide, lane, tile_extent);
    let local_depth_base = graph.binary(BinaryOp::Multiply, lane_quad, fragment_elements);
    let a_row_base = graph.binary(BinaryOp::Multiply, lane_column, full_k);

    let mut local_depths = Vec::with_capacity(TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS as usize);
    let mut c_indices = Vec::with_capacity(TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS as usize);
    for component in 0..TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS {
        let component = graph.index_constant(component);
        let local_depth = graph.binary(BinaryOp::Add, local_depth_base, component);
        let c_row_base = graph.binary(BinaryOp::Multiply, local_depth, tile_extent);
        local_depths.push(local_depth);
        c_indices.push(graph.binary(BinaryOp::Add, c_row_base, lane_column));
    }
    let local_depths: [ValueId; 4] = local_depths
        .try_into()
        .expect("fixed four-value local depth fragment");
    let c_indices: [ValueId; 4] = c_indices
        .try_into()
        .expect("fixed four-value C index fragment");
    let zero = graph.value(
        Type::F32,
        OperationKind::Constant(Constant::F32Bits(0.0f32.to_bits())),
    );

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = graph.take_operations();
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![phase_zero, zero, zero, zero, zero],
    });

    let phase = graph.block_parameter(Type::INDEX);
    let accumulators = [
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
    ];
    let phase_active = graph.value(
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: phase.id,
            rhs: phase_count,
        },
    );
    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = std::iter::once(phase.clone())
        .chain(accumulators.iter().cloned())
        .collect();
    header.operations = graph.take_operations();
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: phase_active,
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: accumulators.iter().map(|value| value.id).collect(),
    });

    let phase_depth_base = graph.binary(BinaryOp::Multiply, phase.id, tile_extent);
    let mut a_values = Vec::with_capacity(TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS as usize);
    let mut b_values = Vec::with_capacity(TILED_GEMM_LDS_K32_V2_FRAGMENT_ELEMENTS as usize);
    for local_depth in local_depths {
        let global_depth = graph.binary(BinaryOp::Add, phase_depth_base, local_depth);
        let a_index = graph.binary(BinaryOp::Add, a_row_base, global_depth);
        let b_row_base = graph.binary(BinaryOp::Multiply, global_depth, tile_extent);
        let b_index = graph.binary(BinaryOp::Add, b_row_base, lane_column);
        a_values.push(graph.global_load(a_base, a_index, bf16.clone(), 2));
        b_values.push(graph.global_load(b_base, b_index, bf16.clone(), 2));
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
    let results = graph.matrix(MatrixOperation::multiply_accumulate(
        lhs,
        rhs,
        accumulators.map(|value| value.id),
    ));
    let result_ids: [ValueId; 4] = results
        .try_into()
        .expect("fixed four-value carried C fragment");
    graph.workgroup_lds_barrier();
    let next_phase = graph.binary(BinaryOp::Add, phase.id, phase_step);

    let mut body = BasicBlock::new(BlockId(2));
    body.operations = graph.take_operations();
    body.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: std::iter::once(next_phase).chain(result_ids).collect(),
    });

    let output_values = [
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
        graph.block_parameter(Type::F32),
    ];
    for (index, value) in c_indices.into_iter().zip(&output_values) {
        graph.global_store(c_base, index, value.id, 4);
    }
    let mut store = BasicBlock::new(BlockId(3));
    store.parameters = output_values.into_iter().collect();
    store.operations = graph.take_operations();
    store.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        TILED_GEMM_LDS_K32_V2_FUNCTION_ID,
        Signature::new(
            vec![read_bf16_slice.clone(), read_bf16_slice, write_f32_slice],
            vec![],
        ),
        (0..3).map(ValueId).collect(),
        vec![entry, header, body, store],
    );
    function.required_capabilities = exact_capabilities();

    let mut kernel = Kernel::new(
        TILED_GEMM_LDS_K32_V2_KERNEL_ID,
        TILED_GEMM_LDS_K32_V2_FUNCTION_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(TILED_GEMM_LDS_K32_V2_LANES),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(TILED_GEMM_LDS_K32_V2_LANES, 1, 1));
    kernel.required_capabilities = exact_capabilities();

    let mut module = crate::Module::new(TILED_GEMM_LDS_K32_V2_MODULE_ID);
    module.required_capabilities = exact_capabilities();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

/// Verifies generic IR invariants, the closed profile, and exact graph identity.
///
/// Exact graph equality seals loop bounds, phase offsets, both synchronization
/// points, accumulator-carried backedges, allocation reuse, and output owners.
/// It creates no source, backend, artifact, proof, load, or execution authority.
pub fn verify_tiled_gemm_lds_k32_v2_module(
    module: &crate::Module,
    profile: &TiledGemmLdsK32V2Profile,
) -> Result<(), TiledGemmLdsK32V2Error> {
    if !profile.is_exact() {
        return Err(TiledGemmLdsK32V2Error::UnsupportedProfile);
    }
    verify_module(module).map_err(TiledGemmLdsK32V2Error::InvalidKernelIr)?;
    if module != &tiled_gemm_lds_k32_v2_module() {
        return Err(TiledGemmLdsK32V2Error::NonCanonicalKernelIr);
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

    fn block_parameter(&mut self, ty: Type) -> ValueDef {
        self.next_value(ty)
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
                extent: WorkgroupMemoryExtent::Static(TILED_GEMM_LDS_K32_V2_TILE_ELEMENTS),
                alignment: TILED_GEMM_LDS_K32_V2_LDS_ALIGNMENT,
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

    fn take_operations(&mut self) -> Vec<Operation> {
        std::mem::take(&mut self.operations)
    }
}
