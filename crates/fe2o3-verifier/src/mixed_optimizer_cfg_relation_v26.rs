//! Reconstruct the live-in relation from actual uses, exact edges and original
//! scalar equations. This is a proof-generator dependency closure, not an
//! optimizer-provided liveness certificate or a substitute for executing Verus.

use super::*;

struct Live {
    definitions: usize,
    cells: Vec<usize>,
    pending: Vec<usize>,
    predecessors: Vec<usize>,
    next: Vec<usize>,
    sources: Vec<usize>,
    inverse_blocks: Vec<usize>,
    tail: usize,
}

impl Live {
    fn contains(&self, block: usize, definition: usize) -> bool {
        self.cells[block * self.definitions + definition] != NONE
    }

    fn mark(
        &mut self,
        input: &Inventory<'_>,
        block: usize,
        definition: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(3)?;
        if local(input, definition, block) {
            return Ok(());
        }
        let index = block * self.definitions + definition;
        if self.cells[index] == NONE {
            self.cells[index] = 1;
            *self
                .pending
                .get_mut(self.tail)
                .ok_or(Error::Statement("CFG live-in queue census"))? = index;
            self.tail += 1;
        }
        Ok(())
    }

    fn build(
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        plan: &Plan,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let definitions = input.definitions().len();
        let cells = input
            .blocks()
            .len()
            .checked_mul(definitions)
            .ok_or(Resource::Accounting)?;
        let mut live = Self {
            definitions,
            cells: allocate(cells, out)?,
            pending: allocate(cells, out)?,
            predecessors: allocate(input.blocks().len(), out)?,
            next: allocate(input.edges().len(), out)?,
            sources: allocate(input.edges().len(), out)?,
            inverse_blocks: allocate(input.blocks().len(), out)?,
            tail: 0,
        };
        for (target, &original) in plan.blocks.iter().enumerate() {
            out.budget.charge_work(3)?;
            if live.inverse_blocks[original] != NONE {
                return Err(Error::Statement("CFG original block permutation"));
            }
            live.inverse_blocks[original] = target;
        }
        for (block, row) in input.blocks().iter().enumerate() {
            for edge in row.edges.clone() {
                out.budget.charge_work(5)?;
                let target = block_index(input, input.edges()[edge].target)?;
                live.sources[edge] = block;
                live.next[edge] = live.predecessors[target];
                live.predecessors[target] = edge;
            }
        }
        for (target, &original) in plan.blocks.iter().enumerate() {
            for operation in input.blocks()[original].operations.clone() {
                for usage in input.operations()[operation].operands.clone() {
                    live.mark(input, original, input.uses()[usage].definition, out)?;
                }
            }
            for usage in input.blocks()[original].terminator_uses.clone() {
                live.mark(input, original, input.uses()[usage].definition, out)?;
            }
            for operation in output.blocks()[target].operations.clone() {
                for usage in output.operations()[operation].operands.clone() {
                    let definition = output.uses()[usage].definition;
                    out.budget.charge_work(1)?;
                    if !local(output, definition, target) {
                        live.mark(input, original, plan.anchors[definition], out)?;
                    }
                }
            }
            for usage in output.blocks()[target].terminator_uses.clone() {
                let definition = output.uses()[usage].definition;
                out.budget.charge_work(1)?;
                if !local(output, definition, target) {
                    live.mark(input, original, plan.anchors[definition], out)?;
                }
            }
        }
        let mut head = 0;
        while head < live.tail {
            out.budget.charge_work(3)?;
            let index = live.pending[head];
            head += 1;
            let block = index / definitions;
            let definition = index % definitions;
            if let Definition::Result { operation, .. } = input.definitions()[definition].coordinate
            {
                let operation = operation_index(input, operation)?;
                if has_original_equation(input, plan, operation) {
                    for usage in input.operations()[operation].operands.clone() {
                        live.mark(input, block, input.uses()[usage].definition, out)?;
                    }
                }
            }
            // Parameters of the receiving block are installed by its incoming
            // edge. Their actual incoming operands are already explicit uses.
            if input.blocks()[block].parameters.contains(&definition) {
                continue;
            }
            let mut edge = live.predecessors[block];
            while edge != NONE {
                out.budget.charge_work(2)?;
                live.mark(input, live.sources[edge], definition, out)?;
                edge = live.next[edge];
            }
        }
        live.validate(input, output, plan, out)?;
        Ok(live)
    }

