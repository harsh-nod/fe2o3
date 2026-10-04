//! Proof partitioning over authentic source cuts, without changing the step contract.

use super::*;
use std::fmt::Write as _;

pub(super) const WRITING_SHARED: &str =
    include_str!("original_semantic_mir_scalar_store_laws_v92.vrs");

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy)]
enum Goal {
    All,
    Map,
    Heap,
    Residual,
    Relation,
    Observations,
    Halted,
    Control,
}

impl Goal {
    fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Map => "map",
            Self::Heap => "heap",
            Self::Residual => "residual",
            Self::Relation => "relation",
            Self::Observations => "observations",
            Self::Halted => "halted",
            Self::Control => "control",
        }
    }
}

pub(super) fn emit(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    row: &Root,
    hints: &SourceStepHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(2)?;
    if hints.fuels.len() != row.instances.len() || hints.entries.len() != row.instances.len() {
        return Err(mismatch());
    }
    if !hints.conserves_heap {
        emit!(
            out,
            r#"proof fn invocation_paired_source_defined_step_valid_{root}_v92(source: InvocationSourceByteStateV36)
 requires invocation_paired_source_defined_{root}_v36(source, 1),
 ensures invocation_paired_source_step_{root}_v36(source).state.machine.valid,
{{
 hide(invocation_paired_source_step_{root}_v36);
 hide(invocation_source_block_runtime_{root}_v36);
 hide(invocation_paired_source_defined_{root}_v36);
 reveal_with_fuel(invocation_paired_source_defined_{root}_v36, 2);
 if source.machine.pc < 0 {{
 reveal(invocation_paired_source_step_{root}_v36);
 }}
}}
"#
        );
    }
    header(root, None, Goal::All, out)?;
    emit!(out, "}}\n");
    for cut in row.cuts.iter().flatten() {
        out.budget.charge_work(1)?;
        let hint = source_hint(row, hints, cut, out)?;
        let constructor = constructor(row, hints, cut, hint, out)?;
        let partition = constructor.is_some() || !hints.conserves_heap;
        if partition {
            for goal in [
                Goal::Map,
                Goal::Heap,
                Goal::Residual,
                Goal::Relation,
                Goal::Observations,
                Goal::Halted,
                Goal::Control,
            ] {
                out.budget.charge_work(1)?;
                header(root, Some(cut.source), goal, out)?;
                match goal {
                    Goal::Relation => {
                        for part in [Goal::Map, Goal::Heap, Goal::Residual] {
                            invoke(root, cut.source, part, out)?;
                        }
                    }
                    _ => {
                        if constructor.is_some() && matches!(goal, Goal::Map) {
                            emit!(
                                out,
                                " hide(invocation_source_byte_state_well_formed_v36);\n"
                            );
                        }
                        unfold(
                            root,
                            row,
                            hints,
                            hint,
                            matches!(goal, Goal::Observations),
                            out,
                        )?;
                        if matches!(goal, Goal::Heap) {
                            hint.emit_write_normalization(root, out)?;
                        }
                        if hint.frame_preserving && matches!(goal, Goal::Heap) {
                            emit!(
                                out,
                                " invocation_cut_source_frame_{root}_{}_v93(source);\n",
                                cut.source
                            );
                        }
                        if let Some((call, entry)) = constructor {
                            if matches!(goal, Goal::Map) {
                                enter(root, hint, call, entry, out)?;
                            } else if matches!(goal, Goal::Control) {
                                control_values(model, root, cut, call, out)?;
                            }
                        }
                    }
                }
                emit!(out, "}}\n");
            }
        }
        header(root, Some(cut.source), Goal::All, out)?;
        if partition {
            for goal in [
                Goal::Relation,
                Goal::Observations,
                Goal::Halted,
                Goal::Control,
            ] {
                invoke(root, cut.source, goal, out)?;
            }
        } else {
            // A hint-shape miss retains all four obligations, not a new premise.
            unfold(root, row, hints, hint, true, out)?;
        }
        emit!(out, "}}\n");
    }
    Ok(())
}

fn source_hint<'a>(
    row: &Root,
    hints: &'a SourceStepHintsV85,
    cut: &Cut,
    out: &mut Writer<'_, '_>,
) -> Result<&'a SourceCutHintsV85> {
    let instance = cut
        .instance
        .checked_sub(row.instances.start)
        .ok_or_else(mismatch)?;
    let mut found = None;
    for hint in &hints.cuts {
        out.budget.charge_work(2)?;
        if hint.pc == cut.source && hint.instance == instance {
            if found.is_some() {
                return Err(mismatch());
            }
            found = Some(hint);
        }
    }
    found.ok_or_else(mismatch)
}

