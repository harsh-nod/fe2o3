//! Closed whole-entry fill profile over the exact neutral and replayed target KIR owners.
//! This is a checked program relation, not compiler-origin, device-memory or launch authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CastKind, ComparePredicate, Constant, FunctionRole,
    IntrinsicOperation, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind,
    ScalarType, Terminator, Type, ValueId, WorkgroupSize, decode_module_v9,
};

use crate::{ValidatedCompilerTargetLineageV1, ValidatedConditionalCompilerProofInputsV1};

mod source;

/// Retains the exact compiler owners for one complete guarded index-fill program.
///
/// Neither deterministic target replay nor this recognizer authenticates the compiler producer.
/// Invocation coverage, machine execution and memory/completion remain separate obligations.
///
/// ```compile_fail
/// use fe2o3_verifier::CheckedConditionalFillProgramV1;
/// fn clone_required<T: Clone>() {}
/// clone_required::<CheckedConditionalFillProgramV1<'static>>();
/// ```
#[derive(Debug)]
#[must_use]
pub struct CheckedConditionalFillProgramV1<'a> {
    inputs: &'a ValidatedConditionalCompilerProofInputsV1,
    lineage: &'a ValidatedCompilerTargetLineageV1,
    neutral_store: (u32, u32),
    target_store: (u32, u32),
}

impl CheckedConditionalFillProgramV1<'_> {
    pub const fn inputs(&self) -> &ValidatedConditionalCompilerProofInputsV1 {
        self.inputs
    }
    pub const fn lineage(&self) -> &ValidatedCompilerTargetLineageV1 {
        self.lineage
    }
    pub fn function_symbol(&self) -> &str {
        self.lineage.replay().replay().target_bound_module().kernels[0]
            .entry
            .as_str()
    }
    pub const fn neutral_store_location(&self) -> (u32, u32) {
        self.neutral_store
    }
    pub const fn target_store_location(&self) -> (u32, u32) {
        self.target_store
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Checks both complete programs, not merely a store's safety or an optimization receipt.
pub fn check_conditional_fill_program_v1<'a>(
    inputs: &'a ValidatedConditionalCompilerProofInputsV1,
    lineage: &'a ValidatedCompilerTargetLineageV1,
) -> Result<CheckedConditionalFillProgramV1<'a>, ConditionalFillProgramErrorV1> {
    use ConditionalFillProgramErrorV1 as E;
    let replay = lineage.replay().replay();
    let association = lineage
        .semantic_to_llvm()
        .inputs()
        .map_err(|_| E::InputBinding)?;
    if inputs.kernel_ir().as_v9().is_none()
        || inputs.kernel_ir().canonical_bytes() != replay.neutral_kernel_ir_bytes()
        || association.proof_binding.sha256() != *inputs.receipt_identity().sha256()
        || association.proof_binding.byte_len() != inputs.receipt_identity().byte_len()
    {
        return Err(E::InputBinding);
    }
    let condition = inputs.verus_execution().obligation();
    if condition.reference_output_argument() != 0
        || condition.ranked_extent_argument() != 0
        || condition.allocation_origin() != 1
        || condition.noalias_class() != 2
        || condition.element_width_bits() != 32
        || condition.workgroup_extents() != [64, 1, 1]
        || condition.subgroup_size() != 64
        || condition.static_global_x_extent().is_some()
    {
        return Err(E::Profile);
    }
    let neutral = decode_module_v9(inputs.kernel_ir().canonical_bytes()).map_err(|_| E::Profile)?;
    let neutral_shape = check_module(&neutral)?;
    source::check_source(inputs, &neutral_shape, neutral.kernels[0].entry.as_str())?;
    let target = replay.target_bound_module();
    let target_shape = check_module(target)?;
    if neutral.kernels[0].entry != target.kernels[0].entry
        || neutral.kernels[0].id != target.kernels[0].id
    {
        return Err(E::InputBinding);
    }
    Ok(CheckedConditionalFillProgramV1 {
        inputs,
        lineage,
        neutral_store: neutral_shape.store,
        target_store: target_shape.store,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    Output,
    Index,
    IndexU64,
    TruncatedIndex,
    Length,
    InBounds,
    Zero,
    SafeIndex,
    Base,
    Pointer,
}

impl Value {
    fn ty(self) -> Type {
        match self {
            Self::Output => Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::WriteOnly,
            ),
            Self::Index | Self::Length | Self::Zero | Self::SafeIndex => {
                Type::Scalar(ScalarType::Index)
            }
            Self::IndexU64 => Type::Scalar(ScalarType::U64),
            Self::TruncatedIndex => Type::Scalar(ScalarType::U32),
            Self::InBounds => Type::Scalar(ScalarType::Bool),
            Self::Base | Self::Pointer => Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::WriteOnly,
            ),
        }
    }
}

#[derive(Debug)]
struct ProgramShape {
    output: ValueId,
    store: (u32, u32),
    values: BTreeMap<ValueId, Value>,
    operations: BTreeMap<(u32, u32), Option<Value>>,
}

