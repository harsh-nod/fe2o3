impl<'i, 'g> CanonicalKirLoopsV1<'i, 'g> {
    fn rows(&self) -> Result<usize> {
        self.members
            .len()
            .checked_add(self.edges.len())
            .and_then(|n| n.checked_add(self.recurrences.len()))
            .ok_or(Resource::Arithmetic.into())
    }
    fn next_row(&self, limits: CanonicalKirLoopLimitsV1, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        count(
            "retained rows",
            self.rows()?.checked_add(1).ok_or(Resource::Arithmetic)?,
            limits.rows,
        )
    }
    fn build(
        inventory: &'i Inventory<'g>,
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        input_limits(inventory, limits, budget)?;
        budget.reserve_storage(size_of::<Self>())?;
        let mut report = Self {
            inventory,
            loops: Vec::new(),
            members: Vec::new(),
            edges: Vec::new(),
            recurrences: Vec::new(),
            retained: 0,
        };
        let incidence = Incidence::build(inventory, budget)?;
        let mut scratch = Scratch::new(inventory.blocks().len(), inventory.edges().len(), budget)?;
        // All output slots touched by the scoped CFG are already prepaid.
        for function in inventory.functions() {
            budget.charge_work(2)?;
            if function.function.body.is_none() {
                continue;
            }
            with_canonical_kir_control_flow_v1(
                inventory.owner(),
                function.coordinate,
                Default::default(),
                budget,
                |cfg, budget| {
                    for index in function.blocks.clone() {
                        budget.charge_work(1)?;
                        scratch.reachable[index] =
                            cfg.is_reachable(inventory.blocks()[index].coordinate, budget)?;
                    }
                    for index in function.edges.clone() {
                        budget.charge_work(2)?;
                        let edge = &inventory.edges()[index];
                        let dominated =
                            cfg.dominates(edge.target, edge.coordinate.source, budget)?;
                        scratch.backedges[index] = dominated;
                        if dominated {
                            scratch.headers[incidence.targets[index]] = true;
                        }
                    }
                    Ok::<_, Error>(())
                },
            )?;
        }
        for header in 0..inventory.blocks().len() {
            budget.charge_work(1)?;
            if !scratch.headers[header] {
                continue;
            }
            count(
                "loops",
                report
                    .loops
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
                limits.loops,
            )?;
            discover_members(header, &incidence, &mut scratch, budget)?;
            report.emit_loop(header, &incidence, &mut scratch, limits, budget)?;
        }
        report.retained = size_of::<Self>()
            .checked_add(bytes::<NaturalLoop>(report.loops.capacity())?)
            .and_then(|n| n.checked_add(bytes::<Block>(report.members.capacity()).ok()?))
            .and_then(|n| n.checked_add(bytes::<Edge>(report.edges.capacity()).ok()?))
            .and_then(|n| n.checked_add(bytes::<Recurrence>(report.recurrences.capacity()).ok()?))
            .ok_or(Resource::Arithmetic)?;
        Ok(report)
    }

    fn emit_loop(
        &mut self,
        header: usize,
        graph: &Incidence,
        scratch: &mut Scratch,
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let inventory = self.inventory;
        let header_coord = inventory.blocks()[header].coordinate;
        let function = &inventory.functions()[header_coord.function.0 as usize];
        let member_start = self.members.len();
        for block in function.blocks.clone() {
            budget.charge_work(1)?;
            if scratch.members[block] {
                self.next_row(limits, budget)?;
                push(
                    &mut self.members,
                    inventory.blocks()[block].coordinate,
                    limits.rows,
                    budget,
                )?;
            }
        }
        let latch_start = self.edges.len();
        for &edge in graph.predecessors(header) {
            budget.charge_work(2)?;
            if scratch.backedges[edge] {
                self.next_row(limits, budget)?;
                push(
                    &mut self.edges,
                    inventory.edges()[edge].coordinate,
                    limits.rows,
                    budget,
                )?;
            }
        }
        let external_start = self.edges.len();
        let mut only_external = None;
        for &edge in graph.predecessors(header) {
            budget.charge_work(2)?;
            if !scratch.members[graph.sources[edge]] {
                self.next_row(limits, budget)?;
                push(
                    &mut self.edges,
                    inventory.edges()[edge].coordinate,
                    limits.rows,
                    budget,
                )?;
                only_external = Some(edge);
            }
        }
        let exit_start = self.edges.len();
        let mut single_entry = header_coord.block != 0;
        let mut dedicated_exits = true;
        budget.charge_work(scratch.avoiding.len())?;
        scratch.avoiding.fill(false);
        for edge in function.edges.clone() {
            budget.charge_work(4)?;
            let source = graph.sources[edge];
            let target = graph.targets[edge];
            if !scratch.members[source] && scratch.members[target] && target != header {
                single_entry = false;
            }
            if !scratch.members[source] {
                scratch.avoiding[target] = true;
            }
        }
        for edge in function.edges.clone() {
            budget.charge_work(4)?;
            let source = graph.sources[edge];
            let target = graph.targets[edge];
            if scratch.members[source] && !scratch.members[target] {
                self.next_row(limits, budget)?;
                push(
                    &mut self.edges,
                    inventory.edges()[edge].coordinate,
                    limits.rows,
                    budget,
                )?;
                if scratch.avoiding[target] {
                    dedicated_exits = false;
                }
            }
        }
        let external = external_start..exit_start;
        let latches = latch_start..external_start;
        let preheader = only_external
            .filter(|&edge| {
                external.len() == 1
                    && scratch.reachable[graph.sources[edge]]
                    && matches!(
                        inventory.blocks()[graph.sources[edge]].terminator,
                        Terminator::Branch { .. }
                    )
            })
            .map(|edge| inventory.edges()[edge].coordinate);
        let recurrence_start = self.recurrences.len();
        if single_entry
            && preheader.is_some()
            && latches.len() == 1
            && graph.predecessors(header).len() == 2
        {
            budget.charge_work(3)?;
            let initial_edge = only_external.ok_or(Error::InconsistentInventory)?;
            let backedge = *graph
                .predecessors(header)
                .iter()
                .find(|&&e| scratch.backedges[e])
                .ok_or(Error::InconsistentInventory)?;
            if matches!(
                inventory.blocks()[graph.sources[backedge]].terminator,
                Terminator::Branch { .. }
            ) {
                for argument in 0..inventory.blocks()[header].block.parameters.len() {
                    budget.charge_work(1)?;
                    if let Some(fact) = discover_recurrence(
                        inventory,
                        header,
                        argument,
                        initial_edge,
                        backedge,
                        &scratch.members,
                        budget,
                    )? {
                        self.next_row(limits, budget)?;
                        push(&mut self.recurrences, fact, limits.rows, budget)?;
                    }
                }
            }
        }
        let fact = NaturalLoop {
            header: header_coord,
            members: member_start..self.members.len(),
            latches,
            external,
            exits: exit_start..self.edges.len(),
            recurrences: recurrence_start..self.recurrences.len(),
            single_entry,
            preheader,
            dedicated_exits,
        };
        push(&mut self.loops, fact, limits.loops, budget)
    }
}