fn constructor<'a>(
    row: &Root,
    hints: &'a SourceStepHintsV85,
    cut: &Cut,
    hint: &'a SourceCutHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<Option<(&'a SourceCallHintsV85, &'a SourceEntryHintsV85)>> {
    out.budget.charge_work(1)?;
    let End::Call(child) = cut.end else {
        return Ok(None);
    };
    let Some(call) = hint.call.as_ref() else {
        return Ok(None);
    };
    let child = child
        .checked_sub(row.instances.start)
        .ok_or_else(mismatch)?;
    if child != call.child {
        return Err(mismatch());
    }
    let Some(entry) = hints.entries.get(child).and_then(Option::as_ref) else {
        return Ok(None);
    };
    out.budget.charge_work(2)?;
    if entry.arguments.len() != call.arguments.len() || hint.operands != call.arguments.len() {
        return Err(mismatch());
    }
    if hint.statements != 0 {
        return Ok(None);
    }
    for (_, moved, _) in &call.arguments {
        out.budget.charge_work(1)?;
        if *moved {
            return Ok(None);
        }
    }
    Ok(Some((call, entry)))
}

fn header(root: usize, pc: Option<usize>, goal: Goal, out: &mut Writer<'_, '_>) -> Result<()> {
    let name = goal.name();
    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_paired_cut_{root}_"
    );
    if let Some(pc) = pc {
        emit!(out, "pc{pc}");
    } else {
        emit!(out, "terminal");
    }
    emit!(
        out,
        "_{name}_v85(source: InvocationSourceByteStateV36, target: MemoryStateV30)\n requires invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n"
    );
    if let Some(pc) = pc {
        emit!(out, " source.machine.pc == {pc},\n");
    } else {
        emit!(out, " source.machine.pc < 0,\n");
    }
    emit!(out, " ensures ");
    match goal {
        Goal::Map => emit!(
            out,
            "invocation_source_byte_map_valid_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state),\n"
        ),
        Goal::Heap => emit!(
            out,
            "invocation_byte_states_related_v36(invocation_paired_source_step_{root}_v36(source).state.machine, invocation_paired_actual_step_{root}_v36(target).state, invocation_source_byte_map_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state)),\n"
        ),
        Goal::Residual => emit!(
            out,
            "invocation_paired_residual_{root}_v85(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state),\n"
        ),
        _ => {
            if matches!(goal, Goal::All | Goal::Relation) {
                emit!(
                    out,
                    "invocation_paired_related_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state),\n"
                );
            }
            if matches!(goal, Goal::All | Goal::Observations) {
                emit!(
                    out,
                    " invocation_paired_observations_related_{root}_v39(invocation_paired_source_step_{root}_v36(source).events, invocation_paired_actual_step_{root}_v36(target).events),\n"
                );
            }
            if matches!(goal, Goal::All | Goal::Halted) {
                emit!(
                    out,
                    " invocation_paired_source_step_{root}_v36(source).halted == invocation_paired_actual_step_{root}_v36(target).halted,\n"
                );
            }
            if matches!(goal, Goal::All | Goal::Control) {
                emit!(
                    out,
                    "source.machine.pc >= 0 ==> invocation_paired_control_values_{root}_v36(source, invocation_source_block_runtime_{root}_v36(source), invocation_byte_boundary_{root}_v36(target)),\n"
                );
            }
        }
    }
    emit!(out, "{{\n");
    Ok(())
}

fn unfold(
    root: usize,
    row: &Root,
    hints: &SourceStepHintsV85,
    cut: &SourceCutHintsV85,
    observations: bool,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for (instance, fuel) in hints.fuels.iter().enumerate() {
        out.budget.charge_work(1)?;
        if *fuel != 0 {
            emit!(
                out,
                " reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {fuel});\n"
            );
        }
    }
    emit!(
        out,
        " reveal_with_fuel(invocation_byte_follow_{root}_v36, {});\n",
        add(row.blocks.len(), 1)?
    );
    if observations {
        emit!(
            out,
            " reveal_with_fuel(invocation_source_operands_observations_v39, {});\n reveal_with_fuel(invocation_source_statements_observations_v39, {});\n",
            add(cut.operands, 1)?,
            add(cut.statements, 1)?
        );
    }
    if hints.conserves_heap {
        emit!(
            out,
            " invocation_paired_source_preserved_{root}_v77(source, target);\n"
        );
    } else {
        emit!(
            out,
            " invocation_paired_source_defined_step_valid_{root}_v92(source);\n invocation_scalar_store_facts_v92();\n"
        );
    }
    Ok(())
}

