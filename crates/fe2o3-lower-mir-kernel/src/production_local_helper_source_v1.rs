fn unit_local_vec_v1<T>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    let bytes = argument_product_v1(count, std::mem::size_of::<T>())?;
    budget.reserve_storage(bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    let extra = rows
        .capacity()
        .checked_sub(count)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.reserve_storage(argument_product_v1(extra, std::mem::size_of::<T>())?)?;
    Ok(rows)
}

fn unit_local_push_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let index = rows.len();
    rows.push(row);
    Ok(index)
}

fn unit_local_mismatch_v1() -> ProductionSemanticKirErrorV1 {
    ProductionSemanticKirErrorV1::CorrespondenceMismatch
}

impl SealedUnitLocalSourceV1 {
    fn find_association(
        &self,
        key: UnitLocalAssociationKeyV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        assert_origin_find_v1(&self.associations, budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.key.key().cmp(&key.key()))
        })
        .map_err(call_index_error_v1)
    }

    fn append_value(
        &mut self,
        row: UnitLocalValueRowV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        unit_local_push_v1(&mut self.values, row, budget)
    }

    fn record_call(
        &mut self,
        association: usize,
        row: UnitLocalCallRowV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let target = self
            .associations
            .get_mut(association)
            .ok_or_else(unit_local_mismatch_v1)?;
        if row.callee_association != association || row.return_control != target.return_control {
            return Err(unit_local_mismatch_v1());
        }
        let next = target
            .call_count
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        unit_local_push_v1(&mut self.calls, row, budget)?;
        target.call_count = next;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct UnitLocalPlannedAssociationV1 {
    key: UnitLocalAssociationKeyV1,
    group: usize,
    body: usize,
}

fn check_unit_local_source_v1(
    subject: CanonicalCallSubjectV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    origins: &SealedAssertOriginsV1,
    physical: UnitLocalPhysicalRowsV1<'_>,
    groups: &[CanonicalCallGroupV1<'_>],
    calls: &[CanonicalCallBindingV1<'_>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SealedUnitLocalSourceV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    let ledger = budget.work_ledger_identity_v1();
    let incoming = budget.storage();
    if !inventory.belongs_to(subject.executable) {
        return Err(unit_local_mismatch_v1());
    }
    budget.charge_work(physical.functions.len())?;
    let body_count = physical
        .functions
        .iter()
        .filter(|row| matches!(row, RetainedHelperKindV1::Local { .. }))
        .count();
    if body_count == 0 {
        return Ok(SealedUnitLocalSourceV1::empty());
    }
    let capture = subject
        .semantic_ssa
        .occurrences_v1()
        .ok_or_else(unit_local_mismatch_v1)?;
    budget.reserve_storage(std::mem::size_of::<Vec<UnitLocalPlannedAssociationV1>>())?;
    let mut planned = unit_local_vec_v1(groups.len(), budget)?;
    let mut rows = SealedUnitLocalSourceV1::empty();
    rows.bodies = unit_local_vec_v1(body_count, budget)?;
    for (ordinal, kind) in physical.functions.iter().enumerate() {
        budget.charge_work(1)?;
        if let RetainedHelperKindV1::Local {
            allocations,
            accesses,
            control,
            edge_bindings,
        } = *kind
        {
            budget.charge_work(5)?;
            if allocations.0 > allocations.1
                || allocations.1 > physical.allocations.len()
                || accesses.0 > accesses.1
                || accesses.1 > physical.accesses.len()
                || control.0 > control.1
                || control.1 > physical.control.len()
                || edge_bindings.0 > edge_bindings.1
                || edge_bindings.1 > physical.edge_bindings.len()
            {
                return Err(unit_local_mismatch_v1());
            }
            unit_local_push_v1(
                &mut rows.bodies,
                UnitLocalBodyRowV1 {
                    physical: ordinal,
                    allocations,
                    accesses,
                    control,
                    edge_bindings,
                },
                budget,
            )?;
        }
    }
    let mut values = 0;
    let mut memory = 0;
    let mut controls = 0;
    for (group_index, group) in groups.iter().enumerate() {
        budget.charge_work(4)?;
        let instance = group.function.source;
        let ordinal = group.function.canonical.coordinate.0 as usize;
        if !matches!(
            physical.functions.get(ordinal),
            Some(RetainedHelperKindV1::Local { .. })
        ) {
            continue;
        }
        if instance.role != SemanticKirFunctionRoleV1::InternalHelper {
            return Err(unit_local_mismatch_v1());
        }
        let key = UnitLocalAssociationKeyV1 {
            root: instance.correspondence_owner,
            function: instance.semantic_function,
            physical: ordinal,
        };
        let body = assert_origin_find_v1(&rows.bodies, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.physical.cmp(&ordinal))
        })
        .map_err(call_index_error_v1)?
        .ok_or_else(unit_local_mismatch_v1)?;
        let source = subject
            .semantic_ssa
            .source_semantic()
            .functions()
            .get(key.function.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        let observed = capture
            .function(key.function)
            .ok_or_else(unit_local_mismatch_v1)?;
        let native = group
            .function
            .canonical
            .function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?;
        let mut operations = 0;
        let mut statements = 0;
        let mut source_parameters = 0;
        let plan = subject
            .semantic_ssa
            .plan_for_function(key.function)
            .ok_or_else(unit_local_mismatch_v1)?;
        budget.charge_work(argument_sum_v1(&[
            native.blocks.len(),
            source.blocks().len(),
        ])?)?;
        for block in &native.blocks {
            operations =
                argument_sum_v1(&[operations, block.operations.len(), block.parameters.len()])?;
        }
        for (index, block) in source.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            statements = argument_sum_v1(&[statements, block.statements().len()])?;
            let block = fe2o3_mir_model::SsaBlockIdV1::new(
                u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            source_parameters = argument_sum_v1(&[
                source_parameters,
                plan.plan()
                    .transport_variables(block)
                    .map_or(0, |variables| variables.len()),
            ])?;
        }
        budget.charge_work(10)?;
        let base = argument_sum_v1(&[
            observed.events().len(),
            observed.constants().len(),
            observed.edge_definitions().len(),
            operations,
            source_parameters,
            statements,
            source.locals().len(),
            source.blocks().len(),
            1,
        ])?;
        values = argument_sum_v1(&[values, argument_product_v1(base, 8)?])?;
        let range = rows.bodies[body];
        memory = argument_sum_v1(&[
            memory,
            range.allocations.1 - range.allocations.0,
            range.accesses.1 - range.accesses.0,
            observed.events().len(),
            statements,
        ])?;
        controls = argument_sum_v1(&[controls, range.control.1 - range.control.0])?;
        unit_local_push_v1(
            &mut planned,
            UnitLocalPlannedAssociationV1 {
                key,
                group: group_index,
                body,
            },
            budget,
        )?;
    }
    assert_origin_sort_v1(&mut planned, budget, |left, right, budget| {
        budget.charge_work(3)?;
        Ok(left.key.key().cmp(&right.key.key()))
    })
    .map_err(call_index_error_v1)?;
    budget.charge_work(argument_product_v1(planned.len(), 2)?)?;
    if planned.is_empty()
        || planned.windows(2).any(|pair| {
            (pair[0].key.root, pair[0].key.function) == (pair[1].key.root, pair[1].key.function)
        })
    {
        return Err(unit_local_mismatch_v1());
    }
    // Each exact call may carry any number of source-only ignored Unit bindings.
    for call in calls {
        budget.charge_work(2)?;
        let callee = groups.get(call.callee).ok_or_else(unit_local_mismatch_v1)?;
        if !matches!(
            physical
                .functions
                .get(callee.function.canonical.coordinate.0 as usize),
            Some(RetainedHelperKindV1::Local { .. })
        ) {
            continue;
        }
        budget.charge_work(5)?;
        let key = call.site.caller;
        let plan = subject
            .semantic_ssa
            .plan_for_function(key.semantic_function)
            .ok_or_else(unit_local_mismatch_v1)?;
        let edge = fe2o3_mir_model::SsaEdgeIdV1::new(
            fe2o3_mir_model::SsaBlockIdV1::new(call.site.anchor.semantic_block.index()),
            0,
        );
        let arguments = plan
            .plan()
            .edge_arguments(edge)
            .ok_or_else(unit_local_mismatch_v1)?;
        values = argument_sum_v1(&[values, arguments.len(), 1])?;
    }
    rows.associations = unit_local_vec_v1(planned.len(), budget)?;
    rows.values = unit_local_vec_v1(values, budget)?;
    rows.memory = unit_local_vec_v1(memory, budget)?;
    rows.control = unit_local_vec_v1(controls, budget)?;
    rows.calls = unit_local_vec_v1(calls.len(), budget)?;
    for selected in &planned {
        budget.charge_work(9)?;
        let group = &groups[selected.group];
        let source = &subject.semantic_ssa.source_semantic().functions()
            [selected.key.function.index() as usize];
        let plan = subject
            .semantic_ssa
            .plan_for_function(selected.key.function)
            .ok_or_else(unit_local_mismatch_v1)?;
        if plan.function_identity() != source.identity() {
            return Err(unit_local_mismatch_v1());
        }
        let (unit_return_local, unit_type) = check_unit_local_unit_signature_v1(
            selected.key.function,
            subject.semantic_ssa.source_semantic().types(),
            source,
            group.function.canonical.function,
            budget,
        )?;
        let input = UnitLocalAssociationInputV1 {
            association: rows.associations.len(),
            key: selected.key,
            subject,
            origins,
            source,
            plan,
            occurrences: capture
                .function(selected.key.function)
                .ok_or_else(unit_local_mismatch_v1)?,
            physical_function: group.function.canonical.function,
            physical,
            body: rows.bodies[selected.body],
            unit_return_local,
            unit_type,
        };
        let starts = (rows.values.len(), rows.memory.len(), rows.control.len());
        let return_control = check_unit_local_association_v1(&input, &mut rows, budget)?;
        let row = UnitLocalAssociationRowV1 {
            key: selected.key,
            source_identity: source.identity(),
            plan_identity: plan.plan().identity(),
            body: selected.body,
            values: (starts.0, rows.values.len()),
            memory: (starts.1, rows.memory.len()),
            control: (starts.2, rows.control.len()),
            unit_return_local,
            unit_type,
            return_control,
            call_count: 0,
        };
        unit_local_push_v1(&mut rows.associations, row, budget)?;
    }
    check_unit_local_calls_v1(subject, inventory, groups, calls, &mut rows, budget)?;
    let scratch = argument_sum_v1(&[
        std::mem::size_of::<Vec<UnitLocalPlannedAssociationV1>>(),
        argument_product_v1(
            planned.capacity(),
            std::mem::size_of::<UnitLocalPlannedAssociationV1>(),
        )?,
    ])?;
    drop(planned);
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < incoming {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.release_storage(scratch)?;
    Ok(rows)
}

fn unit_local_filled_v1<T: Copy>(
    count: usize,
    value: T,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    let mut rows = unit_local_vec_v1(count, budget)?;
    budget.charge_work(count)?;
    rows.resize(count, value);
    Ok(rows)
}

fn unit_local_source_find_v1(
    rows: &[UnitLocalSourceIndexV1],
    key: [u64; 7],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    assert_origin_find_v1(rows, budget, |row, budget| {
        budget.charge_work(7)?;
        Ok(row.key.cmp(&key))
    })
    .map_err(call_index_error_v1)?
    .ok_or_else(unit_local_mismatch_v1)
}

fn unit_local_source_sort_v1(
    rows: &mut [UnitLocalSourceIndexV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_origin_sort_v1(rows, budget, |left, right, budget| {
        budget.charge_work(7)?;
        Ok(left.key.cmp(&right.key))
    })
    .map_err(call_index_error_v1)?;
    budget.charge_work(argument_product_v1(rows.len(), 7)?)?;
    if rows.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(unit_local_mismatch_v1());
    }
    Ok(())
}

fn check_unit_local_association_v1(
    input: &UnitLocalAssociationInputV1<'_>,
    rows: &mut SealedUnitLocalSourceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(std::mem::size_of::<SourceLocalCursorV1<'_, '_>>())?;
    let mut cursor = SourceLocalCursorV1::new(input, rows, budget)?;
    cursor.initialize_memory(budget)?;
    let mut current = input.source.entry();
    let return_control = loop {
        budget.charge_work(4)?;
        let visited = cursor
            .visited
            .get_mut(current.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        if *visited {
            return Err(unit_local_mismatch_v1());
        }
        *visited = true;
        let block = input
            .source
            .blocks()
            .get(current.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        for (ordinal, statement) in block.statements().iter().enumerate() {
            let ordinal = u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            cursor.begin_statement(current, ordinal, budget)?;
            cursor.check_statement(current, ordinal, statement, budget)?;
            cursor.finish_span(budget)?;
        }
        let span = *cursor.terminator_span(current, budget)?;
        cursor.begin_span(
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
            budget,
        )?;
        let step = check_unit_local_control_v1(input, current, &mut cursor, budget)?;
        cursor.finish_span(budget)?;
        match step {
            UnitLocalControlStepV1::Return { control } => break control,
            UnitLocalControlStepV1::Continue {
                target,
                staged_values,
            } => {
                cursor.commit_edge_bindings(target, staged_values, budget)?;
                current = target;
            }
        }
    };
    cursor.finish_memory(budget)?;
    cursor.finish_source(budget)?;
    let storage = cursor.storage_bytes()?;
    drop(cursor);
    if ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.release_storage(storage)?;
    Ok(return_control)
}

impl<'a, 'r> SourceLocalCursorV1<'a, 'r> {
    fn new(
        input: &'a UnitLocalAssociationInputV1<'a>,
        rows: &'r mut SealedUnitLocalSourceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(7)?;
        let body = input
            .physical_function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?;
        if !input.occurrences.entry_definitions().is_empty()
            || !input.plan.plan().entry_definitions().is_empty()
            || !input.plan.plan().entry_arguments().is_empty()
            || !input.occurrences.elisions().is_empty()
            || !input.occurrences.edge_definitions().is_empty()
        {
            return Err(unit_local_mismatch_v1());
        }
        let mut operations = 0;
        let mut definitions = 0;
        let mut statements = 0;
        budget.charge_work(argument_product_v1(
            input.subject.correspondence.statement_operation_spans.len(),
            2,
        )?)?;
        for span in &input.subject.correspondence.statement_operation_spans {
            if span.correspondence_owner == input.key.root
                && span.semantic_function == input.key.function
            {
                statements = argument_sum_v1(&[statements, 1])?;
            }
        }
        budget.charge_work(body.blocks.len())?;
        for block in &body.blocks {
            operations = argument_sum_v1(&[operations, block.operations.len()])?;
            definitions = argument_sum_v1(&[definitions, block.parameters.len()])?;
            budget.charge_work(block.operations.len())?;
            for op in &block.operations {
                definitions = argument_sum_v1(&[definitions, op.results.len()])?;
            }
        }
        let mut result = Self {
            input,
            rows,
            blocks: unit_local_vec_v1(body.blocks.len(), budget)?,
            definitions: unit_local_vec_v1(definitions, budget)?,
            event_index: unit_local_vec_v1(input.occurrences.events().len(), budget)?,
            constant_index: unit_local_vec_v1(input.occurrences.constants().len(), budget)?,
            statement_index: unit_local_vec_v1(statements, budget)?,
            locals: unit_local_filled_v1(
                input.source.locals().len(),
                UnitLocalLocalStateV1::default(),
                budget,
            )?,
            operation_claims: unit_local_filled_v1(operations, false, budget)?,
            event_claims: unit_local_filled_v1(input.occurrences.events().len(), false, budget)?,
            constant_claims: unit_local_filled_v1(
                input.occurrences.constants().len(),
                false,
                budget,
            )?,
            statement_claims: unit_local_filled_v1(statements, false, budget)?,
            visited: unit_local_filled_v1(input.source.blocks().len(), false, budget)?,
            memory: UnitLocalMemoryStateV1::empty(),
            core_control: input.body.control.0,
            core_edge: input.body.edge_bindings.0,
            native_span: None,
            staged: None,
        };
        let mut operation_base = 0;
        for (ordinal, block) in body.blocks.iter().enumerate() {
            budget.charge_work(2)?;
            unit_local_push_v1(
                &mut result.blocks,
                UnitLocalBlockIndexV1 {
                    id: block.id,
                    ordinal,
                    operation_base,
                    terminator_span: None,
                    synthetic_span: None,
                },
                budget,
            )?;
            operation_base = argument_sum_v1(&[operation_base, block.operations.len()])?;
            for (position, value) in block.parameters.iter().enumerate() {
                unit_local_push_v1(
                    &mut result.definitions,
                    UnitLocalDefinitionV1 {
                        value: value.id,
                        block: ordinal,
                        operation: None,
                        result: position,
                    },
                    budget,
                )?;
            }
            for (operation, op) in block.operations.iter().enumerate() {
                budget.charge_work(1)?;
                for (position, value) in op.results.iter().enumerate() {
                    unit_local_push_v1(
                        &mut result.definitions,
                        UnitLocalDefinitionV1 {
                            value: value.id,
                            block: ordinal,
                            operation: Some(operation),
                            result: position,
                        },
                        budget,
                    )?;
                }
            }
        }
        assert_origin_sort_v1(&mut result.blocks, budget, |left, right, budget| {
            budget.charge_work(1)?;
            Ok(left.id.cmp(&right.id))
        })
        .map_err(call_index_error_v1)?;
        assert_origin_sort_v1(&mut result.definitions, budget, |left, right, budget| {
            budget.charge_work(1)?;
            Ok(left.value.cmp(&right.value))
        })
        .map_err(call_index_error_v1)?;
        budget.charge_work(argument_sum_v1(&[
            result.blocks.len(),
            result.definitions.len(),
        ])?)?;
        if result.blocks.windows(2).any(|p| p[0].id == p[1].id)
            || result
                .definitions
                .windows(2)
                .any(|p| p[0].value == p[1].value)
        {
            return Err(unit_local_mismatch_v1());
        }
        for (index, event) in input.occurrences.events().iter().enumerate() {
            budget.charge_work(7)?;
            unit_local_push_v1(
                &mut result.event_index,
                UnitLocalSourceIndexV1 {
                    key: unit_local_source_key_v1(
                        event.site(),
                        event.operand(),
                        Some(event.role()),
                    ),
                    index,
                },
                budget,
            )?;
        }
        for (index, constant) in input.occurrences.constants().iter().enumerate() {
            budget.charge_work(7)?;
            unit_local_push_v1(
                &mut result.constant_index,
                UnitLocalSourceIndexV1 {
                    key: unit_local_source_key_v1(constant.site(), constant.operand(), None),
                    index,
                },
                budget,
            )?;
        }
        for (index, span) in input
            .subject
            .correspondence
            .statement_operation_spans
            .iter()
            .enumerate()
        {
            budget.charge_work(2)?;
            if span.correspondence_owner != input.key.root
                || span.semantic_function != input.key.function
            {
                continue;
            }
            unit_local_push_v1(
                &mut result.statement_index,
                UnitLocalSourceIndexV1 {
                    key: [
                        u64::from(span.semantic_block.index()),
                        u64::from(span.statement_ordinal),
                        0,
                        0,
                        0,
                        0,
                        0,
                    ],
                    index,
                },
                budget,
            )?;
        }
        unit_local_source_sort_v1(&mut result.event_index, budget)?;
        unit_local_source_sort_v1(&mut result.constant_index, budget)?;
        unit_local_source_sort_v1(&mut result.statement_index, budget)?;
        for (index, span) in input
            .subject
            .correspondence
            .terminator_operation_spans
            .iter()
            .enumerate()
        {
            budget.charge_work(2)?;
            if span.correspondence_owner != input.key.root
                || span.semantic_function != input.key.function
            {
                continue;
            }
            let found = assert_origin_find_v1(&result.blocks, budget, |row, budget| {
                budget.charge_work(1)?;
                Ok(row.id.cmp(&span.kernel_ir_block))
            })
            .map_err(call_index_error_v1)?
            .ok_or_else(unit_local_mismatch_v1)?;
            budget.charge_work(2)?;
            if span.kernel_ir_block != BlockId(span.semantic_block.index())
                || result.blocks[found]
                    .terminator_span
                    .replace(index)
                    .is_some()
            {
                return Err(unit_local_mismatch_v1());
            }
        }
        for (index, span) in input
            .subject
            .correspondence
            .synthetic_operation_spans
            .iter()
            .enumerate()
        {
            budget.charge_work(3)?;
            if span.correspondence_owner != input.key.root
                || span.semantic_function != input.key.function
            {
                continue;
            }
            match span.rule {
                SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage => {}
                SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap => {
                    let found = assert_origin_find_v1(&result.blocks, budget, |row, budget| {
                        budget.charge_work(1)?;
                        Ok(row.id.cmp(&span.kernel_ir_block))
                    })
                    .map_err(call_index_error_v1)?
                    .ok_or_else(unit_local_mismatch_v1)?;
                    if result.blocks[found].synthetic_span.replace(index).is_some() {
                        return Err(unit_local_mismatch_v1());
                    }
                }
                _ => return Err(unit_local_mismatch_v1()),
            }
        }
        Ok(result)
    }

    fn storage_bytes(&self) -> Result<usize, ProductionSemanticKirErrorV1> {
        argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            argument_product_v1(
                self.blocks.capacity(),
                std::mem::size_of::<UnitLocalBlockIndexV1>(),
            )?,
            argument_product_v1(
                self.definitions.capacity(),
                std::mem::size_of::<UnitLocalDefinitionV1>(),
            )?,
            argument_product_v1(
                self.event_index.capacity(),
                std::mem::size_of::<UnitLocalSourceIndexV1>(),
            )?,
            argument_product_v1(
                self.constant_index.capacity(),
                std::mem::size_of::<UnitLocalSourceIndexV1>(),
            )?,
            argument_product_v1(
                self.statement_index.capacity(),
                std::mem::size_of::<UnitLocalSourceIndexV1>(),
            )?,
            argument_product_v1(
                self.locals.capacity(),
                std::mem::size_of::<UnitLocalLocalStateV1>(),
            )?,
            self.operation_claims.capacity(),
            self.event_claims.capacity(),
            self.constant_claims.capacity(),
            self.statement_claims.capacity(),
            self.visited.capacity(),
            self.memory.storage_bytes()?,
        ])
        .map_err(Into::into)
    }

    fn begin_span(
        &mut self,
        block: BlockId,
        first: u32,
        count: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let (ordinal, body) = self.physical_block(block, budget)?;
        budget.charge_work(3)?;
        let end = argument_sum_v1(&[first as usize, count as usize])?;
        if end > body.operations.len() || self.native_span.is_some() {
            return Err(unit_local_mismatch_v1());
        }
        self.native_span = Some((ordinal, first as usize, end));
        Ok(())
    }

    fn finish_span(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let (_, next, end) = self.native_span.take().ok_or_else(unit_local_mismatch_v1)?;
        if next != end {
            return Err(unit_local_mismatch_v1());
        }
        Ok(())
    }

    fn begin_statement(
        &mut self,
        block: SemanticBlockIdV1,
        statement: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let index = unit_local_source_find_v1(
            &self.statement_index,
            [
                u64::from(block.index()),
                u64::from(statement),
                0,
                0,
                0,
                0,
                0,
            ],
            budget,
        )?;
        if self.statement_claims[index] {
            return Err(unit_local_mismatch_v1());
        }
        self.statement_claims[index] = true;
        let row = self.input.subject.correspondence.statement_operation_spans
            [self.statement_index[index].index];
        if row.kernel_ir_block != BlockId(block.index()) {
            return Err(unit_local_mismatch_v1());
        }
        self.begin_span(
            row.kernel_ir_block,
            row.first_operation_ordinal,
            row.operation_count,
            budget,
        )
    }

    fn statement_span(
        &self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a SemanticKirStatementOperationSpanV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
            site
        else {
            return Err(unit_local_mismatch_v1());
        };
        let index = unit_local_source_find_v1(
            &self.statement_index,
            [u64::from(block.get()), u64::from(statement), 0, 0, 0, 0, 0],
            budget,
        )?;
        Ok(&self.input.subject.correspondence.statement_operation_spans
            [self.statement_index[index].index])
    }

    fn finish_source(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(argument_sum_v1(&[
            self.operation_claims.len(),
            self.event_claims.len(),
            self.constant_claims.len(),
            self.visited.len(),
            self.statement_claims.len(),
            self.blocks.len(),
            4,
        ])?)?;
        if self.operation_claims.iter().any(|v| !v)
            || self.event_claims.iter().any(|v| !v)
            || self.constant_claims.iter().any(|v| !v)
            || self.visited.iter().any(|v| !v)
            || self.statement_claims.iter().any(|v| !v)
            || self.native_span.is_some()
            || self.staged.is_some()
            || self.core_control != self.input.body.control.1
            || self.core_edge != self.input.body.edge_bindings.1
        {
            return Err(unit_local_mismatch_v1());
        }
        for block in &self.blocks {
            if block.terminator_span.is_none() && block.synthetic_span.is_none() {
                return Err(unit_local_mismatch_v1());
            }
        }
        Ok(())
    }
}

/// Borrowed source/N helper relations for one exact live inventory and owner.
/// These relations do not approve ranked, output, artifact or launch consumers.
pub struct ProductionUnitLocalSourceV1<'a> {
    rows: &'a SealedUnitLocalSourceV1,
    inventory: &'a CanonicalKirInventoryV1<'a>,
    ledger: usize,
    work_ledger: ArgumentLedgerV1,
    floor: usize,
}

/// One source association; copied coordinates and counts are inert diagnostics.
pub struct ProductionUnitLocalAssociationV1<'a> {
    row: &'a UnitLocalAssociationRowV1,
}

impl ProductionUnitLocalAssociationV1<'_> {
    /// Original selected root, not a physical function ordinal.
    pub fn root(&self) -> SemanticFunctionIdV1 {
        self.row.key.root
    }
    /// Original source helper function.
    pub fn function(&self) -> SemanticFunctionIdV1 {
        self.row.key.function
    }
    /// Exact final module function ordinal in the borrowed inventory.
    pub fn physical_function(&self) -> usize {
        self.row.key.physical
    }
    /// Number of retained source/native value relations for this association.
    pub fn value_count(&self) -> usize {
        self.row.values.1 - self.row.values.0
    }
    /// Number of allocation, access and source invalidation relations.
    pub fn memory_count(&self) -> usize {
        self.row.memory.1 - self.row.memory.0
    }
    /// Number of retained selected-control and inactive-sink relations.
    pub fn control_count(&self) -> usize {
        self.row.control.1 - self.row.control.0
    }
    /// Number of actual root-qualified calls to this association.
    pub fn call_count(&self) -> usize {
        self.row.call_count
    }
}

