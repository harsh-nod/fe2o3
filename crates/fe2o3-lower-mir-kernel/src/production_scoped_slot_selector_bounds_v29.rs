use super::*;

#[derive(Clone, Copy)]
struct Guard {
    block: BlockId,
    index: ValueId,
    scalar: ScalarType,
}

struct Query {
    selected: usize,
    guard: BlockId,
    access: BlockId,
    checked: bool,
}

fn unsigned_max(scalar: ScalarType) -> Option<u64> {
    match scalar {
        ScalarType::U8 => Some(u64::from(u8::MAX)),
        ScalarType::U16 => Some(u64::from(u16::MAX)),
        ScalarType::U32 => Some(u64::from(u32::MAX)),
        ScalarType::U64 | ScalarType::Index => Some(u64::MAX),
        _ => None,
    }
}

fn physical_constant(
    context: &Context<'_, '_, '_, '_>,
    graph: &SlotUseGraphV29<'_>,
    definitions: &[Option<PrivateArrayPhysicalLocationV1>],
    value: ValueId,
    scalar: ScalarType,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<Option<u64>> {
    source_selector_unsigned_constant_v29(
        value, scalar, graph.index.values.len(), budget,
        |value, budget| Ok((graph.ty(value, budget)?,
            context.definition(graph, definitions, value, budget)?)),
    )
}

fn local_bound(
    context: &Context<'_, '_, '_, '_>,
    row: SourceReferenceHistorySelectorV29,
    graph: &SlotUseGraphV29<'_>,
    definitions: &[Option<PrivateArrayPhysicalLocationV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<bool> {
    budget.charge_work(3)?;
    let maximum = unsigned_max(row.scalar).ok_or_else(scoped_slot_error_v29)?;
    if maximum < row.source.length {
        return Ok(true);
    }
    if let Some(value) = physical_constant(context, graph, definitions, row.original, row.scalar, budget)? {
        return Ok(value < row.source.length);
    }
    let Some(operation) = context.definition(graph, definitions, row.original, budget)? else {
        return Ok(false);
    };
    if operation.results.len() != 1
        || operation.results[0].id != row.original
        || operation.results[0].ty != Type::Scalar(row.scalar)
    {
        return Err(scoped_slot_error_v29());
    }
    let OperationKind::Binary { op, lhs, rhs } = operation.kind else {
        return Ok(false);
    };
    if graph.ty(lhs, budget)? != &Type::Scalar(row.scalar)
        || graph.ty(rhs, budget)? != &Type::Scalar(row.scalar)
    {
        return Err(scoped_slot_error_v29());
    }
    match op {
        BinaryOp::BitAnd => {
            let left = physical_constant(context, graph, definitions, lhs, row.scalar, budget)?;
            let right = physical_constant(context, graph, definitions, rhs, row.scalar, budget)?;
            Ok(left
                .into_iter()
                .chain(right)
                .any(|mask| mask < row.source.length))
        }
        BinaryOp::Remainder => {
            Ok(
                physical_constant(context, graph, definitions, rhs, row.scalar, budget)?
                    .is_some_and(|divisor| divisor != 0 && divisor <= row.source.length),
            )
        }
        _ => Ok(false),
    }
}

fn semantic_constant(
    types: &[SemanticTypeDeclV1],
    operand: &SemanticOperandV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<Option<u64>> {
    budget.charge_work(4)?;
    let SemanticOperandV1::Constant(constant) = operand else {
        return Ok(None);
    };
    let Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed: false,
        bits,
    })) = types
        .get(constant.ty().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(None);
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Ok(None);
    };
    if !matches!(*bits, 8 | 16 | 32 | 64) || u16::from(value.size_bytes()) * 8 != *bits {
        return Ok(None);
    }
    Ok(u64::try_from(value.bits()).ok())
}

fn normalized_guard_index(
    context: &Context<'_, '_, '_, '_>,
    row: SourceReferenceHistorySelectorV29,
    guard: Guard,
    graph: &SlotUseGraphV29<'_>,
    definitions: &[Option<PrivateArrayPhysicalLocationV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<bool> {
    budget.charge_work(3)?;
    if guard.index == row.original && guard.scalar == row.scalar {
        return Ok(true);
    }
    if guard.scalar != ScalarType::Index {
        return Ok(false);
    }
    let path =
        plan_integer_cast_v1(row.scalar, ScalarType::Index).ok_or_else(scoped_slot_error_v29)?;
    let mut value = guard.index;
    for (kind, scalar) in path.into_iter().flatten().rev() {
        budget.charge_work(4)?;
        let Some(operation) = context.definition(graph, definitions, value, budget)? else {
            return Ok(false);
        };
        let OperationKind::Cast {
            kind: actual,
            value: input,
            ref to,
        } = operation.kind
        else {
            return Ok(false);
        };
        if actual != kind
            || to != &Type::Scalar(scalar)
            || operation.results.len() != 1
            || operation.results[0].id != value
            || operation.results[0].ty != Type::Scalar(scalar)
        {
            return Ok(false);
        }
        value = input;
    }
    Ok(value == row.original)
}

type Guards = BTreeMap<(SsaValueV1, u64), Vec<Guard>>;

fn guards(
    context: &Context<'_, '_, '_, '_>,
    graph: &SlotUseGraphV29<'_>,
    definitions: &[Option<PrivateArrayPhysicalLocationV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<Guards> {
    let plan = context.source.proof.plan;
    let lowered = context.source.lowered;
    let instance = lowered
        .source_call_instance
        .ok_or_else(scoped_slot_error_v29)?;
    let row = plan
        .instances
        .instance(instance)
        .ok_or_else(scoped_slot_error_v29)?;
    let function = row.declaration();
    let occurrences = plan
        .instances
        .occurrences(instance)
        .ok_or_else(scoped_slot_error_v29)?;
    let types = plan.instances.owner().source_semantic().types();
    let capture = lowered
        .instance_assert_origins
        .as_ref()
        .ok_or_else(|| invalid("scoped symbolic index lacks an original array-bound guard"))?;
    capture.check_identity(plan.instances, instance, budget)?;
    let inventory = SourceDescriptorInventoryV29::build(&occurrences, budget)?;
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<Guards>(),
        std::mem::size_of::<Result<Guards, ProductionSemanticKirErrorV1>>(),
    ])?)?;
    let mut guards = BTreeMap::new();
    for recorded in &capture.records {
        budget.charge_work(14)?;
        let block = recorded.site.semantic_block.index();
        let source = function
            .blocks()
            .get(block as usize)
            .ok_or_else(scoped_slot_error_v29)?;
        let SemanticTerminatorKindV1::Assert {
            expected: true,
            message: SemanticAssertMessageV1::BoundsCheck { length, .. },
            target,
            unwind,
            ..
        } = source.terminator().kind()
        else {
            continue;
        };
        if matches!(unwind, SemanticUnwindActionV1::Cleanup(_)) {
            continue;
        }
        let Some(extent) = semantic_constant(types, length, budget)? else {
            continue;
        };
        let site = ExecutionSiteV29::Terminator {
            block: SsaBlockIdV1::new(block),
        };
        let Some(condition_event) = inventory.use_at(
            &occurrences,
            site,
            ExecutionOperandV29::AssertCondition,
            budget,
        )?
        else {
            continue;
        };
        let Some(index_event) = inventory.use_at(
            &occurrences,
            site,
            ExecutionOperandV29::AssertMessage(1),
            budget,
        )?
        else {
            continue;
        };
        if source_descriptor_whole_operand_v29(function, site, ExecutionOperandV29::AssertCondition)
            .is_none()
            || source_descriptor_whole_operand_v29(
                function,
                site,
                ExecutionOperandV29::AssertMessage(1),
            )
            .is_none()
        {
            continue;
        }
        let condition = source_descriptor_use_v29(
            &occurrences,
            condition_event,
            site,
            ExecutionOperandV29::AssertCondition,
            budget,
        )?;
        let index = source_descriptor_use_v29(
            &occurrences,
            index_event,
            site,
            ExecutionOperandV29::AssertMessage(1),
            budget,
        )?;
        let Some(comparison_event) = inventory.definition(condition, budget)? else {
            continue;
        };
        let (comparison_site, comparison) = source_descriptor_assignment_v29(
            function,
            &occurrences,
            comparison_event,
            condition,
            budget,
        )?;
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            right,
            ..
        } = comparison
        else {
            continue;
        };
        if semantic_constant(types, right, budget)? != Some(extent)
            || source_descriptor_whole_operand_v29(
                function,
                comparison_site,
                ExecutionOperandV29::RvalueOperand(0),
            )
            .is_none()
        {
            continue;
        }
        let Some(comparison_index) = inventory.use_at(
            &occurrences,
            comparison_site,
            ExecutionOperandV29::RvalueOperand(0),
            budget,
        )?
        else {
            continue;
        };
        if source_descriptor_use_v29(
            &occurrences,
            comparison_index,
            comparison_site,
            ExecutionOperandV29::RvalueOperand(0),
            budget,
        )? != index
        {
            continue;
        }
        let PendingAssertOutcomeV1::Emitted {
            condition: physical,
            failure,
        } = recorded.outcome
        else {
            continue;
        };
        let mapped = context.source_block(block, budget)?;
        let success = context.source_block(target.target().index(), budget)?;
        assert_origin_string_work_v1(
            &recorded.emitted_function,
            lowered.function.id.as_str(),
            budget,
        )?;
        if recorded.site.correspondence_owner != capture.source.root
            || recorded.site.semantic_function != row.function()
            || !recorded.expected
            || recorded.block != mapped
            || recorded.semantic_success != target.target()
            || recorded.physical_success != success
            || recorded.emitted_function != lowered.function.id.as_str()
            || success == failure
        {
            return Err(invalid(
                "scoped array-bound guard changed its actual source instance",
            ));
        }
        let actual = graph.target(mapped, budget)?;
        if !matches!(actual.terminator.as_ref(), Some(Terminator::ConditionalBranch {
            condition, then_target, else_target, ..
        }) if *condition == physical && *then_target == success && *else_target == failure)
        {
            return Err(invalid(
                "scoped array-bound guard changed its emitted success edge",
            ));
        }
        if graph.ty(physical, budget)? != &Type::BOOL {
            return Err(scoped_slot_error_v29());
        }
        let Some(operation) = context.definition(graph, definitions, physical, budget)? else {
            continue;
        };
        let OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } = operation.kind
        else {
            continue;
        };
        let Type::Scalar(scalar) = graph.ty(lhs, budget)? else {
            continue;
        };
        if unsigned_max(*scalar).is_none()
            || physical_constant(context, graph, definitions, rhs, *scalar, budget)? != Some(extent)
        {
            continue;
        }
        let location =
            definitions[graph.value(physical, budget)?].ok_or_else(scoped_slot_error_v29)?;
        let (source_block, statement) = scoped_memory_site_key_v29(comparison_site);
        charge_execution_cfg_lookup_v29(context.spans.len(), budget)?;
        let spans = context
            .spans
            .get(&(source_block, statement, location.block))
            .ok_or_else(scoped_memory_error_v29)?;
        budget.charge_work(spans.len())?;
        if spans
            .iter()
            .filter(|span| span.contains(&location.operation))
            .count()
            != 1
        {
            return Err(invalid(
                "scoped array-bound comparison left its original source span",
            ));
        }
        charge_execution_cfg_lookup_v29(guards.len(), budget)?;
        if !guards.contains_key(&(index, extent)) {
            reserve_execution_cfg_map_entry_v29::<(SsaValueV1, u64), Vec<Guard>>(
                guards.len(),
                budget,
            )?;
            guards.insert((index, extent), Vec::new());
        }
        emission_push_v1(
            guards
                .get_mut(&(index, extent))
                .ok_or_else(scoped_slot_error_v29)?,
            Guard {
                block: mapped,
                index: lhs,
                scalar: *scalar,
            },
            budget,
        )?;
    }
    Ok(guards)
}

