//! Normal exact-source continuation for the fixed MIR38/KIR21 body profile.
//! No diagnostic observation is converted into source or output custody.
//! Full-prefix input readability/initialization, global alias and compiler-ABI conditions remain typed and unresolved
//! through inert preparation; no protected/finalizer/native launch route is admitted.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    ProductionPhysicalGlobalCopyCheckedKirOwnerV21 as Checked,
    ProductionPhysicalGlobalCopyPreRankedKirOwnerV21 as Materialized,
};

#[derive(Debug)]
pub(crate) enum PhysicalGlobalCopyTargetStageErrorV21 {
    Source(fe2o3_lower_mir_kernel::ProductionPhysicalGlobalCopySourceErrorV21),
    Checks(fe2o3_lower_mir_kernel::ProductionPhysicalGlobalCopyCheckErrorV21),
    Emission(dialect_amdgcn::Gfx942PhysicalGlobalCopyCanonicalEmissionErrorV21),
    Resource(Resource),
    Descriptor(crate::compiler_descriptor::CompilerDescriptorError),
    WrongTarget,
    WrongSourceProfile,
    UnsupportedRefinementBindings,
    ProtectedPublicationUnavailable,
}
impl std::fmt::Display for PhysicalGlobalCopyTargetStageErrorV21 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(f, "exact source materialization: {error}"),
            Self::Descriptor(error) => write!(f, "exact source/compiler ABI preparation: {error}"),
            Self::Checks(error) => write!(f, "mandatory ranked/formal checks: {error}"),
            Self::Emission(error) => write!(f, "canonical emission: {error}"),
            Self::Resource(error) => write!(f, "retained source resource ledger: {error}"),
            Self::WrongTarget => f.write_str("physical-global-copy source requires gfx942:xnack-"),
            Self::WrongSourceProfile => {
                f.write_str("physical-global-copy source requires exact MIR38 and one root")
            }
            Self::UnsupportedRefinementBindings => {
                f.write_str("physical-global-copy functional-refinement bindings are not supported")
            }
            Self::ProtectedPublicationUnavailable => {
                f.write_str("physical-global-copy protected publication is not available")
            }
        }
    }
}
impl std::error::Error for PhysicalGlobalCopyTargetStageErrorV21 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Descriptor(error) => Some(error),
            Self::Checks(error) => Some(error),
            Self::Emission(error) => Some(error),
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
fn stage_error(error: PhysicalGlobalCopyTargetStageErrorV21) -> ProductionPipelineError {
    ProductionPipelineError::PhysicalGlobalCopyTargetStage(Box::new(error))
}
fn resource(error: Resource) -> ProductionPipelineError {
    stage_error(PhysicalGlobalCopyTargetStageErrorV21::Resource(error))
}

