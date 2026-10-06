//! Exact prefix/final CFG simulation with expression-valued relocation cuts.
//! The graph interpreter is shared with V27; its old block relation is not.
use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirLicmV18 as Pair;
use fe2o3_kernel_ir::{
    CanonicalKirControlFlowViewV18 as Flow, with_canonical_kir_control_flow_v18,
};

pub(super) fn plan(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Pair<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<Plan> {
    if !std::ptr::eq(input.owner(), pair.input())
        || !std::ptr::eq(output.owner(), pair.output())
        || input.operations().len() != pair.origins().len()
        || input.operations().len() != output.operations().len()
        || input.definitions().len() != output.definitions().len()
        || input.blocks().len() != output.blocks().len()
    {
        return Err(Error::Statement("exact complete relocation graph pair"));
    }
    out.budget
        .reserve_storage(size_of::<Plan>() + align_of::<Plan>())?;
    let mut plan = Plan {
        anchors: allocate(output.definitions().len(), out)?,
        input_operations: allocate(input.operations().len(), out)?,
        output_operations: allocate(output.operations().len(), out)?,
        blocks: allocate(output.blocks().len(), out)?,
        seen_definitions: allocate(input.definitions().len(), out)?,
        seen_operations: allocate(input.operations().len(), out)?,
        pending_definitions: Vec::new(),
        total_classes: None,
    };
    for (index, row) in pair.origins().iter().enumerate() {
        out.budget.charge_work(7)?;
        if input.operations()[index].coordinate != row.input {
            return Err(Error::Statement("relocation original operation order"));
        }
        let target = operation_index(output, row.output)?;
        if plan.output_operations[target] != NONE {
            return Err(Error::Statement("relocation operation permutation"));
        }
        if row.hoist.is_some() && !total(&input.operations()[index].operation.kind) {
            return Err(Error::Statement("relocation cannot move ordered effects"));
        }
        plan.input_operations[index] = target;
        plan.output_operations[target] = index;
        plan.seen_operations[index] = usize::from(row.hoist.is_some());
    }
    for (index, row) in output.blocks().iter().enumerate() {
        out.budget.charge_work(3)?;
        if input.blocks()[index].coordinate != row.coordinate {
            return Err(Error::Statement("relocation exact CFG block coordinates"));
        }
        plan.blocks[index] = index;
    }
    for (target, row) in output.definitions().iter().enumerate() {
        out.budget.charge_work(6)?;
        let coordinate = match row.coordinate {
            Definition::Result { operation, result } => {
                let original = plan.output_operations[operation_index(output, operation)?];
                Definition::Result {
                    operation: input.operations()[original].coordinate,
                    result,
                }
            }
            coordinate => coordinate,
        };
        let original = definition_index(input, coordinate)?;
        if plan.seen_definitions[original] != NONE {
            return Err(Error::Statement("relocation definition permutation"));
        }
        plan.anchors[target] = original;
        plan.seen_definitions[original] = target;
    }
    out.budget.charge_work(
        input
            .definitions()
            .len()
            .checked_add(output.operations().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if plan.seen_definitions.contains(&NONE) || plan.output_operations.contains(&NONE) {
        return Err(Error::Statement(
            "relocation complete definition and operation mapping",
        ));
    }
    plan.total_classes = Some(congruence_v27::build(
        input,
        output,
        &plan.output_operations,
        out,
    )?);
    Ok(plan)
}

fn moved(input: &Inventory<'_>, plan: &Plan, definition: usize) -> Result<bool> {
    match input.definitions()[definition].coordinate {
        Definition::Result { operation, .. } => {
            Ok(plan.seen_operations[operation_index(input, operation)?] == 1)
        }
        _ => Ok(false),
    }
}
fn expression_operand(
    input: &Inventory<'_>,
    plan: &Plan,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(2)?;
    if moved(input, plan, definition)? {
        emit!(out, "relocated_value_{definition}_v28(base, op)");
    } else {
        emit!(out, "base[{definition}]");
    }
    Ok(())
}

fn expression(
    input: &Inventory<'_>,
    plan: &Plan,
    operation: usize,
    result: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let row = &input.operations()[operation];
    out.budget.charge_work(4)?;
    if let OperationKind::Constant(constant) = &row.operation.kind {
        if result != 0 {
            return Err(Error::Statement("relocated constant result"));
        }
        emit!(out, "{}int", bits(constant));
    } else if matches!(row.operation.kind, OperationKind::Select { .. }) {
        if result != 0 {
            return Err(Error::Statement("relocated Select result ordinal"));
        }
        let operands = select_operands(input, operation, out)?;
        emit!(out, "select_value_v28(");
        for definition in operands {
            expression_operand(input, plan, definition, out)?;
            emit!(out, ",");
        }
        emit!(out, ")");
    } else if let Some((operator, width, signed_value)) = concrete(input, operation) {
        let arity = if matches!(operator, BinaryOp::Checked(_)) {
            2
        } else {
            1
        };
        if row.operands.len() != 2 || row.results.len() != arity || result >= arity {
            return Err(Error::Statement("relocated concrete operation arity"));
        }
        emit!(out, "{{ let left = ");
        expression_operand(
            input,
            plan,
            input.uses()[row.operands.start].definition,
            out,
        )?;
        emit!(out, "; let right = ");
        expression_operand(
            input,
            plan,
            input.uses()[row.operands.start + 1].definition,
            out,
        )?;
        emit!(out, "; ");
        match operator {
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                let symbol = match operator {
                    BinaryOp::BitAnd => "&",
                    BinaryOp::BitOr => "|",
                    _ => "^",
                };
                emit!(
                    out,
                    "((left as u{width}) {symbol} (right as u{width})) as int"
                );
            }
            BinaryOp::Checked(operator) => {
                let modulus = 1u128 << width;
                let symbol = match operator {
                    CheckedBinaryOperator::Add => "+",
                    CheckedBinaryOperator::Subtract => "-",
                    CheckedBinaryOperator::Multiply => "*",
                };
                if signed_value {
                    emit!(
                        out,
                        "let arithmetic = signed(left, {modulus}) {symbol} signed(right, {modulus}); "
                    );
                } else {
                    emit!(out, "let arithmetic = left {symbol} right; ");
                }
                if result == 0 {
                    emit!(out, "arithmetic % {modulus}");
                } else {
                    let (minimum, maximum) = if signed_value {
                        (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
                    } else {
                        (0, (1i128 << width) - 1)
                    };
                    emit!(
                        out,
                        "if {minimum} <= arithmetic <= {maximum} {{ 0int }} else {{ 1int }}"
                    );
                }
            }
            _ => return Err(Error::Statement("relocated arithmetic must be total")),
        }
        emit!(out, " }}");
    } else {
        if !opaque_total(&row.operation.kind) || result >= row.results.len() {
            return Err(Error::Statement("relocated exact total expression"));
        }
        let interpretation = total_interpretation(plan, operation, false)?;
        emit!(out, "op({interpretation}, {result}, seq![");
        for usage in row.operands.clone() {
            expression_operand(input, plan, input.uses()[usage].definition, out)?;
            emit!(out, ",");
        }
        emit!(out, "], 0)");
        if let Some(width) = scalar_width(input.definitions()[row.results.start + result].ty) {
            emit!(out, " % {}", 1u128 << width);
        }
    }
    Ok(())
}

fn available(
    flow: &mut Flow<'_, '_>,
    coordinate: Definition,
    block: Block,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.charge_work(2)?;
    Ok(match coordinate {
        Definition::FunctionArgument { function, .. } => function == block.function,
        Definition::BlockArgument { block: origin, .. } => {
            origin.function == block.function && flow.dominates(origin, block, out.budget)?
        }
        Definition::Result { operation, .. } => {
            operation.block.function == block.function
                && operation.block != block
                && flow.dominates(operation.block, block, out.budget)?
        }
    })
}

fn relations(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    plan: &Plan,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for function in input.functions() {
        out.budget.charge_work(1)?;
        if function.blocks.is_empty() {
            continue;
        }
        // Only this function's definitions are visited. Stream into the same
        // paid writer while one bounded dominance scope serves all its blocks.
        let text = std::mem::take(&mut out.text);
        let failure = out.failure.take();
        let (text, failure) = with_canonical_kir_control_flow_v18(
            input.owner(),
            function.coordinate,
            Default::default(),
            out.budget,
            |flow, budget| {
                let mut writer = Writer {
                    text,
                    budget,
                    failure,
                };
                let out = &mut writer;
                for block in function.blocks.clone() {
                    let coordinate = input.blocks()[block].coordinate;
                    emit!(
                        out,
                        "spec fn relocation_cut_{block}_v28(base: Seq<int>, optimized: Seq<int>, op: spec_fn(int, int, Seq<int>, int) -> int) -> bool {{\n base.len() == {} && optimized.len() == {}",
                        input.definitions().len(),
                        output.definitions().len()
                    );
                    if !flow.is_reachable(coordinate, out.budget)? {
                        emit!(out, " && false\n}}\n");
                        continue;
                    }
                    for definition in function.definitions.clone() {
                        out.budget.charge_work(3)?;
                        let target = plan.seen_definitions[definition];
                        let input_available = available(
                            flow,
                            input.definitions()[definition].coordinate,
                            coordinate,
                            out,
                        )?;
                        let output_available = available(
                            flow,
                            output.definitions()[target].coordinate,
                            coordinate,
                            out,
                        )?;
                        if moved(input, plan, definition)? {
                            if input_available {
                                emit!(
                                    out,
                                    "\n && base[{definition}] == relocated_value_{definition}_v28(base, op)"
                                );
                            }
                            if output_available {
                                emit!(
                                    out,
                                    "\n && optimized[{target}] == relocated_value_{definition}_v28(base, op)"
                                );
                            }
                        } else {
                            if input_available != output_available {
                                return Err(Error::Statement(
                                    "unmoved relocation definition availability",
                                ));
                            }
                            if input_available {
                                emit!(out, "\n && base[{definition}] == optimized[{target}]");
                            }
                        }
                    }
                    emit!(out, "\n}}\n");
                }
                Ok::<_, Error>((writer.text, writer.failure))
            },
        )?;
        out.text = text;
        out.failure = failure;
    }
    emit!(
        out,
        "spec fn relocation_related_v28(n: CfgStateV26, o: CfgStateV26, op: spec_fn(int, int, Seq<int>, int) -> int) -> bool {{\n n.values.len() == {} && o.values.len() == {} && n.memory == o.memory && n.pc == o.pc\n && ((n.pc == -1)",
        input.definitions().len(),
        output.definitions().len()
    );
    for block in 0..input.blocks().len() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "\n || (n.pc == {block} && relocation_cut_{block}_v28(n.values, o.values, op))"
        );
    }
    emit!(out, ")\n}}\n");
    Ok(())
}

fn proofs(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    plan: &Plan,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    for block in 0..input.blocks().len() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "proof fn relocation_block_step_{block}_v28(n: CfgStateV26, o: CfgStateV26, op: spec_fn(int, int, Seq<int>, int) -> int)\n requires relocation_related_v28(n, o, op), n.pc == {block},\n ensures cfg_step_n_v26(n, op).events == cfg_step_o_v26(o, op).events,\n cfg_step_n_v26(n, op).halted == cfg_step_o_v26(o, op).halted,\n relocation_related_v28(cfg_step_n_v26(n, op).state, cfg_step_o_v26(o, op).state, op),\n{{\n let base = n.values; let initial = n.memory;\n"
        );
        body_in(
            input,
            output,
            plan,
            block,
            block,
            Side::Input,
            true,
            Environment::ActualDefinitions,
            out,
        )?;
        emit!(out, " let base = o.values; let initial = o.memory;\n");
        body_in(
            input,
            output,
            plan,
            block,
            block,
            Side::Output,
            true,
            Environment::ActualDefinitions,
            out,
        )?;
        emit!(
            out,
            " cfg_step_frame_n_v26(n, op);\n cfg_step_frame_o_v26(o, op);\n assert(cfg_step_n_v26(n, op).events == cfg_step_o_v26(o, op).events);\n assert(relocation_related_v28(cfg_step_n_v26(n, op).state, cfg_step_o_v26(o, op).state, op));\n}}\n"
        );
    }
    emit!(
        out,
        "proof fn relocation_all_steps_v28(op: spec_fn(int, int, Seq<int>, int) -> int)\n ensures cfg_step_simulates_v26(|n: CfgStateV26| cfg_step_n_v26(n, op), |o: CfgStateV26| cfg_step_o_v26(o, op), |n: CfgStateV26, o: CfgStateV26| relocation_related_v28(n, o, op)),\n{{\n assert forall|n: CfgStateV26, o: CfgStateV26| #[trigger] relocation_related_v28(n, o, op) implies\n cfg_step_n_v26(n, op).events == cfg_step_o_v26(o, op).events\n && cfg_step_n_v26(n, op).halted == cfg_step_o_v26(o, op).halted\n && relocation_related_v28(cfg_step_n_v26(n, op).state, cfg_step_o_v26(o, op).state, op) by {{\n if n.pc == -1 {{ }} else "
    );
    for block in 0..input.blocks().len() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "if n.pc == {block} {{ relocation_block_step_{block}_v28(n, o, op); }} else "
        );
    }
    emit!(out, "{{ assert(false); }}\n }}\n}}\n");
    for function in input.functions() {
        out.budget.charge_work(1)?;
        if function.blocks.is_empty() {
            continue;
        }
        let index = function.coordinate.0;
        emit!(
            out,
            "proof fn relocation_initial_{index}_v28(n: CfgStateV26, o: CfgStateV26, arguments: Seq<int>, op: spec_fn(int, int, Seq<int>, int) -> int)\n requires cfg_entry_n_{index}_v26(n, arguments), cfg_entry_o_{index}_v26(o, arguments), n.memory == o.memory,\n ensures relocation_related_v28(n, o, op),\n{{ }}\n"
        );
        emit!(
            out,
            "proof fn relocation_function_trace_{index}_v28(n: CfgStateV26, o: CfgStateV26, arguments: Seq<int>, op: spec_fn(int, int, Seq<int>, int) -> int, fuel: nat)\n requires cfg_entry_n_{index}_v26(n, arguments), cfg_entry_o_{index}_v26(o, arguments), n.memory == o.memory,\n ensures cfg_trace_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), n, fuel).events == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, op), o, fuel).events,\n cfg_trace_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), n, fuel).halted == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, op), o, fuel).halted,\n cfg_trace_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), n, fuel).steps == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, op), o, fuel).steps,\n{{\n relocation_initial_{index}_v28(n, o, arguments, op);\n relocation_all_steps_v28(op);\n cfg_finite_trace_refinement_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), |s: CfgStateV26| cfg_step_o_v26(s, op), |a: CfgStateV26, b: CfgStateV26| relocation_related_v28(a, b, op), n, o, fuel);\n}}\n"
        );
    }
    Ok(())
}

