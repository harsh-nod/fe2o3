#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TerminalFailureClosureV18 {
    origin: usize,
    block: BlockId,
    original_gap: u32,
    first: u32,
    scope_ends: u32,
    diagnostic: u32,
    generated: bool,
}

struct TerminalFailureRelationV18 {
    origins: TerminalFailureOriginsV18,
    closures: Vec<TerminalFailureClosureV18>,
}

struct PreparedTerminalFailureV18 {
    witness: TerminalFailureClosureV18,
    operations: Vec<Operation>,
    visited: bool,
    source_index: usize,
}

struct PreparedTerminalFailuresV18 {
    rows: Vec<PreparedTerminalFailureV18>,
    by_block: Vec<(BlockId, usize)>,
    redirects: Vec<((BlockId, u32), usize)>,
    generated: Vec<usize>,
    source_blocks: Vec<(BlockId, usize)>,
}

impl PreparedTerminalFailuresV18 {
    fn prepare(
        owner: &OwnedPendingScopedRootV29,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let origins = &owner.terminal_failures;
        budget.charge_work(5)?;
        if origins.ledger != budget.work_ledger_identity_v1()
            || origins.source.semantic != owner.pending.coordinates.semantic_sha256
            || origins.source.ssa != owner.pending.coordinates.ssa
            || origins.source.root != owner.pending.coordinates.root
        {
            return Err(terminal_failure_error_v18());
        }
        let body = owner
            .pending
            .function
            .body
            .as_ref()
            .ok_or_else(terminal_failure_error_v18)?;
        let mut maximum_block = None;
        let mut source_blocks = emission_vec_v1(body.blocks.len(), budget)?;
        for (index, block) in body.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            maximum_block = Some(maximum_block.map_or(block.id.0, |old: u32| old.max(block.id.0)));
            source_blocks.push((block.id, index));
        }
        terminal_failure_sort_v18(&mut source_blocks, budget)?;
        let mut edge_count = 0;
        for row in &origins.rows {
            budget.charge_work(1)?;
            if matches!(row.site, TerminalFailureSiteV18::Edge { .. }) {
                edge_count = argument_sum_v1(&[edge_count, 1])?;
            }
        }
        enforce_limit(
            ProductionSemanticKirResourceV1::Blocks,
            argument_sum_v1(&[body.blocks.len(), edge_count])?,
            limits.max_blocks,
        )?;
        let mut rows = emission_vec_v1(origins.rows.len(), budget)?;
        let mut by_block = emission_vec_v1(origins.rows.len(), budget)?;
        let mut redirects = emission_vec_v1(edge_count, budget)?;
        let mut generated = emission_vec_v1(edge_count, budget)?;
        let mut next_block = if edge_count == 0 {
            0
        } else {
            maximum_block
                .map_or(Some(0), |id| id.checked_add(1))
                .ok_or(ArgumentResourceV1::Arithmetic)?
        };
        for (origin, row) in origins.rows.iter().enumerate() {
            budget.charge_work(3)?;
            let source_block = match row.site {
                TerminalFailureSiteV18::Edge { block, .. }
                | TerminalFailureSiteV18::Operation { block, .. } => block,
            };
            let source_index = terminal_failure_find_v18(&source_blocks, &source_block, budget)?;
            let (block, original_gap, is_generated) = match row.site {
                TerminalFailureSiteV18::Edge {
                    block, successor, ..
                } => {
                    let output = BlockId(next_block);
                    if generated.len() + 1 < edge_count {
                        next_block = next_block
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                    redirects.push(((block, successor), origin));
                    generated.push(origin);
                    (output, 0, true)
                }
                TerminalFailureSiteV18::Operation { block, operation } => (block, operation, false),
            };
            by_block.push((block, origin));
            rows.push(PreparedTerminalFailureV18 {
                witness: TerminalFailureClosureV18 {
                    origin,
                    block,
                    original_gap,
                    first: original_gap,
                    scope_ends: 0,
                    diagnostic: original_gap,
                    generated: is_generated,
                },
                operations: Vec::new(),
                visited: false,
                source_index,
            });
        }
        terminal_failure_sort_v18(&mut by_block, budget)?;
        terminal_failure_sort_v18(&mut redirects, budget)?;
        Ok(Self {
            rows,
            by_block,
            redirects,
            generated,
            source_blocks,
        })
    }

