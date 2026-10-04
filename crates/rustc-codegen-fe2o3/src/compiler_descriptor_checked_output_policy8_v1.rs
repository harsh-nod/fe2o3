//! Historical source/bound inputs, actual K output and fresh K formal reports.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionOwnedCommutativeContinuationV1 as Direct,
    ProductionOwnedUnitLocalCommutativeContinuationV1 as Erased,
};

pub(crate) const fn producer_version(profile: ProductionAmdTargetProfileV1) -> &'static str {
    match profile {
        ProductionAmdTargetProfileV1::Gfx942 => "production-policy8-checked-gfx942-cov6-v1",
        ProductionAmdTargetProfileV1::Gfx950 => "production-policy8-checked-gfx950-cov6-v1",
    }
}

pub(super) fn direct_view(
    owner: &Direct,
) -> Result<CheckedDescriptorViewV1<'_>, CompilerDescriptorError> {
    let mut view = policy7::direct_view(owner.prefix())?;
    view.output = owner.output();
    view.kernels = owner.kernels();
    Ok(view)
}

pub(super) fn erased_view(owner: &Erased) -> CheckedDescriptorViewV1<'_> {
    let mut view = policy7::erased_view(owner.prefix());
    view.output = owner.output();
    view.kernels = owner.kernels();
    view
}

pub(crate) fn construct_checked_output_policy8_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &Direct,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    admitted
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::CheckedOutputPolicy8(Box::new(error)))?;
    construct_checked_descriptor_v1(
        envelope,
        compiler_module,
        typed_roots,
        direct_view(admitted)?,
        8,
        budget,
    )
}

pub(crate) fn construct_erased_checked_output_policy8_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &Erased,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    admitted
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::CheckedOutputPolicy8(Box::new(error)))?;
    construct_checked_descriptor_v1(
        envelope,
        compiler_module,
        typed_roots,
        erased_view(admitted),
        8,
        budget,
    )
}
