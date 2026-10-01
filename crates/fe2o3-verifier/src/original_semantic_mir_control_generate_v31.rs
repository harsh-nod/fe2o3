//! Concrete whole-CFG source/canonical steps and their finite-trace obligation.
//! Pure control preserves the shared physical memory fields; it does not admit
//! any source memory operation or identify unrelated allocation-slot models.

use super::{
    Error, Resource, Result, ScalarV30, Writer,
    canonical::control::TargetBranch,
    control::Branch,
    control_source::{CompleteControl, RootControl},
    emit_graph_v30, relation,
};
use fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18 as Correspondence;
use std::{fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) const SOURCE_STATE: &str = r#"
// MIR locals are distinct from canonical SSA definitions. Undefined/dead locals
// are tracked explicitly; operations below are read from the original MIR.
struct OriginalControlStateV31 {
    pc: int,
    values: Seq<int>,
    defined: Seq<bool>,
    cells: Seq<int>,
    initialized: Seq<bool>,
    external: int,
}
"#;

fn headers() -> usize {
    size_of::<CompleteControl>()
        + size_of::<Result<CompleteControl>>()
        + size_of::<[usize; 6]>()
        + size_of::<Result<[usize; 6]>>()
        + size_of::<(&Correspondence<'_>, &mut Writer<'_, '_>)>()
        + size_of::<(&RootControl, &mut Writer<'_, '_>)>()
        + size_of::<(
            &super::control::SourceBlock,
            &super::control_pair::BlockBindings,
            &super::control_pair::BlockPair,
            &super::canonical::control::TargetBlock,
        )>()
        + 12 * size_of::<usize>()
        + size_of::<Option<usize>>()
        + 4 * size_of::<Result<()>>()
}

pub(crate) fn generate(
    relation: &Correspondence<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<[usize; 6]> {
    out.budget.reserve_storage(headers())?;
    let model = CompleteControl::derive(relation, out)?;
    let definitions = model.census[5];
    emit!(
        out,
        "use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {{\n"
    );
    relation::emit_prelude(out)?;
    emit!(
        out,
        "{}{}{}",
        super::super::structured_state_v30::STATE,
        SOURCE_STATE,
        super::super::cfg_trace::PRELUDE
    );
    for (root, model) in model.roots.iter().enumerate() {
        block_graphs(model, definitions, out)?;
        source_step(root, model, out)?;
        target_step(root, model, out)?;
        state_relation(root, model, definitions, out)?;
        initial(root, model, definitions, out)?;
        proofs(root, model, out)?;
    }
    emit!(out, "}}\n");
    Ok(model.census)
}

fn block_graphs(model: &RootControl, definitions: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    for (block, binding) in model.bindings.iter().enumerate() {
        let Some(binding) = binding else { continue };
        let original = model.original.blocks[block]
            .as_ref()
            .ok_or(Resource::Accounting)?;
        let key = model
            .block_start
            .checked_add(binding.physical.block as usize)
            .ok_or(Resource::Arithmetic)?;
        emit_graph_v30(
            &original.program.nodes,
            model.original.locals,
            key,
            "source_control",
            (0..original.program.nodes.len()).map(|node| Ok(Some(node))),
            None,
            out,
        )?;
    }
    for (block, target) in model.targets.iter().enumerate() {
        let key = model
            .block_start
            .checked_add(block)
            .ok_or(Resource::Arithmetic)?;
        emit_graph_v30(
            &target.program.nodes,
            definitions,
            key,
            "target_control",
            (0..target.program.nodes.len()).map(|node| Ok(Some(node))),
            None,
            out,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_mir_cfg_generator_frames_have_an_independent_field_oracle() {
        type CompleteFields = (Vec<RootControl>, [usize; 6]);
        assert_eq!(size_of::<CompleteControl>(), size_of::<CompleteFields>());
        let expected = size_of::<CompleteFields>()
            + size_of::<Result<CompleteControl>>()
            + size_of::<[usize; 6]>()
            + size_of::<Result<[usize; 6]>>()
            + 8 * size_of::<&()>()
            + 12 * size_of::<usize>()
            + size_of::<Option<usize>>()
            + 4 * size_of::<Result<()>>();
        assert_eq!(headers(), expected);
    }
}

fn source_step(root: usize, model: &RootControl, out: &mut Writer<'_, '_>) -> Result<()> {
    let locals = model.original.locals;
    emit!(
        out,
        "open spec fn original_source_step_{root}_v31(s: OriginalControlStateV31) -> CfgStepV26<OriginalControlStateV31, int>\n recommends s.values.len() == {locals}, s.defined.len() == {locals},\n{{\n"
    );
    for (block, binding) in model.bindings.iter().enumerate() {
        let Some(binding) = binding else { continue };
        let original = model.original.blocks[block]
            .as_ref()
            .ok_or(Resource::Accounting)?;
        let key = model.block_start + binding.physical.block as usize;
        emit!(
            out,
            " if s.pc == {block}int {{\n let n = original_source_control_trace_{key}_v30(s.values);\n let values = s.values"
        );
        for (local, (&changed, &node)) in original
            .changed
            .iter()
            .zip(&original.program.locals)
            .enumerate()
        {
            out.budget.charge_work(1)?;
            if let (true, Some(node)) = (changed, node) {
                emit!(out, ".update({local}int, n[{node}])");
            }
        }
        emit!(out, ";\n let defined = s.defined");
        for (local, (&changed, node)) in original
            .changed
            .iter()
            .zip(&original.program.locals)
            .enumerate()
        {
            out.budget.charge_work(1)?;
            if changed {
                emit!(out, ".update({local}int, {})", node.is_some());
            }
        }
        emit!(out, ";\n let pc: int = ");
        match &original.branch {
            Branch::Goto(next) => emit!(out, "{}int", next.get()),
            Branch::Switch {
                selector,
                cases,
                otherwise,
            } => {
                for &(value, next) in cases {
                    out.budget.charge_work(1)?;
                    emit!(
                        out,
                        "if n[{selector}] == {value}int {{ {}int }} else ",
                        next.get()
                    );
                }
                emit!(out, "{}int", otherwise.get());
            }
            Branch::Return => emit!(out, "-1int"),
            Branch::Call { .. } => {
                return Err(Error::Statement(
                    "original MIR actual call splice is not modeled",
                ));
            }
        }
        emit!(
            out,
            ";\n CfgStepV26 {{ state: OriginalControlStateV31 {{ pc, values, defined, cells: s.cells, initialized: s.initialized, external: s.external }},\n events: seq!["
        );
        for assignment in &original.program.assignments {
            out.budget.charge_work(1)?;
            emit!(out, "n[{}],", assignment.value);
        }
        if let Some(returned) = original.program.returned {
            emit!(out, "n[{returned}],");
        }
        emit!(
            out,
            "], halted: {} }}\n }} else\n",
            matches!(original.branch, Branch::Return)
        );
    }
    emit!(
        out,
        " {{ CfgStepV26 {{ state: s, events: Seq::empty(), halted: true }} }}\n}}\n"
    );
    Ok(())
}

fn target_step(root: usize, model: &RootControl, out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        "open spec fn original_target_step_{root}_v31(s: AggregateStateV30) -> CfgStepV26<AggregateStateV30, int>\n{{\n"
    );
    for (block, target) in model.targets.iter().enumerate() {
        let key = model.block_start + block;
        emit!(
            out,
            " if s.pc == {key}int {{\n let n = original_target_control_trace_{key}_v30(s.values);\n let values = s.values"
        );
        for (definition, &node) in target.program.definitions.iter().enumerate() {
            out.budget.charge_work(1)?;
            if let Some(node) = node {
                emit!(
                    out,
                    ".update({}int, n[{node}])",
                    model.definition_start + definition
                );
            }
        }
        emit!(out, ";\n");
        let return_block = matches!(target.branch, TargetBranch::Return);
        if return_block {
            emit!(out, " let pc = -1int;\n");
        } else {
            emit!(out, " let edge: int = ");
            match &target.branch {
                TargetBranch::Goto => emit!(out, "0int"),
                TargetBranch::Switch {
                    selector,
                    cases,
                    otherwise,
                } => {
                    for &(value, edge) in cases {
                        out.budget.charge_work(1)?;
                        emit!(out, "if n[{selector}] == {value}int {{ {edge}int }} else ");
                    }
                    emit!(out, "{otherwise}int");
                }
                TargetBranch::Return => return Err(Resource::Accounting.into()),
            }
            emit!(out, ";\n let pc = ");
            for (edge, successor) in target.edges.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    "if edge == {edge}int {{ {}int }} else ",
                    model.block_start + successor.target.block as usize
                );
            }
            emit!(out, "-2int;\n let values = ");
            for (edge, successor) in target.edges.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(out, "if edge == {edge}int {{ values");
                for &(definition, value) in &successor.arguments {
                    out.budget.charge_work(1)?;
                    emit!(out, ".update({definition}int, n[{value}])");
                }
                emit!(out, " }} else ");
            }
            emit!(out, "values;\n");
        }
        emit!(
            out,
            " CfgStepV26 {{ state: AggregateStateV30 {{ pc, values, cells: s.cells, initialized: s.initialized, external: s.external }},\n events: seq!["
        );
        // The instrumentation is selected only by retained original assignment
        // locators; invocation-prefix operations have no original Assign event.
        out.budget.charge_work(1)?;
        if let Some(source) = *model
            .original_blocks
            .get(block)
            .ok_or(Resource::Accounting)?
        {
            let pair = model.pairs[source].as_ref().ok_or(Resource::Accounting)?;
            for &node in &pair.target_observations {
                out.budget.charge_work(1)?;
                if let Some(node) = node {
                    emit!(out, "n[{node}],");
                } else {
                    emit!(out, "0int,");
                }
            }
            if let Some(returned) = target.program.returned {
                emit!(out, "n[{returned}],");
            }
        }
        emit!(out, "], halted: {return_block} }}\n }} else\n");
    }
    emit!(
        out,
        " {{ CfgStepV26 {{ state: s, events: Seq::empty(), halted: true }} }}\n}}\n"
    );
    Ok(())
}

