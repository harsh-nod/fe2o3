use std::mem::size_of;

use fe2o3_kernel_ir::{
    AddressSpace, AssemblyOperandKind, ExecutionCapabilityRequirementV1,
    Gfx950LdsTransposeOperationKindV1, MatrixOperationKind, MemoryIntrinsicOperation, Module,
    Operation, OperationKind, ResourceCapabilityRequirementV1, TargetCapability, Terminator, Type,
    ValueId, WaveOperationKind,
};

use crate::IncompleteExecutionCapabilityOperationV13;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LogicalCapabilityErasureError {
    AllocationFailure,
    IncompleteExecutionCapability(IncompleteExecutionCapabilityOperationV13),
    InvalidProjection(&'static str),
}

pub(crate) fn first_unsupported_execution_requirement(
    module: &Module,
    v13_execution_capabilities: bool,
) -> Option<ExecutionCapabilityRequirementV1> {
    module
        .required_capabilities
        .iter()
        .chain(
            module
                .functions
                .iter()
                .flat_map(|function| &function.required_capabilities),
        )
        .chain(
            module
                .kernels
                .iter()
                .flat_map(|kernel| &kernel.required_capabilities),
        )
        .find_map(|capability| match capability {
            TargetCapability::Execution(requirement)
                if !supports_execution_requirement(requirement, v13_execution_capabilities) =>
            {
                Some(requirement.clone())
            }
            _ => None,
        })
}

fn supports_execution_requirement(
    requirement: &ExecutionCapabilityRequirementV1,
    v13_execution_capabilities: bool,
) -> bool {
    use fe2o3_kernel_ir::{
        AsyncCopyCompletionV1, AtomicKind, CollectiveCapabilityOperationV1, MemoryOrdering,
        NumericalModeV1, ScalarType, SynchronizationScope,
    };

    match requirement {
        ExecutionCapabilityRequirementV1::AddressSpace { address_space, .. } => {
            !matches!(address_space, AddressSpace::Generic)
        }
        _ if !v13_execution_capabilities => false,
        ExecutionCapabilityRequirementV1::Atomic {
            value_type,
            operation,
            scope,
            address_space,
            ..
        } => {
            matches!(
                value_type,
                ScalarType::U32 | ScalarType::I32 | ScalarType::U64 | ScalarType::I64
            ) && matches!(
                operation,
                AtomicKind::Load
                    | AtomicKind::Store
                    | AtomicKind::Add
                    | AtomicKind::CompareExchange
            ) && !matches!(scope, SynchronizationScope::Invocation)
                && matches!(
                    address_space,
                    AddressSpace::Global | AddressSpace::Workgroup
                )
        }
        ExecutionCapabilityRequirementV1::Barrier {
            execution_scope,
            memory_scope,
            ordering,
            address_spaces,
        } => {
            matches!(
                execution_scope,
                SynchronizationScope::Subgroup | SynchronizationScope::Workgroup
            ) && memory_scope.rank() >= execution_scope.rank()
                && *ordering != MemoryOrdering::Relaxed
                && !address_spaces.is_empty()
                && address_spaces
                    .iter()
                    .all(|space| matches!(space, AddressSpace::Global | AddressSpace::Workgroup))
        }
        ExecutionCapabilityRequirementV1::Collective {
            execution_scope,
            operation,
            value_type,
            participants,
        } => {
            matches!(
                operation,
                CollectiveCapabilityOperationV1::ReduceAdd
                    | CollectiveCapabilityOperationV1::InclusiveScanAdd
                    | CollectiveCapabilityOperationV1::ExclusiveScanAdd
            ) && matches!(
                value_type,
                ScalarType::U32 | ScalarType::I32 | ScalarType::F32
            ) && match execution_scope {
                SynchronizationScope::Subgroup => matches!(participants, 32 | 64),
                SynchronizationScope::Workgroup => (1..=256).contains(participants),
                _ => false,
            }
        }
        ExecutionCapabilityRequirementV1::Matrix {
            m,
            n,
            k,
            input_type,
            accumulator_type,
        } => {
            *m == 16
                && *n == 16
                && *accumulator_type == ScalarType::F32
                && matches!(
                    (*input_type, *k),
                    (ScalarType::Bf16, 16) | (ScalarType::U8, 128)
                )
        }
        ExecutionCapabilityRequirementV1::AsyncCopy {
            source,
            destination,
            bytes,
            alignment,
            completion,
        } => {
            *source == AddressSpace::Global
                && *destination == AddressSpace::Workgroup
                && *bytes != 0
                && *alignment != 0
                && alignment.is_power_of_two()
                && matches!(completion, AsyncCopyCompletionV1::WorkgroupBarrier)
        }
        ExecutionCapabilityRequirementV1::Numerical { value_type, mode } => {
            *value_type == ScalarType::F32 && *mode == NumericalModeV1::StrictIeee
        }
        ExecutionCapabilityRequirementV1::Resource(requirement) => match requirement {
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(maximum) => {
                (1..=1_024).contains(maximum)
            }
            ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(bytes)
            | ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(bytes)
            | ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(bytes) => {
                *bytes != 0 && *bytes <= u32::MAX.into()
            }
        },
    }
}

pub(crate) fn first_incomplete_execution_capability_v13(
    _module: &Module,
) -> Option<IncompleteExecutionCapabilityOperationV13> {
    None
}

/// Projects compiler-authenticated logical authority onto the physical KIR
/// already implemented by the simulator. No logical value reaches preflight
/// or execution.
pub(crate) fn erase_logical_capabilities(
    mut module: Module,
    retain_v13_receipt: bool,
) -> Result<
    (
        Module,
        usize,
        Option<crate::execution_capability_v13::SimulationCapabilityProjectionReceiptV13>,
    ),
    LogicalCapabilityErasureError,
> {
    let (receipt, receipt_scratch_bytes) = if retain_v13_receipt {
        let (receipt, scratch) =
            crate::execution_capability_v13::record_projection_coordinates_v13(&module).map_err(
                |error| match error {
                    crate::execution_capability_v13::ExecutionCapabilityProjectionErrorV13::AllocationFailure => {
                        LogicalCapabilityErasureError::AllocationFailure
                    }
                    crate::execution_capability_v13::ExecutionCapabilityProjectionErrorV13::Incomplete(
                        operation,
                    ) => LogicalCapabilityErasureError::IncompleteExecutionCapability(operation),
                    crate::execution_capability_v13::ExecutionCapabilityProjectionErrorV13::Invalid(
                        reason,
                    ) => LogicalCapabilityErasureError::InvalidProjection(reason),
                },
            )?;
        (Some(receipt), scratch)
    } else {
        (None, 0)
    };
    let projected = crate::execution_capability_v13::project_execution_capabilities_v13(
        &mut module,
    )
    .map_err(|error| match error {
        crate::execution_capability_v13::ExecutionCapabilityProjectionErrorV13::AllocationFailure => {
            LogicalCapabilityErasureError::AllocationFailure
        }
        crate::execution_capability_v13::ExecutionCapabilityProjectionErrorV13::Incomplete(
            operation,
        ) => LogicalCapabilityErasureError::IncompleteExecutionCapability(operation),
        crate::execution_capability_v13::ExecutionCapabilityProjectionErrorV13::Invalid(reason) => {
            LogicalCapabilityErasureError::InvalidProjection(reason)
        }
    })?;
    let mut maximum_scratch_bytes = projected.scratch_bytes.max(receipt_scratch_bytes);
    for function in &mut module.functions {
        let Some(body) = function.body.as_mut() else {
            erase_signature_types(&mut function.signature)?;
            continue;
        };

        let context_count = body
            .parameters
            .iter()
            .zip(&function.signature.parameters)
            .filter(|(_, ty)| matches!(ty, Type::KernelContext(_) | Type::ExecutionCapability(_)))
            .count()
            + body
                .blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .filter(|parameter| {
                    matches!(
                        parameter.ty,
                        Type::KernelContext(_) | Type::ExecutionCapability(_)
                    )
                })
                .count()
            + body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .flat_map(|operation| &operation.results)
                .filter(|result| {
                    matches!(
                        result.ty,
                        Type::KernelContext(_) | Type::ExecutionCapability(_)
                    )
                })
                .count();
        let mut contexts = Vec::new();
        contexts
            .try_reserve_exact(context_count)
            .map_err(|_| LogicalCapabilityErasureError::AllocationFailure)?;
        contexts.extend(
            body.parameters
                .iter()
                .copied()
                .zip(&function.signature.parameters)
                .filter_map(|(value, ty)| {
                    matches!(ty, Type::KernelContext(_) | Type::ExecutionCapability(_))
                        .then_some(value)
                }),
        );
        contexts.extend(
            body.blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .filter_map(|parameter| {
                    matches!(
                        parameter.ty,
                        Type::KernelContext(_) | Type::ExecutionCapability(_)
                    )
                    .then_some(parameter.id)
                }),
        );
        contexts.extend(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .flat_map(|operation| &operation.results)
                .filter_map(|result| {
                    matches!(
                        result.ty,
                        Type::KernelContext(_) | Type::ExecutionCapability(_)
                    )
                    .then_some(result.id)
                }),
        );
        contexts.sort_unstable();
        contexts.dedup();

        let alias_count = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::GlobalCapabilityBind(_)
                        | OperationKind::GlobalCapabilityIndex(_)
                )
            })
            .count()
            + projected.aliases.get(&function.id).map_or(0, Vec::len);
        let mut aliases = Vec::new();
        aliases
            .try_reserve_exact(alias_count)
            .map_err(|_| LogicalCapabilityErasureError::AllocationFailure)?;
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            let target = match operation.kind {
                OperationKind::GlobalCapabilityBind(bind) => Some(bind.physical),
                OperationKind::GlobalCapabilityIndex(index) => Some(index.index),
                _ => None,
            };
            if let Some(target) = target {
                let [result] = operation.results.as_slice() else {
                    return Err(LogicalCapabilityErasureError::InvalidProjection(
                        "logical capability operation has invalid result arity",
                    ));
                };
                aliases.push((result.id, target));
            }
        }
        if let Some(projected_aliases) = projected.aliases.get(&function.id) {
            aliases.extend(projected_aliases.iter().copied());
        }
        aliases.sort_unstable_by_key(|(source, _)| *source);
        if aliases.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(LogicalCapabilityErasureError::InvalidProjection(
                "logical capability alias is defined more than once",
            ));
        }

        maximum_scratch_bytes = maximum_scratch_bytes.max(
            contexts
                .capacity()
                .checked_mul(size_of::<ValueId>())
                .and_then(|bytes| {
                    aliases
                        .capacity()
                        .checked_mul(size_of::<(ValueId, ValueId)>())
                        .and_then(|alias_bytes| bytes.checked_add(alias_bytes))
                })
                .ok_or(LogicalCapabilityErasureError::AllocationFailure)?,
        );

        let is_context = |value: &ValueId| contexts.binary_search(value).is_ok();
        erase_signature_types(&mut function.signature)?;
        body.parameters.retain(|value| !is_context(value));

        for block in &mut body.blocks {
            block.parameters.retain(|parameter| {
                !matches!(
                    parameter.ty,
                    Type::KernelContext(_) | Type::ExecutionCapability(_)
                )
            });
            for parameter in &mut block.parameters {
                erase_runtime_type(&mut parameter.ty)?;
            }

            block.operations.retain(|operation| {
                !matches!(
                    operation.kind,
                    OperationKind::KernelContextIssue(_)
                        | OperationKind::GlobalCapabilityBind(_)
                        | OperationKind::GlobalCapabilityIndex(_)
                )
            });
            for operation in &mut block.operations {
                for result in &mut operation.results {
                    erase_runtime_type(&mut result.ty)?;
                }
                if let OperationKind::Call { arguments, .. } = &mut operation.kind {
                    arguments.retain(|argument| !is_context(argument));
                }
                rewrite_operation_operands(operation, &aliases)?;
            }
            if let Some(terminator) = &mut block.terminator {
                erase_context_arguments(terminator, is_context);
                rewrite_terminator_operands(terminator, &aliases)?;
            }
        }
    }

    if contains_logical_capability(&module) {
        return Err(LogicalCapabilityErasureError::InvalidProjection(
            "logical capability survived simulator projection",
        ));
    }
    Ok((module, maximum_scratch_bytes, receipt))
}

