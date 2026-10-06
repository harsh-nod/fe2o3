//! Reusable state equations for authentic scalar constructors and empty edge chains.

use super::*;
use fe2o3_kernel_ir::Terminator;

#[path = "original_semantic_mir_constructor_pc_generate_v182.rs"]
mod source_pc;
#[path = "original_semantic_mir_constructor_record_generate_v181.rs"]
mod source_record;

pub(super) struct Summary {
    base: usize,
    start: usize,
    next: usize,
    edges: usize,
}

pub(super) fn derive(
    model: &PairedInvocations<'_, '_, '_>,
    row: &Root,
    block: usize,
    call: &SourceCallHintsV85,
    entry: &SourceEntryHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Summary>> {
    model.check(out)?;
    out.budget.reserve_storage(40 * size_of::<usize>())?;
    out.budget.charge_work(4)?;
    if entry.arguments.len() != call.arguments.len() {
        return Err(mismatch());
    }
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    let start = add(row.blocks.start, block)?;
    let mut current = start;
    for edges in 1..=row.blocks.len() {
        out.budget.charge_work(7)?;
        let current_block = inventory.blocks().get(current).ok_or_else(mismatch)?;
        if !current_block.operations.is_empty()
            || !matches!(current_block.terminator, Terminator::Branch { .. })
            || current_block.edges.len() != 1
        {
            return Ok(None);
        }
        let edge = inventory
            .edges()
            .get(current_block.edges.start)
            .ok_or_else(mismatch)?;
        if edge.target.function != current_block.coordinate.function {
            return Ok(None);
        }
        let local = edge.target.block as usize;
        if local >= row.blocks.len() {
            return Ok(None);
        }
        let next = add(row.blocks.start, local)?;
        let target = inventory.blocks().get(next).ok_or_else(mismatch)?;
        if target.coordinate != edge.target
            || edge.bindings.len() != target.parameters.len()
            || edge.arguments.len() != target.parameters.len()
        {
            return Ok(None);
        }
        for (ordinal, binding) in inventory
            .edge_arguments()
            .get(edge.bindings.clone())
            .ok_or_else(mismatch)?
            .iter()
            .enumerate()
        {
            out.budget.charge_work(3)?;
            if binding.target_definition != add(target.parameters.start, ordinal)?
                || binding.incoming_definition >= model.definitions
                || binding.target_definition >= model.definitions
            {
                return Ok(None);
            }
        }
        if let Some(cut) = row.cuts.get(local).ok_or_else(mismatch)? {
            if cut.source == entry.pc && cut.instance == add(row.instances.start, call.child)? {
                return Ok(Some(Summary {
                    base: row.blocks.start,
                    start,
                    next,
                    edges,
                }));
            }
            return Ok(None);
        }
        if next <= current {
            return Ok(None);
        }
        current = next;
    }
    // Forward non-cut edges are visited at most once; other paths use the old proof.
    Ok(None)
}

pub(super) fn emit(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    cut: &Cut,
    hints: &SourceStepHintsV85,
    hint: &SourceCutHintsV85,
    call: &SourceCallHintsV85,
    entry: &SourceEntryHintsV85,
    summary: &Summary,
    follow_fuel: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    model.check(out)?;
    out.budget.reserve_storage(48 * size_of::<usize>())?;
    out.budget.charge_work(1)?;
    if entry.arguments.len() != call.arguments.len() {
        return Err(mismatch());
    }
    let pc = cut.source;
    emit!(
        out,
        "spec fn invocation_constructor_source_{root}_{pc}_v162(source: InvocationSourceByteStateV36) -> InvocationSourceByteStateV36 {{\n"
    );
    emit!(
        out,
        " let entered = invocation_source_entry_initialize_v166(source, {}, {}, {}, byte_enter_frame_v30(source.machine.frames, {}));\n",
        entry.pc,
        entry.locals.start,
        entry.locals.end,
        entry.owner
    );
    for (destination, (source, moved, _)) in entry.arguments.iter().zip(&call.arguments) {
        out.budget.charge_work(2)?;
        if *moved {
            return Err(mismatch());
        }
        emit!(
            out,
            " let entered = invocation_source_byte_put_local_v36(entered, {destination}, source.machine.values[{source}]);\n"
        );
    }
    emit!(out, " entered\n}}\n");
    emit!(
        out,
        "spec fn invocation_constructor_target_{root}_{pc}_v162(target: MemoryStateV30) -> MemoryStateV30 {{\n let values = target.values;\n"
    );
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    let mut current = summary.start;
    for _ in 0..summary.edges {
        out.budget.charge_work(3)?;
        let block = inventory.blocks().get(current).ok_or_else(mismatch)?;
        let edge = inventory
            .edges()
            .get(block.edges.start)
            .ok_or_else(mismatch)?;
        emit!(out, " let before = values;\n let values = before");
        for binding in inventory
            .edge_arguments()
            .get(edge.bindings.clone())
            .ok_or_else(mismatch)?
        {
            out.budget.charge_work(1)?;
            emit!(
                out,
                ".update({}, before[{}])",
                binding.target_definition,
                binding.incoming_definition
            );
        }
        emit!(out, ";\n");
        current = add(summary.base, edge.target.block as usize)?;
    }
    if current != summary.next {
        return Err(mismatch());
    }
    emit!(
        out,
        " MemoryStateV30 {{ pc: {}, values, valid: true, ..target }}\n}}\n",
        summary.next
    );
    source_state(root, pc, hints, hint, call, entry, out)?;
    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_constructor_states_{root}_{pc}_v162(source: InvocationSourceByteStateV36, target: MemoryStateV30)\n requires invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1), source.machine.pc == {pc},\n ensures\n"
    );
    for (side, argument) in [("source", "source"), ("actual", "target")] {
        out.budget.charge_work(3)?;
        let value = if side == "source" { "source" } else { "target" };
        emit!(
            out,
            " invocation_paired_{side}_step_{root}_v36({argument}).state == invocation_constructor_{value}_{root}_{pc}_v162({argument}),\n invocation_paired_{side}_step_{root}_v36({argument}).events.len() == 0,\n !invocation_paired_{side}_step_{root}_v36({argument}).halted,\n"
        );
    }
    emit!(
        out,
        " invocation_source_block_runtime_{root}_v36(source).source == invocation_constructor_source_{root}_{pc}_v162(source),\n invocation_byte_boundary_{root}_v36(target).state == invocation_constructor_target_{root}_{pc}_v162(target),\n invocation_source_block_runtime_{root}_v36(source).returned.is_none(),\n invocation_byte_boundary_{root}_v36(target).returned.len() == 0,\n invocation_source_block_runtime_{root}_v36(source).operands.len() == {},\n",
        call.arguments.len()
    );
    for (ordinal, (local, _, _)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " invocation_source_block_runtime_{root}_v36(source).operands[{ordinal}].value == InvocationSourceValueV42::Carrier(source.machine.values[{local}]),\n"
        );
    }
    emit!(out, "{{\n");
    out.budget.charge_work(9)?;
    emit!(
        out,
        " hide(invocation_paired_source_step_{root}_v36);\n hide(invocation_paired_actual_step_{root}_v36);\n hide(invocation_source_block_runtime_{root}_v36);\n hide(invocation_byte_boundary_{root}_v36);\n hide(invocation_constructor_source_{root}_{pc}_v162);\n hide(invocation_actual_observations_v39);\n"
    );
    residual_context(root, out)?;
    emit!(out, " assert(target.pc == {});\n", summary.start);
    emit!(
        out,
        " invocation_constructor_source_state_{root}_{pc}_v165(source);\n reveal_with_fuel(invocation_byte_follow_{root}_v36, {follow_fuel});\n"
    );
    emit!(
        out,
        " assert(invocation_byte_boundary_{root}_v36(target).state == invocation_constructor_target_{root}_{pc}_v162(target)\n && invocation_byte_boundary_{root}_v36(target).returned.len() == 0\n && invocation_byte_boundary_{root}_v36(target).observations.len() == 0) by {{\n reveal(invocation_byte_boundary_{root}_v36);\n }}\n"
    );
    out.budget.charge_work(3)?;
    emit!(
        out,
        " assert(invocation_paired_actual_step_{root}_v36(target).state == invocation_constructor_target_{root}_{pc}_v162(target)) by {{\n reveal(invocation_paired_actual_step_{root}_v36);\n }}\n assert(invocation_paired_actual_step_{root}_v36(target).events == Seq::empty()) by {{\n reveal(invocation_paired_actual_step_{root}_v36);\n reveal(invocation_actual_observations_v39);\n }}\n assert(!invocation_paired_actual_step_{root}_v36(target).halted) by {{\n reveal(invocation_paired_actual_step_{root}_v36);\n }}\n}}\n"
    );
    Ok(())
}

