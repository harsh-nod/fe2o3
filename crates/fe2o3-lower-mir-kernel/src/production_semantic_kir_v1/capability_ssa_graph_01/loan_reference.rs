// Frozen pre-reuse loan algorithm: semantic oracle with ample test budgets.
impl CapabilitySsaGraphV1<'_> {
    fn loan_live_reference(
        &mut self,
        loan: CapabilityLoanV1,
        consumer: CapabilityDefinitionSiteV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.use_value_uncached(loan.borrow.block, loan.owner_local)? != loan.owner_value {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !self.reaches_uncached(loan.borrow.block, consumer.block)? {
            return Err(reject("capability loan does not reach its consumer"));
        }
        // SSA establishes reaching definitions; this independent source scan
        // preserves deinitialization and aliasing invalidations too.
        for block in self.ssa.reverse_postorder() {
            if !self.reaches_uncached(loan.borrow.block, block.get())?
                || !self.reaches_uncached(block.get(), consumer.block)?
            {
                continue;
            }
            let source = &self.body.blocks()[block.get() as usize];
            let mut successors = Vec::new();
            source
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    self.charge(1)?;
                    successors.push(edge.target().index());
                    Ok(())
                })?;
            for successor in successors {
                if self.reaches_uncached(successor, block.get())? {
                    return Err(reject(
                        "capability loan crosses a cycle without a proven storage generation",
                    ));
                }
            }
            self.charge(source.statements().len())?;
            for (index, statement) in source.statements().iter().enumerate() {
                if block.get() == loan.borrow.block
                    && loan
                        .borrow
                        .statement
                        .is_some_and(|start| index <= start as usize)
                {
                    continue;
                }
                if block.get() == consumer.block
                    && consumer.statement.is_some_and(|end| index >= end as usize)
                {
                    continue;
                }
                if reference_invalidates(statement.kind(), loan.owner_local) {
                    return Err(reject(
                        "capability loan crosses a move, overwrite, deinitialization or storage death",
                    ));
                }
            }
            if block.get() != consumer.block || consumer.statement.is_none() {
                if matches!(source.terminator().kind(), SemanticTerminatorKindV1::Drop { place, .. } if place.local().index() == loan.owner_local)
                {
                    return Err(reject("capability loan crosses an owner drop"));
                }
                if let SemanticTerminatorKindV1::Call(call) = source.terminator().kind() {
                    self.charge(call.arguments().len())?;
                    if call
                        .arguments()
                        .iter()
                        .any(|operand| reference_moves(operand, loan.owner_local))
                        || call.destination().is_some_and(|destination| {
                            destination.place().local().index() == loan.owner_local
                        })
                    {
                        return Err(reject(
                            "capability loan crosses an owner call move or overwrite",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
