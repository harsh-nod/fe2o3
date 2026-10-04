//! Dedicated actual-source composition path; no custody is reconstructed from KIR bytes.
use super::*;
#[cfg(target_os = "linux")]
#[path = "production_ordered_composition_request_v1.rs"]
mod source_promotion_request;
#[cfg(target_os = "linux")]
pub(crate) use source_promotion_request::SourcePromotionRequestV1;
#[cfg(target_os = "linux")]
#[path = "production_ordered_composition_action_v1.rs"]
mod source_promotion_action;
use crate::production_ordered_composition_source_v1::AuthenticatedOrderedCompositionSourceSeedV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionOrderedCompositionPreRankedKirOwnerV1 as Materialized;

/// This constructor is reachable only through the live composition importer.
/// It deliberately cannot enter a generic worker or protected publication path.
pub(crate) struct AuthenticatedOrderedCompositionDiagnosticV1<'tcx> {
    materialized: Materialized,
    emission: dialect_amdgcn::OrderedProgramCompositionCanonicalEmissionV1,
    bindings: AuthenticatedProductionBindings,
    source_seed: AuthenticatedOrderedCompositionSourceSeedV1<'tcx>,
    ledger: Ledger,
}
impl<'tcx> AuthenticatedOrderedCompositionDiagnosticV1<'tcx> {
    // This is the retained live source owner, not diagnostic file admission.
    pub(super) fn into_source_parts(
        self,
    ) -> (
        Materialized,
        dialect_amdgcn::OrderedProgramCompositionCanonicalEmissionV1,
        AuthenticatedProductionBindings,
        AuthenticatedOrderedCompositionSourceSeedV1<'tcx>,
        Ledger,
    ) {
        (
            self.materialized,
            self.emission,
            self.bindings,
            self.source_seed,
            self.ledger,
        )
    }
    pub(crate) fn materialized(&self) -> &Materialized {
        &self.materialized
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.emission.llvm_ir()
    }
    pub(crate) fn resource_usage(&self) -> (usize, usize, usize) {
        (
            self.ledger.work(),
            self.ledger.storage(),
            self.ledger.peak_storage(),
        )
    }
    pub(crate) fn source_identities(&self) -> ([u8; 32], [u8; 32]) {
        (
            self.bindings.rustc_identity_inventory.sha256(),
            self.bindings.rustc_preflight_plan.sha256(),
        )
    }
    pub(crate) fn source_seed(&self) -> &AuthenticatedOrderedCompositionSourceSeedV1<'tcx> {
        &self.source_seed
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn with_source_observation_budget<T>(
        &mut self,
        observe: impl FnOnce(
            &Materialized,
            &AuthenticatedOrderedCompositionSourceSeedV1<'tcx>,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> T,
    ) -> T {
        self.ledger
            .with_budget(|budget| observe(&self.materialized, &self.source_seed, budget))
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn with_observation_budget<T>(
        &mut self,
        observe: impl FnOnce(
            &Materialized,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> T,
    ) -> T {
        self.ledger
            .with_budget(|budget| observe(&self.materialized, budget))
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn lower_ordered_composition_diagnostic_v1(
        self,
    ) -> Result<AuthenticatedOrderedCompositionDiagnosticV1<'tcx>, String> {
        let CollectedRustStage {
            tcx,
            closure,
            typed_descriptor_roots,
            debug_source_capture,
            transaction,
        } = self.stage;
        if !transaction.compiler_custody.is_extraction_only() {
            return Err(
                "ordered composition diagnostic cannot consume protected publication custody"
                    .into(),
            );
        }
        // The dedicated move-only result, not a domain string or a bool, chooses
        // this route. The same successful source seed remains alive through SSA.
        let imported: crate::collector::AuthenticatedOrderedCompositionMirV1<'tcx> =
            crate::collector::construct_production_semantic_mir_ordered_composition_v1(
                tcx,
                closure,
                debug_source_capture,
            )
            .map_err(|error| format!("ordered composition actual source import: {error}"))?;
        let (constructed, source_seed) = imported.into_parts();
        let crate::collector::ConstructedProductionSemanticMirV1 {
            semantic_mir,
            context_entries,
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            reference_effect_bindings,
            debug_source_files,
            debug_source_scopes,
            debug_source_variables,
            debug_capture_gap,
        } = constructed;
        let typed_descriptor_roots =
            crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
                typed_descriptor_roots,
                &semantic_mir,
            )
            .map_err(|error| format!("ordered composition descriptor root order: {error}"))?;
        let admitted: ProductionCompilation<'tcx, AdmittedSemanticMirStage> =
            ProductionCompilation {
                stage: AdmittedSemanticMirStage {
                    semantic_mir,
                    bindings: AuthenticatedProductionBindings {
                        context_entries,
                        rustc_identity_inventory,
                        rustc_preflight_plan,
                        rustc_target,
                        reference_effect_bindings,
                        debug_source_files,
                        debug_source_scopes,
                        debug_source_variables,
                        debug_capture_gap,
                        typed_descriptor_roots,
                        transaction,
                    },
                },
                invariant_session: PhantomData,
            };
        let ssa = admitted
            .construct_semantic_middle_end()
            .and_then(ProductionCompilation::construct_semantic_ssa)
            .map_err(|error| format!("ordered composition ordinary semantic stages: {error}"))?;
        if ssa.stage.semantic_ssa.source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V32
            || source_seed.semantic_sha256()
                != ssa
                    .stage
                    .semantic_ssa
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes()
        {
            return Err("ordered composition source seed or exact MIR32 identity differs".into());
        }
        if ssa.stage.bindings.rustc_target.profile()
            != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
            || !ssa
                .stage
                .bindings
                .reference_effect_bindings
                .as_slice()
                .is_empty()
        {
            return Err(
                "ordered composition target or functional refinement scope is unsupported".into(),
            );
        }
        if ssa
            .stage
            .bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != ssa.stage.bindings.rustc_identity_inventory.sha256()
        {
            return Err("ordered composition live compiler lineage differs".into());
        }
        let inputs = ssa.prepare_materialization_inputs_v29(|roots| {
            roots.iter().map(|root| {
                let launch = root.source_launch().ok_or(
                    ProductionPipelineError::Geometry(
                        crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
                    ),
                )?;
                Ok(crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                    root.logical_name(), root.kernel_binding_bytes(), launch,
                ))
            }).collect::<Result<Vec<_>, ProductionPipelineError>>()
        }).map_err(|error| format!("ordered composition actual launch/descriptor join: {error}"))?;
        let PreparedSsaMaterializationV29 {
            mut semantic_ssa,
            ranked_roots,
            launch,
            bindings,
        } = inputs;
        if ranked_roots.len() != 1 || bindings.typed_descriptor_roots.len() != 1 {
            return Err("ordered composition requires one exact source root".into());
        }
        let work = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
            .map_err(|_| "ordered composition work limit overflow")?;
        let mut ledger = Ledger::new(
            Work::new(work),
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        // The source seed is a fixed borrowed-rustc payload, not complete HIR/RSS accounting.
        ledger
            .with_budget(|budget| budget.reserve_storage(source_seed.retained_storage_bytes()))
            .map_err(|error| format!("ordered composition retained source seed: {error}"))?;
        let materialized = ledger.with_budget(|budget| {
            context_handoff_v29::check_context_handoff_v29(
                &bindings.context_entries,
                &semantic_ssa,
                &launch,
                budget,
                |_, _| Ok(()),
            )
            .map_err(|error| format!("ordered composition actual context join: {error}"))?;
            let occurrences = semantic_ssa
                .try_capture_occurrences_with_budget_v1(budget)
                .map_err(|error| format!("ordered composition actual SSA occurrences: {error}"))?;
            budget
                .reserve_storage(occurrences.retained_storage())
                .map_err(|error| {
                    format!("ordered composition retained SSA occurrences: {error}")
                })?;
            let owner = Materialized::try_materialize_with_budget(
                semantic_ssa,
                launch,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .map_err(|error| format!("ordered composition pre-ranked materialization: {error}"))?;
            budget
                .reserve_storage(owner.retained_storage().retained_storage())
                .map_err(|error| {
                    format!("ordered composition retained canonical/source roster: {error}")
                })?;
            owner
                .verify_equivalence(budget)
                .map_err(|error| format!("ordered composition complete source replay: {error}"))?;
            Ok::<_, String>(owner)
        })?;
        drop(ranked_roots); // Diagnostic-only: no ranked proof is asserted.
        let emission =
            ledger.with_budget(|budget| {
                let (emission, receipt) =
                dialect_amdgcn::lower_ordered_program_composition_to_gfx942_xnack_minus_llvm_ir_v1(
                    materialized.composition(), budget,
                ).map_err(|error| format!("ordered composition typed LLVM emission: {error}"))?;
                budget
                    .reserve_storage(receipt.retained_storage())
                    .map_err(|error| format!("ordered composition retained emission: {error}"))?;
                Ok::<_, String>(emission)
            })?;
        Ok(AuthenticatedOrderedCompositionDiagnosticV1 {
            materialized,
            emission,
            bindings,
            source_seed,
            ledger,
        })
    }
}
