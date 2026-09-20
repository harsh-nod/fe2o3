//! Reuse the existing exact source/target/ABI census without creating a descriptor.
use super::*;
use crate::production_pipeline::native_checked_output_handoff_v1::{
    NativeOutputHandoffErrorV1 as BindingError, OutputOwnerV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

pub(crate) fn check_native_worker_descriptor_source_v1(
    owner: OutputOwnerV1<'_>,
    typed_roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), BindingError> {
    crate::production_pipeline::native_checked_output_handoff_v1::scoped(budget, |budget| {
        check_native_worker_descriptor_source_inner_v1(owner, typed_roots, profile, budget)
    })
}

fn check_native_worker_descriptor_source_inner_v1(
    owner: OutputOwnerV1<'_>,
    typed_roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), BindingError> {
    let descriptor = |error| BindingError::Descriptor(Box::new(error));
    let view = match owner {
        OutputOwnerV1::Direct5(owner) => policy5::direct_view(owner).map_err(descriptor)?,
        OutputOwnerV1::Erased5(owner) => policy5::erased_view(owner),
        OutputOwnerV1::Direct6(owner) => policy6::direct_view(owner).map_err(descriptor)?,
        OutputOwnerV1::Erased6(owner) => policy6::erased_view(owner),
        OutputOwnerV1::Direct7(owner) => policy7::direct_view(owner).map_err(descriptor)?,
        OutputOwnerV1::Erased7(owner) => policy7::erased_view(owner),
        OutputOwnerV1::Direct8(owner) => policy8::direct_view(owner).map_err(descriptor)?,
        OutputOwnerV1::Erased8(owner) => policy8::erased_view(owner),
        OutputOwnerV1::Direct(owner) => CheckedDescriptorViewV1 {
            semantic: owner.source_semantic_kir().semantic().semantic(),
            source_launch: owner
                .source_semantic_kir()
                .source_launch_roster()
                .ok_or_else(|| {
                    descriptor(CompilerDescriptorError::ProductionDescriptorMismatch(
                        "retained source launch roster",
                    ))
                })?,
            neutral: owner
                .source_semantic_kir()
                .pre_ranked_executable()
                .ok_or_else(|| {
                    descriptor(CompilerDescriptorError::ProductionDescriptorMismatch(
                        "connected historical source",
                    ))
                })?,
            bound: owner.bound(),
            output: owner.output(),
            kernels: owner.kernels(),
        },
        OutputOwnerV1::Erased(owner) => CheckedDescriptorViewV1 {
            semantic: owner.original_source().semantic_ssa().source_semantic(),
            source_launch: owner.original_source().source_launch(),
            neutral: owner.erased(),
            bound: owner.bound(),
            output: owner.output(),
            kernels: owner.kernels(),
        },
    };
    {
        let _relation = dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            view.neutral,
            view.bound,
            profile,
            budget,
        )
        .map_err(|error| descriptor(CompilerDescriptorError::CheckedOutputTarget(error)))?;
    }
    // The existing descriptor/ownership engine keeps its bounded domain. Its
    // visible geometry-vector output is prepaid while the actual owners live.
    let header = std::mem::size_of::<Vec<crate::production_geometry_v1::ProductionGeometryV1>>();
    let element = std::mem::size_of::<crate::production_geometry_v1::ProductionGeometryV1>();
    let amount = typed_roots
        .len()
        .checked_mul(element)
        .and_then(|n| n.checked_add(header))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(3)?;
    budget.reserve_storage(amount)?;
    let geometries =
        validate_checked_output_descriptor_evidence_v1(typed_roots, &view, profile.device_target())
            .map_err(descriptor)?;
    let actual = geometries
        .capacity()
        .checked_mul(element)
        .and_then(|n| n.checked_add(header))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(amount).ok_or(Resource::Accounting)?)?;
    drop(geometries);
    budget.release_storage(actual)?;
    Ok(())
}
