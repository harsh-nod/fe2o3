//! Shared native artifact preparation for distinct direct and erased owners.

use super::checked_output_policy4_v1::CheckedOutputStageErrorV1;
#[cfg(test)]
use super::checked_output_progress_v1 as timing;
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};

pub(super) enum CheckedArtifactsOwnerRefV1<'a> {
    Direct(&'a fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1),
    Erased(&'a fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1),
    Direct5(&'a fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy5V1),
    Erased5(&'a fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1),
}

impl CheckedArtifactsOwnerRefV1<'_> {
    fn output(&self) -> &Owner {
        match self {
            Self::Direct(owner) => owner.output(),
            Self::Erased(owner) => owner.output(),
            Self::Direct5(owner) => owner.output(),
            Self::Erased5(owner) => owner.output(),
        }
    }

    fn semantic_identity(&self) -> [u8; 32] {
        match self {
            Self::Direct5(owner) => *owner
                .source_semantic_kir()
                .semantic()
                .semantic()
                .semantic_sha256()
                .as_bytes(),
            Self::Erased5(owner) => *owner
                .original_source()
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            Self::Direct(owner) => *owner
                .source_semantic_kir()
                .semantic()
                .semantic()
                .semantic_sha256()
                .as_bytes(),
            Self::Erased(owner) => *owner
                .original_source()
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
        }
    }

    fn retained_floor(&self) -> Result<usize, ProductionPipelineError> {
        let result = match self {
            Self::Direct(owner) => owner.retained_input_storage_floor_v1(),
            Self::Erased(owner) => owner.retained_input_storage_floor_v1(),
            Self::Direct5(owner) => {
                return owner
                    .retained_input_storage_floor_v1()
                    .map_err(super::checked_output_policy5_v1::admission);
            }
            Self::Erased5(owner) => {
                return owner
                    .retained_input_storage_floor_v1()
                    .map_err(super::checked_output_policy5_v1::admission);
            }
        };
        result.map_err(|error| {
            ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Admission(
                Box::new(error),
            ))
        })
    }

    fn handoff(
        &self,
        catalog: &Catalog,
        target: fe2o3_compiler_ffi::DeviceTargetV1,
        llvm_ir: String,
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        source_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
        budget: &mut Budget<'_>,
    ) -> Result<
        crate::production_worker_handoff::PreparedProductionWorkerHandoff,
        ProductionPipelineError,
    > {
        let result = match self {
            Self::Direct(owner) => crate::production_worker_handoff::prepare_checked_output_policy4_worker_handoff(owner,catalog,target,llvm_ir,typed_roots,source_envelope,budget),
            Self::Erased(owner) => crate::production_worker_handoff::prepare_erased_checked_output_policy4_worker_handoff(owner,catalog,target,llvm_ir,typed_roots,source_envelope,budget),
            Self::Direct5(owner) => crate::production_worker_handoff::prepare_checked_output_policy5_worker_handoff(owner,catalog,target,llvm_ir,typed_roots,source_envelope,budget),
            Self::Erased5(owner) => crate::production_worker_handoff::prepare_erased_checked_output_policy5_worker_handoff(owner,catalog,target,llvm_ir,typed_roots,source_envelope,budget),
        };
        result.map_err(ProductionPipelineError::WorkerHandoff)
    }
}

pub(super) struct PreparedCheckedArtifactPartsV1 {
    pub(super) catalog: Catalog,
    pub(super) prepared: crate::production_worker_handoff::PreparedProductionWorkerHandoff,
    pub(super) workgroups: Box<[(String, fe2o3_kernel_ir::WorkgroupSize)]>,
}

fn resource(error: Resource) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Resource(error))
}

/// Shared exact-O native preparation. The private closed owner view never
/// reinterprets E as original N. The direct route retains its original charge
/// schedule, native engine maxima, refusal order and cleanup behavior.
pub(super) fn prepare_checked_artifact_parts_v1(
    admitted: CheckedArtifactsOwnerRefV1<'_>,
    profile: Profile,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    source_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    wrapper_bytes: usize,
    budget: &mut Budget<'_>,
) -> Result<(PreparedCheckedArtifactPartsV1, usize), ProductionPipelineError> {
    #[cfg(test)]
    let phase = timing::begin(
        match &admitted {
            CheckedArtifactsOwnerRefV1::Direct(_) | CheckedArtifactsOwnerRefV1::Direct5(_) => {
                timing::Route::Direct
            }
            CheckedArtifactsOwnerRefV1::Erased(_) | CheckedArtifactsOwnerRefV1::Erased5(_) => {
                timing::Route::SilentUnitErased
            }
        },
        timing::Phase::ArtifactPreparation,
    );
    budget.charge_work(4).map_err(resource)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    if floor < admitted.retained_floor()? {
        return Err(resource(Resource::Accounting));
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let semantic = admitted.semantic_identity();
        // Empty catalog remains the admitted domain. Native replay independently
        // rejects an unmatched Execution or verification marker in actual O.
        let (catalog, catalog_storage) = Catalog::from_rows_with_budget(semantic, &[], &[], budget)
            .map_err(|error| {
                ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Catalog(
                    error,
                ))
            })?;
        budget
            .reserve_storage(catalog_storage.retained_storage())
            .map_err(resource)?;
        let text = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES;
        let descriptor = fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES;
        let prepaid = text
            .checked_mul(3)
            .and_then(|value| {
                descriptor
                    .checked_mul(2)
                    .and_then(|descriptor| value.checked_add(descriptor))
            })
            .and_then(|value| value.checked_add(wrapper_bytes))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(prepaid).map_err(resource)?;
        let native = match profile {
            Profile::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(admitted.output()),
            Profile::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(admitted.output()),
        }.map_err(ProductionPipelineError::TargetLowering)?;
        let llvm_ir = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&native)
            .map_err(ProductionPipelineError::UpstreamLlvmLayoutBinding)?;
        drop(native);
        let target = fe2o3_compiler_ffi::DeviceTargetV1::parse(profile.device_target())
            .expect("closed production profile");
        let prepared = admitted.handoff(
            &catalog,
            target,
            llvm_ir,
            typed_roots,
            source_envelope,
            budget,
        )?;
        let workgroups = exact_target_workgroup_roster_v1(admitted.output().module())?;
        let retained = catalog_storage
            .retained_storage()
            .checked_add(text)
            .and_then(|value| value.checked_add(descriptor))
            .and_then(|value| value.checked_add(wrapper_bytes))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        Ok((
            PreparedCheckedArtifactPartsV1 {
                catalog,
                prepared,
                workgroups,
            },
            retained,
        ))
    }));
    if ledger != budget.work_ledger_identity_v1() || slot != budget as *const Budget<'_> as usize {
        drop(result);
        return Err(resource(Resource::Accounting));
    }
    let Some(extra) = budget.storage().checked_sub(floor) else {
        drop(result);
        return Err(resource(Resource::Accounting));
    };
    if let Err(error) = budget.release_storage(extra) {
        drop(result);
        return Err(resource(error));
    }
    match result {
        Ok(result) => {
            #[cfg(test)]
            if result.is_ok() {
                phase.complete();
            }
            result
        }
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
