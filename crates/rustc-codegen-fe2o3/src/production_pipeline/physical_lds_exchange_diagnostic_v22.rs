//! Pre-ranked diagnostic-only actual-source continuation for MIR39/KIR22.
//! Mandatory ranked/formal checks and normal artifact/descriptor continuation
//! remain unavailable. This type cannot enter production worker preparation.
//! No diagnostic observation is converted into source or output custody.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionPhysicalLdsExchangePreRankedKirOwnerV22 as Materialized;

#[derive(Debug)]
pub(crate) enum PhysicalLdsExchangeStageErrorV22 {
    Source(fe2o3_lower_mir_kernel::ProductionPhysicalLdsExchangeSourceErrorV22),
    Emission(dialect_amdgcn::Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22),
    Resource(Resource),
    WrongTarget,
    WrongSourceProfile,
    UnsupportedRefinementBindings,
    ProtectedPublicationUnavailable,
}
impl std::fmt::Display for PhysicalLdsExchangeStageErrorV22 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(f, "exact source materialization: {error}"),
            Self::Emission(error) => write!(f, "canonical emission: {error}"),
            Self::Resource(error) => write!(f, "retained source resource ledger: {error}"),
            Self::WrongTarget => f.write_str("physical-lds-exchange source requires gfx942:xnack-"),
            Self::WrongSourceProfile => {
                f.write_str("physical-lds-exchange source requires exact MIR39 and one root")
            }
            Self::UnsupportedRefinementBindings => f.write_str(
                "physical-lds-exchange functional-refinement bindings are not supported",
            ),
            Self::ProtectedPublicationUnavailable => {
                f.write_str("physical-lds-exchange protected publication is not available")
            }
        }
    }
}
impl std::error::Error for PhysicalLdsExchangeStageErrorV22 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Emission(error) => Some(error),
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
fn stage_error(error: PhysicalLdsExchangeStageErrorV22) -> ProductionPipelineError {
    ProductionPipelineError::PhysicalLdsExchangeDiagnosticStage(Box::new(error))
}
fn resource(error: Resource) -> ProductionPipelineError {
    stage_error(PhysicalLdsExchangeStageErrorV22::Resource(error))
}

