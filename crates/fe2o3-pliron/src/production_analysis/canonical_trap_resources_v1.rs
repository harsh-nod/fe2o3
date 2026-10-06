//! Canonical-byte domain only. Native analysis keeps its separate live receipt.
use super::*;

pub(super) fn inventory_error(
    error: fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
) -> Failure {
    match error {
        fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
            Failure::Resource(error)
        }
        _ => refuse(),
    }
}

pub(super) fn is_trap(
    callee: &fe2o3_kernel_ir::FunctionId,
    budget: &mut Budget<'_>,
) -> Result<bool, Failure> {
    // ir.rs's closed descriptor lookup visits eight rows, each comparing the
    // terminal-inclusive name. Zero arguments excludes every allocating variant.
    budget.charge_work(
        callee
            .as_str()
            .len()
            .checked_add(2)
            .and_then(|n| n.checked_mul(8))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(matches!(
        fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, &[]),
        Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)
    ))
}

pub(super) fn reserve_header<T>(budget: &mut Budget<'_>) -> Result<(), Failure> {
    budget.reserve_storage(checked_add(
        size_of::<CheckedCanonicalTrapPoliciesV1<'_, '_>>(),
        checked_add(
            size_of::<std::thread::Result<Result<T, Failure>>>(),
            drain_header(),
        )?,
    )?)?;
    Ok(())
}