fn erase_signature_types(
    signature: &mut fe2o3_kernel_ir::Signature,
) -> Result<(), LogicalCapabilityErasureError> {
    signature
        .parameters
        .retain(|ty| !matches!(ty, Type::KernelContext(_)));
    for ty in &mut signature.parameters {
        erase_runtime_type(ty)?;
    }
    for ty in &mut signature.results {
        erase_runtime_type(ty)?;
    }
    Ok(())
}

fn erase_runtime_type(ty: &mut Type) -> Result<(), LogicalCapabilityErasureError> {
    match ty {
        Type::Unit | Type::Scalar(_) => Ok(()),
        Type::Pointer(pointer) => erase_runtime_type(&mut pointer.pointee),
        Type::Slice(slice) => erase_runtime_type(&mut slice.element),
        Type::GlobalCapability(capability) => {
            *ty = capability.physical_slice_type();
            Ok(())
        }
        Type::KernelContext(_) | Type::ExecutionCapability(_) => {
            Err(LogicalCapabilityErasureError::InvalidProjection(
                "logical capability appears in an executable type position",
            ))
        }
    }
}

fn rewrite_value(
    value: &mut ValueId,
    aliases: &[(ValueId, ValueId)],
) -> Result<(), LogicalCapabilityErasureError> {
    for _ in 0..=aliases.len() {
        let Ok(index) = aliases.binary_search_by_key(value, |(source, _)| *source) else {
            return Ok(());
        };
        *value = aliases[index].1;
    }
    Err(LogicalCapabilityErasureError::InvalidProjection(
        "logical capability aliases form a cycle",
    ))
}

