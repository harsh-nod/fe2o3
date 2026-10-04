//! Explicit inert source/N capture. No checked-output, publication, or default route.
use super::*;
use crate::production_ranked_projection_v1::scalar_emission_capture_v1::CapturedRankedSourceV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionOwnerV1;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Runs the genuine source, SSA, materialization and ranked projection once,
    /// retaining the opt-in source/N attachment and existing induction reports.
    /// This stops before ranked-roster authentication and all optimizer/native
    /// handoffs. It does not select a shipping pipeline or authorize a rewrite.
    #[allow(dead_code)]
    pub(crate) fn capture_ranked_scalar_emission_v1(
        self,
    ) -> Result<CapturedRankedSourceV1, Box<ProductionPipelineError>> {
        let ssa = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?;
        let PreparedMaterializationV29 {
            materialized: capture,
            ranked_roots: roots,
            bindings,
        } = ssa.materialize_prepared_v29(
            |_, _| Ok(()),
            |ssa, launch, budget| {
                let capture = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
                    ssa,
                    launch,
                    Default::default(),
                    budget,
                )
                .map_err(|error| ProductionPipelineError::ScalarEmissionCapture(Box::new(error)))?;
                let retained = capture.retained_analysis_storage_v1();
                Ok((capture, retained))
            },
        )?;
        let resource_error =
            |error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1| {
                ProductionPipelineError::ScalarEmissionCapture(Box::new(error.into()))
            };
        let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
            .map_err(|_| {
            resource_error(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
            )
        })?;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        budget
            .reserve_storage(capture.retained_analysis_storage_v1())
            .map_err(resource_error)?;
        CapturedRankedSourceV1::try_project_v1(
            capture,
            &roots,
            &bindings.reference_effect_bindings,
            &mut budget,
        )
        .map_err(Box::new)
    }
}
