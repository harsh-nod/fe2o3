// Exact graph85 preimage loan checks; only method names change.
impl CapabilitySsaGraphV1<'_> {
    fn loan_live_before_region_reuse(
        &mut self,
        loan: CapabilityLoanV1,
        consumer: CapabilityDefinitionSiteV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let key = (loan, consumer);
        self.charge(lookup_work(self.reuse.loans.len()))?;
        if self.reuse.loans.contains_key(&key) {
            return Ok(());
        }
        self.loan_live_uncached_before_region_reuse(loan, consumer)?;
        self.charge(insertion_work::<
            (CapabilityLoanV1, CapabilityDefinitionSiteV1),
            (),
        >(self.reuse.loans.len()))?;
        self.reuse.loans.insert(key, ());
        Ok(())
    }

    fn loan_live_uncached_before_region_reuse(
        &mut self,
        loan: CapabilityLoanV1,
        consumer: CapabilityDefinitionSiteV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.use_value(loan.borrow.block, loan.owner_local)? != loan.owner_value {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !self.reaches(loan.borrow.block, consumer.block)? {
            return Err(reject("capability loan does not reach its consumer"));
        }
        // SSA establishes reaching definitions; the exact path intersection
        // retains every original storage invalidation and cycle check.
        let region = self.path_region(loan.borrow.block, consumer.block)?;
        let acyclic = self.region_is_acyclic_cold_reference(&region)?;
        for block in self.ssa.reverse_postorder() {
            self.charge(1)?;
            if !region[block.get() as usize] {
                continue;
            }
            let source = &self.body.blocks()[block.get() as usize];
            if !acyclic {
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
                    if self.reaches(successor, block.get())? {
                        return Err(reject(
                            "capability loan crosses a cycle without a proven storage generation",
                        ));
                    }
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
                if invalidates(statement.kind(), loan.owner_local) {
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
                        .any(|operand| moves(operand, loan.owner_local))
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
