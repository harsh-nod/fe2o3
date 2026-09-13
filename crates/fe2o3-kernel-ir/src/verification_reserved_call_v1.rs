use crate::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AmdGpuDiagnosticIntrinsicDescriptorV1, AmdGpuDiagnosticOperation,
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    DiagnosticCode, FloatIntrinsicCapabilityV1, FloatIntrinsicDescriptorV1, FloatOperation,
    Function, FunctionId, FunctionRole, ScalarType, TargetCapability, Type,
    VerificationDiagnosticCollectorV1, VerificationDiagnosticLocationV1,
    clone_diagnostic_location_v1, emit_dynamic_v1, identifier_message_work_v1,
    verification_types_equal_v1,
};

const DIAGNOSTIC_PREFIX_V1: &str = "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_";
const FLOAT_PREFIX_V1: &str = "__fe2o3_ir_float_v1_";

fn charge_prefix_comparisons_v1(
    id: &FunctionId,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(
        id.as_str()
            .len()
            .checked_add(1)
            .and_then(|work| work.checked_mul(2))
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )
}

fn charge_diagnostic_lookup_v1(
    id: &FunctionId,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(
        AmdGpuDiagnosticOperation::intrinsic_descriptor_lookup_work_v1(id)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )
}

fn charge_float_lookup_v1(
    id: &FunctionId,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(
        FloatOperation::intrinsic_descriptor_lookup_work_v1(id)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )
}

pub(crate) fn verify_reserved_function_declaration_v1(
    function: &Function,
    location: &VerificationDiagnosticLocationV1<'_>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    charge_prefix_comparisons_v1(&function.id, budget)?;
    if function.id.as_str().starts_with(DIAGNOSTIC_PREFIX_V1) {
        charge_diagnostic_lookup_v1(&function.id, budget)?;
        let valid = match AmdGpuDiagnosticOperation::intrinsic_descriptor_v1(&function.id) {
            Some(descriptor) => diagnostic_declaration_matches_v1(function, descriptor, budget)?,
            None => false,
        };
        if !valid {
            emit_dynamic_v1(
                diagnostics,
                clone_diagnostic_location_v1(location, budget)?,
                DiagnosticCode::InvalidAmdGpuDiagnosticOperation,
                identifier_message_work_v1(function.id.as_str().len(), 256)?,
                format_args!(
                    "reserved AMDGPU diagnostic intrinsic {} must have its exact canonical declaration",
                    function.id
                ),
                budget,
            )?;
        }
    }
    if function.id.as_str().starts_with(FLOAT_PREFIX_V1) {
        charge_float_lookup_v1(&function.id, budget)?;
        let valid = match FloatOperation::intrinsic_descriptor_v1(&function.id) {
            Some(descriptor) => float_declaration_matches_v1(function, descriptor, budget)?,
            None => false,
        };
        if !valid {
            emit_dynamic_v1(
                diagnostics,
                clone_diagnostic_location_v1(location, budget)?,
                DiagnosticCode::InvalidFloatOperation,
                identifier_message_work_v1(function.id.as_str().len(), 192)?,
                format_args!(
                    "reserved float intrinsic {} must have its exact canonical declaration",
                    function.id
                ),
                budget,
            )?;
        }
    }
    Ok(())
}

fn diagnostic_declaration_matches_v1(
    function: &Function,
    descriptor: AmdGpuDiagnosticIntrinsicDescriptorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(
        descriptor
            .arity()
            .checked_add(6)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )?;
    let capability_matches = if function.required_capabilities.len() == 1 {
        let capability = function
            .required_capabilities
            .iter()
            .next()
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        let comparison_work = match capability {
            TargetCapability::Extension { namespace, name } => namespace
                .len()
                .checked_add(name.len())
                .and_then(|work| work.checked_add(4))
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            _ => 1,
        };
        budget.charge_work(comparison_work)?;
        matches!(
            capability,
            TargetCapability::Extension { namespace, name }
                if (namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                    && name == AMDGPU_DIAGNOSTICS_CAPABILITY_NAME)
                    || (namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                        && name == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME)
        )
    } else {
        false
    };
    Ok(function.role == FunctionRole::ExternalImport
        && function.body.is_none()
        && function.signature.parameters.len() == descriptor.arity()
        && function
            .signature
            .parameters
            .iter()
            .all(|ty| *ty == Type::Scalar(ScalarType::U32))
        && function.signature.results.len() == usize::from(descriptor.has_result())
        && function
            .signature
            .results
            .first()
            .is_none_or(|ty| *ty == Type::Scalar(ScalarType::U32))
        && capability_matches)
}

