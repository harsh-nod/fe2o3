//! Deterministic target-neutral transforms that need whole-module KIR structure.
//!
//! The Pliron bridge deliberately freezes the function inventory while a graph
//! is live. SROA and interprocedural cleanup therefore run on the same canonical
//! V13 module immediately around that bridge. The preservation layer replays
//! each deterministic relation before the graph can advance.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use fe2o3_kernel_ir::{
    AddressSpace, Function, FunctionId, FunctionRole, Module, Operation, OperationKind, Terminator,
    Type, ValueDef, ValueId, verify_module,
};

pub const HARD_MAX_MODULE_TRANSFORM_FUNCTIONS_V1: usize = 4_096;
pub const HARD_MAX_MODULE_TRANSFORM_OPERATIONS_V1: usize = 1_000_000;
pub const HARD_MAX_INLINE_BODY_OPERATIONS_V1: usize = 256;
pub const HARD_MAX_INLINE_GROWTH_OPERATIONS_V1: usize = 65_536;
pub const HARD_MAX_INTERPROCEDURAL_ITERATIONS_V1: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetNeutralModuleTransformLimitsV1 {
    max_functions: usize,
    max_operations: usize,
    max_inline_body_operations: usize,
    max_inline_growth_operations: usize,
    max_interprocedural_iterations: usize,
}

impl TargetNeutralModuleTransformLimitsV1 {
    pub fn new(
        max_functions: usize,
        max_operations: usize,
        max_inline_body_operations: usize,
        max_inline_growth_operations: usize,
        max_interprocedural_iterations: usize,
    ) -> Result<Self, TargetNeutralModuleTransformErrorV1> {
        let limits = Self {
            max_functions,
            max_operations,
            max_inline_body_operations,
            max_inline_growth_operations,
            max_interprocedural_iterations,
        };
        limits.validate()?;
        Ok(limits)
    }

    fn validate(self) -> Result<(), TargetNeutralModuleTransformErrorV1> {
        let valid = self.max_functions != 0
            && self.max_functions <= HARD_MAX_MODULE_TRANSFORM_FUNCTIONS_V1
            && self.max_operations != 0
            && self.max_operations <= HARD_MAX_MODULE_TRANSFORM_OPERATIONS_V1
            && self.max_inline_body_operations != 0
            && self.max_inline_body_operations <= HARD_MAX_INLINE_BODY_OPERATIONS_V1
            && self.max_inline_growth_operations != 0
            && self.max_inline_growth_operations <= HARD_MAX_INLINE_GROWTH_OPERATIONS_V1
            && self.max_interprocedural_iterations != 0
            && self.max_interprocedural_iterations <= HARD_MAX_INTERPROCEDURAL_ITERATIONS_V1;
        valid
            .then_some(())
            .ok_or(TargetNeutralModuleTransformErrorV1::InvalidLimits)
    }
}

