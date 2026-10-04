//! Concrete KIR micro-steps and authenticated source-observation projections.
//! A source step can span multiple physical blocks; its fuel is never reused as
//! physical fuel. Edge arguments are evaluated before simultaneous phi writes.
use super::super::super::{
    canonical::control::{TargetBranch, TargetEdge},
    emit_graph_v30,
};
use super::*;
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

fn block_key(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    block: usize,
) -> Result<usize> {
    model.inventory.functions()[model.roots[root].physical]
        .blocks
        .start
        .checked_add(block)
        .ok_or_else(|| Resource::Arithmetic.into())
}

pub(super) fn emit(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    emit!(out, "{COMPOSITION}");
    let definitions = model.inventory.definitions().len();
    for (root, physical) in model.roots.iter().enumerate() {
        out.budget.charge_work(1)?;
        for (block, target) in physical.targets.iter().enumerate() {
            out.budget.charge_work(3)?;
            let key = block_key(model, root, block)?;
            emit_graph_v30(
                &target.program.nodes,
                definitions,
                key,
                "invocation_physical",
                (0..target.program.nodes.len()).map(|node| Ok(Some(node))),
                None,
                out,
            )?;
            emit!(
                out,
                "spec fn invocation_physical_body_{key}_v36(v: Seq<int>) -> Seq<int>\n recommends v.len() == {definitions},\n{{\n let n = original_invocation_physical_trace_{key}_v30(v);\n v"
            );
            for (local, node) in target.program.definitions.iter().enumerate() {
                out.budget.charge_work(2)?;
                if let Some(node) = node {
                    let definition = target
                        .program
                        .definition_start
                        .checked_add(local)
                        .ok_or(Resource::Arithmetic)?;
                    emit!(out, ".update({definition}int, n[{node}])");
                }
            }
            emit!(out, "\n}}\n");
        }
        emit!(
            out,
            "spec fn invocation_physical_step_{root}_v36(s: AggregateStateV30) -> CfgStepV26<AggregateStateV30, int>\n recommends s.values.len() == {definitions},\n{{\n"
        );
        for (block, target) in physical.targets.iter().enumerate() {
            out.budget.charge_work(2)?;
            let key = block_key(model, root, block)?;
            emit!(
                out,
                " if s.pc == {block}int {{\n let n = original_invocation_physical_trace_{key}_v30(s.values);\n let values = invocation_physical_body_{key}_v36(s.values);\n"
            );
            match &target.branch {
                TargetBranch::Goto if target.edges.len() == 1 => edge(&target.edges[0], out)?,
                TargetBranch::Switch {
                    selector,
                    cases,
                    otherwise,
                } => {
                    for &(value, edge_index) in cases {
                        out.budget.charge_work(2)?;
                        emit!(out, " if n[{selector}] == {value}int {{ ");
                        edge(target.edges.get(edge_index).ok_or_else(mismatch)?, out)?;
                        emit!(out, " }} else ");
                    }
                    emit!(out, "{{ ");
                    edge(target.edges.get(*otherwise).ok_or_else(mismatch)?, out)?;
                    emit!(out, " }}");
                }
                TargetBranch::Return if target.edges.is_empty() => {
                    emit!(
                        out,
                        " CfgStepV26 {{ state: AggregateStateV30 {{ pc: -1int, values, cells: s.cells, initialized: s.initialized, external: s.external }}, events: Seq::empty(), halted: true }}"
                    );
                }
                _ => return Err(mismatch()),
            }
            emit!(out, "\n }} else\n");
        }
        emit!(
            out,
            " {{ CfgStepV26 {{ state: s, events: Seq::empty(), halted: true }} }}\n}}\n"
        );
    }
    Ok(())
}

const COMPOSITION: &str = r#"
proof fn invocation_physical_trace_split_v36<S, E>(
    step: spec_fn(S) -> CfgStepV26<S, E>, state: S, first: nat, rest: nat,
)
    requires !cfg_trace_v26(step, state, first).halted,
    ensures cfg_trace_v26(step, state, first + rest).state == cfg_trace_v26(step, cfg_trace_v26(step, state, first).state, rest).state,
        cfg_trace_v26(step, state, first + rest).halted == cfg_trace_v26(step, cfg_trace_v26(step, state, first).state, rest).halted,
    decreases first,
{
    if first > 0 {
        let head = step(state);
        assert(!head.halted);
        invocation_physical_trace_split_v36(step, head.state, (first - 1) as nat, rest);
    }
}
"#;

