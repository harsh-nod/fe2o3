//! Explicit silent Unit erasure route; no protected/default publication path.

use super::checked_output_artifacts_v1::{
    CheckedArtifactsOwnerRefV1, prepare_checked_artifact_parts_v1,
};
use super::checked_output_policy4_v1::CheckedOutputStageErrorV1;
#[cfg(test)]
use super::checked_output_progress_v1 as timing;
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1 as Admitted;

fn resource(error: Resource) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Resource(error))
}
fn admission(
    error: fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy4V1,
) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Admission(Box::new(
        error,
    )))
}

/// Actual O text/descriptor remain owned with original source/N, distinct E and
/// final fixed Policy4 custody. No conversion to the historical direct owner.
pub(crate) struct PreparedErasedCheckedOutputArtifactsV1 {
    admitted: Admitted,
    catalog: Catalog,
    prepared: crate::production_worker_handoff::PreparedProductionWorkerHandoff,
    workgroups: Box<[(String, fe2o3_kernel_ir::WorkgroupSize)]>,
}

impl PreparedErasedCheckedOutputArtifactsV1 {
    pub(crate) fn native_worker_output_v1(
        &self,
    ) -> super::native_checked_output_handoff_v1::OutputInputsV1<'_> {
        super::native_checked_output_handoff_v1::OutputInputsV1 {
            owner: super::native_checked_output_handoff_v1::OutputOwnerV1::Erased(&self.admitted),
            catalog: &self.catalog,
            prepared: &self.prepared,
            workgroups: &self.workgroups,
        }
    }
    pub(crate) fn admitted(&self) -> &Admitted {
        &self.admitted
    }
    pub(crate) fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    #[cfg(test)]
    pub(crate) fn descriptor_source(&self) -> &fe2o3_compiler_ffi::CompilerDescriptorSourceV1 {
        self.prepared.descriptor_source()
    }
    #[cfg(test)]
    pub(crate) fn llvm_ir(&self) -> &str {
        self.prepared.llvm_ir()
    }
    pub(crate) fn workgroup_sizes(&self) -> &[(String, fe2o3_kernel_ir::WorkgroupSize)] {
        &self.workgroups
    }
    #[cfg(test)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Additional artifact payload only. Original source/N/E, B and checked C/O
/// remain caller-reserved; inherited descriptor/FFI/workgroup engine storage
/// retains the same bounded domain as direct native preparation.
#[derive(Clone, Copy)]
pub(crate) struct ErasedCheckedOutputArtifactsStorageV1(usize);
impl ErasedCheckedOutputArtifactsStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) fn prepare_erased_checked_output_artifacts_v1(
    admitted: Admitted,
    profile: Profile,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    source_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PreparedErasedCheckedOutputArtifactsV1,
        ErasedCheckedOutputArtifactsStorageV1,
    ),
    ProductionPipelineError,
> {
    let (parts, retained) = prepare_checked_artifact_parts_v1(
        CheckedArtifactsOwnerRefV1::Erased(&admitted),
        profile,
        typed_roots,
        source_envelope,
        std::mem::size_of::<PreparedErasedCheckedOutputArtifactsV1>(),
        budget,
    )?;
    Ok((
        PreparedErasedCheckedOutputArtifactsV1 {
            admitted,
            catalog: parts.catalog,
            prepared: parts.prepared,
            workgroups: parts.workgroups,
        },
        ErasedCheckedOutputArtifactsStorageV1(retained),
    ))
}

/// Authenticated collector bindings and the original verified root roster move
/// with N/E/B/C/O and actual native preparation; none is replaced by inert rows.
pub(crate) struct ErasedCheckedOutputTargetProductionCompilationV1 {
    artifacts: PreparedErasedCheckedOutputArtifactsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_storage_floor: usize,
}

/// Distinct consumed state retaining original collector/ranked custody, typed
/// source/N/E proof, E/B, checked C/O and the actual-O native artifacts. The
/// protected compiler-origin and final proof-format consumers remain absent.
#[allow(
    dead_code,
    reason = "retained custody for the pending protected-native consumer"
)]
pub(crate) struct NativeSourceErasedCheckedOutputProductionCompilationV1 {
    artifacts: PreparedErasedCheckedOutputArtifactsV1,
    source_lineage: crate::production_native_source_lineage_v1::PreparedErasedNativeSourceLineageV1,
    bindings: AuthenticatedProductionBindings,
    retained_storage_floor: usize,
}

