//! Owning pre-artifact source-progress/original-N consistency. No shipping selector.
#![allow(dead_code)]
use super::*;
use crate::production_ranked_projection_v1::{
    ProductionRankedRootInputV1,
    guarded_source_progress_v1::{GuardedRankedSourceV1, resources},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionScalarSsaEmissionErrorV1 as CaptureError,
    ProductionScalarSsaEmissionOwnerV1 as Capture,
};
use std::mem::size_of;

/// Retains genuine C, ranked results, complete guard-site consumption and the
/// original authenticated transaction exactly once. No from-parts, Clone,
/// legacy-roster or artifact conversion exists. This is not final admission.
///
/// Its RESERVED receipt covers C and new wrapper/input/report/site capacities.
/// Replay preserves the complete surviving caller floor, not merely the owner
/// receipt; only the private constructor transfers already-dropped input storage.
/// Existing source/ranked/descriptor/launch payload and diagnostic allocation
/// domains remain inherited; this is not a complete allocator/RSS bound.
pub(crate) struct ProductionGuardedRankedSourceV1 {
    projected: GuardedRankedSourceV1,
    bindings: AuthenticatedProductionBindings,
    retained: usize,
    required_floor: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

fn resource(error: Resource) -> Box<ProductionPipelineError> {
    Box::new(materialization_resource_error_v29(error))
}
fn panicked() -> Box<ProductionPipelineError> {
    Box::new(ProductionPipelineError::ScalarEmissionCapture(Box::new(
        CaptureError::Panicked,
    )))
}

fn root_inputs(
    typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> Result<(Vec<ProductionRankedRootInputV1>, usize), ProductionPipelineError> {
    let map = materialization_resource_error_v29;
    budget
        .reserve_storage(size_of::<Vec<ProductionRankedRootInputV1>>())
        .map_err(map)?;
    let mut roots =
        resources::table::<ProductionRankedRootInputV1>(typed.len(), budget).map_err(map)?;
    let mut retained = size_of::<Vec<ProductionRankedRootInputV1>>()
        .checked_add(
            resources::bytes::<ProductionRankedRootInputV1>(roots.capacity()).map_err(map)?,
        )
        .ok_or_else(|| map(Resource::Arithmetic))?;
    for root in typed {
        budget.charge_work(1).map_err(map)?;
        let launch = root
            .source_launch()
            .ok_or(ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
        ))?;
        let input = ProductionRankedRootInputV1::guarded_with_budget_v1(
            root.logical_name(),
            root.kernel_binding_bytes(),
            launch,
            budget,
        )
        .map_err(ProductionPipelineError::RankedProjection)?;
        retained = retained
            .checked_add(input.guarded_name_capacity_v1())
            .ok_or_else(|| map(Resource::Arithmetic))?;
        if roots.len() == roots.capacity() {
            return Err(map(Resource::Accounting));
        }
        roots.push(input);
    }
    Ok((roots, retained))
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Explicit production construction with one caller-owned canonical ledger.
    /// Import/SSA retain their existing accounting domains; all new owned
    /// metadata and subsequent context/C/report/guard/projection work use this
    /// same Budget slot. Success leaves the complete stage RESERVED. No source
    /// name, environment or driver selector enables this path automatically.
    pub(crate) fn prepare_guarded_ranked_source_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionGuardedRankedSourceV1, Box<ProductionPipelineError>> {
        resources::owned(budget, 0, resource, panicked, |budget| {
            let ssa = self
                .import_semantic_mir()?
                .construct_semantic_middle_end()?
                .construct_semantic_ssa()?;
            let mut inputs_storage = 0;
            let prepared = ssa.prepare_materialization_inputs_v29(|typed| {
                let (roots, retained) = root_inputs(typed, budget)?;
                inputs_storage = retained;
                Ok(roots)
            })?;
            let PreparedMaterializationV29 {
                materialized: capture,
                ranked_roots,
                bindings,
            } = materialize_prepared_with_budget_v29(
                prepared,
                budget,
                |_, _| Ok(()),
                |ssa, launch, budget| {
                    let capture = Capture::try_materialize_with_budget_v1(
                        ssa,
                        launch,
                        Default::default(),
                        budget,
                    )
                    .map_err(|error| {
                        ProductionPipelineError::ScalarEmissionCapture(Box::new(error))
                    })?;
                    let retained = capture.retained_analysis_storage_v1();
                    Ok((capture, retained))
                },
            )?;
            let mut projected = GuardedRankedSourceV1::try_project_v1(
                capture,
                &ranked_roots,
                &bindings.reference_effect_bindings,
                budget,
            )
            .map_err(ProductionPipelineError::RankedProjection)?;
            drop(ranked_roots);
            budget.release_storage(inputs_storage).map_err(resource)?;
            projected
                .release_input_floor_v1(budget, inputs_storage)
                .map_err(ProductionPipelineError::RankedProjection)?;
            let delta = size_of::<ProductionGuardedRankedSourceV1>()
                .checked_sub(size_of::<GuardedRankedSourceV1>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(delta).map_err(resource)?;
            let retained = projected
                .retained_storage()
                .checked_add(delta)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            let result = ProductionGuardedRankedSourceV1 {
                projected,
                bindings,
                retained,
                required_floor: budget.storage(),
                slot: budget as *mut _ as usize,
                ledger: budget.work_ledger_identity_v1(),
            };
            Ok((result, retained))
        })
    }
}

impl ProductionGuardedRankedSourceV1 {
    pub(crate) fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn grants_authority(&self) -> bool {
        false
    }
    pub(crate) fn source(&self) -> &GuardedRankedSourceV1 {
        &self.projected
    }

    /// Fresh inert original-N consistency replay. This does not manufacture
    /// frozen ranked evidence or a source proof, and it invokes no projector.
    pub(crate) fn replay_consistency_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), Box<ProductionPipelineError>> {
        if budget as *mut _ as usize != self.slot
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() < self.required_floor
        {
            return Err(resource(Resource::Accounting));
        }
        let source = self
            .projected
            .capture()
            .original()
            .semantic_ssa()
            .source_semantic();
        crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
            &self.bindings.typed_descriptor_roots,
            source,
        )
        .map_err(ProductionPipelineError::DescriptorEvidence)?;
        self.bindings
            .context_entries
            .validate_source(source)
            .map_err(|error| {
                ProductionPipelineError::SemanticImport(
                    crate::collector::ProductionSemanticImportErrorV1::BodyConstruction(Box::new(
                        error,
                    )),
                )
            })?;
        self.projected
            .replay_consistency_v1(budget)
            .map_err(ProductionPipelineError::RankedProjection)
            .map_err(Box::new)
    }
}