    // Replay the closure equations against actual inventories, independently of
    // the queue traversal. A lost seed or propagation row is a typed refusal.
    fn validate(
        &self,
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        plan: &Plan,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let required = |block, definition| {
            if local(input, definition, block) || self.contains(block, definition) {
                Ok(())
            } else {
                Err(Error::Statement(
                    "CFG live-in closure omits an exact dependency",
                ))
            }
        };
        for (target, &original) in plan.blocks.iter().enumerate() {
            for operation in input.blocks()[original].operations.clone() {
                for usage in input.operations()[operation].operands.clone() {
                    out.budget.charge_work(2)?;
                    required(original, input.uses()[usage].definition)?;
                }
            }
            for usage in input.blocks()[original].terminator_uses.clone() {
                out.budget.charge_work(2)?;
                required(original, input.uses()[usage].definition)?;
            }
            for operation in output.blocks()[target].operations.clone() {
                for usage in output.operations()[operation].operands.clone() {
                    out.budget.charge_work(2)?;
                    let definition = output.uses()[usage].definition;
                    if !local(output, definition, target) {
                        required(original, plan.anchors[definition])?;
                    }
                }
            }
            for usage in output.blocks()[target].terminator_uses.clone() {
                out.budget.charge_work(2)?;
                let definition = output.uses()[usage].definition;
                if !local(output, definition, target) {
                    required(original, plan.anchors[definition])?;
                }
            }
        }
        for (block, row) in input.blocks().iter().enumerate() {
            for definition in 0..self.definitions {
                out.budget.charge_work(1)?;
                if !self.contains(block, definition) {
                    continue;
                }
                if let Definition::Result { operation, .. } =
                    input.definitions()[definition].coordinate
                {
                    let operation = operation_index(input, operation)?;
                    if has_original_equation(input, plan, operation) {
                        for usage in input.operations()[operation].operands.clone() {
                            out.budget.charge_work(2)?;
                            required(block, input.uses()[usage].definition)?;
                        }
                    }
                }
                if row.parameters.contains(&definition) {
                    continue;
                }
                let mut edge = self.predecessors[block];
                while edge != NONE {
                    out.budget.charge_work(3)?;
                    required(self.sources[edge], definition)?;
                    edge = self.next[edge];
                }
            }
        }
        Ok(())
    }
}

fn equation(
    input: &Inventory<'_>,
    plan: &Plan,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let Definition::Result { operation, result } = input.definitions()[definition].coordinate
    else {
        return Ok(());
    };
    let operation = operation_index(input, operation)?;
    let row = &input.operations()[operation];
    if let OperationKind::Constant(constant) = &row.operation.kind {
        emit!(out, " && base[{definition}] == {}int", bits(constant));
        return Ok(());
    }
    let Some((operator, width, signed)) = concrete(input, operation) else {
        if matches!(row.operation.kind, OperationKind::Select { .. })
            || (plan.total_classes.is_some() && opaque_total(&row.operation.kind))
        {
            emit!(out, " && base[{definition}] == ");
            external_total_value(input, plan, operation, result as usize, out)?;
        }
        return Ok(());
    };
    let left = input.uses()[row.operands.start].definition;
    let right = input.uses()[row.operands.start + 1].definition;
    let modulus = 1u128 << width;
    emit!(out, " && base[{definition}] == ");
    if matches!(
        operator,
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
    ) {
        let symbol = match operator {
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            _ => "^",
        };
        emit!(
            out,
            "((base[{left}] as u{width}) {symbol} (base[{right}] as u{width})) as int"
        );
    } else if result == 0 {
        emit!(out, "(");
        external_arithmetic(operator, signed, modulus, left, right, out)?;
        emit!(out, ") % {modulus}");
    } else if result == 1 && matches!(operator, BinaryOp::Checked(_)) {
        let (minimum, maximum) = if signed {
            (-(1i128 << (width - 1)), (1i128 << (width - 1)) - 1)
        } else {
            (0, (1i128 << width) - 1)
        };
        emit!(out, "(if {minimum} <= (");
        external_arithmetic(operator, signed, modulus, left, right, out)?;
        emit!(out, ") <= {maximum} {{ 0int }} else {{ 1int }})");
    } else {
        return Err(Error::Statement("CFG original equation result ordinal"));
    }
    Ok(())
}

