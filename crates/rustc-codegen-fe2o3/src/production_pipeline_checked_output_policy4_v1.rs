//! Native checked-output custody. Not the legacy default or a publication path.

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
use fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy4V1 as Admitted;

#[cfg(test)]
#[path = "production_pipeline_checked_output_snapshots_v1_tests.rs"]
pub(crate) mod snapshots;

#[derive(Debug)]
pub(crate) enum CheckedOutputStageErrorV1 {
    Resource(Resource),
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12),
    Optimization(Box<fe2o3_kernel_opt::CanonicalPolicy4OptimizationErrorV1>),
    Admission(Box<fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy4V1>),
    Catalog(fe2o3_kernel_ir::KernelIrContractCatalogErrorV1),
    NativeSource(Box<crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1>),
    NativePublicationUnavailable,
}

impl fmt::Display for CheckedOutputStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Canonical(e) => e.fmt(f),
            Self::Optimization(e) => e.fmt(f),
            Self::Admission(e) => e.fmt(f),
            Self::Catalog(e) => e.fmt(f),
            Self::NativeSource(e) => e.fmt(f),
            Self::NativePublicationUnavailable => f.write_str(
                "native checked-output protected lineage is not implemented; no legacy relabeling or unoptimized fallback was attempted",
            ),
        }
    }
}

impl std::error::Error for CheckedOutputStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Canonical(e) => Some(e),
            Self::Optimization(e) => Some(e.as_ref()),
            Self::Admission(e) => Some(e.as_ref()),
            Self::Catalog(e) => Some(e),
            Self::NativeSource(e) => Some(e.as_ref()),
            Self::NativePublicationUnavailable => None,
        }
    }
}

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

/// All fields move together; the final text was replayed against this exact O.
/// Neither inert bytes nor this owner grant protected publication authority.
pub(crate) struct PreparedCheckedOutputArtifactsV1 {
    admitted: Admitted,
    catalog: Catalog,
    prepared: crate::production_worker_handoff::PreparedProductionWorkerHandoff,
    workgroups: Box<[(String, fe2o3_kernel_ir::WorkgroupSize)]>,
}

impl PreparedCheckedOutputArtifactsV1 {
    pub(crate) fn native_worker_output_v1(
        &self,
    ) -> super::native_checked_output_handoff_v1::OutputInputsV1<'_> {
        super::native_checked_output_handoff_v1::OutputInputsV1 {
            owner: super::native_checked_output_handoff_v1::OutputOwnerV1::Direct(&self.admitted),
            catalog: &self.catalog,
            prepared: &self.prepared,
            workgroups: &self.workgroups,
        }
    }
    pub(crate) fn admitted(&self) -> &Admitted {
        &self.admitted
    }
    #[cfg(test)]
    pub(crate) fn llvm_ir(&self) -> &str {
        self.prepared.llvm_ir()
    }
    #[cfg(test)]
    pub(crate) fn descriptor_source(&self) -> &fe2o3_compiler_ffi::CompilerDescriptorSourceV1 {
        self.prepared.descriptor_source()
    }
    pub(crate) fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub(crate) fn workgroup_sizes(&self) -> &[(String, fe2o3_kernel_ir::WorkgroupSize)] {
        &self.workgroups
    }
    #[cfg(test)]
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Additional retained logical payload receipt, not the input N/B/C/O receipt.
/// Catalog storage is actual-capacity accounted by its constructor. Text and
/// descriptor encoded payloads use existing engine maxima; internal descriptor,
/// FFI, symbol and workgroup allocations remain their inherited bounded domain.
#[derive(Clone, Copy)]
pub(crate) struct CheckedOutputArtifactsStorageV1(usize);
impl CheckedOutputArtifactsStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Actual native lowering, descriptor assembly and replay, shared with genuine
/// backend-projector tests. No test supplies collector/protected bindings.
/// Reserve the input source/capture + B + checked receipts before entry, and
/// the returned additional receipt immediately before subsequent allocation.
pub(crate) fn prepare_checked_output_artifacts_v1(
    admitted: Admitted,
    profile: Profile,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    source_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        PreparedCheckedOutputArtifactsV1,
        CheckedOutputArtifactsStorageV1,
    ),
    ProductionPipelineError,
