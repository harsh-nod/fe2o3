//! Generated scalar boundary steps are composed from the same authenticated
//! concrete replay used by V35. Physical connector counts are not source fuel.
use super::super::super::{
    ScalarV30, canonical::control::TargetBranch, control::Branch, emit_graph_v30,
    target_trace::ConcreteTrace,
};
use super::*;
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

const STATES: &str = r#"
// The enclosing generator supplies the shared BYTE_MEMORY_V30 vocabulary.
// These scalar-only steps annotate dynamic frames; they do not delete bytes.
struct OriginalInvocationRuntimeV36 {
    scalar: OriginalControlStateV31,
    frames: MemoryFrameRuntimeV30,
}
struct ActualInvocationRuntimeV36 {
    scalar: AggregateStateV30,
    frames: MemoryFrameRuntimeV30,
}
"#;

pub(super) fn emit(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    model.check(out)?;
    out.budget.reserve_storage(headers())?;
    emit!(out, "{STATES}");
    model.original.emit_steps(out)?;
    physical::emit(model, out)?;
    segments::emit(model, out)?;
    for root in 0..model.roots.len() {
        out.budget.charge_work(1)?;
        dispatch(model, root, out)?;
        physical::compose(model, root, out)?;
        related(model, root, out)?;
        initial(model, root, out)?;
        proofs(model, root, out)?;
        concrete_proof(root, out)?;
    }
    Ok(())
}

