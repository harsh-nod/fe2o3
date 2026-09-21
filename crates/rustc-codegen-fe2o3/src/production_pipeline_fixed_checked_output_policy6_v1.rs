//! One fixed checked schedule selected only by the retained source owner.
//!
//! This extraction facade does not replace the protected publication pipeline.
use super::checked_output_policy6_v1::{
    CheckedOutputPolicy6StageErrorV1, CheckedOutputTargetProductionCompilation as Direct,
};
use super::erased_checked_output_policy6_v1::ErasedCheckedOutputTargetProductionCompilationV1 as Erased;
use super::{ProductionPipelineError, RankedVerifiedProductionCompilation};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1 as SourcePolicy;

#[path = "production_pipeline_fixed_native_checked_output_policy6_v1.rs"]
pub(crate) mod native_continuation;

#[cfg(test)]
#[path = "production_pipeline_fixed_checked_output_policy6_v1_tests.rs"]
mod tests;

#[allow(
    clippy::large_enum_variant,
    reason = "move-only in-place custody, not a boxed copy"
)]
enum Stage {
    Direct(Direct),
    Erased(Erased),
}

/// The complete existing stage remains in place, including original N/(E),
/// B/C/S/O, native artifacts, ranked roster and authenticated collector bindings.
pub(crate) struct FixedCheckedOutputProductionCompilationPolicy6V1 {
    stage: Stage,
    retained_storage_floor: usize,
}

fn resource(error: Resource) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputPolicy6Stage(CheckedOutputPolicy6StageErrorV1::Resource(
        error,
    ))
}

const HEADER_WORK: usize = 2;

fn direct_native_source(
    admitted: &fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1,
) -> &Owner {
    admitted
        .source_semantic_kir()
        .pre_ranked_executable()
        .expect("Direct6 admission retains actual pre-ranked V12 N")
}

fn additional_header(source: SourcePolicy) -> Result<usize, ProductionPipelineError> {
    let moved = match source {
        SourcePolicy::RawEmpty => std::mem::size_of::<Direct>(),
        SourcePolicy::UnitLocal => std::mem::size_of::<Erased>(),
    };
    std::mem::size_of::<FixedCheckedOutputProductionCompilationPolicy6V1>()
        .checked_sub(moved)
        .ok_or_else(|| resource(Resource::Arithmetic))
}

/// Only the facade's additional enum/header bytes enter this constant-size
/// domain. The closure cannot access its budget. Source, analysis and native
/// allocation domains remain owned by the unchanged selected stage.
fn transfer_header<T>(
    source: SourcePolicy,
    budget: &mut Budget<'_>,
    lower: impl FnOnce() -> Result<T, ProductionPipelineError>,
) -> Result<T, ProductionPipelineError> {
    budget.charge_work(HEADER_WORK).map_err(resource)?;
    let additional = additional_header(source)?;
    budget.reserve_storage(additional).map_err(resource)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(lower));
    let cleanup = budget.release_storage(additional);
    match result {
        Err(payload) => std::panic::resume_unwind(payload),
        Ok(result) => {
            cleanup.map_err(resource)?;
            result
        }
    }
}

