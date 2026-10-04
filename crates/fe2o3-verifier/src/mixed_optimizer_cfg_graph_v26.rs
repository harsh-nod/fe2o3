//! Exact graph-specific state transitions under the same shared operator model
//! as the local statement. Cross-block environment refinement remains a separate
//! proof obligation; these functions do not upgrade a block receipt.

use super::*;
use fe2o3_kernel_ir::Terminator;

const STATE: &str = r#"
struct CfgStateV26 {
    pc: int,
    values: Seq<int>,
    memory: int,
}
// Events distinguish an ordinary edge, a return, an unreachable terminator,
// and invalid control state (tags 0, 1, 2, 3). They observe abstract memory and
// actual return operands. Concrete memory/call interpretation remains separate.
"#;

fn actual_value(
    inventory: &Inventory<'_>,
    plan: &Plan,
    block: usize,
    side: Side,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    value_in(
        inventory,
        plan,
        block,
        side,
        Environment::ActualDefinitions,
        definition,
        out,
    )
}

fn terminal_use(inventory: &Inventory<'_>, block: usize, ordinal: usize) -> Result<usize> {
    let range = &inventory.blocks()[block].terminator_uses;
    let index = range
        .start
        .checked_add(ordinal)
        .ok_or(Resource::Accounting)?;
    if index >= range.end {
        return Err(Error::Statement("CFG terminator operand census"));
    }
    Ok(inventory.uses()[index].definition)
}

fn edge(
    inventory: &Inventory<'_>,
    plan: &Plan,
    block: usize,
    side: Side,
    ordinal: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(6)?;
    let source = &inventory.blocks()[block];
    let index = source
        .edges
        .start
        .checked_add(ordinal)
        .ok_or(Resource::Accounting)?;
    if index >= source.edges.end {
        return Err(Error::Statement("CFG edge ordinal"));
    }
    let edge = &inventory.edges()[index];
    let target = block_index(inventory, edge.target)?;
    let parameters = &inventory.blocks()[target].parameters;
    if edge.target.function != source.coordinate.function
        || edge.bindings.len() != parameters.len()
        || edge.arguments.len() != parameters.len()
    {
        return Err(Error::Statement("CFG exact target parameter roster"));
    }
    emit!(out, "{{\n");
    for (ordinal, binding) in edge.bindings.clone().enumerate() {
        out.budget.charge_work(4)?;
        let binding = &inventory.edge_arguments()[binding];
        let parameter = parameters
            .start
            .checked_add(ordinal)
            .ok_or(Resource::Accounting)?;
        if binding.target_definition != parameter {
            return Err(Error::Statement("CFG target parameter identity"));
        }
        // Read the old base/local SSA value, never a preceding parameter update.
        emit!(out, " let next = next.update({parameter}, ");
        actual_value(
            inventory,
            plan,
            block,
            side,
            binding.incoming_definition,
            out,
        )?;
        emit!(out, ");\n");
    }
    let label = side.label();
    emit!(
        out,
        " CfgStepV26 {{ state: CfgStateV26 {{ pc: {target}, values: next, memory: {label}_final }}, events: seq![0int, {label}_final], halted: false }}\n}}"
    );
    Ok(())
}

fn terminator(
    inventory: &Inventory<'_>,
    plan: &Plan,
    block: usize,
    side: Side,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(2)?;
    let row = &inventory.blocks()[block];
    match row.terminator {
        Terminator::Branch { .. } => edge(inventory, plan, block, side, 0, out)?,
        Terminator::ConditionalBranch { .. } => {
            emit!(out, " if ");
            actual_value(
                inventory,
                plan,
                block,
                side,
                terminal_use(inventory, block, 0)?,
                out,
            )?;
            emit!(out, " == 1int ");
            edge(inventory, plan, block, side, 0, out)?;
            emit!(out, " else ");
            edge(inventory, plan, block, side, 1, out)?;
        }
        Terminator::Switch { cases, .. } => {
            for (ordinal, case) in cases.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(out, " if ");
                actual_value(
                    inventory,
                    plan,
                    block,
                    side,
                    terminal_use(inventory, block, 0)?,
                    out,
                )?;
                emit!(out, " == {}int ", case.value);
                edge(inventory, plan, block, side, ordinal, out)?;
                emit!(out, " else ");
            }
            edge(inventory, plan, block, side, cases.len(), out)?;
        }
        Terminator::IntegerSwitch { cases, .. } => {
            for (ordinal, case) in cases.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(out, " if ");
                actual_value(
                    inventory,
                    plan,
                    block,
                    side,
                    terminal_use(inventory, block, 0)?,
                    out,
                )?;
                emit!(out, " == {}int ", bits(&case.value));
                edge(inventory, plan, block, side, ordinal, out)?;
                emit!(out, " else ");
            }
            edge(inventory, plan, block, side, cases.len(), out)?;
        }
        Terminator::Return { values } => {
            let label = side.label();
            emit!(
                out,
                " CfgStepV26 {{ state: CfgStateV26 {{ pc: -1, values: next, memory: {label}_final }}, events: seq![1int, {label}_final,"
            );
            for ordinal in 0..values.len() {
                out.budget.charge_work(1)?;
                actual_value(
                    inventory,
                    plan,
                    block,
                    side,
                    terminal_use(inventory, block, ordinal)?,
                    out,
                )?;
                emit!(out, ",");
            }
            emit!(out, "], halted: true }}");
        }
        Terminator::Unreachable => {
            let label = side.label();
            emit!(
                out,
                " CfgStepV26 {{ state: CfgStateV26 {{ pc: -1, values: next, memory: {label}_final }}, events: seq![2int, {label}_final], halted: true }}"
            );
        }
    }
    emit!(out, "\n");
    Ok(())
}