impl Default for TargetNeutralModuleTransformLimitsV1 {
    fn default() -> Self {
        Self {
            max_functions: HARD_MAX_MODULE_TRANSFORM_FUNCTIONS_V1,
            max_operations: HARD_MAX_MODULE_TRANSFORM_OPERATIONS_V1,
            max_inline_body_operations: HARD_MAX_INLINE_BODY_OPERATIONS_V1,
            max_inline_growth_operations: HARD_MAX_INLINE_GROWTH_OPERATIONS_V1,
            max_interprocedural_iterations: HARD_MAX_INTERPROCEDURAL_ITERATIONS_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetNeutralModuleTransformV1 {
    ScalarReplacementOfAggregates,
    SecondarySsaPromotion,
    HelperInlining,
    InterproceduralCleanup,
}

/// Stable source relation for one accepted whole-module edit. Coordinates use
/// canonical function identity and KIR block identity, never arena pointers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TargetNeutralModuleEditV1 {
    AggregateSplit {
        function: FunctionId,
        block: fe2o3_kernel_ir::BlockId,
        operation: u32,
        projected_offsets: Vec<u64>,
    },
    ScalarSlotPromoted {
        function: FunctionId,
        block: fe2o3_kernel_ir::BlockId,
        operation: u32,
        eliminated_loads: u32,
        eliminated_stores: u32,
    },
    CallInlined {
        caller: FunctionId,
        block: fe2o3_kernel_ir::BlockId,
        operation: u32,
        callee: FunctionId,
        cloned_operations: u32,
    },
    HelperArgumentsRemoved {
        helper: FunctionId,
        parameter_indexes: Vec<u32>,
    },
    HelperRemoved {
        helper: FunctionId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetNeutralModuleTransformReportV1 {
    transform: TargetNeutralModuleTransformV1,
    changed: bool,
    eliminated_operations: usize,
    inserted_operations: usize,
    inlined_calls: usize,
    removed_arguments: usize,
    removed_helpers: usize,
    edits: Vec<TargetNeutralModuleEditV1>,
}

impl TargetNeutralModuleTransformReportV1 {
    pub const fn transform(&self) -> TargetNeutralModuleTransformV1 {
        self.transform
    }

    pub const fn changed(&self) -> bool {
        self.changed
    }

    pub const fn eliminated_operations(&self) -> usize {
        self.eliminated_operations
    }

    pub const fn inserted_operations(&self) -> usize {
        self.inserted_operations
    }

    pub const fn inlined_calls(&self) -> usize {
        self.inlined_calls
    }

    pub const fn removed_arguments(&self) -> usize {
        self.removed_arguments
    }

    pub const fn removed_helpers(&self) -> usize {
        self.removed_helpers
    }

    pub fn edits(&self) -> &[TargetNeutralModuleEditV1] {
        &self.edits
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum TargetNeutralModuleTransformErrorV1 {
    InvalidLimits,
    InputRejected,
    OutputRejected,
    FunctionLimitExceeded { required: usize, limit: usize },
    OperationLimitExceeded { required: usize, limit: usize },
    InlineGrowthLimitExceeded { required: usize, limit: usize },
    InterproceduralIterationLimitExceeded { limit: usize },
    RecursiveHelperGraph,
    ValueIdentityOverflow,
}

impl fmt::Display for TargetNeutralModuleTransformErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => {
                formatter.write_str("target-neutral module-transform limits are invalid")
            }
            Self::InputRejected => formatter
                .write_str("target-neutral module-transform input is invalid canonical KIR"),
            Self::OutputRejected => formatter
                .write_str("target-neutral module-transform output is invalid canonical KIR"),
            Self::FunctionLimitExceeded { required, limit } => {
                write!(
                    formatter,
                    "module has {required} functions but the limit is {limit}"
                )
            }
            Self::OperationLimitExceeded { required, limit } => {
                write!(
                    formatter,
                    "module has {required} operations but the limit is {limit}"
                )
            }
            Self::InlineGrowthLimitExceeded { required, limit } => write!(
                formatter,
                "helper inlining requires {required} cloned operations but the limit is {limit}"
            ),
            Self::InterproceduralIterationLimitExceeded { limit } => write!(
                formatter,
                "interprocedural cleanup did not converge within {limit} iterations"
            ),
            Self::RecursiveHelperGraph => {
                formatter.write_str("helper inlining rejects a recursive call-graph SCC")
            }
            Self::ValueIdentityOverflow => formatter
                .write_str("target-neutral transform exhausted the canonical value-identity space"),
        }
    }
}

impl Error for TargetNeutralModuleTransformErrorV1 {}

pub fn execute_target_neutral_module_transform_v1(
    transform: TargetNeutralModuleTransformV1,
    input: &Module,
    limits: TargetNeutralModuleTransformLimitsV1,
) -> Result<(Module, TargetNeutralModuleTransformReportV1), TargetNeutralModuleTransformErrorV1> {
    limits.validate()?;
    verify_module(input).map_err(|_| TargetNeutralModuleTransformErrorV1::InputRejected)?;
    enforce_module_limits(input, limits)?;

    let mut output = input.clone();
    let mut report = TargetNeutralModuleTransformReportV1 {
        transform,
        changed: false,
        eliminated_operations: 0,
        inserted_operations: 0,
        inlined_calls: 0,
        removed_arguments: 0,
        removed_helpers: 0,
        edits: Vec::new(),
    };
    match transform {
        TargetNeutralModuleTransformV1::ScalarReplacementOfAggregates => {
            split_private_array_allocas(&mut output, &mut report)?;
        }
        TargetNeutralModuleTransformV1::SecondarySsaPromotion => {
            promote_private_scalar_allocas(&mut output, &mut report)?;
        }
        TargetNeutralModuleTransformV1::HelperInlining => {
            inline_bounded_scalar_helpers(&mut output, limits, &mut report)?;
        }
        TargetNeutralModuleTransformV1::InterproceduralCleanup => {
            cleanup_interprocedural_contracts(&mut output, limits, &mut report)?;
        }
    }
    enforce_module_limits(&output, limits)?;
    verify_module(&output).map_err(|_| TargetNeutralModuleTransformErrorV1::OutputRejected)?;
    report.changed = &output != input;
    Ok((output, report))
}

/// Reconstructs the transform from immutable input and compares exact output.
/// A pass-reported `changed` bit is never an input to this checker.
pub fn check_target_neutral_module_transform_relation_v1(
    transform: TargetNeutralModuleTransformV1,
    before: &Module,
    after: &Module,
    limits: TargetNeutralModuleTransformLimitsV1,
) -> Result<TargetNeutralModuleTransformReportV1, TargetNeutralModuleTransformErrorV1> {
    let (expected, report) = execute_target_neutral_module_transform_v1(transform, before, limits)?;
    (&expected == after)
        .then_some(report)
        .ok_or(TargetNeutralModuleTransformErrorV1::OutputRejected)
}

fn enforce_module_limits(
    module: &Module,
    limits: TargetNeutralModuleTransformLimitsV1,
) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    if module.functions.len() > limits.max_functions {
        return Err(TargetNeutralModuleTransformErrorV1::FunctionLimitExceeded {
            required: module.functions.len(),
            limit: limits.max_functions,
        });
    }
    let operations = operation_count(module);
    if operations > limits.max_operations {
        return Err(
            TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                required: operations,
                limit: limits.max_operations,
            },
        );
    }
    Ok(())
}

fn operation_count(module: &Module) -> usize {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .map(|block| block.operations.len())
        .sum()
}

fn value_types(function: &Function) -> BTreeMap<ValueId, Type> {
    let mut types = BTreeMap::new();
    if let Some(body) = &function.body {
        types.extend(
            body.parameters
                .iter()
                .copied()
                .zip(function.signature.parameters.iter().cloned()),
        );
        for block in &body.blocks {
            types.extend(
                block
                    .parameters
                    .iter()
                    .map(|value| (value.id, value.ty.clone())),
            );
            types.extend(
                block
                    .operations
                    .iter()
                    .flat_map(|operation| operation.results.iter())
                    .map(|value| (value.id, value.ty.clone())),
            );
        }
    }
    types
}

fn constant_indices(function: &Function) -> BTreeMap<ValueId, u64> {
    let mut constants = BTreeMap::new();
    if let Some(body) = &function.body {
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            let [result] = operation.results.as_slice() else {
                continue;
            };
            let OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(value)) = &operation.kind
            else {
                continue;
            };
            constants.insert(result.id, *value);
        }
    }
    constants
}

