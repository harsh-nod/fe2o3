//! Canonical one-wave tiled GEMM checkpoint for the exact gfx942 profile.
//!
//! This module seals one `16x16x16` BF16 x BF16 + F32 -> F32 graph. Each of
//! the 64 lanes loads four A values, four B values, and four C values, performs
//! one matrix multiply-accumulate, and writes four uniquely owned D values.
//! It intentionally contains no LDS operations.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, Constant, Function, IndexKind,
    IntrinsicKind, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MatrixOperation,
    Operation, OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type, ValueDef,
    ValueId, VerificationErrors, WaveWidth, WorkgroupSize, gfx942_xnack_minus_target_capability,
    verify_module,
};

pub const TILED_GEMM_V1_MODULE_ID: &str = "fe2o3::tiled_gemm_v1";
pub const TILED_GEMM_V1_FUNCTION_ID: &str = "__fe2o3_tiled_gemm_v1_impl";
pub const TILED_GEMM_V1_KERNEL_ID: &str = "tiled_gemm_v1";
pub const TILED_GEMM_V1_TILE_EXTENT: u32 = 16;
pub const TILED_GEMM_V1_ELEMENTS: u32 = 256;
pub const TILED_GEMM_V1_LANES: u32 = 64;
pub const TILED_GEMM_V1_FRAGMENT_ELEMENTS: u32 = 4;

/// Required meaning of the Rust `u16` carrier at the frontend/IR boundary.
///
/// This record is an admission requirement. It does not establish that a
/// frontend performed the bridge or grant authority to any generated artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bf16U16BridgeV1 {
    pub rust_physical_scalar: ScalarType,
    pub kernel_ir_semantic_scalar: ScalarType,
    pub bit_width: u16,
    pub bit_preserving: bool,
}

impl Bf16U16BridgeV1 {
    pub const fn exact() -> Self {
        Self {
            rust_physical_scalar: ScalarType::U16,
            kernel_ir_semantic_scalar: ScalarType::Bf16,
            bit_width: 16,
            bit_preserving: true,
        }
    }
}

/// Closed admission profile for the canonical tiled GEMM V1 graph.
///
/// Slice lengths are profile facts checked before admitting this graph; Kernel
/// IR slice types do not encode static lengths and this module does not claim
/// to insert runtime length checks. COV6 is likewise a requirement for later
/// artifact inspection, not a property proven by this target-neutral IR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TiledGemmV1Profile {
    pub target: TargetCapability,
    pub code_object_version: u8,
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub a_elements: u32,
    pub b_elements: u32,
    pub c_elements: u32,
    pub d_elements: u32,
    pub tile_rows: u32,
    pub tile_columns: u32,
    pub depth_tiles: u32,
    pub wave_width: WaveWidth,
    pub launch_extent_x: u32,
    pub workgroup_size: WorkgroupSize,
    pub bf16_bridge: Bf16U16BridgeV1,
}

impl TiledGemmV1Profile {
    pub fn exact_gfx942_xnack_minus_cov6() -> Self {
        Self {
            target: gfx942_xnack_minus_target_capability(),
            code_object_version: 6,
            m: TILED_GEMM_V1_TILE_EXTENT,
            n: TILED_GEMM_V1_TILE_EXTENT,
            k: TILED_GEMM_V1_TILE_EXTENT,
            a_elements: TILED_GEMM_V1_ELEMENTS,
            b_elements: TILED_GEMM_V1_ELEMENTS,
            c_elements: TILED_GEMM_V1_ELEMENTS,
            d_elements: TILED_GEMM_V1_ELEMENTS,
            tile_rows: 1,
            tile_columns: 1,
            depth_tiles: 1,
            wave_width: WaveWidth::Wave64,
            launch_extent_x: TILED_GEMM_V1_LANES,
            workgroup_size: WorkgroupSize::new(TILED_GEMM_V1_LANES, 1, 1),
            bf16_bridge: Bf16U16BridgeV1::exact(),
        }
    }

    pub fn is_exact(&self) -> bool {
        self == &Self::exact_gfx942_xnack_minus_cov6()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TiledGemmV1Error {
    UnsupportedProfile,
    InvalidKernelIr(VerificationErrors),
    NonCanonicalKernelIr,
}

impl fmt::Display for TiledGemmV1Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProfile => formatter.write_str(
                "tiled GEMM V1 requires one 16x16x16 tile, four 256-element buffers, \
                 a bit-preserving u16/BF16 bridge, wave64, gfx942:xnack-, and downstream COV6 inspection",
            ),
            Self::InvalidKernelIr(error) => error.fmt(formatter),
            Self::NonCanonicalKernelIr => {
                formatter.write_str("kernel IR does not match canonical tiled GEMM V1")
            }
        }
    }
}