fn discover_members(
    header: usize,
    graph: &Incidence,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<()> {
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
        budget.charge_work(1)?;
        let Some(block) = scratch.stack.pop() else {
            break;
        };
        for &edge in graph.predecessors(block) {
            budget.charge_work(4)?;
            let source = graph.sources[edge];
            if scratch.reachable[source] && !scratch.members[source] {
                scratch.members[source] = true;
                scratch.stack.push(source);
            }
        }
    }
    Ok(())
}

fn discover_recurrence(
    inventory: &Inventory<'_>,
    header: usize,
    argument: usize,
    initial_edge: usize,
    backedge: usize,
    members: &[bool],
    budget: &mut Budget<'_>,
) -> Result<Option<Recurrence>> {
    budget.charge_work(9)?;
    let block = &inventory.blocks()[header];
    let parameter = &block.block.parameters[argument];
    let initial_row = &inventory.edges()[initial_edge];
    let back_row = &inventory.edges()[backedge];
    let initial = &inventory.edge_arguments()[initial_row.bindings.start + argument];
    let update = &inventory.edge_arguments()[back_row.bindings.start + argument];
    let update_def = &inventory.definitions()[update.incoming_definition];
    let Definition::Result {
        operation: update_coord,
        result: 0,
    } = update_def.coordinate
    else {
        return Ok(None);
    };
    if !members[block_index(inventory, update_coord.block, budget)?] {
        return Ok(None);
    }
    let producer = operation(inventory, update_coord, budget)?.operation;
    let (lhs, rhs, checked) = match producer.kind {
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs,
            rhs,
        } => (lhs, rhs, false),
        OperationKind::Binary {
            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
            lhs,
            rhs,
        } => (lhs, rhs, true),
        _ => return Ok(None),
    };
    let (step_value, parameter_operand) = if lhs == parameter.id {
        (rhs, 0)
    } else if rhs == parameter.id {
        (lhs, 1)
    } else {
        return Ok(None);
    };
    let Some(step) =
        inventory.definition_for_value(block.coordinate.function, step_value, budget)?
    else {
        return Err(Error::InconsistentInventory);
    };
    let Definition::Result {
        operation: literal_coord,
        result: 0,
    } = step.coordinate
    else {
        return Ok(None);
    };
    let literal = operation(inventory, literal_coord, budget)?.operation;
    let OperationKind::Constant(ref constant) = literal.kind else {
        return Ok(None);
    };
    budget.charge_work(8)?;
    let Some((scalar, step_bits)) = fixed_integer_bits(constant) else {
        return Ok(None);
    };
    if step_bits == 0
        || parameter.ty != Type::Scalar(scalar)
        || *step.ty != parameter.ty
        || *update_def.ty != parameter.ty
    {
        return Ok(None);
    }
    if producer.results.len() != if checked { 2 } else { 1 } || literal.results.len() != 1 {
        return Err(Error::InconsistentInventory);
    }
    let overflow = checked.then_some(Definition::Result {
        operation: update_coord,
        result: 1,
    });
    Ok(Some(Recurrence {
        parameter: Definition::BlockArgument {
            block: block.coordinate,
            argument: u32::try_from(argument).map_err(|_| Resource::Arithmetic)?,
        },
        initial_edge: initial_row.coordinate,
        initial: inventory.definitions()[initial.incoming_definition].coordinate,
        backedge: back_row.coordinate,
        update: update_def.coordinate,
        step: step.coordinate,
        scalar,
        step_bits,
        parameter_operand,
        overflow,
    }))
}
