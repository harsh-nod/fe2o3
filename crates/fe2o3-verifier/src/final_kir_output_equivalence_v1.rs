//! Direct functional comparison of source-derived and final canonical KIR.
//!
//! This module does not trust an optimizer replay. It independently extracts
//! each kernel's observable global stores from both verified KIR modules and
//! emits one Verus lemma per kernel. Each lemma proves store-address, guard,
//! and stored-value equality for arbitrary scalar inputs and arbitrary input
//! memories. Unsupported control flow, effects, aliases, and numerical models
//! fail closed.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
    fmt::Write as _,
};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CastKind, ComparePredicate, Constant,
    ExecutionCapabilityOperationV1, ExecutionCapabilityRequirementV1, ExecutionCollectiveKindV1,
    F32MathFunction, FloatOperation, Function, GlobalCapabilityRoleV1, IntrinsicKind, Kernel,
    MemoryElementType, MemoryIntrinsicOperation, Module, NumericalModeV1, Operation, OperationKind,
    PointerDistanceKind, PointerDistanceUnit, ScalarType, TargetCapability, Terminator, Type,
    UnaryOp, ValueId, WaveF32ReductionKindV1, WaveOperationKind,
};

use crate::final_kir_advanced_semantics_v1::{
    STRICT_FLOAT_BIT_SEMANTICS_V1, address_space_mask_v1, address_space_tag_v1, float_layout_v1,
    memory_ordering_tag_v1, render_wave_reduce_max_f32_v1,
};
use crate::functional_refinement_receipt_v2::ranked_effect_formula_replay_prelude_v2;
use crate::{CanonicalGeneratedVerusProofInputV3, GeneratedVerusProofInputErrorV3};

const MAX_FINAL_KIR_SYMBOLIC_NODES_V1: usize = 8_192;
const MAX_FINAL_KIR_SYMBOLIC_DEPTH_V1: usize = 256;
const MAX_FINAL_KIR_EXECUTION_STATES_V1: usize = 512;
const MAX_FINAL_KIR_BLOCK_VISITS_V1: usize = 64;
const MAX_FINAL_KIR_MEMORY_WRITES_V1: usize = 1_024;
const MAX_FINAL_KIR_SEMANTIC_EVENTS_V1: usize = 2_048;
const MAX_FINAL_KIR_WORKGROUP_INVOCATIONS_V1: u32 = 256;
const PRIVATE_MEMORY_ROOT_BASE_V1: u32 = 1 << 30;
const WORKGROUP_MEMORY_ROOT_BASE_V1: u32 = 1 << 31;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarV1 {
    Bool,
    Signed(u16),
    Unsigned(u16),
    Float(ScalarType),
}

impl ScalarV1 {
    fn from_ir(scalar: ScalarType) -> Result<Self, FinalKirOutputEquivalenceErrorV1> {
        Ok(match scalar {
            ScalarType::Bool => Self::Bool,
            ScalarType::I8 => Self::Signed(8),
            ScalarType::I16 => Self::Signed(16),
            ScalarType::I32 => Self::Signed(32),
            ScalarType::I64 => Self::Signed(64),
            ScalarType::U8 => Self::Unsigned(8),
            ScalarType::U16 => Self::Unsigned(16),
            ScalarType::U32 => Self::Unsigned(32),
            ScalarType::U64 | ScalarType::Index => Self::Unsigned(64),
            ScalarType::F16 | ScalarType::Bf16 | ScalarType::F32 | ScalarType::F64 => {
                Self::Float(scalar)
            }
            ScalarType::I128 | ScalarType::U128 => {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType);
            }
        })
    }

    const fn width(self) -> u16 {
        match self {
            Self::Bool => 1,
            Self::Signed(width) | Self::Unsigned(width) => width,
            Self::Float(scalar) => scalar
                .bit_width()
                .expect("floating KIR scalars have a fixed bit width"),
        }
    }

    const fn is_signed(self) -> bool {
        matches!(self, Self::Signed(_))
    }

    const fn is_float(self) -> bool {
        matches!(self, Self::Float(_))
    }
}

#[derive(Clone, Debug)]
struct ExpressionV1 {
    scalar: ScalarV1,
    kind: ExpressionKindV1,
    depth: usize,
}

#[derive(Clone, Debug)]
enum ExpressionKindV1 {
    Parameter(u32),
    Intrinsic(IntrinsicKind),
    SliceLength(u32),
    Constant(i128),
    Unary {
        operation: UnaryOp,
        operand: Box<ExpressionV1>,
    },
    Binary {
        operation: BinaryOp,
        lhs: Box<ExpressionV1>,
        rhs: Box<ExpressionV1>,
    },
    Compare {
        predicate: ComparePredicate,
        lhs: Box<ExpressionV1>,
        rhs: Box<ExpressionV1>,
    },
    Cast {
        kind: CastKind,
        operand: Box<ExpressionV1>,
    },
    Select {
        condition: Box<ExpressionV1>,
        when_true: Box<ExpressionV1>,
        when_false: Box<ExpressionV1>,
    },
    MemoryRead {
        memory_parameter: u32,
        initial_parameter: Option<u32>,
        cross_lane_width: Option<u32>,
        bounded_elements: Option<u64>,
        writes: Vec<StoreEffectV1>,
        index: Box<ExpressionV1>,
    },
    WaveLaneId {
        width: u32,
    },
    WaveBallot {
        predicate: Box<ExpressionV1>,
        width: u32,
    },
    WaveAny {
        predicate: Box<ExpressionV1>,
        width: u32,
    },
    WaveAll {
        predicate: Box<ExpressionV1>,
        width: u32,
    },
    WaveShuffleIndex {
        value: Box<ExpressionV1>,
        source_lane: Box<ExpressionV1>,
        tile_width: u32,
    },
    WaveBroadcast {
        value: Box<ExpressionV1>,
        source_lane: Box<ExpressionV1>,
        tile_width: u32,
    },
    WaveReduceMaxF32 {
        value: Box<ExpressionV1>,
        tile_width: u32,
    },
    SubgroupSum {
        value: Box<ExpressionV1>,
        width: u32,
        kind: SubgroupSumKindV1,
    },
    FloatAbs {
        operand: Box<ExpressionV1>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgroupSumKindV1 {
    Reduce,
    InclusiveScan,
    ExclusiveScan,
}

impl ExpressionV1 {
    fn leaf(scalar: ScalarV1, kind: ExpressionKindV1) -> Self {
        Self {
            scalar,
            kind,
            depth: 1,
        }
    }

    fn unary(
        scalar: ScalarV1,
        operation: UnaryOp,
        operand: Self,
    ) -> Result<Self, FinalKirOutputEquivalenceErrorV1> {
        let depth = bounded_depth([operand.depth])?;
        Ok(Self {
            scalar,
            kind: ExpressionKindV1::Unary {
                operation,
                operand: Box::new(operand),
            },
            depth,
        })
    }

    fn binary(
        scalar: ScalarV1,
        operation: BinaryOp,
        lhs: Self,
        rhs: Self,
    ) -> Result<Self, FinalKirOutputEquivalenceErrorV1> {
        if matches!(
            operation,
            BinaryOp::Checked(_) | BinaryOp::Divide | BinaryOp::Remainder
        ) {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
        }
        let zero = |value: &Self| evaluate_constant(value) == Some(0);
        let one = |value: &Self| evaluate_constant(value) == Some(1);
        match operation {
            BinaryOp::Add | BinaryOp::BitOr | BinaryOp::BitXor if zero(&rhs) => return Ok(lhs),
            BinaryOp::Add | BinaryOp::BitOr | BinaryOp::BitXor if zero(&lhs) => return Ok(rhs),
            BinaryOp::Subtract | BinaryOp::ShiftLeft | BinaryOp::ShiftRight if zero(&rhs) => {
                return Ok(lhs);
            }
            BinaryOp::Multiply if one(&rhs) => return Ok(lhs),
            BinaryOp::Multiply if one(&lhs) => return Ok(rhs),
            _ => {}
        }
        let depth = bounded_depth([lhs.depth, rhs.depth])?;
        Ok(Self {
            scalar,
            kind: ExpressionKindV1::Binary {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            depth,
        })
    }
}

fn bounded_depth<const N: usize>(
    children: [usize; N],
) -> Result<usize, FinalKirOutputEquivalenceErrorV1> {
    let depth = children
        .into_iter()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if depth > MAX_FINAL_KIR_SYMBOLIC_DEPTH_V1 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    Ok(depth)
}

#[derive(Clone, Debug)]
struct PointerV1 {
    memory_parameter: u32,
    element: ScalarV1,
    address_space: AddressSpace,
    access: AccessMode,
    index: ExpressionV1,
}

#[derive(Clone, Debug)]
struct SliceV1 {
    memory_parameter: u32,
    element: ScalarV1,
    address_space: AddressSpace,
    access: AccessMode,
}

#[derive(Clone, Debug)]
struct CapabilityV1 {
    physical: SliceV1,
    role: GlobalCapabilityRoleV1,
}

#[derive(Clone, Debug)]
struct WorkgroupViewV1 {
    memory_root: u32,
    elements: u64,
    layout: fe2o3_kernel_ir::ExecutionElementLayoutV1,
    initialized: bool,
    published: bool,
}

#[derive(Clone, Debug)]
struct BoundedMemoryViewV1 {
    pointer: PointerV1,
    extent: ExpressionV1,
    extent_contract: fe2o3_kernel_ir::ExecutionMemoryExtentV1,
    layout: fe2o3_kernel_ir::ExecutionElementLayoutV1,
    access: fe2o3_kernel_ir::ExecutionMemoryAccessV1,
}

#[derive(Clone, Debug)]
enum SymbolicValueV1 {
    Scalar(ExpressionV1),
    Pointer(PointerV1),
    Slice(SliceV1),
    Capability(CapabilityV1),
    WorkgroupView(WorkgroupViewV1),
    WorkgroupIndex(ExpressionV1),
    MemoryView(BoundedMemoryViewV1),
    Opaque,
}

#[derive(Clone, Debug)]
struct StoreEffectV1 {
    index: ExpressionV1,
    guard: ExpressionV1,
    value: ExpressionV1,
}

#[derive(Clone, Debug)]
struct MemoryVersionV1 {
    element: Option<ScalarV1>,
    initial_parameter: Option<u32>,
    cross_lane_width: Option<u32>,
    bounded_elements: Option<u64>,
    writes: Vec<StoreEffectV1>,
}

#[derive(Clone, Debug)]
enum EventFieldV1 {
    Constant(i128),
    Expression(ExpressionV1),
}

#[derive(Clone, Debug)]
struct SemanticEventV1 {
    fields: Vec<EventFieldV1>,
}

#[derive(Clone, Debug)]
struct ReturnOutcomeV1 {
    path: ExpressionV1,
    memories: BTreeMap<u32, MemoryVersionV1>,
    written_roots: BTreeSet<u32>,
    events: Vec<SemanticEventV1>,
}

#[derive(Clone, Debug)]
struct ExecutionStateV1 {
    block: BlockId,
    values: BTreeMap<ValueId, SymbolicValueV1>,
    memories: BTreeMap<u32, MemoryVersionV1>,
    written_roots: BTreeSet<u32>,
    events: Vec<SemanticEventV1>,
    path: ExpressionV1,
    visits: BTreeMap<BlockId, usize>,
    next_private_allocation: u32,
    next_workgroup_allocation: u32,
}

struct KernelEffectsV1 {
    parameter_types: Vec<Type>,
    output_roots: BTreeSet<u32>,
    outcomes: Vec<ReturnOutcomeV1>,
    intrinsics: BTreeSet<IntrinsicKind>,
    wave_widths: BTreeSet<u32>,
}

/// Mathematical strength of the generated final-KIR theorem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalKirNumericalModelV1 {
    /// Exact finite bit-vector semantics, including only floating operations
    /// whose raw-bit behavior is modeled directly by this verifier.
    ExactBitVector,
    /// Equal raw output bits follow only from an identical sequence of
    /// deterministic StrictIeee-tagged operators. This is not an IEEE model,
    /// a real-arithmetic theorem, a reassociation theorem, or an error bound.
    /// Retained for receipt compatibility; the direct generator no longer
    /// emits this weaker model and rejects arithmetic without a concrete one.
    StrictFloatOperatorCongruence,
}

/// Exact generated theorem and its complete observable-output cardinality.
#[derive(Debug)]
pub(crate) struct FinalKirOutputEquivalenceProofV1 {
    source: CanonicalGeneratedVerusProofInputV3,
    output_writes: u64,
    numerical_model: FinalKirNumericalModelV1,
}

impl FinalKirOutputEquivalenceProofV1 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        CanonicalGeneratedVerusProofInputV3,
        u64,
        FinalKirNumericalModelV1,
    ) {
        (self.source, self.output_writes, self.numerical_model)
    }
}

/// Fail-closed reason that direct source/final KIR comparison was unavailable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinalKirOutputEquivalenceErrorV1 {
    KernelRosterMismatch,
    KernelContractMismatch,
    UnsupportedControlFlow,
    UnsupportedScalarType,
    UnsupportedOperation,
    MalformedValueFlow,
    MissingOutputWrite,
    DuplicateOutputWrite,
    UnsupportedAliasing,
    UnsupportedNumericalPolicy,
    LoopBoundExceeded,
    UnsupportedCallOrTranscendental,
    UnsupportedCollectiveSemantics,
    UnsupportedTensorSemantics,
    UnsupportedSynchronizationSemantics,
    UnsupportedAtomicSemantics,
    OutputRosterMismatch,
    ResourceLimit,
    GeneratedSource(String),
}

impl fmt::Display for FinalKirOutputEquivalenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "direct final-KIR functional proof failed: {self:?}"
        )
    }
}

impl Error for FinalKirOutputEquivalenceErrorV1 {}

impl From<GeneratedVerusProofInputErrorV3> for FinalKirOutputEquivalenceErrorV1 {
    fn from(error: GeneratedVerusProofInputErrorV3) -> Self {
        Self::GeneratedSource(error.to_string())
    }
}

/// Generates a theorem whose actual expressions are read from `final_module`.
/// The source module supplies the authenticated pre-optimization boundary; no
/// identity equality or transformation replay is accepted as a premise.
pub(crate) fn generate_final_kir_output_equivalence_v1(
    source_module: &Module,
    final_module: &Module,
) -> Result<FinalKirOutputEquivalenceProofV1, FinalKirOutputEquivalenceErrorV1> {
    if complete_capability_closure(source_module) != complete_capability_closure(final_module) {
        return Err(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch);
    }
    let numerical_policies = strict_numerical_policies(source_module)?;
    let source_kernels = kernel_roster(source_module)?;
    let final_kernels = kernel_roster(final_module)?;
    if source_kernels.len() != final_kernels.len() || source_kernels.keys().ne(final_kernels.keys())
    {
        return Err(FinalKirOutputEquivalenceErrorV1::KernelRosterMismatch);
    }

    let mut source = String::new();
    write!(
        source,
        "use vstd::prelude::*;\n\nverus! {{\n{}\n{}\n",
        ranked_effect_formula_replay_prelude_v2(),
        STRICT_FLOAT_BIT_SEMANTICS_V1,
    )
    .map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    let mut output_writes = 0_u64;
    for (kernel_index, (kernel_id, source_kernel)) in source_kernels.iter().enumerate() {
        let final_kernel = final_kernels
            .get(kernel_id)
            .ok_or(FinalKirOutputEquivalenceErrorV1::KernelRosterMismatch)?;
        require_matching_kernel_contract(source_kernel, final_kernel)?;
        let source_function = source_module
            .function(&source_kernel.entry)
            .ok_or(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch)?;
        let final_function = final_module
            .function(&final_kernel.entry)
            .ok_or(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch)?;
        if source_function.signature != final_function.signature
            || source_function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
            || final_function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
            || source_function.effective_capabilities() != final_function.effective_capabilities()
        {
            return Err(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch);
        }
        let workgroup_width = source_kernel
            .workgroup_size
            .filter(|size| size.y == 1 && size.z == 1)
            .map(|size| size.x);
        let source_effects =
            extract_kernel_effects(source_function, &numerical_policies, workgroup_width)?;
        let final_effects =
            extract_kernel_effects(final_function, &numerical_policies, workgroup_width)?;
        if source_effects.output_roots != final_effects.output_roots {
            return Err(FinalKirOutputEquivalenceErrorV1::OutputRosterMismatch);
        }
        output_writes = output_writes
            .checked_add(source_effects.output_roots.len() as u64)
            .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
        render_kernel_lemma(&mut source, kernel_index, &source_effects, &final_effects)?;
    }
    if output_writes == 0 {
        return Err(FinalKirOutputEquivalenceErrorV1::MissingOutputWrite);
    }
    source.push_str("}\n\nfn main() {}\n");
    Ok(FinalKirOutputEquivalenceProofV1 {
        source: CanonicalGeneratedVerusProofInputV3::new(source.into_bytes())?,
        output_writes,
        numerical_model: FinalKirNumericalModelV1::ExactBitVector,
    })
}

fn strict_numerical_policies(
    module: &Module,
) -> Result<BTreeMap<ScalarType, NumericalModeV1>, FinalKirOutputEquivalenceErrorV1> {
    let mut policies = BTreeMap::new();
    for capability in complete_capability_closure(module) {
        let TargetCapability::Execution(ExecutionCapabilityRequirementV1::Numerical {
            value_type,
            mode,
        }) = capability
        else {
            continue;
        };
        if policies
            .insert(value_type, mode)
            .is_some_and(|prior| prior != mode)
        {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
        }
    }
    Ok(policies)
}

fn complete_capability_closure(module: &Module) -> BTreeSet<TargetCapability> {
    module
        .effective_capabilities()
        .into_iter()
        .chain(
            module
                .functions
                .iter()
                .flat_map(Function::effective_capabilities),
        )
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| kernel.required_capabilities.iter().cloned()),
        )
        .collect()
}

fn kernel_roster(
    module: &Module,
) -> Result<BTreeMap<&str, &Kernel>, FinalKirOutputEquivalenceErrorV1> {
    let mut kernels = BTreeMap::new();
    for kernel in &module.kernels {
        if kernels.insert(kernel.id.as_str(), kernel).is_some() {
            return Err(FinalKirOutputEquivalenceErrorV1::KernelRosterMismatch);
        }
    }
    if kernels.is_empty() {
        return Err(FinalKirOutputEquivalenceErrorV1::KernelRosterMismatch);
    }
    Ok(kernels)
}