impl Error for TiledGemmV1Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidKernelIr(error) => Some(error),
            _ => None,
        }
    }
}

/// Returns A's `(row, depth)` coordinate for one admitted fragment component.
pub const fn tiled_gemm_v1_a_coordinate(lane: u32, component: u32) -> Option<(u32, u32)> {
    if lane >= TILED_GEMM_V1_LANES || component >= TILED_GEMM_V1_FRAGMENT_ELEMENTS {
        return None;
    }
    Some((
        lane % TILED_GEMM_V1_TILE_EXTENT,
        TILED_GEMM_V1_FRAGMENT_ELEMENTS * (lane / TILED_GEMM_V1_TILE_EXTENT) + component,
    ))
}

/// Returns B's `(depth, column)` coordinate for one admitted fragment component.
pub const fn tiled_gemm_v1_b_coordinate(lane: u32, component: u32) -> Option<(u32, u32)> {
    if lane >= TILED_GEMM_V1_LANES || component >= TILED_GEMM_V1_FRAGMENT_ELEMENTS {
        return None;
    }
    Some((
        TILED_GEMM_V1_FRAGMENT_ELEMENTS * (lane / TILED_GEMM_V1_TILE_EXTENT) + component,
        lane % TILED_GEMM_V1_TILE_EXTENT,
    ))
}

/// Returns C/D's `(row, column)` coordinate for one fragment component.
pub const fn tiled_gemm_v1_cd_coordinate(lane: u32, component: u32) -> Option<(u32, u32)> {
    tiled_gemm_v1_b_coordinate(lane, component)
}

pub const fn tiled_gemm_v1_a_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_v1_a_coordinate(lane, component) {
        Some((row, depth)) => Some(row * TILED_GEMM_V1_TILE_EXTENT + depth),
        None => None,
    }
}

pub const fn tiled_gemm_v1_b_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_v1_b_coordinate(lane, component) {
        Some((depth, column)) => Some(depth * TILED_GEMM_V1_TILE_EXTENT + column),
        None => None,
    }
}

pub const fn tiled_gemm_v1_cd_index(lane: u32, component: u32) -> Option<u32> {
    match tiled_gemm_v1_cd_coordinate(lane, component) {
        Some((row, column)) => Some(row * TILED_GEMM_V1_TILE_EXTENT + column),
        None => None,
    }
}