fn step_proof(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    plan: &Plan,
    live: &Live,
    target: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let original = plan.blocks[target];
    let (_, op_argument) = relation_operator(plan);
    emit!(
        out,
        "proof fn cfg_block_step_refinement_{target}_v26(n: CfgStateV26, o: CfgStateV26, op: spec_fn(int, int, Seq<int>, int) -> int)\n requires cfg_related_v26(n, o{op_argument}), o.pc == {target},\n ensures cfg_step_n_v26(n, op).events == cfg_step_o_v26(o, op).events,\n cfg_step_n_v26(n, op).halted == cfg_step_o_v26(o, op).halted,\n cfg_related_v26(cfg_step_n_v26(n, op).state, cfg_step_o_v26(o, op).state{op_argument}),\n{{\n let base = n.values;\n let initial = n.memory;\n"
    );
    for definition in 0..input.definitions().len() {
        out.budget.charge_work(1)?;
        if !live.contains(original, definition) {
            continue;
        }
        let Definition::Result {
            operation,
            result: 0,
        } = input.definitions()[definition].coordinate
        else {
            continue;
        };
        let operation = operation_index(input, operation)?;
        if let Some((BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor, width, _)) =
            concrete(input, operation)
        {
            for usage in input.operations()[operation].operands.clone() {
                out.budget.charge_work(1)?;
                emit!(
                    out,
                    " bit_identity_{width}(base[{}] as u{width});\n",
                    input.uses()[usage].definition
                );
            }
        }
    }
    // The local theorem is invoked with its actual original environment. The
    // independent output environment is related by retained descendants, not
    // equated with this environment or rebuilt using original values.
    emit!(out, " block_simulation_{target}(base, initial, op);\n");
    body_in(
        input,
        output,
        plan,
        original,
        target,
        Side::Input,
        true,
        Environment::ActualDefinitions,
        out,
    )?;
    emit!(out, " let base = o.values;\n let initial = o.memory;\n");
    body_in(
        input,
        output,
        plan,
        original,
        target,
        Side::Output,
        true,
        Environment::ActualDefinitions,
        out,
    )?;
    emit!(
        out,
        " cfg_step_frame_n_v26(n, op);\n cfg_step_frame_o_v26(o, op);\n"
    );
    // Splitting on the actual original selector bounds SMT work and exposes
    // each simultaneous assignment. No successor equality is a premise.
    match input.blocks()[original].terminator {
        fe2o3_kernel_ir::Terminator::ConditionalBranch { .. } => {
            let usage = input.blocks()[original].terminator_uses.start;
            emit!(out, " if ");
            emit_proof_value(input, plan, original, input.uses()[usage].definition, out)?;
            emit!(out, " == 1int {{\n");
            step_assertions(plan, out)?;
            emit!(out, " }} else {{\n");
            step_assertions(plan, out)?;
            emit!(out, " }}\n");
        }
        fe2o3_kernel_ir::Terminator::Switch { cases, .. } => {
            let usage = input.blocks()[original].terminator_uses.start;
            for case in cases {
                out.budget.charge_work(1)?;
                emit!(out, " if ");
                emit_proof_value(input, plan, original, input.uses()[usage].definition, out)?;
                emit!(out, " == {}int {{\n", case.value);
                step_assertions(plan, out)?;
                emit!(out, " }} else ");
            }
            emit!(out, " {{\n");
            step_assertions(plan, out)?;
            emit!(out, " }}\n");
        }
        fe2o3_kernel_ir::Terminator::IntegerSwitch { cases, .. } => {
            let usage = input.blocks()[original].terminator_uses.start;
            for case in cases {
                out.budget.charge_work(1)?;
                emit!(out, " if ");
                emit_proof_value(input, plan, original, input.uses()[usage].definition, out)?;
                emit!(out, " == {}int {{\n", bits(&case.value));
                step_assertions(plan, out)?;
                emit!(out, " }} else ");
            }
            emit!(out, " {{\n");
            step_assertions(plan, out)?;
            emit!(out, " }}\n");
        }
        _ => step_assertions(plan, out)?,
    }
    emit!(out, "}}\n");
    Ok(())
}

