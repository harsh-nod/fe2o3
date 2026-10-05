//! Erasure of proof-only phi snapshots, exact argument entry, and instantiation
//! of the existing finite-trace theorem. No executed evidence is produced here.
use super::*;

pub(super) fn generate(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    memory: &MemoryPlan,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let original_values = input.definitions().len();
    let source_values = memory.source_values;
    let slots = memory.slots;
    let blocks = input.blocks().len();
    emit!(
        out,
        "struct AggregatePhysicalStateV30 {{ pc: int, values: Seq<int>, cells: Seq<int>, initialized: Seq<bool>, external: int }}\n"
    );
    emit!(
        out,
        "spec fn aggregate_physical_frame_v30(p: AggregatePhysicalStateV30) -> bool {{ p.values.len() == {original_values} && p.cells.len() == {slots} && p.initialized.len() == {slots} && -1 <= p.pc < {blocks} }}\n"
    );
    emit!(
        out,
        "spec fn aggregate_erase_v30(n: AggregateStateV30) -> AggregatePhysicalStateV30\n recommends n.values.len() == {source_values},\n{{ AggregatePhysicalStateV30 {{ pc: n.pc, values: n.values.subrange(0, {original_values}), cells: n.cells, initialized: n.initialized, external: n.external }} }}\n"
    );
    emit!(
        out,
        "spec fn aggregate_lift_v30(p: AggregatePhysicalStateV30) -> AggregateStateV30\n recommends aggregate_physical_frame_v30(p),\n{{ AggregateStateV30 {{ pc: p.pc, values: p.values + Seq::new({}, |i: int| 0int), cells: p.cells, initialized: p.initialized, external: p.external }} }}\n",
        source_values - original_values
    );
    // The physical interpreter is the exact owner-derived source step with
    // snapshots erased. The commutation theorem below proves independence from
    // every snapshot value; the reset to zero is not a hidden source premise.
    emit!(
        out,
        "spec fn aggregate_physical_step_v30(p: AggregatePhysicalStateV30, op: spec_fn(int, int, Seq<int>, int) -> int) -> CfgStepV26<AggregatePhysicalStateV30, int>\n recommends aggregate_physical_frame_v30(p),\n{{ let step = aggregate_step_n(aggregate_lift_v30(p), op); CfgStepV26 {{ state: aggregate_erase_v30(step.state), events: step.events, halted: step.halted }} }}\n"
    );
    emit!(
        out,
        "spec fn aggregate_erasure_related_v30(p: AggregatePhysicalStateV30, n: AggregateStateV30) -> bool {{ aggregate_physical_frame_v30(p) && n.values.len() == {source_values} && p == aggregate_erase_v30(n) }}\n"
    );
    emit!(
        out,
        "proof fn aggregate_snapshots_do_not_affect_original_step_v30(p: AggregatePhysicalStateV30, n: AggregateStateV30, op: spec_fn(int, int, Seq<int>, int) -> int)\n requires aggregate_erasure_related_v30(p, n),\n ensures aggregate_physical_step_v30(p, op).events == aggregate_step_n(n, op).events,\n aggregate_physical_step_v30(p, op).halted == aggregate_step_n(n, op).halted,\n aggregate_erasure_related_v30(aggregate_physical_step_v30(p, op).state, aggregate_step_n(n, op).state),\n{{\n if n.pc == -1 {{ }} else "
    );
    for block in 0..blocks {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "if n.pc == {block} {{\n assert(aggregate_physical_step_v30(p, op).state.values =~= aggregate_erase_v30(aggregate_step_n(n, op).state).values);\n }} else "
        );
    }
    emit!(out, "{{ assert(false); }}\n}}\n");
    emit!(
        out,
        "proof fn aggregate_erasure_all_steps_v30(op: spec_fn(int, int, Seq<int>, int) -> int)\n ensures cfg_step_simulates_v26(|p: AggregatePhysicalStateV30| aggregate_physical_step_v30(p, op), |n: AggregateStateV30| aggregate_step_n(n, op), |p: AggregatePhysicalStateV30, n: AggregateStateV30| aggregate_erasure_related_v30(p, n)),\n{{\n assert forall|p: AggregatePhysicalStateV30, n: AggregateStateV30| #[trigger] aggregate_erasure_related_v30(p, n) implies\n aggregate_physical_step_v30(p, op).events == aggregate_step_n(n, op).events\n && aggregate_physical_step_v30(p, op).halted == aggregate_step_n(n, op).halted\n && aggregate_erasure_related_v30(aggregate_physical_step_v30(p, op).state, aggregate_step_n(n, op).state) by {{\n aggregate_snapshots_do_not_affect_original_step_v30(p, n, op);\n }}\n}}\n"
    );
    for function in input.functions() {
        out.budget.charge_work(1)?;
        if function.blocks.is_empty() {
            continue;
        }
        let f = function.coordinate.0;
        let entry = function.blocks.start;
        let corresponding = output
            .functions()
            .get(f as usize)
            .ok_or(Error::Statement("aggregate exact output function"))?;
        if corresponding.blocks.start != entry
            || function.function.signature != corresponding.function.signature
        {
            return Err(Error::Statement(
                "aggregate exact function signature and entry",
            ));
        }
        for side in [Side::Input, Side::Output] {
            let label = side.label();
            let (inv, state, values) = match side {
                Side::Input => (input, "AggregatePhysicalStateV30", original_values),
                Side::Output => (output, "AggregateStateV30", output.definitions().len()),
            };
            emit!(
                out,
                "spec fn aggregate_entry_{label}_{f}_v30(s: {state}, arguments: Seq<int>) -> bool {{ s.pc == {entry} && s.values.len() == {values} && s.cells.len() == {slots} && s.initialized.len() == {slots} && arguments.len() == {}",
                function.function.signature.parameters.len()
            );
            for argument in 0..function.function.signature.parameters.len() {
                out.budget.charge_work(2)?;
                let definition = definition_index(
                    inv,
                    Definition::FunctionArgument {
                        function: function.coordinate,
                        argument: u32::try_from(argument).map_err(|_| Resource::Arithmetic)?,
                    },
                )?;
                if matches!(side, Side::Output) {
                    let original = definition_index(
                        input,
                        Definition::FunctionArgument {
                            function: function.coordinate,
                            argument: u32::try_from(argument).map_err(|_| Resource::Arithmetic)?,
                        },
                    )?;
                    if memory.output_sources[definition] != original {
                        return Err(Error::Statement("aggregate exact argument value mapping"));
                    }
                }
                emit!(out, " && s.values[{definition}] == arguments[{argument}]");
            }
            emit!(out, " }}\n");
        }
        emit!(
            out,
            "proof fn aggregate_initial_relation_{f}_v30(p: AggregatePhysicalStateV30, arguments: Seq<int>)\n requires aggregate_entry_n_{f}_v30(p, arguments),\n ensures aggregate_erasure_related_v30(p, aggregate_lift_v30(p)),\n aggregate_entry_o_{f}_v30(aggregate_initial_output_v30(aggregate_lift_v30(p)), arguments),\n aggregate_related_v30(aggregate_lift_v30(p), aggregate_initial_output_v30(aggregate_lift_v30(p))),\n{{\n assert(aggregate_erase_v30(aggregate_lift_v30(p)).values =~= p.values);\n}}\n"
        );
        emit!(
            out,
            "proof fn aggregate_function_trace_refinement_{f}_v30(p: AggregatePhysicalStateV30, arguments: Seq<int>, op: spec_fn(int, int, Seq<int>, int) -> int, fuel: nat)\n requires aggregate_entry_n_{f}_v30(p, arguments),\n ensures cfg_trace_v26(|s: AggregatePhysicalStateV30| aggregate_physical_step_v30(s, op), p, fuel).events == cfg_trace_v26(|s: AggregateStateV30| aggregate_step_o(s, op), aggregate_initial_output_v30(aggregate_lift_v30(p)), fuel).events,\n cfg_trace_v26(|s: AggregatePhysicalStateV30| aggregate_physical_step_v30(s, op), p, fuel).halted == cfg_trace_v26(|s: AggregateStateV30| aggregate_step_o(s, op), aggregate_initial_output_v30(aggregate_lift_v30(p)), fuel).halted,\n cfg_trace_v26(|s: AggregatePhysicalStateV30| aggregate_physical_step_v30(s, op), p, fuel).steps == cfg_trace_v26(|s: AggregateStateV30| aggregate_step_o(s, op), aggregate_initial_output_v30(aggregate_lift_v30(p)), fuel).steps,\n{{\n aggregate_initial_relation_{f}_v30(p, arguments);\n aggregate_erasure_all_steps_v30(op);\n cfg_finite_trace_refinement_v26(|s: AggregatePhysicalStateV30| aggregate_physical_step_v30(s, op), |s: AggregateStateV30| aggregate_step_n(s, op), |a: AggregatePhysicalStateV30, b: AggregateStateV30| aggregate_erasure_related_v30(a, b), p, aggregate_lift_v30(p), fuel);\n aggregate_all_steps_refine_v30(op);\n cfg_finite_trace_refinement_v26(|s: AggregateStateV30| aggregate_step_n(s, op), |s: AggregateStateV30| aggregate_step_o(s, op), |a: AggregateStateV30, b: AggregateStateV30| aggregate_related_v30(a, b), aggregate_lift_v30(p), aggregate_initial_output_v30(aggregate_lift_v30(p)), fuel);\n}}\n"
        );
    }
    emit!(
        out,
        "// Generated obligations only. Successful Verus execution remains unfulfilled.\n// This concrete selected-private-memory boundary does not prove MIR lowering,\n// external device/call interpretation, LLVM correctness, or publication authority.\n"
    );
    Ok(())
}
