//! Exact existing reserved Exp contract, not a general external-call admission.
use super::*;
use fe2o3_kernel_ir::{
    F32MathFunction, F32MathImplementation, FloatOperation, Function, FunctionRole,
};

#[cfg(test)]
#[path = "production_checked_output_exp_v1_tests.rs"]
mod tests;

fn descriptor(
    callee: &fe2o3_kernel_ir::FunctionId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    Ok(matches!(
        FloatOperation::f32_math_descriptor_with_budget_v1(callee, budget).map_err(E::Resource)?,
        Some((F32MathFunction::Exp, F32MathImplementation::OcmlAbiV1))
    ))
}

// Inputs are actual borrowed rows of the verified B/O inventory. The inert
// descriptor lookup alone is not source correspondence or call authority.
pub(super) fn declaration(function: &Function, budget: &mut AssertOriginBudgetV1<'_>) -> R<bool> {
    charge(budget, 4)?;
    if function.role != FunctionRole::ExternalImport
        || function.body.is_some()
        || function.signature.parameters.as_slice() != [Type::F32]
        || function.signature.results.as_slice() != [Type::F32]
        || !function.required_capabilities.is_empty()
    {
        return Ok(false);
    }
    descriptor(&function.id, budget)
}

pub(super) fn call(operation: &Operation, budget: &mut AssertOriginBudgetV1<'_>) -> R<bool> {
    let OperationKind::Call { callee, arguments } = &operation.kind else {
        return Ok(false);
    };
    // Existing caller row charges cover this fixed shape guard. In particular,
    // canonical zero-argument assertion traps retain their previous work path.
    if arguments.len() != 1
        || !matches!(operation.results.as_slice(), [result] if result.ty == Type::F32)
    {
        return Ok(false);
    }
    charge(budget, 2)?;
    descriptor(callee, budget)
}