pub(super) fn generate(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    plan: &Plan,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(out, "{STATE}");
    for target in 0..output.blocks().len() {
        let original = plan.blocks[target];
        for side in [Side::Input, Side::Output] {
            out.budget.charge_work(1)?;
            let (inventory, block) = match side {
                Side::Input => (input, original),
                Side::Output => (output, target),
            };
            let label = side.label();
            emit!(
                out,
                "spec fn cfg_block_{label}_{block}({PARAMETERS}) -> CfgStepV26<CfgStateV26, int>\n recommends base.len() == {},\n{{\n",
                inventory.definitions().len()
            );
            body_in(
                input,
                output,
                plan,
                original,
                target,
                side,
                false,
                Environment::ActualDefinitions,
                out,
            )?;
            emit!(out, " let next = base;\n");
            for operation in inventory.blocks()[block].operations.clone() {
                out.budget.charge_work(1)?;
                for definition in inventory.operations()[operation].results.clone() {
                    out.budget.charge_work(1)?;
                    emit!(
                        out,
                        " let next = next.update({definition}, {label}{definition});\n"
                    );
                }
            }
            terminator(inventory, plan, block, side, out)?;
            emit!(out, "}}\n");
        }
    }
    for side in [Side::Input, Side::Output] {
        let inventory = match side {
            Side::Input => input,
            Side::Output => output,
        };
        let label = side.label();
        emit!(
            out,
            "spec fn cfg_frame_{label}_v26(state: CfgStateV26) -> bool {{ state.values.len() == {} && 0 <= state.pc < {} }}\n",
            inventory.definitions().len(),
            inventory.blocks().len()
        );
        emit!(
            out,
            "spec fn cfg_step_{label}_v26(state: CfgStateV26, op: spec_fn(int, int, Seq<int>, int) -> int) -> CfgStepV26<CfgStateV26, int>\n recommends state.values.len() == {},\n{{\n",
            inventory.definitions().len()
        );
        for block in 0..inventory.blocks().len() {
            out.budget.charge_work(1)?;
            emit!(
                out,
                " if state.pc == {block} {{ cfg_block_{label}_{block}(state.values, state.memory, op) }} else "
            );
        }
        emit!(
            out,
            "{{ CfgStepV26 {{ state, events: seq![3int], halted: true }} }}\n}}\n"
        );
        emit!(
            out,
            "proof fn cfg_step_frame_{label}_v26(state: CfgStateV26, op: spec_fn(int, int, Seq<int>, int) -> int)\n requires cfg_frame_{label}_v26(state),\n ensures cfg_step_{label}_v26(state, op).state.values.len() == {},\n !cfg_step_{label}_v26(state, op).halted ==> cfg_frame_{label}_v26(cfg_step_{label}_v26(state, op).state),\n{{\n",
            inventory.definitions().len()
        );
        for block in 0..inventory.blocks().len() {
            out.budget.charge_work(1)?;
            emit!(out, " if state.pc == {block} {{ }} else ");
        }
        emit!(out, "{{ assert(false); }}\n}}\n");
        for function in inventory.functions() {
            out.budget.charge_work(1)?;
            if function.blocks.is_empty() {
                continue;
            }
            let ordinal = function.coordinate.0;
            emit!(
                out,
                "spec fn cfg_entry_{label}_{ordinal}_v26(state: CfgStateV26, arguments: Seq<int>) -> bool {{\n cfg_frame_{label}_v26(state) && state.pc == {} && arguments.len() == {}",
                function.blocks.start,
                function.function.signature.parameters.len()
            );
            for argument in 0..function.function.signature.parameters.len() {
                out.budget.charge_work(1)?;
                let definition = definition_index(
                    inventory,
                    Definition::FunctionArgument {
                        function: function.coordinate,
                        argument: u32::try_from(argument).map_err(|_| Resource::Accounting)?,
                    },
                )?;
                emit!(
                    out,
                    " && state.values[{definition}] == arguments[{argument}]"
                );
                if let Some(width) = scalar_width(inventory.definitions()[definition].ty) {
                    emit!(out, " && 0 <= arguments[{argument}] < {}", 1u128 << width);
                }
            }
            emit!(out, "\n}}\n");
        }
    }
    Ok(())
}
