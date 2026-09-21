use super::*;

pub(super) fn derive(
    loops: &Loops<'_, '_>,
    rows: &mut Vec<Row>,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let inventory = loops.inventory();
    let mut index = 0;
    for function in inventory.functions() {
        budget.charge_work(2)?;
        if index == loops.loop_count() {
            break;
        }
        if loops.loops[index].header.function != function.coordinate {
            continue;
        }
        with_canonical_kir_control_flow_v1(
            inventory.owner(),
            function.coordinate,
            Default::default(),
            budget,
            |cfg, budget| {
                while index < loops.loop_count() {
                    budget.charge_work(2)?;
                    let natural = loops.natural_loop(index, budget)?;
                    if natural.header.function != function.coordinate {
                        break;
                    }
                    scratch.members(loops, index, budget)?;
                    let iterations = completion(loops, index, scratch, budget)?;
                    for recurrence in loops.recurrences(index, budget)? {
                        budget.charge_work(2)?;
                        let outcome =
                            candidate(loops, index, *recurrence, iterations, cfg, scratch, budget)?;
                        if rows.len() == rows.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        rows.push(Row {
                            loop_ordinal: index,
                            recurrence: *recurrence,
                            outcome,
                        });
                    }
                    index += 1;
                }
                Ok::<_, Error>(())
            },
        )?;
    }
    budget.charge_work(2)?;
    if index != loops.loop_count() || rows.len() != loops.recurrences.len() {
        return Err(Error::ReplayMismatch);
    }
    Ok(())
}