pub(crate) fn fresh_value_id(
    function: &Function,
) -> Result<u32, TargetNeutralModuleTransformErrorV1> {
    function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| {
            body.parameters
                .iter()
                .copied()
                .chain(body.blocks.iter().flat_map(|block| {
                    block.parameters.iter().map(|value| value.id).chain(
                        block
                            .operations
                            .iter()
                            .flat_map(|operation| operation.results.iter().map(|value| value.id)),
                    )
                }))
        })
        .map(|value| value.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(TargetNeutralModuleTransformErrorV1::ValueIdentityOverflow)
}

fn split_private_array_allocas(
    module: &mut Module,
    report: &mut TargetNeutralModuleTransformReportV1,
) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    for function in &mut module.functions {
        let function_id = function.id.clone();
        let constants = constant_indices(function);
        let types = value_types(function);
        let mut next = fresh_value_id(function)?;
        let Some(body) = &mut function.body else {
            continue;
        };
        for block in &mut body.blocks {
            let original = std::mem::take(&mut block.operations);
            let mut replacements = BTreeMap::new();
            let mut removed = BTreeSet::new();
            let mut inserted_before = BTreeMap::<usize, Vec<Operation>>::new();
            for (index, operation) in original.iter().enumerate() {
                let (
                    [pointer],
                    OperationKind::Alloca {
                        element,
                        count: Some(count),
                        address_space: AddressSpace::Private,
                        alignment,
                    },
                ) = (operation.results.as_slice(), &operation.kind)
                else {
                    continue;
                };
                let Some(element_count) = constants.get(count).copied() else {
                    continue;
                };
                if element_count == 0 {
                    continue;
                }
                let uses = collect_value_uses(&original, pointer.id);
                let mut projections = BTreeMap::<u64, Vec<(usize, ValueId)>>::new();
                let mut eligible = !uses.is_empty();
                for use_index in uses {
                    let use_operation = &original[use_index];
                    let ([projected], OperationKind::GetElementPointer { base, offset }) =
                        (use_operation.results.as_slice(), &use_operation.kind)
                    else {
                        eligible = false;
                        break;
                    };
                    let Some(offset) = constants
                        .get(offset)
                        .copied()
                        .filter(|offset| *offset < element_count)
                    else {
                        eligible = false;
                        break;
                    };
                    if *base != pointer.id
                        || !pointer_has_only_plain_memory_uses(&original, projected.id)
                    {
                        eligible = false;
                        break;
                    }
                    projections
                        .entry(offset)
                        .or_default()
                        .push((use_index, projected.id));
                }
                if !eligible {
                    continue;
                }
                report
                    .edits
                    .push(TargetNeutralModuleEditV1::AggregateSplit {
                        function: function_id.clone(),
                        block: block.id,
                        operation: u32::try_from(index).map_err(|_| {
                            TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                                required: index,
                                limit: u32::MAX as usize,
                            }
                        })?,
                        projected_offsets: projections.keys().copied().collect(),
                    });
                let pointer_type = types
                    .get(&pointer.id)
                    .cloned()
                    .ok_or(TargetNeutralModuleTransformErrorV1::InputRejected)?;
                for projected in projections.values() {
                    let scalar_pointer = ValueId(next);
                    next = next
                        .checked_add(1)
                        .ok_or(TargetNeutralModuleTransformErrorV1::ValueIdentityOverflow)?;
                    inserted_before
                        .entry(index)
                        .or_default()
                        .push(Operation::effect_free(
                            ValueDef::new(scalar_pointer, pointer_type.clone()),
                            OperationKind::Alloca {
                                element: element.clone(),
                                count: None,
                                address_space: AddressSpace::Private,
                                alignment: *alignment,
                            },
                        ));
                    for (projection_index, projection_value) in projected {
                        replacements.insert(*projection_value, scalar_pointer);
                        removed.insert(*projection_index);
                    }
                }
                removed.insert(index);
            }
            let mut rewritten = Vec::new();
            for (index, mut operation) in original.into_iter().enumerate() {
                if let Some(inserted) = inserted_before.remove(&index) {
                    report.inserted_operations += inserted.len();
                    rewritten.extend(inserted);
                }
                if removed.contains(&index) {
                    report.eliminated_operations += 1;
                    continue;
                }
                remap_supported_operation(&mut operation, &replacements)?;
                rewritten.push(operation);
            }
            remap_terminator(block.terminator.as_mut(), &replacements);
            block.operations = rewritten;
        }
    }
    Ok(())
}