fn require_matching_kernel_contract(
    source: &Kernel,
    final_kernel: &Kernel,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if source.id != final_kernel.id
        || source.domain != final_kernel.domain
        || source.workgroup_size != final_kernel.workgroup_size
        || source.required_capabilities != final_kernel.required_capabilities
    {
        return Err(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch);
    }
    Ok(())
}

fn extract_kernel_effects(
    function: &Function,
    numerical_policies: &BTreeMap<ScalarType, NumericalModeV1>,
    workgroup_width: Option<u32>,
) -> Result<KernelEffectsV1, FinalKirOutputEquivalenceErrorV1> {
    if !function.signature.results.is_empty() {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
    }
    let body = function
        .body
        .as_ref()
        .ok_or(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch)?;
    if body.parameters.len() != function.signature.parameters.len() {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let Some(entry) = body.blocks.first() else {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
    };
    if !entry.parameters.is_empty() {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
    }
    let execution_element_scalars = infer_execution_element_scalars(function)?;
    let mut blocks = BTreeMap::new();
    let mut definitions = body.parameters.iter().copied().collect::<BTreeSet<_>>();
    for block in &body.blocks {
        if blocks.insert(block.id, block).is_some() || block.terminator.is_none() {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
        }
        for definition in block.parameters.iter().chain(
            block
                .operations
                .iter()
                .flat_map(|operation| &operation.results),
        ) {
            if !definitions.insert(definition.id) {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
        }
    }

    let mut values = BTreeMap::new();
    let mut memories = BTreeMap::new();
    for (ordinal, (identity, ty)) in body
        .parameters
        .iter()
        .zip(&function.signature.parameters)
        .enumerate()
    {
        let ordinal =
            u32::try_from(ordinal).map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
        let value = parameter_value(ordinal, ty)?;
        if values.insert(*identity, value).is_some() {
            return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
        }
        if let Some(element) = memory_element(ty)? {
            memories.insert(
                ordinal,
                MemoryVersionV1 {
                    element: Some(element),
                    initial_parameter: Some(ordinal),
                    cross_lane_width: None,
                    bounded_elements: None,
                    writes: Vec::new(),
                },
            );
        }
    }

    let mut visits = BTreeMap::new();
    visits.insert(entry.id, 1);
    let mut pending = VecDeque::from([ExecutionStateV1 {
        block: entry.id,
        values,
        memories,
        written_roots: BTreeSet::new(),
        events: Vec::new(),
        path: bool_true(),
        visits,
        next_private_allocation: 0,
        next_workgroup_allocation: 0,
    }]);
    let mut outcomes = Vec::new();
    let mut intrinsics = BTreeSet::new();
    let mut wave_widths = BTreeSet::new();
    let mut capability_roots = BTreeMap::new();
    let mut nodes = 0_usize;
    let mut scheduled_states = 1_usize;
    while let Some(mut state) = pending.pop_front() {
        let block = blocks
            .get(&state.block)
            .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow)?;
        for operation in &block.operations {
            nodes = nodes
                .checked_add(1)
                .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
            if nodes > MAX_FINAL_KIR_SYMBOLIC_NODES_V1 {
                return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
            }
            execute_operation(
                operation,
                &mut state,
                &mut intrinsics,
                &mut wave_widths,
                &mut capability_roots,
                numerical_policies,
                workgroup_width,
                &execution_element_scalars,
            )?;
        }
        match block
            .terminator
            .as_ref()
            .expect("terminators checked above")
        {
            Terminator::Branch { target, arguments } => enqueue_successor(
                state,
                *target,
                arguments,
                &blocks,
                &mut pending,
                &mut scheduled_states,
            )?,
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                let condition = scalar_value(&state.values, *condition)?;
                if condition.scalar != ScalarV1::Bool {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                match constant_bool(&condition) {
                    Some(true) => enqueue_successor(
                        state,
                        *then_target,
                        then_arguments,
                        &blocks,
                        &mut pending,
                        &mut scheduled_states,
                    )?,
                    Some(false) => enqueue_successor(
                        state,
                        *else_target,
                        else_arguments,
                        &blocks,
                        &mut pending,
                        &mut scheduled_states,
                    )?,
                    None => {
                        let mut then_state = state.clone();
                        then_state.path = bool_and(then_state.path, condition.clone())?;
                        state.path = bool_and(state.path, bool_not(condition)?)?;
                        enqueue_successor(
                            then_state,
                            *then_target,
                            then_arguments,
                            &blocks,
                            &mut pending,
                            &mut scheduled_states,
                        )?;
                        enqueue_successor(
                            state,
                            *else_target,
                            else_arguments,
                            &blocks,
                            &mut pending,
                            &mut scheduled_states,
                        )?;
                    }
                }
            }
            Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                let selector = scalar_value(&state.values, *selector)?;
                let arms = cases
                    .iter()
                    .map(|case| {
                        Ok((
                            constant_for_scalar(selector.scalar, i128::from(case.value))?,
                            case.target,
                            case.arguments.as_slice(),
                        ))
                    })
                    .collect::<Result<Vec<_>, FinalKirOutputEquivalenceErrorV1>>()?;
                enqueue_switch(
                    state,
                    selector,
                    &arms,
                    *default_target,
                    default_arguments,
                    &blocks,
                    &mut pending,
                    &mut scheduled_states,
                )?;
            }
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                let selector = scalar_value(&state.values, *selector)?;
                let mut arms = Vec::with_capacity(cases.len());
                for case in cases {
                    let value = constant_expression(&case.value)?;
                    if value.scalar != selector.scalar {
                        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                    }
                    arms.push((value, case.target, case.arguments.as_slice()));
                }
                enqueue_switch(
                    state,
                    selector,
                    &arms,
                    *default_target,
                    default_arguments,
                    &blocks,
                    &mut pending,
                    &mut scheduled_states,
                )?;
            }
            Terminator::Return { values } if values.is_empty() => {
                outcomes.push(ReturnOutcomeV1 {
                    path: state.path,
                    memories: state.memories,
                    written_roots: state.written_roots,
                    events: state.events,
                });
            }
            Terminator::Return { .. } | Terminator::Unreachable => {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
            }
        }
    }
    let Some(first) = outcomes.first() else {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
    };
    let output_roots = first.written_roots.clone();
    if output_roots.is_empty()
        || outcomes
            .iter()
            .any(|item| item.written_roots != output_roots)
    {
        return Err(FinalKirOutputEquivalenceErrorV1::MissingOutputWrite);
    }
    require_alias_discipline(&outcomes, &capability_roots)?;
    Ok(KernelEffectsV1 {
        parameter_types: function.signature.parameters.clone(),
        output_roots,
        outcomes,
        intrinsics,
        wave_widths,
    })
}

