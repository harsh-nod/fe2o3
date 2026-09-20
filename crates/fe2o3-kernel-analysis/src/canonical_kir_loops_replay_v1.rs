impl CanonicalKirLoopsV1<'_, '_> {
    fn replay_inner(
        &self,
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let inventory = self.inventory;
        input_limits(inventory, limits, budget)?;
        budget.charge_work(3)?;
        count("loops", self.loops.len(), limits.loops)?;
        count("retained rows", self.rows()?, limits.rows)?;
        let graph = Incidence::build(inventory, budget)?;
        let mut scratch = Scratch::new(inventory.blocks().len(), inventory.edges().len(), budget)?;
        for function in inventory.functions() {
            budget.charge_work(1)?;
            if function.function.body.is_some() {
                replay_reachability(
                    inventory,
                    &graph,
                    function.blocks.clone(),
                    None,
                    &mut scratch.reachable,
                    &mut scratch.stack,
                    budget,
                )?;
            }
        }
        let mut loop_cursor = 0usize;
        let mut member_cursor = 0usize;
        let mut edge_cursor = 0usize;
        let mut recurrence_cursor = 0usize;
        for header in 0..inventory.blocks().len() {
            budget.charge_work(2)?;
            if !scratch.reachable[header] {
                continue;
            }
            let coordinate = inventory.blocks()[header].coordinate;
            let function = &inventory.functions()[coordinate.function.0 as usize];
            replay_reachability(
                inventory,
                &graph,
                function.blocks.clone(),
                Some(header),
                &mut scratch.avoiding,
                &mut scratch.stack,
                budget,
            )?;
            budget.charge_work(scratch.backedges.len())?;
            scratch.backedges.fill(false);
            let mut latch_count = 0;
            let mut last_latch = None;
            for &edge in graph.predecessors(header) {
                budget.charge_work(4)?;
                let source = graph.sources[edge];
                if scratch.reachable[source] && !scratch.avoiding[source] {
                    scratch.backedges[edge] = true;
                    latch_count += 1;
                    last_latch = Some(edge);
                }
            }
            if latch_count == 0 {
                continue;
            }
            count(
                "loops",
                loop_cursor.checked_add(1).ok_or(Resource::Arithmetic)?,
                limits.loops,
            )?;
            let fact = self.loops.get(loop_cursor).ok_or(Error::ReplayMismatch)?;
            if fact.header != coordinate
                || fact.members.start != member_cursor
                || fact.latches.start != edge_cursor
                || fact.recurrences.start != recurrence_cursor
            {
                return Err(Error::ReplayMismatch);
            }
            loop_cursor += 1;

            // Independent reverse closure from actual dominated incoming edges.
            // The producer's membership list is not used as a worklist or filter.
            budget.charge_work(scratch.members.len())?;
            scratch.members.fill(false);
            scratch.stack.clear();
            scratch.members[header] = true;
            for &edge in graph.predecessors(header) {
                budget.charge_work(3)?;
                let source = graph.sources[edge];
                if scratch.backedges[edge] && !scratch.members[source] {
                    scratch.members[source] = true;
                    scratch.stack.push(source);
                }
            }
            loop {
                budget.charge_work(2)?;
                let Some(current) = scratch.stack.pop() else {
                    break;
                };
                if scratch.avoiding[current] {
                    return Err(Error::ReplayMismatch);
                }
                for &edge in graph.predecessors(current) {
                    budget.charge_work(4)?;
                    let before = graph.sources[edge];
                    if scratch.reachable[before] && !scratch.members[before] {
                        scratch.members[before] = true;
                        scratch.stack.push(before);
                    }
                }
            }
            for block in function.blocks.clone() {
                budget.charge_work(2)?;
                if scratch.members[block] {
                    if self.members.get(member_cursor)
                        != Some(&inventory.blocks()[block].coordinate)
                    {
                        return Err(Error::ReplayMismatch);
                    }
                    member_cursor += 1;
                }
            }
            if fact.members.end != member_cursor {
                return Err(Error::ReplayMismatch);
            }
            for &edge in graph.predecessors(header) {
                budget.charge_work(2)?;
                if scratch.backedges[edge] {
                    replay_edge(
                        &self.edges,
                        &mut edge_cursor,
                        inventory.edges()[edge].coordinate,
                    )?;
                }
            }
            if fact.latches.end != edge_cursor || fact.external.start != edge_cursor {
                return Err(Error::ReplayMismatch);
            }
            let mut external_count = 0;
            let mut last_external = None;
            for &edge in graph.predecessors(header) {
                budget.charge_work(3)?;
                if !scratch.members[graph.sources[edge]] {
                    replay_edge(
                        &self.edges,
                        &mut edge_cursor,
                        inventory.edges()[edge].coordinate,
                    )?;
                    external_count += 1;
                    last_external = Some(edge);
                }
            }
            if fact.external.end != edge_cursor || fact.exits.start != edge_cursor {
                return Err(Error::ReplayMismatch);
            }
            let preheader = if external_count == 1 {
                budget.charge_work(3)?;
                let edge = last_external.ok_or(Error::ReplayMismatch)?;
                let source = graph.sources[edge];
                if scratch.reachable[source]
                    && matches!(
                        inventory.blocks()[source].terminator,
                        Terminator::Branch { .. }
                    )
                {
                    Some(inventory.edges()[edge].coordinate)
                } else {
                    None
                }
            } else {
                None
            };
            let mut single_entry = coordinate.block != 0;
            let mut dedicated = true;
            budget.charge_work(scratch.headers.len())?;
            scratch.headers.fill(false);
            for edge in function.edges.clone() {
                budget.charge_work(4)?;
                let source = graph.sources[edge];
                let target = graph.targets[edge];
                if !scratch.members[source] {
                    scratch.headers[target] = true;
                    if scratch.members[target] && target != header {
                        single_entry = false;
                    }
                }
            }
            for edge in function.edges.clone() {
                budget.charge_work(4)?;
                let source = graph.sources[edge];
                let target = graph.targets[edge];
                if scratch.members[source] && !scratch.members[target] {
                    replay_edge(
                        &self.edges,
                        &mut edge_cursor,
                        inventory.edges()[edge].coordinate,
                    )?;
                    if scratch.headers[target] {
                        dedicated = false;
                    }
                }
            }
            if fact.exits.end != edge_cursor
                || fact.preheader != preheader
                || fact.single_entry != single_entry
                || fact.dedicated_exits != dedicated
            {
                return Err(Error::ReplayMismatch);
            }
            if single_entry
                && preheader.is_some()
                && latch_count == 1
                && graph.predecessors(header).len() == 2
            {
                let backedge = last_latch.ok_or(Error::ReplayMismatch)?;
                let initial = last_external.ok_or(Error::ReplayMismatch)?;
                budget.charge_work(1)?;
                if matches!(
                    inventory.blocks()[graph.sources[backedge]].terminator,
                    Terminator::Branch { .. }
                ) {
                    for argument in 0..inventory.blocks()[header].block.parameters.len() {
                        if let Some(expected) = replay_recurrence(
                            inventory,
                            header,
                            argument,
                            initial,
                            backedge,
                            &scratch.members,
                            budget,
                        )? {
                            budget.charge_work(2)?;
                            if self.recurrences.get(recurrence_cursor) != Some(&expected) {
                                return Err(Error::ReplayMismatch);
                            }
                            recurrence_cursor += 1;
                        }
                    }
                }
            }
            if fact.recurrences.end != recurrence_cursor {
                return Err(Error::ReplayMismatch);
            }
        }
        budget.charge_work(4)?;
        if loop_cursor != self.loops.len()
            || member_cursor != self.members.len()
            || edge_cursor != self.edges.len()
            || recurrence_cursor != self.recurrences.len()
        {
            return Err(Error::ReplayMismatch);
        }
        Ok(())
    }
}

