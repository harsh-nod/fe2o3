use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAssertMessageV1, SemanticFunctionDeclV1};

type Site = (SemanticFunctionIdV1, SemanticBlockIdV1, u32);

#[derive(Clone, Copy)]
struct Query {
    store: usize,
    function: SemanticFunctionIdV1,
    anchor: SemanticBlockIdV1,
    first: u32,
    local: SemanticLocalIdV1,
    block: SemanticBlockIdV1,
    last: u32,
}

impl Query {
    fn key(self) -> [usize; 7] {
        [
            self.function.index() as usize,
            self.store,
            self.anchor.index() as usize,
            self.first as usize,
            self.local.index() as usize,
            self.block.index() as usize,
            self.last as usize,
        ]
    }

    fn anchor_key(self) -> [usize; 5] {
        let key = self.key();
        [key[0], key[1], key[2], key[3], key[4]]
    }
}

fn queries(
    source: &AdmittedInertSemanticMirV1,
    proof: &PrivateMemory<'_, '_>,
    sites: &[Option<Site>],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<Query>> {
    charge(budget, 2)?;
    if sites.len() != proof.inventory.operations().len() || sites.len() != proof.latest_stores.len()
    {
        return Err(refused("private source", "complete operation source sites"));
    }
    let mut count = 0usize;
    for (read, store) in proof.latest_stores.iter().enumerate() {
        charge(budget, 3)?;
        if let Some(store) = store {
            let first = sites.get(*store).copied().flatten().ok_or_else(|| {
                refused(
                    "private source",
                    "initializing Store has an actual source statement",
                )
            })?;
            let last = sites[read]
                .ok_or_else(|| refused("private source", "Load has an actual source statement"))?;
            if first.1 != last.1 {
                count = count.checked_add(1).ok_or_else(arithmetic)?;
            }
        }
    }
    let mut rows = scratch::<Query>(count, budget)?;
    for (read, store) in proof.latest_stores.iter().enumerate() {
        charge(budget, 4)?;
        let Some(store) = store else {
            continue;
        };
        let (function, anchor, first) = sites[*store].ok_or_else(arithmetic)?;
        let (read_function, block, last) = sites[read].ok_or_else(arithmetic)?;
        if function != read_function {
            return Err(refused(
                "private source",
                "same semantic Store/Load function",
            ));
        }
        if anchor == block {
            continue;
        }
        let declaration = source
            .functions()
            .get(function.index() as usize)
            .ok_or_else(|| refused("private source", "source function exists"))?;
        let statements = declaration
            .blocks()
            .get(anchor.index() as usize)
            .ok_or_else(|| refused("private source", "source block exists"))?
            .statements();
        let read_statements = declaration
            .blocks()
            .get(block.index() as usize)
            .ok_or_else(|| refused("private source", "source block exists"))?
            .statements();
        charge(budget, 5)?;
        let destination = match statements
            .get(first as usize)
            .map(|statement| statement.kind())
        {
            Some(SemanticStatementKindV1::Assign(assignment)) => assignment.destination(),
            Some(SemanticStatementKindV1::Store(store)) => store.destination(),
            _ => {
                return Err(refused(
                    "private source",
                    "initializing source assignment or Store",
                ));
            }
        };
        if last as usize >= read_statements.len() {
            return Err(refused("private source", "source statement interval"));
        }
        let ty = declaration
            .locals()
            .get(destination.local().index() as usize)
            .and_then(|local| source.types().get(local.ty().index() as usize))
            .ok_or_else(|| refused("private source", "source storage declaration"))?;
        let local = source_storage_local(destination, ty.shape(), budget)?;
        if rows.len() == count || rows.len() == rows.capacity() {
            return Err(arithmetic());
        }
        rows.push(Query {
            store: *store,
            function,
            anchor,
            first,
            local,
            block,
            last,
        });
    }
    charge(budget, 1)?;
    if rows.len() != count {
        return Err(arithmetic());
    }
    assert_origin_sort_v1(&mut rows, budget, |left, right, budget| {
        budget.charge_work(7)?;
        Ok(left.key().cmp(&right.key()))
    })
    .map_err(|error| E::SourceOutput(ProductionSourceOutputErrorV1::SourceOrigin(error)))?;
    Ok(rows)
}

#[derive(Clone, Copy)]
struct Kill {
    function: usize,
    block: usize,
    local: SemanticLocalIdV1,
    statement: usize,
    // An ordinary destination overwrite is suppressed only at the exact
    // authenticated anchor. Moves/lifetime/atomic kills at that endpoint stay.
    hard: bool,
}

fn moved(
    operand: &SemanticOperandV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    visit: &mut impl FnMut(SemanticLocalIdV1, bool, &mut AssertOriginBudgetV1<'_>) -> R<()>,
) -> R<()> {
    charge(budget, 1)?;
    if let SemanticOperandV1::Move(place) = operand {
        visit(place.local(), true, budget)?;
    }
    Ok(())
}

fn statement_kills(
    kind: &SemanticStatementKindV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    mut visit: impl FnMut(SemanticLocalIdV1, bool, &mut AssertOriginBudgetV1<'_>) -> R<()>,
) -> R<()> {
    charge(budget, 2)?;
    match kind {
        SemanticStatementKindV1::Assign(assignment) => {
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(_)
                | SemanticRvalueKindV1::Unary { .. }
                | SemanticRvalueKindV1::Binary { .. }
                | SemanticRvalueKindV1::CheckedBinary(_)
                | SemanticRvalueKindV1::UncheckedBinary(_)
                | SemanticRvalueKindV1::Cast { .. }
                | SemanticRvalueKindV1::Aggregate(_) => {
                    visit_statement_kills(kind, budget, |local, budget| {
                        visit(local, true, budget)
                    })?;
                }
                SemanticRvalueKindV1::Borrow { .. }
                | SemanticRvalueKindV1::AddressOf { .. }
                | SemanticRvalueKindV1::Length(_)
                | SemanticRvalueKindV1::Discriminant(_)
                | SemanticRvalueKindV1::Load(_) => {}
            }
            visit(assignment.destination().local(), false, budget)
        }
        SemanticStatementKindV1::Store(store) => {
            visit_statement_kills(kind, budget, |local, budget| visit(local, true, budget))?;
            visit(store.destination().local(), false, budget)
        }
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Deinitialize(_) => {
            visit_statement_kills(kind, budget, |local, budget| visit(local, true, budget))
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. } => {
            visit(place.local(), true, budget)
        }
        SemanticStatementKindV1::Assume(operand) => moved(operand, budget, &mut visit),
        SemanticStatementKindV1::AtomicRmw(atomic) => {
            visit(atomic.destination().local(), true, budget)?;
            visit(atomic.address().local(), true, budget)?;
            moved(atomic.value(), budget, &mut visit)
        }
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            visit(atomic.destination().local(), true, budget)?;
            visit(atomic.address().local(), true, budget)?;
            moved(atomic.expected(), budget, &mut visit)?;
            moved(atomic.replacement(), budget, &mut visit)
        }
        SemanticStatementKindV1::Nop => Ok(()),
    }
}

fn message_moves(
    message: &SemanticAssertMessageV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    visit: &mut impl FnMut(SemanticLocalIdV1, bool, &mut AssertOriginBudgetV1<'_>) -> R<()>,
) -> R<()> {
    charge(budget, 1)?;
    match message {
        SemanticAssertMessageV1::BoundsCheck {
            length: left,
            index: right,
        }
        | SemanticAssertMessageV1::Overflow { left, right, .. }
        | SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: left,
            found_alignment: right,
        } => {
            moved(left, budget, visit)?;
            moved(right, budget, visit)
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => moved(operand, budget, visit),
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
    }
}

fn terminator_kills(
    kind: &SemanticTerminatorKindV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    mut visit: impl FnMut(SemanticLocalIdV1, bool, &mut AssertOriginBudgetV1<'_>) -> R<()>,
) -> R<()> {
    charge(budget, 2)?;
    match kind {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            moved(discriminant, budget, &mut visit)
        }
        SemanticTerminatorKindV1::Call(call) => {
            for operand in call.arguments() {
                moved(operand, budget, &mut visit)?;
            }
            if let Some(destination) = call.destination() {
                // Conservatively kills both normal and unwind successors.
                visit(destination.place().local(), true, budget)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for operand in call.arguments() {
                moved(operand, budget, &mut visit)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::Drop { place, .. } => visit(place.local(), true, budget),
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            moved(condition, budget, &mut visit)?;
            message_moves(message, budget, &mut visit)
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(()),
    }
}

fn visit_kills(
    source: &AdmittedInertSemanticMirV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    mut visit: impl FnMut(Kill, &mut AssertOriginBudgetV1<'_>) -> R<()>,
) -> R<()> {
    for (function, declaration) in source.functions().iter().enumerate() {
        charge(budget, 1)?;
        for (block, body) in declaration.blocks().iter().enumerate() {
            charge(budget, 1)?;
            for (statement, value) in body.statements().iter().enumerate() {
                charge(budget, 1)?;
                statement_kills(value.kind(), budget, |local, hard, budget| {
                    visit(
                        Kill {
                            function,
                            block,
                            local,
                            statement,
                            hard,
                        },
                        budget,
                    )
                })?;
            }
            terminator_kills(body.terminator().kind(), budget, |local, hard, budget| {
                visit(
                    Kill {
                        function,
                        block,
                        local,
                        statement: body.statements().len(),
                        hard,
                    },
                    budget,
                )
            })?;
        }
    }
    Ok(())
}

fn census(
    source: &AdmittedInertSemanticMirV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<Kill>> {
    let mut count = 0usize;
    visit_kills(source, budget, |_, budget| {
        charge(budget, 2)?;
        count = count.checked_add(1).ok_or_else(arithmetic)?;
        Ok(())
    })?;
    let mut rows = scratch::<Kill>(count, budget)?;
    visit_kills(source, budget, |kill, budget| {
        charge(budget, 3)?;
        if rows.len() == count || rows.len() == rows.capacity() {
            return Err(arithmetic());
        }
        rows.push(kill);
        Ok(())
    })?;
    charge(budget, 1)?;
    if rows.len() != count {
        return Err(arithmetic());
    }
    Ok(rows)
}

#[derive(Clone, Copy)]
struct BlockKills {
    first: Option<usize>,
    anchor_exit_killed: bool,
}

fn solve_function(
    function: &SemanticFunctionDeclV1,
    queries: &[Query],
    kills: &[Kill],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let blocks = function.blocks().len();
    let entry = function.entry().index() as usize;
    if entry >= blocks {
        return Err(arithmetic());
    }
    let mut inputs = scratch::<Option<bool>>(blocks, budget)?;
    let mut processings = scratch::<u8>(blocks, budget)?;
    let mut summaries = scratch::<BlockKills>(blocks, budget)?;
    let mut queue = physical_cfg::Queue::new(blocks, budget)?;
    charge(budget, blocks.checked_mul(3).ok_or_else(arithmetic)?)?;
    inputs.resize(blocks, None);
    processings.resize(blocks, 0);
    summaries.resize(
        blocks,
        BlockKills {
            first: None,
            anchor_exit_killed: false,
        },
    );
    let mut start = 0usize;
    while start < queries.len() {
        charge(budget, 3)?;
        let anchor = queries[start];
        let mut end = start.checked_add(1).ok_or_else(arithmetic)?;
        while end < queries.len() {
            charge(budget, 5)?;
            if queries[end].anchor_key() != anchor.anchor_key() {
                break;
            }
            end = end.checked_add(1).ok_or_else(arithmetic)?;
        }
        charge(budget, blocks.checked_mul(3).ok_or_else(arithmetic)?)?;
        inputs.fill(None);
        processings.fill(0);
        summaries.fill(BlockKills {
            first: None,
            anchor_exit_killed: false,
        });
        queue.reset(budget)?;
        for kill in kills {
            charge(budget, 6)?;
            if kill.local != anchor.local {
                continue;
            }
            let summary = summaries.get_mut(kill.block).ok_or_else(arithmetic)?;
            summary.first = Some(
                summary
                    .first
                    .map_or(kill.statement, |old| old.min(kill.statement)),
            );
            if kill.block == anchor.anchor.index() as usize
                && (kill.statement > anchor.first as usize
                    || (kill.statement == anchor.first as usize && kill.hard))
            {
                summary.anchor_exit_killed = true;
            }
        }
        inputs[entry] = Some(false);
        queue.push(entry, budget)?;
        while let Some(block) = queue.pop(budget)? {
            charge(budget, 5)?;
            processings[block] = processings[block].checked_add(1).ok_or_else(arithmetic)?;
            if processings[block] > 2 {
                return Err(arithmetic());
            }
            let incoming = inputs[block].ok_or_else(arithmetic)?;
            let output = if block == anchor.anchor.index() as usize {
                !summaries[block].anchor_exit_killed
            } else {
                incoming && summaries[block].first.is_none()
            };
            function.blocks()[block]
                .terminator()
                .kind()
                .try_for_each_edge(|edge| {
                    charge(budget, 5)?;
                    let target = edge.target().index() as usize;
                    let slot = inputs.get_mut(target).ok_or_else(arithmetic)?;
                    let joined = slot.map_or(output, |old| old && output);
                    if *slot != Some(joined) {
                        *slot = Some(joined);
                        queue.push(target, budget)?;
                    }
                    Ok(())
                })?;
        }
        for query in &queries[start..end] {
            charge(budget, 5)?;
            let block = query.block.index() as usize;
            if inputs[block] != Some(true)
                || summaries[block]
                    .first
                    .is_some_and(|first| first <= query.last as usize)
            {
                return Err(refused(
                    "private source",
                    "exact cross-block anchor survives every source path",
                ));
            }
        }
        start = end;
    }
    Ok(())
}

pub(super) fn check(
    source: &AdmittedInertSemanticMirV1,
    proof: &PrivateMemory<'_, '_>,
    sites: &[Option<Site>],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let queries = queries(source, proof, sites, budget)?;
    let kills = census(source, budget)?;
    let (mut query_start, mut kill_start) = (0usize, 0usize);
    while query_start < queries.len() {
        charge(budget, 4)?;
        let function = queries[query_start].function.index() as usize;
        let mut query_end = query_start;
        while query_end < queries.len() {
            charge(budget, 2)?;
            if queries[query_end].function.index() as usize != function {
                break;
            }
            query_end = query_end.checked_add(1).ok_or_else(arithmetic)?;
        }
        while kill_start < kills.len() {
            charge(budget, 2)?;
            if kills[kill_start].function >= function {
                break;
            }
            kill_start = kill_start.checked_add(1).ok_or_else(arithmetic)?;
        }
        let mut kill_end = kill_start;
        while kill_end < kills.len() {
            charge(budget, 2)?;
            if kills[kill_end].function != function {
                break;
            }
            kill_end = kill_end.checked_add(1).ok_or_else(arithmetic)?;
        }
        let declaration = source.functions().get(function).ok_or_else(arithmetic)?;
        solve_function(
            declaration,
            &queries[query_start..query_end],
            &kills[kill_start..kill_end],
            budget,
        )?;
        query_start = query_end;
        kill_start = kill_end;
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_checked_output_private_source_cfg_v1_tests.rs"]
mod tests;
