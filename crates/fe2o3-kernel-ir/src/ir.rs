use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BarrierSemantics, LaunchDomain, MemoryIntrinsicOperation,
    MemoryOrdering, ScalarType, SemanticOperation, SynchronizationScope, TargetCapability, Type,
    WaveWidth, WorkgroupSize,
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

    /// All capabilities declared by or semantically required by this module.
    ///
    /// Consumers that construct target or artifact requirements must use this
    /// closure rather than trusting frontend-supplied declarations alone.
    pub fn effective_capabilities(&self) -> BTreeSet<TargetCapability> {
        self.required_capabilities
            .iter()
            .cloned()
            .chain(self.derived_capabilities())
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

    /// All capabilities declared by or semantically required by this function.
    pub fn effective_capabilities(&self) -> BTreeSet<TargetCapability> {
        self.required_capabilities
            .iter()
            .cloned()
            .chain(self.derived_capabilities())
            .collect()
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
        if let Some(semantic) = self.kind.semantic_operation() {
            return semantic.contract().memory_effects;
        }
        match &self.kind {
            OperationKind::Intrinsic(_) => {
                unreachable!("semantic operations return before legacy operation dispatch")
            }
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
            OperationKind::InlineAssembly(assembly) => assembly.memory_effects(),
            OperationKind::Wave(_) => Vec::new(),
            _ => Vec::new(),
        }
    }

    pub fn effect_summary(&self) -> MemoryEffectSummary {
        MemoryEffectSummary::new(self.memory_effects())
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        if let Some(semantic) = self.kind.semantic_operation() {
            return semantic.contract().required_capabilities;
        }
        match &self.kind {
            OperationKind::Intrinsic(_) => {
                unreachable!("semantic operations return before legacy operation dispatch")
            }
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
            OperationKind::Barrier(barrier) => {
                let mut capabilities = match barrier.execution_scope {
                    SynchronizationScope::Subgroup => BTreeSet::from([TargetCapability::Subgroups]),
                    SynchronizationScope::Workgroup => {
                        BTreeSet::from([TargetCapability::WorkgroupBarrier])
                    }
                    _ => BTreeSet::new(),
                };
                add_synchronized_memory_capabilities(
                    &mut capabilities,
                    &barrier.semantics.address_spaces,
                );
                capabilities
            }
            OperationKind::Fence(fence) => {
                let mut capabilities = BTreeSet::new();
                if fence.memory_scope == SynchronizationScope::Subgroup {
                    capabilities.insert(TargetCapability::Subgroups);
                }
                add_synchronized_memory_capabilities(
                    &mut capabilities,
                    &fence.semantics.address_spaces,
                );
                capabilities
            }
            OperationKind::WorkgroupBarrier(barrier) => {
                let mut capabilities = BTreeSet::from([TargetCapability::WorkgroupBarrier]);
                add_synchronized_memory_capabilities(
                    &mut capabilities,
                    &barrier.semantics.address_spaces,
                );
                capabilities
            }
            OperationKind::WorkgroupMemory(memory) => {
                let mut capabilities = BTreeSet::from([TargetCapability::WorkgroupMemory]);
                if memory.extent == WorkgroupMemoryExtent::Dynamic {
                    capabilities.insert(TargetCapability::DynamicWorkgroupMemory);
                }
                capabilities
            }
            OperationKind::Wave(wave) => wave.required_capabilities(),
            OperationKind::InlineAssembly(assembly) => assembly.required_capabilities(),
            OperationKind::Call { callee, arguments } => {
                FloatOperation::from_intrinsic_call(callee, arguments)
                    .map_or_else(BTreeSet::new, |float| float.required_capabilities())
            }
            _ => BTreeSet::new(),
        }
    }
}