// Source validity already follows from source-definedness. Keeping target input
// relations out of this lemma avoids mixing two independent state equations.
fn source_state(
    root: usize,
    pc: usize,
    hints: &SourceStepHintsV85,
    hint: &SourceCutHintsV85,
    call: &SourceCallHintsV85,
    entry: &SourceEntryHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    source_runtime(root, pc, hints, hint, call, entry, out)?;
    out.budget.reserve_storage(13 * size_of::<usize>())?;
    out.budget.charge_work(12)?;
    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_constructor_source_state_{root}_{pc}_v165(source: InvocationSourceByteStateV36)\n requires invocation_paired_source_defined_{root}_v36(source, 1), source.machine.pc == {pc},\n ensures\n invocation_paired_source_step_{root}_v36(source).state == invocation_constructor_source_{root}_{pc}_v162(source),\n invocation_paired_source_step_{root}_v36(source).events.len() == 0,\n !invocation_paired_source_step_{root}_v36(source).halted,\n invocation_source_block_runtime_{root}_v36(source).source == invocation_constructor_source_{root}_{pc}_v162(source),\n invocation_source_block_runtime_{root}_v36(source).returned.is_none(),\n invocation_source_block_runtime_{root}_v36(source).operands.len() == {},\n",
        call.arguments.len()
    );
    for (ordinal, (local, _, _)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " invocation_source_block_runtime_{root}_v36(source).operands[{ordinal}].value == InvocationSourceValueV42::Carrier(source.machine.values[{local}]),\n"
        );
    }
    emit!(
        out,
        "{{\n hide(invocation_paired_source_step_{root}_v36);\n hide(invocation_source_block_runtime_{root}_v36);\n hide(invocation_source_observations_v39);\n hide(invocation_source_entry_initialize_v166);\n invocation_constructor_source_runtime_{root}_{pc}_v167(source);\n assert(invocation_paired_source_step_{root}_v36(source).state == invocation_constructor_source_{root}_{pc}_v162(source)) by {{\n reveal(invocation_paired_source_step_{root}_v36);\n }}\n"
    );
    out.budget.charge_work(5)?;
    emit!(
        out,
        " invocation_constructor_source_pc_{root}_{pc}_v182(source);\n assert(invocation_paired_source_step_{root}_v36(source).state.machine.pc >= 0\n && invocation_paired_source_step_{root}_v36(source).state.machine.pc != -2);\n assert(invocation_paired_source_step_{root}_v36(source).events == Seq::empty()) by {{\n reveal(invocation_paired_source_step_{root}_v36);\n reveal(invocation_source_observations_v39);\n }}\n assert(!invocation_paired_source_step_{root}_v36(source).halted) by {{\n reveal(invocation_paired_source_step_{root}_v36);\n }}\n}}\n"
    );
    Ok(())
}