impl NativeSourceErasedCheckedOutputProductionCompilationV1 {
    pub(super) fn native_worker_inputs_v1(
        &self,
    ) -> super::native_checked_output_handoff_v1::StageInputsV1<'_> {
        super::native_checked_output_handoff_v1::StageInputsV1 {
            output: self.artifacts.native_worker_output_v1(),
            proof: super::native_checked_output_handoff_v1::SourceProofV1::Erased(
                self.source_lineage.proof(),
            ),
            bindings: &self.bindings,
            retained_floor: self.retained_storage_floor,
        }
    }
}

/// Original source/N/E, B and checked O remain reserved on this caller ledger.
pub(super) struct AdmittedErasedPolicy4StageV1 {
    pub(super) admitted: Admitted,
    pub(super) ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    pub(super) bindings: AuthenticatedProductionBindings,
}

impl RankedVerifiedProductionCompilation {
    /// Explicit route only. It does not activate the default pipeline or allow
    /// protected publication while final source/native origin joins are absent.
    #[allow(dead_code)]
    pub(crate) fn lower_silent_unit_checked_output_policy4_v1(
        self,
    ) -> Result<ErasedCheckedOutputTargetProductionCompilationV1, ProductionPipelineError> {
        let mut work = Work::new(
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(|_| resource(Resource::Arithmetic))?,
        );
        let mut budget = Budget::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let AdmittedErasedPolicy4StageV1 {
            admitted,
            ranked_verification,
            bindings,
        } = self.prepare_admitted_erased_policy4_v1(&mut budget)?;
        let (artifacts, storage) = prepare_erased_checked_output_artifacts_v1(
            admitted,
            bindings.rustc_target.profile(),
            &bindings.typed_descriptor_roots,
            bindings.transaction.compiler_ffi_envelope.clone(),
            &mut budget,
        )?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        Ok(ErasedCheckedOutputTargetProductionCompilationV1 {
            artifacts,
            ranked_verification,
            bindings,
            retained_storage_floor: budget.storage(),
        })
    }

    pub(super) fn prepare_admitted_erased_policy4_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<AdmittedErasedPolicy4StageV1, ProductionPipelineError> {
        let Self { ranked, bindings } = self;
        let profile = bindings.rustc_target.profile();
        let original = ranked
            .materialized()
            .unit_local_source_storage_floor_v1()
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        budget.reserve_storage(original).map_err(resource)?;
        #[cfg(test)]
        let phase = timing::begin(
            timing::Route::SilentUnitErased,
            timing::Phase::RankedSourceReplay,
        );
        let (source, ranked_verification, source_storage) = ranked
            .into_verified_roster_receipt()
            .map_err(ProductionPipelineError::RankedVerification)?
            .into_silent_unit_erased_source_v1(budget)
            .map_err(ProductionPipelineError::RankedVerification)?;
        budget
            .reserve_storage(source_storage.retained_storage())
            .map_err(resource)?;
        #[cfg(test)]
        phase.complete();
        #[cfg(test)]
        let phase = timing::begin(
            timing::Route::SilentUnitErased,
            timing::Phase::TargetBindAndAdmit,
        );
        let binding = dialect_amdgcn::bind_production_target_v1(source.erased().module(), profile)
            .map_err(ProductionPipelineError::TargetBinding)?;
        let (bound, bound_storage) = Owner::from_module_ref_with_verification_budget_v12(
            binding.module(),
            budget,
        )
        .map_err(|error| {
            ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Canonical(error))
        })?;
        budget
            .reserve_storage(bound_storage.retained_storage())
            .map_err(resource)?;
        drop(binding);
        #[cfg(test)]
        phase.complete();
        #[cfg(test)]
        let phase = timing::begin(timing::Route::SilentUnitErased, timing::Phase::Optimizer);
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                .map_err(|error| {
                    ProductionPipelineError::CheckedOutputStage(
                        CheckedOutputStageErrorV1::Optimization(Box::new(error)),
                    )
                })?;
        budget
            .reserve_storage(checked.retained_storage())
            .map_err(resource)?;
        #[cfg(test)]
        phase.complete();
        #[cfg(test)]
        super::checked_output_policy4_v1::snapshots::observe(
            &bound,
            checked.intermediate_policy3().owner(),
            checked.owner(),
        );
        #[cfg(test)]
        let phase = timing::begin(
            timing::Route::SilentUnitErased,
            timing::Phase::FinalAdmission,
        );
        let admitted = Admitted::try_admit_v1(source, bound, checked, budget).map_err(admission)?;
        #[cfg(test)]
        phase.complete();
        Ok(AdmittedErasedPolicy4StageV1 {
            admitted,
            ranked_verification,
            bindings,
        })
    }
}

