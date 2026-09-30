//! Explicit admitted-runtime CFG-model stages for the two nominal mixed routes.
//! The copied observation is inert; it is not retained proof/launch authority.

use super::*;
use fe2o3_verifier::{
    FunctionalRefinementVerusRuntimeLeaseV1 as Runtime, MixedOptimizerCfgSubjectV27 as Subject,
};

macro_rules! cfg_stage {
    ($name:ident, $route:ident, $prepare:ident, $execute:ident) => {
        impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
            /// Prepare and execute the exact source/optimized conditional CFG
            /// model using an admitted runtime. Missing admission or failed
            /// proof is a typed error, never a fallback to witness observation.
            /// This nondefault stage does not discharge concrete memory/call,
            /// MIR-lowering, Worker-origin or launch-premise obligations.
            pub(crate) fn $name(
                self,
                runtime: &Runtime,
                timeout_seconds: u32,
            ) -> Result<SourceOwnedCompilationContinuationV29<Subject>, Error> {
                self.$route(|source, handoff, _, _, budget| {
                    let request = fe2o3_verifier::$prepare(source, handoff, budget)
                        .map_err(Error::ConditionalMixedCfg)?;
                    let executed =
                        fe2o3_verifier::$execute(request, runtime, budget, timeout_seconds)
                            .map_err(Error::ConditionalMixedCfg)?;
                    // Inspection can refuse after construction. Always settle
                    // the executed producer before propagating the first error.
                    let observed = executed
                        .replay_signed_receipt(budget)
                        .and_then(|()| executed.subject(budget));
                    let settled = executed.discard(budget);
                    let subject = observed.map_err(Error::ConditionalMixedCfg)?;
                    settled.map_err(Error::ConditionalMixedCfg)?;
                    Ok(subject)
                })
            }
        }
    };
}
cfg_stage!(
    verify_original_source_mixed_worklist_cfg_v27,
    with_original_source_conditional_mixed_worklist_v26,
    prepare_mixed_worklist_cfg_refinement_v27,
    execute_mixed_worklist_cfg_refinement_v27
);
cfg_stage!(
    verify_original_source_mixed_pure_cse_cfg_v27,
    with_original_source_conditional_mixed_pure_cse_v26,
    prepare_mixed_pure_cse_cfg_refinement_v27,
    execute_mixed_pure_cse_cfg_refinement_v27
);