fn rewrite_values(
    values: &mut [ValueId],
    aliases: &[(ValueId, ValueId)],
) -> Result<(), LogicalCapabilityErasureError> {
    for value in values {
        rewrite_value(value, aliases)?;
    }
    Ok(())
}

fn rewrite_operation_operands(
    operation: &mut Operation,
    aliases: &[(ValueId, ValueId)],
) -> Result<(), LogicalCapabilityErasureError> {
    match &mut operation.kind {
        OperationKind::Constant(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::Barrier(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_) => {}
        OperationKind::MemoryIntrinsic(intrinsic) => match intrinsic {
            MemoryIntrinsicOperation::PointerDistance {
                pointer, origin, ..
            } => {
                rewrite_value(pointer, aliases)?;
                rewrite_value(origin, aliases)?;
            }
            MemoryIntrinsicOperation::VolatileLoad { pointer, .. } => {
                rewrite_value(pointer, aliases)?
            }
            MemoryIntrinsicOperation::VolatileStore { pointer, value, .. } => {
                rewrite_value(pointer, aliases)?;
                rewrite_value(value, aliases)?;
            }
            MemoryIntrinsicOperation::CopyNonOverlapping {
                source,
                destination,
                count,
                ..
            } => {
                rewrite_value(source, aliases)?;
                rewrite_value(destination, aliases)?;
                rewrite_value(count, aliases)?;
            }
        },
        OperationKind::Unary { operand, .. } => rewrite_value(operand, aliases)?,
        OperationKind::Binary { lhs, rhs, .. } | OperationKind::Compare { lhs, rhs, .. } => {
            rewrite_value(lhs, aliases)?;
            rewrite_value(rhs, aliases)?;
        }
        OperationKind::Cast { value, to, .. } => {
            rewrite_value(value, aliases)?;
            erase_runtime_type(to)?;
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            rewrite_value(condition, aliases)?;
            rewrite_value(true_value, aliases)?;
            rewrite_value(false_value, aliases)?;
        }
        OperationKind::Call { arguments, .. } => rewrite_values(arguments, aliases)?,
        OperationKind::Alloca { element, count, .. } => {
            erase_runtime_type(element)?;
            if let Some(count) = count {
                rewrite_value(count, aliases)?;
            }
        }
        OperationKind::SliceLength { slice } | OperationKind::SliceData { slice } => {
            rewrite_value(slice, aliases)?
        }
        OperationKind::GetElementPointer { base, offset } => {
            rewrite_value(base, aliases)?;
            rewrite_value(offset, aliases)?;
        }
        OperationKind::Load { pointer, .. } => rewrite_value(pointer, aliases)?,
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            ..
        } => {
            rewrite_value(pointer, aliases)?;
            rewrite_value(predicate, aliases)?;
            rewrite_value(fallback, aliases)?;
        }
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value,
            ..
        } => {
            rewrite_value(pointer, aliases)?;
            rewrite_value(predicate, aliases)?;
            rewrite_value(value, aliases)?;
        }
        OperationKind::Store { pointer, value, .. } => {
            rewrite_value(pointer, aliases)?;
            rewrite_value(value, aliases)?;
        }
        OperationKind::Atomic(atomic) => {
            rewrite_value(&mut atomic.pointer, aliases)?;
            if let Some(value) = &mut atomic.value {
                rewrite_value(value, aliases)?;
            }
            if let Some(compare) = &mut atomic.compare {
                rewrite_value(compare, aliases)?;
            }
        }
        OperationKind::Matrix(matrix) => match &mut matrix.kind {
            MatrixOperationKind::MultiplyAccumulate {
                lhs,
                rhs,
                accumulator,
                ..
            } => {
                rewrite_values(lhs, aliases)?;
                rewrite_values(rhs, aliases)?;
                rewrite_values(accumulator, aliases)?;
            }
            MatrixOperationKind::ScaledMultiplyAccumulate {
                lhs,
                rhs,
                accumulator,
                ..
            } => {
                rewrite_values(lhs, aliases)?;
                rewrite_values(rhs, aliases)?;
                rewrite_values(accumulator, aliases)?;
            }
            MatrixOperationKind::LdsLoad { base, .. } => rewrite_value(base, aliases)?,
            MatrixOperationKind::LdsStore { base, values, .. } => {
                rewrite_value(base, aliases)?;
                rewrite_values(values, aliases)?;
            }
        },
        OperationKind::Gfx950LdsTranspose(transpose) => match &mut transpose.kind {
            Gfx950LdsTransposeOperationKindV1::Current { .. } => {}
            Gfx950LdsTransposeOperationKindV1::Stage {
                storage,
                source_slice,
                offset,
                rows,
                columns,
                stride,
                token_base,
                reduction_base,
                ..
            } => {
                rewrite_value(storage, aliases)?;
                rewrite_value(source_slice, aliases)?;
                rewrite_value(offset, aliases)?;
                rewrite_value(rows, aliases)?;
                rewrite_value(columns, aliases)?;
                rewrite_value(stride, aliases)?;
                rewrite_value(token_base, aliases)?;
                rewrite_value(reduction_base, aliases)?;
            }
            Gfx950LdsTransposeOperationKindV1::Publish { storage, .. }
            | Gfx950LdsTransposeOperationKindV1::Read { storage, .. } => {
                rewrite_value(storage, aliases)?
            }
        },
        OperationKind::Wave(wave) => match &mut wave.kind {
            WaveOperationKind::LaneId => {}
            WaveOperationKind::Ballot { predicate }
            | WaveOperationKind::Any { predicate }
            | WaveOperationKind::All { predicate } => rewrite_value(predicate, aliases)?,
            WaveOperationKind::ShuffleIndex {
                value, source_lane, ..
            }
            | WaveOperationKind::BroadcastF32 {
                value, source_lane, ..
            } => {
                rewrite_value(value, aliases)?;
                rewrite_value(source_lane, aliases)?;
            }
            WaveOperationKind::ReduceF32 { value, .. } => rewrite_value(value, aliases)?,
        },
        OperationKind::InlineAssembly(assembly) => {
            for operand in &mut assembly.operands {
                match &mut operand.kind {
                    AssemblyOperandKind::Input(value)
                    | AssemblyOperandKind::InOut { input: value, .. } => {
                        rewrite_value(value, aliases)?
                    }
                    AssemblyOperandKind::Output { .. } | AssemblyOperandKind::ImmediateI32(_) => {}
                }
            }
        }
        OperationKind::KernelContextIssue(_)
        | OperationKind::GlobalCapabilityBind(_)
        | OperationKind::GlobalCapabilityIndex(_)
        | OperationKind::ExecutionCapability(_) => {
            return Err(LogicalCapabilityErasureError::InvalidProjection(
                "logical operation survived projection",
            ));
        }
    }
    Ok(())
}

