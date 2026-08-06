use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, LaunchDomain, MemoryOrdering, ScalarType,
    SynchronizationScope, TargetCapability, Type, WaveWidth, WorkgroupSize,
};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

string_id!(ModuleId);
string_id!(FunctionId);
string_id!(KernelId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(pub u32);

impl fmt::Display for BlockId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "bb{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueId(pub u32);

impl fmt::Display for ValueId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "%{}", self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub id: ModuleId,
    pub functions: Vec<Function>,
    pub kernels: Vec<Kernel>,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

impl Module {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            functions: Vec::new(),
            kernels: Vec::new(),
            required_capabilities: BTreeSet::new(),
        }
    }

    pub fn function(&self, id: &FunctionId) -> Option<&Function> {
        self.functions.iter().find(|function| &function.id == id)
    }

    /// Capabilities implied by operations in the module's defined functions.
    pub fn derived_capabilities(&self) -> BTreeSet<TargetCapability> {
        self.functions
            .iter()
            .flat_map(Function::derived_capabilities)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    pub parameters: Vec<Type>,
    pub results: Vec<Type>,
}

impl Signature {
    pub fn new(parameters: Vec<Type>, results: Vec<Type>) -> Self {
        Self {
            parameters,
            results,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub signature: Signature,
    /// Semantic linkage and entry role. This is never inferred from identity or reachability.
    pub role: FunctionRole,
    /// Definitions have a body; [`FunctionRole::ExternalImport`] does not.
    pub body: Option<FunctionBody>,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

/// Explicit semantic role of a function in one complete kernel-IR module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionRole {
    KernelEntry,
    InternalHelper,
    DeviceFfiExport,
    ExternalImport,
}

impl Function {
    pub fn definition(
        id: impl Into<FunctionId>,
        signature: Signature,
        parameters: Vec<ValueId>,
        blocks: Vec<BasicBlock>,
    ) -> Self {
        Self::internal_helper(id, signature, parameters, blocks)
    }

    pub fn kernel_entry(
        id: impl Into<FunctionId>,
        signature: Signature,
        parameters: Vec<ValueId>,
        blocks: Vec<BasicBlock>,
    ) -> Self {
        Self::definition_with_role(id, signature, parameters, blocks, FunctionRole::KernelEntry)
    }

    pub fn internal_helper(
        id: impl Into<FunctionId>,
        signature: Signature,
        parameters: Vec<ValueId>,
        blocks: Vec<BasicBlock>,
    ) -> Self {
        Self::definition_with_role(
            id,
            signature,
            parameters,
            blocks,
            FunctionRole::InternalHelper,
        )
    }

    pub fn device_ffi_export(
        id: impl Into<FunctionId>,
        signature: Signature,
        parameters: Vec<ValueId>,
        blocks: Vec<BasicBlock>,
    ) -> Self {
        Self::definition_with_role(
            id,
            signature,
            parameters,
            blocks,
            FunctionRole::DeviceFfiExport,
        )
    }

    fn definition_with_role(
        id: impl Into<FunctionId>,
        signature: Signature,
        parameters: Vec<ValueId>,
        blocks: Vec<BasicBlock>,
        role: FunctionRole,
    ) -> Self {
        Self {
            id: id.into(),
            signature,
            role,
            body: Some(FunctionBody { parameters, blocks }),
            required_capabilities: BTreeSet::new(),
        }
    }

    pub fn declaration(id: impl Into<FunctionId>, signature: Signature) -> Self {
        Self::external_import(id, signature)
    }

    pub fn external_import(id: impl Into<FunctionId>, signature: Signature) -> Self {
        Self {
            id: id.into(),
            signature,
            role: FunctionRole::ExternalImport,
            body: None,
            required_capabilities: BTreeSet::new(),
        }
    }

    /// Capabilities implied by operations in this function body.
    pub fn derived_capabilities(&self) -> BTreeSet<TargetCapability> {
        let Some(body) = &self.body else {
            return BTreeSet::new();
        };
        let mut value_types = body
            .parameters
            .iter()
            .copied()
            .zip(self.signature.parameters.iter().cloned())
            .collect::<BTreeMap<_, _>>();
        for block in &body.blocks {
            value_types.extend(
                block
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.id, parameter.ty.clone())),
            );
            value_types.extend(
                block
                    .operations
                    .iter()
                    .flat_map(|operation| &operation.results)
                    .map(|result| (result.id, result.ty.clone())),
            );
        }

        let mut capabilities = BTreeSet::new();
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            capabilities.extend(operation.required_capabilities());
            let OperationKind::Atomic(atomic) = &operation.kind else {
                continue;
            };
            let Some(Type::Pointer(pointer)) = value_types.get(&atomic.pointer) else {
                continue;
            };
            let Some(width_bits) = pointer.pointee.as_scalar().and_then(ScalarType::bit_width)
            else {
                continue;
            };
            if matches!(width_bits, 8 | 16 | 32 | 64) {
                capabilities.insert(TargetCapability::Atomic {
                    width_bits,
                    address_space: atomic.access.address_space,
                    max_scope: atomic.scope,
                });
            }
        }
        capabilities
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionBody {
    /// SSA identities corresponding positionally to the function signature.
    pub parameters: Vec<ValueId>,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Kernel {
    pub id: KernelId,
    pub entry: FunctionId,
    pub domain: LaunchDomain,
    pub workgroup_size: Option<WorkgroupSize>,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

impl Kernel {
    pub fn new(
        id: impl Into<KernelId>,
        entry: impl Into<FunctionId>,
        domain: LaunchDomain,
    ) -> Self {
        Self {
            id: id.into(),
            entry: entry.into(),
            domain,
            workgroup_size: None,
            required_capabilities: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub parameters: Vec<ValueDef>,
    pub operations: Vec<Operation>,
    /// Kept optional so malformed frontend output can be diagnosed.
    pub terminator: Option<Terminator>,
}

impl BasicBlock {
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueDef {
    pub id: ValueId,
    pub ty: Type,
}

impl ValueDef {
    pub fn new(id: ValueId, ty: Type) -> Self {
        Self { id, ty }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Operation {
    pub results: Vec<ValueDef>,
    pub kind: OperationKind,
}

impl Operation {
    pub fn new(results: Vec<ValueDef>, kind: OperationKind) -> Self {
        Self { results, kind }
    }

    pub fn effect_free(result: ValueDef, kind: OperationKind) -> Self {
        Self::new(vec![result], kind)
    }

    pub fn memory_effects(&self) -> Vec<MemoryEffect> {
        match &self.kind {
            OperationKind::Intrinsic(intrinsic) => intrinsic
                .metadata()
                .memory_effects
                .effects()
                .iter()
                .cloned()
                .collect(),
            OperationKind::Alloca { address_space, .. } => {
                vec![MemoryEffect::Allocate(*address_space)]
            }
            OperationKind::Load { access, .. } => vec![MemoryEffect::Read(access.address_space)],
            OperationKind::Store { access, .. } => vec![MemoryEffect::Write(access.address_space)],
            OperationKind::Atomic(atomic) => vec![MemoryEffect::Atomic {
                address_space: atomic.access.address_space,
                scope: atomic.scope,
                ordering: atomic.ordering,
            }],
            OperationKind::Barrier(barrier) => vec![MemoryEffect::Synchronize {
                execution_scope: barrier.execution_scope,
                memory_scope: barrier.memory_scope,
                address_spaces: barrier.semantics.address_spaces.clone(),
            }],
            OperationKind::Fence(fence) => vec![MemoryEffect::Fence {
                memory_scope: fence.memory_scope,
                ordering: fence.semantics.ordering,
                address_spaces: fence.semantics.address_spaces.clone(),
            }],
            OperationKind::WorkgroupBarrier(barrier) => {
                vec![MemoryEffect::Synchronize {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: barrier.memory_scope,
                    address_spaces: barrier.semantics.address_spaces.clone(),
                }]
            }
            OperationKind::WorkgroupMemory(_) => {
                vec![MemoryEffect::Allocate(AddressSpace::Workgroup)]
            }
            OperationKind::Wave(_) => Vec::new(),
            _ => Vec::new(),
        }
    }

    pub fn effect_summary(&self) -> MemoryEffectSummary {
        MemoryEffectSummary::new(self.memory_effects())
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        match &self.kind {
            OperationKind::Intrinsic(intrinsic) => intrinsic.metadata().required_capabilities,
            OperationKind::Alloca {
                count,
                address_space: AddressSpace::Workgroup,
                ..
            } => {
                let mut capabilities = BTreeSet::from([TargetCapability::WorkgroupMemory]);
                if count.is_some() {
                    capabilities.insert(TargetCapability::DynamicWorkgroupMemory);
                }
                capabilities
            }
            OperationKind::Barrier(barrier) => match barrier.execution_scope {
                SynchronizationScope::Subgroup => BTreeSet::from([TargetCapability::Subgroups]),
                SynchronizationScope::Workgroup => {
                    BTreeSet::from([TargetCapability::WorkgroupBarrier])
                }
                _ => BTreeSet::new(),
            },
            OperationKind::Fence(fence) if fence.memory_scope == SynchronizationScope::Subgroup => {
                BTreeSet::from([TargetCapability::Subgroups])
            }
            OperationKind::WorkgroupBarrier(_) => {
                BTreeSet::from([TargetCapability::WorkgroupBarrier])
            }
            OperationKind::WorkgroupMemory(memory) => {
                let mut capabilities = BTreeSet::from([TargetCapability::WorkgroupMemory]);
                if memory.extent == WorkgroupMemoryExtent::Dynamic {
                    capabilities.insert(TargetCapability::DynamicWorkgroupMemory);
                }
                capabilities
            }
            OperationKind::Wave(wave) => wave.required_capabilities(),
            _ => BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Constant(Constant),
    Intrinsic(IntrinsicOperation),
    Unary {
        op: UnaryOp,
        operand: ValueId,
    },
    Binary {
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    Compare {
        predicate: ComparePredicate,
        lhs: ValueId,
        rhs: ValueId,
    },
    Cast {
        kind: CastKind,
        value: ValueId,
        to: Type,
    },
    Select {
        condition: ValueId,
        true_value: ValueId,
        false_value: ValueId,
    },
    Call {
        callee: FunctionId,
        arguments: Vec<ValueId>,
    },
    Alloca {
        element: Type,
        count: Option<ValueId>,
        address_space: AddressSpace,
        alignment: u32,
    },
    SliceLength {
        slice: ValueId,
    },
    SliceData {
        slice: ValueId,
    },
    GetElementPointer {
        base: ValueId,
        offset: ValueId,
    },
    Load {
        pointer: ValueId,
        access: MemoryAccess,
    },
    Store {
        pointer: ValueId,
        value: ValueId,
        access: MemoryAccess,
    },
    Barrier(Barrier),
    Atomic(Atomic),
    /// A memory-ordering fence without execution synchronization.
    Fence(Fence),
    /// An execution and memory barrier reached uniformly by a workgroup.
    WorkgroupBarrier(WorkgroupBarrier),
    /// A statically or dynamically sized workgroup-memory declaration.
    WorkgroupMemory(WorkgroupMemory),
    /// A width-bound, convergent operation over one physical AMD-style wave.
    Wave(WaveOperation),
}

impl OperationKind {
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            Self::Constant(_)
            | Self::Intrinsic(_)
            | Self::Barrier(_)
            | Self::Fence(_)
            | Self::WorkgroupBarrier(_)
            | Self::WorkgroupMemory(_)
            | Self::Wave(WaveOperation {
                kind: WaveOperationKind::LaneId,
                ..
            }) => Vec::new(),
            Self::Unary { operand, .. } => vec![*operand],
            Self::Binary { lhs, rhs, .. } | Self::Compare { lhs, rhs, .. } => vec![*lhs, *rhs],
            Self::Cast { value, .. } => vec![*value],
            Self::Select {
                condition,
                true_value,
                false_value,
            } => vec![*condition, *true_value, *false_value],
            Self::Call { arguments, .. } => arguments.clone(),
            Self::Alloca { count, .. } => count.iter().copied().collect(),
            Self::SliceLength { slice } | Self::SliceData { slice } => vec![*slice],
            Self::GetElementPointer { base, offset } => vec![*base, *offset],
            Self::Load { pointer, .. } => vec![*pointer],
            Self::Store { pointer, value, .. } => vec![*pointer, *value],
            Self::Atomic(atomic) => atomic.operands(),
            Self::Wave(wave) => wave.operands(),
        }
    }
}

/// A target-neutral GPU intrinsic recognized by the core kernel IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntrinsicKind {
    /// An invocation coordinate at the selected launch hierarchy level.
    InvocationIndex { kind: IndexKind, axis: Axis },
    /// The number of logical invocations in the launch dimension.
    LaunchExtent { axis: Axis },
}

impl IntrinsicKind {
    pub const fn axis(self) -> Axis {
        match self {
            Self::InvocationIndex { axis, .. } | Self::LaunchExtent { axis } => axis,
        }
    }

    pub fn metadata(self) -> IntrinsicMetadata {
        IntrinsicMetadata {
            result_type: Type::INDEX,
            memory_effects: MemoryEffectSummary::pure(),
            required_capabilities: BTreeSet::new(),
        }
    }
}

/// An intrinsic invocation with an explicit frontend-provided result type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntrinsicOperation {
    pub kind: IntrinsicKind,
    pub result_type: Type,
}

impl IntrinsicOperation {
    pub fn new(kind: IntrinsicKind, result_type: Type) -> Self {
        Self { kind, result_type }
    }

    pub fn global_id_1d() -> Self {
        Self::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
            Type::INDEX,
        )
    }

    pub fn launch_extent_1d() -> Self {
        Self::new(IntrinsicKind::LaunchExtent { axis: Axis::X }, Type::INDEX)
    }

    pub fn metadata(&self) -> IntrinsicMetadata {
        self.kind.metadata()
    }
}

/// Canonical semantic metadata for one intrinsic kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntrinsicMetadata {
    pub result_type: Type,
    pub memory_effects: MemoryEffectSummary,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IndexKind {
    Global,
    Workgroup,
    Local,
    WorkgroupSize,
    WorkgroupCount,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Constant {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Index(u64),
    F16Bits(u16),
    Bf16Bits(u16),
    F32Bits(u32),
    F64Bits(u64),
}

impl Constant {
    pub const fn ty(&self) -> Type {
        let scalar = match self {
            Self::Bool(_) => ScalarType::Bool,
            Self::I8(_) => ScalarType::I8,
            Self::I16(_) => ScalarType::I16,
            Self::I32(_) => ScalarType::I32,
            Self::I64(_) => ScalarType::I64,
            Self::U8(_) => ScalarType::U8,
            Self::U16(_) => ScalarType::U16,
            Self::U32(_) => ScalarType::U32,
            Self::U64(_) => ScalarType::U64,
            Self::Index(_) => ScalarType::Index,
            Self::F16Bits(_) => ScalarType::F16,
            Self::Bf16Bits(_) => ScalarType::Bf16,
            Self::F32Bits(_) => ScalarType::F32,
            Self::F64Bits(_) => ScalarType::F64,
        };
        Type::Scalar(scalar)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ComparePredicate {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CastKind {
    Truncate,
    ZeroExtend,
    SignExtend,
    FloatExtend,
    FloatTruncate,
    IntegerToFloat,
    FloatToInteger,
    Bitcast,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryAccess {
    pub address_space: AddressSpace,
    pub alignment: u32,
    pub volatile: bool,
}

impl MemoryAccess {
    pub const fn new(address_space: AddressSpace, alignment: u32) -> Self {
        Self {
            address_space,
            alignment,
            volatile: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Barrier {
    pub execution_scope: SynchronizationScope,
    pub memory_scope: SynchronizationScope,
    pub semantics: BarrierSemantics,
}

/// Frontend evidence that a convergent operation is reached uniformly.
///
/// The core verifier checks that the claimed scope matches the operation.
/// Establishing that the claim is true is the responsibility of uniformity
/// analysis or a proof artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Convergence {
    Uniform { scope: SynchronizationScope },
}

impl Convergence {
    pub const fn uniform(scope: SynchronizationScope) -> Self {
        Self::Uniform { scope }
    }

    pub const fn scope(self) -> SynchronizationScope {
        match self {
            Self::Uniform { scope } => scope,
        }
    }
}

/// A scoped memory fence. Unlike a barrier, this does not synchronize which
/// invocations execute the operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fence {
    pub memory_scope: SynchronizationScope,
    pub semantics: BarrierSemantics,
}

/// A workgroup execution barrier carrying an explicit convergence claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkgroupBarrier {
    pub memory_scope: SynchronizationScope,
    pub semantics: BarrierSemantics,
    pub convergence: Convergence,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkgroupMemoryExtent {
    Static(u32),
    Dynamic,
}

/// One explicit LDS allocation visible to all invocations in a workgroup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkgroupMemory {
    pub element: Type,
    pub extent: WorkgroupMemoryExtent,
    pub alignment: u32,
}

/// A physical-wave operation with explicit execution assumptions.
///
/// `active_lanes` is intentionally explicit even though this first executable
/// subset accepts only a full physical wave. This prevents a consumer from
/// silently treating a partially active final wave as if every source lane
/// were available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaveOperation {
    pub kind: WaveOperationKind,
    pub width: WaveWidth,
    pub active_lanes: u32,
    pub convergence: Convergence,
}

impl WaveOperation {
    pub fn full(kind: WaveOperationKind, width: WaveWidth) -> Self {
        Self {
            kind,
            width,
            active_lanes: width.lanes(),
            convergence: Convergence::uniform(SynchronizationScope::Subgroup),
        }
    }

    pub fn operands(&self) -> Vec<ValueId> {
        match self.kind {
            WaveOperationKind::LaneId => Vec::new(),
            WaveOperationKind::Ballot { predicate }
            | WaveOperationKind::Any { predicate }
            | WaveOperationKind::All { predicate } => vec![predicate],
            WaveOperationKind::ShuffleIndex {
                value, source_lane, ..
            } => vec![value, source_lane],
        }
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        BTreeSet::from([
            TargetCapability::Subgroups,
            TargetCapability::SubgroupSize(self.width.lanes()),
            TargetCapability::WaveWidth(self.width),
        ])
    }
}

/// The bounded first wave-operation vertical.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaveOperationKind {
    /// The physical lane number in `[0, width)`.
    LaneId,
    /// A bit per active lane whose predicate is true.
    Ballot { predicate: ValueId },
    /// True when at least one active lane's predicate is true.
    Any { predicate: ValueId },
    /// True when every active lane's predicate is true.
    All { predicate: ValueId },
    /// Read a 32-bit integer from an indexed lane in a static tiled subgroup.
    ShuffleIndex {
        value: ValueId,
        source_lane: ValueId,
        tile_width: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AtomicKind {
    Load,
    Store,
    Exchange,
    CompareExchange,
    Add,
    Subtract,
    Min,
    Max,
    BitAnd,
    BitOr,
    BitXor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Atomic {
    pub kind: AtomicKind,
    pub pointer: ValueId,
    /// The stored value, desired compare-exchange value, or RMW operand.
    pub value: Option<ValueId>,
    /// The expected value for compare-exchange.
    pub compare: Option<ValueId>,
    pub access: MemoryAccess,
    pub scope: SynchronizationScope,
    pub ordering: MemoryOrdering,
    pub failure_ordering: Option<MemoryOrdering>,
}

impl Atomic {
    pub fn operands(&self) -> Vec<ValueId> {
        let mut values = vec![self.pointer];
        values.extend(self.value);
        values.extend(self.compare);
        values
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Branch {
        target: BlockId,
        arguments: Vec<ValueId>,
    },
    ConditionalBranch {
        condition: ValueId,
        then_target: BlockId,
        then_arguments: Vec<ValueId>,
        else_target: BlockId,
        else_arguments: Vec<ValueId>,
    },
    Switch {
        selector: ValueId,
        cases: Vec<SwitchCase>,
        default_target: BlockId,
        default_arguments: Vec<ValueId>,
    },
    /// A V2 integer switch with typed, strictly increasing case constants.
    IntegerSwitch {
        selector: ValueId,
        cases: Vec<IntegerSwitchCase>,
        default_target: BlockId,
        default_arguments: Vec<ValueId>,
    },
    Return {
        values: Vec<ValueId>,
    },
    Unreachable,
}

impl Terminator {
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            Self::Branch { arguments, .. } => arguments.clone(),
            Self::ConditionalBranch {
                condition,
                then_arguments,
                else_arguments,
                ..
            } => {
                let mut operands = vec![*condition];
                operands.extend(then_arguments);
                operands.extend(else_arguments);
                operands
            }
            Self::Switch {
                selector,
                cases,
                default_arguments,
                ..
            } => {
                let mut operands = vec![*selector];
                for case in cases {
                    operands.extend(&case.arguments);
                }
                operands.extend(default_arguments);
                operands
            }
            Self::IntegerSwitch {
                selector,
                cases,
                default_arguments,
                ..
            } => {
                let mut operands = vec![*selector];
                for case in cases {
                    operands.extend(&case.arguments);
                }
                operands.extend(default_arguments);
                operands
            }
            Self::Return { values } => values.clone(),
            Self::Unreachable => Vec::new(),
        }
    }

    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Self::Branch { target, .. } => vec![*target],
            Self::ConditionalBranch {
                then_target,
                else_target,
                ..
            } => vec![*then_target, *else_target],
            Self::Switch {
                cases,
                default_target,
                ..
            } => cases
                .iter()
                .map(|case| case.target)
                .chain([*default_target])
                .collect(),
            Self::IntegerSwitch {
                cases,
                default_target,
                ..
            } => cases
                .iter()
                .map(|case| case.target)
                .chain([*default_target])
                .collect(),
            Self::Return { .. } | Self::Unreachable => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwitchCase {
    pub value: u64,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
}

/// One typed case of a V2 [`Terminator::IntegerSwitch`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerSwitchCase {
    pub value: Constant,
    pub target: BlockId,
    pub arguments: Vec<ValueId>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryEffect {
    Allocate(AddressSpace),
    Read(AddressSpace),
    Write(AddressSpace),
    Atomic {
        address_space: AddressSpace,
        scope: SynchronizationScope,
        ordering: MemoryOrdering,
    },
    Synchronize {
        execution_scope: SynchronizationScope,
        memory_scope: SynchronizationScope,
        address_spaces: BTreeSet<AddressSpace>,
    },
    Fence {
        memory_scope: SynchronizationScope,
        ordering: MemoryOrdering,
        address_spaces: BTreeSet<AddressSpace>,
    },
}

/// A deterministic, queryable summary of an operation's memory effects.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemoryEffectSummary {
    effects: BTreeSet<MemoryEffect>,
}

impl MemoryEffectSummary {
    pub fn new(effects: impl IntoIterator<Item = MemoryEffect>) -> Self {
        Self {
            effects: effects.into_iter().collect(),
        }
    }

    pub const fn pure() -> Self {
        Self {
            effects: BTreeSet::new(),
        }
    }

    pub fn is_pure(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn reads(&self, address_space: AddressSpace) -> bool {
        self.effects.contains(&MemoryEffect::Read(address_space))
    }

    pub fn writes(&self, address_space: AddressSpace) -> bool {
        self.effects.contains(&MemoryEffect::Write(address_space))
    }

    pub fn effects(&self) -> &BTreeSet<MemoryEffect> {
        &self.effects
    }
}

pub(crate) fn pointer_for(pointee: Type, address_space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(pointee, address_space, access)
}