impl ProductionUnitLocalSourceV1<'_> {
    /// True only for the exact inventory borrowed by this scope.
    pub fn belongs_to(&self, inventory: &CanonicalKirInventoryV1<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    /// Number of independently checked root-qualified helper associations.
    pub fn association_count(&self) -> usize {
        self.rows.associations.len()
    }
    /// Looks up one source helper under the original live budget and full floor.
    pub fn association(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionUnitLocalAssociationV1<'_>>, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let found = assert_origin_find_v1(&self.rows.associations, budget, |row, budget| {
            budget.charge_work(2)?;
            Ok((row.key.root.index(), row.key.function.index())
                .cmp(&(root.index(), function.index())))
        })
        .map_err(call_index_error_v1)?;
        Ok(found.map(|index| ProductionUnitLocalAssociationV1 {
            row: &self.rows.associations[index],
        }))
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Borrows sealed source/N helper relations without advancing any consumer gate.
    /// Keep the existing owner/inventory reservations and any preexisting source
    /// occurrence capture reserved. Numeric floors do not prove allocation custody.
    /// Caller-owned output escaping through R or captured state must be reserved
    /// before entry; callback scratch must be dropped before return. The scope
    /// accounts its own header, not arbitrary escaping allocations.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1,
    ///     inventory: &CanonicalKirInventoryV1<'_>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ///     root: SemanticFunctionIdV1, function: SemanticFunctionIdV1) {
    ///     let mut saved = None;
    ///     owner.with_checked_unit_local_source_v1(inventory, budget, |view, budget| {
    ///         saved = view.association(root, function, budget)?;
    ///         Ok(())
    ///     }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn with_checked_unit_local_source_v1<'w, R>(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        budget: &mut ArgumentBudgetV1<'w>,
        use_view: impl for<'s> FnOnce(
            &ProductionUnitLocalSourceV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        self.with_checked_helper_memory_v1(inventory, budget, |_, budget| {
            budget.charge_work(8)?;
            let entry = budget.storage();
            let work_ledger = budget.work_ledger_identity_v1();
            let minimum = argument_sum_v1(&[
                self.retained_analysis_storage_v1(),
                self.helper_memory.capture.preexisting_storage(),
            ])?;
            if entry < minimum {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.reserve_storage(std::mem::size_of::<ProductionUnitLocalSourceV1<'_>>())?;
            let floor = budget.storage();
            let view = ProductionUnitLocalSourceV1 {
                rows: &self.helper_memory.unit_source,
                inventory,
                ledger: budget as *const ArgumentBudgetV1<'_> as usize,
                work_ledger,
                floor,
            };
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| use_view(&view, budget)));
            if work_ledger != budget.work_ledger_identity_v1() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let imbalance = budget.storage() != floor;
            let release = budget
                .storage()
                .checked_sub(entry)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.release_storage(release)?;
            if imbalance {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }
}