/// Live actual-source pre-ranked diagnostic custody, not a production target.
/// Retains compiler bindings and the cumulative ledger until all exports finish.
/// No conversion into worker handoff or artifact admission is provided.
pub(crate) struct AuthenticatedPhysicalLdsExchangeDiagnosticV22 {
    materialized: Materialized,
    emission: dialect_amdgcn::Gfx942PhysicalLdsExchangeCanonicalEmissionV22,
    native_observation: String,
    _bindings: AuthenticatedProductionBindings,
    _ledger: Ledger,
}
impl AuthenticatedPhysicalLdsExchangeDiagnosticV22 {
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn observe_with_budget<T>(
        &mut self,
        observe: impl FnOnce(
            &Materialized,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> T,
    ) -> T {
        self._ledger
            .with_budget(|budget| observe(&self.materialized, budget))
    }

    /// Test-only move of the same retained source ledger through an owned capture.
    /// All original owners remain live; no new source/authority constructor exists.
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn observe_with_owned_ledger<T>(
        self,
        observe: impl FnOnce(&Materialized, Ledger) -> (T, Ledger),
    ) -> (Self, T) {
        let Self {
            materialized,
            emission,
            native_observation,
            _bindings: bindings,
            _ledger: incoming_ledger,
        } = self;
        let floor = incoming_ledger.storage();
        let work = incoming_ledger.work();
        let peak = incoming_ledger.peak_storage();
        let limits = (
            incoming_ledger.work_limit(),
            incoming_ledger.storage_limit(),
        );
        let failures = (
            incoming_ledger.failed_work(),
            incoming_ledger.failed_storage(),
        );
        let (result, ledger) = observe(&materialized, incoming_ledger);
        assert_eq!(ledger.storage(), floor);
        assert!(ledger.work() >= work && ledger.peak_storage() >= peak);
        assert_eq!((ledger.work_limit(), ledger.storage_limit()), limits);
        if let Some(failed) = failures.0 {
            assert_eq!(ledger.failed_work(), Some(failed));
        }
        if let Some(failed) = failures.1 {
            assert_eq!(ledger.failed_storage(), Some(failed));
        }
        (
            Self {
                materialized,
                emission,
                native_observation,
                _bindings: bindings,
                _ledger: ledger,
            },
            result,
        )
    }

    pub(crate) fn materialized(&self) -> &Materialized {
        &self.materialized
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.emission.llvm_ir()
    }
    pub(crate) fn native_observation(&self) -> &str {
        &self.native_observation
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Dispatch observation only, from the sealed collector closure. The exact
    /// import below still authenticates the terminal, FnABI and source grammar.
    pub(crate) fn has_authenticated_physical_lds_exchange_v22(&self) -> bool {
        self.stage
            .closure
            .contains_physical_lds_exchange_terminal_v22(self.stage.tcx)
    }

    pub(crate) fn lower_physical_lds_exchange_diagnostic_v22(
        self,
    ) -> Result<AuthenticatedPhysicalLdsExchangeDiagnosticV22, Box<ProductionPipelineError>> {
        let ssa = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        if ssa.stage.semantic_ssa.source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V39
        {
            return Err(Box::new(stage_error(
                PhysicalLdsExchangeStageErrorV22::WrongSourceProfile,
            )));
        }
        if ssa.stage.bindings.rustc_target.profile()
            != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
        {
            return Err(Box::new(stage_error(
                PhysicalLdsExchangeStageErrorV22::WrongTarget,
            )));
        }
        if !ssa
            .stage
            .bindings
            .reference_effect_bindings
            .as_slice()
            .is_empty()
        {
            return Err(Box::new(stage_error(
                PhysicalLdsExchangeStageErrorV22::UnsupportedRefinementBindings,
            )));
        }
        // Only the explicit pre-ranked diagnostic route is supported. Normal
        // ranked/formal, worker handoff and protected publication remain closed.
        if !ssa
            .stage
            .bindings
            .transaction
            .compiler_custody
            .is_extraction_only()
        {
            return Err(Box::new(stage_error(
                PhysicalLdsExchangeStageErrorV22::ProtectedPublicationUnavailable,
            )));
        }
        if ssa
            .stage
            .bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != ssa.stage.bindings.rustc_identity_inventory.sha256()
        {
            return Err(Box::new(ProductionPipelineError::RustcLineageMismatch));
        }
        let inputs = ssa.prepare_materialization_inputs_v29(|typed_roots| {
            typed_roots.iter().map(|typed_root| {
                let source_launch = typed_root.source_launch().ok_or(
                    ProductionPipelineError::Geometry(
                        crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
                    ),
                )?;
                Ok(crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                    typed_root.logical_name(), typed_root.kernel_binding_bytes(), source_launch,
                ))
            }).collect::<Result<Vec<_>, ProductionPipelineError>>()
        })?;
        let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
            .map_err(|_| resource(Resource::Arithmetic))?;
        let mut ledger = Ledger::new(
            Work::new(work_limit),
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let prepared = ledger.with_budget(|budget| {
            materialize_prepared_with_budget_v29(
                inputs,
                budget,
                |_, _| Ok(()),
                |source, launch, budget| {
                    let owner = Materialized::try_materialize_with_budget(
                        source,
                        launch,
                        fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                        budget,
                    )
                    .map_err(|error| {
                        stage_error(PhysicalLdsExchangeStageErrorV22::Source(error))
                    })?;
                    let storage = owner.retained_storage();
                    Ok((owner, storage))
                },
            )
        })?;
        let PreparedMaterializationV29 {
            materialized,
            ranked_roots,
            bindings,
        } = prepared;
        if ranked_roots.len() != 1 || bindings.typed_descriptor_roots.len() != 1 {
            return Err(Box::new(stage_error(
                PhysicalLdsExchangeStageErrorV22::WrongSourceProfile,
            )));
        }
        // The shared preparation already reconciled every real typed root,
        // source-launch identity and descriptor ownership condition.
        drop(ranked_roots);
        let emission = ledger.with_budget(|budget| {
            let (emission, storage) =
                dialect_amdgcn::lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(
                    materialized.executable(),
                    budget,
                )
                .map_err(|error| stage_error(PhysicalLdsExchangeStageErrorV22::Emission(error)))?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            Ok::<_, ProductionPipelineError>(emission)
        })?;
        let native_observation = ledger.with_budget(|budget| {
            let (text, storage) =
                dialect_amdgcn::physical_lds_exchange_native_observation_input_v22(
                    materialized.executable(),
                    &emission,
                    budget,
                )
                .map_err(|error| stage_error(PhysicalLdsExchangeStageErrorV22::Emission(error)))?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            Ok::<_, ProductionPipelineError>(text)
        })?;
        Ok(AuthenticatedPhysicalLdsExchangeDiagnosticV22 {
            materialized,
            emission,
            native_observation,
            _bindings: bindings,
            _ledger: ledger,
        })
    }
}