pub(super) fn compose(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let definitions = model.inventory.definitions().len();
    emit!(
        out,
        "spec fn invocation_boundary_{root}_v36(s: AggregateStateV30) -> bool {{ s.values.len() == {definitions} && (s.pc == -1int"
    );
    for index in model.original.roots[root].clone() {
        out.budget.charge_work(1)?;
        let Some(actual) = &model.bodies[index] else {
            continue;
        };
        out.budget.charge_work(actual.bindings.len())?;
        for binding in actual.bindings.iter().flatten() {
            out.budget.charge_work(1)?;
            emit!(out, " || s.pc == {}int", binding.physical.block);
        }
    }
    emit!(out, ") }}\n");
    for observations in [false, true] {
        let (name, result, default) = if observations {
            ("observations", "Seq<int>", "Seq::empty()")
        } else {
            ("count", "nat", "1nat")
        };
        emit!(
            out,
            "spec fn invocation_boundary_{name}_{root}_v36(s: AggregateStateV30) -> {result} {{\n"
        );
        for index in model.original.roots[root].clone() {
            out.budget.charge_work(1)?;
            let Some(body) = &model.original.bodies[index] else {
                continue;
            };
            let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
            for (block, binding) in actual.bindings.iter().enumerate() {
                out.budget.charge_work(1)?;
                let Some(binding) = binding else { continue };
                let key = body
                    .blocks
                    .start
                    .checked_add(block)
                    .ok_or(Resource::Arithmetic)?;
                emit!(out, " if s.pc == {}int {{\n", binding.physical.block);
                let target = &model.roots[root].targets[binding.physical.block as usize];
                if let TargetBranch::Switch {
                    selector, cases, ..
                } = &target.branch
                {
                    let target_key = block_key(model, root, binding.physical.block as usize)?;
                    emit!(
                        out,
                        " let n = original_invocation_physical_trace_{target_key}_v30(s.values);\n"
                    );
                    for (ordinal, &(value, _)) in cases.iter().enumerate() {
                        out.budget.charge_work(1)?;
                        emit!(out, " if n[{selector}] == {value}int {{ ");
                        chosen(key, ordinal, observations, out)?;
                        emit!(out, " }} else ");
                    }
                    emit!(out, "{{ ");
                    chosen(key, cases.len(), observations, out)?;
                    emit!(out, " }}");
                } else {
                    chosen(key, 0, observations, out)?;
                }
                emit!(out, "\n }} else\n");
            }
        }
        emit!(out, " {{ {default} }}\n}}\n");
    }
    emit!(
        out,
        "spec fn invocation_physical_boundary_step_{root}_v36(s: AggregateStateV30) -> CfgStepV26<AggregateStateV30, int> {{\n let run = cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, invocation_boundary_count_{root}_v36(s));\n CfgStepV26 {{ state: run.state, events: invocation_boundary_observations_{root}_v36(s), halted: run.halted }}\n}}\nproof fn invocation_boundary_concrete_{root}_v36(s: AggregateStateV30)\n requires invocation_boundary_{root}_v36(s),\n ensures invocation_physical_boundary_step_{root}_v36(s) == actual_invocation_step_{root}_v36(s),\n invocation_boundary_{root}_v36(invocation_physical_boundary_step_{root}_v36(s).state),\n invocation_boundary_count_{root}_v36(s) > 0,\n{{\n if s.pc == -1int {{ }} else "
    );
    for index in model.original.roots[root].clone() {
        out.budget.charge_work(1)?;
        let Some(body) = &model.original.bodies[index] else {
            continue;
        };
        let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
        for (block, binding) in actual.bindings.iter().enumerate() {
            out.budget.charge_work(1)?;
            let Some(binding) = binding else { continue };
            let key = body
                .blocks
                .start
                .checked_add(block)
                .ok_or(Resource::Arithmetic)?;
            emit!(out, "if s.pc == {}int {{\n", binding.physical.block);
            let target = &model.roots[root].targets[binding.physical.block as usize];
            if let TargetBranch::Switch {
                selector, cases, ..
            } = &target.branch
            {
                let target_key = block_key(model, root, binding.physical.block as usize)?;
                emit!(
                    out,
                    " let n = original_invocation_physical_trace_{target_key}_v30(s.values);\n"
                );
                for (ordinal, &(value, _)) in cases.iter().enumerate() {
                    out.budget.charge_work(1)?;
                    emit!(
                        out,
                        " if n[{selector}] == {value}int {{ invocation_segment_concrete_{key}_{ordinal}_v36(s); }} else "
                    );
                }
                emit!(
                    out,
                    "{{ invocation_segment_concrete_{key}_{}_v36(s); }}\n",
                    cases.len()
                );
            } else {
                emit!(out, " invocation_segment_concrete_{key}_0_v36(s);\n");
            }
            emit!(out, " }} else ");
        }
    }
    emit!(
        out,
        "{{ assert(false); }}\n}}\nspec fn invocation_physical_fuel_{root}_v36(s: AggregateStateV30, source_fuel: nat) -> nat\n decreases source_fuel,\n{{\n if source_fuel == 0 {{ 0nat }} else {{\n let head = invocation_physical_boundary_step_{root}_v36(s);\n let count = invocation_boundary_count_{root}_v36(s);\n if head.halted {{ count }} else {{ count + invocation_physical_fuel_{root}_v36(head.state, (source_fuel - 1) as nat) }}\n }}\n}}\nproof fn invocation_concrete_finite_trace_{root}_v36(s: AggregateStateV30, source_fuel: nat)\n requires invocation_boundary_{root}_v36(s),\n ensures cfg_trace_v26(|p: AggregateStateV30| invocation_physical_boundary_step_{root}_v36(p), s, source_fuel).state == cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, invocation_physical_fuel_{root}_v36(s, source_fuel)).state,\n cfg_trace_v26(|p: AggregateStateV30| invocation_physical_boundary_step_{root}_v36(p), s, source_fuel).halted == cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, invocation_physical_fuel_{root}_v36(s, source_fuel)).halted,\n decreases source_fuel,\n{{\n if source_fuel > 0 {{\n invocation_boundary_concrete_{root}_v36(s);\n let head = invocation_physical_boundary_step_{root}_v36(s);\n if !head.halted {{\n invocation_concrete_finite_trace_{root}_v36(head.state, (source_fuel - 1) as nat);\n invocation_physical_trace_split_v36(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, invocation_boundary_count_{root}_v36(s), invocation_physical_fuel_{root}_v36(head.state, (source_fuel - 1) as nat));\n }}\n }}\n}}\n"
    );
    Ok(())
}

