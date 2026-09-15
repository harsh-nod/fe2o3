#[cfg(test)]
mod kernel_context_entry_relation_tests {
    include!("kernel_context_entry_relation_01/tests.rs");
}

/// The checked SSA relation for one source-authenticated, erased Context entry.
/// This borrows the replayed owner; it does not authenticate an inert input.
pub struct ProductionKernelContextEntrySsaRelationV1<'a> {
    input: ProductionKernelContextEntryTransferV1,
    body: &'a SemanticFunctionDeclV1,
    plan: KernelContextEntryPlanV1,
    issuer_local: SemanticLocalIdV1,
}

impl ProductionKernelContextEntryTransferV1 {
    /// Reuses the lowerer's exact source/call/SSA/lifetime checks. The caller
    /// must obtain this inert transfer from authenticated frontend custody.
    pub fn checked_ssa_relation<'a>(
        self,
        owner: &'a ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        max_work: usize,
    ) -> Result<ProductionKernelContextEntrySsaRelationV1<'a>, ProductionSemanticKirErrorV1> {
        owner.verify_replay().map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let reject = || unsupported(root.index(), None, None, "Context entry SSA relation changed or exceeded its work bound");
        let view = owner.execution_view_for_root(root).ok_or_else(reject)?;
        let types = owner.source_semantic().types();
        let graph_work = max_work.checked_sub(types.len())
            .and_then(|remaining| remaining.checked_sub(view.local_origins().len()))
            .ok_or_else(reject)?;
        let context = types.iter().position(|ty| ty.identity() == self.context)
            .and_then(|index| u32::try_from(index).ok())
            .map(SemanticTypeIdV1::from_index).ok_or_else(reject)?;
        let plan = KernelContextEntryPlanV1::new(owner, root, context, self, graph_work)?;
        let mut issuer_local = None;
        for (local, origin) in view.local_origins().iter().enumerate() {
            if origin.function() == root && origin.instance().index() == 0
                && origin.local() == self.issuer_local
                && issuer_local.replace(SemanticLocalIdV1::from_index(local as u32)).is_some()
            {
                return Err(reject());
            }
        }
        Ok(ProductionKernelContextEntrySsaRelationV1 {
            input: self,
            body: view.body(), plan, issuer_local: issuer_local.ok_or_else(reject)?,
        })
    }
}

impl ProductionKernelContextEntrySsaRelationV1<'_> {
    /// Tests identity with the replayed function borrowed by this relation.
    pub fn matches_body(&self, body: &SemanticFunctionDeclV1) -> bool {
        std::ptr::eq(self.body, body)
    }
    /// Returns the parameter-transfer block in the replayed function.
    pub const fn block(&self) -> SemanticBlockIdV1 { self.plan.block }
    /// Returns the parameter-transfer statement within that block.
    pub const fn statement(&self) -> u32 { self.plan.statement }
    /// Returns the replayed local receiving the Context parameter.
    pub const fn destination(&self) -> SemanticLocalIdV1 { self.plan.destination }
    /// Returns the original issuer's local in the replayed function.
    pub const fn issuer_local(&self) -> SemanticLocalIdV1 { self.issuer_local }
    /// Returns the exact canonical Context type.
    pub const fn context(&self) -> SemanticTypeIdV1 { self.plan.context }
    /// Returns the SSA value produced by the checked Context issuer.
    pub const fn issuer_value(&self) -> SsaValueV1 { self.plan.issuer }
    /// Returns the SSA value defined by the checked parameter transfer.
    pub const fn parameter_value(&self) -> SsaValueV1 { self.plan.parameter }
}
