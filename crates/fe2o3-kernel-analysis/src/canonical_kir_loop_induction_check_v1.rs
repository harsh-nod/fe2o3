use super::super::Incidence;
use super::*;

pub(super) fn replay(
    report: &CanonicalKirInductionFactsV1<'_, '_, '_>,
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    let loops = report.loops;
    let i = loops.inventory();
    meter.derive(|b| Ok(loops.replay(i, limits, b)?))?;
    meter.work(2)?;
    if report.rows.len() != loops.recurrences.len() {
        return Err(Error::ReplayMismatch);
    }
    let (sparse, sparse_bytes) = sparse_inputs(i, limits, meter)?;
    let (incidence, receipt) = meter.derive(|budget| {
        Ok(super::super::scoped(budget, |budget| {
            budget.reserve_storage(size_of::<Incidence>())?;
            let incidence = Incidence::build(i, budget)?;
            budget.charge_work(5)?;
            let rows = [
                incidence.incoming.capacity(),
                incidence.offsets.capacity(),
                incidence.sources.capacity(),
                incidence.targets.capacity(),
            ]
            .into_iter()
            .try_fold(0usize, |sum, n| {
                sum.checked_add(n).ok_or(Resource::Arithmetic)
            })?;
            let bytes = rows
                .checked_mul(size_of::<usize>())
                .and_then(|n| n.checked_add(size_of::<Incidence>()))
                .ok_or(Resource::Arithmetic)?;
            Ok((incidence, bytes))
        })?)
    })?;
    meter.reserve(receipt)?;
    let (mut scratch, scratch_bytes) = Scratch::new(i, meter)?;
    meter.derive(|budget| {
        let mut cursor = 0;
        for index in 0..loops.loop_count() {
            budget.charge_work(1)?;
            scratch.members(loops, index, budget)?;
            let completion = complete_body(loops, index, &mut scratch, budget)?;
            for recurrence in loops.recurrences(index, budget)? {
                // The full fixed-size relation/outcome comparison is prepaid.
                budget.charge_work(128)?;
                let row = report.rows.get(cursor).ok_or(Error::ReplayMismatch)?;
                if row.loop_ordinal != index || row.recurrence != *recurrence {
                    return Err(Error::ReplayMismatch);
                }
                let expected = inspect(
                    (loops, &sparse),
                    index,
                    *recurrence,
                    completion,
                    &incidence,
                    &mut scratch,
                    budget,
                )?;
                if row.outcome != expected {
                    return Err(Error::ReplayMismatch);
                }
                cursor += 1;
            }
        }
        budget.charge_work(3)?;
        if cursor != report.rows.len() {
            return Err(Error::ReplayMismatch);
        }
        Ok(())
    })?;
    drop(scratch);
    meter.release(scratch_bytes)?;
    drop(incidence);
    meter.release(receipt)?;
    drop(sparse);
    meter.release(sparse_bytes)?;
    Ok(())
}