fn chosen(key: usize, ordinal: usize, observations: bool, out: &mut Writer<'_, '_>) -> Result<()> {
    if observations {
        emit!(
            out,
            "invocation_segment_observations_{key}_{ordinal}_v36(s)"
        );
    } else {
        emit!(out, "actual_invocation_path_{key}_{ordinal}_v36().len()");
    }
    Ok(())
}

fn edge(edge: &TargetEdge, out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        " CfgStepV26 {{ state: AggregateStateV30 {{ pc: {}int, values: values",
        edge.target.block
    );
    for &(definition, expression) in &edge.arguments {
        out.budget.charge_work(2)?;
        emit!(out, ".update({definition}int, n[{expression}])");
    }
    emit!(
        out,
        ", cells: s.cells, initialized: s.initialized, external: s.external }}, events: Seq::empty(), halted: false }}"
    );
    Ok(())
}

pub(super) fn prefix(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    root: usize,
    steps: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(3)?;
    let definitions = model.inventory.definitions().len();
    let unfold = steps.checked_add(1).ok_or(Resource::Arithmetic)?;
    emit!(
        out,
        "proof fn invocation_prefix_concrete_{root}_v36(s: AggregateStateV30)\n requires s.values.len() == {definitions}, s.pc == 0int,\n ensures actual_invocation_ready_{root}_v36(s) == cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, {steps}nat).state,\n !cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, {steps}nat).halted,\n cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, {steps}nat).steps == {steps},\n{{\n reveal_with_fuel(cfg_trace_v26, {unfold});\n assert(actual_invocation_ready_{root}_v36(s).values =~= cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, {steps}nat).state.values);\n}}\n"
    );
    Ok(())
}

