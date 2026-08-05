use std::collections::BTreeSet;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, LaunchDomain, MemoryOrdering, ScalarType,
    SynchronizationScope, TargetCapability, Type, WorkgroupSize,
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
    /// `None` represents an external declaration.
    pub body: Option<FunctionBody>,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

impl Function {
    pub fn definition(
        id: impl Into<FunctionId>,
        signature: Signature,
        parameters: Vec<ValueId>,
        blocks: Vec<BasicBlock>,
    ) -> Self {
        Self {
            id: id.into(),
            signature,
            body: Some(FunctionBody { parameters, blocks }),
            required_capabilities: BTreeSet::new(),
        }
    }

    pub fn declaration(id: impl Into<FunctionId>, signature: Signature) -> Self {
        Self {
            id: id.into(),
            signature,
            body: None,
            required_capabilities: BTreeSet::new(),
        }
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
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Constant(Constant),
    InvocationIndex {
        kind: IndexKind,
        axis: Axis,
    },
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
}

impl OperationKind {
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            Self::Constant(_) | Self::InvocationIndex { .. } | Self::Barrier(_) => Vec::new(),
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
        }
    }
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
}

pub(crate) fn pointer_for(pointee: Type, address_space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(pointee, address_space, access)
}