// Keep full runtime state construction separate from its scalar step projections.
fn source_runtime(
    root: usize,
    pc: usize,
    hints: &SourceStepHintsV85,
    hint: &SourceCutHintsV85,
    call: &SourceCallHintsV85,
    entry: &SourceEntryHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(20 * size_of::<usize>())?;
    out.budget.charge_work(24)?;
    if entry.arguments.len() != call.arguments.len() {
        return Err(mismatch());
    }
    let fuel = add(hint.statements, 1)?;
    if fuel > *hints.fuels.get(hint.instance).ok_or_else(mismatch)? {
        return Err(mismatch());
    }
    source_pc::emit(root, pc, call, entry, out)?;
    source_record::emit(root, pc, hint, call, out)?;
    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_constructor_source_runtime_{root}_{pc}_v167(source: InvocationSourceByteStateV36)\n requires invocation_paired_source_defined_{root}_v36(source, 1), source.machine.pc == {pc},\n ensures\n invocation_source_block_runtime_{root}_v36(source).source == invocation_constructor_source_{root}_{pc}_v162(source),\n invocation_source_block_runtime_{root}_v36(source).returned.is_none(),\n invocation_source_block_runtime_{root}_v36(source).operands.len() == {},\n",
        call.arguments.len()
    );
    for (ordinal, (local, _, _)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " invocation_source_block_runtime_{root}_v36(source).operands[{ordinal}].value == InvocationSourceValueV42::Carrier(source.machine.values[{local}]),\n"
        );
    }
    emit!(
        out,
        " invocation_source_observations_v39(invocation_source_block_runtime_{root}_v36(source), invocation_runtime_little_endian_v36()) == Seq::empty(),\n"
    );
    emit!(
        out,
        "{{\n hide(invocation_source_local_observation_v180);\n hide(invocation_paired_source_step_{root}_v36);\n hide(invocation_source_block_runtime_{root}_v36);\n hide(invocation_source_enter_{root}_{}_v36);\n hide(invocation_source_entry_refuses_{root}_{}_v167);\n hide(invocation_source_entry_body_{root}_{}_v167);\n hide(invocation_source_entry_select_v167);\n hide(invocation_source_observations_v39);\n hide(invocation_source_value_evaluate_v42);\n hide(invocation_paired_source_defined_{root}_v36);\n hide(invocation_source_byte_state_well_formed_v36);\n hide(invocation_source_byte_value_typed_v36);\n hide(invocation_source_entry_initialize_v166);\n hide(invocation_source_byte_put_local_v36);\n assert(invocation_paired_source_step_{root}_v36(source).state.machine.valid) by {{\n reveal_with_fuel(invocation_paired_source_defined_{root}_v36, 2);\n }}\n",
        call.child,
        call.child,
        call.child
    );
    out.budget.charge_work(4)?;
    emit!(
        out,
        " assert(invocation_source_block_runtime_{root}_v36(source).source.machine.valid) by {{\n reveal(invocation_paired_source_step_{root}_v36);\n }}\n invocation_constructor_source_record_{root}_{pc}_v181(source);\n"
    );
    out.budget.charge_work(1)?;
    emit!(out, " let copied_0 = source;\n");
    for (ordinal, (local, moved, bits)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(5)?;
        if *moved {
            return Err(mismatch());
        }
        let next = add(ordinal, 1)?;
        emit!(
            out,
            " invocation_source_scalar_copy_valid_identity_v164(copied_{ordinal}, {local}, {bits}, {root}, {}, invocation_runtime_little_endian_v36());\n let captured_{ordinal} = invocation_source_value_evaluate_v42(copied_{ordinal}, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: false }}, bits: {bits}int }}, {root}, {}, invocation_runtime_little_endian_v36());\n let copied_{next} = captured_{ordinal}.source;\n",
            hint.instance,
            hint.instance
        );
    }
    out.budget.charge_work(3)?;
    emit!(out, " let entered_arguments = seq![");
    for ordinal in 0..call.arguments.len() {
        out.budget.charge_work(1)?;
        emit!(out, "captured_{ordinal}.value,");
    }
    emit!(out, "];\n");
    emit!(
        out,
        " invocation_source_entry_select_success_v167(copied_{0}, invocation_source_entry_refuses_{root}_{1}_v167(copied_{0}, entered_arguments, invocation_runtime_little_endian_v36()), invocation_source_entry_body_{root}_{1}_v167(copied_{0}, entered_arguments, invocation_runtime_little_endian_v36()));\n",
        call.arguments.len(),
        call.child
    );
    out.budget.charge_work(3)?;
    emit!(
        out,
        " assert(copied_{}.machine.valid) by {{\n reveal(invocation_source_enter_{root}_{}_v36);\n }}\n assert(copied_{} == source);\n",
        call.arguments.len(),
        call.child,
        call.arguments.len()
    );
    out.budget.charge_work(1)?;
    emit!(
        out,
        " invocation_constructor_source_pc_{root}_{pc}_v182(source);\n"
    );
    emit!(
        out,
        " assert(invocation_source_block_runtime_{root}_v36(source).source == invocation_constructor_source_{root}_{pc}_v162(source)\n && invocation_source_block_runtime_{root}_v36(source).returned.is_none()\n && invocation_source_block_runtime_{root}_v36(source).operands.len() == {}\n",
        call.arguments.len()
    );
    for (ordinal, (local, _, _)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " && invocation_source_block_runtime_{root}_v36(source).operands[{ordinal}].value == InvocationSourceValueV42::Carrier(source.machine.values[{local}])\n"
        );
    }
    out.budget.charge_work(5)?;
    emit!(
        out,
        " && invocation_source_block_runtime_{root}_v36(source).observations.len() == 0) by {{\n reveal(invocation_source_enter_{root}_{}_v36);\n reveal(invocation_source_entry_body_{root}_{}_v167);\n }}\n",
        call.child,
        call.child
    );
    for (ordinal, (local, _, bits)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(2)?;
        emit!(
            out,
            " invocation_source_scalar_copy_observation_checked_v188(copied_{ordinal}, invocation_source_block_runtime_{root}_v36(source).operands[{ordinal}], {local}, {bits}, {root}, {}, invocation_runtime_little_endian_v36());\n",
            hint.instance
        );
    }
    for ordinal in 0..call.arguments.len() {
        out.budget.charge_work(2)?;
        emit!(
            out,
            " invocation_source_local_observations_extend_v186(invocation_source_block_runtime_{root}_v36(source).operands, {ordinal});\n"
        );
    }
    emit!(
        out,
        " invocation_source_local_block_observations_empty_v180(invocation_source_block_runtime_{root}_v36(source), invocation_runtime_little_endian_v36());\n"
    );
    emit!(out, "}}\n");
    Ok(())
}

pub(super) fn opacity(root: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(5)?;
    emit!(
        out,
        " hide(invocation_paired_source_step_{root}_v36);\n hide(invocation_paired_actual_step_{root}_v36);\n hide(invocation_source_block_runtime_{root}_v36);\n hide(invocation_byte_boundary_{root}_v36);\n hide(invocation_paired_source_defined_{root}_v36);\n"
    );
    Ok(())
}

pub(super) fn consume(root: usize, pc: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(1)?;
    emit!(
        out,
        " invocation_constructor_states_{root}_{pc}_v162(source, target);\n"
    );
    Ok(())
}