fn promote_private_scalar_allocas(
    module: &mut Module,
    report: &mut TargetNeutralModuleTransformReportV1,
) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    for function in &mut module.functions {
        let function_id = function.id.clone();
        let Some(body) = &mut function.body else {
            continue;
        };
        for block in &mut body.blocks {
            let original = std::mem::take(&mut block.operations);
            let mut replacements = BTreeMap::new();
            let mut removed = BTreeSet::new();
            for (index, operation) in original.iter().enumerate() {
                let (
                    [pointer],
                    OperationKind::Alloca {
                        count: None,
                        address_space: AddressSpace::Private,
                        ..
                    },
                ) = (operation.results.as_slice(), &operation.kind)
                else {
                    continue;
                };
                if !pointer_has_only_plain_memory_uses(&original, pointer.id) {
                    continue;
                }
                let mut current = None;
                let mut loads = Vec::new();
                let mut stores = Vec::new();
                let mut valid = true;
                for (use_index, use_operation) in original.iter().enumerate().skip(index + 1) {
                    match &use_operation.kind {
                        OperationKind::Store {
                            pointer: used,
                            value,
                            access,
                        } if *used == pointer.id
                            && !access.volatile
                            && access.address_space == AddressSpace::Private =>
                        {
                            current = Some(resolve(*value, &replacements));
                            stores.push(use_index);
                        }
                        OperationKind::Load {
                            pointer: used,
                            access,
                        } if *used == pointer.id
                            && !access.volatile
                            && access.address_space == AddressSpace::Private =>
                        {
                            let ([result], Some(value)) =
                                (use_operation.results.as_slice(), current)
                            else {
                                valid = false;
                                break;
                            };
                            replacements.insert(result.id, value);
                            loads.push(use_index);
                        }
                        _ if use_operation.kind.operands().contains(&pointer.id) => {
                            valid = false;
                            break;
                        }
                        _ => {}
                    }
                }
                if valid && !loads.is_empty() {
                    report
                        .edits
                        .push(TargetNeutralModuleEditV1::ScalarSlotPromoted {
                            function: function_id.clone(),
                            block: block.id,
                            operation: u32::try_from(index).map_err(|_| {
                                TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                                    required: index,
                                    limit: u32::MAX as usize,
                                }
                            })?,
                            eliminated_loads: u32::try_from(loads.len()).map_err(|_| {
                                TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                                    required: loads.len(),
                                    limit: u32::MAX as usize,
                                }
                            })?,
                            eliminated_stores: u32::try_from(stores.len()).map_err(|_| {
                                TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                                    required: stores.len(),
                                    limit: u32::MAX as usize,
                                }
                            })?,
                        });
                    removed.insert(index);
                    removed.extend(loads);
                    removed.extend(stores);
                }
            }
            let mut rewritten = Vec::new();
            for (index, mut operation) in original.into_iter().enumerate() {
                if removed.contains(&index) {
                    report.eliminated_operations += 1;
                    continue;
                }
                remap_supported_operation(&mut operation, &replacements)?;
                rewritten.push(operation);
            }
            remap_terminator(block.terminator.as_mut(), &replacements);
            block.operations = rewritten;
        }
    }
    Ok(())
}