fn float_declaration_matches_v1(
    function: &Function,
    descriptor: FloatIntrinsicDescriptorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(
        descriptor
            .arity()
            .checked_add(7)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
    )?;
    if function.role != FunctionRole::ExternalImport
        || function.body.is_some()
        || function.signature.parameters.len() != descriptor.arity()
        || function.signature.results.len() != 1
    {
        return Ok(false);
    }
    for (index, actual) in function.signature.parameters.iter().enumerate() {
        let expected = descriptor
            .parameter_type(index)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        if !verification_types_equal_v1(actual, &expected, budget)? {
            return Ok(false);
        }
    }
    let expected_result = descriptor.result_type();
    if !verification_types_equal_v1(&function.signature.results[0], &expected_result, budget)? {
        return Ok(false);
    }
    let capability_matches = match descriptor.capability() {
        FloatIntrinsicCapabilityV1::None => function.required_capabilities.is_empty(),
        FloatIntrinsicCapabilityV1::Float16 => {
            function.required_capabilities.len() == 1
                && function
                    .required_capabilities
                    .contains(&TargetCapability::Float16)
        }
        FloatIntrinsicCapabilityV1::BFloat16 => {
            function.required_capabilities.len() == 1
                && function
                    .required_capabilities
                    .contains(&TargetCapability::BFloat16)
        }
    };
    Ok(capability_matches)
}

pub(crate) fn verify_reserved_call_shape_v1(
    callee: &FunctionId,
    argument_count: usize,
    location: &VerificationDiagnosticLocationV1<'_>,
    diagnostics: &mut VerificationDiagnosticCollectorV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    charge_prefix_comparisons_v1(callee, budget)?;
    let invalid = if callee.as_str().starts_with(DIAGNOSTIC_PREFIX_V1) {
        charge_diagnostic_lookup_v1(callee, budget)?;
        AmdGpuDiagnosticOperation::intrinsic_descriptor_v1(callee)
            .is_none_or(|descriptor| descriptor.arity() != argument_count)
    } else if callee.as_str().starts_with(FLOAT_PREFIX_V1) {
        charge_float_lookup_v1(callee, budget)?;
        FloatOperation::intrinsic_descriptor_v1(callee)
            .is_none_or(|descriptor| descriptor.arity() != argument_count)
    } else {
        false
    };
    if invalid {
        let (code, prefix) = if callee.as_str().starts_with(DIAGNOSTIC_PREFIX_V1) {
            (
                DiagnosticCode::InvalidAmdGpuDiagnosticOperation,
                "reserved AMDGPU diagnostic intrinsic call",
            )
        } else {
            (
                DiagnosticCode::InvalidFloatOperation,
                "reserved float intrinsic call",
            )
        };
        emit_dynamic_v1(
            diagnostics,
            clone_diagnostic_location_v1(location, budget)?,
            code,
            identifier_message_work_v1(callee.as_str().len(), 256)?,
            format_args!("{prefix} {callee} must use its exact canonical contract"),
            budget,
        )?;
    }
    Ok(())
}

pub(crate) fn reserved_diagnostic_call_is_terminating_v1(
    callee: &FunctionId,
    argument_count: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    charge_prefix_comparisons_v1(callee, budget)?;
    if !callee.as_str().starts_with(DIAGNOSTIC_PREFIX_V1) {
        return Ok(false);
    }
    charge_diagnostic_lookup_v1(callee, budget)?;
    let descriptor = AmdGpuDiagnosticOperation::intrinsic_descriptor_v1(callee);
    Ok(matches!(
        descriptor,
        Some(AmdGpuDiagnosticIntrinsicDescriptorV1::Trap)
            | Some(AmdGpuDiagnosticIntrinsicDescriptorV1::AssertFail)
    ) && descriptor.is_some_and(|descriptor| descriptor.arity() == argument_count))
}
