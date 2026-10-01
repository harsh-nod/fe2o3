//! Actual ordered terminators and immutable preedge argument snapshots.
use super::*;

pub(super) fn headers() -> usize {
    size_of::<(
        [Range<usize>; 4],
        [usize; 20],
        [&(); 20],
        [Result<()>; 5],
        std::slice::Iter<'static, fe2o3_kernel_analysis::CanonicalKirUseRefV1>,
        std::slice::Iter<'static, fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1>,
        std::iter::Enumerate<std::ops::Range<usize>>,
        Option<usize>,
    )>()
}

pub(super) fn check(
    inventory: &Inventory<'_>,
    block: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(5)?;
    let row = inventory.blocks().get(block).ok_or_else(mismatch)?;
    let function = &inventory.functions()[row.coordinate.function.0 as usize];
    let uses = inventory
        .uses()
        .get(row.terminator_uses.clone())
        .ok_or_else(mismatch)?;
    let mut ordinal = 0;
    row.terminator.try_visit_operands(|value| -> Result<()> {
        out.budget.charge_work(3)?;
        let actual = uses.get(ordinal).ok_or_else(mismatch)?;
        if actual.value != value
            || !function.definitions.contains(&actual.definition)
            || actual.coordinate
                != (Use::TerminatorOperand {
                    block: row.coordinate,
                    operand: u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                })
        {
            return Err(mismatch());
        }
        value_type(inventory.definitions()[actual.definition].ty)?;
        ordinal += 1;
        Ok(())
    })?;
    if ordinal != uses.len() {
        return Err(mismatch());
    }
    for (ordinal, edge) in inventory.edges()[row.edges.clone()].iter().enumerate() {
        out.budget.charge_work(4)?;
        let target = &inventory.blocks()[block_index(inventory, edge.target)?];
        if edge.coordinate.source != row.coordinate
            || edge.coordinate.successor as usize != ordinal
            || edge.target.function != row.coordinate.function
            || edge.arguments.len() != edge.bindings.len()
            || target.parameters.len() != edge.bindings.len()
        {
            return Err(mismatch());
        }
        for (ordinal, binding) in inventory.edge_arguments()[edge.bindings.clone()]
            .iter()
            .enumerate()
        {
            out.budget.charge_work(5)?;
            let target_definition = target
                .parameters
                .start
                .checked_add(ordinal)
                .ok_or(Resource::Arithmetic)?;
            if binding.coordinate.edge != edge.coordinate
                || binding.coordinate.argument as usize != ordinal
                || binding.value != edge.arguments[ordinal]
                || !function.definitions.contains(&binding.incoming_definition)
                || binding.target_definition != target_definition
                || inventory.definitions()[target_definition].coordinate
                    != (Definition::BlockArgument {
                        block: edge.target,
                        argument: u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                    })
                || inventory.definitions()[target_definition].ty
                    != inventory.definitions()[binding.incoming_definition].ty
            {
                return Err(mismatch());
            }
        }
    }
    let selector = || -> Result<&Type> {
        Ok(inventory.definitions()[uses.first().ok_or_else(mismatch)?.definition].ty)
    };
    match row.terminator {
        Terminator::Branch { .. } if row.edges.len() == 1 => (),
        Terminator::ConditionalBranch { .. }
            if row.edges.len() == 2 && selector()? == &Type::BOOL =>
        {
            ()
        }
        Terminator::Switch { cases, .. }
            if row.edges.len() == cases.len() + 1 && matches!(selector()?, Type::Scalar(_)) =>
        {
            ()
        }
        Terminator::IntegerSwitch { cases, .. }
            if row.edges.len() == cases.len() + 1 && matches!(selector()?, Type::Scalar(_)) =>
        {
            ()
        }
        Terminator::Return { values }
            if row.edges.is_empty()
                && uses.len() == values.len()
                && values.len() == function.function.signature.results.len() =>
        {
            for (value, ty) in uses.iter().zip(&function.function.signature.results) {
                out.budget.charge_work(1)?;
                if inventory.definitions()[value.definition].ty != ty {
                    return Err(mismatch());
                }
            }
        }
        Terminator::Unreachable if row.edges.is_empty() && uses.is_empty() => (),
        _ => return Err(Error::Statement("actual byte terminator is not modeled")),
    }
    Ok(())
}

fn emit_edge<R: ByteAllocationResolverV30>(
    model: &ByteFunctionV30<'_, '_, R>,
    edge: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let edge = &model.inventory.edges()[edge];
    let target = block_index(model.inventory, edge.target)?;
    emit!(out, " {{ let preedge = done.values;\n let values = preedge");
    for binding in &model.inventory.edge_arguments()[edge.bindings.clone()] {
        out.budget.charge_work(1)?;
        emit!(
            out,
            ".update({}, preedge[{}])",
            binding.target_definition,
            binding.incoming_definition
        );
    }
    emit!(out, ";\n let valid = done.valid && control_valid");
    for binding in &model.inventory.edge_arguments()[edge.bindings.clone()] {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " && ({{ let transferred = preedge[{}]; ",
            binding.incoming_definition
        );
        emit_value_type(
            model.inventory.definitions()[binding.target_definition].ty,
            model.interpretation.width,
            "transferred",
            out,
        )?;
        emit!(out, " }})");
    }
    emit!(
        out,
        ";\n MemoryBlockResultV30 {{ state: MemoryStateV30 {{ pc: {target}, values, valid, ..done }}, observations, returned: Seq::empty() }} }}"
    );
    Ok(())
}

