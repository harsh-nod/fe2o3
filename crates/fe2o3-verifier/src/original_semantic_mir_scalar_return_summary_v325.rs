//! Local state equations for an owner-checked scalar helper return.

use super::*;
use fe2o3_kernel_ir::{OperationKind, Terminator as KirTerminator, Type};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    Unknown,
    Visiting,
    Unsupported,
    Cut { block: usize, hops: usize },
}

#[derive(Clone, Copy)]
enum Bridge {
    Cut,
    Next(usize),
    Unsupported,
}

fn bridge_shape(operations: usize, edges: usize, bindings: usize, branch: bool) -> bool {
    operations == 0 && edges == 1 && bindings == 0 && branch
}

pub(super) struct Routes<'a> {
    row: &'a Root,
    rows: Vec<Route>,
    required: usize,
}

pub(super) fn scan<'a>(
    model: &'a PairedInvocations<'_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Routes<'a>> {
    model.check(out)?;
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(1)?;
    let row = model.roots.get(root).ok_or_else(mismatch)?;
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    let describe = |current: usize, out: &mut Writer<'_, '_>| -> Result<Bridge> {
        out.budget.charge_work(4)?;
        if row.cuts.get(current).and_then(Option::as_ref).is_some() {
            return Ok(Bridge::Cut);
        }
        let target = inventory
            .blocks()
            .get(add(row.blocks.start, current)?)
            .ok_or_else(mismatch)?;
        if target.edges.len() != 1 {
            return Ok(Bridge::Unsupported);
        }
        let edge = inventory
            .edges()
            .get(target.edges.start)
            .ok_or_else(mismatch)?;
        if edge.target.function != target.coordinate.function {
            return Err(mismatch());
        }
        Ok(
            if bridge_shape(
                target.operations.len(),
                target.edges.len(),
                edge.bindings.len(),
                matches!(target.terminator, KirTerminator::Branch { .. }),
            ) {
                Bridge::Next(edge.target.block as usize)
            } else {
                Bridge::Unsupported
            },
        )
    };
    let rows = resolve_routes(row.blocks.len(), describe, out)?;
    Ok(Routes {
        row,
        rows,
        required: out.budget.storage(),
    })
}

fn resolve_routes<F>(count: usize, mut describe: F, out: &mut Writer<'_, '_>) -> Result<Vec<Route>>
where
    F: FnMut(usize, &mut Writer<'_, '_>) -> Result<Bridge>,
{
    out.budget
        .reserve_storage(headers() + size_of::<F>() + size_of::<Result<Bridge>>())?;
    let mut rows = vector(count, out)?;
    let mut path = vector(count, out)?;
    out.budget.charge_work(count)?;
    rows.resize(count, Route::Unknown);
    // Each block is discovered once and each bridge is popped once. Cached
    // terminal cuts are reused even when many return cuts share a bridge.
    for start in 0..count {
        out.budget.charge_work(1)?;
        if !matches!(rows[start], Route::Unknown) {
            continue;
        }
        let mut current = start;
        let mut result = loop {
            out.budget.charge_work(5)?;
            match rows[current] {
                Route::Unknown => {}
                Route::Visiting => break Route::Unsupported,
                prior => break prior,
            }
            let next = match describe(current, out)? {
                Bridge::Cut => {
                    rows[current] = Route::Cut {
                        block: current,
                        hops: 0,
                    };
                    break rows[current];
                }
                Bridge::Unsupported => {
                    rows[current] = Route::Unsupported;
                    break Route::Unsupported;
                }
                Bridge::Next(next) => next,
            };
            if next >= rows.len() {
                return Err(mismatch());
            }
            rows[current] = Route::Visiting;
            path.push(current);
            current = next;
        };
        while let Some(bridge) = path.pop() {
            out.budget.charge_work(1)?;
            result = match result {
                Route::Cut { block, hops } => Route::Cut {
                    block,
                    hops: add(hops, 1)?,
                },
                _ => Route::Unsupported,
            };
            rows[bridge] = result;
        }
    }
    Ok(rows)
}