fn erase_context_arguments(terminator: &mut Terminator, is_context: impl Fn(&ValueId) -> bool) {
    match terminator {
        Terminator::Branch { arguments, .. } => arguments.retain(|value| !is_context(value)),
        Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        } => {
            then_arguments.retain(|value| !is_context(value));
            else_arguments.retain(|value| !is_context(value));
        }
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } => {
            for case in cases {
                case.arguments.retain(|value| !is_context(value));
            }
            default_arguments.retain(|value| !is_context(value));
        }
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } => {
            for case in cases {
                case.arguments.retain(|value| !is_context(value));
            }
            default_arguments.retain(|value| !is_context(value));
        }
        Terminator::Return { values } => values.retain(|value| !is_context(value)),
        Terminator::Unreachable => {}
    }
}

fn rewrite_terminator_operands(
    terminator: &mut Terminator,
    aliases: &[(ValueId, ValueId)],
) -> Result<(), LogicalCapabilityErasureError> {
    match terminator {
        Terminator::Branch { arguments, .. } => rewrite_values(arguments, aliases)?,
        Terminator::ConditionalBranch {
            condition,
            then_arguments,
            else_arguments,
            ..
        } => {
            rewrite_value(condition, aliases)?;
            rewrite_values(then_arguments, aliases)?;
            rewrite_values(else_arguments, aliases)?;
        }
        Terminator::Switch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            rewrite_value(selector, aliases)?;
            for case in cases {
                rewrite_values(&mut case.arguments, aliases)?;
            }
            rewrite_values(default_arguments, aliases)?;
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_arguments,
            ..
        } => {
            rewrite_value(selector, aliases)?;
            for case in cases {
                rewrite_values(&mut case.arguments, aliases)?;
            }
            rewrite_values(default_arguments, aliases)?;
        }
        Terminator::Return { values } => rewrite_values(values, aliases)?,
        Terminator::Unreachable => {}
    }
    Ok(())
}