fn replay_edge(rows: &[Edge], cursor: &mut usize, actual: Edge) -> Result<()> {
    if rows.get(*cursor) != Some(&actual) {
        return Err(Error::ReplayMismatch);
    }
    *cursor += 1;
    Ok(())
}

fn replay_reachability(
    inventory: &Inventory<'_>,
    graph: &Incidence,
    blocks: Range<usize>,
    removed: Option<usize>,
    visited: &mut [bool],
    stack: &mut Vec<usize>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(blocks.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
    visited[blocks.clone()].fill(false);
    stack.clear();
    if blocks.is_empty() {
        return Err(Error::InconsistentInventory);
    }
    if removed == Some(blocks.start) {
        return Ok(());
    }
    visited[blocks.start] = true;
    stack.push(blocks.start);
    loop {
        budget.charge_work(1)?;
        let Some(block) = stack.pop() else {
            break;
        };
        for edge in inventory.blocks()[block].edges.clone() {
            budget.charge_work(4)?;
            let target = graph.targets[edge];
            if !blocks.contains(&target) {
                return Err(Error::InconsistentInventory);
            }
            if Some(target) != removed && !visited[target] {
                visited[target] = true;
                stack.push(target);
            }
        }
    }
    Ok(())
}

// This checker does not call the producer's recurrence matcher. It starts with
// actual edge bindings and independently re-reads result/operand/literal types.
fn replay_recurrence(
    inventory: &Inventory<'_>,
    header: usize,
    argument: usize,
    initial_edge: usize,
    backedge: usize,
    members: &[bool],
    budget: &mut Budget<'_>,
) -> Result<Option<Recurrence>> {
    budget.charge_work(12)?;
    let block = &inventory.blocks()[header];
    let param = &block.block.parameters[argument];
    let initial = &inventory.edges()[initial_edge];
    let back = &inventory.edges()[backedge];
    let initial_binding = &inventory.edge_arguments()[initial.bindings.start + argument];
    let back_binding = &inventory.edge_arguments()[back.bindings.start + argument];
    let update = &inventory.definitions()[back_binding.incoming_definition];
    let initial_definition = &inventory.definitions()[initial_binding.incoming_definition];
    let parameter_definition = Definition::BlockArgument {
        block: block.coordinate,
        argument: u32::try_from(argument).map_err(|_| Resource::Arithmetic)?,
    };
    budget.charge_work(8)?;
    if initial.arguments.get(argument).copied() != initial_definition.value
        || back.arguments.get(argument).copied() != update.value
        || inventory.definitions()[initial_binding.target_definition].coordinate
            != parameter_definition
        || inventory.definitions()[back_binding.target_definition].coordinate
            != parameter_definition
        || *initial_definition.ty != param.ty
    {
        return Err(Error::InconsistentInventory);
    }
    let update_operation = match update.coordinate {
        Definition::Result {
            operation,
            result: 0,
        } => operation,
        _ => return Ok(None),
    };
    if !members[block_index(inventory, update_operation.block, budget)?] {
        return Ok(None);
    }
    let actual = operation(inventory, update_operation, budget)?.operation;
    let (op, operands) = match actual.kind {
        OperationKind::Binary { op, lhs, rhs } => (op, [lhs, rhs]),
        _ => return Ok(None),
    };
    if !matches!(
        op,
        BinaryOp::Add | BinaryOp::Checked(CheckedBinaryOperator::Add)
    ) {
        return Ok(None);
    }
    let parameter_operand = match operands {
        [lhs, _] if lhs == param.id => 0usize,
        [_, rhs] if rhs == param.id => 1usize,
        _ => return Ok(None),
    };
    let step = inventory
        .definition_for_value(
            block.coordinate.function,
            operands[1 - parameter_operand],
            budget,
        )?
        .ok_or(Error::InconsistentInventory)?;
    let literal_operation = match step.coordinate {
        Definition::Result {
            operation,
            result: 0,
        } => operation,
        _ => return Ok(None),
    };
    let literal = operation(inventory, literal_operation, budget)?.operation;
    let (scalar, step_bits) = match &literal.kind {
        OperationKind::Constant(value) => match fixed_integer_bits(value) {
            Some(value) => value,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    budget.charge_work(9)?;
    if step_bits == 0
        || *step.ty != Type::Scalar(scalar)
        || param.ty != *step.ty
        || *update.ty != *step.ty
    {
        return Ok(None);
    }
    let checked = op == BinaryOp::Checked(CheckedBinaryOperator::Add);
    if actual.results.len() != if checked { 2 } else { 1 } || literal.results.len() != 1 {
        return Err(Error::InconsistentInventory);
    }
    if checked && actual.results[1].ty != Type::BOOL {
        return Err(Error::InconsistentInventory);
    }
    Ok(Some(Recurrence {
        parameter: parameter_definition,
        initial_edge: initial.coordinate,
        initial: initial_definition.coordinate,
        backedge: back.coordinate,
        update: update.coordinate,
        step: step.coordinate,
        scalar,
        step_bits,
        parameter_operand: parameter_operand as u8,
        overflow: if checked {
            Some(Definition::Result {
                operation: update_operation,
                result: 1,
            })
        } else {
            None
        },
    }))
}
