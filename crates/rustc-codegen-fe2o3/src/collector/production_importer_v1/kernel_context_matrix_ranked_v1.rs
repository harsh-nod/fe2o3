//! Private source-custody adapter, before any ranked/final roster exists.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionKernelContextLoweringInputV1, ProductionScopedMatrixSourceSessionV1,
    ProductionScopedMatrixUseRelationV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticDefinedCapabilityContractV1, SemanticTerminatorKindV1,
};

impl AuthenticatedProductionKernelContextsV1 {
    // Seed for the separate Global BF16 memory consumer. This is not a ranked
    // success arm: that consumer must resolve constructor/storage and reads.
    pub(crate) fn checked_ranked_bf16_source<'a>(
        &self,
        owner: &'a ProductionSemanticSsaOwnerV1,
        selected_root: SemanticFunctionIdV1,
        launch: &LaunchContract,
        entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
        max_work: usize,
    ) -> Result<
        Option<ProductionScopedMatrixSourceSessionV1<'a>>,
        crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1,
    > {
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as E;
        let Some(root) = self.checked_ranked_source_root(owner, selected_root, launch)? else {
            return Ok(None);
        };
        let view = owner
            .execution_view_for_root(selected_root)
            .ok_or(E::Incomplete(
                "ranked BF16 custody lost its checked source root",
            ))?;
        let has_bind = view.instances().iter().any(|instance| {
            matches!(
                owner.source_semantic().functions()[instance.function().index() as usize]
                    .defined_capability_contract(),
                Some(SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_))
            )
        });
        let has_load = view.body().blocks().iter().any(|block| {
            matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
            if matches!(owner.source_semantic().callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { .. }, ..
                })))
        });
        if !has_bind || !has_load {
            return Ok(None);
        }
        let mut input = ProductionKernelContextLoweringInputV1::new(
            root.selected_root,
            self.frontend_unit_identity,
            root.kernel_marker_identity,
            self.target_brand_identity,
            root.launch_brand_identity,
            root.issuance_identity,
        );
        if let Some(transfer) = root.entry_transfer {
            input = input.with_entry_transfer(transfer);
        }
        ProductionScopedMatrixSourceSessionV1::new(owner, &input, entry, max_work)
            .map(Some)
            .map_err(E::StructuralValidation)
    }

    pub(crate) fn checked_ranked_matrix<'a>(
        &self,
        owner: &'a ProductionSemanticSsaOwnerV1,
        selected_root: SemanticFunctionIdV1,
        launch: &LaunchContract,
        entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
        max_work: usize,
    ) -> Result<
        Option<ProductionScopedMatrixUseRelationV1<'a>>,
        crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1,
    > {
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as E;
        let Some(root) = self.checked_ranked_source_root(owner, selected_root, launch)? else {
            return Ok(None);
        };
        let view = owner
            .execution_view_for_root(selected_root)
            .ok_or(E::Incomplete(
                "ranked Matrix custody lost its checked source root",
            ))?;
        if !view.instances().iter().any(|instance| {
            matches!(
                owner.source_semantic().functions()[instance.function().index() as usize]
                    .defined_capability_contract(),
                Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_))
            )
        }) || !ProductionScopedMatrixUseRelationV1::has_potential_source_consumers(
            view.body(),
            owner.source_semantic().callables(),
        ) {
            return Ok(None);
        }
        let mut input = ProductionKernelContextLoweringInputV1::new(
            root.selected_root,
            self.frontend_unit_identity,
            root.kernel_marker_identity,
            self.target_brand_identity,
            root.launch_brand_identity,
            root.issuance_identity,
        );
        if let Some(transfer) = root.entry_transfer {
            input = input.with_entry_transfer(transfer);
        }
        ProductionScopedMatrixUseRelationV1::checked_source_uses(owner, &input, entry, max_work)
            .map(Some)
            .map_err(E::StructuralValidation)
    }
}
