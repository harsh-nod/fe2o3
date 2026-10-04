//! Borrow the real pre-ranked owner for source tests, without admitting checks.
use super::*;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_pre_ranked_for_test_v1<R>(
        self,
        observe: impl FnOnce(
            &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &[crate::compiler_descriptor::TypedDescriptorRootV1],
        ) -> R,
    ) -> Result<R, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        let stage = admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .materialize_target_neutral()?;
        Ok(observe(
            &stage.materialized,
            &stage.bindings.typed_descriptor_roots,
        ))
    }
}