fn candidate(
    loops: &Loops<'_, '_>,
    index: usize,
    recurrence: Recurrence,
    iterations: Iterations,
    cfg: &mut Cfg<'_, '_>,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<Outcome> {
    let i = loops.inventory();
    let natural = loops.natural_loop(index, budget)?;
    budget.charge_work(4)?;
    let Some(maximum) = maximum(recurrence.scalar) else {
        return Ok(Outcome::Unavailable(Unavailable::Scalar));
    };
    if recurrence.step_bits == 0 || recurrence.step_bits > maximum {
        return Ok(Outcome::Unavailable(Unavailable::Scalar));
    }
    let external = loops.external_header_edges(index, budget)?;
    let latches = loops.latch_edges(index, budget)?;
    let Some(preheader) = natural.preheader else {
        return Ok(Outcome::Unavailable(Unavailable::Entries));
    };
    if !natural.single_entry
        || external != [recurrence.initial_edge]
        || latches != [recurrence.backedge]
        || recurrence.initial_edge != preheader
    {
        return Ok(Outcome::Unavailable(Unavailable::Entries));
    }

    let h = block_index(i, natural.header, budget)?;
    budget.charge_work(4)?;
    let Terminator::ConditionalBranch { condition, .. } = i.blocks()[h].terminator else {
        return Ok(Outcome::Unavailable(Unavailable::Guard));
    };
    let condition = resolve(i, natural.header, *condition, budget)?;
    let Definition::Result {
        operation: guard,
        result: 0,
    } = condition
    else {
        return Ok(Outcome::Unavailable(Unavailable::Guard));
    };
    if guard.block != natural.header {
        return Ok(Outcome::Unavailable(Unavailable::Guard));
    }
    let guard_row = operation(i, guard, budget)?;
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = guard_row.operation.kind
    else {
        return Ok(Outcome::Unavailable(Unavailable::Guard));
    };
    let bound = resolve(i, natural.header, rhs, budget)?;
    if resolve(i, natural.header, lhs, budget)? != recurrence.parameter
        || definition(i, condition, budget)?.ty != &Type::BOOL
        || definition(i, bound, budget)?.ty != &Type::Scalar(recurrence.scalar)
    {
        return Ok(Outcome::Unavailable(Unavailable::Guard));
    }
    let body = Edge {
        source: natural.header,
        successor: 0,
    };
    let exit = Edge {
        source: natural.header,
        successor: 1,
    };
    let body_target = edge(i, body, budget)?.target;
    let exit_target = edge(i, exit, budget)?.target;
    budget.charge_work(2)?;
    if scratch.members[block_index(i, body_target, budget)?] == 0
        || scratch.members[block_index(i, exit_target, budget)?] != 0
        || body_target == natural.header
    {
        return Ok(Outcome::Unavailable(Unavailable::Guard));
    }

    if let Some(block) = definition_block(bound) {
        budget.charge_work(1)?;
        if scratch.members[block_index(i, block, budget)?] != 0
            || !cfg.dominates(block, preheader.source, budget)?
        {
            return Ok(Outcome::Unavailable(Unavailable::Bound));
        }
    }
    let Definition::Result {
        operation: update,
        result: 0,
    } = recurrence.update
    else {
        return Ok(Outcome::Unavailable(Unavailable::Control));
    };
    // An update in the guard block executes before its terminator, not after
    // the taken body edge. Never substitute header dominance for edge custody.
    budget.charge_work(3)?;
    if update.block == natural.header
        || scratch.members[block_index(i, update.block, budget)?] == 0
        || !cfg.dominates(update.block, recurrence.backedge.source, budget)?
        || reachable_without(i, update.block, Some(body), None, scratch, budget)?
    {
        return Ok(Outcome::Unavailable(Unavailable::Control));
    }
    let update_row = operation(i, update, budget)?;
    budget.charge_work(1)?;
    if update_row.coordinate.operation as usize
        >= i.blocks()[block_index(i, update.block, budget)?]
            .block
            .operations
            .len()
    {
        return Err(Error::ReplayMismatch);
    }

    let initial = literal(i, recurrence.initial, recurrence.scalar, budget)?;
    let bound_literal = literal(i, bound, recurrence.scalar, budget)?;
    budget.charge_work(8)?;
    let (distance, update) = match (initial, bound_literal) {
        (Some(a), Some(b)) if a >= b => (Distance::Literal(0), Update::NoUpdate),
        (Some(a), Some(b)) => {
            let a = u128::from(a);
            let b = u128::from(b);
            let step = u128::from(recurrence.step_bits);
            let difference = b.checked_sub(a).ok_or(Resource::Arithmetic)?;
            let n = (difference / step)
                .checked_add(u128::from(difference % step != 0))
                .ok_or(Resource::Arithmetic)?;
            let final_value = n
                .checked_mul(step)
                .and_then(|v| a.checked_add(v))
                .ok_or(Resource::Arithmetic)?;
            if final_value > u128::from(maximum) {
                return Ok(Outcome::Unavailable(Unavailable::Arithmetic));
            }
            (
                Distance::Literal(n.try_into().map_err(|_| Resource::Arithmetic)?),
                Update::NonWrapping,
            )
        }
        _ if recurrence.step_bits == 1 => (
            Distance::UnitStride {
                initial: recurrence.initial,
                bound,
            },
            Update::NonWrapping,
        ),
        _ => return Ok(Outcome::Unavailable(Unavailable::Arithmetic)),
    };
    Ok(Outcome::Guarded(Fact {
        guard,
        body,
        exit,
        bound,
        update,
        distance,
        iterations,
    }))
}

// Forward queue, independent of the checker's reverse edge walk. The queue
// contains each dense block at most once; exact duplicate occurrences remain.
fn reachable_without(
    i: &Inventory<'_>,
    target: Block,
    omit_edge: Option<Edge>,
    omit_block: Option<Block>,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    scratch.reset(budget)?;
    let function = &i.functions()[target.function.0 as usize];
    let entry = function.blocks.start;
    budget.charge_work(2)?;
    if Some(i.blocks()[entry].coordinate) == omit_block {
        return Ok(false);
    }
    scratch.marks[entry] = 1;
    scratch.enqueue(entry, 0, budget)?;
    let mut head = 0;
    while head < scratch.pending.len() {
        budget.charge_work(3)?;
        let block = scratch.pending[head].0;
        head += 1;
        if i.blocks()[block].coordinate == target {
            return Ok(true);
        }
        for ordinal in i.blocks()[block].edges.clone() {
            budget.charge_work(3)?;
            let edge = &i.edges()[ordinal];
            if Some(edge.coordinate) == omit_edge || Some(edge.target) == omit_block {
                continue;
            }
            let next = block_index(i, edge.target, budget)?;
            if scratch.marks[next] == 0 {
                scratch.marks[next] = 1;
                scratch.enqueue(next, 0, budget)?;
            }
        }
    }
    Ok(false)
}

// Kahn elimination proves acyclicity of the whole loop body with its header
// removed. All normal successors must stay in the body or be the unique latch
// edge to the header. Explicit calls/abnormal terminals keep counts unavailable.
fn completion(
    loops: &Loops<'_, '_>,
    index: usize,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<Iterations> {
    let i = loops.inventory();
    let natural = loops.natural_loop(index, budget)?;
    let h = block_index(i, natural.header, budget)?;
    let latches = loops.latch_edges(index, budget)?;
    if latches.len() != 1 {
        return Ok(Iterations::Unavailable);
    }
    budget.charge_work(scratch.degrees.len())?;
    scratch.degrees.fill(0);
    scratch.reset(budget)?;
    let mut count = 0usize;
    for block in loops.members(index, budget)? {
        budget.charge_work(2)?;
        let position = block_index(i, *block, budget)?;
        for op in i.blocks()[position].operations.clone() {
            budget.charge_work(1)?;
            if matches!(
                i.operations()[op].operation.kind,
                OperationKind::Call { .. } | OperationKind::InlineAssembly(_)
            ) {
                return Ok(Iterations::Unavailable);
            }
        }
        if position == h {
            continue;
        }
        count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
        let edges = i.blocks()[position].edges.clone();
        if edges.is_empty() {
            return Ok(Iterations::Unavailable);
        }
        for ordinal in edges {
            budget.charge_work(3)?;
            let row = &i.edges()[ordinal];
            let target = block_index(i, row.target, budget)?;
            if target == h {
                if row.coordinate != latches[0] {
                    return Ok(Iterations::Unavailable);
                }
            } else {
                if scratch.members[target] == 0 {
                    return Ok(Iterations::Unavailable);
                }
                scratch.degrees[target] = scratch.degrees[target]
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?;
            }
        }
    }
    for block in loops.members(index, budget)? {
        budget.charge_work(1)?;
        let position = block_index(i, *block, budget)?;
        if position != h && scratch.degrees[position] == 0 {
            scratch.enqueue(position, 0, budget)?;
        }
    }
    let mut head = 0usize;
    while head < scratch.pending.len() {
        budget.charge_work(2)?;
        let position = scratch.pending[head].0;
        head += 1;
        for ordinal in i.blocks()[position].edges.clone() {
            budget.charge_work(2)?;
            let target = block_index(i, i.edges()[ordinal].target, budget)?;
            if target == h {
                continue;
            }
            scratch.degrees[target] = scratch.degrees[target]
                .checked_sub(1)
                .ok_or(Resource::Accounting)?;
            if scratch.degrees[target] == 0 {
                scratch.enqueue(target, 0, budget)?;
            }
        }
    }
    budget.charge_work(1)?;
    Ok(if head == count {
        Iterations::NormalHeaderCompletion
    } else {
        Iterations::Unavailable
    })
}