fn state_relation(
    root: usize,
    model: &RootControl,
    definitions: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let locals = model.original.locals;
    emit!(
        out,
        "open spec fn original_control_related_{root}_v31(n: OriginalControlStateV31, o: AggregateStateV30) -> bool {{\n n.values.len() == {locals} && n.defined.len() == {locals} && o.values.len() == {definitions}\n && n.cells == o.cells && n.initialized == o.initialized && n.external == o.external\n && (if n.pc == -1int {{ o.pc == -1int }} else "
    );
    for (block, binding) in model.bindings.iter().enumerate() {
        let Some(binding) = binding else { continue };
        emit!(
            out,
            "if n.pc == {block}int {{ o.pc == {}int",
            model.block_start + binding.physical.block as usize
        );
        for &(local, definition) in &binding.live {
            out.budget.charge_work(2)?;
            let scalar = model.original.types[local as usize];
            let modulus = 1u128 << scalar.width();
            emit!(
                out,
                "\n && n.defined[{local}] && 0 <= n.values[{local}] < {modulus}int"
            );
            match definition {
                Some(definition) => emit!(out, " && n.values[{local}] == o.values[{definition}]"),
                None if scalar == ScalarV30::Unit => emit!(out, " && n.values[{local}] == 0int"),
                None => {
                    return Err(Error::Statement(
                        "original MIR control live scalar endpoint absent",
                    ));
                }
            }
        }
        emit!(out, " }} else ");
    }
    emit!(out, "false)\n}}\n");
    Ok(())
}

