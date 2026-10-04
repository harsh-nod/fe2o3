//! Historical source/bound inputs, actual J output and fresh J formal reports.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionOwnedRedundantStoreContinuationV1 as Direct,
    ProductionOwnedUnitLocalRedundantStoreContinuationV1 as Erased,
};

pub(super) fn direct_view(
    owner: &Direct,
) -> Result<CheckedDescriptorViewV1<'_>, CompilerDescriptorError> {
    let mut view = policy6::direct_view(owner.prefix())?;
    view.output = owner.output();
    view.kernels = owner.kernels();
    Ok(view)
}

pub(super) fn erased_view(owner: &Erased) -> CheckedDescriptorViewV1<'_> {
    let mut view = policy6::erased_view(owner.prefix());
    view.output = owner.output();
    view.kernels = owner.kernels();
    view
}

pub(crate) fn construct_checked_output_policy7_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &Direct,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    admitted
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::CheckedOutputPolicy7(Box::new(error)))?;
    construct_checked_descriptor_v1(
        envelope,
        compiler_module,
        typed_roots,
        direct_view(admitted)?,
        7,
        budget,
    )
}

pub(crate) fn construct_erased_checked_output_policy7_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &Erased,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    admitted
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::CheckedOutputPolicy7(Box::new(error)))?;
    construct_checked_descriptor_v1(
        envelope,
        compiler_module,
        typed_roots,
        erased_view(admitted),
        7,
        budget,
    )
}
