use std::collections::BTreeSet;

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1, Diagnostic,
    DiagnosticCode, DiagnosticLocation, MeteredKernelIrVerificationErrorV1, Module, OperationKind,
    TargetCapability, Type, VerificationDiagnosticLocationV1, VerificationErrors,
    VerifiedKernelIrModuleV1, verify_depth_bounded_module_with_budget_v1,
};

pub(crate) const PUBLIC_VERIFIER_TYPE_DEPTH_MESSAGE_V1: &str =
    "type nesting exceeds the canonical verifier limit of 64";

pub(crate) fn verify_public_module_with_shared_engine_v1<'module>(
    module: &'module Module,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
) -> Result<VerifiedKernelIrModuleV1<'module>, VerificationErrors> {
    if let Some(location) = first_excessive_type_depth_location_v1(module) {
        return Err(single_resource_diagnostic_v1(
            location,
            PUBLIC_VERIFIER_TYPE_DEPTH_MESSAGE_V1.to_owned(),
        ));
    }

    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    match verify_depth_bounded_module_with_budget_v1(module, supported_capabilities, &mut budget) {
        Ok(verified) => Ok(verified),
        Err(MeteredKernelIrVerificationErrorV1::Verification(error)) => Err(error),
        Err(MeteredKernelIrVerificationErrorV1::Resource(error)) => {
            Err(single_resource_diagnostic_v1(
                DiagnosticLocation::module(module),
                format!("kernel IR verification resource failure: {error}"),
            ))
        }
    }
}

fn single_resource_diagnostic_v1(
    location: DiagnosticLocation,
    message: String,
) -> VerificationErrors {
    VerificationErrors::from_sorted_diagnostics_v1(vec![Diagnostic {
        location,
        code: DiagnosticCode::ResourceLimit,
        message,
    }])
}

fn first_excessive_type_depth_location_v1(module: &Module) -> Option<DiagnosticLocation> {
    match first_excessive_type_depth_location_with_visits_v1(module, &mut |_| {
        Ok::<(), std::convert::Infallible>(())
    }) {
        Ok(location) => location.map(|location| DiagnosticLocation {
            module: location.module.clone(),
            function: location.function.cloned(),
            kernel: location.kernel.cloned(),
            block: location.block,
            operation: location.operation,
        }),
        Err(error) => match error {},
    }
}

// The legacy entry uses an infallible no-op visitor. The borrowed budgeted
// entry prepays the same iterative traversal, without duplicating its checks.
pub(crate) fn first_excessive_type_depth_location_with_visits_v1<'m, E>(
    module: &'m Module,
    visit: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<VerificationDiagnosticLocationV1<'m>>, E> {
    visit(1)?;
    for function in &module.functions {
        visit(1)?;
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            visit(1)?;
            if type_exceeds_public_verifier_depth_v1(ty, visit)? {
                visit(5)?;
                return Ok(Some(VerificationDiagnosticLocationV1::function(
                    module, function,
                )));
            }
        }
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            visit(1)?;
            for parameter in &block.parameters {
                visit(1)?;
                if type_exceeds_public_verifier_depth_v1(&parameter.ty, visit)? {
                    visit(5)?;
                    return Ok(Some(
                        VerificationDiagnosticLocationV1::function(module, function)
                            .at_block(block.id),
                    ));
                }
            }
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                visit(1)?;
                let mut excessive_result = false;
                for result in &operation.results {
                    visit(1)?;
                    if type_exceeds_public_verifier_depth_v1(&result.ty, visit)? {
                        excessive_result = true;
                        break;
                    }
                }
                if excessive_result
                    || operation_kind_exceeds_public_verifier_depth_v1(&operation.kind, visit)?
                {
                    visit(5)?;
                    return Ok(Some(
                        VerificationDiagnosticLocationV1::function(module, function)
                            .at_block(block.id)
                            .at_operation(operation_ordinal),
                    ));
                }
            }
        }
    }
    Ok(None)
}

fn operation_kind_exceeds_public_verifier_depth_v1<E>(
    kind: &OperationKind,
    visit: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    visit(1)?;
    let ty = match kind {
        OperationKind::Intrinsic(intrinsic) => Some(&intrinsic.result_type),
        OperationKind::Cast { to, .. } | OperationKind::Alloca { element: to, .. } => Some(to),
        OperationKind::WorkgroupMemory(memory) => Some(&memory.element),
        OperationKind::Constant(_)
        | OperationKind::VerificationContract(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Select { .. }
        | OperationKind::Call { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Store { .. }
        | OperationKind::VectorLoad(_)
        | OperationKind::VectorStore(_)
        | OperationKind::VectorLayoutConvert(_)
        | OperationKind::Barrier(_)
        | OperationKind::Atomic(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::Wave(_)
        | OperationKind::InlineAssembly(_)
        | OperationKind::Gfx942OrderedRegion(_)
        | OperationKind::Gfx942OrderedProgram(_)
        | OperationKind::Gfx942CompleteBodyDeclaration(_)
        | OperationKind::Gfx942CompleteBodyStep(_)
        | OperationKind::Gfx942PhysicalEntryDeclaration(_)
        | OperationKind::Gfx942PhysicalEntryStep(_)
        | OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
        | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
        | OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
        | OperationKind::Gfx942PhysicalLdsExchangeStep(_)
        | OperationKind::Execution(_) => None,
    };
    match ty {
        Some(ty) => type_exceeds_public_verifier_depth_v1(ty, visit),
        None => Ok(false),
    }
}

fn type_exceeds_public_verifier_depth_v1<E>(
    mut ty: &Type,
    visit: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    let mut depth = 0_usize;
    loop {
        visit(1)?;
        if depth > crate::MAX_TYPE_DEPTH_V1 {
            return Ok(true);
        }
        match ty {
            Type::Pointer(pointer) => ty = &pointer.pointee,
            Type::Slice(slice) => ty = &slice.element,
            Type::Unit | Type::Scalar(_) | Type::Vector(_) | Type::Execution(_) => {
                return Ok(false);
            }
        }
        depth += 1;
    }
}

#[cfg(test)]
#[path = "verification_public_preflight_v1_tests.rs"]
mod tests;
