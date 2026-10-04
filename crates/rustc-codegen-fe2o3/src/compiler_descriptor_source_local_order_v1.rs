//! New fixed source-local-order composition, not a Policy6/Policy8 relabel.
use super::*;
use crate::production_pipeline::native_checked_output_handoff_v1::NativeOutputHandoffErrorV1;
use fe2o3_lower_mir_kernel::ProductionOwnedSourceLocalOrderContinuationV1 as Admitted;

pub(crate) const PRODUCER_VERSION: &str = "source-local-order-policy6-v1/gfx942";

fn view(owner: &Admitted) -> Result<CheckedDescriptorViewV1<'_>, CompilerDescriptorError> {
    let mut view = policy6::direct_view(owner.prefix())?;
    view.output = owner.output();
    view.kernels = owner.kernels();
    Ok(view)
}

pub(crate) fn construct_source_local_order_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &Admitted,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    let target = envelope.target().to_string();
    if ProductionAmdTargetProfileV1::from_device_target(&target)
        != Some(ProductionAmdTargetProfileV1::Gfx942)
    {
        return Err(CompilerDescriptorError::UnsupportedTarget(target));
    }
    admitted
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::SourceLocalOrder(Box::new(error)))?;
    construct_checked_descriptor_v1(
        envelope,
        compiler_module,
        typed_roots,
        view(admitted)?,
        0x0100,
        budget,
    )
}

/// Rechecks the same source/target/ABI/formal geometry for actual L, without
/// admitting L to the closed protected OutputOwnerV1/finalizer family.
pub(crate) fn check_source_local_order_descriptor_source_v1(
    admitted: &Admitted,
    typed_roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), NativeOutputHandoffErrorV1> {
    crate::production_pipeline::native_checked_output_handoff_v1::scoped(budget, |budget| {
        budget.charge_work(1)?;
        if profile != ProductionAmdTargetProfileV1::Gfx942 {
            return Err(NativeOutputHandoffErrorV1::Mismatch(
                "source local-order gfx942 profile",
            ));
        }
        native_worker_binding_v1::check_descriptor_view_v1(
            view(admitted)
                .map_err(|error| NativeOutputHandoffErrorV1::Descriptor(Box::new(error)))?,
            typed_roots,
            profile,
            budget,
        )
    })
}
