//! Explicit diagnostic continuation of the real collected transaction into V17.
//! One private source owner retains authenticated bindings while ordinary source
//! and SSA stages run. Its exported bytes never carry that custody or authority.

use super::*;

pub(crate) struct OrderedProgramObservationOwnerV32 {
    materialized: fe2o3_lower_mir_kernel::ProductionOrderedProgramPreRankedKirOwnerV17,
    // Keep actual source, target, descriptor and transaction bindings alive for
    // the observation. No later ranked/proof/artifact stage is fabricated.
    bindings: AuthenticatedProductionBindings,
}

impl OrderedProgramObservationOwnerV32 {
    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn diagnostic_source_projection_v17(
        &self,
    ) -> Result<ExactDebugSourceProjectionV1, String> {
        let borrowed = ExactDebugSourceOwnerV1::Ordered(&self.materialized);
        super::source_candidate_debug_join_v17_tests::preflight_projection(
            borrowed,
            &self.bindings.debug_source_files,
        )?;
        compiler_debug_source_projection_v1(borrowed, &self.bindings.debug_source_files)
            .map_err(|error| format!("source-candidate debug projection: {error}"))
    }

    pub(crate) fn materialized(
        &self,
    ) -> &fe2o3_lower_mir_kernel::ProductionOrderedProgramPreRankedKirOwnerV17 {
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
    pub(crate) fn observe_ordered_program_v32(
        self,
    ) -> Result<OrderedProgramObservationOwnerV32, String> {
        let ssa = self
            .import_semantic_mir()
            .and_then(ProductionCompilation::construct_semantic_middle_end)
            .and_then(ProductionCompilation::construct_semantic_ssa)
            .map_err(|error| format!("ordered-program source stages: {error}"))?;
        let SsaSemanticMirStage {
            semantic_ssa,
            bindings,
        } = ssa.stage;
        if semantic_ssa.source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V32
        {
            return Err("ordered-program diagnostic requires exact semantic MIR V32".to_owned());
        }
        crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
            &bindings.typed_descriptor_roots,
            semantic_ssa.source_semantic(),
        )
        .map_err(|error| format!("ordered-program descriptor ownership: {error}"))?;
        let [typed_root] = bindings.typed_descriptor_roots.as_slice() else {
            return Err("ordered-program diagnostic requires exactly one root".to_owned());
        };
        let source_launch = typed_root
            .source_launch()
            .ok_or("ordered-program descriptor has no exact source launch")?;
        let launch_inputs = [
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                typed_root.logical_name(),
                typed_root.kernel_binding_bytes(),
                source_launch,
            ),
        ];
        // This shared helper constructs source launch agreement, not ranked
        // proof. Its name does not promote this diagnostic continuation.
        let launch =
            crate::production_ranked_projection_v1::source_launch_roster_for_ranked_inputs_v1(
                &semantic_ssa,
                &launch_inputs,
            )
            .map_err(|error| format!("ordered-program exact source launch: {error:?}"))?;
        let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
            .map_err(|_| "ordered-program work limit conversion overflow")?;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let materialized =
            fe2o3_lower_mir_kernel::ProductionOrderedProgramPreRankedKirOwnerV17::
                try_materialize_with_budget(
                    semantic_ssa,
                    launch,
                    fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                    &mut budget,
                )
                .map_err(|error| format!("ordered-program V17 materialization: {error}"))?;
        budget
            .reserve_storage(materialized.executable_storage().retained_storage())
            .map_err(|error| format!("ordered-program retained canonical storage: {error}"))?;
        budget
            .reserve_storage(materialized.call_correspondence_storage())
            .map_err(|error| format!("ordered-program retained return correspondence: {error}"))?;
        Ok(OrderedProgramObservationOwnerV32 {
            materialized,
            bindings,
        })
    }
}
