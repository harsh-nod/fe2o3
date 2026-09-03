//! Machine-checkable semantic ownership for the admitted Kernel IR surface.

use fe2o3_kernel_ir::{
    BinaryOp, CastKind, CheckedBinaryOperator, ComparePredicate, OperationKind, ScalarType,
    Terminator, UnaryOp,
};
use serde::Serialize;

use crate::preflight::{supported_cast, supports_binary, supports_compare, supports_unary};
use crate::{IndexWidthV1, SimulationTargetV1, UnsupportedFeatureV1};

pub const SEMANTIC_CAPABILITY_MATRIX_SCHEMA_V1: &str =
    "fe2o3-kir-sim-semantic-capability-matrix-v1";
/// Exact newline-terminated compact JSON size emitted by the V1 command.
pub const SEMANTIC_CAPABILITY_MATRIX_JSON_BYTES_V1: usize = 4_751_390;
pub const TOP_LEVEL_CAPABILITY_ROWS_V1: usize = SimulationOperationSurfaceV1::COUNT
    * SimulationCapabilityProfileV1::COUNT
    * SimulationKirWireVersionV1::COUNT;
pub const SCALAR_CAPABILITY_ROWS_V1: usize = SimulationCapabilityProfileV1::COUNT
    * (UNARY_OPERATIONS.len() * SCALAR_TYPES.len()
        + BINARY_OPERATIONS.len() * SCALAR_TYPES.len() * SCALAR_TYPES.len()
        + COMPARE_OPERATIONS.len() * SCALAR_TYPES.len() * SCALAR_TYPES.len()
        + CAST_OPERATIONS.len() * SCALAR_TYPES.len() * SCALAR_TYPES.len());

const SCALAR_TYPES: [ScalarType; 16] = [
    ScalarType::Bool,
    ScalarType::I8,
    ScalarType::I16,
    ScalarType::I32,
    ScalarType::I64,
    ScalarType::I128,
    ScalarType::U8,
    ScalarType::U16,
    ScalarType::U32,
    ScalarType::U64,
    ScalarType::U128,
    ScalarType::Index,
    ScalarType::F16,
    ScalarType::Bf16,
    ScalarType::F32,
    ScalarType::F64,
];