pub(super) struct Summary<'a> {
    source: &'a SourceScalarReturnV325,
    instance: usize,
    target_pc: usize,
    target_next: usize,
    target_result: usize,
    target_fuel: usize,
    required: usize,
}

pub(super) fn derive<'a>(
    model: &PairedInvocations<'_, '_, '_>,
    scan: &root_returns::RootScan<'_>,
    routes: &Routes<'_>,
    root: usize,
    block: usize,
    hint: &'a SourceCutHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Summary<'a>>> {
    model
        .slots
        .check_query_storage_floor(routes.required, out.budget)?;
    model.check(out)?;
    let row = model.roots.get(root).ok_or_else(mismatch)?;
    if !std::ptr::eq(routes.row, row) {
        return Err(mismatch());
    }
    let allocation_free = scan.allocation_free(model, root, out)?;
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(12)?;
    let cut = row
        .cuts
        .get(block)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if hint.pc != cut.source || add(row.instances.start, hint.instance)? != cut.instance {
        return Err(mismatch());
    }
    let Some(source) = &hint.scalar_return_v325 else {
        return Ok(None);
    };
    let returned = &source.returned;
    if source.root != root
        || source.instance != hint.instance
        || source.pc != cut.source
        || source.statements.len() != hint.statements
    {
        return Err(mismatch());
    }
    if !allocation_free
        || !matches!(cut.end, End::Return)
        || hint.instance == 0
        || hint.call.is_some()
        || hint.operands != 0
        || returned.depth < 2
        || returned.locals.start >= returned.locals.end
        || returned.locals.end > model.locals
        || !returned.locals.contains(&returned.returned)
        || returned.destination >= returned.locals.start
        || returned.bits == 0
    {
        return Ok(None);
    }
    let instance = model
        .instances
        .get(cut.instance)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if instance.owners.len() != returned.depth || instance.owners.last() != Some(&returned.owner) {
        return Err(mismatch());
    }
    let Some(binding) = instance.returned else {
        return Ok(None);
    };
    let (SourceValue::Local(destination), Some(target_result), LogicalBinding::Plain) =
        (binding.source, binding.definition, binding.logical)
    else {
        return Ok(None);
    };
    if destination != returned.destination || binding.frame != returned.depth - 2 {
        return Err(mismatch());
    }
    for statement in &source.statements {
        out.budget.charge_work(3)?;
        if !returned.locals.contains(&statement.destination)
            || statement
                .inputs
                .iter()
                .flatten()
                .any(|input| !returned.locals.contains(input))
        {
            return Err(mismatch());
        }
    }
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    let result_type = inventory
        .definitions()
        .get(target_result)
        .ok_or_else(mismatch)?
        .ty;
    if scalar_width(result_type, model.width) != Some(returned.bits) {
        return Ok(None);
    }
    let target_pc = add(row.blocks.start, block)?;
    let first = inventory.blocks().get(target_pc).ok_or_else(mismatch)?;
    for operation in inventory
        .operations()
        .get(first.operations.clone())
        .ok_or_else(mismatch)?
    {
        out.budget.charge_work(2)?;
        if !matches!(
            operation.operation.kind,
            OperationKind::Constant(_)
                | OperationKind::Unary { .. }
                | OperationKind::Binary { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Cast { .. }
                | OperationKind::Select { .. }
        ) || operation.results.len() != 1
        {
            return Ok(None);
        }
        for definition in inventory
            .definitions()
            .get(operation.results.clone())
            .ok_or_else(mismatch)?
        {
            out.budget.charge_work(1)?;
            if !matches!(definition.ty, Type::Scalar(_)) {
                return Ok(None);
            }
        }
        for input in inventory
            .uses()
            .get(operation.operands.clone())
            .ok_or_else(mismatch)?
        {
            out.budget.charge_work(1)?;
            if !matches!(
                inventory
                    .definitions()
                    .get(input.definition)
                    .ok_or_else(mismatch)?
                    .ty,
                Type::Scalar(_)
            ) {
                return Ok(None);
            }
        }
    }
    if !matches!(first.terminator, KirTerminator::Branch { .. }) || first.edges.len() != 1 {
        return Ok(None);
    }
    let edge = inventory
        .edges()
        .get(first.edges.start)
        .ok_or_else(mismatch)?;
    if edge.target.function != first.coordinate.function {
        return Err(mismatch());
    }
    if edge.bindings.len() != 1 {
        return Ok(None);
    }
    let transfer = inventory
        .edge_arguments()
        .get(edge.bindings.start)
        .ok_or_else(mismatch)?;
    if transfer.target_definition != target_result {
        return Ok(None);
    }
    if inventory
        .definitions()
        .get(transfer.incoming_definition)
        .ok_or_else(mismatch)?
        .ty
        != result_type
    {
        return Err(mismatch());
    }
    let Some(Route::Cut {
        block: next_block,
        hops,
    }) = routes.rows.get(edge.target.block as usize).copied()
    else {
        return Ok(None);
    };
    let next = row
        .cuts
        .get(next_block)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let parent = model
        .instances
        .get(next.instance)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    out.budget.charge_work(returned.depth)?;
    if next.source != returned.continuation
        || parent.owners.as_slice() != &instance.owners[..returned.depth - 1]
    {
        return Ok(None);
    }
    Ok(Some(Summary {
        source,
        instance: hint.instance,
        target_pc,
        target_next: add(row.blocks.start, next_block)?,
        target_result,
        target_fuel: add(hops, 1)?,
        required: out.budget.storage(),
    }))
}

fn scalar_width(ty: &Type, width: FormalIndexWidth) -> Option<u32> {
    let Type::Scalar(scalar) = ty else {
        return None;
    };
    scalar.bit_width().map(u32::from).or(match width {
        FormalIndexWidth::Bits32 => Some(32),
        FormalIndexWidth::Bits64 => Some(64),
        FormalIndexWidth::Unknown => None,
    })
}

pub(super) fn emit(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    summary: &Summary<'_>,
    goal: Goal,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    model
        .slots
        .check_query_storage_floor(summary.required, out.budget)?;
    model.check(out)?;
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(40)?;
    let source = summary.source;
    let returned = &source.returned;
    let instance = summary.instance;
    let block = source.block;
    let count = source.statements.len();
    let source_fuel = add(count, 1)?;
    let target_fuel = summary.target_fuel;
    let pc = source.pc;
    let begin = returned.locals.start;
    let end = returned.locals.end;
    let result_local = returned.returned;
    let destination = returned.destination;
    let continuation = returned.continuation;
    let target_pc = summary.target_pc;
    let target_next = summary.target_next;
    let target_result = summary.target_result;
    emit!(
        out,
        " hide(invocation_paired_source_step_{root}_v36);\n hide(invocation_paired_actual_step_{root}_v36);\n hide(invocation_source_block_runtime_{root}_v36);\n hide(invocation_byte_boundary_{root}_v36);\n hide(invocation_paired_source_defined_{root}_v36);\n hide(invocation_source_byte_storage_related_{root}_v36);\n hide(invocation_source_byte_state_well_formed_v36);\n hide(invocation_byte_states_related_v36);\n hide(invocation_byte_heaps_related_v36);\n hide(byte_memory_well_formed_v30);\n hide(byte_frame_runtime_well_formed_v30);\n hide(byte_private_frames_live_v30);\n hide(private_generation_counters_valid_v30);\n hide(invocation_source_logical_write_v38);\n hide(invocation_source_logical_clear_v38);\n hide(invocation_source_return_v36);\n hide(byte_end_frame_v30);\n"
    );
    for statement in 0..count {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " hide(invocation_source_scalar_{root}_{instance}_{block}_{statement}_v36);\n"
        );
    }
    emit!(
        out,
        " let map = invocation_source_byte_map_{root}_v36(source, target);\n assert(invocation_source_byte_state_well_formed_v36(source) && invocation_byte_states_related_v36(source.machine, target, map) && invocation_byte_heaps_related_v36(source.machine.memory, target.memory, map) && source.machine.valid && target.valid) by {{\n  reveal(invocation_source_byte_storage_related_{root}_v36);\n  reveal(invocation_byte_states_related_v36);\n }}\n invocation_related_target_inputs_v96(source.machine, target, map);\n invocation_paired_source_defined_step_valid_{root}_v92(source);\n let little_endian = invocation_runtime_little_endian_v36();\n let original = invocation_paired_source_step_{root}_v36(source).state;\n let actual = invocation_paired_actual_step_{root}_v36(target).state;\n assert(source.machine.pc == {pc} && target.pc == {target_pc});\n let state_0 = source;\n"
    );
    for statement in 0..count {
        out.budget.charge_work(4)?;
        let next = add(statement, 1)?;
        emit!(
            out,
            " let state_{next} = invocation_source_scalar_{root}_{instance}_{block}_{statement}_v36(state_{statement});\n"
        );
    }
    emit!(
        out,
        " assert(original == invocation_source_return_{root}_{instance}_v36(state_{count}, little_endian).source) by {{\n  reveal(invocation_paired_source_step_{root}_v36);\n  reveal(invocation_source_block_runtime_{root}_v36);\n  reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {source_fuel});\n }}\n assert(state_{count}.machine.valid);\n"
    );
    for statement in (0..count).rev() {
        out.budget.charge_work(2)?;
        emit!(
            out,
            " assert(state_{statement}.machine.valid) by {{\n  reveal(invocation_source_scalar_{root}_{instance}_{block}_{statement}_v36);\n }}\n"
        );
    }
    for (statement, coordinates) in source.statements.iter().enumerate() {
        out.budget.charge_work(7)?;
        let next = add(statement, 1)?;
        let local = coordinates.destination;
        emit!(out, " let value_{statement} = ");
        if coordinates.unit {
            emit!(out, "MemoryValueV30::Unit;\n");
        } else {
            emit!(
                out,
                "MemoryValueV30::Scalar(original_invocation_source_scalar_trace_{}_v30(seq![",
                coordinates.graph
            );
            for input in coordinates.inputs.iter().flatten() {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    "match state_{statement}.machine.values[{input}] {{ MemoryValueV30::Scalar(v) => v, _ => 0int }},"
                );
            }
            emit!(out, "])[0]);\n");
        }
        emit!(
            out,
            " assert(state_{next} == invocation_source_byte_put_local_v36(state_{statement}, {local}, value_{statement})) by {{\n  reveal(invocation_source_scalar_{root}_{instance}_{block}_{statement}_v36);\n }}\n invocation_source_put_local_preserves_heap_v78(state_{statement}, {local}, value_{statement});\n"
        );
    }
    emit!(
        out,
        " let value = state_{count}.machine.values[{result_local}];\n let destination = Some(InvocationSourceReturnDestinationV42 {{ local: {destination}, component: None, memory: None, descriptor: None }});\n let returned = invocation_source_return_v36(state_{count}, {begin}, {end}, InvocationSourceValueV42::Carrier(value), destination, {continuation}, little_endian);\n assert(original == returned.source);\n assert(original.machine.valid);\n assert(invocation_source_byte_state_well_formed_v36(original) && original.machine.frames.execution == source.machine.frames.execution) by {{\n  reveal(invocation_source_return_v36);\n }}\n assert(actual.valid && actual.pc == {target_next} && actual.memory == target.memory && actual.generations == target.generations && actual.frames == target.frames && actual.values.len() == target.values.len()) by {{\n  reveal(invocation_paired_actual_step_{root}_v36);\n  reveal(invocation_byte_boundary_{root}_v36);\n  reveal_with_fuel(invocation_byte_follow_{root}_v36, {target_fuel});\n }}\n"
    );
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    emit!(
        out,
        " assert({{ let actual = actual.values[{target_result}]; "
    );
    emit_value_type(
        inventory
            .definitions()
            .get(target_result)
            .ok_or_else(mismatch)?
            .ty,
        model.width,
        "actual",
        out,
    )?;
    emit!(
        out,
        " }}) by {{\n  reveal(invocation_paired_actual_step_{root}_v36);\n  reveal(invocation_byte_boundary_{root}_v36);\n  reveal_with_fuel(invocation_byte_follow_{root}_v36, {target_fuel});\n }}\n"
    );
    match goal {
        Goal::Heap => {
            emit!(
                out,
                " assert(map.private == Map::<MemoryAllocationV30, InvocationByteBindingV36>::empty());\n assert(invocation_byte_heaps_related_v36(source.machine.memory, target.memory, map));\n invocation_empty_private_map_has_no_private_source_v78(source.machine.memory, target.memory, map);\n invocation_source_plain_return_preserves_heap_v78(state_{count}, {begin}, {end}, value, destination, {continuation}, little_endian);\n assert(original.machine.memory == source.machine.memory && original.machine.generations == source.machine.generations);\n assert(invocation_source_byte_map_{root}_v36(original, actual) == map);\n assert(invocation_byte_heaps_related_v36(original.machine.memory, actual.memory, map));\n assert(invocation_byte_states_related_v36(original.machine, actual, map)) by {{\n  reveal(invocation_source_byte_state_well_formed_v36);\n  reveal(invocation_byte_states_related_v36);\n }}\n"
            );
        }
        Goal::Control => {
            emit!(
                out,
                " assert(value == actual.values[{target_result}]) by {{\n  reveal(invocation_paired_actual_step_{root}_v36);\n  reveal(invocation_byte_boundary_{root}_v36);\n  reveal_with_fuel(invocation_byte_follow_{root}_v36, {target_fuel});\n }}\n assert(invocation_paired_control_values_{root}_v36(source, invocation_source_block_runtime_{root}_v36(source), invocation_byte_boundary_{root}_v36(target))) by {{\n  reveal(invocation_source_block_runtime_{root}_v36);\n  reveal(invocation_byte_boundary_{root}_v36);\n  reveal(invocation_source_return_v36);\n  reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {source_fuel});\n  reveal_with_fuel(invocation_byte_follow_{root}_v36, {target_fuel});\n }}\n"
            );
        }
        Goal::Halted => {
            emit!(
                out,
                " assert(original.machine.pc == {continuation}) by {{ reveal(invocation_source_return_v36); }}\n assert(!invocation_paired_source_step_{root}_v36(source).halted && !invocation_paired_actual_step_{root}_v36(target).halted) by {{\n  reveal(invocation_paired_source_step_{root}_v36);\n  reveal(invocation_paired_actual_step_{root}_v36);\n }}\n"
            );
        }
        _ => return Err(mismatch()),
    }
    Ok(())
}