fn inspect(
    inputs: (&Loops<'_, '_>, &Sparse<'_, '_>),
    ordinal: usize,
    recurrence: Recurrence,
    iterations: Iterations,
    incidence: &Incidence,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<Outcome> {
    let (loops, sparse) = inputs;
    let i = loops.inventory();
    budget.charge_work(4)?;
    let max = match maximum(recurrence.scalar) {
        Some(max) if recurrence.step_bits != 0 && recurrence.step_bits <= max => max,
        _ => return Ok(Outcome::Unavailable(Unavailable::Scalar)),
    };
    let natural = loops.natural_loop(ordinal, budget)?;
    let outside = loops.external_header_edges(ordinal, budget)?;
    let back = loops.latch_edges(ordinal, budget)?;
    let single = natural.single_entry && outside.len() == 1 && back.len() == 1;
    if !single
        || natural.preheader != Some(recurrence.initial_edge)
        || outside[0] != recurrence.initial_edge
        || back[0] != recurrence.backedge
    {
        return Ok(Outcome::Unavailable(Unavailable::Entries));
    }
    let header = block_index(i, natural.header, budget)?;
    budget.charge_work(4)?;
    let condition_id = match i.blocks()[header].terminator {
        Terminator::ConditionalBranch { condition, .. } => *condition,
        _ => return Ok(Outcome::Unavailable(Unavailable::Guard)),
    };
    let condition = resolve(i, natural.header, condition_id, budget)?;
    let guard = match condition {
        Definition::Result {
            operation,
            result: 0,
        } if operation.block == natural.header => operation,
        _ => return Ok(Outcome::Unavailable(Unavailable::Guard)),
    };
    let (lhs, rhs) = match &operation(i, guard, budget)?.operation.kind {
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } => (*lhs, *rhs),
        _ => return Ok(Outcome::Unavailable(Unavailable::Guard)),
    };
    let actual_parameter = resolve(i, natural.header, lhs, budget)?;
    let bound = resolve(i, natural.header, rhs, budget)?;
    if actual_parameter != recurrence.parameter
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
    let yes = block_index(i, edge(i, body, budget)?.target, budget)?;
    let no = block_index(i, edge(i, exit, budget)?.target, budget)?;
    budget.charge_work(2)?;
    if yes == header || scratch.members[yes] == 0 || scratch.members[no] != 0 {
        return Ok(Outcome::Unavailable(Unavailable::Guard));
    }
    if let Some(definition) = definition_block(bound) {
        let position = block_index(i, definition, budget)?;
        budget.charge_work(1)?;
        if scratch.members[position] != 0
            || reaches_entry(
                i,
                recurrence.initial_edge.source,
                None,
                Some(definition),
                incidence,
                scratch,
                budget,
            )?
        {
            return Ok(Outcome::Unavailable(Unavailable::Bound));
        }
    }
    let update = match recurrence.update {
        Definition::Result {
            operation,
            result: 0,
        } => operation,
        _ => return Ok(Outcome::Unavailable(Unavailable::Control)),
    };
    budget.charge_work(3)?;
    let update_position = block_index(i, update.block, budget)?;
    if update_position == header
        || scratch.members[update_position] == 0
        || reaches_entry(
            i,
            recurrence.backedge.source,
            None,
            Some(update.block),
            incidence,
            scratch,
            budget,
        )?
        || reaches_entry(
            i,
            update.block,
            Some(body),
            None,
            incidence,
            scratch,
            budget,
        )?
    {
        return Ok(Outcome::Unavailable(Unavailable::Control));
    }
    // Actual result lookup validates operation ordinal/type; nonheader placement
    // then puts the operation strictly after the header's taken terminator edge.
    let update_definition = definition(i, recurrence.update, budget)?;
    budget.charge_work(2)?;
    if update_definition.ty != &Type::Scalar(recurrence.scalar)
        || update.operation as usize >= i.blocks()[update_position].block.operations.len()
    {
        return Err(Error::ReplayMismatch);
    }

    let a = literal(sparse, recurrence.initial, recurrence.scalar, budget)?;
    let b = literal(sparse, bound, recurrence.scalar, budget)?;
    budget.charge_work(12)?;
    let (distance, update) = if let (Some(a), Some(b)) = (a, b) {
        if a >= b {
            (Distance::Literal(0), Update::NoUpdate)
        } else {
            // Independent arithmetic: 1+(B-A-1)/S, then explicit inequalities.
            let a = u128::from(a);
            let b = u128::from(b);
            let s = u128::from(recurrence.step_bits);
            let n = b
                .checked_sub(a)
                .and_then(|v| v.checked_sub(1))
                .map(|v| v / s)
                .and_then(|v| v.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            let last = n
                .checked_sub(1)
                .and_then(|v| v.checked_mul(s))
                .and_then(|v| a.checked_add(v))
                .ok_or(Resource::Arithmetic)?;
            let final_ = n
                .checked_mul(s)
                .and_then(|v| a.checked_add(v))
                .ok_or(Resource::Arithmetic)?;
            if !(last < b && b <= final_ && final_ <= u128::from(max)) {
                return Ok(Outcome::Unavailable(Unavailable::Arithmetic));
            }
            (
                Distance::Literal(n.try_into().map_err(|_| Resource::Arithmetic)?),
                Update::NonWrapping,
            )
        }
    } else if recurrence.step_bits == 1 {
        (
            Distance::UnitStride {
                initial: recurrence.initial,
                bound,
            },
            Update::NonWrapping,
        )
    } else {
        return Ok(Outcome::Unavailable(Unavailable::Arithmetic));
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

// Reverse reachability uses existing exact occurrence incidence, never producer
// CFG flags/dominators or forward traversal state. Each dense block queues once.
fn reaches_entry(
    i: &Inventory<'_>,
    target: Block,
    omitted_edge: Option<Edge>,
    omitted_block: Option<Block>,
    incidence: &Incidence,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    scratch.reset(budget)?;
    budget.charge_work(2)?;
    if Some(target) == omitted_block {
        return Ok(false);
    }
    let target = block_index(i, target, budget)?;
    let entry = i.functions()[i.blocks()[target].coordinate.function.0 as usize]
        .blocks
        .start;
    scratch.marks[target] = 1;
    scratch.enqueue(target, 0, budget)?;
    let mut head = 0;
    while head < scratch.pending.len() {
        budget.charge_work(3)?;
        let current = scratch.pending[head].0;
        head += 1;
        if current == entry {
            return Ok(true);
        }
        for ordinal in incidence.predecessors(current) {
            budget.charge_work(3)?;
            let edge = &i.edges()[*ordinal];
            if Some(edge.coordinate) == omitted_edge
                || Some(edge.coordinate.source) == omitted_block
            {
                continue;
            }
            let source = incidence.sources[*ordinal];
            if scratch.marks[source] == 0 {
                scratch.marks[source] = 1;
                scratch.enqueue(source, 0, budget)?;
            }
        }
    }
    Ok(false)
}

// Independently inspect the actual condition definition, never a producer flag.
fn excluded_external_successor(
    i: &Inventory<'_>,
    coordinate: Edge,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    budget.charge_work(8)?;
    let source = &i.blocks()[block_index(i, coordinate.source, budget)?];
    let condition = match source.terminator {
        Terminator::ConditionalBranch { condition, .. } => *condition,
        _ => return Ok(false),
    };
    let definition = resolve(i, coordinate.source, condition, budget)?;
    let site = match definition {
        Definition::Result {
            operation,
            result: 0,
        } => operation,
        _ => return Ok(false),
    };
    let actual = operation(i, site, budget)?.operation;
    match &actual.kind {
        OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(value)) => {
            if actual.results.len() != 1
                || actual.results[0].id != condition
                || actual.results[0].ty != Type::BOOL
                || coordinate.successor > 1
            {
                return Err(Error::ReplayMismatch);
            }
            Ok(matches!(
                (*value, coordinate.successor),
                (false, 0) | (true, 1)
            ))
        }
        _ => Ok(false),
    }
}

// Independent color/stack DFS detects every body cycle. It does not consume a
// producer topological order, member count or Kahn-indegree result.
fn complete_body(
    loops: &Loops<'_, '_>,
    index: usize,
    scratch: &mut Scratch,
    budget: &mut Budget<'_>,
) -> Result<Iterations> {
    let i = loops.inventory();
    let natural = loops.natural_loop(index, budget)?;
    let header = block_index(i, natural.header, budget)?;
    let latch = loops.latch_edges(index, budget)?;
    if latch.len() != 1 {
        return Ok(Iterations::Unavailable);
    }
    scratch.reset(budget)?;
    for block in loops.members(index, budget)? {
        let position = block_index(i, *block, budget)?;
        for ordinal in i.blocks()[position].operations.clone() {
            budget.charge_work(1)?;
            if matches!(
                i.operations()[ordinal].operation.kind,
                OperationKind::Call { .. } | OperationKind::InlineAssembly(_)
            ) {
                return Ok(Iterations::Unavailable);
            }
        }
        budget.charge_work(2)?;
        if position == header {
            continue;
        }
        if i.blocks()[position].edges.is_empty() {
            return Ok(Iterations::Unavailable);
        }
        for ordinal in i.blocks()[position].edges.clone() {
            budget.charge_work(3)?;
            let row = &i.edges()[ordinal];
            let target = block_index(i, row.target, budget)?;
            if scratch.members[target] == 0 {
                if excluded_external_successor(i, row.coordinate, budget)? {
                    continue;
                }
                return Ok(Iterations::Unavailable);
            }
            if target == header && row.coordinate != latch[0] {
                return Ok(Iterations::Unavailable);
            }
        }
    }
    for block in loops.members(index, budget)? {
        let position = block_index(i, *block, budget)?;
        budget.charge_work(2)?;
        if position == header || scratch.marks[position] != 0 {
            continue;
        }
        scratch.marks[position] = 1;
        scratch.enqueue(position, 0, budget)?;
        while let Some(&(current, next)) = scratch.pending.last() {
            budget.charge_work(3)?;
            let edges = i.blocks()[current].edges.clone();
            if next == edges.len() {
                scratch.marks[current] = 2;
                scratch.pending.pop();
                continue;
            }
            let last = scratch.pending.len() - 1;
            scratch.pending[last].1 = next.checked_add(1).ok_or(Resource::Arithmetic)?;
            let row = &i.edges()[edges.start + next];
            let target = block_index(i, row.target, budget)?;
            if scratch.members[target] == 0
                && excluded_external_successor(i, row.coordinate, budget)?
            {
                continue;
            }
            if target == header {
                continue;
            }
            match scratch.marks[target] {
                1 => return Ok(Iterations::Unavailable),
                0 => {
                    scratch.marks[target] = 1;
                    scratch.enqueue(target, 0, budget)?;
                }
                _ => {}
            }
        }
    }
    Ok(Iterations::NormalHeaderCompletion)
}
