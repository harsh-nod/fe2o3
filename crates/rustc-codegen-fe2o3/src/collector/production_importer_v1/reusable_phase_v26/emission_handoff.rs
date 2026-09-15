//! Consumed Context/source custody across ranked target-neutral lowering.
use super::*;
use crate::production_pipeline::ProductionPipelineError as Error;
use fe2o3_lower_mir_kernel::{
    ProductionKernelContextLoweringInputV1, ProductionRankedSemanticProjectionModuleReceiptV1,
    ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
};

pub(crate) struct ConsumedSourceLoweringInputs {
    contexts: Vec<ProductionKernelContextLoweringInputV1>,
    source: Option<SourceStatus>,
}

impl AuthenticatedProductionKernelContextsV1 {
    pub(crate) fn into_consumed_source_lowering_inputs(
        mut self,
        inventory: &crate::collector::AuthenticatedRustcIdentityInventoryV3,
        target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
        ranked_roots: &[crate::production_ranked_projection_v1::ProductionRankedRootProgramV1],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    ) -> PhaseResult<ConsumedSourceLoweringInputs> {
        let source = self.reusable_phase_source.take();
        // Keep all existing Context/root/target/launch checks in the original
        // consuming method. Only retain the previously dropped source owner.
        let contexts = self.into_lowering_inputs(inventory, target, ranked_roots, typed_roots)?;
        Ok(ConsumedSourceLoweringInputs { contexts, source })
    }
}

impl ConsumedSourceLoweringInputs {
    pub(crate) fn lower(
        self,
        receipt: ProductionRankedSemanticProjectionModuleReceiptV1,
        limits: ProductionSemanticKirLimitsV1,
    ) -> Result<ProductionSemanticKirOwnerV1, Error> {
        let Self { contexts, source } = self;
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_source_emission(
            receipt,
            limits,
            contexts,
            |scope| match source.as_ref() {
                Some(SourceStatus::Checked(seal)) => seal
                    .rebind(scope.semantic_ssa())
                    .and_then(ssa_protocol::CheckedSsa::into_emission_owner)
                    .map_err(Error::SemanticImport)?
                    .lower(scope),
                Some(SourceStatus::Unsupported(detail)) => {
                    Err(Error::SemanticImport(rejected(detail)))
                }
                None => scope
                    .emit_without_phases()
                    .map_err(Error::TargetNeutralLowering),
            },
            Error::TargetNeutralLowering,
        )
        // `source` is still owned here until lowerer validation/canonical replay
        // has completed. The callback cannot outlive it or substitute its owner.
    }
}
