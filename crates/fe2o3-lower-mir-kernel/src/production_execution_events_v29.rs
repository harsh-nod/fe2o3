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
        Self::new_with_references(occurrences, function, nominal, &[], None, budget)
    }

    fn new_with_references(
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        function: &SemanticFunctionDeclV1,
        nominal: &[usize],
        reference_locals: &[bool],
        references: Option<(
            &SourceReferenceEmissionV29<'_, '_>,
            ProductionCallInstanceIdV1,
        )>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_with_identity(
            occurrences,
            function,
            nominal,
            reference_locals,
            references,
            None,
            None,
            budget,
        )
    }

    fn new_with_identity(
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        function: &SemanticFunctionDeclV1,
        nominal: &[usize],
        reference_locals: &[bool],
        references: Option<(
            &SourceReferenceEmissionV29<'_, '_>,
            ProductionCallInstanceIdV1,
        )>,
        identities: Option<(&ExecutionIdentityPlanV1<'_, '_>, ProductionCallInstanceIdV1)>,
        control: Option<&ExecutionSourceControlV29<'_>>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let events = occurrences.events();
        let mut required = emission_vec_v1(events.len(), budget)?;
        let mut blocks = emission_vec_v1(function.blocks().len(), budget)?;
        let mut next = 0;
        for block in 0..function.blocks().len() {
            budget.charge_work(1)?;
            let active = match control {
                Some(control) => {
                    control.block_reachable(SemanticBlockIdV1::from_index(block as u32), budget)?
                }
                None => true,
            };
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
                if !active {
                    next += 1;
                    continue;
                }
                let local = event.event().variable().get() as usize;
                let root = *nominal
                    .get(local)
                    .ok_or_else(execution_availability_error_v29)?
                    != 0
                    || reference_locals.get(local).copied().unwrap_or(false);
                let mut retained_index = false;
                let needed = match event.role() {
                    ExecutionEventV29::BaseUse => {
                        base = Some((event.site(), event.operand(), root));
                        root || if let Some((references, instance)) = references {
                            !references.plan.descriptors.is_empty()
                                && references
                                    .plan
                                    .descriptor_holder_event(instance, next, budget)?
                        } else {
                            false
                        }
                    }
                    ExecutionEventV29::ProjectionIndexUse(projection) => {
                        let selected = if let Some((references, instance)) = references {
                            let place = source_reference_selector_place_v29(
                                function,
                                event.site(),
                                event.operand(),
                            )
                            .ok_or_else(execution_availability_error_v29)?;
                            let selector = references.plan.selector_at(
                                instance,
                                event.site(),
                                place,
                                projection as usize,
                                budget,
                            )?;
                            if let Some((_, row)) = selector
                                && let SourceReferenceSelectorValueV29::Retained { event: original } =
                                    row.value
                            {
                                row.check(references.plan.instances, budget)?;
                                if original != next || row.local.index() as usize != local {
                                    return Err(execution_availability_error_v29());
                                }
                                retained_index = true;
                            }
                            selector.is_some()
                                || (!references.plan.descriptors.is_empty()
                                    && references
                                        .plan
                                        .descriptor_at(
                                            instance,
                                            event.site(),
                                            place,
                                            projection as usize,
                                            budget,
                                        )?
                                        .is_some())
                        } else {
                            false
                        };
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
                                let local = destination.place().local().index() as usize;
                                nominal[local] != 0
                                    || reference_locals.get(local).copied().unwrap_or(false)
                            } else {
                                let Some((site, operand, nominal)) = base else {
                                    return Err(execution_availability_error_v29());
                                };
                                if site != event.site() || operand != event.operand() {
                                    return Err(execution_availability_error_v29());
                                }
                                nominal
                            };
                        nominal_root || root || selected
                    }
                    _ => root,
                };
                if event.is_reachable() && needed {
                    if (!event.is_promoted() || event.resolved().is_none()) && !retained_index {
                        let (identities, instance) =
                            identities.ok_or_else(execution_availability_error_v29)?;
                        let original = identities
                            .index
                            .instances
                            .instance(instance)
                            .ok_or_else(execution_identity_error_v1)?;
                        let original_occurrences = identities
                            .index
                            .instances
                            .occurrences(instance)
                            .ok_or_else(execution_identity_error_v1)?;
                        if !std::ptr::eq(original.declaration(), function)
                            || !std::ptr::eq(original_occurrences.events(), events)
                        {
                            return Err(execution_identity_error_v1());
                        }
                        identities.index.retained_use(
                            instance,
                            next,
                            SemanticLocalIdV1::from_index(local as u32),
                            budget,
                        )?;
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
            if let Some(control) = control
                && !control.block_reachable(SemanticBlockIdV1::from_index(block.get()), budget)?
            {
                continue;
            }
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
            if reference_locals
                .get(assignment.destination().local().index() as usize)
                .copied()
                .unwrap_or(false)
                || reference_locals
                    .get(source.local().index() as usize)
                    .copied()
                    .unwrap_or(false)
            {
                let (references, instance) =
                    references.ok_or_else(execution_availability_error_v29)?;
                let site = SourceReferenceSiteV29 {
                    instance,
                    block: SemanticBlockIdV1::from_index(block.get()),
                    statement: Some(statement as usize),
                };
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Borrow { .. } => {
                        if references.loan_at(site, budget)?.is_none() {
                            return Err(execution_availability_error_v29());
                        }
                    }
                    SemanticRvalueKindV1::AddressOf { place, mutability } => {
                        references.check_raw_formation_source_v29(
                            site,
                            place,
                            assignment.value().result_type(),
                            *mutability,
                            budget,
                        )?;
                    }
                    _ => return Err(execution_availability_error_v29()),
                }
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
        for index in 0..self.visited.len() {
            budget.charge_work(1)?;
            let active = self
                .source_block_reachable_v29(SemanticBlockIdV1::from_index(index as u32), budget)?;
            if self.visited[index] != active {
                return Err(execution_availability_error_v29());
            }
        }
        self.events.finished = true;
        Ok(())
    }
}