fn emit_proof_value(
    input: &Inventory<'_>,
    plan: &Plan,
    original: usize,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    // `base` currently denotes O. External N operands must keep their actual
    // N environment; local N result variables already name the executed body.
    if local(input, definition, original) {
        value_in(
            input,
            plan,
            original,
            Side::Input,
            Environment::ActualDefinitions,
            definition,
            out,
        )
    } else {
        emit!(out, "n.values[{definition}]");
        Ok(())
    }
}

fn relation_operator(plan: &Plan) -> (&'static str, &'static str) {
    if plan.total_classes.is_some() {
        (", op: spec_fn(int, int, Seq<int>, int) -> int", ", op")
    } else {
        ("", "")
    }
}

fn step_assertions(plan: &Plan, out: &mut Writer<'_, '_>) -> Result<()> {
    let (_, op_argument) = relation_operator(plan);
    emit!(
        out,
        " assert(cfg_step_n_v26(n, op).events == cfg_step_o_v26(o, op).events);\n assert(cfg_step_n_v26(n, op).halted == cfg_step_o_v26(o, op).halted);\n assert(cfg_related_v26(cfg_step_n_v26(n, op).state, cfg_step_o_v26(o, op).state{op_argument}));\n"
    );
    Ok(())
}

pub(super) fn generate(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    plan: &Plan,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let (op_parameter, op_argument) = relation_operator(plan);
    let live = Live::build(input, output, plan, out)?;
    for (target, &original) in plan.blocks.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "spec fn cfg_live_{target}_v26(base: Seq<int>, optimized: Seq<int>{op_parameter}) -> bool {{\n base.len() == {} && optimized.len() == {}",
            input.definitions().len(),
            output.definitions().len()
        );
        for definition in 0..input.definitions().len() {
            out.budget.charge_work(1)?;
            if !live.contains(original, definition) {
                continue;
            }
            if let Some(width) = scalar_width(input.definitions()[definition].ty) {
                emit!(out, "\n && 0 <= base[{definition}] < {}", 1u128 << width);
            }
            equation(input, plan, definition, out)?;
            for descendant in descendants(rows, definition)? {
                out.budget.charge_work(3)?;
                let result = definition_index(output, descendant.output)?;
                if local(output, result, target) {
                    return Err(Error::Statement(
                        "CFG live-in descendant is not available at entry",
                    ));
                }
                emit!(out, "\n && base[{definition}] == optimized[{result}]");
            }
        }
        emit!(out, "\n}}\n");
    }
    emit!(
        out,
        "spec fn cfg_related_v26(n: CfgStateV26, o: CfgStateV26{op_parameter}) -> bool {{\n n.values.len() == {} && o.values.len() == {} && n.memory == o.memory\n && ((n.pc == -1 && o.pc == -1)",
        input.definitions().len(),
        output.definitions().len()
    );
    for (target, &original) in plan.blocks.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "\n || (n.pc == {original} && o.pc == {target} && cfg_live_{target}_v26(n.values, o.values{op_argument}))"
        );
    }
    emit!(out, ")\n}}\n");
    for target in 0..output.blocks().len() {
        out.budget.charge_work(1)?;
        step_proof(input, output, plan, &live, target, out)?;
    }
    emit!(
        out,
        "proof fn cfg_all_steps_refine_v26(op: spec_fn(int, int, Seq<int>, int) -> int)\n ensures cfg_step_simulates_v26(|n: CfgStateV26| cfg_step_n_v26(n, op), |o: CfgStateV26| cfg_step_o_v26(o, op), |n: CfgStateV26, o: CfgStateV26| cfg_related_v26(n, o{op_argument})),\n{{\n assert forall|n: CfgStateV26, o: CfgStateV26| #[trigger] cfg_related_v26(n, o{op_argument}) implies\n cfg_step_n_v26(n, op).events == cfg_step_o_v26(o, op).events\n && cfg_step_n_v26(n, op).halted == cfg_step_o_v26(o, op).halted\n && cfg_related_v26(cfg_step_n_v26(n, op).state, cfg_step_o_v26(o, op).state{op_argument}) by {{\n if n.pc == -1 {{ }} else "
    );
    for target in 0..output.blocks().len() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            "if o.pc == {target} {{ cfg_block_step_refinement_{target}_v26(n, o, op); }} else "
        );
    }
    emit!(out, "{{ assert(false); }}\n }}\n}}\n");
    // The entry statement uses the independently emitted, actual signature
    // argument predicates. It does not assume the new relation as a premise.
    for function in input.functions() {
        out.budget.charge_work(2)?;
        if function.blocks.is_empty() {
            continue;
        }
        let ordinal = function.coordinate.0;
        let original = function.blocks.start;
        let target = live.inverse_blocks[original];
        let corresponding = output
            .functions()
            .get(ordinal as usize)
            .ok_or(Error::Statement("CFG exact output function entry"))?;
        if corresponding.blocks.start != target {
            return Err(Error::Statement("CFG function entry block mapping"));
        }
        for definition in 0..input.definitions().len() {
            out.budget.charge_work(1)?;
            if live.contains(original, definition)
                && !matches!(input.definitions()[definition].coordinate,
                    Definition::FunctionArgument { function: owner, .. } if owner == function.coordinate)
            {
                return Err(Error::Statement(
                    "CFG entry live-in lacks signature argument transport",
                ));
            }
        }
        emit!(
            out,
            "proof fn cfg_initial_relation_{ordinal}_v26(n: CfgStateV26, o: CfgStateV26, arguments: Seq<int>{op_parameter})\n requires cfg_entry_n_{ordinal}_v26(n, arguments), cfg_entry_o_{ordinal}_v26(o, arguments), n.memory == o.memory,\n ensures cfg_related_v26(n, o{op_argument}),\n{{ }}\n"
        );
        emit!(
            out,
            "proof fn cfg_function_trace_refinement_{ordinal}_v26(n: CfgStateV26, o: CfgStateV26, arguments: Seq<int>, op: spec_fn(int, int, Seq<int>, int) -> int, fuel: nat)\n requires cfg_entry_n_{ordinal}_v26(n, arguments), cfg_entry_o_{ordinal}_v26(o, arguments), n.memory == o.memory,\n ensures cfg_trace_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), n, fuel).events == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, op), o, fuel).events,\n cfg_trace_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), n, fuel).halted == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, op), o, fuel).halted,\n cfg_trace_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), n, fuel).steps == cfg_trace_v26(|s: CfgStateV26| cfg_step_o_v26(s, op), o, fuel).steps,\n{{\n cfg_initial_relation_{ordinal}_v26(n, o, arguments{op_argument});\n cfg_all_steps_refine_v26(op);\n cfg_finite_trace_refinement_v26(|s: CfgStateV26| cfg_step_n_v26(s, op), |s: CfgStateV26| cfg_step_o_v26(s, op), |a: CfgStateV26, b: CfgStateV26| cfg_related_v26(a, b{op_argument}), n, o, fuel);\n}}\n"
        );
    }
    emit!(
        out,
        "// These are generated proof obligations, not evidence of successful execution.\n// The shared operator interpretation still does not prove MIR lowering or\n// concrete device memory/call semantics, and this does not upgrade a block receipt.\n"
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn check_each_omission(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    check_omissions(input, output, rows, false, out)
}

#[cfg(test)]
pub(super) fn check_each_omission_cfg_v27(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    check_omissions(input, output, rows, true, out)
}

#[cfg(test)]
fn check_omissions(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Rows<'_>,
    congruence: bool,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let mut plan = Plan::build(input, output, rows, out)?;
    if congruence {
        plan.total_classes = Some(congruence_v27::build(
            input,
            output,
            &plan.output_operations,
            out,
        )?);
    }
    let mut live = Live::build(input, output, &plan, out)?;
    for ordinal in 0..live.tail {
        let cell = live.pending[ordinal];
        assert_eq!(live.cells[cell], 1);
        live.cells[cell] = NONE;
        assert!(matches!(
            live.validate(input, output, &plan, out),
            Err(Error::Statement(
                "CFG live-in closure omits an exact dependency"
            ))
        ));
        live.cells[cell] = 1;
    }
    Ok(live.tail)
}
