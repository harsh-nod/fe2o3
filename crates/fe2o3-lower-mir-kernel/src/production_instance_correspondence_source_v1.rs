// Validate the exact reachable source census, including zero-operation rows.
// Synthetic prologues/failure blocks never impersonate source statements.
fn instance_check_source_rows_v1(
    instance: &production_call_instances_v1::ProductionCallInstanceV1<'_>,
    owner: SemanticFunctionIdV1,
    lowered: &LoweredFunctionResultV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> InstanceMapResultV1<()> {
    budget.charge_work(1)?;
    let function = instance.function();
    let source = instance.declaration();
    let order = instance.ssa().plan().reverse_postorder();
    let body = lowered
        .function
        .body
        .as_ref()
        .ok_or(InstanceCorrespondenceErrorV1::Source)?;
    if lowered.blocks.len() != order.len()
        || lowered.terminator_operation_spans.len() != order.len()
        || body.blocks.len() < order.len()
    {
        return Err(InstanceCorrespondenceErrorV1::Source);
    }
    budget.charge_work(order.len())?;
    let mut statements = 0_usize;
    let mut synthetic = 0_usize;
    for (index, block) in order.iter().enumerate() {
        let semantic_block = SemanticBlockIdV1::from_index(block.get());
        let source_block = source
            .blocks()
            .get(block.get() as usize)
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        let row = &lowered.blocks[index];
        if row.correspondence_owner != owner
            || row.semantic_function != function
            || row.semantic_block != semantic_block
            || row.source_statement_count as usize != source_block.statements().len()
            || row.kernel_ir_block != body.blocks[index].id
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let terminator = &lowered.terminator_operation_spans[index];
        if terminator.correspondence_owner != owner
            || terminator.semantic_function != function
            || terminator.semantic_block != semantic_block
            || terminator.kernel_ir_block != row.kernel_ir_block
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        // lower_one emits retained slots, then enum slots, then each source
        // statement and the terminator. Sorting later must not hide a swap.
        let mut cursor = 0_u32;
        let mut retained = false;
        let mut enums = false;
        while let Some(span) = lowered.synthetic_operation_spans.get(synthetic) {
            budget.charge_work(1)?;
            if span.kernel_ir_block != row.kernel_ir_block {
                break;
            }
            if span.correspondence_owner != owner || span.semantic_function != function {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            match span.rule {
                SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage
                    if !retained && !enums =>
                {
                    retained = true
                }
                SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage if !enums => enums = true,
                _ => return Err(InstanceCorrespondenceErrorV1::Source),
            }
            if span.first_operation_ordinal != cursor || span.operation_count == 0 {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            cursor = cursor
                .checked_add(span.operation_count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            synthetic += 1;
        }
        let end = statements
            .checked_add(source_block.statements().len())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let spans = lowered
            .statement_operation_spans
            .get(statements..end)
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        budget.charge_work(spans.len())?;
        for (ordinal, statement) in spans.iter().enumerate() {
            if statement.correspondence_owner != owner
                || statement.semantic_function != function
                || statement.semantic_block != semantic_block
                || statement.statement_ordinal as usize != ordinal
                || statement.kernel_ir_block != row.kernel_ir_block
                || statement.first_operation_ordinal != cursor
            {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            cursor = cursor
                .checked_add(statement.operation_count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        if terminator.first_operation_ordinal != cursor {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        cursor = cursor
            .checked_add(terminator.operation_count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if cursor as usize != body.blocks[index].operations.len() {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        statements = end;
    }
    if statements != lowered.statement_operation_spans.len() {
        return Err(InstanceCorrespondenceErrorV1::Source);
    }
    let failures = if let Some(span) = lowered.synthetic_operation_spans.get(synthetic) {
        budget.charge_work(1)?;
        if span.correspondence_owner != owner
            || span.semantic_function != function
            || span.rule != SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let block = body
            .blocks
            .get(order.len())
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        if span.kernel_ir_block != block.id
            || span.first_operation_ordinal != 0
            || span.operation_count != 1
            || block.operations.len() != 1
            || !matches!(block.terminator, Some(Terminator::Unreachable))
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        synthetic += 1;
        1
    } else {
        0
    };
    if synthetic != lowered.synthetic_operation_spans.len()
        || body.blocks.len()
            != order
                .len()
                .checked_add(failures)
                .ok_or(ArgumentResourceV1::Arithmetic)?
    {
        return Err(InstanceCorrespondenceErrorV1::Source);
    }
    Ok(())
}