/// Constructs the only Kernel IR graph admitted by tiled GEMM V1.
pub fn tiled_gemm_v1_module() -> crate::Module {
    let bf16 = Type::Scalar(ScalarType::Bf16);
    let read_bf16_slice = Type::slice(bf16.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let read_f32_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let write_f32_slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let read_bf16_pointer = Type::pointer(bf16.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let read_f32_pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let write_f32_pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);

    let mut graph = GraphBuilder::new(4);
    let a_base = graph.value(
        read_bf16_pointer.clone(),
        OperationKind::SliceData { slice: ValueId(0) },
    );
    let b_base = graph.value(
        read_bf16_pointer.clone(),
        OperationKind::SliceData { slice: ValueId(1) },
    );
    let c_base = graph.value(
        read_f32_pointer,
        OperationKind::SliceData { slice: ValueId(2) },
    );
    let d_base = graph.value(
        write_f32_pointer,
        OperationKind::SliceData { slice: ValueId(3) },
    );
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
    let sixteen = graph.index_constant(TILED_GEMM_V1_TILE_EXTENT);
    let four = graph.index_constant(TILED_GEMM_V1_FRAGMENT_ELEMENTS);
    let lane_column = graph.binary(BinaryOp::Remainder, lane, sixteen);
    let lane_quad = graph.binary(BinaryOp::Divide, lane, sixteen);
    let depth_base = graph.binary(BinaryOp::Multiply, lane_quad, four);
    let a_row_base = graph.binary(BinaryOp::Multiply, lane_column, sixteen);

    let mut lhs = Vec::with_capacity(TILED_GEMM_V1_FRAGMENT_ELEMENTS as usize);
    let mut rhs = Vec::with_capacity(TILED_GEMM_V1_FRAGMENT_ELEMENTS as usize);
    let mut accumulator = Vec::with_capacity(TILED_GEMM_V1_FRAGMENT_ELEMENTS as usize);
    let mut d_indices = Vec::with_capacity(TILED_GEMM_V1_FRAGMENT_ELEMENTS as usize);

    for component in 0..TILED_GEMM_V1_FRAGMENT_ELEMENTS {
        let component = graph.index_constant(component);
        let depth = graph.binary(BinaryOp::Add, depth_base, component);
        let a_index = graph.binary(BinaryOp::Add, a_row_base, depth);
        let cd_row_base = graph.binary(BinaryOp::Multiply, depth, sixteen);
        let b_cd_index = graph.binary(BinaryOp::Add, cd_row_base, lane_column);

        lhs.push(graph.global_load(a_base, a_index, bf16.clone(), 2));
        rhs.push(graph.global_load(b_base, b_cd_index, bf16.clone(), 2));
        accumulator.push(graph.global_load(c_base, b_cd_index, Type::F32, 4));
        d_indices.push(b_cd_index);
    }

    let lhs: [ValueId; 4] = lhs.try_into().expect("fixed four-value A fragment");
    let rhs: [ValueId; 4] = rhs.try_into().expect("fixed four-value B fragment");
    let accumulator: [ValueId; 4] = accumulator
        .try_into()
        .expect("fixed four-value accumulator fragment");
    let matrix = MatrixOperation::multiply_accumulate(lhs, rhs, accumulator);
    let results = matrix
        .result_types()
        .into_iter()
        .map(|ty| graph.next_value(ty))
        .collect::<Vec<_>>();
    let result_ids: [ValueId; 4] = results
        .iter()
        .map(|result| result.id)
        .collect::<Vec<_>>()
        .try_into()
        .expect("fixed four-value D fragment");
    graph
        .operations
        .push(Operation::new(results, OperationKind::Matrix(matrix)));

    for (index, value) in d_indices.into_iter().zip(result_ids) {
        graph.global_store(d_base, index, value, 4);
    }

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = graph.operations;
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut function = Function::kernel_entry(
        TILED_GEMM_V1_FUNCTION_ID,
        Signature::new(
            vec![
                read_bf16_slice.clone(),
                read_bf16_slice,
                read_f32_slice,
                write_f32_slice,
            ],
            vec![],
        ),
        (0..4).map(ValueId).collect(),
        vec![block],
    );
    function.required_capabilities = exact_capabilities();

    let mut kernel = Kernel::new(
        TILED_GEMM_V1_KERNEL_ID,
        TILED_GEMM_V1_FUNCTION_ID,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(TILED_GEMM_V1_LANES),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(TILED_GEMM_V1_LANES, 1, 1));
    kernel.required_capabilities = exact_capabilities();

    let mut module = crate::Module::new(TILED_GEMM_V1_MODULE_ID);
    module.required_capabilities = exact_capabilities();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

/// Checks ordinary IR invariants, the exact admission profile, and exact graph
/// equality. No proof, artifact, load, or execution authority is created.
pub fn verify_tiled_gemm_v1_module(
    module: &crate::Module,
    profile: &TiledGemmV1Profile,
) -> Result<(), TiledGemmV1Error> {
    if !profile.is_exact() {
        return Err(TiledGemmV1Error::UnsupportedProfile);
    }
    verify_module(module).map_err(TiledGemmV1Error::InvalidKernelIr)?;
    if module != &tiled_gemm_v1_module() {
        return Err(TiledGemmV1Error::NonCanonicalKernelIr);
    }
    Ok(())
}

fn exact_capabilities() -> BTreeSet<TargetCapability> {
    let matrix =
        MatrixOperation::multiply_accumulate([ValueId(0); 4], [ValueId(0); 4], [ValueId(0); 4]);
    matrix
        .required_capabilities()
        .into_iter()
        .chain([
            gfx942_xnack_minus_target_capability(),
            TargetCapability::WaveWidth(WaveWidth::Wave64),
        ])
        .collect()
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

    fn global_load(&mut self, base: ValueId, offset: ValueId, ty: Type, alignment: u32) -> ValueId {
        let pointer_ty = Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
        let pointer = self.value(
            pointer_ty,
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
}