fn execute_operation(
    operation: &Operation,
    state: &mut ExecutionStateV1,
    intrinsics: &mut BTreeSet<IntrinsicKind>,
    wave_widths: &mut BTreeSet<u32>,
    capability_roots: &mut BTreeMap<u32, u8>,
    numerical_policies: &BTreeMap<ScalarType, NumericalModeV1>,
    workgroup_width: Option<u32>,
    execution_element_scalars: &BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarV1>,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let defined = match &operation.kind {
        OperationKind::Constant(constant) => {
            Some(SymbolicValueV1::Scalar(constant_expression(constant)?))
        }
        OperationKind::Intrinsic(intrinsic) => {
            let scalar = scalar_result(operation)?;
            if intrinsic.result_type != operation.results[0].ty {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            intrinsics.insert(intrinsic.kind);
            Some(SymbolicValueV1::Scalar(ExpressionV1::leaf(
                scalar,
                ExpressionKindV1::Intrinsic(intrinsic.kind),
            )))
        }
        OperationKind::Unary { op, operand } => {
            let operand = scalar_value(&state.values, *operand)?;
            let scalar = scalar_result(operation)?;
            if operand.scalar != scalar || (scalar.is_float() && *op == UnaryOp::Not) {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
            }
            require_strict_float_bit_policy(scalar, numerical_policies)?;
            Some(SymbolicValueV1::Scalar(ExpressionV1::unary(
                scalar, *op, operand,
            )?))
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let lhs = scalar_value(&state.values, *lhs)?;
            let rhs = scalar_value(&state.values, *rhs)?;
            let scalar = scalar_result(operation)?;
            if lhs.scalar != scalar || rhs.scalar != scalar {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            if scalar.is_float() {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
            }
            Some(SymbolicValueV1::Scalar(ExpressionV1::binary(
                scalar, *op, lhs, rhs,
            )?))
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            let lhs = scalar_value(&state.values, *lhs)?;
            let rhs = scalar_value(&state.values, *rhs)?;
            if lhs.scalar != rhs.scalar || scalar_result(operation)? != ScalarV1::Bool {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            require_strict_float_bit_policy(lhs.scalar, numerical_policies)?;
            let depth = bounded_depth([lhs.depth, rhs.depth])?;
            Some(SymbolicValueV1::Scalar(ExpressionV1 {
                scalar: ScalarV1::Bool,
                kind: ExpressionKindV1::Compare {
                    predicate: *predicate,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                depth,
            }))
        }
        OperationKind::Cast { kind, value, to } => {
            let value = state
                .values
                .get(value)
                .cloned()
                .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?;
            match value {
                SymbolicValueV1::Scalar(operand) => {
                    let scalar = scalar_result(operation)?;
                    if to.as_scalar().and_then(|ty| ScalarV1::from_ir(ty).ok()) != Some(scalar) {
                        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                    }
                    if (operand.scalar.is_float() || scalar.is_float())
                        && (*kind != CastKind::Bitcast || operand.scalar.width() != scalar.width())
                    {
                        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
                    }
                    let depth = bounded_depth([operand.depth])?;
                    Some(SymbolicValueV1::Scalar(ExpressionV1 {
                        scalar,
                        kind: ExpressionKindV1::Cast {
                            kind: *kind,
                            operand: Box::new(operand),
                        },
                        depth,
                    }))
                }
                SymbolicValueV1::Pointer(pointer) if *kind == CastKind::RestrictPointerAccess => {
                    Some(SymbolicValueV1::Pointer(pointer))
                }
                _ => return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation),
            }
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            let condition = scalar_value(&state.values, *condition)?;
            let when_true = scalar_value(&state.values, *true_value)?;
            let when_false = scalar_value(&state.values, *false_value)?;
            let scalar = scalar_result(operation)?;
            if condition.scalar != ScalarV1::Bool
                || when_true.scalar != scalar
                || when_false.scalar != scalar
            {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            let depth = bounded_depth([condition.depth, when_true.depth, when_false.depth])?;
            Some(SymbolicValueV1::Scalar(ExpressionV1 {
                scalar,
                kind: ExpressionKindV1::Select {
                    condition: Box::new(condition),
                    when_true: Box::new(when_true),
                    when_false: Box::new(when_false),
                },
                depth,
            }))
        }
        OperationKind::SliceLength { slice } => {
            let slice = slice_value(&state.values, *slice)?;
            if scalar_result(operation)? != ScalarV1::Unsigned(64) {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            Some(SymbolicValueV1::Scalar(ExpressionV1::leaf(
                ScalarV1::Unsigned(64),
                ExpressionKindV1::SliceLength(slice.memory_parameter),
            )))
        }
        OperationKind::SliceData { slice } => {
            let slice = slice_value(&state.values, *slice)?;
            Some(SymbolicValueV1::Pointer(PointerV1 {
                memory_parameter: slice.memory_parameter,
                element: slice.element,
                address_space: slice.address_space,
                access: slice.access,
                index: index_zero(),
            }))
        }
        OperationKind::GetElementPointer { base, offset } => {
            let base = pointer_value(&state.values, *base)?;
            let offset = scalar_value(&state.values, *offset)?;
            if offset.scalar != ScalarV1::Unsigned(64) {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            let index =
                ExpressionV1::binary(ScalarV1::Unsigned(64), BinaryOp::Add, base.index, offset)?;
            Some(SymbolicValueV1::Pointer(PointerV1 { index, ..base }))
        }
        OperationKind::Load { pointer, access } => {
            let pointer = pointer_value(&state.values, *pointer)?;
            require_load_access(&pointer, access.address_space)?;
            let scalar = scalar_result(operation)?;
            if scalar != pointer.element {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            Some(SymbolicValueV1::Scalar(memory_read(state, pointer)?))
        }
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            access,
        } => {
            let pointer = pointer_value(&state.values, *pointer)?;
            require_load_access(&pointer, access.address_space)?;
            let condition = scalar_value(&state.values, *predicate)?;
            let fallback = scalar_value(&state.values, *fallback)?;
            let scalar = scalar_result(operation)?;
            if condition.scalar != ScalarV1::Bool
                || fallback.scalar != scalar
                || pointer.element != scalar
            {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            let load = memory_read(state, pointer)?;
            let depth = bounded_depth([condition.depth, load.depth, fallback.depth])?;
            Some(SymbolicValueV1::Scalar(ExpressionV1 {
                scalar,
                kind: ExpressionKindV1::Select {
                    condition: Box::new(condition),
                    when_true: Box::new(load),
                    when_false: Box::new(fallback),
                },
                depth,
            }))
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } => {
            let pointer = pointer_value(&state.values, *pointer)?;
            let value = scalar_value(&state.values, *value)?;
            record_store(state, pointer, bool_true(), value, access.address_space)?;
            None
        }
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value,
            access,
        } => {
            let pointer = pointer_value(&state.values, *pointer)?;
            let guard = scalar_value(&state.values, *predicate)?;
            let value = scalar_value(&state.values, *value)?;
            if guard.scalar != ScalarV1::Bool {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            record_store(state, pointer, guard, value, access.address_space)?;
            None
        }
        OperationKind::KernelContextIssue(_) => Some(SymbolicValueV1::Opaque),
        OperationKind::GlobalCapabilityBind(bind) => {
            let physical = slice_value(&state.values, bind.physical)?;
            let [result] = operation.results.as_slice() else {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            };
            let Type::GlobalCapability(capability) = &result.ty else {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            };
            if capability
                .element()
                .as_scalar()
                .and_then(|item| ScalarV1::from_ir(item).ok())
                != Some(physical.element)
                || capability.role().access() != physical.access
            {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            let alias_tag = legacy_alias_contract_tag(capability.role());
            match capability_roots.insert(physical.memory_parameter, alias_tag) {
                Some(previous) if previous != alias_tag => {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
                }
                _ => {}
            }
            Some(SymbolicValueV1::Capability(CapabilityV1 {
                physical,
                role: capability.role(),
            }))
        }
        OperationKind::GlobalCapabilityIndex(index) => {
            let capability = capability_value(&state.values, index.capability)?;
            let projected = scalar_value(&state.values, index.index)?;
            if projected.scalar != ScalarV1::Unsigned(64)
                || index.index_space
                    != match capability.role {
                        GlobalCapabilityRoleV1::DisjointWrite(contract) => Some(contract),
                        GlobalCapabilityRoleV1::ReadOnly
                        | GlobalCapabilityRoleV1::ExclusiveReadWrite => None,
                    }
            {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            Some(SymbolicValueV1::Scalar(projected))
        }
        OperationKind::Call { callee, arguments } => {
            let Some(float) = FloatOperation::from_intrinsic_call(callee, arguments) else {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCallOrTranscendental);
            };
            match float {
                FloatOperation::F32Math {
                    function: F32MathFunction::Abs,
                    arguments,
                    ..
                } => {
                    let [operand] = arguments.as_slice() else {
                        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                    };
                    let operand = scalar_value(&state.values, *operand)?;
                    let scalar = scalar_result(operation)?;
                    if operand.scalar != ScalarV1::Float(ScalarType::F32)
                        || scalar != operand.scalar
                    {
                        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                    }
                    require_strict_float_bit_policy(scalar, numerical_policies)?;
                    let depth = bounded_depth([operand.depth])?;
                    Some(SymbolicValueV1::Scalar(ExpressionV1 {
                        scalar,
                        kind: ExpressionKindV1::FloatAbs {
                            operand: Box::new(operand),
                        },
                        depth,
                    }))
                }
                FloatOperation::Convert { .. }
                | FloatOperation::WidenedBinary { .. }
                | FloatOperation::F32Math { .. }
                | FloatOperation::Bf16x2FusedMultiplyAdd { .. } => {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
                }
            }
        }
        OperationKind::MemoryIntrinsic(intrinsic) => match intrinsic {
            MemoryIntrinsicOperation::PointerDistance {
                pointer,
                origin,
                kind,
                unit,
                element,
                address_space,
                layout,
                ..
            } => {
                let pointer = pointer_value(&state.values, *pointer)?;
                let origin = pointer_value(&state.values, *origin)?;
                let scalar = scalar_result(operation)?;
                let expected = match kind {
                    PointerDistanceKind::Signed => ScalarV1::Signed(64),
                    PointerDistanceKind::Unsigned => ScalarV1::Unsigned(64),
                };
                if scalar != expected
                    || pointer.memory_parameter != origin.memory_parameter
                    || pointer.element != origin.element
                    || pointer.address_space != *address_space
                    || origin.address_space != *address_space
                    || memory_intrinsic_scalar(*element)? != pointer.element
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
                }
                let mut difference =
                    ExpressionV1::binary(scalar, BinaryOp::Subtract, pointer.index, origin.index)?;
                if *unit == PointerDistanceUnit::Bytes {
                    difference = ExpressionV1::binary(
                        scalar,
                        BinaryOp::Multiply,
                        difference,
                        ExpressionV1::leaf(
                            scalar,
                            ExpressionKindV1::Constant(i128::from(layout.size_bytes)),
                        ),
                    )?;
                }
                Some(SymbolicValueV1::Scalar(difference))
            }
            MemoryIntrinsicOperation::VolatileLoad {
                pointer,
                element,
                address_space,
                layout,
                ..
            } => {
                let pointer = pointer_value(&state.values, *pointer)?;
                let scalar = scalar_result(operation)?;
                if pointer.address_space != *address_space
                    || pointer.element != scalar
                    || memory_intrinsic_scalar(*element)? != scalar
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
                }
                let loaded = memory_read(state, pointer.clone())?;
                record_event(
                    state,
                    memory_event_fields(110, &pointer, *address_space, *layout, None),
                )?;
                Some(SymbolicValueV1::Scalar(loaded))
            }
            MemoryIntrinsicOperation::VolatileStore {
                pointer,
                value,
                element,
                address_space,
                layout,
                ..
            } => {
                let pointer = pointer_value(&state.values, *pointer)?;
                let value = scalar_value(&state.values, *value)?;
                if memory_intrinsic_scalar(*element)? != value.scalar {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
                }
                record_store(
                    state,
                    pointer.clone(),
                    bool_true(),
                    value.clone(),
                    *address_space,
                )?;
                record_event(
                    state,
                    memory_event_fields(111, &pointer, *address_space, *layout, Some(value)),
                )?;
                None
            }
            MemoryIntrinsicOperation::CopyNonOverlapping {
                source,
                destination,
                count,
                element,
                source_address_space,
                destination_address_space,
                layout,
                ..
            } => {
                let source = pointer_value(&state.values, *source)?;
                let destination = pointer_value(&state.values, *destination)?;
                let count_expression = scalar_value(&state.values, *count)?;
                let count = evaluate_constant(&count_expression)
                    .and_then(|value| usize::try_from(value).ok())
                    .filter(|value| *value <= 64)
                    .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
                if source.address_space != *source_address_space
                    || destination.address_space != *destination_address_space
                    || source.element != destination.element
                    || memory_intrinsic_scalar(*element)? != source.element
                    || source.memory_parameter == destination.memory_parameter
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
                }
                let mut copied = Vec::with_capacity(count);
                for offset in 0..count {
                    copied.push(memory_read(state, offset_pointer(source.clone(), offset)?)?);
                }
                for (offset, value) in copied.into_iter().enumerate() {
                    record_store(
                        state,
                        offset_pointer(destination.clone(), offset)?,
                        bool_true(),
                        value,
                        *destination_address_space,
                    )?;
                }
                let mut fields =
                    memory_event_fields(112, &source, *source_address_space, *layout, None);
                fields.extend(memory_event_fields(
                    113,
                    &destination,
                    *destination_address_space,
                    *layout,
                    Some(count_expression),
                ));
                record_event(state, fields)?;
                None
            }
        },
        OperationKind::Wave(wave) => {
            let width = wave.width.lanes();
            if wave.active_lanes != width
                || wave.convergence.scope() != fe2o3_kernel_ir::SynchronizationScope::Subgroup
            {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
            }
            wave_widths.insert(width);
            let scalar = scalar_result(operation)?;
            let expression = match wave.kind {
                WaveOperationKind::LaneId => {
                    ExpressionV1::leaf(scalar, ExpressionKindV1::WaveLaneId { width })
                }
                WaveOperationKind::Ballot { predicate } => {
                    let predicate = scalar_value(&state.values, predicate)?;
                    if predicate.scalar != ScalarV1::Bool
                        || scalar.width()
                            != u16::try_from(width).map_err(|_| {
                                FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics
                            })?
                    {
                        return Err(
                            FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics,
                        );
                    }
                    let depth = bounded_depth([predicate.depth])?;
                    ExpressionV1 {
                        scalar,
                        kind: ExpressionKindV1::WaveBallot {
                            predicate: Box::new(predicate),
                            width,
                        },
                        depth,
                    }
                }
                WaveOperationKind::Any { predicate } | WaveOperationKind::All { predicate } => {
                    let predicate = scalar_value(&state.values, predicate)?;
                    if predicate.scalar != ScalarV1::Bool || scalar != ScalarV1::Bool {
                        return Err(
                            FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics,
                        );
                    }
                    let depth = bounded_depth([predicate.depth])?;
                    let kind = if matches!(wave.kind, WaveOperationKind::Any { .. }) {
                        ExpressionKindV1::WaveAny {
                            predicate: Box::new(predicate),
                            width,
                        }
                    } else {
                        ExpressionKindV1::WaveAll {
                            predicate: Box::new(predicate),
                            width,
                        }
                    };
                    ExpressionV1 {
                        scalar,
                        kind,
                        depth,
                    }
                }
                WaveOperationKind::ShuffleIndex {
                    value,
                    source_lane,
                    tile_width,
                } => {
                    let value = scalar_value(&state.values, value)?;
                    let source_lane = scalar_value(&state.values, source_lane)?;
                    if value.scalar != scalar
                        || value.scalar.is_float()
                        || source_lane.scalar != ScalarV1::Unsigned(32)
                        || !valid_wave_tile(tile_width, width)
                    {
                        return Err(
                            FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics,
                        );
                    }
                    let depth = bounded_depth([value.depth, source_lane.depth])?;
                    ExpressionV1 {
                        scalar,
                        kind: ExpressionKindV1::WaveShuffleIndex {
                            value: Box::new(value),
                            source_lane: Box::new(source_lane),
                            tile_width,
                        },
                        depth,
                    }
                }
                WaveOperationKind::BroadcastF32 {
                    value,
                    source_lane,
                    tile_width,
                } => {
                    let value = scalar_value(&state.values, value)?;
                    let source_lane = scalar_value(&state.values, source_lane)?;
                    if value.scalar != scalar
                        || scalar != ScalarV1::Float(ScalarType::F32)
                        || source_lane.scalar != ScalarV1::Unsigned(32)
                        || !valid_wave_tile(tile_width, width)
                    {
                        return Err(
                            FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics,
                        );
                    }
                    let depth = bounded_depth([value.depth, source_lane.depth])?;
                    ExpressionV1 {
                        scalar,
                        kind: ExpressionKindV1::WaveBroadcast {
                            value: Box::new(value),
                            source_lane: Box::new(source_lane),
                            tile_width,
                        },
                        depth,
                    }
                }
                WaveOperationKind::ReduceF32 {
                    value,
                    tile_width,
                    kind,
                } => {
                    if kind == WaveF32ReductionKindV1::Sum {
                        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
                    }
                    let value = scalar_value(&state.values, value)?;
                    if value.scalar != ScalarV1::Float(ScalarType::F32)
                        || scalar != value.scalar
                        || !valid_wave_tile(tile_width, width)
                    {
                        return Err(
                            FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics,
                        );
                    }
                    require_strict_float_bit_policy(scalar, numerical_policies)?;
                    let depth = bounded_depth([value.depth])?;
                    ExpressionV1 {
                        scalar,
                        kind: ExpressionKindV1::WaveReduceMaxF32 {
                            value: Box::new(value),
                            tile_width,
                        },
                        depth,
                    }
                }
            };
            Some(SymbolicValueV1::Scalar(expression))
        }
        OperationKind::Matrix(_) | OperationKind::Gfx950LdsTranspose(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedTensorSemantics);
        }
        OperationKind::Barrier(barrier) => {
            record_event(
                state,
                vec![
                    EventFieldV1::Constant(100),
                    EventFieldV1::Constant(i128::from(barrier.execution_scope.rank())),
                    EventFieldV1::Constant(i128::from(barrier.memory_scope.rank())),
                    EventFieldV1::Constant(i128::from(memory_ordering_tag_v1(
                        barrier.semantics.ordering,
                    ))),
                    EventFieldV1::Constant(i128::from(address_space_mask_v1(
                        &barrier.semantics.address_spaces,
                    ))),
                ],
            )?;
            None
        }
        OperationKind::Fence(fence) => {
            record_event(
                state,
                vec![
                    EventFieldV1::Constant(101),
                    EventFieldV1::Constant(i128::from(fence.memory_scope.rank())),
                    EventFieldV1::Constant(i128::from(memory_ordering_tag_v1(
                        fence.semantics.ordering,
                    ))),
                    EventFieldV1::Constant(i128::from(address_space_mask_v1(
                        &fence.semantics.address_spaces,
                    ))),
                ],
            )?;
            None
        }
        OperationKind::WorkgroupBarrier(barrier) => {
            if barrier.convergence.scope() != fe2o3_kernel_ir::SynchronizationScope::Workgroup {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics);
            }
            record_event(
                state,
                vec![
                    EventFieldV1::Constant(102),
                    EventFieldV1::Constant(i128::from(barrier.memory_scope.rank())),
                    EventFieldV1::Constant(i128::from(memory_ordering_tag_v1(
                        barrier.semantics.ordering,
                    ))),
                    EventFieldV1::Constant(i128::from(address_space_mask_v1(
                        &barrier.semantics.address_spaces,
                    ))),
                ],
            )?;
            None
        }
        OperationKind::WorkgroupMemory(memory) => {
            let fe2o3_kernel_ir::WorkgroupMemoryExtent::Static(extent) = memory.extent else {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics);
            };
            let [result] = operation.results.as_slice() else {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            };
            let Type::Pointer(pointer) = &result.ty else {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            };
            let element = memory
                .element
                .as_scalar()
                .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
                .and_then(ScalarV1::from_ir)?;
            if extent == 0
                || pointer.address_space != AddressSpace::Workgroup
                || pointer.pointee.as_scalar() != memory.element.as_scalar()
            {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            }
            let memory_parameter = allocate_workgroup_memory_root(
                state,
                wave_widths,
                Some(element),
                element.width(),
                u64::from(extent),
                memory.alignment,
                workgroup_width,
            )?;
            Some(SymbolicValueV1::Pointer(PointerV1 {
                memory_parameter,
                element,
                address_space: AddressSpace::Workgroup,
                access: pointer.access,
                index: index_zero(),
            }))
        }
        OperationKind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } => {
            let [result] = operation.results.as_slice() else {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            };
            let Type::Pointer(pointer_type) = &result.ty else {
                return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
            };
            let scalar = element
                .as_scalar()
                .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
                .and_then(ScalarV1::from_ir)?;
            let elements = match count {
                Some(count) => {
                    let count = scalar_value(&state.values, *count)?;
                    evaluate_constant(&count).and_then(|value| u64::try_from(value).ok())
                }
                None => Some(1),
            }
            .filter(|elements| *elements != 0)
            .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
            if *address_space != AddressSpace::Private
                || pointer_type.address_space != AddressSpace::Private
                || pointer_type.pointee.as_scalar() != element.as_scalar()
            {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
            }
            let memory_parameter =
                allocate_private_memory_root(state, scalar, elements, *alignment)?;
            Some(SymbolicValueV1::Pointer(PointerV1 {
                memory_parameter,
                element: scalar,
                address_space: AddressSpace::Private,
                access: pointer_type.access,
                index: index_zero(),
            }))
        }
        OperationKind::ExecutionCapability(capability) => match &capability.operation {
            ExecutionCapabilityOperationV1::WorkgroupDerive { .. } => {
                let [context] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(context), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                Some(SymbolicValueV1::Opaque)
            }
            ExecutionCapabilityOperationV1::SubgroupDerive { width, .. } => {
                let [workgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque))
                    || *width == 0
                    || !width.is_power_of_two()
                    || *width > 64
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
                }
                wave_widths.insert(*width);
                Some(SymbolicValueV1::Opaque)
            }
            ExecutionCapabilityOperationV1::WorkgroupBarrier { semantics, .. } => {
                let [workgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque))
                    || semantics.scope != fe2o3_kernel_ir::ExecutionMemoryScopeV1::Workgroup
                {
                    return Err(
                        FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics,
                    );
                }
                record_execution_synchronization_event(state, 102, *semantics)?;
                Some(SymbolicValueV1::Opaque)
            }
            ExecutionCapabilityOperationV1::SubgroupBarrier {
                semantics, width, ..
            } => {
                let [workgroup, subgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque))
                    || !matches!(state.values.get(subgroup), Some(SymbolicValueV1::Opaque))
                    || semantics.scope != fe2o3_kernel_ir::ExecutionMemoryScopeV1::Subgroup
                    || !matches!(width, 32 | 64)
                {
                    return Err(
                        FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics,
                    );
                }
                wave_widths.insert(*width);
                record_execution_synchronization_event(state, 101, *semantics)?;
                bind_operation_results(
                    operation,
                    state,
                    vec![SymbolicValueV1::Opaque, SymbolicValueV1::Opaque],
                )?;
                return Ok(());
            }
            ExecutionCapabilityOperationV1::WorkgroupFence { semantics, .. } => {
                let [workgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                record_execution_synchronization_event(state, 101, *semantics)?;
                None
            }
            ExecutionCapabilityOperationV1::SubgroupFence {
                semantics, width, ..
            } => {
                let [subgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(subgroup), Some(SymbolicValueV1::Opaque))
                    || !matches!(width, 32 | 64)
                {
                    return Err(
                        FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics,
                    );
                }
                wave_widths.insert(*width);
                record_execution_synchronization_event(state, 101, *semantics)?;
                None
            }
            ExecutionCapabilityOperationV1::MatrixAccess { width, .. } => {
                let [subgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(subgroup), Some(SymbolicValueV1::Opaque))
                    || !matches!(width, 32 | 64)
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedTensorSemantics);
                }
                wave_widths.insert(*width);
                Some(SymbolicValueV1::Opaque)
            }
            ExecutionCapabilityOperationV1::LdsAllocate {
                layout, elements, ..
            }
            | ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
                layout, elements, ..
            } => {
                let [workgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let memory_root = allocate_workgroup_memory_root(
                    state,
                    wave_widths,
                    None,
                    execution_layout_width(*layout)?,
                    *elements,
                    u32::from(layout.byte_alignment),
                    workgroup_width,
                )?;
                Some(SymbolicValueV1::WorkgroupView(WorkgroupViewV1 {
                    memory_root,
                    elements: *elements,
                    layout: *layout,
                    initialized: false,
                    published: false,
                }))
            }
            ExecutionCapabilityOperationV1::WorkgroupMemoryIndex { .. } => {
                let [workgroup] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let kind = IntrinsicKind::InvocationIndex {
                    kind: fe2o3_kernel_ir::IndexKind::Local,
                    axis: fe2o3_kernel_ir::Axis::X,
                };
                intrinsics.insert(kind);
                Some(SymbolicValueV1::WorkgroupIndex(ExpressionV1::leaf(
                    ScalarV1::Unsigned(64),
                    ExpressionKindV1::Intrinsic(kind),
                )))
            }
            ExecutionCapabilityOperationV1::RawMemoryBind {
                extent,
                layout,
                space: fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Global,
                access,
                ..
            } => {
                let [authority, pointer, length, unsafe_obligation] =
                    capability.operands.as_slice()
                else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(authority), Some(SymbolicValueV1::Opaque))
                    || extent.operand != 2
                    || extent.source_argument != 2
                    || extent.bound_check_operand != 3
                    || extent.nonnegative_check_operand.is_some()
                    || !matches!(extent.value_type, ScalarType::U64 | ScalarType::Index)
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let mut pointer = pointer_value(&state.values, *pointer)?;
                let length = scalar_value(&state.values, *length)?;
                let proof = scalar_value(&state.values, *unsafe_obligation)?;
                if pointer.address_space != AddressSpace::Global
                    || pointer.element.width() != execution_layout_width(*layout)?
                    || pointer.access != AccessMode::ReadWrite
                    || length.scalar != ScalarV1::Unsigned(64)
                    || proof.scalar != ScalarV1::Bool
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let alias_tag = execution_alias_contract_tag(*access);
                match capability_roots.insert(pointer.memory_parameter, alias_tag) {
                    Some(previous) if previous != alias_tag => {
                        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
                    }
                    _ => {}
                }
                pointer.access = access.access_mode();
                Some(SymbolicValueV1::MemoryView(BoundedMemoryViewV1 {
                    pointer,
                    extent: length,
                    extent_contract: fe2o3_kernel_ir::ExecutionMemoryExtentV1::Dynamic(*extent),
                    layout: *layout,
                    access: *access,
                }))
            }
            ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
                element,
                layout,
                elements,
                ..
            } => {
                let [context] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                let scalar = execution_element_scalars
                    .get(element)
                    .copied()
                    .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)?;
                if !matches!(state.values.get(context), Some(SymbolicValueV1::Opaque))
                    || execution_layout_width(*layout)? != scalar.width()
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let memory_parameter = allocate_private_memory_root(
                    state,
                    scalar,
                    *elements,
                    u32::from(layout.byte_alignment),
                )?;
                Some(SymbolicValueV1::MemoryView(BoundedMemoryViewV1 {
                    pointer: PointerV1 {
                        memory_parameter,
                        element: scalar,
                        address_space: AddressSpace::Private,
                        access: AccessMode::ReadWrite,
                        index: index_zero(),
                    },
                    extent: ExpressionV1::leaf(
                        ScalarV1::Unsigned(64),
                        ExpressionKindV1::Constant(i128::from(*elements)),
                    ),
                    extent_contract: fe2o3_kernel_ir::ExecutionMemoryExtentV1::Static(*elements),
                    layout: *layout,
                    access: fe2o3_kernel_ir::ExecutionMemoryAccessV1::ExclusiveReadWrite,
                }))
            }
            ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
                layout, elements, ..
            } => {
                let [view, workgroup, value] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let mut view = workgroup_view_value(&state.values, *view)?;
                let value = scalar_value(&state.values, *value)?;
                require_workgroup_view(&view, *layout, *elements, false)?;
                let kind = IntrinsicKind::InvocationIndex {
                    kind: fe2o3_kernel_ir::IndexKind::Local,
                    axis: fe2o3_kernel_ir::Axis::X,
                };
                intrinsics.insert(kind);
                let index =
                    ExpressionV1::leaf(ScalarV1::Unsigned(64), ExpressionKindV1::Intrinsic(kind));
                record_workgroup_store(state, &view, index, value)?;
                view.initialized = true;
                Some(SymbolicValueV1::WorkgroupView(view))
            }
            ExecutionCapabilityOperationV1::LdsPublish {
                layout, elements, ..
            } => {
                let [workgroup, view] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let mut view = workgroup_view_value(&state.values, *view)?;
                require_workgroup_view(&view, *layout, *elements, false)?;
                require_complete_workgroup_initialization(
                    state
                        .memories
                        .get(&view.memory_root)
                        .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?,
                )?;
                view.published = true;
                record_workgroup_publication(state)?;
                bind_operation_results(
                    operation,
                    state,
                    vec![
                        SymbolicValueV1::Opaque,
                        SymbolicValueV1::WorkgroupView(view),
                    ],
                )?;
                return Ok(());
            }
            ExecutionCapabilityOperationV1::WorkgroupMemoryPublish { layout, .. } => {
                let [workgroup, view] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let mut view = workgroup_view_value(&state.values, *view)?;
                require_workgroup_view(&view, *layout, view.elements, false)?;
                require_complete_workgroup_initialization(
                    state
                        .memories
                        .get(&view.memory_root)
                        .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?,
                )?;
                view.published = true;
                record_workgroup_publication(state)?;
                bind_operation_results(
                    operation,
                    state,
                    vec![
                        SymbolicValueV1::Opaque,
                        SymbolicValueV1::WorkgroupView(view),
                    ],
                )?;
                return Ok(());
            }
            ExecutionCapabilityOperationV1::LdsReadPublished {
                layout, elements, ..
            } => {
                let [view, workgroup, index] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let view = workgroup_view_value(&state.values, *view)?;
                require_workgroup_view(&view, *layout, *elements, true)?;
                let index = workgroup_index_value(&state.values, *index)?;
                let values = workgroup_optional_load(operation, state, &view, index)?;
                bind_operation_results(operation, state, values)?;
                return Ok(());
            }
            ExecutionCapabilityOperationV1::MemoryStore {
                workgroup: Some(_),
                layout,
                space: fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Workgroup,
                access: fe2o3_kernel_ir::ExecutionMemoryAccessV1::DisjointWrite,
                ..
            } => {
                let [view, workgroup, index, value] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let view = workgroup_view_value(&state.values, *view)?;
                require_workgroup_view(&view, *layout, view.elements, false)?;
                let index = workgroup_index_value(&state.values, *index)?;
                let value = scalar_value(&state.values, *value)?;
                let present = record_workgroup_store(state, &view, index, value)?;
                Some(SymbolicValueV1::Scalar(present))
            }
            ExecutionCapabilityOperationV1::MemoryLoad {
                workgroup: Some(_),
                layout,
                space: fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Workgroup,
                access: fe2o3_kernel_ir::ExecutionMemoryAccessV1::ReadOnly,
                ..
            } => {
                let [view, workgroup, index] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(workgroup), Some(SymbolicValueV1::Opaque)) {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let view = workgroup_view_value(&state.values, *view)?;
                require_workgroup_view(&view, *layout, view.elements, true)?;
                let index = workgroup_index_value(&state.values, *index)?;
                let values = workgroup_optional_load(operation, state, &view, index)?;
                bind_operation_results(operation, state, values)?;
                return Ok(());
            }
            ExecutionCapabilityOperationV1::MemoryStore {
                workgroup: None,
                layout,
                space,
                access,
                ..
            } if matches!(
                space,
                fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Global
                    | fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Private
            ) =>
            {
                let [view, index, value] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                let view = memory_view_value(&state.values, *view)?;
                let index = scalar_value(&state.values, *index)?;
                let value = scalar_value(&state.values, *value)?;
                if *layout != view.layout
                    || *access != view.access
                    || space.address_space() != view.pointer.address_space
                    || !matches!(
                        access,
                        fe2o3_kernel_ir::ExecutionMemoryAccessV1::ExclusiveReadWrite
                            | fe2o3_kernel_ir::ExecutionMemoryAccessV1::DisjointWrite
                    )
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let present = record_bounded_view_store(state, &view, index, value)?;
                Some(SymbolicValueV1::Scalar(present))
            }
            ExecutionCapabilityOperationV1::MemoryLoad {
                workgroup: None,
                layout,
                space,
                access,
                ..
            } if matches!(
                space,
                fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Global
                    | fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Private
            ) =>
            {
                let [view, index] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                let view = memory_view_value(&state.values, *view)?;
                let index = scalar_value(&state.values, *index)?;
                if *layout != view.layout
                    || *access != view.access
                    || space.address_space() != view.pointer.address_space
                    || !matches!(
                        access,
                        fe2o3_kernel_ir::ExecutionMemoryAccessV1::ReadOnly
                            | fe2o3_kernel_ir::ExecutionMemoryAccessV1::ExclusiveReadWrite
                    )
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                }
                let values = bounded_view_optional_load(operation, state, &view, index)?;
                bind_operation_results(operation, state, values)?;
                return Ok(());
            }
            ExecutionCapabilityOperationV1::SubgroupCollective {
                kind,
                value_type,
                width,
                ..
            } => {
                let [authority, value] = capability.operands.as_slice() else {
                    return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
                };
                if !matches!(state.values.get(authority), Some(SymbolicValueV1::Opaque))
                    || *width == 0
                    || !width.is_power_of_two()
                    || *width > 64
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
                }
                let value = scalar_value(&state.values, *value)?;
                let scalar = scalar_result(operation)?;
                if ScalarV1::from_ir(*value_type)? != scalar
                    || value.scalar != scalar
                    || scalar.is_float()
                    || scalar == ScalarV1::Bool
                {
                    return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
                }
                let kind = match kind {
                    ExecutionCollectiveKindV1::ReduceSum => SubgroupSumKindV1::Reduce,
                    ExecutionCollectiveKindV1::InclusiveScanSum => SubgroupSumKindV1::InclusiveScan,
                    ExecutionCollectiveKindV1::ExclusiveScanSum => SubgroupSumKindV1::ExclusiveScan,
                };
                wave_widths.insert(*width);
                let depth = bounded_depth([value.depth])?;
                Some(SymbolicValueV1::Scalar(ExpressionV1 {
                    scalar,
                    kind: ExpressionKindV1::SubgroupSum {
                        value: Box::new(value),
                        width: *width,
                        kind,
                    },
                    depth,
                }))
            }
            _ => {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics);
            }
        },
        OperationKind::Atomic(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAtomicSemantics);
        }
        OperationKind::InlineAssembly(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
        }
    };
    if let Some(defined) = defined {
        bind_operation_results(operation, state, vec![defined])?;
    } else if !operation.results.is_empty() {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    Ok(())
}