pub(super) fn check(
    context: &mut Context<'_, '_, '_, '_>,
    graph: &SlotUseGraphV29<'_>,
    definitions: &[Option<PrivateArrayPhysicalLocationV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    let mut pending = 0;
    for index in 0..context.selected.len() {
        budget.charge_work(1)?;
        if matches!(context.row(index).source.value, SourceReferenceSelectorValueV29::Retained { .. }) {
            return Err(invalid("retained source index requires immutable memory-version admission"));
        }
        let checked = local_bound(context, *context.row(index), graph, definitions, budget)?;
        context.selected[index].bounded = checked;
        if !checked {
            pending = argument_sum_v1(&[pending, 1])?;
        }
    }
    if pending == 0 {
        return Ok(());
    }
    // All arrays and callbacks stay in the enclosing owned history scratch.
    // This scope owns no source/C2 backing and performs no detached refund.
    let guards = guards(context, graph, definitions, budget)?;
    let mut capacity = 0;
    for (index, selected) in context.selected.iter().enumerate() {
        budget.charge_work(1)?;
        if selected.bounded {
            continue;
        }
        let row = context.row(index);
        let SourceReferenceSelectorValueV29::Promoted(value) = row.source.value else {
            return Err(scoped_slot_error_v29());
        };
        charge_execution_cfg_lookup_v29(guards.len(), budget)?;
        capacity = argument_sum_v1(&[
            capacity,
            guards
                .get(&(value, row.source.length))
                .map_or(0, Vec::len),
        ])?;
    }
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<Vec<Query>>(),
        std::mem::size_of::<Result<Vec<Query>, ProductionSemanticKirErrorV1>>(),
    ])?)?;
    let mut queries = emission_vec_v1(capacity, budget)?;
    for (index, selected) in context.selected.iter().enumerate() {
        budget.charge_work(1)?;
        if selected.bounded {
            continue;
        }
        let row = *context.row(index);
        let SourceReferenceSelectorValueV29::Promoted(value) = row.source.value else {
            return Err(scoped_slot_error_v29());
        };
        charge_execution_cfg_lookup_v29(guards.len(), budget)?;
        let Some(candidates) = guards.get(&(value, row.source.length)) else {
            continue;
        };
        for &guard in candidates {
            budget.charge_work(1)?;
            if normalized_guard_index(context, row, guard, graph, definitions, budget)? {
                queries.push(Query {
                    selected: index,
                    guard: guard.block,
                    access: row.block,
                    checked: false,
                });
            }
        }
    }
    if !queries.is_empty() {
        fe2o3_kernel_ir::with_function_control_flow_v1(
            &context.source.lowered.function,
            Default::default(),
            budget,
            |view| {
                for query in &mut queries {
                    query.checked = view.success_edge_dominates(query.guard, 0, query.access)?;
                }
                Ok(())
            },
        )
        .map_err(|error| match error {
            fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => {
                ProductionSemanticKirErrorV1::from(error)
            }
            _ => invalid("scoped array-bound guard has invalid raw CFG coordinates"),
        })?;
    }
    for query in &queries {
        budget.charge_work(1)?;
        context.selected[query.selected].bounded |= query.checked;
    }
    budget.charge_work(context.selected.len())?;
    if context.selected.iter().any(|row| !row.bounded) {
        return Err(invalid(
            "scoped symbolic index lacks an exact current bound and success-edge proof",
        ));
    }
    Ok(())
}