fn pointer_has_only_plain_memory_uses(operations: &[Operation], pointer: ValueId) -> bool {
    let mut any = false;
    for operation in operations {
        if !operation.kind.operands().contains(&pointer) {
            continue;
        }
        any = true;
        match &operation.kind {
            OperationKind::Load {
                pointer: used,
                access,
            } if *used == pointer
                && !access.volatile
                && access.address_space == AddressSpace::Private => {}
            OperationKind::Store {
                pointer: used,
                access,
                ..
            } if *used == pointer
                && !access.volatile
                && access.address_space == AddressSpace::Private => {}
            _ => return false,
        }
    }
    any
}

fn collect_value_uses(operations: &[Operation], value: ValueId) -> Vec<usize> {
    operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            operation.kind.operands().contains(&value).then_some(index)
        })
        .collect()
}

fn inline_bounded_scalar_helpers(
    module: &mut Module,
    limits: TargetNeutralModuleTransformLimitsV1,
    report: &mut TargetNeutralModuleTransformReportV1,
) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    reject_recursive_call_graph(module)?;
    let helpers = module
        .functions
        .iter()
        .filter(|function| function.role == FunctionRole::InternalHelper)
        .filter_map(|function| {
            inlineable_scalar_helper(function, limits.max_inline_body_operations)
                .then_some((function.id.clone(), function.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let growth = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, .. } => helpers
                .get(callee)
                .and_then(|helper| helper.body.as_ref())
                .map(|body| body.blocks[0].operations.len()),
            _ => None,
        })
        .try_fold(0_usize, usize::checked_add)
        .ok_or(
            TargetNeutralModuleTransformErrorV1::InlineGrowthLimitExceeded {
                required: usize::MAX,
                limit: limits.max_inline_growth_operations,
            },
        )?;
    if growth > limits.max_inline_growth_operations {
        return Err(
            TargetNeutralModuleTransformErrorV1::InlineGrowthLimitExceeded {
                required: growth,
                limit: limits.max_inline_growth_operations,
            },
        );
    }

    for function in &mut module.functions {
        let caller = function.id.clone();
        let mut next = fresh_value_id(function)?;
        let Some(body) = &mut function.body else {
            continue;
        };
        for block in &mut body.blocks {
            let original = std::mem::take(&mut block.operations);
            let mut replacements = BTreeMap::new();
            let mut rewritten = Vec::new();
            for (operation_index, mut operation) in original.into_iter().enumerate() {
                remap_supported_operation(&mut operation, &replacements)?;
                let OperationKind::Call { callee, arguments } = &operation.kind else {
                    rewritten.push(operation);
                    continue;
                };
                let Some(helper) = helpers.get(callee) else {
                    rewritten.push(operation);
                    continue;
                };
                let helper_body = helper.body.as_ref().expect("inlineable helper has a body");
                let helper_block = &helper_body.blocks[0];
                let mut local = helper_body
                    .parameters
                    .iter()
                    .copied()
                    .zip(arguments.iter().copied())
                    .collect::<BTreeMap<_, _>>();
                for helper_operation in &helper_block.operations {
                    let mut cloned = helper_operation.clone();
                    remap_supported_operation(&mut cloned, &local)?;
                    for result in &mut cloned.results {
                        let old = result.id;
                        result.id = ValueId(next);
                        next = next
                            .checked_add(1)
                            .ok_or(TargetNeutralModuleTransformErrorV1::ValueIdentityOverflow)?;
                        local.insert(old, result.id);
                    }
                    rewritten.push(cloned);
                    report.inserted_operations += 1;
                }
                let Terminator::Return { values } = helper_block
                    .terminator
                    .as_ref()
                    .expect("inlineable helper returns")
                else {
                    unreachable!()
                };
                if values.len() != operation.results.len() {
                    return Err(TargetNeutralModuleTransformErrorV1::InputRejected);
                }
                for (result, value) in operation.results.iter().zip(values) {
                    replacements.insert(result.id, resolve(*value, &local));
                }
                report.eliminated_operations += 1;
                report.inlined_calls += 1;
                report.edits.push(TargetNeutralModuleEditV1::CallInlined {
                    caller: caller.clone(),
                    block: block.id,
                    operation: u32::try_from(operation_index).map_err(|_| {
                        TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                            required: operation_index,
                            limit: u32::MAX as usize,
                        }
                    })?,
                    callee: callee.clone(),
                    cloned_operations: u32::try_from(helper_block.operations.len()).map_err(
                        |_| TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                            required: helper_block.operations.len(),
                            limit: u32::MAX as usize,
                        },
                    )?,
                });
            }
            for operation in &mut rewritten {
                remap_supported_operation(operation, &replacements)?;
            }
            remap_terminator(block.terminator.as_mut(), &replacements);
            block.operations = rewritten;
        }
    }
    Ok(())
}

