//! Normal exact-source continuation for the fixed MIR36/KIR19 body profile.
//! No diagnostic observation is converted into source or output custody.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    ProductionCompleteBodyCheckedKirOwnerV19 as Checked,
    ProductionCompleteBodyPreRankedKirOwnerVNext as Materialized,
};

#[derive(Debug)]
pub(crate) enum CompleteBodyStageErrorVNext {
    Source(fe2o3_lower_mir_kernel::ProductionCompleteBodySourceErrorVNext),
    Checks(fe2o3_lower_mir_kernel::ProductionCompleteBodyCheckErrorV19),
    Emission(dialect_amdgcn::Gfx942CompleteBodyCanonicalEmissionErrorV19),
    Resource(Resource),
    WrongTarget,
    WrongSourceProfile,
    UnsupportedRefinementBindings,
    ProtectedPublicationUnavailable,
}
impl std::fmt::Display for CompleteBodyStageErrorVNext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(f, "exact source materialization: {error}"),
            Self::Checks(error) => write!(f, "mandatory ranked/formal checks: {error}"),
            Self::Emission(error) => write!(f, "canonical emission: {error}"),
            Self::Resource(error) => write!(f, "retained source resource ledger: {error}"),
            Self::WrongTarget => f.write_str("complete-body source requires gfx942:xnack-"),
            Self::WrongSourceProfile => {
                f.write_str("complete-body source requires exact MIR36 and one root")
            }
            Self::UnsupportedRefinementBindings => {
                f.write_str("complete-body functional-refinement bindings are not supported")
            }
            Self::ProtectedPublicationUnavailable => {
                f.write_str("complete-body protected publication is not available")
            }
        }
    }
}
impl std::error::Error for CompleteBodyStageErrorVNext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Checks(error) => Some(error),
            Self::Emission(error) => Some(error),
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
fn stage_error(error: CompleteBodyStageErrorVNext) -> ProductionPipelineError {
    ProductionPipelineError::CompleteBodyStage(Box::new(error))
}
fn resource(error: Resource) -> ProductionPipelineError {
    stage_error(CompleteBodyStageErrorVNext::Resource(error))
}

/// One exact normal target source owner plus the independently reconciled
/// canonical-to-LLVM emission. The same owned source ledger survives handoff.
pub(crate) struct AuthenticatedCompleteBodyTargetModuleV19 {
    checked: Checked,
    emission: dialect_amdgcn::Gfx942CompleteBodyCanonicalEmissionV19,
    bindings: AuthenticatedProductionBindings,
    ledger: Ledger,
}
impl AuthenticatedCompleteBodyTargetModuleV19 {
    pub(crate) const fn checked(&self) -> &Checked {
        &self.checked
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.emission.llvm_ir()
    }
    pub(crate) const fn target_name(&self) -> &'static str {
        "gfx942:xnack-"
    }
    pub(crate) fn into_parts(
        self,
    ) -> (
        Checked,
        fe2o3_compiler_ffi::DeviceTargetV1,
        dialect_amdgcn::Gfx942CompleteBodyCanonicalEmissionV19,
        Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
        Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
        Ledger,
    ) {
        (
            self.checked,
            self.bindings.rustc_target.device_target(),
            self.emission,
            self.bindings.typed_descriptor_roots,
            self.bindings.transaction.compiler_ffi_envelope,
            self.ledger,
        )
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Dispatch observation only, from the sealed collector closure. The exact
    /// import below still authenticates the terminal, FnABI and source grammar.
    pub(crate) fn has_authenticated_complete_body_v19(&self) -> bool {
        self.stage
            .closure
            .contains_complete_body_terminal_v19(self.stage.tcx)
    }

    pub(crate) fn lower_complete_body_target_v19(
        self,
    ) -> Result<AuthenticatedCompleteBodyTargetModuleV19, Box<ProductionPipelineError>> {
        let ssa = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        if ssa.stage.semantic_ssa.source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V36
        {
            return Err(Box::new(stage_error(
                CompleteBodyStageErrorVNext::WrongSourceProfile,
            )));
        }
        if ssa.stage.bindings.rustc_target.profile()
            != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
        {
            return Err(Box::new(stage_error(
                CompleteBodyStageErrorVNext::WrongTarget,
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
                CompleteBodyStageErrorVNext::UnsupportedRefinementBindings,
            )));
        }
        // The normal inert driver is supported. Protected publication and
        // semantic handoff V3 need their own new exact protocol owners.
        if !ssa
            .stage
            .bindings
            .transaction
            .compiler_custody
            .is_extraction_only()
        {
            return Err(Box::new(stage_error(
                CompleteBodyStageErrorVNext::ProtectedPublicationUnavailable,
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
                    .map_err(|error| stage_error(CompleteBodyStageErrorVNext::Source(error)))?;
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
                CompleteBodyStageErrorVNext::WrongSourceProfile,
            )));
        }
        // The shared preparation already reconciled every real typed root,
        // source-launch identity and descriptor ownership condition.
        drop(ranked_roots);
        let source_storage = materialized.retained_storage();
        let checked = ledger.with_budget(|budget| {
            let owner = Checked::try_check(materialized, budget)
                .map_err(|error| stage_error(CompleteBodyStageErrorVNext::Checks(error)))?;
            let added = owner
                .retained_storage()
                .checked_sub(source_storage)
                .ok_or_else(|| resource(Resource::Accounting))?;
            budget.reserve_storage(added).map_err(resource)?;
            Ok::<_, ProductionPipelineError>(owner)
        })?;
        let emission =
            ledger.with_budget(|budget| {
                let (emission, storage) =
                dialect_amdgcn::lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(
                    checked.executable(), budget)
                    .map_err(|error| stage_error(CompleteBodyStageErrorVNext::Emission(error)))?;
                budget
                    .reserve_storage(storage.retained_storage())
                    .map_err(resource)?;
                Ok::<_, ProductionPipelineError>(emission)
            })?;
        Ok(AuthenticatedCompleteBodyTargetModuleV19 {
            checked,
            emission,
            bindings,
            ledger,
        })
    }
}
