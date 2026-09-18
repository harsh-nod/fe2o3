//! Test-only continuation of the real collected transaction into exact V16.
//! This retains the authenticated bindings; it neither exposes stage fields nor
//! supplies ranked, functional, protected publication or execution authority.

use super::*;

pub(crate) struct OrderedRegionObservationOwnerV31 {
    materialized: fe2o3_lower_mir_kernel::ProductionOrderedRegionPreRankedKirOwnerV16,
    // Keep the original source/target/descriptor/transaction custody alive for
    // the complete observation. These bindings retain their existing meaning;
    // they do not imply completion of any later production stage.
    bindings: AuthenticatedProductionBindings,
}

impl OrderedRegionObservationOwnerV31 {
    pub(crate) fn materialized(
        &self,
    ) -> &fe2o3_lower_mir_kernel::ProductionOrderedRegionPreRankedKirOwnerV16 {
        &self.materialized
    }

    pub(crate) fn authenticated_source_identities(&self) -> ([u8; 32], [u8; 32]) {
        (
            self.bindings.rustc_identity_inventory.sha256(),
            self.bindings.rustc_preflight_plan.sha256(),
        )
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_ordered_region_v31(
        self,
    ) -> Result<OrderedRegionObservationOwnerV31, String> {
        // The same private transitions as the production path authenticate and
        // admit actual MIR and construct/replay its normal middle-end owners.
        let ssa = self
            .import_semantic_mir()
            .and_then(ProductionCompilation::construct_semantic_middle_end)
            .and_then(ProductionCompilation::construct_semantic_ssa)
            .map_err(|error| format!("ordered-region source stages: {error}"))?;
        let SsaSemanticMirStage {
            semantic_ssa,
            bindings,
        } = ssa.stage;
        crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
            &bindings.typed_descriptor_roots,
            semantic_ssa.source_semantic(),
        )
        .map_err(|error| format!("ordered-region descriptor ownership: {error}"))?;
        let ranked_roots = bindings
            .typed_descriptor_roots
            .iter()
            .map(|typed_root| {
                let source_launch = typed_root
                    .source_launch()
                    .ok_or("ordered-region descriptor has no exact source launch")?;
                Ok(
                    crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                        typed_root.logical_name(),
                        typed_root.kernel_binding_bytes(),
                        source_launch,
                    ),
                )
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let launch =
            crate::production_ranked_projection_v1::source_launch_roster_for_ranked_inputs_v1(
                &semantic_ssa,
                &ranked_roots,
            )
            .map_err(|error| format!("ordered-region exact source launch: {error:?}"))?;
        let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
            .map_err(|_| "ordered-region work limit conversion overflow")?;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let materialized =
            fe2o3_lower_mir_kernel::ProductionOrderedRegionPreRankedKirOwnerV16::
                try_materialize_with_budget(
                    semantic_ssa,
                    launch,
                    fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                    &mut budget,
                )
                .map_err(|error| format!("ordered-region V16 materialization: {error}"))?;
        budget
            .reserve_storage(materialized.executable_storage().retained_storage())
            .map_err(|error| format!("ordered-region retained canonical storage: {error}"))?;
        budget
            .reserve_storage(materialized.call_correspondence_storage())
            .map_err(|error| format!("ordered-region retained return correspondence: {error}"))?;
        Ok(OrderedRegionObservationOwnerV31 {
            materialized,
            bindings,
        })
    }
}
