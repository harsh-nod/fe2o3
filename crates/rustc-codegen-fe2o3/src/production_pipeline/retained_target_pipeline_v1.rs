//! Opt-in original-account custody through the unchanged ordinary pipeline.
//!
//! This does not charge the later ranked constructor, Context/import, formal,
//! optimizer or LLVM work to the source account. Those retain their existing
//! contracts. No default route, nominal BF16 gate or target policy is changed.
use super::*;

// Shared by the historical borrowed-account entry and this owning entry.
// Keep this exact original constructor/retained-storage convention in one place.
pub(in crate::production_pipeline) fn materialize_ordinary_owner_with_budget_v1(
    semantic_ssa: fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    launch: fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
    budget: &mut Budget<'_>,
) -> std::result::Result<
    (fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1, usize),
    ProductionPipelineError,
> {
    let owner = fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        semantic_ssa,
        launch,
        fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .map_err(ProductionPipelineError::PreRankedMaterialization)?;
    let retained = owner.retained_analysis_storage_v1();
    Ok((owner, retained))
}

impl<'tcx> ProductionCompilation<'tcx, SsaSemanticMirStage> {
    #[allow(dead_code)] // Opt-in custody path; historical production entry is unchanged.
    pub(in crate::production_pipeline) fn materialize_target_neutral_retained_v1(
        self,
    ) -> Result<RetainedMaterializationPhaseV1<MaterializedNeutralProductionCompilation>> {
        self.with_retained_materialization_budget_v1(|prepared, budget| {
            let PreparedMaterializationV29 {
                materialized,
                ranked_roots,
                bindings,
            } = materialize_prepared_with_budget_v29(
                prepared,
                budget,
                |_, _| Ok(()),
                materialize_ordinary_owner_with_budget_v1,
            )?;
            Ok(MaterializedNeutralProductionCompilation {
                materialized,
                ranked_roots,
                bindings,
            })
        })
    }
}

impl RetainedMaterializationPhaseV1<MaterializedNeutralProductionCompilation> {
    #[allow(dead_code)]
    pub(in crate::production_pipeline) fn verify_general_kernel_checks_retained_v1(
        self,
    ) -> Result<RetainedMaterializationPhaseV1<RankedVerifiedProductionCompilation>> {
        self.try_map(|owner, _original_account| {
            owner.verify_general_kernel_checks().map_err(Box::new)
        })
    }
}

impl RetainedMaterializationPhaseV1<RankedVerifiedProductionCompilation> {
    // Preserve the default collected-target entry's direct-conditional refusal.
    // This is its existing distinct target account, not a replacement source account.
    #[allow(dead_code)]
    pub(in crate::production_pipeline) fn require_ordinary_target_route_retained_v1(
        self,
        target_budget: &mut Budget<'_>,
    ) -> Result<Self> {
        self.try_map(|owner, _original_account| {
            if owner.has_direct_conditional_roots_v2() {
                return Err(Box::new(
                    owner.conditional_production_finalizer_refusal_v5(target_budget),
                ));
            }
            Ok(owner)
        })
    }

    #[allow(dead_code)]
    pub(in crate::production_pipeline) fn attach_target_neutral_checks_retained_v1(
        self,
    ) -> Result<RetainedMaterializationPhaseV1<TargetNeutralProductionCompilation>> {
        self.try_map(|owner, _original_account| {
            owner.attach_target_neutral_checks().map_err(Box::new)
        })
    }
}

impl RetainedMaterializationPhaseV1<TargetNeutralProductionCompilation> {
    #[allow(dead_code)]
    pub(in crate::production_pipeline) fn admit_formal_memory_retained_v1(
        self,
    ) -> Result<RetainedMaterializationPhaseV1<FormalMemoryAdmittedProductionCompilation>> {
        self.try_map(|owner, _original_account| owner.admit_formal_memory().map_err(Box::new))
    }
}

impl RetainedMaterializationPhaseV1<FormalMemoryAdmittedProductionCompilation> {
    #[allow(dead_code)]
    pub(in crate::production_pipeline) fn lower_production_target_retained_v1(
        self,
    ) -> Result<RetainedMaterializationPhaseV1<TargetLoweredProductionCompilation>> {
        self.try_map(|owner, _original_account| owner.lower_production_target().map_err(Box::new))
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Complete opt-in ordinary route. Every mandatory existing transition still
    /// runs; only the lifetime of the original source account is extended.
    /// The returned typed owner cannot be detached from its account.
    #[allow(dead_code)]
    pub(in crate::production_pipeline) fn lower_target_with_retained_source_account_v1(
        self,
        target_budget: &mut Budget<'_>,
    ) -> Result<RetainedMaterializationPhaseV1<TargetLoweredProductionCompilation>> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .materialize_target_neutral_retained_v1()?
            .verify_general_kernel_checks_retained_v1()?
            .require_ordinary_target_route_retained_v1(target_budget)?
            .attach_target_neutral_checks_retained_v1()?
            .admit_formal_memory_retained_v1()?
            .lower_production_target_retained_v1()
    }
}

#[cfg(test)]
#[path = "retained_target_pipeline_v1_tests.rs"]
mod tests;