/// One exact normal target source owner plus the independently reconciled
/// canonical-to-LLVM emission. The same owned source ledger survives handoff.
pub(crate) struct AuthenticatedPhysicalGlobalCopyTargetModuleV21 {
    checked: Checked,
    emission: dialect_amdgcn::Gfx942PhysicalGlobalCopyCanonicalEmissionV21,
    abi: crate::compiler_descriptor::physical_global_copy_v21::PhysicalGlobalCopyPreparedAbiV21,
    bindings: AuthenticatedProductionBindings,
    ledger: Ledger,
}
impl AuthenticatedPhysicalGlobalCopyTargetModuleV21 {
    pub(crate) const fn checked(&self) -> &Checked {
        &self.checked
    }
    pub(crate) fn unresolved_abi_conditions(&self) -> &'static str {
        self.abi.unresolved_conditions()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.emission.llvm_ir()
    }
    pub(crate) const fn target_name(&self) -> &'static str {
        "gfx942:xnack-"
    }
    #[cfg(test)]
    pub(crate) fn qualify_actual_abi_controls_v21(&mut self) -> usize {
        self.ledger.with_budget(|budget| {
            crate::compiler_descriptor::physical_global_copy_v21::qualify_actual_owner_abi_controls_v21(
                &self.checked,
                &self.bindings.typed_descriptor_roots,
                &self.abi,
                budget,
            )
        })
    }
    #[cfg(test)]
    pub(crate) fn observe_with_budget<T>(
        &mut self,
        observe: impl FnOnce(
            &Checked,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> T,
    ) -> T {
        self.ledger
            .with_budget(|budget| observe(&self.checked, budget))
    }
    pub(crate) fn into_parts(
        self,
    ) -> (
        Checked,
        fe2o3_compiler_ffi::DeviceTargetV1,
        dialect_amdgcn::Gfx942PhysicalGlobalCopyCanonicalEmissionV21,
        Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
        Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
        crate::compiler_descriptor::physical_global_copy_v21::PhysicalGlobalCopyPreparedAbiV21,
        Ledger,
    ) {
        (
            self.checked,
            self.bindings.rustc_target.device_target(),
            self.emission,
            self.bindings.typed_descriptor_roots,
            self.bindings.transaction.compiler_ffi_envelope,
            self.abi,
            self.ledger,
        )
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn lower_physical_global_copy_target_v21(
        self,
    ) -> Result<AuthenticatedPhysicalGlobalCopyTargetModuleV21, Box<ProductionPipelineError>> {
        let ssa = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        if ssa.stage.semantic_ssa.source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V38
        {
            return Err(Box::new(stage_error(
                PhysicalGlobalCopyTargetStageErrorV21::WrongSourceProfile,
            )));
        }
        if ssa.stage.bindings.rustc_target.profile()
            != fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
        {
            return Err(Box::new(stage_error(
                PhysicalGlobalCopyTargetStageErrorV21::WrongTarget,
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
                PhysicalGlobalCopyTargetStageErrorV21::UnsupportedRefinementBindings,
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
                PhysicalGlobalCopyTargetStageErrorV21::ProtectedPublicationUnavailable,
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
                        stage_error(PhysicalGlobalCopyTargetStageErrorV21::Source(error))
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
                PhysicalGlobalCopyTargetStageErrorV21::WrongSourceProfile,
            )));
        }
        // The shared preparation already reconciled every real typed root,
        // source-launch identity and descriptor ownership condition.
        drop(ranked_roots);
        let source_storage = materialized.retained_storage();
        let checked = ledger.with_budget(|budget| {
            let owner = Checked::try_check(materialized, budget).map_err(|error| {
                stage_error(PhysicalGlobalCopyTargetStageErrorV21::Checks(error))
            })?;
            let added = owner
                .retained_storage()
                .checked_sub(source_storage)
                .ok_or_else(|| resource(Resource::Accounting))?;
            budget.reserve_storage(added).map_err(resource)?;
            Ok::<_, ProductionPipelineError>(owner)
        })?;
        let abi = ledger.with_budget(|budget| {
            let abi =
                crate::compiler_descriptor::physical_global_copy_v21::prepare_physical_global_copy_abi_v21(
                    &checked,
                    &bindings.typed_descriptor_roots,
                    budget,
                )
                .map_err(|error| {
                    stage_error(PhysicalGlobalCopyTargetStageErrorV21::Descriptor(error))
                })?;
            budget
                .reserve_storage(abi.retained_storage())
                .map_err(resource)?;
            Ok::<_, ProductionPipelineError>(abi)
        })?;
        let emission = ledger.with_budget(|budget| {
            let (emission, storage) =
                dialect_amdgcn::lower_canonical_v21_compiler_module_to_gfx942_xnack_minus_llvm_ir(
                    checked.executable(),
                    budget,
                )
                .map_err(|error| {
                    stage_error(PhysicalGlobalCopyTargetStageErrorV21::Emission(error))
                })?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            Ok::<_, ProductionPipelineError>(emission)
        })?;
        Ok(AuthenticatedPhysicalGlobalCopyTargetModuleV21 {
            checked,
            emission,
            abi,
            bindings,
            ledger,
        })
    }
}