fn inlineable_scalar_helper(function: &Function, max_body: usize) -> bool {
    let Some(body) = &function.body else {
        return false;
    };
    let [block] = body.blocks.as_slice() else {
        return false;
    };
    block.parameters.is_empty()
        && block.operations.len() <= max_body
        && matches!(block.terminator, Some(Terminator::Return { .. }))
        && block.operations.iter().all(|operation| {
            matches!(
                operation.kind,
                OperationKind::Constant(_)
                    | OperationKind::Unary { .. }
                    | OperationKind::Binary { .. }
                    | OperationKind::Compare { .. }
                    | OperationKind::Cast { .. }
                    | OperationKind::Select { .. }
            )
        })
}

fn reject_recursive_call_graph(module: &Module) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    let defined = module
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .map(|function| function.id.clone())
        .collect::<BTreeSet<_>>();
    let edges = module
        .functions
        .iter()
        .filter_map(|function| {
            function.body.as_ref().map(|body| {
                let callees = body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match &operation.kind {
                        OperationKind::Call { callee, .. } if defined.contains(callee) => {
                            Some(callee.clone())
                        }
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>();
                (function.id.clone(), callees)
            })
        })
        .collect::<BTreeMap<_, _>>();

    fn visit(
        node: &FunctionId,
        edges: &BTreeMap<FunctionId, BTreeSet<FunctionId>>,
        visiting: &mut BTreeSet<FunctionId>,
        done: &mut BTreeSet<FunctionId>,
    ) -> bool {
        if done.contains(node) {
            return false;
        }
        if !visiting.insert(node.clone()) {
            return true;
        }
        if edges
            .get(node)
            .is_some_and(|next| next.iter().any(|next| visit(next, edges, visiting, done)))
        {
            return true;
        }
        visiting.remove(node);
        done.insert(node.clone());
        false
    }

    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    if edges
        .keys()
        .any(|node| visit(node, &edges, &mut visiting, &mut done))
    {
        Err(TargetNeutralModuleTransformErrorV1::RecursiveHelperGraph)
    } else {
        Ok(())
    }
}

