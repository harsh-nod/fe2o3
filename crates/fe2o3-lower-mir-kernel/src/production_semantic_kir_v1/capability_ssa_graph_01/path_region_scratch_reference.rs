// Exact Graph89 path, geometry-cache and loan checkers; only linked method names change.
impl CapabilitySsaGraphV1<'_> {
    fn path_region_before_scratch(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<Vec<bool>, ProductionSemanticKirErrorV1> {
        let count = self.body.blocks().len();
        if from as usize >= count || to as usize >= count {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        // Charge the bounded tables and traversal scratch before allocation.
        for _ in 0..4 {
            self.charge(count)?;
        }
        let mut region = vec![false; count];
        if !self.ssa.is_reachable(SsaBlockIdV1::new(from)) {
            return Ok(region);
        }
        let mut seen = vec![false; count];
        let mut predecessors = vec![Vec::new(); count];
        let mut pending = vec![from];
        seen[from as usize] = true;
        while let Some(block) = pending.pop() {
            self.charge(1)?;
            self.body.blocks()[block as usize]
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|edge| {
                    self.charge(1)?;
                    let target = edge.target().index();
                    if target as usize >= count {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    if self.ssa.is_reachable(SsaBlockIdV1::new(target)) {
                        predecessors[target as usize].push(block);
                        if !seen[target as usize] {
                            seen[target as usize] = true;
                            pending.push(target);
                        }
                    }
                    Ok(())
                })?;
        }
        if !seen[to as usize] {
            return Ok(region);
        }
        region[to as usize] = true;
        pending.push(to);
        while let Some(block) = pending.pop() {
            self.charge(1)?;
            for &predecessor in &predecessors[block as usize] {
                self.charge(1)?;
                if !region[predecessor as usize] {
                    region[predecessor as usize] = true;
                    pending.push(predecessor);
                }
            }
        }
        Ok(region)
    }

    fn loan_region_before_path_scratch(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<Arc<CapabilityLoanRegionV1>, ProductionSemanticKirErrorV1> {
        // Keys are meaningful only within this graph's immutable body/SSA pair.
        // The extra unit covers the Arc clone, before any retained row is used.
        self.charge(lookup_work(self.reuse.loan_regions.len()).saturating_add(1))?;
        if let Some(region) = self.reuse.loan_regions.get(&(from, to)) {
            return Ok(Arc::clone(region));
        }
        let blocks = self.path_region_before_scratch(from, to)?;
        let acyclic = self.region_is_acyclic_cold_reference(&blocks)?;
        // path_region already charged its vector allocation; moving it retains
        // that exact buffer. Charge the new Arc allocation and B-tree row.
        let allocation_words = std::mem::size_of::<CapabilityLoanRegionV1>()
            .div_ceil(std::mem::size_of::<usize>())
            .saturating_add(2);
        self.charge(
            insertion_work::<(u32, u32), Arc<CapabilityLoanRegionV1>>(
                self.reuse.loan_regions.len(),
            )
            .saturating_add(allocation_words),
        )?;
        let region = Arc::new(CapabilityLoanRegionV1 { blocks, acyclic });
        self.reuse
            .loan_regions
            .insert((from, to), Arc::clone(&region));
        Ok(region)
    }

    fn loan_live_before_path_scratch(
        &mut self,
        loan: CapabilityLoanV1,
        consumer: CapabilityDefinitionSiteV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let key = (loan, consumer);
        self.charge(lookup_work(self.reuse.loans.len()))?;
        if self.reuse.loans.contains_key(&key) {
            return Ok(());
        }
        self.loan_live_uncached_before_path_scratch(loan, consumer)?;
        self.charge(insertion_work::<
            (CapabilityLoanV1, CapabilityDefinitionSiteV1),
            (),
        >(self.reuse.loans.len()))?;
        self.reuse.loans.insert(key, ());
        Ok(())
    }

    fn loan_live_uncached_before_path_scratch(
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
        let geometry = self.loan_region_before_path_scratch(loan.borrow.block, consumer.block)?;
        let region = &geometry.blocks;
        let acyclic = geometry.acyclic;
        let invalidations = self.owner_statement_index(loan.owner_local)?;
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
            let first = if block.get() == loan.borrow.block {
                loan.borrow.statement.map_or(0, |start| (start as usize).saturating_add(1))
            } else {
                0
            };
            let end = if block.get() == consumer.block {
                consumer.statement.map_or(source.statements().len(), |end| end as usize)
            } else {
                source.statements().len()
            };
            if self.indexed_statement_invalidated(&invalidations, block.get(), first, end)? {
                return Err(reject(
                    "capability loan crosses a move, overwrite, deinitialization or storage death",
                ));
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