const UNARY_OPERATIONS: [UnaryOp; 2] = [UnaryOp::Negate, UnaryOp::Not];
const BINARY_OPERATIONS: [BinaryOp; 13] = [
    BinaryOp::Add,
    BinaryOp::Subtract,
    BinaryOp::Multiply,
    BinaryOp::Divide,
    BinaryOp::Remainder,
    BinaryOp::BitAnd,
    BinaryOp::BitOr,
    BinaryOp::BitXor,
    BinaryOp::ShiftLeft,
    BinaryOp::ShiftRight,
    BinaryOp::Checked(CheckedBinaryOperator::Add),
    BinaryOp::Checked(CheckedBinaryOperator::Subtract),
    BinaryOp::Checked(CheckedBinaryOperator::Multiply),
];
const COMPARE_OPERATIONS: [ComparePredicate; 6] = [
    ComparePredicate::Equal,
    ComparePredicate::NotEqual,
    ComparePredicate::LessThan,
    ComparePredicate::LessThanOrEqual,
    ComparePredicate::GreaterThan,
    ComparePredicate::GreaterThanOrEqual,
];
const CAST_OPERATIONS: [CastKind; 8] = [
    CastKind::Truncate,
    CastKind::ZeroExtend,
    CastKind::SignExtend,
    CastKind::FloatExtend,
    CastKind::FloatTruncate,
    CastKind::IntegerToFloat,
    CastKind::FloatToInteger,
    CastKind::Bitcast,
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
/// CPU simulation layouts only. A named GPU profile does not claim compiler
/// lowering, ISA support, launch authority, or observed hardware behavior.
pub enum SimulationCapabilityProfileV1 {
    LittleEndianIndex32,
    Amdgpu64TargetNeutral,
    Gfx942XnackMinus,
    Gfx950XnackMinus,
}

impl SimulationCapabilityProfileV1 {
    const ALL: [Self; 4] = [
        Self::LittleEndianIndex32,
        Self::Amdgpu64TargetNeutral,
        Self::Gfx942XnackMinus,
        Self::Gfx950XnackMinus,
    ];
    const COUNT: usize = Self::ALL.len();

    pub const fn simulation_target(self) -> SimulationTargetV1 {
        match self {
            Self::LittleEndianIndex32 => SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
            Self::Amdgpu64TargetNeutral | Self::Gfx942XnackMinus | Self::Gfx950XnackMinus => {
                SimulationTargetV1::amdgpu_64()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationKirWireVersionV1 {
    V7,
    V9,
    V10,
}

impl SimulationKirWireVersionV1 {
    const ALL: [Self; 3] = [Self::V7, Self::V9, Self::V10];
    const COUNT: usize = Self::ALL.len();
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum SimulationOperationSurfaceV1 {
    Constant = 0,
    Intrinsic = 1,
    MemoryIntrinsic = 2,
    Unary = 3,
    Binary = 4,
    Compare = 5,
    Cast = 6,
    Select = 7,
    Call = 8,
    Alloca = 9,
    SliceLength = 10,
    SliceData = 11,
    GetElementPointer = 12,
    Load = 13,
    GuardedLoad = 14,
    GuardedStore = 15,
    Store = 16,
    Barrier = 17,
    Atomic = 18,
    Fence = 19,
    WorkgroupBarrier = 20,
    WorkgroupMemory = 21,
    Matrix = 22,
    Gfx950LdsTranspose = 23,
    Wave = 24,
    InlineAssembly = 25,
    Branch = 26,
    ConditionalBranch = 27,
    Switch = 28,
    IntegerSwitch = 29,
    Return = 30,
    Unreachable = 31,
    /// Additive launch surface for one exact reachable dynamic LDS base.
    DynamicWorkgroupMemoryRequest = 32,
}

impl SimulationOperationSurfaceV1 {
    const ALL: [Self; 33] = [
        Self::Constant,
        Self::Intrinsic,
        Self::MemoryIntrinsic,
        Self::Unary,
        Self::Binary,
        Self::Compare,
        Self::Cast,
        Self::Select,
        Self::Call,
        Self::Alloca,
        Self::SliceLength,
        Self::SliceData,
        Self::GetElementPointer,
        Self::Load,
        Self::GuardedLoad,
        Self::GuardedStore,
        Self::Store,
        Self::Barrier,
        Self::Atomic,
        Self::Fence,
        Self::WorkgroupBarrier,
        Self::WorkgroupMemory,
        Self::Matrix,
        Self::Gfx950LdsTranspose,
        Self::Wave,
        Self::InlineAssembly,
        Self::Branch,
        Self::ConditionalBranch,
        Self::Switch,
        Self::IntegerSwitch,
        Self::Return,
        Self::Unreachable,
        Self::DynamicWorkgroupMemoryRequest,
    ];
    const COUNT: usize = Self::DynamicWorkgroupMemoryRequest as usize + 1;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationSemanticOwnerV1 {
    ScalarBits,
    LaunchGeometry,
    SoftwareFloat,
    CallDispatch,
    PrivateMemory,
    TypedMemory,
    AtomicMemory,
    WorkgroupCooperative,
    WaveCooperative,
    ControlFlow,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationUnsupportedReasonCodeV1 {
    FloatType,
    UnsupportedType,
    MemoryIntrinsic,
    ExternalVolatileMemory,
    MemoryIntrinsicTargetLayout,
    FloatConstant,
    FloatOperation,
    FloatFunction,
    InvalidIntegerCast,
    ExternalCall,
    NonInternalCall,
    WorkgroupAllocation,
    NonScalarMemory,
    UnsupportedAddressSpace,
    Barrier,
    Atomic,
    Fence,
    WorkgroupBarrier,
    WorkgroupMemory,
    DynamicWorkgroupMemory,
    Matrix,
    UnsupportedNumericalContract,
    Wave,
    Gfx950LdsTranspose,
    InlineAssembly,
    UnsupportedScalarOperation,
    TargetConstantOutOfRange,
    DynamicWorkgroupMemoryMissingBase,
    DynamicWorkgroupMemoryAmbiguousBases,
    DynamicWorkgroupMemoryAuthenticatedMinimum,
    DynamicWorkgroupMemoryExtentLayout,
}

impl UnsupportedFeatureV1 {
    pub const fn reason_code(&self) -> SimulationUnsupportedReasonCodeV1 {
        match self {
            Self::FloatType(_) => SimulationUnsupportedReasonCodeV1::FloatType,
            Self::UnsupportedType => SimulationUnsupportedReasonCodeV1::UnsupportedType,
            Self::MemoryIntrinsic => SimulationUnsupportedReasonCodeV1::MemoryIntrinsic,
            Self::ExternalVolatileMemory => {
                SimulationUnsupportedReasonCodeV1::ExternalVolatileMemory
            }
            Self::MemoryIntrinsicTargetLayout => {
                SimulationUnsupportedReasonCodeV1::MemoryIntrinsicTargetLayout
            }
            Self::FloatConstant => SimulationUnsupportedReasonCodeV1::FloatConstant,
            Self::FloatOperation => SimulationUnsupportedReasonCodeV1::FloatOperation,
            Self::FloatFunction(_) => SimulationUnsupportedReasonCodeV1::FloatFunction,
            Self::InvalidIntegerCast { .. } => {
                SimulationUnsupportedReasonCodeV1::InvalidIntegerCast
            }
            Self::ExternalCall(_) => SimulationUnsupportedReasonCodeV1::ExternalCall,
            Self::NonInternalCall { .. } => SimulationUnsupportedReasonCodeV1::NonInternalCall,
            Self::WorkgroupAllocation => SimulationUnsupportedReasonCodeV1::WorkgroupAllocation,
            Self::NonScalarMemory => SimulationUnsupportedReasonCodeV1::NonScalarMemory,
            Self::UnsupportedAddressSpace(_) => {
                SimulationUnsupportedReasonCodeV1::UnsupportedAddressSpace
            }
            Self::Barrier => SimulationUnsupportedReasonCodeV1::Barrier,
            Self::Atomic => SimulationUnsupportedReasonCodeV1::Atomic,
            Self::Fence => SimulationUnsupportedReasonCodeV1::Fence,
            Self::WorkgroupBarrier => SimulationUnsupportedReasonCodeV1::WorkgroupBarrier,
            Self::WorkgroupMemory => SimulationUnsupportedReasonCodeV1::WorkgroupMemory,
            Self::DynamicWorkgroupMemory => {
                SimulationUnsupportedReasonCodeV1::DynamicWorkgroupMemory
            }
            Self::Matrix => SimulationUnsupportedReasonCodeV1::Matrix,
            Self::UnsupportedNumericalContract => {
                SimulationUnsupportedReasonCodeV1::UnsupportedNumericalContract
            }
            Self::Wave => SimulationUnsupportedReasonCodeV1::Wave,
            Self::Gfx950LdsTranspose => SimulationUnsupportedReasonCodeV1::Gfx950LdsTranspose,
            Self::InlineAssembly => SimulationUnsupportedReasonCodeV1::InlineAssembly,
            Self::UnsupportedScalarOperation => {
                SimulationUnsupportedReasonCodeV1::UnsupportedScalarOperation
            }
            Self::TargetConstantOutOfRange => {
                SimulationUnsupportedReasonCodeV1::TargetConstantOutOfRange
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum SimulationCapabilityDispositionV1 {
    Owned {
        owner: SimulationSemanticOwnerV1,
        typed_rejections: &'static [SimulationUnsupportedReasonCodeV1],
    },
    Unsupported {
        reason: SimulationUnsupportedReasonCodeV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SimulationOperationCapabilityRowV1 {
    pub profile: SimulationCapabilityProfileV1,
    pub kir_wire_version: SimulationKirWireVersionV1,
    pub operation: SimulationOperationSurfaceV1,
    #[serde(flatten)]
    pub capability: SimulationCapabilityDispositionV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationScalarOperationFamilyV1 {
    Unary,
    Binary,
    Compare,
    Cast,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SimulationScalarCapabilityRowV1 {
    pub profile: SimulationCapabilityProfileV1,
    pub family: SimulationScalarOperationFamilyV1,
    pub operation: &'static str,
    pub lhs: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rhs: Option<&'static str>,
    #[serde(flatten)]
    pub capability: SimulationCapabilityDispositionV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SimulationCapabilityMatrixV1 {
    pub schema: &'static str,
    pub truth_origin: &'static str,
    pub authority: &'static str,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub top_level_rows: Vec<SimulationOperationCapabilityRowV1>,
    pub scalar_rows: Vec<SimulationScalarCapabilityRowV1>,
}

pub fn semantic_capability_matrix_v1() -> SimulationCapabilityMatrixV1 {
    let mut top_level_rows = Vec::with_capacity(TOP_LEVEL_CAPABILITY_ROWS_V1);
    let mut scalar_rows = Vec::with_capacity(SCALAR_CAPABILITY_ROWS_V1);
    for profile in SimulationCapabilityProfileV1::ALL {
        for kir_wire_version in SimulationKirWireVersionV1::ALL {
            for operation in SimulationOperationSurfaceV1::ALL {
                top_level_rows.push(SimulationOperationCapabilityRowV1 {
                    profile,
                    kir_wire_version,
                    operation,
                    capability: top_level_capability(operation, profile, kir_wire_version),
                });
            }
        }
        append_scalar_rows(&mut scalar_rows, profile);
    }
    debug_assert_eq!(top_level_rows.len(), TOP_LEVEL_CAPABILITY_ROWS_V1);
    debug_assert_eq!(scalar_rows.len(), SCALAR_CAPABILITY_ROWS_V1);
    SimulationCapabilityMatrixV1 {
        schema: SEMANTIC_CAPABILITY_MATRIX_SCHEMA_V1,
        truth_origin: "declared",
        authority: "none",
        hardware_observed: false,
        performance_prediction: false,
        top_level_rows,
        scalar_rows,
    }
}

fn append_scalar_rows(
    rows: &mut Vec<SimulationScalarCapabilityRowV1>,
    profile: SimulationCapabilityProfileV1,
) {
    let target = profile.simulation_target();
    for operation in UNARY_OPERATIONS {
        for ty in SCALAR_TYPES {
            rows.push(scalar_row(
                profile,
                SimulationScalarOperationFamilyV1::Unary,
                unary_name(operation),
                ty,
                None,
                supports_unary(operation, ty),
                scalar_owner(ty),
                SimulationUnsupportedReasonCodeV1::UnsupportedScalarOperation,
            ));
        }
    }
    for operation in BINARY_OPERATIONS {
        for lhs in SCALAR_TYPES {
            for rhs in SCALAR_TYPES {
                rows.push(scalar_row(
                    profile,
                    SimulationScalarOperationFamilyV1::Binary,
                    binary_name(operation),
                    lhs,
                    Some(rhs),
                    supports_binary(operation, lhs, rhs),
                    scalar_owner(lhs),
                    SimulationUnsupportedReasonCodeV1::UnsupportedScalarOperation,
                ));
            }
        }
    }
    for operation in COMPARE_OPERATIONS {
        for lhs in SCALAR_TYPES {
            for rhs in SCALAR_TYPES {
                rows.push(scalar_row(
                    profile,
                    SimulationScalarOperationFamilyV1::Compare,
                    compare_name(operation),
                    lhs,
                    Some(rhs),
                    supports_compare(operation, lhs, rhs),
                    scalar_owner(lhs),
                    SimulationUnsupportedReasonCodeV1::UnsupportedScalarOperation,
                ));
            }
        }
    }
    for operation in CAST_OPERATIONS {
        for from in SCALAR_TYPES {
            for to in SCALAR_TYPES {
                rows.push(scalar_row(
                    profile,
                    SimulationScalarOperationFamilyV1::Cast,
                    cast_name(operation),
                    from,
                    Some(to),
                    supported_cast(operation, from, to, target),
                    cast_owner(operation),
                    SimulationUnsupportedReasonCodeV1::InvalidIntegerCast,
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn scalar_row(
    profile: SimulationCapabilityProfileV1,
    family: SimulationScalarOperationFamilyV1,
    operation: &'static str,
    lhs: ScalarType,
    rhs: Option<ScalarType>,
    supported: bool,
    owner: SimulationSemanticOwnerV1,
    reason: SimulationUnsupportedReasonCodeV1,
) -> SimulationScalarCapabilityRowV1 {
    SimulationScalarCapabilityRowV1 {
        profile,
        family,
        operation,
        lhs: scalar_name(lhs),
        rhs: rhs.map(scalar_name),
        capability: if supported {
            SimulationCapabilityDispositionV1::Owned {
                owner,
                typed_rejections: &[],
            }
        } else {
            SimulationCapabilityDispositionV1::Unsupported { reason }
        },
    }
}

fn top_level_capability(
    operation: SimulationOperationSurfaceV1,
    profile: SimulationCapabilityProfileV1,
    kir_wire_version: SimulationKirWireVersionV1,
) -> SimulationCapabilityDispositionV1 {
    use SimulationOperationSurfaceV1 as Surface;
    use SimulationSemanticOwnerV1 as Owner;
    use SimulationUnsupportedReasonCodeV1 as Reason;
    let owned = |owner, typed_rejections| SimulationCapabilityDispositionV1::Owned {
        owner,
        typed_rejections,
    };
    let unsupported = |reason| SimulationCapabilityDispositionV1::Unsupported { reason };
    match operation {
        Surface::Constant => owned(
            Owner::ScalarBits,
            if profile == SimulationCapabilityProfileV1::LittleEndianIndex32 {
                &[Reason::TargetConstantOutOfRange]
            } else {
                &[]
            },
        ),
        Surface::Intrinsic => owned(Owner::LaunchGeometry, &[]),
        Surface::MemoryIntrinsic
            if matches!(
                kir_wire_version,
                SimulationKirWireVersionV1::V7 | SimulationKirWireVersionV1::V9
            ) =>
        {
            unsupported(Reason::MemoryIntrinsic)
        }
        Surface::MemoryIntrinsic => owned(
            Owner::TypedMemory,
            &[
                Reason::NonScalarMemory,
                Reason::UnsupportedAddressSpace,
                Reason::ExternalVolatileMemory,
                Reason::MemoryIntrinsicTargetLayout,
            ],
        ),
        Surface::Unary | Surface::Binary | Surface::Compare => {
            owned(Owner::ScalarBits, &[Reason::UnsupportedScalarOperation])
        }
        Surface::Cast => owned(Owner::ScalarBits, &[Reason::InvalidIntegerCast]),
        Surface::Select => owned(Owner::ScalarBits, &[]),
        Surface::Call => owned(
            Owner::CallDispatch,
            &[
                Reason::FloatFunction,
                Reason::ExternalCall,
                Reason::NonInternalCall,
            ],
        ),
        Surface::Alloca => owned(
            Owner::PrivateMemory,
            &[
                Reason::WorkgroupAllocation,
                Reason::UnsupportedAddressSpace,
                Reason::NonScalarMemory,
            ],
        ),
        Surface::SliceLength
        | Surface::SliceData
        | Surface::GetElementPointer
        | Surface::Load
        | Surface::GuardedLoad
        | Surface::GuardedStore
        | Surface::Store => owned(
            Owner::TypedMemory,
            &[Reason::UnsupportedAddressSpace, Reason::NonScalarMemory],
        ),
        Surface::Barrier => unsupported(Reason::Barrier),
        Surface::Atomic => owned(
            Owner::AtomicMemory,
            &[
                Reason::FloatType,
                Reason::UnsupportedAddressSpace,
                Reason::NonScalarMemory,
            ],
        ),
        Surface::Fence => owned(Owner::AtomicMemory, &[]),
        Surface::WorkgroupBarrier => owned(Owner::WorkgroupCooperative, &[]),
        Surface::WorkgroupMemory => owned(
            Owner::WorkgroupCooperative,
            &[Reason::DynamicWorkgroupMemory, Reason::NonScalarMemory],
        ),
        Surface::DynamicWorkgroupMemoryRequest => owned(
            Owner::WorkgroupCooperative,
            &[
                Reason::DynamicWorkgroupMemoryMissingBase,
                Reason::DynamicWorkgroupMemoryAmbiguousBases,
                Reason::DynamicWorkgroupMemoryAuthenticatedMinimum,
                Reason::DynamicWorkgroupMemoryExtentLayout,
                Reason::NonScalarMemory,
            ],
        ),
        Surface::Matrix => owned(
            Owner::WaveCooperative,
            &[Reason::UnsupportedNumericalContract],
        ),
        Surface::Gfx950LdsTranspose if kir_wire_version == SimulationKirWireVersionV1::V7 => {
            unsupported(Reason::Gfx950LdsTranspose)
        }
        Surface::Gfx950LdsTranspose => owned(Owner::WorkgroupCooperative, &[]),
        Surface::Wave if kir_wire_version == SimulationKirWireVersionV1::V7 => {
            owned(Owner::WaveCooperative, &[Reason::Wave])
        }
        Surface::Wave => owned(Owner::WaveCooperative, &[]),
        Surface::InlineAssembly => unsupported(Reason::InlineAssembly),
        Surface::Branch | Surface::ConditionalBranch | Surface::Return | Surface::Unreachable => {
            owned(Owner::ControlFlow, &[])
        }
        Surface::Switch => owned(Owner::ControlFlow, &[Reason::FloatOperation]),
        Surface::IntegerSwitch => owned(
            Owner::ControlFlow,
            &[Reason::FloatOperation, Reason::TargetConstantOutOfRange],
        ),
    }
}

pub(crate) fn operation_surface_v1(operation: &OperationKind) -> SimulationOperationSurfaceV1 {
    match operation {
        OperationKind::Constant(_) => SimulationOperationSurfaceV1::Constant,
        OperationKind::Intrinsic(_) => SimulationOperationSurfaceV1::Intrinsic,
        OperationKind::MemoryIntrinsic(_) => SimulationOperationSurfaceV1::MemoryIntrinsic,
        OperationKind::Unary { .. } => SimulationOperationSurfaceV1::Unary,
        OperationKind::Binary { .. } => SimulationOperationSurfaceV1::Binary,
        OperationKind::Compare { .. } => SimulationOperationSurfaceV1::Compare,
        OperationKind::Cast { .. } => SimulationOperationSurfaceV1::Cast,
        OperationKind::Select { .. } => SimulationOperationSurfaceV1::Select,
        OperationKind::Call { .. } => SimulationOperationSurfaceV1::Call,
        OperationKind::Alloca { .. } => SimulationOperationSurfaceV1::Alloca,
        OperationKind::SliceLength { .. } => SimulationOperationSurfaceV1::SliceLength,
        OperationKind::SliceData { .. } => SimulationOperationSurfaceV1::SliceData,
        OperationKind::GetElementPointer { .. } => SimulationOperationSurfaceV1::GetElementPointer,
        OperationKind::Load { .. } => SimulationOperationSurfaceV1::Load,
        OperationKind::GuardedLoad { .. } => SimulationOperationSurfaceV1::GuardedLoad,
        OperationKind::GuardedStore { .. } => SimulationOperationSurfaceV1::GuardedStore,
        OperationKind::Store { .. } => SimulationOperationSurfaceV1::Store,
        OperationKind::Barrier(_) => SimulationOperationSurfaceV1::Barrier,
        OperationKind::Atomic(_) => SimulationOperationSurfaceV1::Atomic,
        OperationKind::Fence(_) => SimulationOperationSurfaceV1::Fence,
        OperationKind::WorkgroupBarrier(_) => SimulationOperationSurfaceV1::WorkgroupBarrier,
        OperationKind::WorkgroupMemory(_) => SimulationOperationSurfaceV1::WorkgroupMemory,
        OperationKind::Matrix(_) => SimulationOperationSurfaceV1::Matrix,
        OperationKind::Gfx950LdsTranspose(_) => SimulationOperationSurfaceV1::Gfx950LdsTranspose,
        OperationKind::Wave(_) => SimulationOperationSurfaceV1::Wave,
        OperationKind::InlineAssembly(_) => SimulationOperationSurfaceV1::InlineAssembly,
    }
}

pub(crate) fn terminator_surface_v1(terminator: &Terminator) -> SimulationOperationSurfaceV1 {
    match terminator {
        Terminator::Branch { .. } => SimulationOperationSurfaceV1::Branch,
        Terminator::ConditionalBranch { .. } => SimulationOperationSurfaceV1::ConditionalBranch,
        Terminator::Switch { .. } => SimulationOperationSurfaceV1::Switch,
        Terminator::IntegerSwitch { .. } => SimulationOperationSurfaceV1::IntegerSwitch,
        Terminator::Return { .. } => SimulationOperationSurfaceV1::Return,
        Terminator::Unreachable => SimulationOperationSurfaceV1::Unreachable,
    }
}

const fn scalar_owner(ty: ScalarType) -> SimulationSemanticOwnerV1 {
    if ty.is_float() {
        SimulationSemanticOwnerV1::SoftwareFloat
    } else {
        SimulationSemanticOwnerV1::ScalarBits
    }
}

const fn cast_owner(kind: CastKind) -> SimulationSemanticOwnerV1 {
    match kind {
        CastKind::FloatExtend
        | CastKind::FloatTruncate
        | CastKind::IntegerToFloat
        | CastKind::FloatToInteger => SimulationSemanticOwnerV1::SoftwareFloat,
        CastKind::Truncate | CastKind::ZeroExtend | CastKind::SignExtend | CastKind::Bitcast => {
            SimulationSemanticOwnerV1::ScalarBits
        }
    }
}

const fn scalar_name(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Bool => "bool",
        ScalarType::I8 => "i8",
        ScalarType::I16 => "i16",
        ScalarType::I32 => "i32",
        ScalarType::I64 => "i64",
        ScalarType::I128 => "i128",
        ScalarType::U8 => "u8",
        ScalarType::U16 => "u16",
        ScalarType::U32 => "u32",
        ScalarType::U64 => "u64",
        ScalarType::U128 => "u128",
        ScalarType::Index => "index",
        ScalarType::F16 => "f16",
        ScalarType::Bf16 => "bf16",
        ScalarType::F32 => "f32",
        ScalarType::F64 => "f64",
    }
}

const fn unary_name(operation: UnaryOp) -> &'static str {
    match operation {
        UnaryOp::Negate => "negate",
        UnaryOp::Not => "not",
    }
}

const fn binary_name(operation: BinaryOp) -> &'static str {
    match operation {
        BinaryOp::Add => "add",
        BinaryOp::Subtract => "subtract",
        BinaryOp::Multiply => "multiply",
        BinaryOp::Divide => "divide",
        BinaryOp::Remainder => "remainder",
        BinaryOp::BitAnd => "bit_and",
        BinaryOp::BitOr => "bit_or",
        BinaryOp::BitXor => "bit_xor",
        BinaryOp::ShiftLeft => "shift_left",
        BinaryOp::ShiftRight => "shift_right",
        BinaryOp::Checked(CheckedBinaryOperator::Add) => "checked_add",
        BinaryOp::Checked(CheckedBinaryOperator::Subtract) => "checked_subtract",
        BinaryOp::Checked(CheckedBinaryOperator::Multiply) => "checked_multiply",
    }
}

const fn compare_name(operation: ComparePredicate) -> &'static str {
    match operation {
        ComparePredicate::Equal => "equal",
        ComparePredicate::NotEqual => "not_equal",
        ComparePredicate::LessThan => "less_than",
        ComparePredicate::LessThanOrEqual => "less_than_or_equal",
        ComparePredicate::GreaterThan => "greater_than",
        ComparePredicate::GreaterThanOrEqual => "greater_than_or_equal",
    }
}

const fn cast_name(operation: CastKind) -> &'static str {
    match operation {
        CastKind::Truncate => "truncate",
        CastKind::ZeroExtend => "zero_extend",
        CastKind::SignExtend => "sign_extend",
        CastKind::FloatExtend => "float_extend",
        CastKind::FloatTruncate => "float_truncate",
        CastKind::IntegerToFloat => "integer_to_float",
        CastKind::FloatToInteger => "float_to_integer",
        CastKind::Bitcast => "bitcast",
    }
}