> {
    use super::checked_output_artifacts_v1::{
        CheckedArtifactsOwnerRefV1, prepare_checked_artifact_parts_v1,
    };
    let (parts, retained) = prepare_checked_artifact_parts_v1(
        CheckedArtifactsOwnerRefV1::Direct(&admitted),
        profile,
        typed_roots,
        source_envelope,
        std::mem::size_of::<PreparedCheckedOutputArtifactsV1>(),
        budget,
    )?;
    Ok((
        PreparedCheckedOutputArtifactsV1 {
            admitted,
            catalog: parts.catalog,
            prepared: parts.prepared,
            workgroups: parts.workgroups,
        },
        CheckedOutputArtifactsStorageV1(retained),
    ))
}

/// Private pipeline stage retaining the authenticated roster and collector
/// bindings alongside N/B/C/O and the actually replayed native handoff.
pub(crate) struct CheckedOutputTargetProductionCompilation {
    artifacts: PreparedCheckedOutputArtifactsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_storage_floor: usize,
}

/// Native source proof and final checked output retain their original custody.
/// This is not a protected artifact stage: the final native proof-format and
/// protected compiler-origin join still have to consume this complete state.
#[allow(dead_code)]
pub(crate) struct NativeSourceCheckedOutputProductionCompilationV1 {
    artifacts: PreparedCheckedOutputArtifactsV1,
    source_lineage: crate::production_native_source_lineage_v1::PreparedNativeSourceLineageV1,
    bindings: AuthenticatedProductionBindings,
    retained_storage_floor: usize,
}

impl NativeSourceCheckedOutputProductionCompilationV1 {
    pub(super) fn native_worker_inputs_v1(
        &self,
    ) -> super::native_checked_output_handoff_v1::StageInputsV1<'_> {
        super::native_checked_output_handoff_v1::StageInputsV1 {
            output: self.artifacts.native_worker_output_v1(),
            proof: super::native_checked_output_handoff_v1::SourceProofV1::Direct(
                self.source_lineage.proof(),
            ),
            bindings: &self.bindings,
            retained_floor: self.retained_storage_floor,
        }
    }
}

/// Same-ledger transfer before descriptor or native artifact construction.
pub(super) struct AdmittedPolicy4StageV1 {
    pub(super) admitted: Admitted,
    pub(super) ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    pub(super) bindings: AuthenticatedProductionBindings,
}

impl RankedVerifiedProductionCompilation {
    /// Not selected by the legacy default until corpus admission and the native
    /// protected-lineage contract are complete. No caller policy parameter.
    #[allow(dead_code)]
    pub(crate) fn lower_checked_output_policy4_v1(
        self,
    ) -> Result<CheckedOutputTargetProductionCompilation, ProductionPipelineError> {
        let mut work = Work::new(
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(|_| resource(Resource::Arithmetic))?,
        );
        let mut budget = Budget::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let AdmittedPolicy4StageV1 {
            admitted,
            ranked_verification,
            bindings,
        } = self.prepare_admitted_policy4_v1(&mut budget)?;
        let (artifacts, storage) = prepare_checked_output_artifacts_v1(
            admitted,
            bindings.rustc_target.profile(),
            &bindings.typed_descriptor_roots,
            bindings.transaction.compiler_ffi_envelope.clone(),
            &mut budget,
        )?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        Ok(CheckedOutputTargetProductionCompilation {
            artifacts,
            ranked_verification,
            bindings,
            retained_storage_floor: budget.storage(),
        })
    }