fn contains_logical_capability(module: &Module) -> bool {
    module.functions.iter().any(|function| {
        function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
            .any(Type::contains_logical_capability)
            || function.body.iter().any(|body| {
                body.blocks.iter().any(|block| {
                    block
                        .parameters
                        .iter()
                        .any(|parameter| parameter.ty.contains_logical_capability())
                        || block
                            .operations
                            .iter()
                            .any(operation_contains_logical_capability)
                })
            })
    })
}

fn operation_contains_logical_capability(operation: &Operation) -> bool {
    operation
        .results
        .iter()
        .any(|result| result.ty.contains_logical_capability())
        || match &operation.kind {
            OperationKind::KernelContextIssue(_)
            | OperationKind::GlobalCapabilityBind(_)
            | OperationKind::GlobalCapabilityIndex(_)
            | OperationKind::ExecutionCapability(_) => true,
            OperationKind::Intrinsic(intrinsic) => {
                intrinsic.result_type.contains_logical_capability()
            }
            OperationKind::Cast { to, .. } => to.contains_logical_capability(),
            OperationKind::Alloca { element, .. } => element.contains_logical_capability(),
            OperationKind::WorkgroupMemory(memory) => memory.element.contains_logical_capability(),
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        AccessMode, BasicBlock, BlockId, Function, Kernel, KernelContextSourceIdentityV1,
        KernelContextTypeV1, LaunchDomain, LaunchExtent, MemoryAccess, Operation, Signature,
        ValueDef, verify_module,
    };

    use super::*;

    #[test]
    fn global_authority_projects_to_existing_slice_and_index_values() {
        let scalar = Type::Scalar(fe2o3_kernel_ir::ScalarType::U32);
        let physical = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
        let pointer = Type::pointer(scalar, AddressSpace::Global, AccessMode::ReadOnly);
        let context = KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32]);
        let capability = fe2o3_kernel_ir::GlobalCapabilityTypeV1::read_only(
            Type::Scalar(fe2o3_kernel_ir::ScalarType::U32),
            context.clone(),
        );
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::kernel_context_issue(
                ValueId(2),
                context,
                KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
            ),
            Operation::global_capability_bind(ValueId(3), capability, ValueId(2), ValueId(0)),
            Operation::global_capability_index(ValueId(4), ValueId(3), ValueId(1), None),
            Operation::effect_free(
                ValueDef::new(ValueId(5), pointer.clone()),
                OperationKind::SliceData { slice: ValueId(3) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(6), pointer),
                OperationKind::GetElementPointer {
                    base: ValueId(5),
                    offset: ValueId(4),
                },
            ),
            Operation::new(
                vec![ValueDef::new(
                    ValueId(7),
                    Type::Scalar(fe2o3_kernel_ir::ScalarType::U32),
                )],
                OperationKind::Load {
                    pointer: ValueId(6),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::kernel_entry(
            "entry",
            Signature::new(vec![physical, Type::INDEX], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        );
        let mut module = Module::new("logical-projection");
        module.functions.push(function);
        module.kernels.push(Kernel::new(
            "logical-projection",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));

        verify_module(&module).unwrap();
        let (module, scratch, receipt) = erase_logical_capabilities(module, false).unwrap();
        assert!(receipt.is_none());
        verify_module(&module).unwrap();
        assert!(scratch >= 3 * size_of::<ValueId>());
        let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
        assert_eq!(operations.len(), 3);
        assert!(matches!(
            operations[0].kind,
            OperationKind::SliceData { slice: ValueId(0) }
        ));
        assert!(matches!(
            operations[1].kind,
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(1)
            }
        ));
    }
}
