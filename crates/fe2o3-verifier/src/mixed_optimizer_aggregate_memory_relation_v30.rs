//! Concrete memory cuts and exact parallel phi assignments. Every local and
//! finite-trace claim below is a generated obligation, never an admitted axiom.
use super::*;

fn assertions(out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        " assert(aggregate_step_n(n, op).events == aggregate_step_o(o, op).events);\n assert(aggregate_step_n(n, op).halted == aggregate_step_o(o, op).halted);\n assert(aggregate_related_v30(aggregate_step_n(n, op).state, aggregate_step_o(o, op).state));\n"
    );
    Ok(())
}

fn selector(input: &Inventory<'_>, block: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    let usage = input.blocks()[block].terminator_uses.start;
    let definition = input.uses()[usage].definition;
    if local(input, definition, block) {
        emit!(out, "n{definition}");
    } else {
        emit!(out, "n.values[{definition}]");
    }
    Ok(())
}

fn block_proof(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    scalar: &Plan,
    memory: &MemoryPlan,
    witness: &Witness,
    block: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        "proof fn aggregate_block_refinement_{block}_v30(n: AggregateStateV30, o: AggregateStateV30, op: spec_fn(int, int, Seq<int>, int) -> int)\n requires aggregate_related_v30(n, o), n.pc == {block},\n ensures aggregate_step_n(n, op).events == aggregate_step_o(o, op).events,\n aggregate_step_n(n, op).halted == aggregate_step_o(o, op).halted,\n aggregate_related_v30(aggregate_step_n(n, op).state, aggregate_step_o(o, op).state),\n{{\n let base = n.values; let initial = n.external; let cells = n.cells; let initialized = n.initialized;\n"
    );
    body_in_memory(
        input,
        output,
        scalar,
        block,
        block,
        Side::Input,
        true,
        Environment::ActualDefinitions,
        Some(witness),
        out,
    )?;
    emit!(
        out,
        " assert(memory_valid);\n let base = o.values; let initial = o.external;\n"
    );
    body_in_memory(
        input,
        output,
        scalar,
        block,
        block,
        Side::Output,
        true,
        Environment::ActualDefinitions,
        None,
        out,
    )?;
    for operation in output.blocks()[block].operations.clone() {
        for definition in output.operations()[operation].results.clone() {
            out.budget.charge_work(2)?;
            let source = memory.output_sources[definition];
            if source == memory_plan::TRUE {
                emit!(out, " assert(o{definition} == 1int);\n");
            } else if source != NONE {
                if source >= input.definitions().len() || !local(input, source, block) {
                    return Err(Error::Statement("aggregate retained local result identity"));
                }
                emit!(out, " assert(o{definition} == n{source});\n");
            }
        }
    }
    match input.blocks()[block].terminator {
        fe2o3_kernel_ir::Terminator::ConditionalBranch { .. } => {
            emit!(out, " if ");
            selector(input, block, out)?;
            emit!(out, " == 1int {{\n");
            assertions(out)?;
            emit!(out, " }} else {{\n");
            assertions(out)?;
            emit!(out, " }}\n");
        }
        fe2o3_kernel_ir::Terminator::Switch { cases, .. } => {
            for case in cases {
                out.budget.charge_work(1)?;
                emit!(out, " if ");
                selector(input, block, out)?;
                emit!(out, " == {}int {{\n", case.value);
                assertions(out)?;
                emit!(out, " }} else ");
            }
            emit!(out, " {{\n");
            assertions(out)?;
            emit!(out, " }}\n");
        }
        fe2o3_kernel_ir::Terminator::IntegerSwitch { cases, .. } => {
            for case in cases {
                out.budget.charge_work(1)?;
                emit!(out, " if ");
                selector(input, block, out)?;
                emit!(out, " == {}int {{\n", bits(&case.value));
                assertions(out)?;
                emit!(out, " }} else ");
            }
            emit!(out, " {{\n");
            assertions(out)?;
            emit!(out, " }}\n");
        }
        _ => assertions(out)?,
    }
    emit!(out, "}}\n");
    Ok(())
}

