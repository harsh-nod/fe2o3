//! Independent source/target summaries for authenticated descriptor-length cuts.

use super::*;
use fe2o3_kernel_ir::{OperationKind, Terminator as KirTerminator, Type};

pub(super) const SHARED: &str = include_str!("original_semantic_mir_cut_summary_laws_v96.vrs");

pub(super) struct Summary {
    target_pc: usize,
    target_next: usize,
    source_next: usize,
    input: usize,
}

pub(super) fn derive(
    model: &PairedInvocations<'_, '_, '_>,
    row: &Root,
    hints: &SourceStepHintsV85,
    block: usize,
    cut: &Cut,
    hint: &SourceCutHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Summary>> {
    // Revalidate custody before a tactic query incurs any new storage or work.
    model.check(out)?;
    let relation = model.slots.correspondence(out)?;
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(5)?;
    let Some(source_next) = hint.descriptor_continuation_v96() else {
        return Ok(None);
    };
    if hints.conserves_heap || !matches!(cut.end, End::Ordinary) {
        return Ok(None);
    }
    let inventory = relation.inventory(out.budget)?;
    let target_pc = add(row.blocks.start, block)?;
    let target = inventory.blocks().get(target_pc).ok_or_else(mismatch)?;
    let KirTerminator::Branch { arguments, .. } = target.terminator else {
        return Ok(None);
    };
    if !arguments.is_empty() || target.operations.len() != 1 || target.edges.len() != 1 {
        return Ok(None);
    }
    let operation = inventory
        .operations()
        .get(target.operations.start)
        .ok_or_else(mismatch)?;
    if !matches!(operation.operation.kind, OperationKind::SliceLength { .. })
        || operation.operands.len() != 1
        || operation.results.len() != 1
    {
        return Ok(None);
    }
    let input = inventory
        .uses()
        .get(operation.operands.start)
        .ok_or_else(mismatch)?
        .definition;
    if !matches!(
        inventory.definitions().get(input).ok_or_else(mismatch)?.ty,
        Type::Slice(_)
    ) {
        return Ok(None);
    }
    let edge = inventory
        .edges()
        .get(target.edges.start)
        .ok_or_else(mismatch)?;
    if edge.target.function != target.coordinate.function {
        return Err(mismatch());
    }
    let next_block = edge.target.block as usize;
    let Some(next) = row.cuts.get(next_block).and_then(Option::as_ref) else {
        return Ok(None);
    };
    if next.source != source_next || next.instance != cut.instance {
        return Ok(None);
    }
    let instance = model
        .instances
        .get(cut.instance)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if !instance.suspended.is_empty() {
        return Ok(None);
    }
    // This closed route obtains empty source slots/map from the existing
    // allocation census. Memory-bearing roots retain their original proofs.
    for block in inventory
        .blocks()
        .get(row.blocks.clone())
        .ok_or_else(mismatch)?
    {
        out.budget.charge_work(1)?;
        for operation in inventory
            .operations()
            .get(block.operations.clone())
            .ok_or_else(mismatch)?
        {
            out.budget.charge_work(1)?;
            if matches!(operation.operation.kind, OperationKind::Alloca { .. }) {
                return Ok(None);
            }
        }
    }
    let mut input_bound = false;
    for binding in &cut.live {
        out.budget.charge_work(1)?;
        input_bound |= binding.definition == Some(input)
            && matches!(binding.source, SourceValue::Local(_))
            && matches!(binding.logical, LogicalBinding::Plain);
    }
    if !input_bound {
        return Ok(None);
    }
    for binding in &next.live {
        out.budget.charge_work(2)?;
        let (SourceValue::Local(local), Some(definition), LogicalBinding::Plain) =
            (binding.source, binding.definition, binding.logical)
        else {
            return Ok(None);
        };
        if definition == operation.results.start {
            return Ok(None);
        }
        let mut retained = false;
        for before in &cut.live {
            out.budget.charge_work(1)?;
            retained |= matches!(before.source, SourceValue::Local(value) if value == local)
                && before.definition == Some(definition)
                && before.frame == binding.frame
                && matches!(before.logical, LogicalBinding::Plain);
        }
        if !retained {
            return Ok(None);
        }
    }
    Ok(Some(Summary {
        target_pc,
        target_next: add(row.blocks.start, next_block)?,
        source_next,
        input,
    }))
}

pub(super) fn emit(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    row: &Root,
    cut: &Cut,
    hint: &SourceCutHintsV85,
    summary: &Summary,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    model.check(out)?;
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(8)?;
    let pc = cut.source;
    let namespace = super::super::super::super::TARGET_TAG_NAMESPACE_V40;
    let next = row
        .cuts
        .get(
            summary
                .target_next
                .checked_sub(row.blocks.start)
                .ok_or_else(mismatch)?,
        )
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_source_cut_summary_{root}_{pc}_v96(source: InvocationSourceByteStateV36)\n requires source.machine.pc == {pc}, source.machine.valid, source.machine.values.len() == {},\n invocation_source_byte_state_well_formed_v36(source), source.slots.dom().is_empty(),\n invocation_paired_source_defined_{root}_v36(source, 1),\n ensures invocation_source_byte_state_well_formed_v36(invocation_paired_source_step_{root}_v36(source).state),\n invocation_source_frame_equal_v93(source, invocation_paired_source_step_{root}_v36(source).state),\n invocation_paired_source_step_{root}_v36(source).state.objects == source.objects,\n invocation_paired_source_step_{root}_v36(source).state.slots == source.slots,\n invocation_paired_source_step_{root}_v36(source).state.machine.valid,\n invocation_paired_source_step_{root}_v36(source).state.machine.pc == {},\n",
        model.locals,
        summary.source_next
    );
    for binding in &next.live {
        out.budget.charge_work(1)?;
        let SourceValue::Local(local) = binding.source else {
            return Err(mismatch());
        };
        emit!(
            out,
            " invocation_paired_source_step_{root}_v36(source).state.machine.values[{local}] == source.machine.values[{local}],\n"
        );
    }
    emit!(
        out,
        " invocation_paired_source_step_{root}_v36(source).events.len() == 0,\n invocation_source_block_runtime_{root}_v36(source).returned.is_none(),\n{{\n hide(invocation_source_descriptor_step_v51);\n hide(invocation_source_descriptor_length_v51);\n hide(invocation_source_pointer_step_v36);\n hide(invocation_source_byte_put_local_v36);\n hide(invocation_source_carrier_evaluate_v36);\n hide(invocation_paired_source_defined_{root}_v36);\n hide(byte_memory_well_formed_v30);\n hide(byte_frame_runtime_well_formed_v30);\n hide(byte_execution_well_formed_v37);\n hide(byte_private_frames_live_v30);\n hide(private_generation_counters_valid_v30);\n hide(invocation_source_byte_state_well_formed_v36);\n reveal_with_fuel(invocation_source_micro_run_{root}_{}_v36, {});\n invocation_paired_source_defined_step_valid_{root}_v92(source);\n",
        hint.instance,
        add(hint.statements, 1)?
    );
    hint.emit_descriptor_wf(root, out)?;
    emit!(
        out,
        " reveal_with_fuel(invocation_source_operands_observations_v39, {});\n reveal_with_fuel(invocation_source_statements_observations_v39, {});\n}}\n",
        add(hint.operands, 1)?,
        add(hint.statements, 1)?
    );

    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_target_cut_summary_{root}_{pc}_v96(target: MemoryStateV30)\n requires target.pc == {}, target.valid, byte_inputs_{root}_v55(target, invocation_runtime_little_endian_v36()),\n ({{ let actual = target.values[{}]; ",
        summary.target_pc,
        summary.input
    );
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    emit_value_type(
        inventory
            .definitions()
            .get(summary.input)
            .ok_or_else(mismatch)?
            .ty,
        model.width,
        "actual",
        out,
    )?;
    emit!(
        out,
        " }}),\n ensures invocation_paired_actual_step_{root}_v36(target).state.valid,\n invocation_paired_actual_step_{root}_v36(target).state.pc == {},\n invocation_paired_actual_step_{root}_v36(target).state.memory == target.memory,\n invocation_paired_actual_step_{root}_v36(target).state.generations == target.generations,\n invocation_paired_actual_step_{root}_v36(target).state.frames == target.frames,\n invocation_paired_actual_step_{root}_v36(target).state.values.len() == target.values.len(),\n",
        summary.target_next
    );
    for binding in &next.live {
        out.budget.charge_work(1)?;
        let definition = binding.definition.ok_or_else(mismatch)?;
        emit!(
            out,
            " invocation_paired_actual_step_{root}_v36(target).state.values[{definition}] == target.values[{definition}],\n"
        );
    }
    emit!(
        out,
        " invocation_paired_actual_step_{root}_v36(target).events.len() == 0,\n invocation_byte_boundary_{root}_v36(target).returned.len() == 0,\n{{\n hide(byte_memory_well_formed_v30);\n hide(byte_frame_runtime_well_formed_v30);\n hide(byte_private_frames_live_v30);\n hide(private_generation_counters_valid_v30);\n hide(byte_target_view_contracts_match_{namespace}_v38);\n hide(byte_pointer_type_v30);\n reveal_with_fuel(invocation_byte_follow_{root}_v36, 1);\n reveal_with_fuel(invocation_actual_observations_v39, 2);\n}}\n"
    );
    Ok(())
}

pub(super) fn compose(root: usize, pc: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(3)?;
    emit!(
        out,
        " hide(invocation_source_block_runtime_{root}_v36);\n hide(invocation_byte_boundary_{root}_v36);\n hide(invocation_paired_source_defined_{root}_v36);\n hide(invocation_source_byte_state_well_formed_v36);\n hide(byte_memory_well_formed_v30);\n hide(byte_frame_runtime_well_formed_v30);\n hide(byte_execution_well_formed_v37);\n hide(byte_private_frames_live_v30);\n hide(private_generation_counters_valid_v30);\n hide(invocation_byte_heaps_related_v36);\n hide(invocation_value_related_v36);\n assert(source.slots.dom().is_empty()) by {{\n assert forall|descriptor: int| !source.slots.dom().contains(descriptor) by {{\n assert(!source.slots.contains_key(descriptor));\n }}\n }}\n invocation_source_cut_summary_{root}_{pc}_v96(source);\n invocation_related_target_inputs_v96(source.machine, target, invocation_source_byte_map_{root}_v36(source, target));\n invocation_target_cut_summary_{root}_{pc}_v96(target);\n invocation_cut_frame_states_related_v93(source.machine, target, invocation_paired_source_step_{root}_v36(source).state.machine, invocation_paired_actual_step_{root}_v36(target).state, invocation_source_byte_map_{root}_v36(source, target));\n"
    );
    Ok(())
}

fn headers() -> usize {
    24 * size_of::<usize>()
        + 16 * size_of::<&()>()
        + 4 * size_of::<bool>()
        + size_of::<Summary>()
        + size_of::<Option<Summary>>()
        + 2 * size_of::<Result<Option<Summary>>>()
        + 2 * size_of::<Result<()>>()
        + 3 * size_of::<std::slice::Iter<'static, Binding>>()
        + size_of::<std::slice::Iter<'static, fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'static>>>(
        )
        + size_of::<
            std::slice::Iter<'static, fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'static>>,
        >()
}