pub(super) fn segment(
    model: &ActualInvocations<'_, '_, '_, '_, '_, '_>,
    index: usize,
    block: usize,
    ordinal: usize,
    connector_steps: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(8)?;
    let body = model.original.bodies[index].as_ref().ok_or_else(mismatch)?;
    let actual = model.bodies[index].as_ref().ok_or_else(mismatch)?;
    let binding = actual.bindings[block].as_ref().ok_or_else(mismatch)?;
    let source = body.control.blocks[block].as_ref().ok_or_else(mismatch)?;
    let root = body.root;
    let key = body
        .blocks
        .start
        .checked_add(block)
        .ok_or(Resource::Arithmetic)?;
    let physical = binding.physical.block as usize;
    let target = &model.roots[root].targets[physical];
    let target_key = block_key(model, root, physical)?;
    let steps = connector_steps.checked_add(1).ok_or(Resource::Arithmetic)?;
    let unfold = steps.checked_add(1).ok_or(Resource::Arithmetic)?;
    let definitions = model.inventory.definitions().len();
    emit!(
        out,
        "spec fn invocation_segment_guard_{key}_{ordinal}_v36(s: AggregateStateV30) -> bool {{\n s.pc == {physical}int && s.values.len() == {definitions} && "
    );
    if let TargetBranch::Switch {
        selector, cases, ..
    } = &target.branch
    {
        emit!(
            out,
            "{{ let n = original_invocation_physical_trace_{target_key}_v30(s.values); true"
        );
        for (at, &(value, _)) in cases.iter().enumerate() {
            out.budget.charge_work(1)?;
            if at < ordinal {
                emit!(out, " && n[{selector}] != {value}int");
            }
            if at == ordinal {
                emit!(out, " && n[{selector}] == {value}int");
            }
        }
        emit!(out, " }}");
    } else {
        if ordinal != 0 {
            return Err(mismatch());
        }
        emit!(out, "true");
    }
    emit!(
        out,
        "\n}}\nspec fn invocation_segment_physical_{key}_{ordinal}_v36(s: AggregateStateV30) -> CfgTraceV26<AggregateStateV30, int> {{\n cfg_trace_v26(|p: AggregateStateV30| invocation_physical_step_{root}_v36(p), s, {steps}nat)\n}}\nspec fn invocation_segment_observations_{key}_{ordinal}_v36(s: AggregateStateV30) -> Seq<int> {{\n let body = invocation_physical_body_{target_key}_v36(s.values);\n let end = invocation_segment_physical_{key}_{ordinal}_v36(s);\n seq!["
    );
    for definition in &binding.assignments {
        out.budget.charge_work(1)?;
        match definition {
            Some(definition) => emit!(out, "body[{definition}],"),
            None => emit!(out, "0int,"),
        }
    }
    if source.program.returned.is_some() {
        if body.returned.is_some() {
            let definition = actual.returned.flatten().ok_or_else(mismatch)?;
            emit!(out, "end.state.values[{definition}],");
        } else {
            let node = target.program.returned.ok_or_else(mismatch)?;
            emit!(
                out,
                "original_invocation_physical_trace_{target_key}_v30(s.values)[{node}],"
            );
        }
    }
    emit!(
        out,
        "]\n}}\nproof fn invocation_segment_concrete_{key}_{ordinal}_v36(s: AggregateStateV30)\n requires invocation_segment_guard_{key}_{ordinal}_v36(s),\n ensures actual_invocation_segment_{key}_{ordinal}_v36(s).state == invocation_segment_physical_{key}_{ordinal}_v36(s).state,\n actual_invocation_segment_{key}_{ordinal}_v36(s).halted == invocation_segment_physical_{key}_{ordinal}_v36(s).halted,\n actual_invocation_segment_{key}_{ordinal}_v36(s).events == invocation_segment_observations_{key}_{ordinal}_v36(s),\n invocation_segment_physical_{key}_{ordinal}_v36(s).steps == {steps},\n{{\n reveal_with_fuel(cfg_trace_v26, {unfold});\n assert(actual_invocation_segment_{key}_{ordinal}_v36(s).state.values =~= invocation_segment_physical_{key}_{ordinal}_v36(s).state.values);\n assert(actual_invocation_segment_{key}_{ordinal}_v36(s).events =~= invocation_segment_observations_{key}_{ordinal}_v36(s));\n}}\n"
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
        + h::<&TargetBlock>()
        + h::<&TargetEdge>()
        + h::<&mut Writer<'_, '_>>()
        + 32 * size_of::<usize>()
        + 16 * size_of::<&()>()
}
