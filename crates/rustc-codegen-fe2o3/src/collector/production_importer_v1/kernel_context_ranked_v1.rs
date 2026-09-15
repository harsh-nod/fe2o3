//! The ranked entry receives the same private source custody as the lowerer.
use super::*;
use fe2o3_lower_mir_kernel::ProductionKernelContextEntrySsaRelationV1;
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

impl AuthenticatedProductionKernelContextsV1 {
    pub(crate) fn checked_ranked_entry<'a>(
        &'a self,
        owner: &'a ProductionSemanticSsaOwnerV1,
        selected_root: SemanticFunctionIdV1,
        launch: &LaunchContract,
        max_work: usize,
    ) -> Result<Option<(ProductionKernelContextEntrySsaRelationV1<'a>, SemanticKernelCapabilityProvenanceV1)>, crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1> {
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as E;
        let rejected = || E::Incomplete("ranked Context entry lost authenticated frontend custody");
        let Some(root) = self.checked_ranked_source_root(owner, selected_root, launch)? else {
            if !owner.source_semantic().transpose_owned_flows().is_empty() { return Err(rejected()); }
            return Ok(None);
        };
        let entry = root.entry_transfer.map(|transfer| -> Result<_, E> {
            let relation = transfer.checked_ssa_relation(owner, selected_root, max_work)
                .map_err(E::StructuralValidation)?;
            // Independent source custody, not provenance copied from a consumer.
            let provenance = capability_memory_provenance_v1(root, self)
                .map_err(|_| rejected())?;
            Ok((relation, provenance))
        }).transpose()?;
        transpose_owned_source_v1::typed::check(
            self, root, owner, entry.as_ref().map(|(relation, _)| relation), max_work,
        ).map_err(|error| E::SourceCustody(Box::new(
            ProductionSemanticImportErrorV1::TransposeOwnedSource(Box::new(error)),
        )))?;
        Ok(entry)
    }
}

#[path = "kernel_context_matrix_ranked_v1.rs"]
mod matrix;

impl AuthenticatedProductionKernelContextsV1 {
    fn checked_ranked_source_root(
        &self, owner: &ProductionSemanticSsaOwnerV1,
        selected_root: SemanticFunctionIdV1, launch: &LaunchContract,
    ) -> Result<Option<&AuthenticatedProductionKernelContextRootV1>, crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1> {
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as E;
        let rejected = || E::Incomplete("ranked Context entry lost authenticated frontend custody");
        if kernel_context_custody_identity_v1(self.frontend_unit_identity, self.target_brand_identity,
            &self.expected_roots, &self.roots) != self.custody_identity
            || !self.roots.iter().map(|root| root.selected_root).eq(self.expected_roots.iter().copied())
        { return Err(rejected()); }
        let Some(root) = self.roots.iter().find(|root| root.selected_root == selected_root) else {
            if self.expected_roots.contains(&selected_root) { return Err(rejected()); }
            return Ok(None);
        };
        let source = owner.source_semantic().functions().get(selected_root.index() as usize)
            .ok_or_else(rejected)?;
        if source.identity().as_bytes() != &root.root_function_identity
            || source.kernel_entry().map(|entry| *entry.kernel_binding_identity().as_bytes()) != Some(root.kernel_binding)
            || kernel_context_launch_brand_identity_v1(launch) != root.launch_brand_identity
        { return Err(rejected()); }
        Ok(Some(root))
    }
}