pub(super) fn generate(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    scalar: &Plan,
    memory: &MemoryPlan,
    witness: &Witness,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        "spec fn aggregate_related_v30(n: AggregateStateV30, o: AggregateStateV30) -> bool {{\n n.values.len() == {} && o.values.len() == {}\n && n.cells.len() == {} && o.cells.len() == {}\n && n.initialized.len() == {} && o.initialized.len() == {}\n && n.pc == o.pc && -1 <= n.pc < {} && n.external == o.external",
        memory.source_values,
        output.definitions().len(),
        memory.slots,
        memory.slots,
        memory.slots,
        memory.slots,
        input.blocks().len()
    );
    for (definition, &source) in memory.output_sources.iter().enumerate() {
        out.budget.charge_work(1)?;
        if source == memory_plan::TRUE {
            emit!(out, "\n && o.values[{definition}] == 1int");
        } else if source != NONE {
            emit!(out, "\n && o.values[{definition}] == n.values[{source}]");
        }
    }
    for block in 0..input.blocks().len() {
        for slot in 0..memory.slots {
            out.budget.charge_work(1)?;
            let needed = memory.requirements[block * memory.slots + slot];
            if needed != NONE {
                emit!(
                    out,
                    "\n && (n.pc == {block} ==> n.initialized[{slot}] && n.cells[{slot}] == o.values[{needed}])"
                );
            }
        }
    }
    emit!(out, "\n}}\n");
    for block in 0..input.blocks().len() {
        block_proof(input, output, scalar, memory, witness, block, out)?;
    }
    emit!(
        out,
        "proof fn aggregate_all_steps_refine_v30(op: spec_fn(int, int, Seq<int>, int) -> int)\n ensures cfg_step_simulates_v26(|n: AggregateStateV30| aggregate_step_n(n, op), |o: AggregateStateV30| aggregate_step_o(o, op), |n: AggregateStateV30, o: AggregateStateV30| aggregate_related_v30(n, o)),\n{{\n assert forall|n: AggregateStateV30, o: AggregateStateV30| #[trigger] aggregate_related_v30(n, o) implies\n aggregate_step_n(n, op).events == aggregate_step_o(o, op).events\n && aggregate_step_n(n, op).halted == aggregate_step_o(o, op).halted\n && aggregate_related_v30(aggregate_step_n(n, op).state, aggregate_step_o(o, op).state) by {{\n if n.pc == -1 {{ }} else "
    );
    for block in 0..input.blocks().len() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "if n.pc == {block} {{ aggregate_block_refinement_{block}_v30(n, o, op); }} else "
        );
    }
    emit!(out, "{{ assert(false); }}\n }}\n}}\n");
    // Only non-argument virtual registers are initialized by this conversion.
    // Every actual argument is mapped by its checked original SSA definition.
    emit!(
        out,
        "spec fn aggregate_initial_output_v30(n: AggregateStateV30) -> AggregateStateV30\n recommends n.values.len() == {},\n{{\n let values = Seq::new({}, |i: int| ",
        memory.source_values,
        output.definitions().len()
    );
    for (definition, &source) in memory.output_sources.iter().enumerate() {
        out.budget.charge_work(1)?;
        if source == NONE {
            continue;
        }
        emit!(out, "if i == {definition} {{ ");
        if source == memory_plan::TRUE {
            emit!(out, "1int");
        } else {
            emit!(out, "n.values[{source}]");
        }
        emit!(out, " }} else ");
    }
    emit!(
        out,
        "{{ 0int }});\n AggregateStateV30 {{ pc: n.pc, values, cells: n.cells, initialized: n.initialized, external: n.external }}\n}}\n"
    );
    Ok(())
}
