//! Typed Policy6 views reuse exact source/target/ABI and actual-output checks.
//! Original N is retained independently of optional erased target input E.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionCheckedOutputOwnerPolicy6V1 as Direct,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1 as Erased,
};

pub(super) fn direct_view(
    owner: &Direct,
) -> Result<CheckedDescriptorViewV1<'_>, CompilerDescriptorError> {
    Ok(CheckedDescriptorViewV1 {
        semantic: owner.source_semantic_kir().semantic().semantic(),
        source_launch: owner.source_semantic_kir().source_launch_roster().ok_or(
            CompilerDescriptorError::ProductionDescriptorMismatch("retained source launch roster"),
        )?,
        neutral: owner.source_semantic_kir().pre_ranked_executable().ok_or(
            CompilerDescriptorError::ProductionDescriptorMismatch("connected historical source"),
        )?,
        bound: owner.bound(),
        output: owner.output(),
        kernels: owner.kernels(),
    })
}
pub(super) fn erased_view(owner: &Erased) -> CheckedDescriptorViewV1<'_> {
    CheckedDescriptorViewV1 {
        semantic: owner.original_source().semantic_ssa().source_semantic(),
        source_launch: owner.original_source().source_launch(),
        neutral: owner.erased(),
        bound: owner.bound(),
        output: owner.output(),
        kernels: owner.kernels(),
    }
}

pub(crate) fn construct_checked_output_policy6_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &Direct,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    admitted
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::CheckedOutputPolicy6(Box::new(error)))?;
    construct_checked_descriptor_v1(
        envelope,
        compiler_module,
        typed_roots,
        direct_view(admitted)?,
        6,
        budget,
    )
}

pub(crate) fn construct_erased_checked_output_policy6_descriptor_source_v1(
    envelope: &CompilerFfiEnvelopeV1,
    compiler_module: &InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &Erased,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<CompilerDescriptorSourceV1, CompilerDescriptorError> {
    admitted
        .verify_equivalence(budget)
        .map_err(|error| CompilerDescriptorError::CheckedOutputPolicy6(Box::new(error)))?;
    construct_checked_descriptor_v1(
        envelope,
        compiler_module,
        typed_roots,
        erased_view(admitted),
        6,
        budget,
    )
}
