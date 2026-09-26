#[cfg(test)]
#[path = "production_execution_tile_producer_v29_tests.rs"]
mod tile_producer_tests;

// Retained tile calls are still inert. Ordinary execution needs a separately
// checked schedule expansion and the existing final production admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeferredTileInputV29 {
    Load {
        workgroup: SemanticExecutionIdentityV29,
        input: ValueId,
        base: ValueId,
    },
    Fragment {
        tile: SemanticExecutionIdentityV29,
    },
    Parts {
        fragment: SemanticExecutionIdentityV29,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeferredTileEventV29 {
    input: DeferredTileInputV29,
    producer: ProductionCallOccurrenceV1,
    result_type: SemanticTypeIdV1,
    first_result: ValueId,
    lanes: u16,
    elements: u16,
}

fn is_execution_tile_operation_v29(operation: SemanticExecutionOperationV29) -> bool {
    matches!(
        operation,
        SemanticExecutionOperationV29::MaskedTileLoadU32 { .. }
            | SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 { .. }
            | SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 { .. }
    )
}

impl DeferredTileEventV29 {
    fn result_range(self) -> Result<std::ops::Range<u32>, ProductionSemanticKirErrorV1> {
        fe2o3_kernel_ir::ExecutionRoleV15::MaskedTileU32 {
            lanes: self.lanes,
            elements: self.elements,
        }
        .validate()
        .map_err(|_| execution_lifecycle_error_v29())?;
        let count = if matches!(self.input, DeferredTileInputV29::Parts { .. }) {
            u32::from(self.elements)
                .checked_mul(2)
                .ok_or(ArgumentResourceV1::Arithmetic)?
        } else {
            1
        };
        let end = self
            .first_result
            .0
            .checked_add(count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(self.first_result.0..end)
    }

    fn result_type(self, index: usize) -> Type {
        use fe2o3_kernel_ir::ExecutionRoleV15 as Role;
        match self.input {
            DeferredTileInputV29::Load { .. } => Type::Execution(Role::MaskedTileU32 {
                lanes: self.lanes,
                elements: self.elements,
            }),
            DeferredTileInputV29::Fragment { .. } => Type::Execution(Role::LaneFragmentU32 {
                lanes: self.lanes,
                elements: self.elements,
            }),
            DeferredTileInputV29::Parts { .. } => {
                Type::Scalar(if index < usize::from(self.elements) {
                    ScalarType::U32
                } else {
                    ScalarType::Bool
                })
            }
        }
    }

    fn definitions(
        self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Vec<ValueDef>, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let range = self.result_range()?;
        let mut definitions = emission_vec_v1(range.len(), budget)?;
        budget.charge_work(range.len())?;
        for (index, value) in range.enumerate() {
            definitions.push(ValueDef::new(ValueId(value), self.result_type(index)));
        }
        Ok(definitions)
    }

    fn operation(
        self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Operation, ProductionSemanticKirErrorV1> {
        use fe2o3_kernel_ir::ExecutionOperationV15 as Op;
        let kind = match self.input {
            DeferredTileInputV29::Load {
                workgroup,
                input,
                base,
            } => Op::MaskedTileLoadU32 {
                workgroup: workgroup.value,
                input,
                base,
                lanes: self.lanes,
                elements: self.elements,
            },
            DeferredTileInputV29::Fragment { tile } => Op::TileIntoFragmentU32 {
                tile: tile.value,
                lanes: self.lanes,
                elements: self.elements,
            },
            DeferredTileInputV29::Parts { fragment } => Op::FragmentIntoPartsU32 {
                fragment: fragment.value,
                lanes: self.lanes,
                elements: self.elements,
            },
        };
        Ok(Operation::new(
            self.definitions(budget)?,
            OperationKind::Execution(kind),
        ))
    }

    fn source_operation(self) -> SemanticExecutionOperationV29 {
        match self.input {
            DeferredTileInputV29::Load { workgroup, .. } => {
                SemanticExecutionOperationV29::MaskedTileLoadU32 {
                    workgroup: workgroup.semantic_type,
                    tile: self.result_type,
                }
            }
            DeferredTileInputV29::Fragment { tile } => {
                SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 {
                    tile: tile.semantic_type,
                    fragment: self.result_type,
                }
            }
            DeferredTileInputV29::Parts { fragment } => {
                SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 {
                    fragment: fragment.semantic_type,
                    parts: self.result_type,
                }
            }
        }
    }
}

fn prepare_tile_discards_v29(
    body: &fe2o3_kernel_ir::FunctionBody,
    events: &mut [PreparedLifecycleEventV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    prepare_lifecycle_ownership_v18(body, events, None, budget)
}

fn prepare_lifecycle_ownership_v18(
    body: &fe2o3_kernel_ir::FunctionBody,
    events: &mut [PreparedLifecycleEventV29],
    mut failures: Option<&mut PreparedTerminalFailuresV18>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use fe2o3_kernel_ir::{ExecutionOperationV15 as Op, ExecutionRoleV15 as Role};
    use LifecycleLiveV18 as Live;
    fn find<K: Ord, V>(
        rows: &[(K, V)],
        key: &K,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(rows.len().checked_ilog2().map_or(1, |n| n as usize + 2))?;
        rows.binary_search_by(|row| row.0.cmp(key))
            .map_err(|_| execution_lifecycle_error_v29())
    }
    fn result(
        results: &[ValueDef],
        role: Role,
        slots: &[(ValueId, Role)],
        state: &[Live],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        let [value] = results else {
            return Err(execution_lifecycle_error_v29());
        };
        if value.ty != Type::Execution(role) {
            return Err(execution_lifecycle_error_v29());
        }
        let index = find(slots, &value.id, budget)?;
        if slots[index].1 != role || state[index] != Live::Absent {
            return Err(execution_lifecycle_error_v29());
        }
        Ok(index)
    }
    if events.is_empty() && failures.is_none() {
        return Ok(());
    }
    let floor = budget.storage();
    let mut retained = 0;
    {
        let n = match &failures {
            Some(plan) => plan.block_count(body.blocks.len())?,
            None => body.blocks.len(),
        };
        if n == 0 {
            return Err(execution_lifecycle_error_v29());
        }
        let mut blocks = emission_vec_v1(n, budget)?;
        let mut ranges = emission_vec_v1(n, budget)?;
        let mut slots = emission_vec_v1(events.len(), budget)?;
        let mut cursor = 0;
        let mut issuances = 0;
        for (index, block) in body.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            blocks.push((block.id, index));
            let start = cursor;
            while cursor < events.len() && events[cursor].block == index {
                budget.charge_work(1)?;
                let event = &events[cursor];
                if event.witness.before.block != block.id
                    || event.witness.before.first as usize > block.operations.len()
                    || (cursor > start
                        && events[cursor - 1].witness.before.first > event.witness.before.first)
                {
                    return Err(execution_lifecycle_error_v29());
                }
                let operation = event
                    .operation
                    .as_ref()
                    .ok_or_else(execution_lifecycle_error_v29)?;
                let OperationKind::Execution(kind) = &operation.kind else {
                    return Err(execution_lifecycle_error_v29());
                };
                issuances =
                    argument_sum_v1(&[issuances, usize::from(matches!(kind, Op::ContextIssue))])?;
                budget.charge_work(operation.results.len())?;
                for value in &operation.results {
                    if let Type::Execution(role) = &value.ty {
                        emission_push_v1(&mut slots, (value.id, *role), budget)?;
                    }
                }
                cursor += 1;
            }
            ranges.push(start..cursor);
        }
        if let Some(plan) = &failures {
            for index in 0..plan.generated.len() {
                budget.charge_work(1)?;
                blocks.push((plan.generated_block(index)?, body.blocks.len() + index));
                ranges.push(cursor..cursor);
            }
        }
        if cursor != events.len() || (failures.is_none() && slots.is_empty())
            || (issuances != 1 && !(failures.is_some() && issuances == 0 && slots.is_empty())) {
            return Err(execution_lifecycle_error_v29());
        }
        for count in [blocks.len(), slots.len()] {
            call_splice_sort_work_v1(count, budget)
                .map_err(|error| pending_scope_correspondence_error_v29(error.into()))?;
        }
        blocks.sort_unstable_by_key(|row| row.0);
        slots.sort_unstable_by_key(|row| row.0);
        budget.charge_work(argument_sum_v1(&[blocks.len(), slots.len()])?)?;
        if blocks.windows(2).any(|p| p[0].0 == p[1].0) || slots.windows(2).any(|p| p[0].0 == p[1].0)
        {
            return Err(execution_lifecycle_error_v29());
        }
        let width = slots.len();
        let input_cells = argument_product_v1(n, width)?;
        let cells = argument_sum_v1(&[input_cells, width])?;
        let mut matrix = emission_vec_v1(cells, budget)?;
        let mut seen = emission_vec_v1(n, budget)?;
        let mut queue = emission_vec_v1(n, budget)?;
        budget.charge_work(argument_sum_v1(&[cells, n])?)?;
        matrix.resize(cells, Live::Absent);
        seen.resize(n, false);
        seen[0] = true;
        for block in &body.blocks {
            budget.charge_work(1)?;
            block
                .terminator
                .as_ref()
                .ok_or_else(execution_lifecycle_error_v29)?
                .try_visit_edges_v1(|target, _| {
                    budget.charge_work(1)?;
                    find(&blocks, &target, budget)?;
                    Ok::<_, ProductionSemanticKirErrorV1>(())
                })?;
        }
        // The first incoming edge fixes a block's complete ownership state.
        // Every later edge, including a backedge, must agree exactly. Each
        // reachable block is evaluated once, so discard payloads are never
        // widened, recomputed or allocated once per dynamic iteration.
        queue.push(0);
        let (incoming, state) = matrix.split_at_mut(input_cells);
        let mut visited = 0;
        while visited < queue.len() {
            budget.charge_work(1)?;
            let index = queue[visited];
            visited += 1;
            let block = body.blocks.get(index);
            let block_id = match block {
                Some(block) => block.id,
                None => failures.as_ref().ok_or_else(terminal_failure_error_v18)?
                    .generated_block(index - body.blocks.len())?,
            };
            budget.charge_work(width)?;
            state.copy_from_slice(&incoming[index * width..(index + 1) * width]);
            for event in &mut events[ranges[index].clone()] {
                budget.charge_work(1)?;
                let operation = event
                    .operation
                    .as_mut()
                    .ok_or_else(execution_lifecycle_error_v29)?;
                let Operation { results, kind } = operation;
                let OperationKind::Execution(kind) = kind else {
                    return Err(execution_lifecycle_error_v29());
                };
                let payload_work = match &*kind {
                    Op::ScopeEnd { discarded, .. } => argument_sum_v1(&[discarded.len(), 1])?,
                    _ => 1,
                };
                budget.charge_work(payload_work)?;
                if kind.validate_payload().is_err() {
                    return Err(execution_lifecycle_error_v29());
                }
                match kind {
                    Op::ContextIssue => {
                        let output = result(results, Role::Context, &slots, state, budget)?;
                        state[output] = Live::Context;
                    }
                    Op::WorkgroupDerive { context } => {
                        let context = find(&slots, context, budget)?;
                        if state[context] != Live::Context {
                            return Err(execution_lifecycle_error_v29());
                        }
                        let output = result(results, Role::Workgroup, &slots, state, budget)?;
                        state[context] = Live::Borrowed(output);
                        state[output] = Live::Workgroup(context);
                    }
                    Op::MaskedTileLoadU32 {
                        workgroup,
                        lanes,
                        elements,
                        ..
                    } => {
                        let workgroup = find(&slots, workgroup, budget)?;
                        if !matches!(state[workgroup], Live::Workgroup(_)) {
                            return Err(execution_lifecycle_error_v29());
                        }
                        let output = result(
                            results,
                            Role::MaskedTileU32 {
                                lanes: *lanes,
                                elements: *elements,
                            },
                            &slots,
                            state,
                            budget,
                        )?;
                        state[output] = Live::Descendant(workgroup);
                    }
                    Op::TileIntoFragmentU32 {
                        tile,
                        lanes,
                        elements,
                    } => {
                        let tile = find(&slots, tile, budget)?;
                        let Live::Descendant(workgroup) = state[tile] else {
                            return Err(execution_lifecycle_error_v29());
                        };
                        if slots[tile].1
                            != (Role::MaskedTileU32 {
                                lanes: *lanes,
                                elements: *elements,
                            })
                        {
                            return Err(execution_lifecycle_error_v29());
                        }
                        let output = result(
                            results,
                            Role::LaneFragmentU32 {
                                lanes: *lanes,
                                elements: *elements,
                            },
                            &slots,
                            state,
                            budget,
                        )?;
                        state[tile] = Live::Absent;
                        state[output] = Live::Descendant(workgroup);
                    }
                    Op::FragmentIntoPartsU32 {
                        fragment,
                        lanes,
                        elements,
                    } => {
                        let fragment = find(&slots, fragment, budget)?;
                        if slots[fragment].1
                            != (Role::LaneFragmentU32 {
                                lanes: *lanes,
                                elements: *elements,
                            })
                            || !matches!(state[fragment], Live::Descendant(_))
                        {
                            return Err(execution_lifecycle_error_v29());
                        }
                        let count = argument_product_v1(*elements as usize, 2)?;
                        budget.charge_work(results.len())?;
                        if results.len() != count
                            || results.iter().enumerate().any(|(i, value)| {
                                value.ty
                                    != Type::Scalar(if i < *elements as usize {
                                        ScalarType::U32
                                    } else {
                                        ScalarType::Bool
                                    })
                            })
                        {
                            return Err(execution_lifecycle_error_v29());
                        }
                        state[fragment] = Live::Absent;
                    }
                    Op::ScopeEnd {
                        workgroup,
                        discarded,
                    } => {
                        if !results.is_empty() || !discarded.is_empty() {
                            return Err(execution_lifecycle_error_v29());
                        }
                        let workgroup = find(&slots, workgroup, budget)?;
                        let before = budget.storage();
                        let values = lifecycle_scope_discard_v18(&slots, state, workgroup, budget)?;
                        retained = argument_sum_v1(&[
                            retained,
                            budget
                                .storage()
                                .checked_sub(before)
                                .ok_or(ArgumentResourceV1::Accounting)?,
                        ])?;
                        *discarded = values;
                        budget.charge_work(argument_sum_v1(&[discarded.len(), 1])?)?;
                        if kind.validate_payload().is_err() {
                            return Err(execution_lifecycle_error_v29());
                        }
                    }
                }
            }
            if let Some(plan) = failures.as_deref_mut() {
                if let Some(row) = plan.row_for_block(block_id, budget)? {
                    let before = budget.storage();
                    let failure = &mut plan.rows[row];
                    if let Some(block) = block {
                        budget.charge_work(ranges[index].len())?;
                        if failure.witness.generated || argument_sum_v1(&[failure.witness.original_gap as usize, 1])? != block.operations.len()
                            || events[ranges[index].clone()].iter().any(|event|
                                event.witness.before.first > failure.witness.original_gap) {
                            return Err(terminal_failure_error_v18());
                        }
                    }
                    prepare_terminal_scope_ends_v18(failure, &slots, state, budget)?;
                    retained = argument_sum_v1(&[retained,
                        budget.storage().checked_sub(before).ok_or(ArgumentResourceV1::Accounting)?])?;
                }
            }
            if matches!(
                block.and_then(|block| block.terminator.as_ref()),
                Some(Terminator::Return { .. } | Terminator::Unreachable)
            ) || block.is_none() {
                budget.charge_work(width)?;
                if state
                    .iter()
                    .any(|live| !matches!(live, Live::Absent | Live::Context))
                {
                    return Err(execution_lifecycle_error_v29());
                }
            }
            let Some(block) = block else { continue; };
            let mut successor = 0u32;
            block
                .terminator
                .as_ref()
                .ok_or_else(execution_lifecycle_error_v29)?
                .try_visit_edges_v1(|target, _| {
                    budget.charge_work(1)?;
                    let target = match &failures {
                        Some(plan) => plan.redirect(block.id, successor, target, budget)?,
                        None => target,
                    };
                    successor = successor.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                    let next = blocks[find(&blocks, &target, budget)?].1;
                    budget.charge_work(width)?;
                    let target = &mut incoming[next * width..(next + 1) * width];
                    if seen[next] {
                        if target[..] != state[..] {
                            return Err(execution_lifecycle_error_v29());
                        }
                    } else {
                        target.copy_from_slice(state);
                        seen[next] = true;
                        queue.push(next);
                    }
                    Ok::<_, ProductionSemanticKirErrorV1>(())
                })?;
        }
        budget.charge_work(n)?;
        if seen
            .iter()
            .zip(&ranges)
            .any(|(reachable, events)| !reachable && !events.is_empty())
        {
            return Err(execution_lifecycle_error_v29());
        }
        if let Some(plan) = &failures { plan.check_complete(budget)?; }
    }
    let scratch = budget
        .storage()
        .checked_sub(floor)
        .and_then(|bytes| bytes.checked_sub(retained))
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.release_storage(scratch)?;
    Ok(())
}

include!("production_execution_deferred_parts_v29.rs");

fn tile_geometry_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    fragment: bool,
) -> Result<(u16, u16), ProductionSemanticKirErrorV1> {
    let (_, role) =
        semantic_execution_type_v29(types, ty).map_err(|_| execution_lifecycle_error_v29())?;
    match role {
        SemanticExecutionRoleV29::MaskedTileU32 { lanes, elements } if !fragment => {
            Ok((lanes, elements))
        }
        SemanticExecutionRoleV29::LaneFragmentU32 { lanes, elements } if fragment => {
            Ok((lanes, elements))
        }
        _ => Err(execution_lifecycle_error_v29()),
    }
}

fn check_lifecycle_result_range_v29(
    range: std::ops::Range<u32>,
    body: &fe2o3_kernel_ir::FunctionBody,
    prepared: &[PreparedLifecycleEventV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[
        body.parameters.len(),
        body.blocks.len(),
        prepared.len(),
    ])?)?;
    if range.is_empty() || body.parameters.iter().any(|id| range.contains(&id.0)) {
        return Err(execution_lifecycle_error_v29());
    }
    for prior in prepared {
        let operation = prior
            .operation
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        budget.charge_work(operation.results.len())?;
        if operation
            .results
            .iter()
            .any(|def| range.contains(&def.id.0))
        {
            return Err(execution_lifecycle_error_v29());
        }
    }
    for block in &body.blocks {
        budget.charge_work(argument_sum_v1(&[
            block.parameters.len(),
            block.operations.len(),
        ])?)?;
        if block.parameters.iter().any(|def| range.contains(&def.id.0)) {
            return Err(execution_lifecycle_error_v29());
        }
        for operation in &block.operations {
            budget.charge_work(operation.results.len())?;
            if operation
                .results
                .iter()
                .any(|def| range.contains(&def.id.0))
            {
                return Err(execution_lifecycle_error_v29());
            }
        }
    }
    Ok(())
}

impl ExecutionLifecycleProducerV29<'_> {
    fn produce_tile_v29(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        source: DeferredLifecycleSourceV29,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operation: SemanticExecutionOperationV29,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        if source
            != (DeferredLifecycleSourceV29::Intrinsic {
                callee: call.callee(),
            })
        {
            return Err(execution_lifecycle_error_v29());
        }
        let destination = call
            .destination()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let result_type = destination.place().ty();
        let occurrence = ProductionCallOccurrenceV1 {
            caller: self.pending.instance,
            block,
        };
        let (input, lanes, elements, binding) = match operation {
            SemanticExecutionOperationV29::MaskedTileLoadU32 { workgroup, tile } => {
                if call.arguments().len() != 3 || result_type != tile {
                    return Err(execution_lifecycle_error_v29());
                }
                let borrowed = lowering.lower_source_operand_v29(
                    block,
                    None,
                    Some(ExecutionOperandV29::CallArgument(0)),
                    &call.arguments()[0],
                    operations,
                )?;
                let SemanticValueBindingV1::ExecutionBorrow(borrowed) = borrowed else {
                    return Err(execution_lifecycle_error_v29());
                };
                if borrowed.kind != SemanticBorrowKindV1::Shared
                    || borrowed.borrowed.semantic_type() != workgroup
                {
                    return Err(execution_lifecycle_error_v29());
                }
                borrowed
                    .check_type(lowering.types, call.arguments()[0].ty())
                    .map_err(|_| execution_lifecycle_error_v29())?;
                let input = lowering.lower_source_operand_v29(
                    block,
                    None,
                    Some(ExecutionOperandV29::CallArgument(1)),
                    &call.arguments()[1],
                    operations,
                )?;
                let SemanticValueBindingV1::Value {
                    id: input,
                    ty: Type::Slice(slice),
                } = input
                else {
                    return Err(execution_lifecycle_error_v29());
                };
                if slice.address_space != AddressSpace::Global
                    || slice.access != AccessMode::ReadOnly
                    || *slice.element != Type::Scalar(ScalarType::U32)
                {
                    return Err(execution_lifecycle_error_v29());
                }
                let base = lowering.lower_source_operand_v29(
                    block,
                    None,
                    Some(ExecutionOperandV29::CallArgument(2)),
                    &call.arguments()[2],
                    operations,
                )?;
                let base = lowering.coerce_index(block, operations, base)?;
                let SemanticValueBindingV1::Value {
                    id: base,
                    ty: Type::INDEX,
                } = base
                else {
                    return Err(execution_lifecycle_error_v29());
                };
                let (lanes, elements) = tile_geometry_v29(lowering.types, tile, false)?;
                let binding = SemanticExecutionBindingV29::tile(
                    lowering.types,
                    tile,
                    occurrence,
                    ValueId(lowering.next_value),
                    &borrowed.borrowed,
                )
                .map_err(|_| execution_lifecycle_error_v29())?;
                (
                    DeferredTileInputV29::Load {
                        workgroup: borrowed.borrowed.identity,
                        input,
                        base,
                    },
                    lanes,
                    elements,
                    SemanticValueBindingV1::Execution(binding),
                )
            }
            SemanticExecutionOperationV29::MaskedTileIntoFragmentU32 { tile, fragment } => {
                if call.arguments().len() != 1 || result_type != fragment {
                    return Err(execution_lifecycle_error_v29());
                }
                let input = lowering.lower_source_operand_v29(
                    block,
                    None,
                    Some(ExecutionOperandV29::CallArgument(0)),
                    &call.arguments()[0],
                    operations,
                )?;
                let SemanticValueBindingV1::Execution(input) = input else {
                    return Err(execution_lifecycle_error_v29());
                };
                input
                    .check_type(lowering.types, tile)
                    .map_err(|_| execution_lifecycle_error_v29())?;
                let (lanes, elements) = tile_geometry_v29(lowering.types, tile, false)?;
                let binding = SemanticExecutionBindingV29::fragment(
                    lowering.types,
                    fragment,
                    occurrence,
                    ValueId(lowering.next_value),
                    &input,
                )
                .map_err(|_| execution_lifecycle_error_v29())?;
                (
                    DeferredTileInputV29::Fragment {
                        tile: input.identity,
                    },
                    lanes,
                    elements,
                    SemanticValueBindingV1::Execution(binding),
                )
            }
            SemanticExecutionOperationV29::LaneFragmentIntoPartsU32 { fragment, parts } => {
                if call.arguments().len() != 1 || result_type != parts {
                    return Err(execution_lifecycle_error_v29());
                }
                let input = lowering.lower_source_operand_v29(
                    block,
                    None,
                    Some(ExecutionOperandV29::CallArgument(0)),
                    &call.arguments()[0],
                    operations,
                )?;
                let SemanticValueBindingV1::Execution(input) = input else {
                    return Err(execution_lifecycle_error_v29());
                };
                input
                    .check_type(lowering.types, fragment)
                    .map_err(|_| execution_lifecycle_error_v29())?;
                let (lanes, elements) = tile_geometry_v29(lowering.types, fragment, true)?;
                let event = DeferredTileEventV29 {
                    input: DeferredTileInputV29::Parts {
                        fragment: input.identity,
                    },
                    producer: occurrence,
                    result_type,
                    first_result: ValueId(lowering.next_value),
                    lanes,
                    elements,
                };
                let budget = lowering
                    .emission_work
                    .as_deref_mut()
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let definitions = event.definitions(budget)?;
                let scratch = argument_product_v1(definitions.capacity(), size_of::<ValueDef>())?;
                let mut leaves = [].iter();
                let mut values = definitions.iter();
                let binding = rebuild_execution_cfg_binding_v29(
                    lowering.types,
                    parts,
                    true,
                    &mut leaves,
                    &mut values,
                    &mut 0,
                    budget,
                )?;
                if values.next().is_some() {
                    return Err(execution_lifecycle_error_v29());
                }
                drop(definitions);
                budget.release_storage(scratch)?;
                (event.input, lanes, elements, binding)
            }
            _ => return Err(execution_lifecycle_error_v29()),
        };
        let event = DeferredTileEventV29 {
            input,
            producer: occurrence,
            result_type,
            first_result: ValueId(lowering.next_value),
            lanes,
            elements,
        };
        let next_value = event.result_range()?.end;
        self.record(
            source,
            block,
            operations.len(),
            DeferredLifecycleKindV29::Tile(event),
        )?;
        lowering.next_value = next_value;
        Ok(binding)
    }
}