fn initial(
    root: usize,
    model: &RootControl,
    definitions: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let source = &model.original;
    emit!(
        out,
        "open spec fn original_control_arguments_{root}_v31(a: Seq<int>) -> bool {{ a.len() == {}",
        source.arguments
    );
    for &(local, argument) in &source.initial {
        out.budget.charge_work(2)?;
        let modulus = 1u128 << source.types[local as usize].width();
        emit!(out, " && 0 <= a[{argument}] < {modulus}int");
    }
    emit!(
        out,
        " }}\nopen spec fn original_control_source_initial_{root}_v31(a: Seq<int>, p: AggregateStateV30) -> OriginalControlStateV31 {{\n OriginalControlStateV31 {{ pc: {}int, values: Seq::new({}nat, |i: int| ",
        source.entry.get(),
        source.locals
    );
    for &(local, argument) in &source.initial {
        emit!(out, "if i == {local}int {{ a[{argument}] }} else ");
    }
    emit!(
        out,
        "0int), defined: Seq::new({}nat, |i: int| false",
        source.locals
    );
    for &(local, _) in &source.initial {
        emit!(out, " || i == {local}int");
    }
    emit!(
        out,
        "), cells: p.cells, initialized: p.initialized, external: p.external }}\n}}\nopen spec fn original_control_target_initial_{root}_v31(a: Seq<int>, p: AggregateStateV30) -> AggregateStateV30 {{\n AggregateStateV30 {{ pc: {}int, values: Seq::new({definitions}nat, |i: int| ",
        model.block_start
    );
    for argument in 0..source.arguments {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "if i == {}int {{ a[{argument}] }} else ",
            model.definition_start + argument
        );
    }
    emit!(
        out,
        "0int), cells: p.cells, initialized: p.initialized, external: p.external }}\n}}\nopen spec fn original_control_ready_{root}_v31(a: Seq<int>, p: AggregateStateV30) -> AggregateStateV30 {{\n let s0 = original_control_target_initial_{root}_v31(a, p);\n"
    );
    for at in 0..model.prefix.len() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " let s{} = original_target_step_{root}_v31(s{at}).state;\n",
            at + 1
        );
    }
    emit!(out, " s{}\n}}\n", model.prefix.len());
    Ok(())
}