pub(super) fn emit_block<R: ByteAllocationResolverV30>(
    model: &ByteFunctionV30<'_, '_, R>,
    namespace: usize,
    block: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(2)?;
    let row = &model.inventory.blocks()[block];
    let definitions = model.inventory.definitions().len();
    emit!(
        out,
        "open spec fn byte_block_{namespace}_{block}_v30(s: MemoryStateV30, little_endian: bool) -> MemoryBlockResultV30 {{\n if s.values.len() != {definitions} || s.pc != {block} {{ MemoryBlockResultV30 {{ state: MemoryStateV30 {{ valid: false, ..s }}, observations: Seq::empty(), returned: Seq::empty() }} }} else {{\n let current = s;\n"
    );
    for operation in row.operations.clone() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " let step{operation} = byte_operation_{namespace}_{operation}_v30(current, little_endian);\n let current = step{operation}.state;\n"
        );
    }
    emit!(out, " let done = current;\n let observations = seq![");
    for operation in row.operations.clone() {
        out.budget.charge_work(1)?;
        emit!(out, "step{operation}.observation,");
    }
    emit!(
        out,
        "];\n byte_control_{namespace}_{block}_v30(done, observations)\n }}\n}}\n"
    );
    emit!(
        out,
        "open spec fn byte_control_{namespace}_{block}_v30(done: MemoryStateV30, observations: Seq<MemoryOperationObservationV30>) -> MemoryBlockResultV30 {{\n let trapped = "
    );
    model.emit_trap_terminal(block, "done", "observations", out)?;
    emit!(
        out,
        ";\n if done.values.len() != {definitions} || (done.pc != {block} && !trapped) || !byte_state_memory_well_formed_v30(done) || !"
    );
    model
        .interpretation
        .emit_state_predicate("done.memory", "done.values", None, out)?;
    emit!(
        out,
        " {{ MemoryBlockResultV30 {{ state: MemoryStateV30 {{ valid: false, ..done }}, observations, returned: Seq::empty() }} }} else {{\n"
    );
    emit!(
        out,
        " if trapped {{ MemoryBlockResultV30 {{ state: done, observations, returned: Seq::empty() }} }} else {{\n"
    );
    let uses = &model.inventory.uses()[row.terminator_uses.clone()];
    match row.terminator {
        Terminator::Branch { .. } => {
            emit!(out, " let control_valid = true;\n");
            emit_edge(model, row.edges.start, out)?;
        }
        Terminator::ConditionalBranch { .. } => {
            let selector = uses[0].definition;
            emit!(
                out,
                " let control_valid = matches!(done.values[{selector}], MemoryValueV30::Scalar(0) | MemoryValueV30::Scalar(1));\n if done.values[{selector}] == MemoryValueV30::Scalar(1) "
            );
            emit_edge(model, row.edges.start, out)?;
            emit!(out, " else ");
            emit_edge(model, row.edges.start + 1, out)?;
        }
        Terminator::Switch { cases, .. } => {
            emit_switch_prefix(model, uses[0].definition, out)?;
            for (ordinal, case) in cases.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(out, " if selector == {} ", case.value);
                emit_edge(model, row.edges.start + ordinal, out)?;
                emit!(out, " else ");
            }
            emit_edge(model, row.edges.end - 1, out)?;
        }
        Terminator::IntegerSwitch { cases, .. } => {
            emit_switch_prefix(model, uses[0].definition, out)?;
            for (ordinal, case) in cases.iter().enumerate() {
                out.budget.charge_work(1)?;
                emit!(out, " if selector == {} ", super::super::bits(&case.value));
                emit_edge(model, row.edges.start + ordinal, out)?;
                emit!(out, " else ");
            }
            emit_edge(model, row.edges.end - 1, out)?;
        }
        Terminator::Return { .. } => {
            emit!(out, " let valid = done.valid");
            for value in uses {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    " && ({{ let returned = done.values[{}]; ",
                    value.definition
                );
                emit_value_type(
                    model.inventory.definitions()[value.definition].ty,
                    model.interpretation.width,
                    "returned",
                    out,
                )?;
                emit!(out, " }})");
            }
            emit!(
                out,
                ";\n MemoryBlockResultV30 {{ state: MemoryStateV30 {{ pc: -1, valid, ..done }}, observations, returned: seq!["
            );
            for value in uses {
                out.budget.charge_work(1)?;
                emit!(out, "done.values[{}],", value.definition);
            }
            // No logical source return can free physical target storage here.
            emit!(out, "] }}");
        }
        Terminator::Unreachable => emit!(
            out,
            " MemoryBlockResultV30 {{ state: MemoryStateV30 {{ pc: -1, valid: false, ..done }}, observations, returned: Seq::empty() }}"
        ),
    }
    emit!(out, "\n }}\n }}\n}}\n");
    Ok(())
}

fn emit_switch_prefix<R>(
    model: &ByteFunctionV30<'_, '_, R>,
    selector: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    emit!(
        out,
        " let selector_value = done.values[{selector}];\n let control_valid = "
    );
    emit_value_type(
        model.inventory.definitions()[selector].ty,
        model.interpretation.width,
        "selector_value",
        out,
    )?;
    emit!(
        out,
        ";\n let selector = match selector_value {{ MemoryValueV30::Scalar(v) => v, _ => 0 }};\n"
    );
    Ok(())
}
