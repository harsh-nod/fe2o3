// A cache entry is an exact input/output pair, not a replacement for a callee
// transfer on another caller state. Retained emission views join all evaluations.

fn source_reference_call_transfer_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Result<SourceReferenceFunctionOutcomeV29, ProductionSemanticKirErrorV1>>(
        ),
        argument_product_v1(2, std::mem::size_of::<usize>())?,
        std::mem::size_of::<bool>(),
    ])
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_call_entry(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        entry: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        let previous = *self
            .plan
            .entries
            .get(instance.index())
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        if let Some(previous) = previous {
            self.merge_cfg_state(instance, previous, entry, budget)?;
        } else {
            self.plan.entries[instance.index()] = Some(self.clone_state(entry, budget)?);
        }
        Ok(())
    }

    fn retain_call_block_entry(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        block: SemanticBlockIdV1,
        entry: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let key = (instance.index(), block.index());
        charge_execution_cfg_lookup_v29(self.block_sites.len(), budget)?;
        if let Some(&index) = self.block_sites.get(&key) {
            budget.charge_work(2)?;
            let previous = self
                .plan
                .blocks
                .get(index)
                .ok_or_else(source_reference_cfg_obligation_v29)?;
            if previous.instance != instance || previous.block != block {
                return Err(source_reference_cfg_obligation_v29());
            }
            self.merge_cfg_state(instance, previous.entry, entry, budget)?;
            return Ok(());
        }
        reserve_execution_cfg_map_entry_v29::<(usize, u32), usize>(self.block_sites.len(), budget)?;
        let index = self.plan.blocks.len();
        emission_push_v1(
            &mut self.plan.blocks,
            SourceReferenceBlockV29 {
                instance,
                block,
                entry,
            },
            budget,
        )?;
        if self.block_sites.insert(key, index).is_some() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn retain_call_summary(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        mut summary: SourceReferenceCallSummaryV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if let Some(previous) = self
            .summaries
            .get(instance.index())
            .ok_or_else(source_reference_cfg_obligation_v29)?
        {
            summary.reuse_count = argument_sum_v1(&[previous.reuse_count, 1])?;
            summary.evaluations = argument_sum_v1(&[previous.evaluations, 1])?;
            // The old cache's built-in vectors are dropped only after their
            // bounded traversal is prepaid. Its arena snapshots remain retained.
            budget.charge_work(argument_sum_v1(&[
                previous.arguments.as_ref().map_or(0, Vec::len),
                previous.before.frames.len(),
                previous
                    .after
                    .as_ref()
                    .map_or(0, |after| after.frames.len()),
                1,
            ])?)?;
        }
        self.summaries[instance.index()] = Some(summary);
        Ok(())
    }

    fn reuse_call_summary(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        arguments: Option<&[usize]>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceFunctionOutcomeV29, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(source_reference_call_transfer_headers_v29()?)?;
        budget.charge_work(2)?;
        // Reentering a live frame is not another iteration of a static call.
        if self
            .frames
            .get(instance.index())
            .is_none_or(Option::is_some)
        {
            return Err(source_reference_cfg_obligation_v29());
        }
        let summary = self
            .summaries
            .get(instance.index())
            .and_then(Option::as_ref)
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        let mut exact = true;
        match (summary.arguments.as_deref(), arguments) {
            (None, None) => {}
            (Some(previous), Some(actual)) if previous.len() == actual.len() => {
                for (&left, &right) in previous.iter().zip(actual) {
                    exact &= self.nodes_equal(left, right, 0, budget)?;
                }
            }
            _ => return Err(source_reference_cfg_obligation_v29()),
        }
        let current = self.cfg_capture(budget)?;
        if current.frames.len() != summary.before.frames.len() {
            return Err(source_reference_cfg_obligation_v29());
        }
        for (&(actual_instance, actual), &(expected, previous)) in
            current.frames.iter().zip(&summary.before.frames)
        {
            budget.charge_work(2)?;
            if actual_instance != expected {
                return Err(source_reference_cfg_obligation_v29());
            }
            exact &= self.states_equal(actual, previous, budget)?;
        }
        if !exact {
            // The existing source driver rechecks reads, lifetimes, writes and
            // nested calls against the current frames. No cached frame is installed.
            return self.evaluate_call_transfer(instance, arguments, budget);
        }
        match (summary.result, summary.after.is_some()) {
            (SourceReferenceFunctionOutcomeV29::Returned(_), true) => {
                let after = self.cfg_clone_summary_after(instance, budget)?;
                self.cfg_install(&after, budget)?;
            }
            (SourceReferenceFunctionOutcomeV29::NoNormalReturn, false) => {}
            _ => return Err(source_reference_cfg_obligation_v29()),
        }
        budget.charge_work(1)?;
        let summary = self.summaries[instance.index()]
            .as_mut()
            .ok_or_else(source_reference_cfg_obligation_v29)?;
        summary.reuse_count = argument_sum_v1(&[summary.reuse_count, 1])?;
        Ok(summary.result)
    }
}
