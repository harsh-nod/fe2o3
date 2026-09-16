use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    FixedVectorTypeErrorV12, Type,
};

/// Compares recursive types without cloning them, charging each inspected node
/// before its tag and fields are read.
pub(crate) fn verification_types_equal_v1(
    left: &Type,
    right: &Type,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(1)?;
    Ok(match (left, right) {
        (Type::Unit, Type::Unit) => true,
        (Type::Scalar(left), Type::Scalar(right)) => left == right,
        (Type::Execution(left), Type::Execution(right)) => left == right,
        (Type::Vector(left), Type::Vector(right)) => left == right,
        (Type::Pointer(left), Type::Pointer(right)) => {
            left.address_space == right.address_space
                && left.access == right.access
                && verification_types_equal_v1(&left.pointee, &right.pointee, budget)?
        }
        (Type::Slice(left), Type::Slice(right)) => {
            left.address_space == right.address_space
                && left.access == right.access
                && verification_types_equal_v1(&left.element, &right.element, budget)?
        }
        _ => false,
    })
}

/// Returns a conservative Debug-format work bound after charging the exact
/// recursive type-node census used to derive it.
pub(crate) fn verification_type_message_work_upper_v1(
    ty: &Type,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    let nodes = verification_type_nodes_v1(ty, budget)?;
    nodes
        .checked_mul(128)
        .and_then(|work| work.checked_add(512))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
}

pub(crate) fn verification_type_nodes_v1(
    ty: &Type,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    budget.charge_work(1)?;
    match ty {
        Type::Pointer(pointer) => verification_type_nodes_v1(&pointer.pointee, budget)?
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic),
        Type::Slice(slice) => verification_type_nodes_v1(&slice.element, budget)?
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic),
        _ => Ok(1),
    }
}

#[derive(Default)]
pub(crate) struct VerificationTypeFactsV15 {
    pub(crate) vector_error: Option<FixedVectorTypeErrorV12>,
    pub(crate) invalid_execution_role: bool,
    pub(crate) contains_execution_role: bool,
    pub(crate) storable: bool,
}

/// Validates vector and execution nodes in one traversal, charging each type
/// node before reading it, including ordinary legacy scalar terminals.
pub(crate) fn verification_type_facts_v15(
    mut ty: &Type,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerificationTypeFactsV15, CanonicalKernelIrVerificationResourceErrorV1> {
    let mut nested = false;
    let mut storable = false;
    loop {
        budget.charge_work(1)?;
        if !nested {
            storable = matches!(ty, Type::Scalar(_) | Type::Vector(_) | Type::Pointer(_));
        }
        match ty {
            Type::Vector(vector) => {
                return Ok(VerificationTypeFactsV15 {
                    vector_error: vector.validate().err(),
                    storable,
                    ..VerificationTypeFactsV15::default()
                });
            }
            Type::Execution(role) => {
                return Ok(VerificationTypeFactsV15 {
                    vector_error: None,
                    invalid_execution_role: nested || role.validate().is_err(),
                    contains_execution_role: true,
                    storable: false,
                });
            }
            Type::Pointer(pointer) => ty = &pointer.pointee,
            Type::Slice(slice) => ty = &slice.element,
            Type::Unit | Type::Scalar(_) => {
                return Ok(VerificationTypeFactsV15 {
                    storable,
                    ..VerificationTypeFactsV15::default()
                });
            }
        }
        nested = true;
    }
}

/// The caller's fixed operation charge covers the root tag; pointer descendants
/// need their own paid traversal before recursive storability can be decided.
pub(crate) fn verification_type_is_storable_v15(
    ty: &Type,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
    match ty {
        Type::Unit | Type::Slice(_) | Type::Execution(_) => Ok(false),
        Type::Scalar(_) | Type::Vector(_) => Ok(true),
        Type::Pointer(pointer) => {
            Ok(!verification_type_facts_v15(&pointer.pointee, budget)?.contains_execution_role)
        }
    }
}

#[cfg(test)]
#[path = "verification_type_comparison_v1_tests.rs"]
mod tests;