    fn block_count(&self, original: usize) -> Result<usize, ProductionSemanticKirErrorV1> {
        Ok(argument_sum_v1(&[original, self.generated.len()])?)
    }

    fn generated_block(&self, index: usize) -> Result<BlockId, ProductionSemanticKirErrorV1> {
        self.generated
            .get(index)
            .and_then(|row| self.rows.get(*row))
            .map(|row| row.witness.block)
            .ok_or_else(terminal_failure_error_v18)
    }

    fn redirect(
        &self,
        block: BlockId,
        edge: u32,
        target: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<BlockId, ProductionSemanticKirErrorV1> {
        budget.charge_work(call_splice_search_work_v1(self.redirects.len()))?;
        match self
            .redirects
            .binary_search_by_key(&(block, edge), |row| row.0)
        {
            Ok(index) => Ok(self.rows[self.redirects[index].1].witness.block),
            Err(_) => Ok(target),
        }
    }

    fn row_for_block(
        &self,
        block: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        budget.charge_work(call_splice_search_work_v1(self.by_block.len()))?;
        Ok(self
            .by_block
            .binary_search_by_key(&block, |row| row.0)
            .ok()
            .map(|i| self.by_block[i].1))
    }

    fn check_complete(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(self.rows.len())?;
        if self.rows.iter().any(|row| !row.visited) {
            return Err(terminal_failure_error_v18());
        }
        Ok(())
    }

    fn scratch_storage(&self) -> Result<usize, ProductionSemanticKirErrorV1> {
        Ok(argument_sum_v1(&[
            argument_product_v1(
                self.rows.capacity(),
                size_of::<PreparedTerminalFailureV18>(),
            )?,
            argument_product_v1(self.by_block.capacity(), size_of::<(BlockId, usize)>())?,
            argument_product_v1(
                self.redirects.capacity(),
                size_of::<((BlockId, u32), usize)>(),
            )?,
            argument_product_v1(self.generated.capacity(), size_of::<usize>())?,
            argument_product_v1(self.source_blocks.capacity(), size_of::<(BlockId, usize)>())?,
        ])?)
    }
}

fn terminal_failure_edge_v18(
    terminator: &Terminator,
    successor: u32,
) -> Option<(BlockId, &[ValueId])> {
    match (terminator, successor) {
        (Terminator::Branch { target, arguments }, 0) => Some((*target, arguments)),
        (
            Terminator::ConditionalBranch {
                then_target,
                then_arguments,
                ..
            },
            0,
        ) => Some((*then_target, then_arguments)),
        (
            Terminator::ConditionalBranch {
                else_target,
                else_arguments,
                ..
            },
            1,
        ) => Some((*else_target, else_arguments)),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_terminal_replacements_v18(
    owner: &OwnedPendingScopedRootV29,
    events: &[PreparedLifecycleEventV29],
    failures: &mut PreparedTerminalFailuresV18,
    replacements: &mut Vec<PreparedLifecycleBlockV29>,
    generated: &mut Vec<BasicBlock>,
    closures: &mut Vec<TerminalFailureClosureV18>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let body = owner
        .pending
        .function
        .body
        .as_ref()
        .ok_or_else(terminal_failure_error_v18)?;
    let mut total = events.len();
    for block in &body.blocks {
        budget.charge_work(1)?;
        total = argument_sum_v1(&[total, block.operations.len()])?;
    }
    for (index, row) in failures.rows.iter_mut().enumerate() {
        budget.charge_work(5)?;
        let origin = owner
            .terminal_failures
            .rows
            .get(row.witness.origin)
            .ok_or_else(terminal_failure_error_v18)?;
        let source = body
            .blocks
            .get(row.source_index)
            .ok_or_else(terminal_failure_error_v18)?;
        if !row.visited || row.operations.len() != row.witness.scope_ends as usize {
            return Err(terminal_failure_error_v18());
        }
        total = argument_sum_v1(&[
            total,
            row.operations.len(),
            usize::from(row.witness.generated),
        ])?;
        row.witness.first = row.witness.original_gap;
        row.witness.diagnostic = row
            .witness
            .first
            .checked_add(row.witness.scope_ends)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        match origin.site {
            TerminalFailureSiteV18::Edge {
                block,
                successor,
                target,
            } => {
                if !row.witness.generated || source.id != block {
                    return Err(terminal_failure_error_v18());
                }
                let actual = source
                    .terminator
                    .as_ref()
                    .and_then(|t| terminal_failure_edge_v18(t, successor))
                    .ok_or_else(terminal_failure_error_v18)?;
                if actual.0 != target || !actual.1.is_empty() {
                    return Err(terminal_failure_error_v18());
                }
                let trap_index =
                    terminal_failure_find_v18(&failures.source_blocks, &target, budget)?;
                let trap = body
                    .blocks
                    .get(trap_index)
                    .ok_or_else(terminal_failure_error_v18)?;
                let [operation] = trap.operations.as_slice() else {
                    return Err(terminal_failure_error_v18());
                };
                if !terminal_failure_is_trap_v18(operation, budget)? {
                    return Err(terminal_failure_error_v18());
                }
                let OperationKind::Call { callee, .. } = &operation.kind else {
                    unreachable!()
                };
                let count = argument_sum_v1(&[row.operations.len(), 1])?;
                enforce_limit(
                    ProductionSemanticKirResourceV1::Operations,
                    count,
                    MAX_BLOCK_OPERATIONS_V1,
                )?;
                let mut output = BasicBlock::new(row.witness.block);
                output.operations = emission_vec_v1(count, budget)?;
                budget.charge_work(count)?;
                let callee = FunctionId::new(scoped_copy_string_v29(callee.as_str(), budget)?);
                output.operations.append(&mut row.operations);
                output.operations.push(Operation::new(
                    Vec::new(),
                    OperationKind::Call {
                        callee,
                        arguments: Vec::new(),
                    },
                ));
                output.terminator = Some(Terminator::Unreachable);
                generated.push(output);
            }
            TerminalFailureSiteV18::Operation { block, operation } => {
                budget.charge_work(call_splice_search_work_v1(events.len()))?;
                if row.witness.generated
                    || source.id != block
                    || row.witness.original_gap != operation
                    || argument_sum_v1(&[operation as usize, 1])? != source.operations.len()
                    || events
                        .binary_search_by_key(&row.source_index, |event| event.block)
                        .is_ok()
                    || !matches!(source.terminator, Some(Terminator::Unreachable))
                    || !terminal_failure_is_trap_v18(
                        &source.operations[operation as usize],
                        budget,
                    )?
                {
                    return Err(terminal_failure_error_v18());
                }
                if !row.operations.is_empty() {
                    let count = argument_sum_v1(&[source.operations.len(), row.operations.len()])?;
                    enforce_limit(
                        ProductionSemanticKirResourceV1::Operations,
                        count,
                        MAX_BLOCK_OPERATIONS_V1,
                    )?;
                    budget.charge_work(count)?;
                    replacements.push(PreparedLifecycleBlockV29 {
                        index: row.source_index,
                        events: 0..0,
                        operations: emission_vec_v1(count, budget)?,
                        terminal: Some(index),
                    });
                }
            }
        }
        closures.push(row.witness);
    }
    enforce_limit(
        ProductionSemanticKirResourceV1::Operations,
        total,
        limits.max_operations,
    )?;
    budget.charge_work(argument_sum_v1(&[
        body.blocks.len(),
        generated.len(),
        argument_product_v1(failures.rows.len(), 2)?,
    ])?)?;
    Ok(())
}

fn install_terminal_edges_v18(
    body: &mut fe2o3_kernel_ir::FunctionBody,
    origins: &TerminalFailureOriginsV18,
    failures: &PreparedTerminalFailuresV18,
) {
    for row in &failures.rows {
        let TerminalFailureSiteV18::Edge { successor, .. } = origins.rows[row.witness.origin].site
        else {
            continue;
        };
        let terminator = body.blocks[row.source_index]
            .terminator
            .as_mut()
            .expect("validated failure terminator");
        let target = match (terminator, successor) {
            (Terminator::Branch { target, .. }, 0) => target,
            (Terminator::ConditionalBranch { then_target, .. }, 0) => then_target,
            (Terminator::ConditionalBranch { else_target, .. }, 1) => else_target,
            _ => unreachable!("validated terminal successor"),
        };
        *target = row.witness.block;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleLiveV18 {
    Absent,
    Context,
    Borrowed(usize),
    Workgroup(usize),
    Descendant(usize),
}

fn lifecycle_scope_discard_v18(
    slots: &[(ValueId, fe2o3_kernel_ir::ExecutionRoleV15)],
    state: &mut [LifecycleLiveV18],
    workgroup: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ValueId>, ProductionSemanticKirErrorV1> {
    use LifecycleLiveV18 as Live;
    let Some(Live::Workgroup(context)) = state.get(workgroup).copied() else {
        return Err(execution_lifecycle_error_v29());
    };
    if state.get(context) != Some(&Live::Borrowed(workgroup)) {
        return Err(execution_lifecycle_error_v29());
    }
    budget.charge_work(argument_product_v1(state.len(), 2)?)?;
    let count = state
        .iter()
        .filter(|live| **live == Live::Descendant(workgroup))
        .count();
    if count >= fe2o3_kernel_ir::MAX_VALUE_ARGUMENTS_V1 {
        return Err(execution_lifecycle_error_v29());
    }
    let mut values = emission_vec_v1(count, budget)?;
    for (slot, live) in slots.iter().zip(state.iter_mut()) {
        if *live == Live::Descendant(workgroup) {
            values.push(slot.0);
            *live = Live::Absent;
        }
    }
    state[workgroup] = Live::Absent;
    state[context] = Live::Context;
    Ok(values)
}

fn prepare_terminal_scope_ends_v18(
    row: &mut PreparedTerminalFailureV18,
    slots: &[(ValueId, fe2o3_kernel_ir::ExecutionRoleV15)],
    state: &mut [LifecycleLiveV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if row.visited || !row.operations.is_empty() {
        return Err(terminal_failure_error_v18());
    }
    budget.charge_work(state.len())?;
    let count = state
        .iter()
        .filter(|live| matches!(live, LifecycleLiveV18::Workgroup(_)))
        .count();
    row.operations = emission_vec_v1(count, budget)?;
    row.witness.scope_ends = u32::try_from(count).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    for workgroup in 0..state.len() {
        budget.charge_work(1)?;
        if !matches!(state[workgroup], LifecycleLiveV18::Workgroup(_)) {
            continue;
        }
        let discarded = lifecycle_scope_discard_v18(slots, state, workgroup, budget)?;
        budget.charge_work(argument_sum_v1(&[discarded.len(), 1])?)?;
        let operation = fe2o3_kernel_ir::ExecutionOperationV15::ScopeEnd {
            workgroup: slots[workgroup].0,
            discarded,
        };
        operation
            .validate_payload()
            .map_err(|_| terminal_failure_error_v18())?;
        row.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Execution(operation),
        ));
    }
    budget.charge_work(state.len())?;
    if state
        .iter()
        .any(|live| !matches!(live, LifecycleLiveV18::Absent | LifecycleLiveV18::Context))
    {
        return Err(terminal_failure_error_v18());
    }
    row.visited = true;
    Ok(())
}

fn terminal_failure_operation_storage_v18(
    rows: &[PreparedTerminalFailureV18],
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let mut storage = 0;
    for row in rows {
        storage = argument_sum_v1(&[
            storage,
            argument_product_v1(row.operations.capacity(), size_of::<Operation>())?,
        ])?;
    }
    Ok(storage)
}
