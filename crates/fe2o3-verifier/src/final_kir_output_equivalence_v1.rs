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
    ExecutionCapabilityRequirementV1, Function, GlobalCapabilityRoleV1, IntrinsicKind, Kernel,
    Module, NumericalModeV1, Operation, OperationKind, ScalarType, TargetCapability, Terminator,
    Type, UnaryOp, ValueId,
};

use crate::functional_refinement_receipt_v2::ranked_effect_formula_replay_prelude_v2;
use crate::{CanonicalGeneratedVerusProofInputV3, GeneratedVerusProofInputErrorV3};

const MAX_FINAL_KIR_SYMBOLIC_NODES_V1: usize = 8_192;
const MAX_FINAL_KIR_SYMBOLIC_DEPTH_V1: usize = 256;
const MAX_FINAL_KIR_EXECUTION_STATES_V1: usize = 512;
const MAX_FINAL_KIR_BLOCK_VISITS_V1: usize = 64;
const MAX_FINAL_KIR_MEMORY_WRITES_V1: usize = 1_024;

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
        writes: Vec<StoreEffectV1>,
        index: Box<ExpressionV1>,
    },
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
enum SymbolicValueV1 {
    Scalar(ExpressionV1),
    Pointer(PointerV1),
    Slice(SliceV1),
    Capability(CapabilityV1),
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
    element: ScalarV1,
    writes: Vec<StoreEffectV1>,
}

#[derive(Clone, Debug)]
struct ReturnOutcomeV1 {
    path: ExpressionV1,
    memories: BTreeMap<u32, MemoryVersionV1>,
    written_roots: BTreeSet<u32>,
}

#[derive(Clone, Debug)]
struct ExecutionStateV1 {
    block: BlockId,
    values: BTreeMap<ValueId, SymbolicValueV1>,
    memories: BTreeMap<u32, MemoryVersionV1>,
    written_roots: BTreeSet<u32>,
    path: ExpressionV1,
    visits: BTreeMap<BlockId, usize>,
}

struct KernelEffectsV1 {
    parameter_types: Vec<Type>,
    output_roots: BTreeSet<u32>,
    outcomes: Vec<ReturnOutcomeV1>,
    intrinsics: BTreeSet<IntrinsicKind>,
    uses_strict_float_operator_congruence: bool,
}

/// Mathematical strength of the generated final-KIR theorem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalKirNumericalModelV1 {
    /// Exact finite bit-vector semantics; no floating operator occurs.
    ExactBitVector,
    /// Equal raw output bits follow only from an identical sequence of
    /// deterministic StrictIeee-tagged operators. This is not an IEEE model,
    /// a real-arithmetic theorem, a reassociation theorem, or an error bound.
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
        FLOAT_CONGRUENCE_V1,
    )
    .map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    let mut output_writes = 0_u64;
    let mut uses_strict_float_operator_congruence = false;
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
        let source_effects = extract_kernel_effects(source_function, &numerical_policies)?;
        let final_effects = extract_kernel_effects(final_function, &numerical_policies)?;
        if source_effects.output_roots != final_effects.output_roots {
            return Err(FinalKirOutputEquivalenceErrorV1::OutputRosterMismatch);
        }
        if source_effects.uses_strict_float_operator_congruence
            != final_effects.uses_strict_float_operator_congruence
        {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
        }
        uses_strict_float_operator_congruence |=
            source_effects.uses_strict_float_operator_congruence;
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
        numerical_model: if uses_strict_float_operator_congruence {
            FinalKirNumericalModelV1::StrictFloatOperatorCongruence
        } else {
            FinalKirNumericalModelV1::ExactBitVector
        },
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
    let mut blocks = BTreeMap::new();
    for block in &body.blocks {
        if blocks.insert(block.id, block).is_some() || block.terminator.is_none() {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedControlFlow);
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
                    element,
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
        path: bool_true(),
        visits,
    }]);
    let mut outcomes = Vec::new();
    let mut intrinsics = BTreeSet::new();
    let mut capability_roots = BTreeMap::new();
    let mut nodes = 0_usize;
    let mut scheduled_states = 1_usize;
    let mut uses_strict_float_operator_congruence = false;
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
                &mut capability_roots,
                numerical_policies,
                &mut uses_strict_float_operator_congruence,
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
        uses_strict_float_operator_congruence,
    })
}