/// Checks the transferred receipt against the owning phase's existing ceiling.
/// This O(1) reservation is a receipt-bound check, not a replay of the source or
/// a second charge to the selected stage's analysis ledger.
fn combined_storage_floor(
    inherited: usize,
    additional: usize,
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionPipelineError> {
    if budget.storage() != 0 {
        return Err(resource(Resource::Accounting));
    }
    let combined = inherited
        .checked_add(additional)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.reserve_storage(combined).map_err(resource)?;
    budget.release_storage(combined).map_err(resource)?;
    Ok(combined)
}

impl RankedVerifiedProductionCompilation {
    /// No pass list, route flag, fallback or independently supplied source policy.
    pub(crate) fn lower_fixed_checked_output_policy6_v1(
        self,
    ) -> Result<FixedCheckedOutputProductionCompilationPolicy6V1, ProductionPipelineError> {
        let source = self.ranked.materialized().helper_source_policy_v1();
        // This new ledger meters only O(1) facade transfer. Neither existing
        // analysis ledger is replaced, passed through or reset here.
        let mut work = Work::new(HEADER_WORK);
        let mut budget = Budget::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let stage = transfer_header(source, &mut budget, || match source {
            SourcePolicy::RawEmpty => self.lower_checked_output_policy6_v1().map(Stage::Direct),
            SourcePolicy::UnitLocal => self
                .lower_silent_unit_checked_output_policy6_v1()
                .map(Stage::Erased),
        })?;
        let inherited = match &stage {
            Stage::Direct(stage) => stage.retained_storage_floor_v1(),
            Stage::Erased(stage) => stage.retained_storage_floor_v1(),
        };
        let retained_storage_floor =
            combined_storage_floor(inherited, additional_header(source)?, &mut budget)?;
        Ok(FixedCheckedOutputProductionCompilationPolicy6V1 {
            stage,
            retained_storage_floor,
        })
    }
}

impl FixedCheckedOutputProductionCompilationPolicy6V1 {
    pub(crate) fn output(&self) -> &Owner {
        match &self.stage {
            Stage::Direct(stage) => stage.output().output(),
            Stage::Erased(stage) => stage.output().output(),
        }
    }

    pub(crate) fn checked_output(
        &self,
    ) -> &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1 {
        match &self.stage {
            Stage::Direct(stage) => stage.output().checked_output(),
            Stage::Erased(stage) => stage.output().checked_output(),
        }
    }

    pub(crate) fn original_digest(&self) -> [u8; 32] {
        match &self.stage {
            Stage::Direct(stage) => *direct_native_source(stage.output())
                .canonical()
                .identity()
                .digest(),
            Stage::Erased(stage) => *stage
                .output()
                .original_source()
                .executable()
                .canonical()
                .identity()
                .digest(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_bound_owner_v1(&self) -> &Owner {
        match &self.stage {
            Stage::Direct(stage) => stage.output().bound(),
            Stage::Erased(stage) => stage.output().bound(),
        }
    }

    /// Actual binder input: original N for Direct, retained E after erasure.
    #[cfg(test)]
    pub(crate) fn test_prebind_owner_v1(&self) -> &Owner {
        match &self.stage {
            Stage::Direct(stage) => direct_native_source(stage.output()),
            Stage::Erased(stage) => stage.output().erased(),
        }
    }

    /// Actual retained original N, not an owner reconstructed from its bytes.
    #[cfg(test)]
    pub(crate) fn test_original_owner_v1(&self) -> &Owner {
        match &self.stage {
            Stage::Direct(stage) => direct_native_source(stage.output()),
            Stage::Erased(stage) => stage.output().original_source().executable(),
        }
    }

    #[cfg(test)]
    pub(crate) fn original_canonical_bytes(&self) -> &[u8] {
        match &self.stage {
            Stage::Direct(stage) => direct_native_source(stage.output())
                .canonical()
                .canonical_bytes(),
            Stage::Erased(stage) => stage
                .output()
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes(),
        }
    }

    pub(crate) fn erased_digest(&self) -> Option<&[u8; 32]> {
        match &self.stage {
            Stage::Direct(_) => None,
            Stage::Erased(stage) => Some(stage.output().erased().canonical().identity().digest()),
        }
    }

    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_storage_floor
    }

    #[cfg(test)]
    pub(crate) fn kernels(&self) -> &[fe2o3_kernel_ir::FormalMemoryObligations] {
        match &self.stage {
            Stage::Direct(stage) => stage.output().kernels(),
            Stage::Erased(stage) => stage.output().kernels(),
        }
    }

    /// Exercises the actual unsigned component only. Signed native preparation
    /// and its mandatory missing-proof refusal remain separate and unchanged.
    #[cfg(test)]
    pub(crate) fn exercise_final_receipt_component_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), super::native_checked_output_handoff_v1::NativeOutputHandoffErrorV1> {
        if budget.storage() < self.retained_storage_floor {
            return Err(
                super::native_checked_output_handoff_v1::NativeOutputHandoffErrorV1::Resource(
                    Resource::Accounting,
                ),
            );
        }
        match &self.stage {
            Stage::Direct(stage) => stage.exercise_final_receipt_component_v1(budget),
            Stage::Erased(stage) => stage.exercise_final_receipt_component_v1(budget),
        }
    }

    #[cfg(test)]
    pub(crate) fn semantic(&self) -> &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 {
        match &self.stage {
            Stage::Direct(stage) => stage.output().source_semantic_kir().semantic().semantic(),
            Stage::Erased(stage) => stage
                .output()
                .original_source()
                .semantic_ssa()
                .source_semantic(),
        }
    }

    #[cfg(test)]
    pub(crate) fn exercise_original_receipt_component_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), super::native_checked_output_handoff_v1::policy6::original_receipts::OriginalNativeInputReceiptErrorV1>{
        if budget.storage() < self.retained_storage_floor {
            return Err(Resource::Accounting.into());
        }
        match &self.stage {
            Stage::Direct(stage) => stage.exercise_original_receipt_component_v1(budget),
            Stage::Erased(stage) => stage.exercise_original_receipt_component_v1(budget),
        }
    }

    #[cfg(test)]
    pub(crate) fn original_module(&self) -> &fe2o3_kernel_ir::Module {
        match &self.stage {
            Stage::Direct(stage) => direct_native_source(stage.output()).module(),
            Stage::Erased(stage) => stage.output().original_source().executable().module(),
        }
    }

    #[cfg(test)]
    pub(crate) fn probe_native_source_lineage_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<(), ProductionPipelineError> {
        self.prepare_native_checked_output_v1(budget).map(drop)
    }

    /// Delegates every extraction-only, rustc, source, ranked and native check
    /// to the existing owning endpoint. No protected publisher conversion.
    pub(crate) fn into_worker_handoff_extraction_v1(
        self,
    ) -> Result<
        (
            fe2o3_compiler_ffi::CompilerModuleHandoffV2,
            fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
        ),
        ProductionPipelineError,
    > {
        match self.stage {
            Stage::Direct(stage) => stage.into_worker_handoff_extraction_v1(),
            Stage::Erased(stage) => stage.into_worker_handoff_extraction_v1(),
        }
    }
}