fn key(body: &Body, block: usize) -> Result<usize> {
    body.blocks
        .start
        .checked_add(block)
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn graph_key(block: usize, edge: usize) -> Result<usize> {
    let sum = block.checked_add(edge).ok_or(Resource::Arithmetic)?;
    sum.checked_add(1)
        .and_then(|next| sum.checked_mul(next))
        .map(|product| product / 2)
        .and_then(|base| base.checked_add(edge))
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn value(node: Option<usize>, out: &mut Writer<'_, '_>) -> Result<()> {
    if let Some(node) = node {
        emit!(out, "n[{node}]");
    } else {
        emit!(out, "0int");
    }
    Ok(())
}

fn state(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    trace: &ConcreteTrace<'_, '_>,
    next: Option<usize>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let start = model.inventory.functions()[model.roots[root].physical]
        .definitions
        .start;
    emit!(out, "let values = s.values");
    for (local, &node) in trace.values.iter().enumerate() {
        out.budget.charge_work(2)?;
        if let Some(node) = node {
            let definition = start.checked_add(local).ok_or(Resource::Arithmetic)?;
            emit!(out, ".update({definition}int, n[{node}])");
        }
    }
    emit!(out, ";\n let state = AggregateStateV30 {{ pc: ");
    match next {
        Some(next) => emit!(out, "{next}int"),
        None => emit!(out, "-1int"),
    }
    emit!(
        out,
        ", values, cells: s.cells, initialized: s.initialized, external: s.external }};\n"
    );
    Ok(())
}

pub(super) fn entry(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    trace: &ConcreteTrace<'_, '_>,
    path: &[usize],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(4)?;
    let body_index = model.original.roots[root].start;
    let body = model.original.bodies[body_index]
        .as_ref()
        .ok_or_else(mismatch)?;
    let actual = model.bodies[body_index].as_ref().ok_or_else(mismatch)?;
    let entry = actual.bindings[body.control.entry.get() as usize]
        .as_ref()
        .ok_or_else(mismatch)?;
    let count = model.inventory.definitions().len();
    emit_graph_v30(
        &trace.nodes,
        count,
        root,
        "invocation_entry",
        (0..trace.nodes.len()).map(|node| Ok(Some(node))),
        None,
        out,
    )?;
    emit!(
        out,
        "spec fn actual_invocation_ready_{root}_v36(s: AggregateStateV30) -> AggregateStateV30\n recommends s.values.len() == {count},\n{{\n let n = original_invocation_entry_trace_{root}_v30(s.values);\n"
    );
    state(model, root, trace, Some(entry.physical.block as usize), out)?;
    emit!(
        out,
        " state\n}}\nspec fn actual_invocation_prefix_{root}_v36() -> Seq<int> {{ seq!["
    );
    for &block in path {
        out.budget.charge_work(1)?;
        emit!(out, "{block}int,");
    }
    emit!(out, "] }}\n");
    physical::prefix(model, root, path.len(), out)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn segment(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    index: usize,
    block: usize,
    ordinal: usize,
    trace: &ConcreteTrace<'_, '_>,
    assignments: &[(usize, Option<usize>)],
    returned: Option<usize>,
    next: Option<(usize, usize)>,
    path: &[usize],
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(5)?;
    let body = model.original.bodies[index].as_ref().ok_or_else(mismatch)?;
    let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
    let binding = actual.bindings[block].as_ref().ok_or_else(mismatch)?;
    let source = body.control.blocks[block].as_ref().ok_or_else(mismatch)?;
    let key = key(body, block)?;
    let graph = graph_key(key, ordinal)?;
    let count = model.inventory.definitions().len();
    let target = next
        .map(|(body, block)| {
            model
                .bodies
                .get(body)
                .and_then(Option::as_ref)
                .and_then(|body| body.bindings.get(block))
                .and_then(Option::as_ref)
                .map(|binding| binding.physical.block as usize)
                .ok_or_else(mismatch)
        })
        .transpose()?;
    if assignments.len() != source.program.assignments.len()
        || returned.is_some() != source.program.returned.is_some()
    {
        return Err(mismatch());
    }
    emit_graph_v30(
        &trace.nodes,
        count,
        graph,
        "invocation_actual",
        (0..trace.nodes.len()).map(|node| Ok(Some(node))),
        None,
        out,
    )?;
    emit!(
        out,
        "spec fn actual_invocation_segment_{key}_{ordinal}_v36(s: AggregateStateV30) -> CfgStepV26<AggregateStateV30, int>\n recommends s.values.len() == {count},\n{{\n let n = original_invocation_actual_trace_{graph}_v30(s.values);\n"
    );
    state(model, body.root, trace, target, out)?;
    emit!(out, " CfgStepV26 {{ state, events: seq![");
    for &(_, node) in assignments {
        out.budget.charge_work(1)?;
        value(node, out)?;
        emit!(out, ",");
    }
    if let Some(node) = returned {
        emit!(out, "n[{node}],");
    }
    emit!(
        out,
        "], halted: {} }}\n}}\nspec fn actual_invocation_path_{key}_{ordinal}_v36() -> Seq<int> {{ seq![{}int,",
        next.is_none(),
        binding.physical.block
    );
    for &block in path {
        out.budget.charge_work(1)?;
        emit!(out, "{block}int,");
    }
    emit!(out, "] }}\n");
    physical::segment(model, index, block, ordinal, path.len(), out)?;
    Ok(())
}

fn owner(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    index: usize,
    out: &Writer<'_, '_>,
) -> Result<u32> {
    let body = model.original.bodies[index].as_ref().ok_or_else(mismatch)?;
    Ok(model
        .original
        .transfers
        .plan(out)?
        .instance(body.root, body.instance, out)?
        .function
        .index())
}

fn next_frames(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    body: &Body,
    branch: &Branch,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    match branch {
        Branch::Call { transfer, .. } => {
            let transfer = model.original.transfer(*transfer, out)?;
            let index = model.original.roots[body.root]
                .start
                .checked_add(transfer.child)
                .ok_or(Resource::Arithmetic)?;
            let owner = owner(model, index, out)?;
            emit!(out, "byte_enter_frame_v30(f, {owner}int)");
        }
        Branch::Return => emit!(out, "byte_pop_frame_v30(f)"),
        _ => emit!(out, "f"),
    }
    Ok(())
}

fn dispatch(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let definitions = model.inventory.definitions().len();
    for index in model.original.roots[root].clone() {
        out.budget.charge_work(1)?;
        let Some(body) = model.original.bodies[index].as_ref() else {
            continue;
        };
        let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
        for (block, binding) in actual.bindings.iter().enumerate() {
            out.budget.charge_work(1)?;
            let Some(binding) = binding else { continue };
            let target = &model.roots[root].targets[binding.physical.block as usize];
            if matches!(target.branch, TargetBranch::Switch { .. }) {
                let key = key(body, block)?;
                emit_graph_v30(
                    &target.program.nodes,
                    definitions,
                    key,
                    "invocation_selector",
                    (0..target.program.nodes.len()).map(|node| Ok(Some(node))),
                    None,
                    out,
                )?;
            }
        }
    }
    emit!(
        out,
        "spec fn actual_invocation_step_{root}_v36(s: AggregateStateV30) -> CfgStepV26<AggregateStateV30, int>\n recommends s.values.len() == {definitions},\n{{\n"
    );
    for index in model.original.roots[root].clone() {
        out.budget.charge_work(1)?;
        let Some(body) = model.original.bodies[index].as_ref() else {
            continue;
        };
        let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
        for (block, binding) in actual.bindings.iter().enumerate() {
            out.budget.charge_work(1)?;
            let Some(binding) = binding else { continue };
            let key = key(body, block)?;
            emit!(out, " if s.pc == {}int {{\n", binding.physical.block);
            let target = &model.roots[root].targets[binding.physical.block as usize];
            if let TargetBranch::Switch {
                selector, cases, ..
            } = &target.branch
            {
                emit!(
                    out,
                    " let n = original_invocation_selector_trace_{key}_v30(s.values);\n"
                );
                for (ordinal, &(value, _)) in cases.iter().enumerate() {
                    out.budget.charge_work(1)?;
                    emit!(
                        out,
                        " if n[{selector}] == {value}int {{ actual_invocation_segment_{key}_{ordinal}_v36(s) }} else "
                    );
                }
                emit!(
                    out,
                    "{{ actual_invocation_segment_{key}_{}_v36(s) }}",
                    cases.len()
                );
            } else {
                emit!(out, " actual_invocation_segment_{key}_0_v36(s)");
            }
            emit!(out, "\n }} else\n");
        }
    }
    emit!(
        out,
        " {{ CfgStepV26 {{ state: s, events: Seq::empty(), halted: true }} }}\n}}\n"
    );
    for source in [true, false] {
        let side = if source { "source" } else { "actual" };
        emit!(
            out,
            "spec fn invocation_{side}_frames_{root}_v36(pc: int, f: MemoryFrameRuntimeV30) -> MemoryFrameRuntimeV30 {{\n"
        );
        for index in model.original.roots[root].clone() {
            out.budget.charge_work(1)?;
            let Some(body) = model.original.bodies[index].as_ref() else {
                continue;
            };
            let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
            for (block, original) in body.control.blocks.iter().enumerate() {
                out.budget.charge_work(1)?;
                let Some(original) = original else { continue };
                let pc = if source {
                    key(body, block)?
                } else {
                    actual.bindings[block]
                        .as_ref()
                        .ok_or_else(mismatch)?
                        .physical
                        .block as usize
                };
                emit!(out, " if pc == {pc}int {{ ");
                next_frames(model, body, &original.branch, out)?;
                emit!(out, " }} else\n");
            }
        }
        emit!(out, " {{ f }}\n}}\n");
    }
    emit!(
        out,
        "spec fn invocation_source_step_{root}_v36(s: OriginalInvocationRuntimeV36) -> CfgStepV26<OriginalInvocationRuntimeV36, int> {{\n let step = original_invocation_step_{root}_v34(s.scalar);\n CfgStepV26 {{ state: OriginalInvocationRuntimeV36 {{ scalar: step.state, frames: invocation_source_frames_{root}_v36(s.scalar.pc, s.frames) }}, events: step.events, halted: step.halted }}\n}}\nspec fn invocation_actual_step_{root}_v36(s: ActualInvocationRuntimeV36) -> CfgStepV26<ActualInvocationRuntimeV36, int> {{\n let step = invocation_physical_boundary_step_{root}_v36(s.scalar);\n CfgStepV26 {{ state: ActualInvocationRuntimeV36 {{ scalar: step.state, frames: invocation_actual_frames_{root}_v36(s.scalar.pc, s.frames) }}, events: step.events, halted: step.halted }}\n}}\n"
    );
    Ok(())
}

fn live(
    local: usize,
    scalar: ScalarV30,
    definition: Option<usize>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(2)?;
    let modulus = 1u128 << scalar.width();
    emit!(
        out,
        "\n && n.scalar.defined[{local}] && 0 <= n.scalar.values[{local}] < {modulus}int && n.scalar.values[{local}] == "
    );
    match definition {
        Some(definition) => emit!(out, "o.scalar.values[{definition}]"),
        None if scalar == ScalarV30::Unit => emit!(out, "0int"),
        None => return Err(mismatch()),
    }
    Ok(())
}

fn ancestors(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    index: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<usize>> {
    let body = model.original.bodies[index].as_ref().ok_or_else(mismatch)?;
    let mut result = vector(model.original.roots[body.root].len(), out)?;
    let mut at = index;
    loop {
        out.budget.charge_work(3)?;
        if result.len() == result.capacity() {
            return Err(mismatch());
        }
        result.push(at);
        let body = model.original.bodies[at].as_ref().ok_or_else(mismatch)?;
        let incoming = model
            .original
            .transfers
            .plan(out)?
            .instance(body.root, body.instance, out)?
            .incoming;
        let Some((parent, _)) = incoming else { break };
        let parent = model.original.roots[body.root]
            .start
            .checked_add(parent)
            .ok_or(Resource::Arithmetic)?;
        if parent >= at {
            return Err(mismatch());
        }
        at = parent;
    }
    Ok(result)
}

fn related(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let locals = model.original.locals;
    let definitions = model.inventory.definitions().len();
    emit!(
        out,
        "spec fn invocation_related_{root}_v36(n: OriginalInvocationRuntimeV36, o: ActualInvocationRuntimeV36) -> bool {{\n n.scalar.values.len() == {locals} && n.scalar.defined.len() == {locals} && o.scalar.values.len() == {definitions}\n && n.scalar.cells == o.scalar.cells && n.scalar.initialized == o.scalar.initialized && n.scalar.external == o.scalar.external\n && invocation_boundary_{root}_v36(o.scalar)\n && n.frames == o.frames && byte_frame_runtime_well_formed_v30(n.frames)\n && (if n.scalar.pc == -1int {{ o.scalar.pc == -1int && n.frames.active.len() == 0 }} else "
    );
    for index in model.original.roots[root].clone() {
        out.budget.charge_work(1)?;
        let Some(body) = model.original.bodies[index].as_ref() else {
            continue;
        };
        let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
        let chain = ancestors(model, index, out)?;
        for (block, binding) in actual.bindings.iter().enumerate() {
            out.budget.charge_work(1)?;
            let Some(binding) = binding else { continue };
            let key = key(body, block)?;
            emit!(
                out,
                "if n.scalar.pc == {key}int {{ o.scalar.pc == {}int && n.frames.active.len() == {}",
                binding.physical.block,
                chain.len()
            );
            for (depth, &ancestor) in chain.iter().rev().enumerate() {
                out.budget.charge_work(1)?;
                let owner = owner(model, ancestor, out)?;
                emit!(out, " && n.frames.active[{depth}].owner == {owner}int");
            }
            for &(local, definition) in &binding.live {
                let global = body
                    .locals
                    .start
                    .checked_add(local as usize)
                    .ok_or(Resource::Arithmetic)?;
                live(global, body.control.types[local as usize], definition, out)?;
            }
            for suspended in &actual.suspended {
                live(suspended.local, suspended.scalar, suspended.definition, out)?;
            }
            emit!(out, " }} else ");
        }
    }
    emit!(out, "false)\n}}\n");
    Ok(())
}

fn proofs(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        "proof fn invocation_step_relation_{root}_v36(n: OriginalInvocationRuntimeV36, o: ActualInvocationRuntimeV36)\n requires invocation_related_{root}_v36(n, o),\n ensures invocation_source_step_{root}_v36(n).events == invocation_actual_step_{root}_v36(o).events,\n invocation_source_step_{root}_v36(n).halted == invocation_actual_step_{root}_v36(o).halted,\n invocation_related_{root}_v36(invocation_source_step_{root}_v36(n).state, invocation_actual_step_{root}_v36(o).state),\n{{\n invocation_boundary_concrete_{root}_v36(o.scalar);\n if n.scalar.pc == -1int {{ }} else "
    );
    for index in model.original.roots[root].clone() {
        out.budget.charge_work(1)?;
        let Some(body) = model.original.bodies[index].as_ref() else {
            continue;
        };
        for (block, source) in body.control.blocks.iter().enumerate() {
            out.budget.charge_work(1)?;
            let Some(source) = source else { continue };
            let key = key(body, block)?;
            emit!(out, "if n.scalar.pc == {key}int {{\n");
            if let Branch::Switch {
                selector, cases, ..
            } = &source.branch
            {
                emit!(
                    out,
                    " let a = original_invocation_body_trace_{key}_v30(n.scalar.values.subrange({}int, {}int));\n",
                    body.locals.start,
                    body.locals.end
                );
                for &(value, _) in cases {
                    out.budget.charge_work(1)?;
                    emit!(
                        out,
                        " if a[{selector}] == {value}int {{\n assert(invocation_source_step_{root}_v36(n).events =~= invocation_actual_step_{root}_v36(o).events);\n }} else "
                    );
                }
                emit!(
                    out,
                    "{{ assert(invocation_source_step_{root}_v36(n).events =~= invocation_actual_step_{root}_v36(o).events); }}\n"
                );
            } else {
                emit!(
                    out,
                    " assert(invocation_source_step_{root}_v36(n).events =~= invocation_actual_step_{root}_v36(o).events);\n"
                );
            }
            emit!(out, " }} else ");
        }
    }
    emit!(
        out,
        "{{ assert(false); }}\n}}\nproof fn invocation_all_steps_{root}_v36()\n ensures cfg_step_simulates_v26(|n: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(n), |o: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(o), |n: OriginalInvocationRuntimeV36, o: ActualInvocationRuntimeV36| invocation_related_{root}_v36(n, o)),\n{{\n assert forall|n: OriginalInvocationRuntimeV36, o: ActualInvocationRuntimeV36| #[trigger] invocation_related_{root}_v36(n, o) implies\n invocation_source_step_{root}_v36(n).events == invocation_actual_step_{root}_v36(o).events\n && invocation_source_step_{root}_v36(n).halted == invocation_actual_step_{root}_v36(o).halted\n && invocation_related_{root}_v36(invocation_source_step_{root}_v36(n).state, invocation_actual_step_{root}_v36(o).state) by {{ invocation_step_relation_{root}_v36(n, o); }}\n}}\nproof fn invocation_finite_trace_{root}_v36(n: OriginalInvocationRuntimeV36, o: ActualInvocationRuntimeV36, fuel: nat)\n requires invocation_related_{root}_v36(n, o),\n ensures cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), n, fuel).events == cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), o, fuel).events,\n cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), n, fuel).halted == cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), o, fuel).halted,\n invocation_related_{root}_v36(cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), n, fuel).state, cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), o, fuel).state),\n{{\n invocation_all_steps_{root}_v36();\n cfg_finite_trace_refinement_v26(|n: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(n), |o: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(o), |n: OriginalInvocationRuntimeV36, o: ActualInvocationRuntimeV36| invocation_related_{root}_v36(n, o), n, o, fuel);\n}}\n"
    );
    Ok(())
}

fn initial(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let index = model.original.roots[root].start;
    let body = model.original.bodies[index].as_ref().ok_or_else(mismatch)?;
    let owner = owner(model, index, out)?;
    let locals = model.original.locals;
    let definitions = model.inventory.definitions().len();
    let first = model.inventory.functions()[model.roots[root].physical]
        .definitions
        .start;
    let entry = key(body, body.control.entry.get() as usize)?;
    emit!(
        out,
        "spec fn invocation_arguments_{root}_v36(a: Seq<int>) -> bool {{ a.len() == {}",
        body.control.arguments
    );
    for &(local, argument) in &body.control.initial {
        out.budget.charge_work(2)?;
        let modulus = 1u128 << body.control.types[local as usize].width();
        emit!(out, " && 0 <= a[{argument}] < {modulus}int");
    }
    emit!(
        out,
        " }}\nspec fn invocation_source_initial_{root}_v36(a: Seq<int>, p: AggregateStateV30) -> OriginalInvocationRuntimeV36\n recommends invocation_arguments_{root}_v36(a),\n{{\n OriginalInvocationRuntimeV36 {{ frames: byte_root_frame_v30({owner}int), scalar: OriginalControlStateV31 {{ pc: {entry}int, values: Seq::new({locals}nat, |i: int| "
    );
    for &(local, argument) in &body.control.initial {
        out.budget.charge_work(1)?;
        let global = body
            .locals
            .start
            .checked_add(local as usize)
            .ok_or(Resource::Arithmetic)?;
        emit!(out, "if i == {global}int {{ a[{argument}] }} else ");
    }
    emit!(out, "0int), defined: Seq::new({locals}nat, |i: int| false");
    for &(local, _) in &body.control.initial {
        out.budget.charge_work(1)?;
        let global = body
            .locals
            .start
            .checked_add(local as usize)
            .ok_or(Resource::Arithmetic)?;
        emit!(out, " || i == {global}int");
    }
    emit!(
        out,
        "), cells: p.cells, initialized: p.initialized, external: p.external }} }}\n}}\nspec fn invocation_actual_raw_initial_{root}_v36(a: Seq<int>, p: AggregateStateV30) -> AggregateStateV30\n recommends invocation_arguments_{root}_v36(a),\n{{\n AggregateStateV30 {{ pc: 0int, values: Seq::new({definitions}nat, |i: int| "
    );
    for argument in 0..body.control.arguments {
        out.budget.charge_work(1)?;
        let definition = first.checked_add(argument).ok_or(Resource::Arithmetic)?;
        emit!(out, "if i == {definition}int {{ a[{argument}] }} else ");
    }
    emit!(
        out,
        "0int), cells: p.cells, initialized: p.initialized, external: p.external }}\n}}\nspec fn invocation_actual_initial_{root}_v36(a: Seq<int>, p: AggregateStateV30) -> ActualInvocationRuntimeV36\n recommends invocation_arguments_{root}_v36(a),\n{{\n ActualInvocationRuntimeV36 {{ frames: byte_root_frame_v30({owner}int), scalar: actual_invocation_ready_{root}_v36(invocation_actual_raw_initial_{root}_v36(a, p)) }}\n}}\nproof fn invocation_initial_relation_{root}_v36(a: Seq<int>, p: AggregateStateV30)\n requires invocation_arguments_{root}_v36(a),\n ensures invocation_related_{root}_v36(invocation_source_initial_{root}_v36(a, p), invocation_actual_initial_{root}_v36(a, p)),\n{{ }}\n"
    );
    Ok(())
}

fn concrete_proof(root: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        "proof fn invocation_runtime_projection_{root}_v36(o: ActualInvocationRuntimeV36, fuel: nat)\n requires invocation_boundary_{root}_v36(o.scalar),\n ensures cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), o, fuel).state.scalar == cfg_trace_v26(|s: AggregateStateV30| invocation_physical_boundary_step_{root}_v36(s), o.scalar, fuel).state,\n cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), o, fuel).events == cfg_trace_v26(|s: AggregateStateV30| invocation_physical_boundary_step_{root}_v36(s), o.scalar, fuel).events,\n cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), o, fuel).halted == cfg_trace_v26(|s: AggregateStateV30| invocation_physical_boundary_step_{root}_v36(s), o.scalar, fuel).halted,\n decreases fuel,\n{{\n if fuel > 0 {{\n invocation_boundary_concrete_{root}_v36(o.scalar);\n let head = invocation_actual_step_{root}_v36(o);\n if !head.halted {{ invocation_runtime_projection_{root}_v36(head.state, (fuel - 1) as nat); }}\n }}\n}}\nproof fn invocation_original_concrete_trace_{root}_v36(n: OriginalInvocationRuntimeV36, o: ActualInvocationRuntimeV36, fuel: nat)\n requires invocation_related_{root}_v36(n, o),\n ensures cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), n, fuel).events == cfg_trace_v26(|s: AggregateStateV30| invocation_physical_boundary_step_{root}_v36(s), o.scalar, fuel).events,\n cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), n, fuel).halted == cfg_trace_v26(|s: AggregateStateV30| invocation_physical_step_{root}_v36(s), o.scalar, invocation_physical_fuel_{root}_v36(o.scalar, fuel)).halted,\n invocation_related_{root}_v36(\n cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), n, fuel).state,\n ActualInvocationRuntimeV36 {{ scalar: cfg_trace_v26(|s: AggregateStateV30| invocation_physical_step_{root}_v36(s), o.scalar, invocation_physical_fuel_{root}_v36(o.scalar, fuel)).state,\n frames: cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), o, fuel).state.frames }}),\n{{\n invocation_finite_trace_{root}_v36(n, o, fuel);\n invocation_runtime_projection_{root}_v36(o, fuel);\n invocation_concrete_finite_trace_{root}_v36(o.scalar, fuel);\n}}\n"
    );
    initial_concrete_proof(root, out)
}

fn initial_concrete_proof(root: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        "spec fn invocation_initial_physical_fuel_{root}_v36(a: Seq<int>, p: AggregateStateV30, fuel: nat) -> nat\n{{\n actual_invocation_prefix_{root}_v36().len() + invocation_physical_fuel_{root}_v36(invocation_actual_initial_{root}_v36(a, p).scalar, fuel)\n}}\nproof fn invocation_initial_concrete_trace_{root}_v36(a: Seq<int>, p: AggregateStateV30, fuel: nat)\n requires invocation_arguments_{root}_v36(a),\n ensures cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), invocation_source_initial_{root}_v36(a, p), fuel).events == cfg_trace_v26(|s: AggregateStateV30| invocation_physical_boundary_step_{root}_v36(s), invocation_actual_initial_{root}_v36(a, p).scalar, fuel).events,\n cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), invocation_source_initial_{root}_v36(a, p), fuel).halted == cfg_trace_v26(|s: AggregateStateV30| invocation_physical_step_{root}_v36(s), invocation_actual_raw_initial_{root}_v36(a, p), invocation_initial_physical_fuel_{root}_v36(a, p, fuel)).halted,\n invocation_related_{root}_v36(\n cfg_trace_v26(|s: OriginalInvocationRuntimeV36| invocation_source_step_{root}_v36(s), invocation_source_initial_{root}_v36(a, p), fuel).state,\n ActualInvocationRuntimeV36 {{ scalar: cfg_trace_v26(|s: AggregateStateV30| invocation_physical_step_{root}_v36(s), invocation_actual_raw_initial_{root}_v36(a, p), invocation_initial_physical_fuel_{root}_v36(a, p, fuel)).state,\n frames: cfg_trace_v26(|s: ActualInvocationRuntimeV36| invocation_actual_step_{root}_v36(s), invocation_actual_initial_{root}_v36(a, p), fuel).state.frames }}),\n{{\n let n = invocation_source_initial_{root}_v36(a, p);\n let o = invocation_actual_initial_{root}_v36(a, p);\n let raw = invocation_actual_raw_initial_{root}_v36(a, p);\n invocation_initial_relation_{root}_v36(a, p);\n invocation_prefix_concrete_{root}_v36(raw);\n invocation_original_concrete_trace_{root}_v36(n, o, fuel);\n invocation_physical_trace_split_v36(|s: AggregateStateV30| invocation_physical_step_{root}_v36(s), raw, actual_invocation_prefix_{root}_v36().len(), invocation_physical_fuel_{root}_v36(o.scalar, fuel));\n}}\n"
    );
    Ok(())
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<&ActualInvocations<'_, '_, '_, '_, '_, '_>>()
        + h::<&Body>()
        + h::<&ActualBody>()
        + h::<&BlockBindings>()
        + h::<&ConcreteTrace<'_, '_>>()
        + h::<Vec<usize>>()
        + h::<&[(usize, Option<usize>)]>()
        + h::<&[usize]>()
        + h::<Option<(usize, usize)>>()
        + h::<Option<usize>>()
        + h::<&mut Writer<'_, '_>>()
        + 32 * size_of::<usize>()
        + 16 * size_of::<&()>()
}