fn check_module(module: &Module) -> Result<ProgramShape, ConditionalFillProgramErrorV1> {
    use ConditionalFillProgramErrorV1 as E;
    let [kernel] = module.kernels.as_slice() else {
        return Err(E::Profile);
    };
    let [function] = module.functions.as_slice() else {
        return Err(E::Profile);
    };
    if kernel.entry != function.id
        || kernel.id.as_str() != kernel.entry.as_str()
        || function.role != FunctionRole::KernelEntry
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        || kernel.domain
            != (LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            })
        || function.signature.parameters != [Value::Output.ty()]
        || !function.signature.results.is_empty()
    {
        return Err(E::Profile);
    }
    let body = function.body.as_ref().ok_or(E::Profile)?;
    let [output] = body.parameters.as_slice() else {
        return Err(E::Profile);
    };
    // Bound the complete profile, including neutral compiler plumbing, without dense SSA scratch.
    if body.blocks.is_empty() || body.blocks.len() > 64 {
        return Err(E::ControlFlow);
    }
    let mut blocks = BTreeMap::new();
    for block in &body.blocks {
        if !block.parameters.is_empty() || blocks.insert(block.id, block).is_some() {
            return Err(E::ControlFlow);
        }
    }
    let mut values = BTreeMap::from([(*output, Value::Output)]);
    let mut operations = BTreeMap::new();
    let mut visited = BTreeSet::new();
    let mut next = body.blocks[0].id;
    let mut store = None;
    loop {
        if !visited.insert(next) {
            return Err(E::ControlFlow);
        }
        let block = blocks.get(&next).ok_or(E::ControlFlow)?;
        for (index, operation) in block.operations.iter().enumerate() {
            if operations.len() == 64 {
                return Err(E::Profile);
            }
            let site = (block.id.0, index as u32);
            let value = check_operation(operation, &values).ok_or(E::Operation {
                block: site.0,
                operation: site.1,
            })?;
            match (value, operation.results.as_slice()) {
                (Some(value), [result]) if result.ty == value.ty() => {
                    if values.insert(result.id, value).is_some() {
                        return Err(E::ValueDefinition);
                    }
                }
                (None, []) if store.replace(site).is_none() => {}
                _ => {
                    return Err(E::Operation {
                        block: site.0,
                        operation: site.1,
                    });
                }
            }
            operations.insert(site, value);
        }
        match &block.terminator {
            Some(Terminator::Branch { target, arguments }) if arguments.is_empty() => {
                next = *target
            }
            Some(Terminator::Return { values }) if values.is_empty() => break,
            _ => return Err(E::ControlFlow),
        }
    }
    if visited.len() != blocks.len() {
        return Err(E::ControlFlow);
    }
    Ok(ProgramShape {
        output: *output,
        store: store.ok_or(E::MissingStore)?,
        values,
        operations,
    })
}

// Each accepted value has one exact denotation in terms of the selected output and global X.
// In particular, safe-address construction is not allowed to weaken the actual store predicate.
fn check_operation(
    operation: &Operation,
    values: &BTreeMap<ValueId, Value>,
) -> Option<Option<Value>> {
    let is = |id: &ValueId, expected| values.get(id) == Some(&expected);
    let value = match &operation.kind {
        OperationKind::Intrinsic(intrinsic) if *intrinsic == IntrinsicOperation::global_id_1d() => {
            Value::Index
        }
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value,
            to,
        } if is(value, Value::Index) && *to == Value::IndexU64.ty() => Value::IndexU64,
        OperationKind::Cast {
            kind: CastKind::Truncate,
            value,
            to,
        } if is(value, Value::IndexU64) && *to == Value::TruncatedIndex.ty() => {
            Value::TruncatedIndex
        }
        OperationKind::SliceLength { slice } if is(slice, Value::Output) => Value::Length,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } if is(lhs, Value::Index) && is(rhs, Value::Length) => Value::InBounds,
        OperationKind::Constant(Constant::Index(0)) => Value::Zero,
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } if is(condition, Value::InBounds)
            && is(true_value, Value::Index)
            && is(false_value, Value::Zero) =>
        {
            Value::SafeIndex
        }
        OperationKind::SliceData { slice } if is(slice, Value::Output) => Value::Base,
        OperationKind::GetElementPointer { base, offset }
            if is(base, Value::Base) && is(offset, Value::SafeIndex) =>
        {
            Value::Pointer
        }
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value,
            access,
        } if is(pointer, Value::Pointer)
            && is(predicate, Value::InBounds)
            && is(value, Value::TruncatedIndex)
            && *access == MemoryAccess::new(AddressSpace::Global, 4) =>
        {
            return Some(None);
        }
        _ => return None,
    };
    Some(Some(value))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalFillProgramErrorV1 {
    InputBinding,
    Profile,
    ControlFlow,
    ValueDefinition,
    MissingStore,
    Source(&'static str),
    Operation { block: u32, operation: u32 },
}
impl fmt::Display for ConditionalFillProgramErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional fill program rejected: {self:?}")
    }
}
impl Error for ConditionalFillProgramErrorV1 {}

#[cfg(test)]
mod tests;