impl ErasedCheckedOutputTargetProductionCompilationV1 {
    #[allow(dead_code)]
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_storage_floor
    }
    #[allow(dead_code)]
    pub(crate) fn output(&self) -> &Admitted {
        self.artifacts.admitted()
    }

    /// Consumes this stage's actual original roster. Missing signed execution
    /// is an error, never a conversion to the Direct/N-only producer.
    #[allow(dead_code)]
    pub(crate) fn prepare_native_source_lineage_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            NativeSourceErasedCheckedOutputProductionCompilationV1,
            crate::production_native_source_lineage_v1::ErasedNativeSourceLineageStorageV1,
        ),
        ProductionPipelineError,
    > {
        budget.charge_work(2).map_err(resource)?;
        if budget.storage() < self.retained_storage_floor {
            return Err(resource(Resource::Accounting));
        }
        let (source_lineage, storage) =
            crate::production_native_source_lineage_v1::try_prepare_erased_native_source_lineage_v1(
                self.artifacts.admitted().erased_source(),
                self.ranked_verification,
                budget,
            ).map_err(|error| ProductionPipelineError::CheckedOutputStage(
                CheckedOutputStageErrorV1::NativeSource(Box::new(error))))?;
        // The producer returned its receipt unreserved. Keep it live through
        // the cross-stage comparison, then transfer it unchanged to the caller.
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        let catalog_result =
            source_lineage.check_output_catalog_v1(self.artifacts.catalog(), budget);
        if let Err(error) = catalog_result {
            drop(source_lineage);
            budget
                .release_storage(storage.retained_storage())
                .map_err(resource)?;
            return Err(ProductionPipelineError::CheckedOutputStage(
                CheckedOutputStageErrorV1::NativeSource(Box::new(error)),
            ));
        }
        budget
            .release_storage(storage.retained_storage())
            .map_err(resource)?;
        let retained_storage_floor = self
            .retained_storage_floor
            .checked_add(storage.retained_storage())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        Ok((
            NativeSourceErasedCheckedOutputProductionCompilationV1 {
                artifacts: self.artifacts,
                source_lineage,
                bindings: self.bindings,
                retained_storage_floor,
            },
            storage,
        ))
    }

    /// Extraction-only coordination bytes, retaining all original extraction
    /// guards. No old N-only lineage producer or protected publication is used.
    #[allow(dead_code)]
    pub(crate) fn into_worker_handoff_extraction_v1(
        self,
    ) -> Result<
        (
            fe2o3_compiler_ffi::CompilerModuleHandoffV2,
            fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
        ),
        ProductionPipelineError,
    > {
        if !self
            .bindings
            .transaction
            .compiler_custody
            .is_extraction_only()
        {
            return Err(ProductionPipelineError::CheckedOutputStage(
                CheckedOutputStageErrorV1::NativePublicationUnavailable,
            ));
        }
        if self
            .bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != self.bindings.rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        let kernels = &self.artifacts.admitted().output().module().kernels;
        if self.ranked_verification.root_count() != kernels.len()
            || !self
                .ranked_verification
                .every_functional_verification_is_coherent()
            || self.artifacts.workgroup_sizes().len() != kernels.len()
            || self
                .artifacts
                .workgroup_sizes()
                .iter()
                .zip(kernels)
                .any(|((name, size), kernel)| {
                    name != kernel.id.as_str() || Some(*size) != kernel.workgroup_size
                })
            || self.artifacts.catalog().semantic_source()
                != self
                    .artifacts
                    .admitted()
                    .original_source()
                    .semantic_ssa()
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes()
        {
            return Err(ProductionPipelineError::RankedVerification(crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1::RosterMetadata(
                "erased checked-output extraction source, ranked or workgroup roster")));
        }
        self.artifacts
            .prepared
            .into_validated_parts()
            .map_err(ProductionPipelineError::WorkerHandoff)
    }
}
