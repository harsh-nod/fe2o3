// Indices preserve the captured adapter's block-event order, including gaps
// for ordinary events. This is a census, not another source or CFG analysis.
struct ExecutionEventsV29 {
    required: Vec<usize>,
    blocks: Vec<std::ops::Range<usize>>,
    pending: std::ops::Range<usize>,
    finished: bool,
}

fn execution_event_block_v29(site: ExecutionSiteV29) -> SsaBlockIdV1 {
    match site {
        ExecutionSiteV29::Statement { block, .. } | ExecutionSiteV29::Terminator { block } => block,
    }
}

impl ExecutionEventsV29 {
    fn new(
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        function: &SemanticFunctionDeclV1,
        nominal: &[usize],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let events = occurrences.events();
        let mut required = borrowed_aggregate_vec_v1(events.len(), budget)?;
        let mut blocks = borrowed_aggregate_vec_v1(function.blocks().len(), budget)?;
        let mut next = 0;
        for block in 0..function.blocks().len() {
            budget.charge_work(1)?;
            let first = required.len();
            let mut ordinal = 0;
            let mut base = None;
            while let Some(event) = events.get(next) {
                budget.charge_work(8)?;
                if execution_event_block_v29(event.site()).get() as usize != block {
                    break;
                }
                if event.ordinal() != ordinal {
                    return Err(execution_availability_error_v29());
                }
                ordinal += 1;
                let local = event.event().variable().get() as usize;
                let root = *nominal
                    .get(local)
                    .ok_or_else(execution_availability_error_v29)?
                    != 0;
                let needed = match event.role() {
                    ExecutionEventV29::BaseUse => {
                        base = Some((event.site(), event.operand(), root));
                        root
                    }
                    ExecutionEventV29::ProjectionIndexUse(_) => {
                        let nominal_root =
                            if event.operand() == ExecutionOperandV29::CallDestinationAddress {
                                // Direct indexed destinations have no BaseUse row.
                                budget.charge_work(4)?;
                                let ExecutionSiteV29::Terminator { block } = event.site() else {
                                    return Err(execution_availability_error_v29());
                                };
                                let SemanticTerminatorKindV1::Call(call) =
                                    function.blocks()[block.get() as usize].terminator().kind()
                                else {
                                    return Err(execution_availability_error_v29());
                                };
                                let destination = call
                                    .destination()
                                    .ok_or_else(execution_availability_error_v29)?;
                                nominal[destination.place().local().index() as usize] != 0
                            } else {
                                let Some((site, operand, nominal)) = base else {
                                    return Err(execution_availability_error_v29());
                                };
                                if site != event.site() || operand != event.operand() {
                                    return Err(execution_availability_error_v29());
                                }
                                nominal
                            };
                        nominal_root || root
                    }
                    _ => root,
                };
                if event.is_reachable() && needed {
                    if !event.is_promoted() || event.resolved().is_none() {
                        return Err(execution_availability_error_v29());
                    }
                    required.push(next);
                }
                next += 1;
            }
            blocks.push(first..required.len());
        }
        if next != events.len() {
            return Err(execution_availability_error_v29());
        }
        // An elided borrow has no captured source read. Until there is an
        // explicit consumption rule, neither end may conceal a nominal root.
        for site in occurrences.elisions() {
            budget.charge_work(6)?;
            let ExecutionSiteV29::Statement { block, statement } = *site else {
                return Err(execution_availability_error_v29());
            };
            let SemanticStatementKindV1::Assign(assignment) =
                function.blocks()[block.get() as usize].statements()[statement as usize].kind()
            else {
                return Err(execution_availability_error_v29());
            };
            let source = match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. } => place,
                _ => return Err(execution_availability_error_v29()),
            };
            if nominal[source.local().index() as usize] != 0
                || nominal[assignment.destination().local().index() as usize] != 0
            {
                return Err(execution_availability_error_v29());
            }
        }
        Ok(Self {
            required,
            blocks,
            pending: 0..0,
            finished: false,
        })
    }

    fn complete(
        &self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if !self.pending.is_empty() || self.finished {
            return Err(execution_availability_error_v29());
        }
        Ok(())
    }
}

impl ExecutionAvailabilityV29<'_> {
    fn claim_events(
        &mut self,
        indices: &[usize],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        #[cfg(test)]
        if self
            .skipped_event
            .is_some_and(|index| indices.contains(&index))
        {
            return Ok(());
        }
        budget.charge_work(argument_product_v1(indices.len(), 3)?)?;
        let mut next = self.events.pending.start;
        for &index in indices {
            if self.events.finished || self.claimed[index] {
                return Err(execution_availability_error_v29());
            }
            if next < self.events.pending.end {
                match index.cmp(&self.events.required[next]) {
                    std::cmp::Ordering::Equal => next += 1,
                    std::cmp::Ordering::Greater => return Err(execution_availability_error_v29()),
                    std::cmp::Ordering::Less => {}
                }
            }
        }
        // A whole move commits Use and Kill together, after both are checked.
        for &index in indices {
            self.claimed[index] = true;
        }
        self.events.pending.start = next;
        Ok(())
    }

    fn finish_block(
        &mut self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        self.events.complete(budget)?;
        let block = self.block.ok_or_else(execution_availability_error_v29)?;
        budget.charge_work(self.cfg.edges.len().checked_ilog2().unwrap_or(0) as usize + 2)?;
        let first = self
            .cfg
            .edges
            .partition_point(|edge| edge.id.source() < block);
        for edge in &self.cfg.edges[first..] {
            budget.charge_work(2)?;
            if edge.id.source() != block {
                break;
            }
            if !edge.claimed {
                return Err(execution_availability_error_v29());
            }
        }
        self.block = None;
        Ok(())
    }

    fn finish(
        &mut self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        self.events.complete(budget)?;
        if self.block.is_some() {
            return Err(execution_availability_error_v29());
        }
        for block in self.ssa.plan().reverse_postorder() {
            budget.charge_work(1)?;
            if !self.visited[block.get() as usize] {
                return Err(execution_availability_error_v29());
            }
        }
        self.events.finished = true;
        Ok(())
    }
}
