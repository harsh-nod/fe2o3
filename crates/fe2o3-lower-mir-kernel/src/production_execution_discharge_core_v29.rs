//! One raw lifecycle-erasure engine shared by typed compatibility and V18 wrappers.
use super::{
    BasicBlock, Budget, ExecutionOperationV15, Function, FunctionBody, Module, OperationKind,
    ProductionExecutionErasureKindV29, ProductionExecutionErasureV29, ResourceError,
};

#[derive(Debug)]
pub(super) enum RuleError {
    Resource(ResourceError),
    NoExecution,
    UnsupportedExecution,
    ReplayMismatch,
}
impl From<ResourceError> for RuleError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum TableProfile {
    LegacyEmpty,
    Complete,
}

fn erasure_kind(
    kind: &OperationKind,
) -> Result<Option<ProductionExecutionErasureKindV29>, RuleError> {
    use ProductionExecutionErasureKindV29 as Kind;
    match kind {
        OperationKind::Execution(ExecutionOperationV15::ContextIssue) => {
            Ok(Some(Kind::ContextIssue))
        }
        OperationKind::Execution(ExecutionOperationV15::WorkgroupDerive { .. }) => {
            Ok(Some(Kind::WorkgroupDerive))
        }
        OperationKind::Execution(ExecutionOperationV15::ScopeEnd { discarded, .. })
            if discarded.is_empty() =>
        {
            Ok(Some(Kind::ScopeEnd))
        }
        OperationKind::Execution(_) => Err(RuleError::UnsupportedExecution),
        _ => Ok(None),
    }
}

pub(super) fn preflight(
    input: &Module,
    canonical_bytes: usize,
    budget: &mut Budget<'_>,
) -> Result<usize, RuleError> {
    // The exact encoding bounds all vector walks, including empty containers.
    budget.charge_work(canonical_bytes)?;
    let mut count = 0usize;
    for function in &input.functions {
        if let Some(body) = &function.body {
            for block in &body.blocks {
                for operation in &block.operations {
                    if erasure_kind(&operation.kind)?.is_some() {
                        count = count.checked_add(1).ok_or(ResourceError::Arithmetic)?;
                    }
                }
            }
        }
    }
    if count == 0 {
        return Err(RuleError::NoExecution);
    }
    Ok(count)
}

pub(super) fn erase(
    candidate: &mut Module,
    canonical_bytes: usize,
    count: usize,
    erased_operations: &mut Vec<ProductionExecutionErasureV29>,
    budget: &mut Budget<'_>,
) -> Result<(), RuleError> {
    budget.charge_work(canonical_bytes)?;
    for (function_ordinal, function) in candidate.functions.iter_mut().enumerate() {
        if let Some(body) = &mut function.body {
            for (block_ordinal, block) in body.blocks.iter_mut().enumerate() {
                let mut operation_ordinal = 0;
                block.operations.retain(|operation| {
                    let ordinal = operation_ordinal;
                    operation_ordinal += 1;
                    // Preflight checked the immutable inverse before any mutation.
                    if let Ok(Some(kind)) = erasure_kind(&operation.kind) {
                        erased_operations.push(ProductionExecutionErasureV29 {
                            function_ordinal,
                            block_ordinal,
                            operation_ordinal: ordinal,
                            kind,
                        });
                        false
                    } else {
                        true
                    }
                });
            }
        }
    }
    if erased_operations.len() != count {
        return Err(RuleError::ReplayMismatch);
    }
    Ok(())
}

pub(super) fn replay(
    input: &Module,
    input_bytes: usize,
    output: &Module,
    output_bytes: usize,
    rows: &[ProductionExecutionErasureV29],
    profile: TableProfile,
    budget: &mut Budget<'_>,
) -> Result<(), RuleError> {
    let work = input_bytes
        .checked_add(output_bytes)
        .and_then(|value| value.checked_add(rows.len()))
        .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(work)?;
    // Exhaustive destructuring makes newly added IR fields require a replay decision.
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
        storage_layouts,
    } = input;
    let Module {
        id: out_id,
        functions: out_functions,
        kernels: out_kernels,
        required_capabilities: out_capabilities,
        storage_layouts: output_storage_layouts,
    } = output;
    if (profile == TableProfile::LegacyEmpty
        && !super::execution_discharge_legacy_storage_tables_v29(
            storage_layouts,
            output_storage_layouts,
        ))
        || storage_layouts != output_storage_layouts
    {
        return Err(RuleError::ReplayMismatch);
    }
    if (id, kernels, required_capabilities) != (out_id, out_kernels, out_capabilities)
        || functions.len() != out_functions.len()
    {
        return Err(RuleError::ReplayMismatch);
    }
    let mut remaining = rows.iter();
    for (function_ordinal, (function, out_function)) in
        functions.iter().zip(out_functions).enumerate()
    {
        let Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = function;
        let Function {
            id: out_id,
            signature: out_signature,
            role: out_role,
            body: out_body,
            required_capabilities: out_capabilities,
        } = out_function;
        if (id, signature, role, required_capabilities)
            != (out_id, out_signature, out_role, out_capabilities)
        {
            return Err(RuleError::ReplayMismatch);
        }
        let (body, out_body) = match (body, out_body) {
            (Some(body), Some(out_body)) => (body, out_body),
            (None, None) => continue,
            _ => return Err(RuleError::ReplayMismatch),
        };
        let FunctionBody { parameters, blocks } = body;
        let FunctionBody {
            parameters: out_parameters,
            blocks: out_blocks,
        } = out_body;
        if parameters != out_parameters || blocks.len() != out_blocks.len() {
            return Err(RuleError::ReplayMismatch);
        }
        for (block_ordinal, (block, out_block)) in blocks.iter().zip(out_blocks).enumerate() {
            let BasicBlock {
                id,
                parameters,
                operations,
                terminator,
            } = block;
            let BasicBlock {
                id: out_id,
                parameters: out_parameters,
                operations: out_operations,
                terminator: out_terminator,
            } = out_block;
            if (id, parameters, terminator) != (out_id, out_parameters, out_terminator) {
                return Err(RuleError::ReplayMismatch);
            }
            let mut physical = out_operations.iter();
            for (operation_ordinal, operation) in operations.iter().enumerate() {
                // Deliberately independent of the producer's erasure classifier.
                let kind = match &operation.kind {
                    OperationKind::Execution(ExecutionOperationV15::ContextIssue) => {
                        ProductionExecutionErasureKindV29::ContextIssue
                    }
                    OperationKind::Execution(ExecutionOperationV15::WorkgroupDerive { .. }) => {
                        ProductionExecutionErasureKindV29::WorkgroupDerive
                    }
                    OperationKind::Execution(ExecutionOperationV15::ScopeEnd {
                        discarded, ..
                    }) if discarded.is_empty() => ProductionExecutionErasureKindV29::ScopeEnd,
                    OperationKind::Execution(_) => return Err(RuleError::ReplayMismatch),
                    _ => {
                        if physical.next() != Some(operation) {
                            return Err(RuleError::ReplayMismatch);
                        }
                        continue;
                    }
                };
                let expected = ProductionExecutionErasureV29 {
                    function_ordinal,
                    block_ordinal,
                    operation_ordinal,
                    kind,
                };
                if remaining.next() != Some(&expected) {
                    return Err(RuleError::ReplayMismatch);
                }
            }
            if physical.next().is_some() {
                return Err(RuleError::ReplayMismatch);
            }
        }
    }
    if remaining.next().is_some() || rows.is_empty() {
        return Err(RuleError::ReplayMismatch);
    }
    Ok(())
}