fn bind_operation_results(
    operation: &Operation,
    state: &mut ExecutionStateV1,
    values: Vec<SymbolicValueV1>,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if operation.results.len() != values.len() {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    for (result, value) in operation.results.iter().zip(values) {
        if !value_matches_type(&value, &result.ty)
            || state.values.insert(result.id, value).is_some()
        {
            return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
        }
    }
    Ok(())
}

fn enqueue_successor(
    mut state: ExecutionStateV1,
    target: BlockId,
    arguments: &[ValueId],
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    pending: &mut VecDeque<ExecutionStateV1>,
    scheduled_states: &mut usize,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let block = blocks
        .get(&target)
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow)?;
    if block.parameters.len() != arguments.len() {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let incoming = arguments
        .iter()
        .map(|identity| {
            state
                .values
                .get(identity)
                .cloned()
                .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Snapshot incoming values before discarding this block's previous iteration.
    // Static SSA uniqueness is checked once by extract_kernel_effects.
    for result in block
        .operations
        .iter()
        .flat_map(|operation| &operation.results)
    {
        state.values.remove(&result.id);
    }
    for (parameter, value) in block.parameters.iter().zip(incoming) {
        if !value_matches_type(&value, &parameter.ty) {
            return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
        }
        state.values.insert(parameter.id, value);
    }
    let visits = state.visits.entry(target).or_default();
    *visits = visits
        .checked_add(1)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if *visits > MAX_FINAL_KIR_BLOCK_VISITS_V1 {
        return Err(FinalKirOutputEquivalenceErrorV1::LoopBoundExceeded);
    }
    *scheduled_states = scheduled_states
        .checked_add(1)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if *scheduled_states > MAX_FINAL_KIR_EXECUTION_STATES_V1 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    state.block = target;
    pending.push_back(state);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn enqueue_switch(
    state: ExecutionStateV1,
    selector: ExpressionV1,
    arms: &[(ExpressionV1, BlockId, &[ValueId])],
    default_target: BlockId,
    default_arguments: &[ValueId],
    blocks: &BTreeMap<BlockId, &BasicBlock>,
    pending: &mut VecDeque<ExecutionStateV1>,
    scheduled_states: &mut usize,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if let Some(selected) = evaluate_constant(&selector) {
        if let Some((_, target, arguments)) = arms
            .iter()
            .find(|(value, _, _)| evaluate_constant(value) == Some(selected))
        {
            return enqueue_successor(state, *target, arguments, blocks, pending, scheduled_states);
        }
        return enqueue_successor(
            state,
            default_target,
            default_arguments,
            blocks,
            pending,
            scheduled_states,
        );
    }

    let mut default_guard = state.path.clone();
    for (value, target, arguments) in arms {
        let equal = equality(selector.clone(), value.clone())?;
        let mut case_state = state.clone();
        case_state.path = bool_and(case_state.path, equal.clone())?;
        enqueue_successor(
            case_state,
            *target,
            arguments,
            blocks,
            pending,
            scheduled_states,
        )?;
        default_guard = bool_and(default_guard, bool_not(equal)?)?;
    }
    let mut default_state = state;
    default_state.path = default_guard;
    enqueue_successor(
        default_state,
        default_target,
        default_arguments,
        blocks,
        pending,
        scheduled_states,
    )
}

fn value_matches_type(value: &SymbolicValueV1, ty: &Type) -> bool {
    match (value, ty) {
        (SymbolicValueV1::Scalar(value), Type::Scalar(scalar)) => {
            ScalarV1::from_ir(*scalar).ok() == Some(value.scalar)
        }
        (SymbolicValueV1::Pointer(value), Type::Pointer(pointer)) => {
            pointer
                .pointee
                .as_scalar()
                .and_then(|item| ScalarV1::from_ir(item).ok())
                == Some(value.element)
                && pointer.address_space == value.address_space
                && pointer.access == value.access
        }
        (SymbolicValueV1::Slice(value), Type::Slice(slice)) => {
            slice
                .element
                .as_scalar()
                .and_then(|item| ScalarV1::from_ir(item).ok())
                == Some(value.element)
                && slice.address_space == value.address_space
                && slice.access == value.access
        }
        (SymbolicValueV1::Capability(value), Type::GlobalCapability(capability)) => {
            capability
                .element()
                .as_scalar()
                .and_then(|item| ScalarV1::from_ir(item).ok())
                == Some(value.physical.element)
                && capability.role() == value.role
        }
        (SymbolicValueV1::WorkgroupView(value), Type::ExecutionCapability(capability)) => {
            let expected_lds_state = if value.published {
                fe2o3_kernel_ir::ExecutionLdsStateV1::Published
            } else if value.initialized {
                fe2o3_kernel_ir::ExecutionLdsStateV1::InvocationInitialized
            } else {
                fe2o3_kernel_ir::ExecutionLdsStateV1::Uninitialized
            };
            value.elements != 0
                && value.layout.is_complete()
                && capability.role.is_complete()
                && match &capability.role {
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::Lds {
                        layout,
                        elements,
                        state,
                        ..
                    } => {
                        *layout == value.layout
                            && *elements == value.elements
                            && *state == expected_lds_state
                    }
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::MemoryView {
                        layout,
                        extent: fe2o3_kernel_ir::ExecutionMemoryExtentV1::Static(elements),
                        space: fe2o3_kernel_ir::ExecutionMemoryAddressSpaceV1::Workgroup,
                        initialization,
                        ..
                    } => {
                        *layout == value.layout
                            && *elements == value.elements
                            && *initialization
                                == if value.published {
                                    fe2o3_kernel_ir::ExecutionMemoryInitializationV1::Published
                                } else {
                                    fe2o3_kernel_ir::ExecutionMemoryInitializationV1::Uninitialized
                                }
                    }
                    _ => false,
                }
        }
        (SymbolicValueV1::WorkgroupIndex(value), Type::ExecutionCapability(capability)) => {
            value.scalar == ScalarV1::Unsigned(64)
                && matches!(
                    &capability.role,
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::WorkgroupMemoryIndex
                )
        }
        (SymbolicValueV1::MemoryView(value), Type::ExecutionCapability(capability)) => {
            matches!(
                &capability.role,
                fe2o3_kernel_ir::ExecutionCapabilityRoleV1::MemoryView {
                    layout,
                    space,
                    access,
                    extent,
                    initialization:
                        fe2o3_kernel_ir::ExecutionMemoryInitializationV1::FullyInitialized,
                    ..
                } if *layout == value.layout
                    && *access == value.access
                    && *extent == value.extent_contract
                    && space.address_space() == value.pointer.address_space
            )
        }
        (
            SymbolicValueV1::Opaque,
            Type::Unit | Type::KernelContext(_) | Type::ExecutionCapability(_),
        ) => true,
        _ => false,
    }
}

fn bool_not(value: ExpressionV1) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    ExpressionV1::unary(ScalarV1::Bool, UnaryOp::Not, value)
}

const fn valid_wave_tile(tile_width: u32, wave_width: u32) -> bool {
    tile_width != 0 && tile_width.is_power_of_two() && tile_width <= wave_width
}

fn require_strict_float_bit_policy(
    scalar: ScalarV1,
    numerical_policies: &BTreeMap<ScalarType, NumericalModeV1>,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let ScalarV1::Float(value_type) = scalar else {
        return Ok(());
    };
    if numerical_policies.get(&value_type) != Some(&NumericalModeV1::StrictIeee) {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
    }
    Ok(())
}

fn bool_and(
    lhs: ExpressionV1,
    rhs: ExpressionV1,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    ExpressionV1::binary(ScalarV1::Bool, BinaryOp::BitAnd, lhs, rhs)
}

fn equality(
    lhs: ExpressionV1,
    rhs: ExpressionV1,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    if lhs.scalar != rhs.scalar {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let depth = bounded_depth([lhs.depth, rhs.depth])?;
    Ok(ExpressionV1 {
        scalar: ScalarV1::Bool,
        kind: ExpressionKindV1::Compare {
            predicate: ComparePredicate::Equal,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        depth,
    })
}

fn constant_for_scalar(
    scalar: ScalarV1,
    value: i128,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    if scalar.is_float() {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
    }
    Ok(ExpressionV1::leaf(
        scalar,
        ExpressionKindV1::Constant(value),
    ))
}

fn constant_bool(value: &ExpressionV1) -> Option<bool> {
    (value.scalar == ScalarV1::Bool)
        .then(|| evaluate_constant(value).map(|item| item != 0))
        .flatten()
}

fn evaluate_constant(expression: &ExpressionV1) -> Option<i128> {
    evaluate_constant_with_lane(expression, None)
}

fn evaluate_constant_at_lane(expression: &ExpressionV1, lane: u32) -> Option<i128> {
    evaluate_constant_with_lane(expression, Some(i128::from(lane)))
}

fn evaluate_constant_with_lane(expression: &ExpressionV1, lane: Option<i128>) -> Option<i128> {
    let width = expression.scalar.width();
    let value = match &expression.kind {
        ExpressionKindV1::Constant(value) => *value,
        ExpressionKindV1::Intrinsic(IntrinsicKind::InvocationIndex {
            kind: fe2o3_kernel_ir::IndexKind::Local,
            axis: fe2o3_kernel_ir::Axis::X,
        }) => lane?,
        ExpressionKindV1::WaveLaneId { width } => lane? % i128::from(*width),
        ExpressionKindV1::Unary { operation, operand } => {
            let operand = evaluate_constant_with_lane(operand, lane)?;
            match operation {
                UnaryOp::Not if expression.scalar == ScalarV1::Bool => i128::from(operand == 0),
                UnaryOp::Not => modulus(width)?.checked_sub(1)?.checked_sub(operand)?,
                UnaryOp::Negate if !expression.scalar.is_float() => operand.checked_neg()?,
                UnaryOp::Negate => return None,
            }
        }
        ExpressionKindV1::Binary {
            operation,
            lhs,
            rhs,
        } if !expression.scalar.is_float() => {
            let lhs = evaluate_constant_with_lane(lhs, lane)?;
            let rhs = evaluate_constant_with_lane(rhs, lane)?;
            match operation {
                BinaryOp::Add => lhs.checked_add(rhs)?,
                BinaryOp::Subtract => lhs.checked_sub(rhs)?,
                BinaryOp::Multiply => lhs.checked_mul(rhs)?,
                BinaryOp::BitAnd => lhs & rhs,
                BinaryOp::BitOr => lhs | rhs,
                BinaryOp::BitXor => lhs ^ rhs,
                BinaryOp::ShiftLeft => lhs.checked_shl(u32::try_from(rhs).ok()?)?,
                BinaryOp::ShiftRight if expression.scalar.is_signed() => {
                    signed_value(lhs, width).checked_shr(u32::try_from(rhs).ok()?)?
                }
                BinaryOp::ShiftRight => lhs.checked_shr(u32::try_from(rhs).ok()?)?,
                BinaryOp::Divide | BinaryOp::Remainder | BinaryOp::Checked(_) => return None,
            }
        }
        ExpressionKindV1::Compare {
            predicate,
            lhs,
            rhs,
        } if !lhs.scalar.is_float() => {
            let mut lhs_value = evaluate_constant_with_lane(lhs, lane)?;
            let mut rhs_value = evaluate_constant_with_lane(rhs, lane)?;
            if lhs.scalar.is_signed() {
                lhs_value = signed_value(lhs_value, lhs.scalar.width());
                rhs_value = signed_value(rhs_value, rhs.scalar.width());
            }
            i128::from(match predicate {
                ComparePredicate::Equal => lhs_value == rhs_value,
                ComparePredicate::NotEqual => lhs_value != rhs_value,
                ComparePredicate::LessThan => lhs_value < rhs_value,
                ComparePredicate::LessThanOrEqual => lhs_value <= rhs_value,
                ComparePredicate::GreaterThan => lhs_value > rhs_value,
                ComparePredicate::GreaterThanOrEqual => lhs_value >= rhs_value,
            })
        }
        ExpressionKindV1::Cast { kind, operand } if !expression.scalar.is_float() => {
            let operand_value = evaluate_constant_with_lane(operand, lane)?;
            match kind {
                CastKind::SignExtend => signed_value(operand_value, operand.scalar.width()),
                CastKind::Truncate
                | CastKind::ZeroExtend
                | CastKind::Bitcast
                | CastKind::RestrictPointerAccess => operand_value,
                CastKind::FloatExtend
                | CastKind::FloatTruncate
                | CastKind::IntegerToFloat
                | CastKind::FloatToInteger => return None,
            }
        }
        ExpressionKindV1::Select {
            condition,
            when_true,
            when_false,
        } => {
            if evaluate_constant_with_lane(condition, lane)? != 0 {
                evaluate_constant_with_lane(when_true, lane)?
            } else {
                evaluate_constant_with_lane(when_false, lane)?
            }
        }
        _ => return None,
    };
    Some(normalize_constant(value, width))
}

fn modulus(width: u16) -> Option<i128> {
    1_i128.checked_shl(u32::from(width))
}

fn normalize_constant(value: i128, width: u16) -> i128 {
    let modulus = modulus(width).expect("supported scalar widths are at most 64 bits");
    value.rem_euclid(modulus)
}

fn signed_value(value: i128, width: u16) -> i128 {
    let normalized = normalize_constant(value, width);
    let sign = 1_i128 << (width - 1);
    if normalized >= sign {
        normalized - modulus(width).expect("supported scalar widths are at most 64 bits")
    } else {
        normalized
    }
}

fn infer_execution_element_scalars(
    function: &Function,
) -> Result<
    BTreeMap<fe2o3_kernel_ir::ExecutionTypeIdentityV1, ScalarV1>,
    FinalKirOutputEquivalenceErrorV1,
> {
    let body = function
        .body
        .as_ref()
        .ok_or(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch)?;
    let mut value_types = body
        .parameters
        .iter()
        .copied()
        .zip(function.signature.parameters.iter())
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        value_types.extend(block.parameters.iter().map(|value| (value.id, &value.ty)));
        value_types.extend(
            block
                .operations
                .iter()
                .flat_map(|operation| &operation.results)
                .map(|value| (value.id, &value.ty)),
        );
    }

    let mut inferred = BTreeMap::new();
    for operation in body.blocks.iter().flat_map(|block| &block.operations) {
        let OperationKind::ExecutionCapability(capability) = &operation.kind else {
            continue;
        };
        let candidate = match &capability.operation {
            ExecutionCapabilityOperationV1::RawMemoryBind { element, .. } => capability
                .operands
                .get(1)
                .and_then(|value| value_types.get(value))
                .and_then(|ty| match ty {
                    Type::Pointer(pointer) => pointer.pointee.as_scalar(),
                    Type::Slice(slice) => slice.element.as_scalar(),
                    _ => None,
                })
                .map(|scalar| (*element, scalar)),
            ExecutionCapabilityOperationV1::LdsInitializeByInvocation { element, .. }
            | ExecutionCapabilityOperationV1::MemoryStore { element, .. } => capability
                .operands
                .last()
                .and_then(|value| value_types.get(value))
                .and_then(|ty| ty.as_scalar())
                .map(|scalar| (*element, scalar)),
            ExecutionCapabilityOperationV1::LdsReadPublished { element, .. }
            | ExecutionCapabilityOperationV1::MemoryLoad { element, .. } => operation
                .results
                .first()
                .and_then(|value| value.ty.as_scalar())
                .map(|scalar| (*element, scalar)),
            ExecutionCapabilityOperationV1::Atomic {
                element,
                value_type,
                ..
            }
            | ExecutionCapabilityOperationV1::WorkgroupCollective {
                element,
                value_type,
                ..
            }
            | ExecutionCapabilityOperationV1::SubgroupCollective {
                element,
                value_type,
                ..
            } => Some((*element, *value_type)),
            _ => None,
        };
        let Some((identity, scalar)) = candidate else {
            continue;
        };
        let scalar = ScalarV1::from_ir(scalar)?;
        if inferred
            .insert(identity, scalar)
            .is_some_and(|prior| prior != scalar)
        {
            return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
        }
    }
    Ok(inferred)
}

fn parameter_value(
    ordinal: u32,
    ty: &Type,
) -> Result<SymbolicValueV1, FinalKirOutputEquivalenceErrorV1> {
    Ok(match ty {
        Type::Scalar(scalar) => SymbolicValueV1::Scalar(ExpressionV1::leaf(
            ScalarV1::from_ir(*scalar)?,
            ExpressionKindV1::Parameter(ordinal),
        )),
        Type::Pointer(pointer) => SymbolicValueV1::Pointer(PointerV1 {
            memory_parameter: ordinal,
            element: pointer
                .pointee
                .as_scalar()
                .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
                .and_then(ScalarV1::from_ir)?,
            address_space: pointer.address_space,
            access: pointer.access,
            index: index_zero(),
        }),
        Type::Slice(slice) => SymbolicValueV1::Slice(SliceV1 {
            memory_parameter: ordinal,
            element: slice
                .element
                .as_scalar()
                .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
                .and_then(ScalarV1::from_ir)?,
            address_space: slice.address_space,
            access: slice.access,
        }),
        Type::KernelContext(_) | Type::GlobalCapability(_) | Type::ExecutionCapability(_) => {
            SymbolicValueV1::Opaque
        }
        Type::Unit => return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType),
    })
}

fn memory_element(ty: &Type) -> Result<Option<ScalarV1>, FinalKirOutputEquivalenceErrorV1> {
    match ty {
        Type::Pointer(pointer) => pointer
            .pointee
            .as_scalar()
            .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
            .and_then(ScalarV1::from_ir)
            .map(Some),
        Type::Slice(slice) => slice
            .element
            .as_scalar()
            .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
            .and_then(ScalarV1::from_ir)
            .map(Some),
        _ => Ok(None),
    }
}

fn constant_expression(
    constant: &Constant,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    let value = match constant {
        Constant::Bool(value) => i128::from(*value),
        Constant::I8(value) => i128::from(*value),
        Constant::I16(value) => i128::from(*value),
        Constant::I32(value) => i128::from(*value),
        Constant::I64(value) => i128::from(*value),
        Constant::U8(value) => i128::from(*value),
        Constant::U16(value) | Constant::F16Bits(value) | Constant::Bf16Bits(value) => {
            i128::from(*value)
        }
        Constant::U32(value) | Constant::F32Bits(value) => i128::from(*value),
        Constant::U64(value) | Constant::Index(value) | Constant::F64Bits(value) => {
            i128::from(*value)
        }
    };
    let scalar = constant
        .ty()
        .as_scalar()
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
        .and_then(ScalarV1::from_ir)?;
    Ok(ExpressionV1::leaf(
        scalar,
        ExpressionKindV1::Constant(value),
    ))
}

fn scalar_result(
    operation: &fe2o3_kernel_ir::Operation,
) -> Result<ScalarV1, FinalKirOutputEquivalenceErrorV1> {
    let [result] = operation.results.as_slice() else {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    };
    result
        .ty
        .as_scalar()
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
        .and_then(ScalarV1::from_ir)
}

fn scalar_value(
    values: &BTreeMap<ValueId, SymbolicValueV1>,
    identity: ValueId,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    match values.get(&identity) {
        Some(SymbolicValueV1::Scalar(expression)) => Ok(expression.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}

fn pointer_value(
    values: &BTreeMap<ValueId, SymbolicValueV1>,
    identity: ValueId,
) -> Result<PointerV1, FinalKirOutputEquivalenceErrorV1> {
    match values.get(&identity) {
        Some(SymbolicValueV1::Pointer(pointer)) => Ok(pointer.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}

fn slice_value(
    values: &BTreeMap<ValueId, SymbolicValueV1>,
    identity: ValueId,
) -> Result<SliceV1, FinalKirOutputEquivalenceErrorV1> {
    match values.get(&identity) {
        Some(SymbolicValueV1::Slice(slice)) => Ok(slice.clone()),
        Some(SymbolicValueV1::Capability(capability)) => Ok(capability.physical.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}

fn capability_value(
    values: &BTreeMap<ValueId, SymbolicValueV1>,
    identity: ValueId,
) -> Result<CapabilityV1, FinalKirOutputEquivalenceErrorV1> {
    match values.get(&identity) {
        Some(SymbolicValueV1::Capability(capability)) => Ok(capability.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}

fn workgroup_view_value(
    values: &BTreeMap<ValueId, SymbolicValueV1>,
    identity: ValueId,
) -> Result<WorkgroupViewV1, FinalKirOutputEquivalenceErrorV1> {
    match values.get(&identity) {
        Some(SymbolicValueV1::WorkgroupView(view)) => Ok(view.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}

fn workgroup_index_value(
    values: &BTreeMap<ValueId, SymbolicValueV1>,
    identity: ValueId,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    match values.get(&identity) {
        Some(SymbolicValueV1::WorkgroupIndex(index)) => Ok(index.clone()),
        Some(SymbolicValueV1::Scalar(index)) => Ok(index.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}

fn memory_view_value(
    values: &BTreeMap<ValueId, SymbolicValueV1>,
    identity: ValueId,
) -> Result<BoundedMemoryViewV1, FinalKirOutputEquivalenceErrorV1> {
    match values.get(&identity) {
        Some(SymbolicValueV1::MemoryView(view)) => Ok(view.clone()),
        _ => Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow),
    }
}

fn require_load_access(
    pointer: &PointerV1,
    access_space: AddressSpace,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if access_space != pointer.address_space
        || !matches!(
            pointer.address_space,
            AddressSpace::Private
                | AddressSpace::Global
                | AddressSpace::Constant
                | AddressSpace::Workgroup
        )
        || pointer.access == AccessMode::WriteOnly
    {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
    }
    Ok(())
}

fn memory_read(
    state: &ExecutionStateV1,
    pointer: PointerV1,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    let memory = state
        .memories
        .get(&pointer.memory_parameter)
        .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?;
    if memory.element != Some(pointer.element) {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    if memory.initial_parameter.is_none() {
        if memory.cross_lane_width.is_some() {
            require_complete_workgroup_initialization(memory)?;
        } else {
            require_complete_private_initialization(memory)?;
        }
    }
    let mut child_depths = Vec::with_capacity(1 + memory.writes.len() * 3);
    child_depths.push(pointer.index.depth);
    for write in &memory.writes {
        child_depths.extend([write.index.depth, write.guard.depth, write.value.depth]);
    }
    let depth = child_depths
        .into_iter()
        .max()
        .unwrap_or(0)
        .checked_add(memory.writes.len() + 1)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if depth > MAX_FINAL_KIR_SYMBOLIC_DEPTH_V1 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    Ok(ExpressionV1 {
        scalar: pointer.element,
        kind: ExpressionKindV1::MemoryRead {
            memory_parameter: pointer.memory_parameter,
            initial_parameter: memory.initial_parameter,
            cross_lane_width: memory.cross_lane_width,
            bounded_elements: memory.bounded_elements,
            writes: memory.writes.clone(),
            index: Box::new(pointer.index),
        },
        depth,
    })
}

fn record_store(
    state: &mut ExecutionStateV1,
    pointer: PointerV1,
    guard: ExpressionV1,
    value: ExpressionV1,
    access_space: AddressSpace,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if pointer.address_space != access_space
        || !matches!(
            access_space,
            AddressSpace::Private | AddressSpace::Global | AddressSpace::Workgroup
        )
        || pointer.access == AccessMode::ReadOnly
        || pointer.element != value.scalar
    {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
    }
    let memory = state
        .memories
        .get_mut(&pointer.memory_parameter)
        .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?;
    if memory
        .element
        .is_some_and(|element| element != value.scalar)
        || memory.writes.len() >= MAX_FINAL_KIR_MEMORY_WRITES_V1
    {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    memory.element = Some(value.scalar);
    memory.writes.push(StoreEffectV1 {
        index: pointer.index,
        guard,
        value,
    });
    if pointer.address_space == AddressSpace::Global {
        state.written_roots.insert(pointer.memory_parameter);
    }
    Ok(())
}

fn require_complete_workgroup_initialization(
    memory: &MemoryVersionV1,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let width = memory
        .cross_lane_width
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics)?;
    let elements = memory
        .bounded_elements
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics)?;
    let mut initialized = BTreeSet::new();
    for lane in 0..width {
        for write in &memory.writes {
            if evaluate_constant_at_lane(&write.guard, lane) != Some(1) {
                continue;
            }
            let index = evaluate_constant_at_lane(&write.index, lane)
                .and_then(|value| u64::try_from(value).ok())
                .filter(|index| *index < elements)
                .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics)?;
            if !initialized.insert(index) {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
            }
        }
    }
    if initialized.len() != usize::try_from(elements).unwrap_or(usize::MAX) {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics);
    }
    Ok(())
}

fn require_complete_private_initialization(
    memory: &MemoryVersionV1,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let elements = memory
        .bounded_elements
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation)?;
    if elements > 64 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    let mut initialized = BTreeSet::new();
    for write in &memory.writes {
        if evaluate_constant(&write.guard) != Some(1) {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
        }
        let index = evaluate_constant(&write.index)
            .and_then(|index| u64::try_from(index).ok())
            .filter(|index| *index < elements)
            .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation)?;
        initialized.insert(index);
    }
    if initialized.len() != usize::try_from(elements).unwrap_or(usize::MAX) {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
    }
    Ok(())
}

fn record_event(
    state: &mut ExecutionStateV1,
    fields: Vec<EventFieldV1>,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if fields.is_empty() || state.events.len() >= MAX_FINAL_KIR_SEMANTIC_EVENTS_V1 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    state.events.push(SemanticEventV1 { fields });
    Ok(())
}

fn allocate_private_memory_root(
    state: &mut ExecutionStateV1,
    element: ScalarV1,
    elements: u64,
    alignment: u32,
) -> Result<u32, FinalKirOutputEquivalenceErrorV1> {
    if elements == 0 || elements > 64 || alignment == 0 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    let ordinal = state.next_private_allocation;
    state.next_private_allocation = ordinal
        .checked_add(1)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    let memory_root = PRIVATE_MEMORY_ROOT_BASE_V1
        .checked_add(ordinal)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if state
        .memories
        .insert(
            memory_root,
            MemoryVersionV1 {
                element: Some(element),
                initial_parameter: None,
                cross_lane_width: None,
                bounded_elements: Some(elements),
                writes: Vec::new(),
            },
        )
        .is_some()
    {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    Ok(memory_root)
}

fn allocate_workgroup_memory_root(
    state: &mut ExecutionStateV1,
    lane_widths: &mut BTreeSet<u32>,
    element: Option<ScalarV1>,
    element_width: u16,
    elements: u64,
    alignment: u32,
    workgroup_width: Option<u32>,
) -> Result<u32, FinalKirOutputEquivalenceErrorV1> {
    let width = workgroup_width
        .filter(|width| *width != 0 && *width <= MAX_FINAL_KIR_WORKGROUP_INVOCATIONS_V1)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if elements == 0 || elements > u64::from(u32::MAX) || alignment == 0 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    let ordinal = state.next_workgroup_allocation;
    state.next_workgroup_allocation = ordinal
        .checked_add(1)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    let memory_root = WORKGROUP_MEMORY_ROOT_BASE_V1
        .checked_add(ordinal)
        .ok_or(FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    if state
        .memories
        .insert(
            memory_root,
            MemoryVersionV1 {
                element,
                initial_parameter: None,
                cross_lane_width: Some(width),
                bounded_elements: Some(elements),
                writes: Vec::new(),
            },
        )
        .is_some()
    {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    lane_widths.insert(width);
    record_event(
        state,
        vec![
            EventFieldV1::Constant(120),
            EventFieldV1::Constant(i128::from(ordinal)),
            EventFieldV1::Constant(i128::from(element_width)),
            EventFieldV1::Constant(i128::from(elements)),
            EventFieldV1::Constant(i128::from(alignment)),
        ],
    )?;
    Ok(memory_root)
}

fn execution_layout_width(
    layout: fe2o3_kernel_ir::ExecutionElementLayoutV1,
) -> Result<u16, FinalKirOutputEquivalenceErrorV1> {
    layout
        .byte_size
        .checked_mul(8)
        .and_then(|width| u16::try_from(width).ok())
        .filter(|_| layout.is_complete())
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
}

fn require_workgroup_view(
    view: &WorkgroupViewV1,
    layout: fe2o3_kernel_ir::ExecutionElementLayoutV1,
    elements: u64,
    published: bool,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if view.layout != layout || view.elements != elements || view.published != published {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics);
    }
    Ok(())
}

fn index_in_bounds(
    index: ExpressionV1,
    elements: u64,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    if index.scalar != ScalarV1::Unsigned(64) || elements == 0 {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let limit = ExpressionV1::leaf(
        ScalarV1::Unsigned(64),
        ExpressionKindV1::Constant(i128::from(elements)),
    );
    less_than_expression(index, limit)
}

fn less_than_expression(
    lhs: ExpressionV1,
    rhs: ExpressionV1,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    if lhs.scalar != rhs.scalar || lhs.scalar.is_float() {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let depth = bounded_depth([lhs.depth, rhs.depth])?;
    Ok(ExpressionV1 {
        scalar: ScalarV1::Bool,
        kind: ExpressionKindV1::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        },
        depth,
    })
}

fn select_expression(
    condition: ExpressionV1,
    when_true: ExpressionV1,
    when_false: ExpressionV1,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    if condition.scalar != ScalarV1::Bool || when_true.scalar != when_false.scalar {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let scalar = when_true.scalar;
    let depth = bounded_depth([condition.depth, when_true.depth, when_false.depth])?;
    Ok(ExpressionV1 {
        scalar,
        kind: ExpressionKindV1::Select {
            condition: Box::new(condition),
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
        },
        depth,
    })
}

fn record_workgroup_store(
    state: &mut ExecutionStateV1,
    view: &WorkgroupViewV1,
    index: ExpressionV1,
    value: ExpressionV1,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    if view.published || execution_layout_width(view.layout)? != value.scalar.width() {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics);
    }
    let present = index_in_bounds(index.clone(), view.elements)?;
    let safe_index = select_expression(present.clone(), index, index_zero())?;
    record_store(
        state,
        PointerV1 {
            memory_parameter: view.memory_root,
            element: value.scalar,
            address_space: AddressSpace::Workgroup,
            access: AccessMode::WriteOnly,
            index: safe_index,
        },
        present.clone(),
        value,
        AddressSpace::Workgroup,
    )?;
    Ok(present)
}

fn workgroup_optional_load(
    operation: &Operation,
    state: &ExecutionStateV1,
    view: &WorkgroupViewV1,
    index: ExpressionV1,
) -> Result<Vec<SymbolicValueV1>, FinalKirOutputEquivalenceErrorV1> {
    let [value_result, present_result] = operation.results.as_slice() else {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    };
    let scalar = value_result
        .ty
        .as_scalar()
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
        .and_then(ScalarV1::from_ir)?;
    if present_result.ty != Type::BOOL || execution_layout_width(view.layout)? != scalar.width() {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let present = index_in_bounds(index.clone(), view.elements)?;
    let safe_index = select_expression(present.clone(), index, index_zero())?;
    let loaded = memory_read(
        state,
        PointerV1 {
            memory_parameter: view.memory_root,
            element: scalar,
            address_space: AddressSpace::Workgroup,
            access: AccessMode::ReadOnly,
            index: safe_index,
        },
    )?;
    let value = select_expression(
        present.clone(),
        loaded,
        ExpressionV1::leaf(scalar, ExpressionKindV1::Constant(0)),
    )?;
    Ok(vec![
        SymbolicValueV1::Scalar(value),
        SymbolicValueV1::Scalar(present),
    ])
}

fn bounded_view_optional_load(
    operation: &Operation,
    state: &ExecutionStateV1,
    view: &BoundedMemoryViewV1,
    index: ExpressionV1,
) -> Result<Vec<SymbolicValueV1>, FinalKirOutputEquivalenceErrorV1> {
    let [value_result, present_result] = operation.results.as_slice() else {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    };
    let scalar = value_result
        .ty
        .as_scalar()
        .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType)
        .and_then(ScalarV1::from_ir)?;
    if present_result.ty != Type::BOOL
        || execution_layout_width(view.layout)? != scalar.width()
        || matches!(
            view.access,
            fe2o3_kernel_ir::ExecutionMemoryAccessV1::DisjointWrite
        )
    {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let present = less_than_expression(index.clone(), view.extent.clone())?;
    let safe_index = select_expression(present.clone(), index, index_zero())?;
    let pointer = offset_bounded_view_pointer(view, safe_index, scalar)?;
    let loaded = memory_read(state, pointer)?;
    let value = select_expression(
        present.clone(),
        loaded,
        ExpressionV1::leaf(scalar, ExpressionKindV1::Constant(0)),
    )?;
    Ok(vec![
        SymbolicValueV1::Scalar(value),
        SymbolicValueV1::Scalar(present),
    ])
}

fn record_bounded_view_store(
    state: &mut ExecutionStateV1,
    view: &BoundedMemoryViewV1,
    index: ExpressionV1,
    value: ExpressionV1,
) -> Result<ExpressionV1, FinalKirOutputEquivalenceErrorV1> {
    if execution_layout_width(view.layout)? != value.scalar.width()
        || matches!(
            view.access,
            fe2o3_kernel_ir::ExecutionMemoryAccessV1::ReadOnly
        )
    {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let present = less_than_expression(index.clone(), view.extent.clone())?;
    let safe_index = select_expression(present.clone(), index, index_zero())?;
    let pointer = offset_bounded_view_pointer(view, safe_index, value.scalar)?;
    record_store(
        state,
        pointer,
        present.clone(),
        value,
        view.pointer.address_space,
    )?;
    Ok(present)
}

fn offset_bounded_view_pointer(
    view: &BoundedMemoryViewV1,
    offset: ExpressionV1,
    element: ScalarV1,
) -> Result<PointerV1, FinalKirOutputEquivalenceErrorV1> {
    if view.pointer.element != element {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
    }
    let index = ExpressionV1::binary(
        ScalarV1::Unsigned(64),
        BinaryOp::Add,
        view.pointer.index.clone(),
        offset,
    )?;
    Ok(PointerV1 {
        index,
        access: view.access.access_mode(),
        ..view.pointer.clone()
    })
}

fn record_execution_synchronization_event(
    state: &mut ExecutionStateV1,
    tag: i128,
    semantics: fe2o3_kernel_ir::ExecutionMemorySemanticsV1,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    record_event(
        state,
        vec![
            EventFieldV1::Constant(tag),
            EventFieldV1::Constant(i128::from(semantics.scope.synchronization_scope().rank())),
            EventFieldV1::Constant(i128::from(memory_ordering_tag_v1(
                semantics.ordering.memory_ordering(),
            ))),
            EventFieldV1::Constant(i128::from(address_space_mask_v1(
                &semantics.spaces.address_spaces(),
            ))),
        ],
    )
}

fn record_workgroup_publication(
    state: &mut ExecutionStateV1,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    record_event(
        state,
        vec![
            EventFieldV1::Constant(102),
            EventFieldV1::Constant(i128::from(
                fe2o3_kernel_ir::SynchronizationScope::Workgroup.rank(),
            )),
            EventFieldV1::Constant(i128::from(memory_ordering_tag_v1(
                fe2o3_kernel_ir::MemoryOrdering::AcquireRelease,
            ))),
            EventFieldV1::Constant(i128::from(address_space_mask_v1(&BTreeSet::from([
                AddressSpace::Workgroup,
            ])))),
        ],
    )
}

fn memory_intrinsic_scalar(
    element: MemoryElementType,
) -> Result<ScalarV1, FinalKirOutputEquivalenceErrorV1> {
    let MemoryElementType::Scalar(scalar) = element else {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedScalarType);
    };
    ScalarV1::from_ir(scalar)
}

fn offset_pointer(
    mut pointer: PointerV1,
    offset: usize,
) -> Result<PointerV1, FinalKirOutputEquivalenceErrorV1> {
    let offset =
        i128::try_from(offset).map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    pointer.index = ExpressionV1::binary(
        ScalarV1::Unsigned(64),
        BinaryOp::Add,
        pointer.index,
        ExpressionV1::leaf(ScalarV1::Unsigned(64), ExpressionKindV1::Constant(offset)),
    )?;
    Ok(pointer)
}

fn memory_event_fields(
    tag: i128,
    pointer: &PointerV1,
    address_space: AddressSpace,
    layout: fe2o3_kernel_ir::MemoryLayout,
    value: Option<ExpressionV1>,
) -> Vec<EventFieldV1> {
    let mut fields = vec![
        EventFieldV1::Constant(tag),
        EventFieldV1::Constant(i128::from(pointer.memory_parameter)),
        EventFieldV1::Constant(i128::from(address_space_tag_v1(address_space))),
        EventFieldV1::Constant(i128::from(layout.size_bytes)),
        EventFieldV1::Constant(i128::from(layout.alignment_bytes)),
        EventFieldV1::Expression(pointer.index.clone()),
    ];
    fields.extend(value.map(EventFieldV1::Expression));
    fields
}

const fn legacy_alias_contract_tag(role: GlobalCapabilityRoleV1) -> u8 {
    match role {
        GlobalCapabilityRoleV1::ReadOnly => 1,
        GlobalCapabilityRoleV1::ExclusiveReadWrite => 2,
        GlobalCapabilityRoleV1::DisjointWrite(_) => 3,
    }
}

const fn execution_alias_contract_tag(access: fe2o3_kernel_ir::ExecutionMemoryAccessV1) -> u8 {
    match access {
        fe2o3_kernel_ir::ExecutionMemoryAccessV1::ReadOnly => 1,
        fe2o3_kernel_ir::ExecutionMemoryAccessV1::ExclusiveReadWrite => 2,
        fe2o3_kernel_ir::ExecutionMemoryAccessV1::DisjointWrite => 3,
        fe2o3_kernel_ir::ExecutionMemoryAccessV1::AtomicReadWrite => 4,
    }
}

fn require_alias_discipline(
    outcomes: &[ReturnOutcomeV1],
    capability_roots: &BTreeMap<u32, u8>,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let memory_roots = outcomes
        .first()
        .map(|item| {
            item.memories
                .iter()
                .filter_map(|(root, memory)| memory.initial_parameter.map(|_| *root))
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    if memory_roots.len() > 1
        && !memory_roots
            .iter()
            .all(|root| capability_roots.contains_key(root))
    {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
    }
    for outcome in outcomes {
        for root in &outcome.written_roots {
            let writes = &outcome
                .memories
                .get(root)
                .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?
                .writes;
            if writes.len() < 2 || capability_roots.contains_key(root) {
                continue;
            }
            for (index, lhs) in writes.iter().enumerate() {
                for rhs in &writes[index + 1..] {
                    let Some(lhs) = evaluate_constant(&lhs.index) else {
                        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
                    };
                    let Some(rhs) = evaluate_constant(&rhs.index) else {
                        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
                    };
                    if lhs == rhs {
                        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing);
                    }
                }
            }
        }
    }
    Ok(())
}

fn index_zero() -> ExpressionV1 {
    ExpressionV1::leaf(ScalarV1::Unsigned(64), ExpressionKindV1::Constant(0))
}

fn bool_true() -> ExpressionV1 {
    ExpressionV1::leaf(ScalarV1::Bool, ExpressionKindV1::Constant(1))
}

fn render_kernel_lemma(
    source: &mut String,
    kernel_index: usize,
    reference: &KernelEffectsV1,
    actual: &KernelEffectsV1,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if reference.parameter_types != actual.parameter_types {
        return Err(FinalKirOutputEquivalenceErrorV1::KernelContractMismatch);
    }
    if reference.wave_widths != actual.wave_widths {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
    }
    let mut intrinsics = reference.intrinsics.clone();
    intrinsics.extend(&actual.intrinsics);
    let lane_lifted = !reference.wave_widths.is_empty();
    let lane = if lane_lifted { "lane" } else { "" };
    let mut parameters = if lane_lifted {
        vec!["lane: int".to_owned()]
    } else {
        Vec::new()
    };
    for (ordinal, ty) in reference.parameter_types.iter().enumerate() {
        match ty {
            Type::Scalar(_) if lane_lifted => {
                parameters.push(format!("p{ordinal}: spec_fn(int) -> int"));
            }
            Type::Scalar(_) => parameters.push(format!("p{ordinal}: int")),
            Type::Pointer(_) => parameters.push(format!("m{ordinal}: spec_fn(int) -> int")),
            Type::Slice(_) => {
                parameters.push(format!("m{ordinal}: spec_fn(int) -> int"));
                parameters.push(format!("len{ordinal}: int"));
            }
            Type::Unit
            | Type::KernelContext(_)
            | Type::GlobalCapability(_)
            | Type::ExecutionCapability(_) => {}
        }
    }
    parameters.extend(intrinsics.iter().map(|intrinsic| {
        if lane_lifted {
            format!("{}: spec_fn(int) -> int", intrinsic_name(*intrinsic))
        } else {
            format!("{}: int", intrinsic_name(*intrinsic))
        }
    }));
    parameters.extend(
        reference
            .output_roots
            .iter()
            .enumerate()
            .map(|(index, _)| format!("output_query_{index}: int")),
    );
    if lane_lifted {
        writeln!(
            source,
            "    #[verifier::rlimit(50)]\n    proof fn fe2o3_final_kir_output_equivalence_{kernel_index}({})\n        requires 0 <= lane\n    {{",
            parameters.join(", ")
        )
        .map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    } else {
        writeln!(
            source,
            "    #[verifier::rlimit(50)]\n    proof fn fe2o3_final_kir_output_equivalence_{kernel_index}({}) {{",
            parameters.join(", ")
        )
        .map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    }
    for (output_index, memory) in reference.output_roots.iter().enumerate() {
        let query = normalize(&format!("output_query_{output_index}"), 64);
        let reference_value = render_final_memory(reference, *memory, &query, lane)?;
        let actual_value = render_final_memory(actual, *memory, &query, lane)?;
        writeln!(
            source,
            "        let reference_output_{output_index}: int = {reference_value};"
        )
        .and_then(|_| {
            writeln!(
                source,
                "        let actual_output_{output_index}: int = {actual_value};"
            )
        })
        .and_then(|_| {
            writeln!(
                source,
                "        assert(reference_output_{output_index} == actual_output_{output_index});"
            )
        })
        .map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    }
    let reference_events = render_final_event_trace(reference, lane)?;
    let actual_events = render_final_event_trace(actual, lane)?;
    writeln!(
        source,
        "        let reference_events: Seq<int> = {reference_events};\n        let actual_events: Seq<int> = {actual_events};\n        assert(reference_events == actual_events);"
    )
    .map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    source.push_str("    }\n\n");
    Ok(())
}

fn render_final_event_trace(
    effects: &KernelEffectsV1,
    lane: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let mut result = "seq![]".to_owned();
    for outcome in effects.outcomes.iter().rev() {
        let path = render_expression(&outcome.path, lane)?;
        let trace = render_event_trace(&outcome.events, lane)?;
        result = format!("if {path} == 1 {{ {trace} }} else {{ {result} }}");
    }
    Ok(result)
}

fn render_event_trace(
    events: &[SemanticEventV1],
    lane: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let mut fields = Vec::new();
    for event in events {
        fields.push("4096".to_owned());
        fields.push(event.fields.len().to_string());
        for field in &event.fields {
            fields.push(match field {
                EventFieldV1::Constant(value) => value.to_string(),
                EventFieldV1::Expression(value) => render_expression(value, lane)?,
            });
        }
    }
    Ok(format!("seq![{}]", fields.join(", ")))
}

fn render_final_memory(
    effects: &KernelEffectsV1,
    memory_parameter: u32,
    query: &str,
    lane: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let mut result = format!("m{memory_parameter}({query})");
    for outcome in effects.outcomes.iter().rev() {
        let memory = outcome
            .memories
            .get(&memory_parameter)
            .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?;
        let path = render_expression(&outcome.path, lane)?;
        let value = render_memory_version(memory_parameter, memory, query, lane)?;
        result = format!("if {path} == 1 {{ {value} }} else {{ {result} }}");
    }
    Ok(result)
}

fn render_memory_version(
    _memory_parameter: u32,
    memory: &MemoryVersionV1,
    index: &str,
    lane: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let element = memory
        .element
        .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?;
    let mut result = if let Some(parameter) = memory.initial_parameter {
        normalize(&format!("m{parameter}({index})"), element.width())
    } else {
        "0".to_owned()
    };
    for write in &memory.writes {
        let writer_lanes = memory.cross_lane_width.map_or_else(
            || vec![lane.to_owned()],
            |width| (0..width).map(|lane| lane.to_string()).collect(),
        );
        for writer_lane in writer_lanes {
            let write_index = render_expression(&write.index, &writer_lane)?;
            let guard = render_expression(&write.guard, &writer_lane)?;
            let value = render_expression(&write.value, &writer_lane)?;
            result = format!(
                "if ({guard} == 1) && (({index}) == ({write_index})) {{ {value} }} else {{ {result} }}"
            );
        }
    }
    Ok(result)
}

fn render_expression(
    expression: &ExpressionV1,
    lane: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let width = expression.scalar.width();
    let rendered = match &expression.kind {
        ExpressionKindV1::Parameter(ordinal) if lane.is_empty() => {
            normalize(&format!("p{ordinal}"), width)
        }
        ExpressionKindV1::Parameter(ordinal) => normalize(&format!("p{ordinal}({lane})"), width),
        ExpressionKindV1::Intrinsic(IntrinsicKind::InvocationIndex {
            kind: fe2o3_kernel_ir::IndexKind::Local,
            axis: fe2o3_kernel_ir::Axis::X,
        }) if !lane.is_empty() => normalize(lane, width),
        ExpressionKindV1::Intrinsic(intrinsic) if lane.is_empty() => {
            normalize(intrinsic_name(*intrinsic), width)
        }
        ExpressionKindV1::Intrinsic(intrinsic) => {
            normalize(&format!("{}({lane})", intrinsic_name(*intrinsic)), width)
        }
        ExpressionKindV1::SliceLength(ordinal) => normalize(&format!("len{ordinal}"), width),
        ExpressionKindV1::Constant(value) => normalize(&value.to_string(), width),
        ExpressionKindV1::Unary { operation, operand } => {
            let operand = render_expression(operand, lane)?;
            match (operation, expression.scalar) {
                (UnaryOp::Not, ScalarV1::Bool) => format!("if {operand} == 0 {{ 1 }} else {{ 0 }}"),
                (UnaryOp::Not, ScalarV1::Signed(_) | ScalarV1::Unsigned(_)) => {
                    format!("(fe2o3_bv_modulus_v2({width}) - 1) - ({operand})")
                }
                (UnaryOp::Negate, ScalarV1::Signed(_) | ScalarV1::Unsigned(_)) => {
                    normalize(&format!("-({operand})"), width)
                }
                (UnaryOp::Negate, ScalarV1::Float(_)) => {
                    let sign = format!("fe2o3_bv_modulus_v2({})", width - 1);
                    format!(
                        "if {operand} < {sign} {{ {operand} + {sign} }} else {{ {operand} - {sign} }}"
                    )
                }
                _ => return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation),
            }
        }
        ExpressionKindV1::Binary {
            operation,
            lhs,
            rhs,
        } => {
            let lhs = render_expression(lhs, lane)?;
            let rhs = render_expression(rhs, lane)?;
            if expression.scalar.is_float() {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
            }
            render_integer_binary(*operation, expression.scalar, &lhs, &rhs)?
        }
        ExpressionKindV1::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            let lhs_rendered = render_expression(lhs, lane)?;
            let rhs_rendered = render_expression(rhs, lane)?;
            if lhs.scalar.is_float() {
                let ScalarV1::Float(scalar) = lhs.scalar else {
                    unreachable!("floating scalar checked above")
                };
                let (exponent_bits, fraction_bits) = float_layout_v1(scalar)
                    .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy)?;
                format!(
                    "fe2o3_ieee_compare_v1({}, {}, {}, {lhs_rendered}, {rhs_rendered})",
                    compare_tag(*predicate),
                    exponent_bits,
                    fraction_bits,
                )
            } else {
                let lhs = comparison_operand(lhs.scalar, &lhs_rendered);
                let rhs = comparison_operand(rhs.scalar, &rhs_rendered);
                let operator = match predicate {
                    ComparePredicate::Equal => "==",
                    ComparePredicate::NotEqual => "!=",
                    ComparePredicate::LessThan => "<",
                    ComparePredicate::LessThanOrEqual => "<=",
                    ComparePredicate::GreaterThan => ">",
                    ComparePredicate::GreaterThanOrEqual => ">=",
                };
                format!("if ({lhs}) {operator} ({rhs}) {{ 1 }} else {{ 0 }}")
            }
        }
        ExpressionKindV1::Cast { kind, operand } => {
            let rendered = render_expression(operand, lane)?;
            render_cast(*kind, operand.scalar, expression.scalar, &rendered)?
        }
        ExpressionKindV1::Select {
            condition,
            when_true,
            when_false,
        } => format!(
            "if {} == 1 {{ {} }} else {{ {} }}",
            render_expression(condition, lane)?,
            render_expression(when_true, lane)?,
            render_expression(when_false, lane)?,
        ),
        ExpressionKindV1::MemoryRead {
            memory_parameter,
            initial_parameter,
            cross_lane_width,
            bounded_elements,
            writes,
            index,
        } => {
            let memory = MemoryVersionV1 {
                element: Some(expression.scalar),
                initial_parameter: *initial_parameter,
                cross_lane_width: *cross_lane_width,
                bounded_elements: *bounded_elements,
                writes: writes.clone(),
            };
            render_memory_version(
                *memory_parameter,
                &memory,
                &render_expression(index, lane)?,
                lane,
            )?
        }
        ExpressionKindV1::WaveLaneId { width: wave_width } => {
            if lane.is_empty() {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
            }
            normalize(&format!("({lane}) % {wave_width}"), width)
        }
        ExpressionKindV1::WaveBallot {
            predicate,
            width: wave_width,
        } => {
            let lanes = (0..*wave_width)
                .map(|wave_lane| {
                    Ok(format!(
                        "(if {} != 0 {{ {} }} else {{ 0 }})",
                        render_expression(predicate, &wave_lane.to_string())?,
                        1_u128 << wave_lane,
                    ))
                })
                .collect::<Result<Vec<_>, FinalKirOutputEquivalenceErrorV1>>()?;
            normalize(&lanes.join(" + "), width)
        }
        ExpressionKindV1::WaveAny {
            predicate,
            width: wave_width,
        } => {
            let predicates = render_wave_predicates(predicate, *wave_width)?;
            format!("if {} {{ 1 }} else {{ 0 }}", predicates.join(" || "))
        }
        ExpressionKindV1::WaveAll {
            predicate,
            width: wave_width,
        } => {
            let predicates = render_wave_predicates(predicate, *wave_width)?;
            format!("if {} {{ 1 }} else {{ 0 }}", predicates.join(" && "))
        }
        ExpressionKindV1::WaveShuffleIndex {
            value,
            source_lane,
            tile_width,
        }
        | ExpressionKindV1::WaveBroadcast {
            value,
            source_lane,
            tile_width,
        } => {
            let source_lane = render_expression(source_lane, lane)?;
            let selected_lane = format!(
                "((({lane}) / {tile_width}) * {tile_width}) + (({source_lane}) % {tile_width})"
            );
            render_expression(value, &selected_lane)?
        }
        ExpressionKindV1::WaveReduceMaxF32 { value, tile_width } => {
            if lane.is_empty() {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
            }
            let tile_base = format!("((({lane}) / {tile_width}) * {tile_width})");
            let inputs = (0..*tile_width)
                .map(|offset| render_expression(value, &format!("{tile_base} + {offset}")))
                .collect::<Result<Vec<_>, _>>()?;
            let outputs = render_wave_reduce_max_f32_v1(inputs)
                .ok_or(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics)?;
            let local_lane = format!("({lane}) % {tile_width}");
            outputs.into_iter().enumerate().rev().fold(
                "0".to_owned(),
                |selected, (index, output)| {
                    format!("if {local_lane} == {index} {{ {output} }} else {{ {selected} }}")
                },
            )
        }
        ExpressionKindV1::SubgroupSum {
            value,
            width: subgroup_width,
            kind,
        } => {
            if lane.is_empty() {
                return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
            }
            let base = format!("((({lane}) / {subgroup_width}) * {subgroup_width})");
            let local_lane = format!("({lane}) % {subgroup_width}");
            let terms = (0..*subgroup_width)
                .map(|offset| {
                    let value = render_expression(value, &format!("{base} + {offset}"))?;
                    Ok(match kind {
                        SubgroupSumKindV1::Reduce => value,
                        SubgroupSumKindV1::InclusiveScan => {
                            format!("if {local_lane} >= {offset} {{ {value} }} else {{ 0 }}")
                        }
                        SubgroupSumKindV1::ExclusiveScan => {
                            format!("if {local_lane} > {offset} {{ {value} }} else {{ 0 }}")
                        }
                    })
                })
                .collect::<Result<Vec<_>, FinalKirOutputEquivalenceErrorV1>>()?;
            normalize(&terms.join(" + "), width)
        }
        ExpressionKindV1::FloatAbs { operand } => {
            let operand = render_expression(operand, lane)?;
            format!(
                "({operand}) % fe2o3_bv_modulus_v2({})",
                expression.scalar.width() - 1,
            )
        }
    };
    Ok(rendered)
}

fn render_wave_predicates(
    predicate: &ExpressionV1,
    width: u32,
) -> Result<Vec<String>, FinalKirOutputEquivalenceErrorV1> {
    (0..width)
        .map(|wave_lane| {
            render_expression(predicate, &wave_lane.to_string())
                .map(|value| format!("{value} != 0"))
        })
        .collect()
}

fn normalize(value: &str, width: u16) -> String {
    format!("fe2o3_bv_norm_v2(({value}), {width})")
}

fn comparison_operand(scalar: ScalarV1, value: &str) -> String {
    if scalar.is_signed() {
        format!("fe2o3_bv_signed_v2({value}, {})", scalar.width())
    } else {
        value.to_owned()
    }
}

fn render_integer_binary(
    operation: BinaryOp,
    scalar: ScalarV1,
    lhs: &str,
    rhs: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let width = scalar.width();
    Ok(match operation {
        BinaryOp::Add => normalize(&format!("({lhs}) + ({rhs})"), width),
        BinaryOp::Subtract => normalize(&format!("({lhs}) - ({rhs})"), width),
        BinaryOp::Multiply => normalize(&format!("({lhs}) * ({rhs})"), width),
        BinaryOp::BitAnd => format!("fe2o3_bitwise_v2(1, {lhs}, {rhs}, {width})"),
        BinaryOp::BitOr => format!("fe2o3_bitwise_v2(2, {lhs}, {rhs}, {width})"),
        BinaryOp::BitXor => format!("fe2o3_bitwise_v2(0, {lhs}, {rhs}, {width})"),
        BinaryOp::ShiftLeft => {
            format!("fe2o3_shift_left_v2({lhs}, ({rhs}) as nat, {width})")
        }
        BinaryOp::ShiftRight => format!(
            "fe2o3_shift_right_v2({lhs}, ({rhs}) as nat, {width}, {})",
            scalar.is_signed(),
        ),
        BinaryOp::Divide | BinaryOp::Remainder | BinaryOp::Checked(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
        }
    })
}

fn render_cast(
    kind: CastKind,
    from: ScalarV1,
    to: ScalarV1,
    value: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    Ok(match (kind, from, to) {
        (CastKind::Bitcast, _, _) if from.width() == to.width() => value.to_owned(),
        (CastKind::Truncate | CastKind::Bitcast, _, _) => normalize(value, to.width()),
        (CastKind::ZeroExtend, _, _) => normalize(&normalize(value, from.width()), to.width()),
        (CastKind::SignExtend, _, _) => normalize(
            &format!("fe2o3_bv_signed_v2({value}, {})", from.width()),
            to.width(),
        ),
        _ => return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy),
    })
}

const fn compare_tag(predicate: ComparePredicate) -> u8 {
    match predicate {
        ComparePredicate::Equal => 0,
        ComparePredicate::NotEqual => 1,
        ComparePredicate::LessThan => 2,
        ComparePredicate::LessThanOrEqual => 3,
        ComparePredicate::GreaterThan => 4,
        ComparePredicate::GreaterThanOrEqual => 5,
    }
}

fn intrinsic_name(intrinsic: IntrinsicKind) -> &'static str {
    use fe2o3_kernel_ir::{Axis, IndexKind};
    match intrinsic {
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Global,
            axis: Axis::X,
        } => "global_x",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Global,
            axis: Axis::Y,
        } => "global_y",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Global,
            axis: Axis::Z,
        } => "global_z",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Workgroup,
            axis: Axis::X,
        } => "group_x",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Workgroup,
            axis: Axis::Y,
        } => "group_y",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Workgroup,
            axis: Axis::Z,
        } => "group_z",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Local,
            axis: Axis::X,
        } => "local_x",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Local,
            axis: Axis::Y,
        } => "local_y",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Local,
            axis: Axis::Z,
        } => "local_z",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::WorkgroupSize,
            axis: Axis::X,
        } => "group_size_x",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::WorkgroupSize,
            axis: Axis::Y,
        } => "group_size_y",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::WorkgroupSize,
            axis: Axis::Z,
        } => "group_size_z",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::WorkgroupCount,
            axis: Axis::X,
        } => "group_count_x",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::WorkgroupCount,
            axis: Axis::Y,
        } => "group_count_y",
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::WorkgroupCount,
            axis: Axis::Z,
        } => "group_count_z",
        IntrinsicKind::LaunchExtent { axis: Axis::X } => "extent_x",
        IntrinsicKind::LaunchExtent { axis: Axis::Y } => "extent_y",
        IntrinsicKind::LaunchExtent { axis: Axis::Z } => "extent_z",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BinaryOp, Constant, Function, Kernel, LaunchDomain,
        LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
        Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13, WaveOperation,
        WaveWidth, WorkgroupSize,
    };

    use super::*;

    fn kernel_module(addend: Option<u32>) -> Module {
        let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
        let stored = if let Some(addend) = addend {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(addend)),
            ));
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(0),
                    rhs: ValueId(2),
                },
            ));
            ValueId(3)
        } else {
            ValueId(0)
        };
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(1),
                value: stored,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });

        let signature = Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::WriteOnly,
                ),
            ],
            Vec::new(),
        );
        let mut module = Module::new("direct-final-kir-proof");
        module.functions.push(Function::kernel_entry(
            "entry",
            signature,
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        module
    }

    fn recurrence_memory_module(
        subtract_transition: bool,
        preserving_wrapper: bool,
        second_offset: u64,
    ) -> Module {
        let scalar = Type::Scalar(ScalarType::U32);
        let index = Type::Scalar(ScalarType::Index);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let effect = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);

        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.extend([
            effect(
                2,
                index.clone(),
                OperationKind::Constant(Constant::Index(0)),
            ),
            effect(
                3,
                index.clone(),
                OperationKind::Constant(Constant::Index(4)),
            ),
        ]);
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(2), ValueId(0)],
        });

        let mut header = BasicBlock::new(BlockId(1));
        header.parameters = vec![
            ValueDef::new(ValueId(10), index.clone()),
            ValueDef::new(ValueId(11), scalar.clone()),
        ];
        header.operations.push(effect(
            12,
            Type::Scalar(ScalarType::Bool),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(10),
                rhs: ValueId(3),
            },
        ));
        header.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(12),
            then_target: BlockId(2),
            then_arguments: Vec::new(),
            else_target: BlockId(3),
            else_arguments: vec![ValueId(11)],
        });

        let mut body = BasicBlock::new(BlockId(2));
        body.operations.extend([
            effect(
                13,
                index.clone(),
                OperationKind::Constant(Constant::Index(1)),
            ),
            effect(
                14,
                index.clone(),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(10),
                    rhs: ValueId(13),
                },
            ),
            effect(
                15,
                scalar.clone(),
                OperationKind::Binary {
                    op: if subtract_transition {
                        BinaryOp::Subtract
                    } else {
                        BinaryOp::Add
                    },
                    lhs: ValueId(11),
                    rhs: ValueId(0),
                },
            ),
        ]);
        let next_accumulator = if preserving_wrapper {
            body.operations.extend([
                effect(
                    16,
                    scalar.clone(),
                    OperationKind::Constant(Constant::U32(0)),
                ),
                effect(
                    17,
                    scalar.clone(),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(15),
                        rhs: ValueId(16),
                    },
                ),
            ]);
            ValueId(17)
        } else {
            ValueId(15)
        };
        body.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(14), next_accumulator],
        });

        let mut exit = BasicBlock::new(BlockId(3));
        exit.parameters = vec![ValueDef::new(ValueId(20), scalar.clone())];
        exit.operations.extend([
            Operation::new(
                Vec::new(),
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(20),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            effect(
                21,
                scalar.clone(),
                OperationKind::Load {
                    pointer: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            effect(
                22,
                index.clone(),
                OperationKind::Constant(Constant::Index(second_offset)),
            ),
            effect(
                23,
                pointer.clone(),
                OperationKind::GetElementPointer {
                    base: ValueId(1),
                    offset: ValueId(22),
                },
            ),
            effect(
                24,
                scalar.clone(),
                OperationKind::Constant(Constant::U32(1)),
            ),
            effect(
                25,
                scalar.clone(),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(21),
                    rhs: ValueId(24),
                },
            ),
            Operation::new(
                Vec::new(),
                OperationKind::Store {
                    pointer: ValueId(23),
                    value: ValueId(25),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ]);
        exit.terminator = Some(Terminator::Return { values: Vec::new() });

        let signature = Signature::new(vec![scalar, pointer], Vec::new());
        let mut module = Module::new("bounded-cfg-memory-proof");
        module.functions.push(Function::kernel_entry(
            "entry",
            signature,
            vec![ValueId(0), ValueId(1)],
            vec![entry, header, body, exit],
        ));
        module.kernels.push(Kernel::new(
            "bounded_recurrence",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        module
    }

    fn strict_float_module(operation: BinaryOp, mode: NumericalModeV1) -> Module {
        let scalar = Type::Scalar(ScalarType::F32);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(2), scalar.clone()),
                OperationKind::Constant(Constant::F32Bits(0)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), scalar.clone()),
                OperationKind::Binary {
                    op: operation,
                    lhs: ValueId(0),
                    rhs: ValueId(2),
                },
            ),
            Operation::new(
                Vec::new(),
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ]);
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new("strict-float-operator-congruence");
        module
            .required_capabilities
            .insert(TargetCapability::Execution(
                ExecutionCapabilityRequirementV1::Numerical {
                    value_type: ScalarType::F32,
                    mode,
                },
            ));
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![scalar, pointer], Vec::new()),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "strict_float",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        module
    }

    fn strict_float_compare_module(predicate: ComparePredicate) -> Module {
        let float = Type::Scalar(ScalarType::F32);
        let output = Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::WriteOnly);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(3), Type::BOOL),
                OperationKind::Compare {
                    predicate,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ),
            Operation::new(
                Vec::new(),
                OperationKind::Store {
                    pointer: ValueId(2),
                    value: ValueId(3),
                    access: MemoryAccess::new(AddressSpace::Global, 1),
                },
            ),
        ]);
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new("strict-ieee-comparison");
        module
            .required_capabilities
            .insert(TargetCapability::Execution(
                ExecutionCapabilityRequirementV1::Numerical {
                    value_type: ScalarType::F32,
                    mode: NumericalModeV1::StrictIeee,
                },
            ));
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![float.clone(), float, output], Vec::new()),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "strict_float_compare",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        module
    }

    fn subgroup_shuffle_module(source_lane: u32, preserving_wrapper: bool) -> Module {
        let scalar = Type::Scalar(ScalarType::I32);
        let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(source_lane)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), scalar.clone()),
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::ShuffleIndex {
                        value: ValueId(0),
                        source_lane: ValueId(2),
                        tile_width: 32,
                    },
                    WaveWidth::Wave64,
                )),
            ),
        ]);
        let stored = if preserving_wrapper {
            block.operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(4), scalar.clone()),
                    OperationKind::Constant(Constant::I32(0)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(5), scalar.clone()),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(3),
                        rhs: ValueId(4),
                    },
                ),
            ]);
            ValueId(5)
        } else {
            ValueId(3)
        };
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(1),
                value: stored,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });

        let mut function = Function::kernel_entry(
            "subgroup_shuffle_entry",
            Signature::new(vec![scalar, output], Vec::new()),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        );
        function.required_capabilities = function.derived_capabilities();
        let mut kernel = Kernel::new(
            "subgroup_shuffle",
            "subgroup_shuffle_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new("subgroup-shuffle-proof");
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    fn subgroup_predicate_module(all: bool, preserving_wrapper: bool) -> Module {
        let output = Type::pointer(Type::BOOL, AddressSpace::Global, AccessMode::WriteOnly);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(2), Type::BOOL),
            OperationKind::Wave(WaveOperation::full(
                if all {
                    WaveOperationKind::All {
                        predicate: ValueId(0),
                    }
                } else {
                    WaveOperationKind::Any {
                        predicate: ValueId(0),
                    }
                },
                WaveWidth::Wave32,
            )),
        ));
        let stored = if preserving_wrapper {
            block.operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(3), Type::BOOL),
                    OperationKind::Constant(Constant::Bool(false)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(4), Type::BOOL),
                    OperationKind::Binary {
                        op: BinaryOp::BitOr,
                        lhs: ValueId(2),
                        rhs: ValueId(3),
                    },
                ),
            ]);
            ValueId(4)
        } else {
            ValueId(2)
        };
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(1),
                value: stored,
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });

        let mut function = Function::kernel_entry(
            "subgroup_predicate_entry",
            Signature::new(vec![Type::BOOL, output], Vec::new()),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        );
        function.required_capabilities = function.derived_capabilities();
        let mut kernel = Kernel::new(
            "subgroup_predicate",
            "subgroup_predicate_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
        let mut module = Module::new("subgroup-predicate-proof");
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    fn subgroup_reduce_max_module(broadcast_instead: bool, preserving_wrapper: bool) -> Module {
        let scalar = Type::Scalar(ScalarType::F32);
        let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(0)),
        ));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            OperationKind::Wave(WaveOperation::full(
                if broadcast_instead {
                    WaveOperationKind::BroadcastF32 {
                        value: ValueId(0),
                        source_lane: ValueId(2),
                        tile_width: 4,
                    }
                } else {
                    WaveOperationKind::ReduceF32 {
                        value: ValueId(0),
                        tile_width: 4,
                        kind: WaveF32ReductionKindV1::Maximum,
                    }
                },
                WaveWidth::Wave32,
            )),
        ));
        let stored = if preserving_wrapper {
            block.operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        value: ValueId(3),
                        to: Type::Scalar(ScalarType::U32),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(5), scalar.clone()),
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        value: ValueId(4),
                        to: scalar.clone(),
                    },
                ),
            ]);
            ValueId(5)
        } else {
            ValueId(3)
        };
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(1),
                value: stored,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });

        let mut function = Function::kernel_entry(
            "subgroup_reduce_max_entry",
            Signature::new(vec![scalar, output], Vec::new()),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        );
        function.required_capabilities = function.derived_capabilities();
        let mut kernel = Kernel::new(
            "subgroup_reduce_max",
            "subgroup_reduce_max_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
        let mut module = Module::new("subgroup-reduce-max-proof");
        module
            .required_capabilities
            .insert(TargetCapability::Execution(
                ExecutionCapabilityRequirementV1::Numerical {
                    value_type: ScalarType::F32,
                    mode: NumericalModeV1::StrictIeee,
                },
            ));
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    fn execution_identity(byte: u8) -> fe2o3_kernel_ir::ExecutionTypeIdentityV1 {
        fe2o3_kernel_ir::ExecutionTypeIdentityV1::new([byte; 32])
    }

    fn execution_provenance() -> fe2o3_kernel_ir::ExecutionCapabilityProvenanceV1 {
        fe2o3_kernel_ir::ExecutionCapabilityProvenanceV1 {
            root: fe2o3_kernel_ir::FunctionId::new("subgroup_scan_entry"),
            kernel_binding: [0x61; 32],
            frontend_unit: [0x62; 32],
            kernel_marker: [0x63; 32],
            target_brand: [0x64; 32],
            launch_brand: [0x65; 32],
            issuance: [0x66; 32],
        }
    }

    fn execution_capability_type(
        source_type: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
        role: fe2o3_kernel_ir::ExecutionCapabilityRoleV1,
    ) -> Type {
        Type::ExecutionCapability(fe2o3_kernel_ir::ExecutionCapabilityTypeV1 {
            source_type,
            provenance: execution_provenance(),
            workgroup_brand: Some([0x71; 32]),
            epoch: Some([0x72; 32]),
            role,
        })
    }

    fn execution_operation(
        result: ValueDef,
        operation: ExecutionCapabilityOperationV1,
        operands: Vec<ValueId>,
        signature_arguments: &[fe2o3_kernel_ir::ExecutionTypeIdentityV1],
        signature_output: fe2o3_kernel_ir::ExecutionTypeIdentityV1,
        source_ordinal: u8,
    ) -> Operation {
        Operation::effect_free(
            result,
            OperationKind::ExecutionCapability(fe2o3_kernel_ir::ExecutionCapabilityOpV1 {
                operands,
                signature: fe2o3_kernel_ir::ExecutionCapabilitySignatureV1::new(
                    signature_arguments,
                    signature_output,
                )
                .unwrap(),
                provenance: execution_provenance(),
                workgroup_brand: Some([0x71; 32]),
                epoch_before: Some([0x72; 32]),
                epoch_after: None,
                obligations: fe2o3_kernel_ir::ExecutionSafetyObligationsV1::from_bits(
                    fe2o3_kernel_ir::required_execution_obligations_v1(&operation),
                ),
                source: fe2o3_kernel_ir::ExecutionCapabilitySourceV1 {
                    function: [0x67; 32],
                    operation: [source_ordinal; 32],
                    block: 0,
                },
                operation,
            }),
        )
    }

    fn subgroup_scan_module(kind: ExecutionCollectiveKindV1, preserving_wrapper: bool) -> Module {
        let context_id = execution_identity(0x10);
        let workgroup_id = execution_identity(0x11);
        let subgroup_id = execution_identity(0x12);
        let subgroup_reference = execution_identity(0x2a);
        let epoch_id = execution_identity(0x2b);
        let element_id = execution_identity(0x15);
        let context_type = fe2o3_kernel_ir::KernelContextTypeV1::new(
            "subgroup_scan_entry",
            execution_provenance().kernel_marker,
            execution_provenance().target_brand,
            execution_provenance().launch_brand,
        );
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::kernel_context_issue(
            ValueId(2),
            context_type,
            fe2o3_kernel_ir::KernelContextSourceIdentityV1::new(
                [0x81; 32], [0x82; 32], [0x83; 32], [0x84; 32],
            ),
        ));
        let workgroup = ExecutionCapabilityOperationV1::WorkgroupDerive {
            context: context_id,
            workgroup: workgroup_id,
        };
        block.operations.push(execution_operation(
            ValueDef::new(
                ValueId(3),
                execution_capability_type(
                    workgroup_id,
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::Workgroup,
                ),
            ),
            workgroup,
            vec![ValueId(2)],
            &[context_id],
            workgroup_id,
            1,
        ));
        let subgroup = ExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup: workgroup_id,
            subgroup: subgroup_id,
            width: 32,
        };
        block.operations.push(execution_operation(
            ValueDef::new(
                ValueId(4),
                execution_capability_type(
                    subgroup_id,
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::Subgroup { width: 32 },
                ),
            ),
            subgroup,
            vec![ValueId(3)],
            &[workgroup_id],
            subgroup_id,
            2,
        ));
        let collective = ExecutionCapabilityOperationV1::SubgroupCollective {
            kind,
            subgroup_reference,
            subgroup: subgroup_id,
            epoch: epoch_id,
            element: element_id,
            value_type: ScalarType::U32,
            width: 32,
        };
        block.operations.push(execution_operation(
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
            collective,
            vec![ValueId(4), ValueId(0)],
            &[subgroup_reference, epoch_id, element_id],
            element_id,
            3,
        ));
        let stored = if preserving_wrapper {
            block.operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
                    OperationKind::Constant(Constant::U32(0)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(5),
                        rhs: ValueId(6),
                    },
                ),
            ]);
            ValueId(7)
        } else {
            ValueId(5)
        };
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(1),
                value: stored,
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });

        let output = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::WriteOnly,
        );
        let mut function = Function::kernel_entry(
            "subgroup_scan_entry",
            Signature::new(vec![Type::Scalar(ScalarType::U32), output], Vec::new()),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        );
        function.required_capabilities = function.derived_capabilities();
        let mut kernel = Kernel::new(
            "subgroup_scan",
            "subgroup_scan_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
        let mut module = Module::new("subgroup-scan-proof");
        module.required_capabilities = function.required_capabilities.clone();
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    fn fenced_kernel_module(
        ordering: fe2o3_kernel_ir::MemoryOrdering,
        preserving_wrapper: bool,
    ) -> Module {
        let mut module = kernel_module(preserving_wrapper.then_some(0));
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .insert(
                0,
                Operation::new(
                    Vec::new(),
                    OperationKind::Fence(fe2o3_kernel_ir::Fence {
                        memory_scope: fe2o3_kernel_ir::SynchronizationScope::Device,
                        semantics: fe2o3_kernel_ir::BarrierSemantics::new(
                            ordering,
                            [AddressSpace::Global],
                        ),
                    }),
                ),
            );
        module.functions[0].required_capabilities = module.functions[0].derived_capabilities();
        module
    }

    fn volatile_store_module(volatile: bool, preserving_wrapper: bool) -> Module {
        let scalar = Type::Scalar(ScalarType::U32);
        let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
        let mut block = BasicBlock::new(BlockId(0));
        let stored = if preserving_wrapper {
            block.operations.extend([
                Operation::effect_free(
                    ValueDef::new(ValueId(2), scalar.clone()),
                    OperationKind::Constant(Constant::U32(0)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(3), scalar.clone()),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(0),
                        rhs: ValueId(2),
                    },
                ),
            ]);
            ValueId(3)
        } else {
            ValueId(0)
        };
        block.operations.push(Operation::new(
            Vec::new(),
            if volatile {
                OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                    pointer: ValueId(1),
                    value: stored,
                    element: MemoryElementType::Scalar(ScalarType::U32),
                    address_space: AddressSpace::Global,
                    layout: fe2o3_kernel_ir::MemoryLayout::new(4, 4),
                    contract: fe2o3_kernel_ir::VolatileAccessContract::rust_allocation_store(),
                })
            } else {
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: stored,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                }
            },
        ));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut module = Module::new("volatile-store-proof");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![scalar, output], Vec::new()),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "volatile_store",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        module
    }

    fn static_lds_event_module(extent: u32, preserving_wrapper: bool) -> Module {
        let mut module = kernel_module(preserving_wrapper.then_some(0));
        module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .insert(
                0,
                Operation::new(
                    vec![ValueDef::new(
                        ValueId(100),
                        Type::pointer(
                            Type::Scalar(ScalarType::U32),
                            AddressSpace::Workgroup,
                            AccessMode::ReadWrite,
                        ),
                    )],
                    OperationKind::WorkgroupMemory(fe2o3_kernel_ir::WorkgroupMemory {
                        element: Type::Scalar(ScalarType::U32),
                        extent: fe2o3_kernel_ir::WorkgroupMemoryExtent::Static(extent),
                        alignment: 16,
                    }),
                ),
            );
        module.functions[0].required_capabilities = module.functions[0].derived_capabilities();
        module
    }

    fn pinned_rust_verify() -> PathBuf {
        std::env::var_os("FE2O3_PINNED_RUST_VERIFY")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("/home/harsh/.cache/fe2o3-verus-0.2026.08.02/verus-x86-linux/verus")
            })
    }

    fn run_verus(verus: &Path, source: &[u8], suffix: &str) -> std::process::Output {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-final-kir-{suffix}-{}-{nonce}.rs",
            std::process::id(),
        ));
        fs::write(&path, source).unwrap();
        let output = Command::new(verus).arg(&path).output().unwrap();
        fs::remove_file(path).unwrap();
        output
    }

    fn assert_verus_accepts(output: std::process::Output, context: &str) {
        assert!(
            output.status.success(),
            "{context} failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn assert_verus_rejects(output: std::process::Output, context: &str) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "{context} unexpectedly verified:\nstdout:\n{stdout}\nstderr:\n{stderr}",
        );
        assert!(
            stderr.contains("assertion failed") && !stderr.contains("Resource limit"),
            "{context} was not a semantic assertion rejection:\nstdout:\n{stdout}\nstderr:\n{stderr}",
        );
    }

    #[test]
    fn pinned_verus_accepts_semantics_preserving_mutation_and_rejects_changed_store() {
        let verus = pinned_rust_verify();
        assert!(
            verus.is_file(),
            "pinned rust_verify is missing at {}",
            verus.display()
        );

        let reference = kernel_module(None);
        let preserved = kernel_module(Some(0));
        let changed = kernel_module(Some(1));
        let reference_canonical =
            VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        let preserved_canonical =
            VerifiedCanonicalKernelIrV13::from_module(preserved.clone()).unwrap();
        let changed_canonical = VerifiedCanonicalKernelIrV13::from_module(changed.clone()).unwrap();
        assert_ne!(
            reference_canonical.identity(),
            preserved_canonical.identity()
        );
        assert_ne!(reference_canonical.identity(), changed_canonical.identity());

        let preserved_proof = generate_final_kir_output_equivalence_v1(&reference, &preserved)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, preserved_proof.source(), "preserved"),
            "preserving proof",
        );

        let changed_proof = generate_final_kir_output_equivalence_v1(&reference, &changed)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, changed_proof.source(), "changed"),
            "changed store",
        );
    }

    #[test]
    fn missing_output_is_rejected_before_proof_generation() {
        let source = kernel_module(None);
        let mut final_module = kernel_module(None);
        final_module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .clear();
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&source, &final_module).unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::MissingOutputWrite,
        );
    }

    #[test]
    fn pinned_verus_proves_bounded_phi_recurrence_and_raw_memory_versions() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");

        let reference = recurrence_memory_module(false, false, 1);
        let preserving = recurrence_memory_module(false, true, 1);
        let wrong_recurrence = recurrence_memory_module(true, true, 1);
        let reference_owner = VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        let preserving_owner =
            VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        let wrong_owner =
            VerifiedCanonicalKernelIrV13::from_module(wrong_recurrence.clone()).unwrap();
        assert_ne!(reference_owner.identity(), preserving_owner.identity());
        assert_ne!(reference_owner.identity(), wrong_owner.identity());

        let proof = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, proof.source(), "bounded-cfg-preserved"),
            "bounded CFG proof",
        );

        let hostile = generate_final_kir_output_equivalence_v1(&reference, &wrong_recurrence)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, hostile.source(), "bounded-cfg-wrong-recurrence"),
            "wrong loop-carried recurrence",
        );
    }

    #[test]
    fn loop_reexecution_does_not_allow_duplicate_static_definitions() {
        let reference = recurrence_memory_module(false, false, 1);
        let mut duplicate = reference.clone();
        let body = duplicate.functions[0].body.as_mut().unwrap();
        let definition = body.blocks[0].operations[0].clone();
        body.blocks[1].operations.insert(0, definition);
        assert!(VerifiedCanonicalKernelIrV13::from_module(duplicate.clone()).is_err());
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&reference, &duplicate).unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::MalformedValueFlow,
        );
    }

    #[test]
    fn pinned_verus_proves_lane_extensional_shuffle_and_rejects_source_lane_mutation() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");

        let reference = subgroup_shuffle_module(0, false);
        let preserving = subgroup_shuffle_module(0, true);
        let wrong_lane = subgroup_shuffle_module(1, true);
        VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(wrong_lane.clone()).unwrap();

        let positive = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, positive.source(), "subgroup-shuffle-preserved"),
            "subgroup shuffle proof",
        );

        let negative = generate_final_kir_output_equivalence_v1(&reference, &wrong_lane)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, negative.source(), "subgroup-shuffle-wrong-lane"),
            "source-lane mutation",
        );
    }

    #[test]
    fn pinned_verus_proves_any_all_semantics_and_rejects_collective_mutation() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = subgroup_predicate_module(false, false);
        let preserving = subgroup_predicate_module(false, true);
        let all_instead = subgroup_predicate_module(true, true);
        VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(all_instead.clone()).unwrap();

        let positive = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, positive.source(), "subgroup-any-preserved"),
            "subgroup any proof",
        );

        let negative = generate_final_kir_output_equivalence_v1(&reference, &all_instead)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, negative.source(), "subgroup-any-to-all"),
            "any-to-all mutation",
        );
    }

    #[test]
    fn pinned_verus_proves_butterfly_maximum_and_rejects_broadcast_mutation() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = subgroup_reduce_max_module(false, false);
        let preserving = subgroup_reduce_max_module(false, true);
        let broadcast = subgroup_reduce_max_module(true, true);
        VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(broadcast.clone()).unwrap();

        let positive = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, positive.source(), "subgroup-max-preserved"),
            "subgroup maximum proof",
        );

        let negative = generate_final_kir_output_equivalence_v1(&reference, &broadcast)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, negative.source(), "subgroup-max-to-broadcast"),
            "maximum-to-broadcast mutation",
        );
    }

    #[test]
    fn pinned_verus_proves_integer_scan_and_rejects_value_mutation() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = subgroup_scan_module(ExecutionCollectiveKindV1::InclusiveScanSum, false);
        let preserving = subgroup_scan_module(ExecutionCollectiveKindV1::InclusiveScanSum, true);
        let mut changed = preserving.clone();
        changed.functions[0].body.as_mut().unwrap().blocks[0].operations[4].kind =
            OperationKind::Constant(Constant::U32(1));
        let exclusive = subgroup_scan_module(ExecutionCollectiveKindV1::ExclusiveScanSum, true);
        VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(changed.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(exclusive.clone()).unwrap();

        let positive = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, positive.source(), "subgroup-scan-preserved"),
            "subgroup scan proof",
        );

        let negative = generate_final_kir_output_equivalence_v1(&reference, &changed)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, negative.source(), "subgroup-scan-value-mutation"),
            "subgroup scan value mutation",
        );
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&reference, &exclusive).unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::KernelContractMismatch,
        );
    }

    #[test]
    fn pinned_verus_proves_ordered_fence_trace_and_rejects_ordering_mutation() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = fenced_kernel_module(fe2o3_kernel_ir::MemoryOrdering::Release, false);
        let preserving = fenced_kernel_module(fe2o3_kernel_ir::MemoryOrdering::Release, true);
        let wrong_ordering = fenced_kernel_module(fe2o3_kernel_ir::MemoryOrdering::Acquire, true);
        VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(wrong_ordering.clone()).unwrap();

        let positive = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, positive.source(), "fence-preserved"),
            "fence proof",
        );

        let negative = generate_final_kir_output_equivalence_v1(&reference, &wrong_ordering)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, negative.source(), "fence-wrong-ordering"),
            "fence-ordering mutation",
        );
    }

    #[test]
    fn pinned_verus_proves_volatile_event_and_rejects_nonvolatile_mutation() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = volatile_store_module(true, false);
        let preserving = volatile_store_module(true, true);
        let nonvolatile = volatile_store_module(false, true);
        VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(nonvolatile.clone()).unwrap();

        let positive = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, positive.source(), "volatile-preserved"),
            "volatile proof",
        );

        let negative = generate_final_kir_output_equivalence_v1(&reference, &nonvolatile)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, negative.source(), "volatile-removed"),
            "volatile removal mutation",
        );
    }

    #[test]
    fn pinned_verus_proves_static_lds_allocation_event_and_rejects_extent_mutation() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = static_lds_event_module(64, false);
        let preserving = static_lds_event_module(64, true);
        let wrong_extent = static_lds_event_module(32, true);
        let mut unbounded = reference.clone();
        unbounded.kernels[0].workgroup_size = None;
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&unbounded, &unbounded).unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::ResourceLimit,
        );
        VerifiedCanonicalKernelIrV13::from_module(reference.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(preserving.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(wrong_extent.clone()).unwrap();

        let positive = generate_final_kir_output_equivalence_v1(&reference, &preserving)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_accepts(
            run_verus(&verus, positive.source(), "static-lds-preserved"),
            "static LDS allocation proof",
        );

        let negative = generate_final_kir_output_equivalence_v1(&reference, &wrong_extent)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, negative.source(), "static-lds-wrong-extent"),
            "LDS extent mutation",
        );
    }

    #[test]
    fn overlapping_unbranded_writes_are_rejected_before_proof_generation() {
        let reference = recurrence_memory_module(false, false, 1);
        let overlapping = recurrence_memory_module(false, false, 0);
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&reference, &overlapping).unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::UnsupportedAliasing,
        );
    }

    #[test]
    fn pinned_verus_proves_ieee_comparison_bits_and_rejects_arithmetic_without_a_model() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = strict_float_compare_module(ComparePredicate::LessThan);
        let same = strict_float_compare_module(ComparePredicate::LessThan);
        let changed = strict_float_compare_module(ComparePredicate::GreaterThan);

        let (proof, _, model) = generate_final_kir_output_equivalence_v1(&reference, &same)
            .unwrap()
            .into_parts();
        assert_eq!(model, FinalKirNumericalModelV1::ExactBitVector);
        assert_verus_accepts(
            run_verus(&verus, proof.source(), "strict-ieee-compare-same"),
            "strict IEEE comparison proof",
        );

        let hostile = generate_final_kir_output_equivalence_v1(&reference, &changed)
            .unwrap()
            .into_parts()
            .0;
        assert_verus_rejects(
            run_verus(&verus, hostile.source(), "strict-ieee-compare-changed"),
            "strict IEEE comparison mutation",
        );

        let strict_arithmetic = strict_float_module(BinaryOp::Add, NumericalModeV1::StrictIeee);
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&strict_arithmetic, &strict_arithmetic)
                .unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy,
        );
        let approximate = strict_float_module(BinaryOp::Add, NumericalModeV1::AllowApproximation);
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&approximate, &approximate).unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy,
        );
    }
}