fn headers() -> usize {
    size_of::<Summary<'_>>()
        + size_of::<Option<Summary<'_>>>()
        + size_of::<Result<Option<Summary<'_>>>>()
        + size_of::<Result<()>>()
        + size_of::<Route>()
        + size_of::<Option<Route>>()
        + size_of::<Bridge>()
        + size_of::<Result<Bridge>>()
        + size_of::<Routes<'_>>()
        + size_of::<Result<Routes<'_>>>()
        + size_of::<Vec<Route>>()
        + size_of::<Result<Vec<Route>>>()
        + size_of::<Vec<usize>>()
        + size_of::<Result<Vec<usize>>>()
        + size_of::<Option<usize>>()
        + size_of::<Binding>()
        + size_of::<Option<Binding>>()
        + size_of::<SourceValue>()
        + size_of::<LogicalBinding>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<std::slice::Iter<'_, ScalarCopyV325>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, ScalarCopyV325>>>()
        + size_of::<std::iter::Rev<std::ops::Range<usize>>>()
        + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>()
        + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>>()
        + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirUseRefV1>>()
        + size_of::<std::iter::Flatten<std::slice::Iter<'_, Option<usize>>>>()
        + 32 * size_of::<usize>()
        + 32 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_scalar_return_summary_v325_tests.rs"]
mod tests;