fn enter(
    root: usize,
    hint: &SourceCutHintsV85,
    call: &SourceCallHintsV85,
    entry: &SourceEntryHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let instance = hint.instance;
    emit!(
        out,
        " let little_endian = invocation_runtime_little_endian_v36();\n"
    );
    for (i, (local, moved, bits)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " let evaluated_{i} = invocation_source_value_evaluate_v42(source, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: {moved} }}, bits: {bits}int }}, {root}, {instance}, little_endian);\n let source = evaluated_{i}.source;\n"
        );
    }
    emit!(out, " let arguments: Seq<InvocationSourceValueV42> = seq![");
    for i in 0..call.arguments.len() {
        out.budget.charge_work(1)?;
        if i != 0 {
            emit!(out, ", ");
        }
        emit!(out, "evaluated_{i}.value");
    }
    let (begin, end, owner, pc) = (entry.locals.start, entry.locals.end, entry.owner, entry.pc);
    emit!(
        out,
        "];\n invocation_source_constructor_clear_well_formed_v84(source, {begin}, {end}, {owner}, {pc});\n let entered = InvocationSourceByteStateV36 {{ machine: MemoryStateV30 {{ pc: {pc}, values: Seq::new(source.machine.values.len(), |i: int| if {begin} <= i < {end} {{ MemoryValueV30::Undefined }} else {{ source.machine.values[i] }}), memory: source.machine.memory, generations: source.machine.generations, frames: byte_enter_frame_v30(source.machine.frames, {owner}), valid: true }}, logical: invocation_source_logical_clear_v38(source.logical, {begin}, {end}), ..source }};\n"
    );
    for (i, local) in entry.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " let argument_{i} = match arguments[{i}] {{ InvocationSourceValueV42::Carrier(value) => value, _ => MemoryValueV30::Undefined }};\n invocation_source_put_local_well_formed_v78(entered, {local}, argument_{i});\n let entered = invocation_source_byte_put_local_v36(entered, {local}, argument_{i});\n"
        );
    }
    Ok(())
}

fn control_values(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    cut: &Cut,
    call: &SourceCallHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let End::Call(child) = cut.end else {
        return Err(mismatch());
    };
    let parent = model
        .instances
        .get(cut.instance)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    let child = model
        .instances
        .get(child)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    emit!(
        out,
        " let original = invocation_source_block_runtime_{root}_v36(source);\n let actual = invocation_byte_boundary_{root}_v36(target);\n assert(original.returned.is_none());\n assert(actual.returned.len() == 0);\n assert(original.operands.len() == {});\n assert(actual.state.values.len() == target.values.len());\n",
        call.arguments.len()
    );
    for (i, (local, _, _)) in call.arguments.iter().enumerate() {
        out.budget.charge_work(1)?;
        // These optional equalities are proof assertions, never assumptions or admission facts.
        let Some(outgoing) = child.arguments.get(i) else {
            return Err(mismatch());
        };
        if !matches!(outgoing.logical, LogicalBinding::Plain) {
            continue;
        }
        let Some(output) = outgoing.definition else {
            continue;
        };
        for incoming in cut.live.iter().chain(&parent.suspended) {
            out.budget.charge_work(1)?;
            if matches!(incoming.source, SourceValue::Local(found) if found == *local)
                && matches!(incoming.logical, LogicalBinding::Plain)
            {
                if let Some(input) = incoming.definition {
                    emit!(
                        out,
                        " assert(source.machine.values[{local}] == target.values[{input}]);\n assert(original.operands[{i}].value == InvocationSourceValueV42::Carrier(source.machine.values[{local}]));\n assert(actual.state.values[{output}] == target.values[{input}]);\n"
                    );
                    break;
                }
            }
        }
    }
    Ok(())
}

fn invoke(root: usize, pc: usize, goal: Goal, out: &mut Writer<'_, '_>) -> Result<()> {
    let name = goal.name();
    emit!(
        out,
        " invocation_paired_cut_{root}_pc{pc}_{name}_v85(source, target);\n"
    );
    Ok(())
}

pub(super) fn dispatch(root: usize, row: &Root, out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        " if source.machine.pc < 0 {{ invocation_paired_cut_{root}_terminal_all_v85(source, target); }}\n"
    );
    for cut in row.cuts.iter().flatten() {
        out.budget.charge_work(1)?;
        emit!(out, " else if source.machine.pc == {} {{\n", cut.source);
        invoke(root, cut.source, Goal::All, out)?;
        emit!(out, " }}\n");
    }
    emit!(out, " else {{ assert(false); }}\n");
    Ok(())
}

fn headers() -> usize {
    size_of::<Goal>()
        + 2 * size_of::<bool>()
        + size_of::<Option<usize>>()
        + size_of::<Result<&'static SourceCutHintsV85>>()
        + size_of::<Result<Option<(&'static SourceCallHintsV85, &'static SourceEntryHintsV85)>>>()
        + size_of::<std::iter::Flatten<std::slice::Iter<'static, Option<Cut>>>>()
        + size_of::<std::slice::Iter<'static, SourceCutHintsV85>>()
        + size_of::<std::array::IntoIter<Goal, 7>>()
        + size_of::<std::array::IntoIter<Goal, 4>>()
        + size_of::<std::array::IntoIter<Goal, 3>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'static, usize>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'static, (usize, bool, u32)>>>()
        + size_of::<
            std::iter::Chain<
                std::slice::Iter<'static, Binding>,
                std::slice::Iter<'static, Binding>,
            >,
        >()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + 12 * size_of::<Result<()>>()
}