fn add_synchronized_memory_capabilities(
    capabilities: &mut BTreeSet<TargetCapability>,
    address_spaces: &BTreeSet<AddressSpace>,
) {
    if address_spaces.contains(&AddressSpace::Workgroup) {
        capabilities.insert(TargetCapability::WorkgroupMemory);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Constant(Constant),
    Intrinsic(IntrinsicOperation),
    MemoryIntrinsic(MemoryIntrinsicOperation),
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
    /// Source-bound target assembly whose authority was established by the frontend.
    InlineAssembly(InlineAssembly),
}

impl OperationKind {
    /// Returns the common target-neutral contract for registered semantic operations.
    pub fn semantic_operation(&self) -> Option<&dyn SemanticOperation> {
        match self {
            Self::Intrinsic(intrinsic) => Some(intrinsic),
            Self::MemoryIntrinsic(intrinsic) => Some(intrinsic),
            _ => None,
        }
    }

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
            Self::MemoryIntrinsic(intrinsic) => intrinsic.operands(),
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
            Self::InlineAssembly(assembly) => assembly.operands(),
        }
    }
}

/// Target capability required by the first authenticated gfx942 assembly profile.
pub const AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE: &str = "fe2o3.amdgpu";
pub const AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME: &str =
    "authenticated-inline-assembly.gfx942.v1";

/// Exact compiler identities that bind one assembly statement to authenticated source records.
///
/// These digests are evidence references, not self-authenticating flags. A compiler bridge must
/// construct them from the already authenticated frontend unit, monomorphized function, kernel
/// contract, and statement record. Backends consume the bound semantic operation and never grant
/// authority based on a symbol or mnemonic spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssemblySourceIdentity {
    pub frontend_unit: [u8; 32],
    pub function: [u8; 32],
    pub contract: [u8; 32],
    pub statement: [u8; 32],
}

impl AssemblySourceIdentity {
    pub const fn new(
        frontend_unit: [u8; 32],
        function: [u8; 32],
        contract: [u8; 32],
        statement: [u8; 32],
    ) -> Self {
        Self {
            frontend_unit,
            function,
            contract,
            statement,
        }
    }

    pub fn is_complete(self) -> bool {
        [
            self.frontend_unit,
            self.function,
            self.contract,
            self.statement,
        ]
        .into_iter()
        .all(|identity| identity != [0; 32])
    }
}

/// Assembly syntax and register model selected by an authenticated source contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InlineAssemblyTarget {
    AmdGpuGfx942,
}

/// Exact operand constraint accepted by the bounded assembly carrier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssemblyConstraint {
    Sgpr32,
    Vgpr32,
    ImmediateI32,
}

/// The SSA role of one source-order assembly operand.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssemblyOperandKind {
    Input(ValueId),
    Output { result_index: u32 },
    InOut { input: ValueId, result_index: u32 },
    ImmediateI32(i32),
}

/// One source-order operand with its authenticated target constraint.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssemblyOperand {
    pub kind: AssemblyOperandKind,
    pub constraint: AssemblyConstraint,
}

impl AssemblyOperand {
    pub const fn input(value: ValueId, constraint: AssemblyConstraint) -> Self {
        Self {
            kind: AssemblyOperandKind::Input(value),
            constraint,
        }
    }

    pub const fn output(result_index: u32, constraint: AssemblyConstraint) -> Self {
        Self {
            kind: AssemblyOperandKind::Output { result_index },
            constraint,
        }
    }
}

/// Source options whose meaning must survive target lowering exactly.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssemblyOption {
    NoMemory,
    ReadOnly,
    Pure,
    PreservesFlags,
    NoStack,
}

/// Effects declared by the authenticated source contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AssemblyEffect {
    ReadGlobal,
    WriteGlobal,
    ReadWorkgroup,
    WriteWorkgroup,
    Atomic,
    Barrier,
    ControlFlow,
}

/// One source-bound assembly statement.
///
/// The mnemonic is data to be validated by a target backend. It is never a trust token and is
/// never emitted verbatim unless the backend recognizes its complete operand/effect contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineAssembly {
    pub target: InlineAssemblyTarget,
    pub source: AssemblySourceIdentity,
    pub mnemonic: String,
    pub operands: Vec<AssemblyOperand>,
    pub options: BTreeSet<AssemblyOption>,
    pub declared_effects: BTreeSet<AssemblyEffect>,
}