fn proofs(root: usize, model: &RootControl, out: &mut Writer<'_, '_>) -> Result<()> {
    let prefix = model.prefix.len();
    emit!(
        out,
        "proof fn original_control_initial_relation_{root}_v31(a: Seq<int>, p: AggregateStateV30)\n requires original_control_arguments_{root}_v31(a),\n ensures original_control_related_{root}_v31(original_control_source_initial_{root}_v31(a, p), original_control_ready_{root}_v31(a, p)),\n{{\n"
    );
    emit!(
        out,
        " let s0 = original_control_target_initial_{root}_v31(a, p);\n"
    );
    for (at, &block) in model.prefix.iter().enumerate() {
        out.budget.charge_work(2)?;
        emit!(
            out,
            " assert(s{at}.pc == {}int);\n assert(original_target_step_{root}_v31(s{at}).events == Seq::<int>::empty());\n assert(!original_target_step_{root}_v31(s{at}).halted);\n let s{} = original_target_step_{root}_v31(s{at}).state;\n",
            model.block_start + block,
            at + 1
        );
    }
    emit!(
        out,
        "}}\nproof fn original_control_step_relation_{root}_v31(n: OriginalControlStateV31, o: AggregateStateV30)\n requires original_control_related_{root}_v31(n, o),\n ensures original_source_step_{root}_v31(n).events == original_target_step_{root}_v31(o).events,\n original_source_step_{root}_v31(n).halted == original_target_step_{root}_v31(o).halted,\n original_control_related_{root}_v31(original_source_step_{root}_v31(n).state, original_target_step_{root}_v31(o).state),\n{{\n if n.pc == -1int {{ }} else "
    );
    for (block, binding) in model.bindings.iter().enumerate() {
        let Some(binding) = binding else { continue };
        let original = model.original.blocks[block]
            .as_ref()
            .ok_or(Resource::Accounting)?;
        let pair = model.pairs[block].as_ref().ok_or(Resource::Accounting)?;
        let key = model.block_start + binding.physical.block as usize;
        emit!(
            out,
            "if n.pc == {block}int {{\n let a = original_source_control_trace_{key}_v30(n.values);\n let b = original_target_control_trace_{key}_v30(o.values);\n"
        );
        for (&left, &right) in pair
            .source_observations
            .iter()
            .zip(&pair.target_observations)
        {
            out.budget.charge_work(1)?;
            if let Some(right) = right {
                emit!(out, " assert(a[{left}] == b[{right}]);\n");
            } else {
                emit!(out, " assert(a[{left}] == 0int);\n");
            }
        }
        if let Some(left) = original.program.returned {
            let right = model.targets[binding.physical.block as usize]
                .program
                .returned
                .ok_or(Resource::Accounting)?;
            emit!(out, " assert(a[{left}] == b[{right}]);\n");
        }
        emit!(
            out,
            " assert(original_source_step_{root}_v31(n).events =~= original_target_step_{root}_v31(o).events);\n }} else "
        );
    }
    emit!(
        out,
        "{{ assert(false); }}\n}}\nproof fn original_control_all_steps_{root}_v31()\n ensures cfg_step_simulates_v26(|n: OriginalControlStateV31| original_source_step_{root}_v31(n), |o: AggregateStateV30| original_target_step_{root}_v31(o), |n: OriginalControlStateV31, o: AggregateStateV30| original_control_related_{root}_v31(n, o)),\n{{\n assert forall|n: OriginalControlStateV31, o: AggregateStateV30| #[trigger] original_control_related_{root}_v31(n, o) implies\n original_source_step_{root}_v31(n).events == original_target_step_{root}_v31(o).events\n && original_source_step_{root}_v31(n).halted == original_target_step_{root}_v31(o).halted\n && original_control_related_{root}_v31(original_source_step_{root}_v31(n).state, original_target_step_{root}_v31(o).state) by {{ original_control_step_relation_{root}_v31(n, o); }}\n}}\n"
    );
    emit!(
        out,
        "proof fn original_control_invocation_trace_{root}_v31(a: Seq<int>, p: AggregateStateV30, fuel: nat)\n requires original_control_arguments_{root}_v31(a),\n ensures\n cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).events == cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_ready_{root}_v31(a, p), fuel).events,\n cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).halted == cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_ready_{root}_v31(a, p), fuel).halted,\n cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).state == cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_ready_{root}_v31(a, p), fuel).state,\n cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).steps == {prefix}nat + cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_ready_{root}_v31(a, p), fuel).steps,\n{{\n reveal_with_fuel(cfg_trace_v26, {});\n let s0 = original_control_target_initial_{root}_v31(a, p);\n",
        prefix.checked_add(1).ok_or(Resource::Arithmetic)?
    );
    for (at, &block) in model.prefix.iter().enumerate() {
        out.budget.charge_work(2)?;
        emit!(
            out,
            " assert(s{at}.pc == {}int);\n assert(original_target_step_{root}_v31(s{at}).events == Seq::<int>::empty());\n assert(!original_target_step_{root}_v31(s{at}).halted);\n let s{} = original_target_step_{root}_v31(s{at}).state;\n",
            model.block_start + block,
            at + 1
        );
    }
    emit!(
        out,
        "}}\nproof fn original_mir_cfg_refines_canonical_{root}_v31(a: Seq<int>, p: AggregateStateV30, fuel: nat)\n requires original_control_arguments_{root}_v31(a),\n ensures\n cfg_trace_v26(|s: OriginalControlStateV31| original_source_step_{root}_v31(s), original_control_source_initial_{root}_v31(a, p), fuel).events == cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).events,\n cfg_trace_v26(|s: OriginalControlStateV31| original_source_step_{root}_v31(s), original_control_source_initial_{root}_v31(a, p), fuel).halted == cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).halted,\n cfg_trace_v26(|s: OriginalControlStateV31| original_source_step_{root}_v31(s), original_control_source_initial_{root}_v31(a, p), fuel).steps + {prefix}nat == cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).steps,\n original_control_related_{root}_v31(cfg_trace_v26(|s: OriginalControlStateV31| original_source_step_{root}_v31(s), original_control_source_initial_{root}_v31(a, p), fuel).state, cfg_trace_v26(|s: AggregateStateV30| original_target_step_{root}_v31(s), original_control_target_initial_{root}_v31(a, p), fuel + {prefix}nat).state),\n{{\n original_control_initial_relation_{root}_v31(a, p);\n original_control_all_steps_{root}_v31();\n cfg_finite_trace_refinement_v26(|n: OriginalControlStateV31| original_source_step_{root}_v31(n), |o: AggregateStateV30| original_target_step_{root}_v31(o), |n: OriginalControlStateV31, o: AggregateStateV30| original_control_related_{root}_v31(n, o), original_control_source_initial_{root}_v31(a, p), original_control_ready_{root}_v31(a, p), fuel);\n original_control_invocation_trace_{root}_v31(a, p, fuel);\n}}\n"
    );
    Ok(())
}
