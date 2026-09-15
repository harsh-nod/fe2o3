// Included in session.rs; shares its immutable owner, Scope and one Graph.
use super::typed_uses::ProductionTransposeOwnedSourceUsesV1;

impl<'a> ProductionScopedMatrixSourceSessionV1<'a> {
    /// Checks the complete transpose source-use batch with the existing graph.
    /// This publishes no Matrix/BF16, numerical, allocation or barrier receipt.
    /// Production additionally requires the live frontend source/footer seal.
    pub fn check_transpose_owned_uses<'u>(
        owner: &'a ProductionSemanticSsaOwnerV1,
        input: &ProductionKernelContextLoweringInputV1,
        entry: Option<&ProductionKernelContextEntrySsaRelationV1<'a>>,
        uses: impl IntoIterator<Item = ProductionTransposeOwnedSourceUsesV1<'a, 'u>>,
        max_work: usize,
    ) -> Result<()>
    where
        'a: 'u,
    {
        let mut session = Self::new_scoped(owner, input, entry, max_work, Scope::TransposeOnly)?;
        let mut expected = BTreeSet::new();
        for (block, body) in session.view.body().blocks().iter().enumerate() {
            session.graph.charge(1)?;
            if matches!(body.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                if matches!(owner.source_semantic().callables().get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic { operation:
                        SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, .. })
                    if matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t)
                        if matches!(t.operation(), fe2o3_mir_model::semantic_mir_v1::SemanticGfx950TransposeOperationV1::Publish { .. }))))
            {
                session.graph.charge(8)?;
                expected.insert(block as u32);
            }
        }
        let mut consumed = BTreeSet::new();
        session.graph.charge(std::mem::size_of::<(
            Option<super::old_epoch::Index<'a>>,
            Option<super::typed_inventory::MatrixSites<'a>>,
        )>().div_ceil(std::mem::size_of::<usize>()))?;
        let mut old_uses = None;
        let mut matrix_sites = None;
        for uses in uses {
            let publish = uses.workgroup.site().block().index();
            session
                .graph
                .charge(2 + 2 * (usize::BITS - expected.len().leading_zeros()) as usize)?;
            if !expected.contains(&publish) || consumed.contains(&publish) {
                return Err(reject(
                    "transpose typed Publish roster is changed or consumed twice",
                ));
            }
            super::typed_flow::check(&mut session, &uses, &mut old_uses, &mut matrix_sites)?;
            session.graph.charge(8)?;
            consumed.insert(publish);
        }
        session.graph.charge(expected.len())?;
        if consumed != expected {
            return Err(reject(
                "transpose typed source batch omitted a Publish occurrence",
            ));
        }
        Ok(())
    }
}
