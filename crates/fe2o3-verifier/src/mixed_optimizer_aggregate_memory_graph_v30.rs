//! Exact owner-derived block steps, with proof-only memory snapshots for new
//! phi parameters. Snapshots never drive original operations or control flow.
use super::*;
use fe2o3_kernel_ir::Terminator;

pub(super) use super::super::structured_state_v30::STATE;

fn value(
    inv: &Inventory<'_>,
    scalar: &Plan,
    block: usize,
    side: Side,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    value_in(
        inv,
        scalar,
        block,
        side,
        Environment::ActualDefinitions,
        definition,
        out,
    )
}
fn terminal_value(
    inv: &Inventory<'_>,
    scalar: &Plan,
    block: usize,
    side: Side,
    ordinal: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let row = &inv.blocks()[block];
    let usage = row
        .terminator_uses
        .start
        .checked_add(ordinal)
        .ok_or(Resource::Arithmetic)?;
    if usage >= row.terminator_uses.end {
        return Err(Error::Statement("aggregate terminator occurrence"));
    }
    value(inv, scalar, block, side, inv.uses()[usage].definition, out)
}
fn state_tail(side: Side, pc: Option<usize>, out: &mut Writer<'_, '_>) -> Result<()> {
    let label = side.label();
    let (cells, initialized) = match side {
        Side::Input => ("private", "initialized"),
        Side::Output => ("cells", "initialized"),
    };
    emit!(out, "AggregateStateV30 {{ pc: ");
    match pc {
        Some(target) => emit!(out, "{target}"),
        None => emit!(out, "-1"),
    }
    emit!(
        out,
        ", values: next, cells: {cells}, initialized: {initialized}, external: {label}_final }}"
    );
    Ok(())
}
fn edge(
    inv: &Inventory<'_>,
    scalar: &Plan,
    memory: &MemoryPlan,
    witness: &Witness,
    block: usize,
    side: Side,
    ordinal: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let index = inv.blocks()[block]
        .edges
        .start
        .checked_add(ordinal)
        .ok_or(Resource::Arithmetic)?;
    if index >= inv.blocks()[block].edges.end {
        return Err(Error::Statement("aggregate edge occurrence"));
    }
    let edge = &inv.edges()[index];
    let target = block_index(inv, edge.target)?;
    emit!(out, "{{\n");
    for binding in edge.bindings.clone() {
        out.budget.charge_work(2)?;
        let binding = &inv.edge_arguments()[binding];
        emit!(
            out,
            " let next = next.update({}, ",
            binding.target_definition
        );
        value(inv, scalar, block, side, binding.incoming_definition, out)?;
        emit!(out, ");\n");
    }
    if matches!(side, Side::Input) {
        for link in witness.memory_parameters() {
            out.budget.charge_work(2)?;
            if witness.parameters()[link.parameter].block == target {
                let ghost = memory.source_values - witness.parameters().len() + link.parameter;
                // The instrumented snapshot is unobservable and does not add an
                // original read/trap. Its initialization is a relation obligation.
                emit!(
                    out,
                    " let next = next.update({ghost}, private[{}]);\n",
                    link.slot
                );
            }
        }
    }
    emit!(out, " CfgStepV26 {{ state: ");
    state_tail(side, Some(target), out)?;
    emit!(
        out,
        ", events: seq![0int, {}_final], halted: false }}\n}}",
        side.label()
    );
    Ok(())
}
fn terminator(
    inv: &Inventory<'_>,
    scalar: &Plan,
    memory: &MemoryPlan,
    witness: &Witness,
    block: usize,
    side: Side,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    match inv.blocks()[block].terminator {
        Terminator::Branch { .. } => edge(inv, scalar, memory, witness, block, side, 0, out)?,
        Terminator::ConditionalBranch { .. } => {
            emit!(out, "if ");
            terminal_value(inv, scalar, block, side, 0, out)?;
            emit!(out, " == 1int ");
            edge(inv, scalar, memory, witness, block, side, 0, out)?;
            emit!(out, " else ");
            edge(inv, scalar, memory, witness, block, side, 1, out)?;
        }
        Terminator::Switch { cases, .. } => {
            for (ordinal, case) in cases.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(out, "if ");
                terminal_value(inv, scalar, block, side, 0, out)?;
                emit!(out, " == {}int ", case.value);
                edge(inv, scalar, memory, witness, block, side, ordinal, out)?;
                emit!(out, " else ");
            }
            edge(inv, scalar, memory, witness, block, side, cases.len(), out)?;
        }
        Terminator::IntegerSwitch { cases, .. } => {
            for (ordinal, case) in cases.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(out, "if ");
                terminal_value(inv, scalar, block, side, 0, out)?;
                emit!(out, " == {}int ", bits(&case.value));
                edge(inv, scalar, memory, witness, block, side, ordinal, out)?;
                emit!(out, " else ");
            }
            edge(inv, scalar, memory, witness, block, side, cases.len(), out)?;
        }
        Terminator::Return { values } => {
            emit!(out, "CfgStepV26 {{ state: ");
            state_tail(side, None, out)?;
            emit!(out, ", events: seq![1int, {}_final,", side.label());
            for ordinal in 0..values.len() {
                terminal_value(inv, scalar, block, side, ordinal, out)?;
                emit!(out, ",");
            }
            emit!(out, "], halted: true }}");
        }
        Terminator::Unreachable => {
            emit!(out, "CfgStepV26 {{ state: ");
            state_tail(side, None, out)?;
            emit!(
                out,
                ", events: seq![2int, {}_final], halted: true }}",
                side.label()
            );
        }
    }
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
    emit!(out, "{STATE}");
    for block in 0..input.blocks().len() {
        for side in [Side::Input, Side::Output] {
            let inv = match side {
                Side::Input => input,
                Side::Output => output,
            };
            let label = side.label();
            let values = match side {
                Side::Input => memory.source_values,
                Side::Output => output.definitions().len(),
            };
            emit!(
                out,
                "open spec fn aggregate_block_{label}_{block}(base: Seq<int>, cells: Seq<int>, initialized: Seq<bool>, initial: int, op: spec_fn(int, int, Seq<int>, int) -> int) -> CfgStepV26<AggregateStateV30, int>\n recommends base.len() == {values}, cells.len() == {}, initialized.len() == {},\n{{\n",
                memory.slots,
                memory.slots
            );
            body_in_memory(
                input,
                output,
                scalar,
                block,
                block,
                side,
                false,
                Environment::ActualDefinitions,
                if matches!(side, Side::Input) {
                    Some(witness)
                } else {
                    None
                },
                out,
            )?;
            emit!(out, " let next = base;\n");
            for op in inv.blocks()[block].operations.clone() {
                out.budget.charge_work(2)?;
                if matches!(side, Side::Input)
                    && matches!(
                        witness.memory_events()[op],
                        Memory::Allocate { .. } | Memory::Project { .. }
                    )
                {
                    continue;
                }
                for definition in inv.operations()[op].results.clone() {
                    emit!(
                        out,
                        " let next = next.update({definition}, {label}{definition});\n"
                    );
                }
            }
            if matches!(side, Side::Input) {
                emit!(out, " if !memory_valid {{ CfgStepV26 {{ state: ");
                state_tail(side, None, out)?;
                emit!(out, ", events: seq![4int], halted: true }} }} else {{\n");
            }
            terminator(inv, scalar, memory, witness, block, side, out)?;
            if matches!(side, Side::Input) {
                emit!(out, "\n}}\n");
            }
            emit!(out, "\n}}\n");
        }
    }
    for side in [Side::Input, Side::Output] {
        let label = side.label();
        let values = match side {
            Side::Input => memory.source_values,
            Side::Output => output.definitions().len(),
        };
        emit!(
            out,
            "open spec fn aggregate_step_{label}(s: AggregateStateV30, op: spec_fn(int, int, Seq<int>, int) -> int) -> CfgStepV26<AggregateStateV30, int> {{\n if s.values.len() == {values} && s.cells.len() == {} && s.initialized.len() == {} {{\n",
            memory.slots,
            memory.slots
        );
        for b in 0..input.blocks().len() {
            out.budget.charge_work(1)?;
            emit!(
                out,
                "if s.pc == {b} {{ aggregate_block_{label}_{b}(s.values, s.cells, s.initialized, s.external, op) }} else "
            );
        }
        emit!(
            out,
            "{{ CfgStepV26 {{ state: s, events: Seq::empty(), halted: true }} }}\n }} else {{ CfgStepV26 {{ state: s, events: seq![3int], halted: true }} }}\n}}\n"
        );
    }
    Ok(())
}