pub(in super::super) fn generate(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Pair<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let floor = out.budget.storage();
    let result = (|| {
        let plan = plan(input, output, pair, out)?;
        emit!(
            out,
            "use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {{\n{}",
            cfg_trace::PRELUDE
        );
        emit!(
            out,
            "// V28 prefix-to-final relocation only; generated obligations are not executed proofs.\nspec fn signed(x: int, m: int) -> int {{ if x < m / 2 {{ x }} else {{ x - m }} }}\n"
        );
        select_prelude(input, output, out)?;
        for width in [8, 16, 32, 64] {
            let max = (1u128 << width) - 1;
            emit!(
                out,
                "proof fn bit_identity_{width}(x: u{width})\n ensures (x & {max}u{width}) == x, (x | 0u{width}) == x, (x ^ 0u{width}) == x,\n{{ assert((x & {max}u{width}) == x) by(bit_vector); assert((x | 0u{width}) == x) by(bit_vector); assert((x ^ 0u{width}) == x) by(bit_vector); }}\n"
            );
        }
        for (ordinal, operation) in input.operations().iter().enumerate() {
            out.budget.charge_work(1)?;
            if plan.seen_operations[ordinal] != 1 {
                continue;
            }
            for (result, definition) in operation.results.clone().enumerate() {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    "spec fn relocated_value_{definition}_v28(base: Seq<int>, op: spec_fn(int, int, Seq<int>, int) -> int) -> int\n recommends base.len() == {},\n{{ ",
                    input.definitions().len()
                );
                expression(input, &plan, ordinal, result, out)?;
                emit!(out, " }}\n");
            }
        }
        cfg_graph::generate(input, output, &plan, out)?;
        relations(input, output, &plan, out)?;
        proofs(input, output, &plan, out)?;
        emit!(out, "}}\nfn main() {{}}\n");
        Ok(output.blocks().len())
    })();
    let release = out
        .budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    out.budget.release_storage(release)?;
    result
}