fn execute_operation(
    operation: &Operation,
    state: &mut ExecutionStateV1,
    intrinsics: &mut BTreeSet<IntrinsicKind>,
    capability_roots: &mut BTreeMap<u32, GlobalCapabilityRoleV1>,
    numerical_policies: &BTreeMap<ScalarType, NumericalModeV1>,
    uses_strict_float_operator_congruence: &mut bool,
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
            require_strict_float_operator_policy(
                scalar,
                numerical_policies,
                uses_strict_float_operator_congruence,
            )?;
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
            require_strict_float_operator_policy(
                scalar,
                numerical_policies,
                uses_strict_float_operator_congruence,
            )?;
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
            require_strict_float_operator_policy(
                lhs.scalar,
                numerical_policies,
                uses_strict_float_operator_congruence,
            )?;
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
            match capability_roots.insert(physical.memory_parameter, capability.role()) {
                Some(previous) if previous != capability.role() => {
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
        OperationKind::Call { .. } | OperationKind::MemoryIntrinsic(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCallOrTranscendental);
        }
        OperationKind::Wave(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedCollectiveSemantics);
        }
        OperationKind::Matrix(_) | OperationKind::Gfx950LdsTranspose(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedTensorSemantics);
        }
        OperationKind::Barrier(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::ExecutionCapability(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedSynchronizationSemantics);
        }
        OperationKind::Atomic(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedAtomicSemantics);
        }
        OperationKind::Alloca { .. } | OperationKind::InlineAssembly(_) => {
            return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
        }
    };
    if let Some(defined) = defined {
        let [result] = operation.results.as_slice() else {
            return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
        };
        state.values.insert(result.id, defined);
    } else if !operation.results.is_empty() {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
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

fn require_strict_float_operator_policy(
    scalar: ScalarV1,
    numerical_policies: &BTreeMap<ScalarType, NumericalModeV1>,
    used: &mut bool,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let ScalarV1::Float(value_type) = scalar else {
        return Ok(());
    };
    if numerical_policies.get(&value_type) != Some(&NumericalModeV1::StrictIeee) {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy);
    }
    *used = true;
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
    let width = expression.scalar.width();
    let value = match &expression.kind {
        ExpressionKindV1::Constant(value) => *value,
        ExpressionKindV1::Unary { operation, operand } => {
            let operand = evaluate_constant(operand)?;
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
            let lhs = evaluate_constant(lhs)?;
            let rhs = evaluate_constant(rhs)?;
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
            let mut lhs_value = evaluate_constant(lhs)?;
            let mut rhs_value = evaluate_constant(rhs)?;
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
            let operand_value = evaluate_constant(operand)?;
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
            if constant_bool(condition)? {
                evaluate_constant(when_true)?
            } else {
                evaluate_constant(when_false)?
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

fn require_load_access(
    pointer: &PointerV1,
    access_space: AddressSpace,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    if access_space != pointer.address_space
        || !matches!(
            pointer.address_space,
            AddressSpace::Global | AddressSpace::Constant
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
    if memory.element != pointer.element {
        return Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow);
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
        scalar: memory.element,
        kind: ExpressionKindV1::MemoryRead {
            memory_parameter: pointer.memory_parameter,
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
    if pointer.address_space != AddressSpace::Global
        || access_space != AddressSpace::Global
        || pointer.access == AccessMode::ReadOnly
        || pointer.element != value.scalar
    {
        return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation);
    }
    let memory = state
        .memories
        .get_mut(&pointer.memory_parameter)
        .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?;
    if memory.element != value.scalar || memory.writes.len() >= MAX_FINAL_KIR_MEMORY_WRITES_V1 {
        return Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit);
    }
    memory.writes.push(StoreEffectV1 {
        index: pointer.index,
        guard,
        value,
    });
    state.written_roots.insert(pointer.memory_parameter);
    Ok(())
}

fn require_alias_discipline(
    outcomes: &[ReturnOutcomeV1],
    capability_roots: &BTreeMap<u32, GlobalCapabilityRoleV1>,
) -> Result<(), FinalKirOutputEquivalenceErrorV1> {
    let memory_roots = outcomes
        .first()
        .map(|item| item.memories.keys().copied().collect::<BTreeSet<_>>())
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
    let mut intrinsics = reference.intrinsics.clone();
    intrinsics.extend(&actual.intrinsics);
    let mut parameters = Vec::new();
    for (ordinal, ty) in reference.parameter_types.iter().enumerate() {
        match ty {
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
    parameters.extend(
        intrinsics
            .iter()
            .map(|intrinsic| format!("{}: int", intrinsic_name(*intrinsic))),
    );
    parameters.extend(
        reference
            .output_roots
            .iter()
            .enumerate()
            .map(|(index, _)| format!("output_query_{index}: int")),
    );
    writeln!(
        source,
        "    proof fn fe2o3_final_kir_output_equivalence_{kernel_index}({}) {{",
        parameters.join(", ")
    )
    .map_err(|_| FinalKirOutputEquivalenceErrorV1::ResourceLimit)?;
    for (output_index, memory) in reference.output_roots.iter().enumerate() {
        let query = normalize(&format!("output_query_{output_index}"), 64);
        let reference_value = render_final_memory(reference, *memory, &query)?;
        let actual_value = render_final_memory(actual, *memory, &query)?;
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
    source.push_str("    }\n\n");
    Ok(())
}

fn render_final_memory(
    effects: &KernelEffectsV1,
    memory_parameter: u32,
    query: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let mut result = format!("m{memory_parameter}({query})");
    for outcome in effects.outcomes.iter().rev() {
        let memory = outcome
            .memories
            .get(&memory_parameter)
            .ok_or(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)?;
        let path = render_expression(&outcome.path)?;
        let value = render_memory_version(memory_parameter, memory, query)?;
        result = format!("if {path} == 1 {{ {value} }} else {{ {result} }}");
    }
    Ok(result)
}

fn render_memory_version(
    memory_parameter: u32,
    memory: &MemoryVersionV1,
    index: &str,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let mut result = normalize(
        &format!("m{memory_parameter}({index})"),
        memory.element.width(),
    );
    for write in &memory.writes {
        let write_index = render_expression(&write.index)?;
        let guard = render_expression(&write.guard)?;
        let value = render_expression(&write.value)?;
        result = format!(
            "if ({guard} == 1) && (({index}) == ({write_index})) {{ {value} }} else {{ {result} }}"
        );
    }
    Ok(result)
}

fn render_expression(
    expression: &ExpressionV1,
) -> Result<String, FinalKirOutputEquivalenceErrorV1> {
    let width = expression.scalar.width();
    let rendered = match &expression.kind {
        ExpressionKindV1::Parameter(ordinal) => normalize(&format!("p{ordinal}"), width),
        ExpressionKindV1::Intrinsic(intrinsic) => normalize(intrinsic_name(*intrinsic), width),
        ExpressionKindV1::SliceLength(ordinal) => normalize(&format!("len{ordinal}"), width),
        ExpressionKindV1::Constant(value) => normalize(&value.to_string(), width),
        ExpressionKindV1::Unary { operation, operand } => {
            let operand = render_expression(operand)?;
            match (operation, expression.scalar) {
                (UnaryOp::Not, ScalarV1::Bool) => format!("if {operand} == 0 {{ 1 }} else {{ 0 }}"),
                (UnaryOp::Not, ScalarV1::Signed(_) | ScalarV1::Unsigned(_)) => {
                    format!("(fe2o3_bv_modulus_v2({width}) - 1) - ({operand})")
                }
                (UnaryOp::Negate, ScalarV1::Signed(_) | ScalarV1::Unsigned(_)) => {
                    normalize(&format!("-({operand})"), width)
                }
                (UnaryOp::Negate, ScalarV1::Float(_)) => {
                    format!("fe2o3_float_unary_v1(1, 0, {width}, {operand})")
                }
                _ => return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation),
            }
        }
        ExpressionKindV1::Binary {
            operation,
            lhs,
            rhs,
        } => {
            let lhs = render_expression(lhs)?;
            let rhs = render_expression(rhs)?;
            if expression.scalar.is_float() {
                format!(
                    "fe2o3_float_binary_v1(1, {}, {width}, {lhs}, {rhs})",
                    binary_tag(*operation)?,
                )
            } else {
                render_integer_binary(*operation, expression.scalar, &lhs, &rhs)?
            }
        }
        ExpressionKindV1::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            let lhs_rendered = render_expression(lhs)?;
            let rhs_rendered = render_expression(rhs)?;
            if lhs.scalar.is_float() {
                format!(
                    "fe2o3_float_compare_v1(1, {}, {}, {lhs_rendered}, {rhs_rendered})",
                    compare_tag(*predicate),
                    lhs.scalar.width(),
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
            let rendered = render_expression(operand)?;
            render_cast(*kind, operand.scalar, expression.scalar, &rendered)
        }
        ExpressionKindV1::Select {
            condition,
            when_true,
            when_false,
        } => format!(
            "if {} == 1 {{ {} }} else {{ {} }}",
            render_expression(condition)?,
            render_expression(when_true)?,
            render_expression(when_false)?,
        ),
        ExpressionKindV1::MemoryRead {
            memory_parameter,
            writes,
            index,
        } => {
            let memory = MemoryVersionV1 {
                element: expression.scalar,
                writes: writes.clone(),
            };
            render_memory_version(*memory_parameter, &memory, &render_expression(index)?)?
        }
    };
    Ok(rendered)
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

fn render_cast(kind: CastKind, from: ScalarV1, to: ScalarV1, value: &str) -> String {
    match (kind, from, to) {
        (CastKind::Truncate | CastKind::Bitcast, _, _) => normalize(value, to.width()),
        (CastKind::ZeroExtend, _, _) => normalize(&normalize(value, from.width()), to.width()),
        (CastKind::SignExtend, _, _) => normalize(
            &format!("fe2o3_bv_signed_v2({value}, {})", from.width()),
            to.width(),
        ),
        _ => format!(
            "fe2o3_scalar_cast_v1({}, {}, {}, {value})",
            cast_tag(kind),
            from.width(),
            to.width(),
        ),
    }
}

fn binary_tag(operation: BinaryOp) -> Result<u8, FinalKirOutputEquivalenceErrorV1> {
    Ok(match operation {
        BinaryOp::Add => 0,
        BinaryOp::Subtract => 1,
        BinaryOp::Multiply => 2,
        BinaryOp::Divide => 3,
        BinaryOp::Remainder => 4,
        BinaryOp::BitAnd => 5,
        BinaryOp::BitOr => 6,
        BinaryOp::BitXor => 7,
        BinaryOp::ShiftLeft => 8,
        BinaryOp::ShiftRight => 9,
        BinaryOp::Checked(_) => return Err(FinalKirOutputEquivalenceErrorV1::UnsupportedOperation),
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

const fn cast_tag(kind: CastKind) -> u8 {
    match kind {
        CastKind::RestrictPointerAccess => 0,
        CastKind::Truncate => 1,
        CastKind::ZeroExtend => 2,
        CastKind::SignExtend => 3,
        CastKind::FloatExtend => 4,
        CastKind::FloatTruncate => 5,
        CastKind::IntegerToFloat => 6,
        CastKind::FloatToInteger => 7,
        CastKind::Bitcast => 8,
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

const FLOAT_CONGRUENCE_V1: &str = r#"
    // These functions provide only deterministic operator congruence over raw
    // floating-point bits. They do not imply real arithmetic or error bounds.
    // numerical_policy=1 is the exact StrictIeee KIR requirement. The
    // uninterpreted result deliberately proves congruence only, not IEEE value
    // semantics, transcendental accuracy, or a finite-error theorem.
    uninterp spec fn fe2o3_float_unary_v1(numerical_policy: int, operation: int, width: int, value: int) -> int;
    uninterp spec fn fe2o3_float_binary_v1(numerical_policy: int, operation: int, width: int, lhs: int, rhs: int) -> int;
    uninterp spec fn fe2o3_float_compare_v1(numerical_policy: int, operation: int, width: int, lhs: int, rhs: int) -> int;
    uninterp spec fn fe2o3_scalar_cast_v1(operation: int, from_width: int, to_width: int, value: int) -> int;
"#;

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
        Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13,
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
        let preserved_output = run_verus(&verus, preserved_proof.source(), "preserved");
        assert!(
            preserved_output.status.success(),
            "preserving proof failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&preserved_output.stdout),
            String::from_utf8_lossy(&preserved_output.stderr),
        );

        let changed_proof = generate_final_kir_output_equivalence_v1(&reference, &changed)
            .unwrap()
            .into_parts()
            .0;
        let changed_output = run_verus(&verus, changed_proof.source(), "changed");
        assert!(
            !changed_output.status.success(),
            "semantics-changing proof unexpectedly verified:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&changed_output.stdout),
            String::from_utf8_lossy(&changed_output.stderr),
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
        let accepted = run_verus(&verus, proof.source(), "bounded-cfg-preserved");
        assert!(
            accepted.status.success(),
            "bounded CFG proof failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&accepted.stdout),
            String::from_utf8_lossy(&accepted.stderr),
        );

        let hostile = generate_final_kir_output_equivalence_v1(&reference, &wrong_recurrence)
            .unwrap()
            .into_parts()
            .0;
        let rejected = run_verus(&verus, hostile.source(), "bounded-cfg-wrong-recurrence");
        assert!(
            !rejected.status.success(),
            "wrong loop-carried recurrence unexpectedly verified"
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
    fn pinned_verus_models_strict_float_only_as_operator_congruence() {
        let verus = pinned_rust_verify();
        assert!(verus.is_file(), "pinned rust_verify is missing");
        let reference = strict_float_module(BinaryOp::Add, NumericalModeV1::StrictIeee);
        let same = strict_float_module(BinaryOp::Add, NumericalModeV1::StrictIeee);
        let changed = strict_float_module(BinaryOp::Subtract, NumericalModeV1::StrictIeee);

        let (proof, _, model) = generate_final_kir_output_equivalence_v1(&reference, &same)
            .unwrap()
            .into_parts();
        assert_eq!(
            model,
            FinalKirNumericalModelV1::StrictFloatOperatorCongruence
        );
        assert!(
            run_verus(&verus, proof.source(), "strict-float-same")
                .status
                .success()
        );

        let hostile = generate_final_kir_output_equivalence_v1(&reference, &changed)
            .unwrap()
            .into_parts()
            .0;
        assert!(
            !run_verus(&verus, hostile.source(), "strict-float-changed")
                .status
                .success()
        );

        let approximate = strict_float_module(BinaryOp::Add, NumericalModeV1::AllowApproximation);
        assert_eq!(
            generate_final_kir_output_equivalence_v1(&approximate, &approximate).unwrap_err(),
            FinalKirOutputEquivalenceErrorV1::UnsupportedNumericalPolicy,
        );
    }
}