    pub(super) fn prepare_admitted_policy4_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<AdmittedPolicy4StageV1, ProductionPipelineError> {
        let Self { ranked, bindings } = self.replay_conditional_for_target_v1()?;
        let profile = bindings.rustc_target.profile();
        let retained = ranked
            .materialized()
            .unit_local_source_storage_floor_v1()
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        budget.reserve_storage(retained).map_err(resource)?;
        #[cfg(test)]
        let phase = timing::begin(timing::Route::Direct, timing::Phase::TargetBindAndAdmit);
        let binding = dialect_amdgcn::bind_production_target_v1(
            ranked.materialized().executable().module(),
            profile,
        )
        .map_err(ProductionPipelineError::TargetBinding)?;
        let (bound, bound_storage) = Owner::from_module_ref_with_verification_budget_v12(
            binding.module(),
            budget,
        )
        .map_err(|e| {
            ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Canonical(e))
        })?;
        budget
            .reserve_storage(bound_storage.retained_storage())
            .map_err(resource)?;
        drop(binding);
        #[cfg(test)]
        phase.complete();
        #[cfg(test)]
        let phase = timing::begin(timing::Route::Direct, timing::Phase::Optimizer);
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                .map_err(|e| {
                    ProductionPipelineError::CheckedOutputStage(
                        CheckedOutputStageErrorV1::Optimization(Box::new(e)),
                    )
                })?;
        budget
            .reserve_storage(checked.retained_storage())
            .map_err(resource)?;
        #[cfg(test)]
        phase.complete();
        #[cfg(test)]
        snapshots::observe(
            &bound,
            checked.intermediate_policy3().owner(),
            checked.owner(),
        );
        #[cfg(test)]
        let phase = timing::begin(timing::Route::Direct, timing::Phase::RankedSourceReplay);
        let (receipt, ranked_verification) = ranked
            .into_verified_roster_receipt()
            .map_err(ProductionPipelineError::RankedVerification)?
            .into_module_verified_receipt()
            .map_err(ProductionPipelineError::RankedVerification)?;
        #[cfg(test)]
        phase.complete();
        #[cfg(test)]
        let phase = timing::begin(timing::Route::Direct, timing::Phase::FinalAdmission);
        let admitted =
            Admitted::try_admit_v1(receipt, bound, checked, budget).map_err(admission)?;
        #[cfg(test)]
        phase.complete();
        Ok(AdmittedPolicy4StageV1 {
            admitted,
            ranked_verification,
            bindings,
        })
    }
}

impl CheckedOutputTargetProductionCompilation {
    /// Complete transferred canonical source/B/C/O and native-artifact receipts.
    #[allow(dead_code)]
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_storage_floor
    }

    /// Consumes the original ranked roster, never an independently supplied
    /// packet. The complete transferred logical floor must be caller-reserved;
    /// inherited descriptor internals and collector custody keep their bounded
    /// accounting domains, not an assertion of heap/RSS coverage.
    /// Missing authenticated proof execution is an error, not an unsigned path.
    #[allow(dead_code)]
    pub(crate) fn prepare_native_source_lineage_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            NativeSourceCheckedOutputProductionCompilationV1,
            crate::production_native_source_lineage_v1::NativeSourceLineageStorageV1,
        ),
        ProductionPipelineError,
    > {
        budget.charge_work(2).map_err(resource)?;
        if budget.storage() < self.retained_storage_floor {
            return Err(resource(Resource::Accounting));
        }
        let (source_lineage, storage) =
            crate::production_native_source_lineage_v1::try_prepare_native_source_lineage_v1(
                self.artifacts.admitted().source_semantic_kir(),
                self.ranked_verification,
                budget,
            )
            .map_err(|error| {
                ProductionPipelineError::CheckedOutputStage(
                    CheckedOutputStageErrorV1::NativeSource(Box::new(error)),
                )
            })?;
        Ok((
            NativeSourceCheckedOutputProductionCompilationV1 {
                artifacts: self.artifacts,
                source_lineage,
                bindings: self.bindings,
                retained_storage_floor: self
                    .retained_storage_floor
                    .checked_add(storage.retained_storage())
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            },
            storage,
        ))
    }

    #[allow(dead_code)]
    pub(crate) fn output(&self) -> &Admitted {
        self.artifacts.admitted()
    }

    /// Extraction-only coordination bytes. Protected publication must not pass
    /// through the historical V8/V9/V11 lineage producer.
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
                    .source_semantic_kir()
                    .semantic()
                    .semantic()
                    .semantic_sha256()
                    .as_bytes()
        {
            return Err(ProductionPipelineError::RankedVerification(
                crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1::RosterMetadata(
                    "checked-output extraction source, ranked or workgroup roster",
                ),
            ));
        }
        self.artifacts
            .prepared
            .into_validated_parts()
            .map_err(ProductionPipelineError::WorkerHandoff)
    }
}