fn cleanup_interprocedural_contracts(
    module: &mut Module,
    limits: TargetNeutralModuleTransformLimitsV1,
    report: &mut TargetNeutralModuleTransformReportV1,
) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    for _ in 0..limits.max_interprocedural_iterations {
        let before = module.clone();
        remove_unused_helper_arguments(module, report)?;
        remove_uncalled_helpers(module, report);
        if *module == before {
            return Ok(());
        }
    }
    Err(
        TargetNeutralModuleTransformErrorV1::InterproceduralIterationLimitExceeded {
            limit: limits.max_interprocedural_iterations,
        },
    )
}

fn remove_unused_helper_arguments(
    module: &mut Module,
    report: &mut TargetNeutralModuleTransformReportV1,
) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    let removals = module
        .functions
        .iter()
        .filter(|function| function.role == FunctionRole::InternalHelper)
        .filter_map(|function| {
            let body = function.body.as_ref()?;
            let used = body
                .blocks
                .iter()
                .flat_map(|block| {
                    block
                        .operations
                        .iter()
                        .flat_map(|operation| operation.kind.operands())
                        .chain(block.terminator.iter().flat_map(Terminator::operands))
                })
                .collect::<BTreeSet<_>>();
            let indexes = body
                .parameters
                .iter()
                .enumerate()
                .filter_map(|(index, parameter)| (!used.contains(parameter)).then_some(index))
                .collect::<Vec<_>>();
            (!indexes.is_empty()).then_some((function.id.clone(), indexes))
        })
        .collect::<BTreeMap<_, _>>();

    for function in &mut module.functions {
        if let Some(indexes) = removals.get(&function.id) {
            let Some(body) = &mut function.body else {
                continue;
            };
            retain_unlisted(&mut function.signature.parameters, indexes);
            retain_unlisted(&mut body.parameters, indexes);
            report.removed_arguments += indexes.len();
            report
                .edits
                .push(TargetNeutralModuleEditV1::HelperArgumentsRemoved {
                    helper: function.id.clone(),
                    parameter_indexes: indexes
                        .iter()
                        .map(|index| {
                            u32::try_from(*index).map_err(|_| {
                                TargetNeutralModuleTransformErrorV1::OperationLimitExceeded {
                                    required: *index,
                                    limit: u32::MAX as usize,
                                }
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                });
        }
        let Some(body) = &mut function.body else {
            continue;
        };
        for operation in body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            let OperationKind::Call { callee, arguments } = &mut operation.kind else {
                continue;
            };
            if let Some(indexes) = removals.get(callee) {
                retain_unlisted(arguments, indexes);
            }
        }
    }
    Ok(())
}

fn retain_unlisted<T>(values: &mut Vec<T>, removed: &[usize]) {
    let mut index = 0_usize;
    values.retain(|_| {
        let keep = removed.binary_search(&index).is_err();
        index += 1;
        keep
    });
}

fn remove_uncalled_helpers(module: &mut Module, report: &mut TargetNeutralModuleTransformReportV1) {
    let edges = module
        .functions
        .iter()
        .filter_map(|function| {
            function.body.as_ref().map(|body| {
                let callees = body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match &operation.kind {
                        OperationKind::Call { callee, .. } => Some(callee.clone()),
                        _ => None,
                    })
                    .collect::<BTreeSet<_>>();
                (function.id.clone(), callees)
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut reachable = module
        .functions
        .iter()
        .filter(|function| function.role != FunctionRole::InternalHelper)
        .map(|function| function.id.clone())
        .collect::<BTreeSet<_>>();
    let mut pending = reachable.iter().cloned().collect::<Vec<_>>();
    while let Some(function) = pending.pop() {
        for callee in edges.get(&function).into_iter().flatten() {
            if reachable.insert(callee.clone()) {
                pending.push(callee.clone());
            }
        }
    }
    let removed = module
        .functions
        .iter()
        .filter(|function| {
            function.role == FunctionRole::InternalHelper && !reachable.contains(&function.id)
        })
        .map(|function| function.id.clone())
        .collect::<Vec<_>>();
    let before = module.functions.len();
    module.functions.retain(|function| {
        function.role != FunctionRole::InternalHelper || reachable.contains(&function.id)
    });
    report.removed_helpers += before - module.functions.len();
    report.edits.extend(
        removed
            .into_iter()
            .map(|helper| TargetNeutralModuleEditV1::HelperRemoved { helper }),
    );
}

fn resolve(mut value: ValueId, replacements: &BTreeMap<ValueId, ValueId>) -> ValueId {
    let mut remaining = replacements.len();
    while let Some(next) = replacements.get(&value).copied() {
        value = next;
        if remaining == 0 {
            break;
        }
        remaining -= 1;
    }
    value
}

pub(crate) fn remap_supported_operation(
    operation: &mut Operation,
    replacements: &BTreeMap<ValueId, ValueId>,
) -> Result<(), TargetNeutralModuleTransformErrorV1> {
    let map = |value: &mut ValueId| *value = resolve(*value, replacements);
    match &mut operation.kind {
        OperationKind::Constant(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::Barrier(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::KernelContextIssue(_) => {}
        OperationKind::Unary { operand, .. } => map(operand),
        OperationKind::Binary { lhs, rhs, .. } | OperationKind::Compare { lhs, rhs, .. } => {
            map(lhs);
            map(rhs);
        }
        OperationKind::Cast { value, .. } => map(value),
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            map(condition);
            map(true_value);
            map(false_value);
        }
        OperationKind::Call { arguments, .. } => arguments.iter_mut().for_each(map),
        OperationKind::Alloca { count, .. } => count.iter_mut().for_each(map),
        OperationKind::SliceLength { slice } | OperationKind::SliceData { slice } => map(slice),
        OperationKind::GetElementPointer { base, offset } => {
            map(base);
            map(offset);
        }
        OperationKind::Load { pointer, .. } => map(pointer),
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            ..
        } => {
            map(pointer);
            map(predicate);
            map(fallback);
        }
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value,
            ..
        } => {
            map(pointer);
            map(predicate);
            map(value);
        }
        OperationKind::Store { pointer, value, .. } => {
            map(pointer);
            map(value);
        }
        OperationKind::GlobalCapabilityBind(bind) => {
            map(&mut bind.context);
            map(&mut bind.physical);
        }
        OperationKind::GlobalCapabilityIndex(index) => {
            map(&mut index.capability);
            map(&mut index.index);
        }
        unsupported
            if unsupported
                .operands()
                .iter()
                .any(|value| replacements.contains_key(value)) =>
        {
            return Err(TargetNeutralModuleTransformErrorV1::OutputRejected);
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn remap_terminator(
    terminator: Option<&mut Terminator>,
    replacements: &BTreeMap<ValueId, ValueId>,
) {
    let Some(terminator) = terminator else {
        return;
    };
    let map = |value: &mut ValueId| *value = resolve(*value, replacements);
    match terminator {
        Terminator::Branch { arguments, .. } | Terminator::Return { values: arguments } => {
            arguments.iter_mut().for_each(map);
        }
        Terminator::ConditionalBranch {
            condition,
            then_arguments,
            else_arguments,
            ..
        } => {
            map(condition);
            then_arguments.iter_mut().for_each(map);
            else_arguments.iter_mut().for_each(map);
        }
        Terminator::Switch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            map(selector);
            for case in cases {
                case.arguments.iter_mut().for_each(map);
            }
            default_arguments.iter_mut().for_each(map);
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            map(selector);
            for case in cases {
                case.arguments.iter_mut().for_each(map);
            }
            default_arguments.iter_mut().for_each(map);
        }
        Terminator::Unreachable => {}
    }
}