impl InlineAssembly {
    pub fn operands(&self) -> Vec<ValueId> {
        self.operands
            .iter()
            .filter_map(|operand| match operand.kind {
                AssemblyOperandKind::Input(value)
                | AssemblyOperandKind::InOut { input: value, .. } => Some(value),
                AssemblyOperandKind::Output { .. } | AssemblyOperandKind::ImmediateI32(_) => None,
            })
            .collect()
    }

    pub fn memory_effects(&self) -> Vec<MemoryEffect> {
        self.declared_effects
            .iter()
            .filter_map(|effect| match effect {
                AssemblyEffect::ReadGlobal => Some(MemoryEffect::Read(AddressSpace::Global)),
                AssemblyEffect::WriteGlobal => Some(MemoryEffect::Write(AddressSpace::Global)),
                AssemblyEffect::ReadWorkgroup => Some(MemoryEffect::Read(AddressSpace::Workgroup)),
                AssemblyEffect::WriteWorkgroup => {
                    Some(MemoryEffect::Write(AddressSpace::Workgroup))
                }
                AssemblyEffect::Atomic | AssemblyEffect::Barrier | AssemblyEffect::ControlFlow => {
                    None
                }
            })
            .collect()
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        BTreeSet::from([TargetCapability::Extension {
            namespace: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME.to_owned(),
        }])
    }
}

/// One exact conversion between the integer-backed narrow-float values and `f32`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatConversionKind {
    F16ToF32,
    F32ToF16RoundTiesEven,
    Bf16ToF32,
    F32ToBf16RoundTiesEven,
}

/// The integer-backed narrow format used by widened arithmetic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NarrowFloatFormat {
    F16,
    Bf16,
}

impl NarrowFloatFormat {
    pub const fn ty(self) -> Type {
        match self {
            Self::F16 => Type::Scalar(ScalarType::F16),
            Self::Bf16 => Type::Scalar(ScalarType::Bf16),
        }
    }

    pub const fn capability(self) -> TargetCapability {
        match self {
            Self::F16 => TargetCapability::Float16,
            Self::Bf16 => TargetCapability::BFloat16,
        }
    }
}

/// Binary arithmetic performed after exact widening to `f32`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WidenedFloatBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}

/// Scalar functions exposed by `fe2o3-device::DeviceMath`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum F32MathFunction {
    Sqrt,
    FusedMultiplyAdd,
    Floor,
    Ceil,
    Truncate,
    RoundTiesEven,
    Sin,
    Cos,
    Exp,
    Exp2,
    Ln,
    Log2,
    Log10,
}

impl F32MathFunction {
    pub const fn arity(self) -> usize {
        match self {
            Self::FusedMultiplyAdd => 3,
            _ => 1,
        }
    }

    pub const fn required_implementation(self) -> F32MathImplementation {
        match self {
            Self::Sqrt
            | Self::FusedMultiplyAdd
            | Self::Floor
            | Self::Ceil
            | Self::Truncate
            | Self::RoundTiesEven => F32MathImplementation::ConstrainedLlvm,
            Self::Sin
            | Self::Cos
            | Self::Exp
            | Self::Exp2
            | Self::Ln
            | Self::Log2
            | Self::Log10 => F32MathImplementation::OcmlAbiV1,
        }
    }
}

/// The implementation contract that gives an `f32` math operation meaning.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum F32MathImplementation {
    /// LLVM constrained intrinsics, round-to-nearest-even, ignored exceptions.
    ConstrainedLlvm,
    /// The strict `__ocml_*_f32` ABI, with fast/finite/unsafe modes disabled.
    OcmlAbiV1,
}

/// A pure floating-point operation with no implicit contraction or target fallback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FloatOperation {
    Convert {
        kind: FloatConversionKind,
        value: ValueId,
    },
    /// Widen both operands exactly, perform one constrained `f32` operation,
    /// then narrow once using round-to-nearest, ties-to-even.
    WidenedBinary {
        format: NarrowFloatFormat,
        op: WidenedFloatBinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    F32Math {
        function: F32MathFunction,
        implementation: F32MathImplementation,
        arguments: Vec<ValueId>,
    },
    /// Two independent constrained `f32` FMAs followed by exact BF16 RNE packing.
    /// Each packed operand and the result use lane zero in bits 0..16.
    Bf16x2FusedMultiplyAdd {
        value: ValueId,
        multiplier: ValueId,
        addend: ValueId,
    },
}

impl FloatOperation {
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            Self::Convert { value, .. } => vec![*value],
            Self::WidenedBinary { lhs, rhs, .. } => vec![*lhs, *rhs],
            Self::F32Math { arguments, .. } => arguments.clone(),
            Self::Bf16x2FusedMultiplyAdd {
                value,
                multiplier,
                addend,
            } => vec![*value, *multiplier, *addend],
        }
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        match self {
            Self::Convert { kind, .. } => BTreeSet::from([match kind {
                FloatConversionKind::F16ToF32 | FloatConversionKind::F32ToF16RoundTiesEven => {
                    TargetCapability::Float16
                }
                FloatConversionKind::Bf16ToF32 | FloatConversionKind::F32ToBf16RoundTiesEven => {
                    TargetCapability::BFloat16
                }
            }]),
            Self::WidenedBinary { format, .. } => BTreeSet::from([format.capability()]),
            Self::F32Math { .. } => BTreeSet::new(),
            Self::Bf16x2FusedMultiplyAdd { .. } => BTreeSet::from([TargetCapability::BFloat16]),
        }
    }

    /// Closed semantic identity used to carry this operation through the existing call node.
    pub fn intrinsic_function_id(&self) -> FunctionId {
        FunctionId::new(match self {
            Self::Convert { kind, .. } => match kind {
                FloatConversionKind::F16ToF32 => "__fe2o3_ir_float_v1_f16_to_f32",
                FloatConversionKind::F32ToF16RoundTiesEven => "__fe2o3_ir_float_v1_f32_to_f16_rne",
                FloatConversionKind::Bf16ToF32 => "__fe2o3_ir_float_v1_bf16_to_f32",
                FloatConversionKind::F32ToBf16RoundTiesEven => {
                    "__fe2o3_ir_float_v1_f32_to_bf16_rne"
                }
            },
            Self::WidenedBinary { format, op, .. } => match (format, op) {
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Add) => {
                    "__fe2o3_ir_float_v1_f16_add_widened_rne"
                }
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Subtract) => {
                    "__fe2o3_ir_float_v1_f16_sub_widened_rne"
                }
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Multiply) => {
                    "__fe2o3_ir_float_v1_f16_mul_widened_rne"
                }
                (NarrowFloatFormat::F16, WidenedFloatBinaryOp::Divide) => {
                    "__fe2o3_ir_float_v1_f16_div_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Add) => {
                    "__fe2o3_ir_float_v1_bf16_add_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Subtract) => {
                    "__fe2o3_ir_float_v1_bf16_sub_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Multiply) => {
                    "__fe2o3_ir_float_v1_bf16_mul_widened_rne"
                }
                (NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Divide) => {
                    "__fe2o3_ir_float_v1_bf16_div_widened_rne"
                }
            },
            Self::F32Math {
                function,
                implementation,
                ..
            } if function.required_implementation() != *implementation => {
                "__fe2o3_ir_float_v1_invalid_contract"
            }
            Self::F32Math { function, .. } => match function {
                F32MathFunction::Sqrt => "__fe2o3_ir_float_v1_sqrt_f32",
                F32MathFunction::FusedMultiplyAdd => "__fe2o3_ir_float_v1_fma_f32",
                F32MathFunction::Floor => "__fe2o3_ir_float_v1_floor_f32",
                F32MathFunction::Ceil => "__fe2o3_ir_float_v1_ceil_f32",
                F32MathFunction::Truncate => "__fe2o3_ir_float_v1_trunc_f32",
                F32MathFunction::RoundTiesEven => "__fe2o3_ir_float_v1_roundeven_f32",
                F32MathFunction::Sin => "__fe2o3_ir_float_v1_sin_f32",
                F32MathFunction::Cos => "__fe2o3_ir_float_v1_cos_f32",
                F32MathFunction::Exp => "__fe2o3_ir_float_v1_exp_f32",
                F32MathFunction::Exp2 => "__fe2o3_ir_float_v1_exp2_f32",
                F32MathFunction::Ln => "__fe2o3_ir_float_v1_log_f32",
                F32MathFunction::Log2 => "__fe2o3_ir_float_v1_log2_f32",
                F32MathFunction::Log10 => "__fe2o3_ir_float_v1_log10_f32",
            },
            Self::Bf16x2FusedMultiplyAdd { .. } => "__fe2o3_ir_float_v1_fma_bf16x2",
        })
    }

    pub fn result_type(&self) -> Type {
        match self {
            Self::Convert { kind, .. } => match kind {
                FloatConversionKind::F16ToF32 | FloatConversionKind::Bf16ToF32 => Type::F32,
                FloatConversionKind::F32ToF16RoundTiesEven => Type::Scalar(ScalarType::F16),
                FloatConversionKind::F32ToBf16RoundTiesEven => Type::Scalar(ScalarType::Bf16),
            },
            Self::WidenedBinary { format, .. } => format.ty(),
            Self::F32Math { .. } => Type::F32,
            Self::Bf16x2FusedMultiplyAdd { .. } => Type::Scalar(ScalarType::U32),
        }
    }

    pub fn parameter_types(&self) -> Vec<Type> {
        match self {
            Self::Convert { kind, .. } => vec![match kind {
                FloatConversionKind::F16ToF32 => Type::Scalar(ScalarType::F16),
                FloatConversionKind::F32ToF16RoundTiesEven => Type::F32,
                FloatConversionKind::Bf16ToF32 => Type::Scalar(ScalarType::Bf16),
                FloatConversionKind::F32ToBf16RoundTiesEven => Type::F32,
            }],
            Self::WidenedBinary { format, .. } => vec![format.ty(), format.ty()],
            Self::F32Math { function, .. } => vec![Type::F32; function.arity()],
            Self::Bf16x2FusedMultiplyAdd { .. } => vec![Type::Scalar(ScalarType::U32); 3],
        }
    }

    pub fn declaration(&self) -> Function {
        let mut function = Function::external_import(
            self.intrinsic_function_id(),
            Signature::new(self.parameter_types(), vec![self.result_type()]),
        );
        function.required_capabilities = self.required_capabilities();
        function
    }

    pub fn operation(&self, result: ValueId) -> Operation {
        Operation::effect_free(
            ValueDef::new(result, self.result_type()),
            OperationKind::Call {
                callee: self.intrinsic_function_id(),
                arguments: self.operands(),
            },
        )
    }

    pub fn from_intrinsic_call(callee: &FunctionId, arguments: &[ValueId]) -> Option<Self> {
        let mut float = Self::from_intrinsic_id(callee)?;
        if float.operands().len() != arguments.len() {
            return None;
        }
        match &mut float {
            Self::Convert { value, .. } => *value = arguments[0],
            Self::WidenedBinary { lhs, rhs, .. } => {
                *lhs = arguments[0];
                *rhs = arguments[1];
            }
            Self::F32Math {
                arguments: values, ..
            } => values.clone_from_slice(arguments),
            Self::Bf16x2FusedMultiplyAdd {
                value,
                multiplier,
                addend,
            } => {
                *value = arguments[0];
                *multiplier = arguments[1];
                *addend = arguments[2];
            }
        }
        Some(float)
    }

    pub fn from_intrinsic_id(callee: &FunctionId) -> Option<Self> {
        let values = [ValueId(0), ValueId(1), ValueId(2)];
        let convert = |kind| Self::Convert {
            kind,
            value: values[0],
        };
        let binary = |format, op| Self::WidenedBinary {
            format,
            op,
            lhs: values[0],
            rhs: values[1],
        };
        let math = |function| Self::F32Math {
            function,
            implementation: function.required_implementation(),
            arguments: values[..function.arity()].to_vec(),
        };
        Some(match callee.as_str() {
            "__fe2o3_ir_float_v1_f16_to_f32" => convert(FloatConversionKind::F16ToF32),
            "__fe2o3_ir_float_v1_f32_to_f16_rne" => {
                convert(FloatConversionKind::F32ToF16RoundTiesEven)
            }
            "__fe2o3_ir_float_v1_bf16_to_f32" => convert(FloatConversionKind::Bf16ToF32),
            "__fe2o3_ir_float_v1_f32_to_bf16_rne" => {
                convert(FloatConversionKind::F32ToBf16RoundTiesEven)
            }
            "__fe2o3_ir_float_v1_f16_add_widened_rne" => {
                binary(NarrowFloatFormat::F16, WidenedFloatBinaryOp::Add)
            }
            "__fe2o3_ir_float_v1_f16_sub_widened_rne" => {
                binary(NarrowFloatFormat::F16, WidenedFloatBinaryOp::Subtract)
            }
            "__fe2o3_ir_float_v1_f16_mul_widened_rne" => {
                binary(NarrowFloatFormat::F16, WidenedFloatBinaryOp::Multiply)
            }
            "__fe2o3_ir_float_v1_f16_div_widened_rne" => {
                binary(NarrowFloatFormat::F16, WidenedFloatBinaryOp::Divide)
            }
            "__fe2o3_ir_float_v1_bf16_add_widened_rne" => {
                binary(NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Add)
            }
            "__fe2o3_ir_float_v1_bf16_sub_widened_rne" => {
                binary(NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Subtract)
            }
            "__fe2o3_ir_float_v1_bf16_mul_widened_rne" => {
                binary(NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Multiply)
            }
            "__fe2o3_ir_float_v1_bf16_div_widened_rne" => {
                binary(NarrowFloatFormat::Bf16, WidenedFloatBinaryOp::Divide)
            }
            "__fe2o3_ir_float_v1_sqrt_f32" => math(F32MathFunction::Sqrt),
            "__fe2o3_ir_float_v1_fma_f32" => math(F32MathFunction::FusedMultiplyAdd),
            "__fe2o3_ir_float_v1_floor_f32" => math(F32MathFunction::Floor),
            "__fe2o3_ir_float_v1_ceil_f32" => math(F32MathFunction::Ceil),
            "__fe2o3_ir_float_v1_trunc_f32" => math(F32MathFunction::Truncate),
            "__fe2o3_ir_float_v1_roundeven_f32" => math(F32MathFunction::RoundTiesEven),
            "__fe2o3_ir_float_v1_sin_f32" => math(F32MathFunction::Sin),
            "__fe2o3_ir_float_v1_cos_f32" => math(F32MathFunction::Cos),
            "__fe2o3_ir_float_v1_exp_f32" => math(F32MathFunction::Exp),
            "__fe2o3_ir_float_v1_exp2_f32" => math(F32MathFunction::Exp2),
            "__fe2o3_ir_float_v1_log_f32" => math(F32MathFunction::Ln),
            "__fe2o3_ir_float_v1_log2_f32" => math(F32MathFunction::Log2),
            "__fe2o3_ir_float_v1_log10_f32" => math(F32MathFunction::Log10),
            "__fe2o3_ir_float_v1_fma_bf16x2" => Self::Bf16x2FusedMultiplyAdd {
                value: values[0],
                multiplier: values[1],
                addend: values[2],
            },
            _ => return None,
        })
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
    VolatileRead(AddressSpace),
    VolatileWrite(AddressSpace),
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
            || self
                .effects
                .contains(&MemoryEffect::VolatileRead(address_space))
    }

    pub fn writes(&self, address_space: AddressSpace) -> bool {
        self.effects.contains(&MemoryEffect::Write(address_space))
            || self
                .effects
                .contains(&MemoryEffect::VolatileWrite(address_space))
    }

    pub fn volatile_reads(&self, address_space: AddressSpace) -> bool {
        self.effects
            .contains(&MemoryEffect::VolatileRead(address_space))
    }

    pub fn volatile_writes(&self, address_space: AddressSpace) -> bool {
        self.effects
            .contains(&MemoryEffect::VolatileWrite(address_space))
    }

    pub fn effects(&self) -> &BTreeSet<MemoryEffect> {
        &self.effects
    }
}

pub(crate) fn pointer_for(pointee: Type, address_space: AddressSpace, access: AccessMode) -> Type {
    Type::pointer(pointee, address_space, access)
}
