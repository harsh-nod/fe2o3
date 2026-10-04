//! Strict CSE-only framing. No CFG/phi folding, synthesis or unrelated DCE.
use super::{
    Budget, Definition, DescendantKind, Error, NONE, Origin, PROVED, Result, State, index, payload,
};
use fe2o3_kernel_ir::{CanonicalKirUseCoordinateV1 as Use, Terminator};

impl State<'_, '_, '_, '_> {
    pub(super) fn check_shape(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let a = self.input;
        let b = self.output;
        let rows = self.rows;
        budget.charge_work(1)?;
        if a.functions().len() != b.functions().len()
            || a.blocks().len() != b.blocks().len()
            || a.edges().len() != b.edges().len()
            || a.edge_arguments().len() != b.edge_arguments().len()
            || rows.functions.len() != b.functions().len()
            || rows.blocks.len() != b.blocks().len()
            || rows.segments.len() != b.blocks().len()
            || rows.operations.len() != b.operations().len()
            || rows.definitions.len() != a.definitions().len()
            || rows.definition_outputs.len() != a.definitions().len()
            || rows.uses.len() != b.uses().len()
            || rows.edges.len() != b.edges().len()
            || rows.edge_arguments.len() != b.edge_arguments().len()
        {
            return Err(Error::Rule("complete CSE-only row counts"));
        }
        self.declarations(budget)?;
        for (ordinal, (input, output)) in a.blocks().iter().zip(b.blocks()).enumerate() {
            budget.charge_work(2)?;
            let row = rows.blocks[ordinal];
            let range = index::range(row.segments, rows.segments.len(), budget)?;
            let segment = rows.segments[ordinal];
            if input.coordinate != output.coordinate
                || row.output != output.coordinate
                || range != (ordinal..ordinal + 1)
                || segment.input != input.coordinate
                || segment.connector.is_some()
                || input.parameters.len() != output.parameters.len()
                || input.terminator_uses.len() != output.terminator_uses.len()
                || input.edges.len() != output.edges.len()
                || !same_terminator(input.terminator, output.terminator, budget)?
            {
                return Err(Error::Rule("unchanged block/parameter/terminator shape"));
            }
        }
        for (ordinal, output) in b.definitions().iter().enumerate() {
            budget.charge_work(1)?;
            if !matches!(output.coordinate, Definition::Result { .. }) {
                let input = index::definition(a, output.coordinate, budget)?;
                if !payload::ty(a.definitions()[input].ty, output.ty, budget)? {
                    return Err(Error::Rule("argument identity/type"));
                }
                self.anchors[ordinal] = input;
            }
        }
        let mut previous = None;
        for (ordinal, (output, row)) in b.operations().iter().zip(rows.operations).enumerate() {
            budget.charge_work(1)?;
            let Origin::Retained(source) = row.origin else {
                return Err(Error::Rule("no synthesized operations"));
            };
            let source_index = index::operation(a, source, budget)?;
            let input = &a.operations()[source_index];
            if row.output != output.coordinate
                || source.block != output.coordinate.block
                || previous.is_some_and(|previous| previous >= source_index)
                || self.retained[source_index] != NONE
                || input.operands.len() != output.operands.len()
                || input.results.len() != output.results.len()
                || !payload::operation(&input.operation.kind, &output.operation.kind, budget)?
            {
                return Err(Error::Rule("exact retained operation payload/order"));
            }
            previous = Some(source_index);
            self.retained[source_index] = ordinal;
            self.origins[ordinal] = source_index;
            for (source, target) in input.results.clone().zip(output.results.clone()) {
                budget.charge_work(1)?;
                if !payload::ty(
                    a.definitions()[source].ty,
                    b.definitions()[target].ty,
                    budget,
                )? {
                    return Err(Error::Rule("retained result type"));
                }
                self.anchors[target] = source;
            }
        }
        for (ordinal, (input, row)) in a.definitions().iter().zip(rows.definitions).enumerate() {
            budget.charge_work(2)?;
            let range = index::range(row.outputs, rows.definition_outputs.len(), budget)?;
            if row.input != input.coordinate || range != (ordinal..ordinal + 1) {
                return Err(Error::Rule("one exact descendant per definition"));
            }
            let descendant = rows.definition_outputs[ordinal];
            let target = index::definition(b, descendant.output, budget)?;
            let anchor = self.anchors[target];
            if anchor == NONE
                || !payload::ty(input.ty, b.definitions()[target].ty, budget)?
                || index::definition_function(input.coordinate)
                    != index::definition_function(descendant.output)
            {
                return Err(Error::Rule("descendant owner/type/anchor"));
            }
            self.descendants[ordinal] = target;
            if anchor == ordinal {
                if descendant.kind != DescendantKind::Retained || self.seen[target] != 0 {
                    return Err(Error::Rule("exact retained definition"));
                }
                self.seen[target] = 1;
                self.status[ordinal] = PROVED;
            } else {
                if descendant.kind != DescendantKind::Substituted {
                    return Err(Error::Rule("only omitted definitions may substitute"));
                }
                let removed = self.eligible(ordinal, budget)?;
                let retained = self.eligible(anchor, budget)?;
                if self.retained[removed] != NONE || self.retained[retained] == NONE {
                    return Err(Error::Rule("substitution requires omitted/retained pair"));
                }
            }
        }
        for seen in &self.seen {
            budget.charge_work(1)?;
            if *seen != 1 {
                return Err(Error::Rule("complete unique final retained definitions"));
            }
        }
        for (ordinal, operation) in a.operations().iter().enumerate() {
            budget.charge_work(1)?;
            if self.retained[ordinal] == NONE {
                if operation.results.len() != 1
                    || self.eligible(operation.results.start, budget)? != ordinal
                {
                    return Err(Error::Rule(
                        "only eligible bitwise operations may disappear",
                    ));
                }
                if self.status[operation.results.start] == PROVED {
                    return Err(Error::Rule("deleted definition labeled retained"));
                }
            }
        }
        for (ordinal, ((input, output), row)) in
            a.edges().iter().zip(b.edges()).zip(rows.edges).enumerate()
        {
            budget.charge_work(1)?;
            if input.coordinate != output.coordinate
                || row.input != input.coordinate
                || row.output != output.coordinate
                || input.target != output.target
                || input.bindings.len() != output.bindings.len()
                || index::edge(a, row.input, budget)? != ordinal
            {
                return Err(Error::Rule("exact CFG successor occurrence"));
            }
        }
        Ok(())
    }

    fn declarations(&self, budget: &mut Budget<'_>) -> Result<()> {
        let a = self.input.owner().module();
        let b = self.output.owner().module();
        budget.charge_work(1)?;
        if !payload::bytes(a.id.as_str().as_bytes(), b.id.as_str().as_bytes(), budget)?
            || !payload::capabilities(&a.required_capabilities, &b.required_capabilities, budget)?
            || a.kernels.len() != b.kernels.len()
        {
            return Err(Error::Rule("module metadata"));
        }
        for (a, b) in a.kernels.iter().zip(&b.kernels) {
            budget.charge_work(1)?;
            if !payload::bytes(a.id.as_str().as_bytes(), b.id.as_str().as_bytes(), budget)?
                || !payload::bytes(
                    a.entry.as_str().as_bytes(),
                    b.entry.as_str().as_bytes(),
                    budget,
                )?
                || a.domain != b.domain
                || a.workgroup_size != b.workgroup_size
                || !payload::capabilities(
                    &a.required_capabilities,
                    &b.required_capabilities,
                    budget,
                )?
            {
                return Err(Error::Rule("kernel declaration"));
            }
        }
        for ((a, b), row) in self
            .input
            .functions()
            .iter()
            .zip(self.output.functions())
            .zip(self.rows.functions)
        {
            budget.charge_work(1)?;
            if a.coordinate != b.coordinate
                || row.input != a.coordinate
                || row.output != b.coordinate
                || a.blocks.len() != b.blocks.len()
                || a.function.role != b.function.role
                || a.function.body.is_some() != b.function.body.is_some()
                || !payload::bytes(
                    a.function.id.as_str().as_bytes(),
                    b.function.id.as_str().as_bytes(),
                    budget,
                )?
                || !payload::types(
                    &a.function.signature.parameters,
                    &b.function.signature.parameters,
                    budget,
                )?
                || !payload::types(
                    &a.function.signature.results,
                    &b.function.signature.results,
                    budget,
                )?
                || !payload::capabilities(
                    &a.function.required_capabilities,
                    &b.function.required_capabilities,
                    budget,
                )?
            {
                return Err(Error::Rule("exact function declaration/order"));
            }
        }
        Ok(())
    }

    pub(super) fn check_uses(&self, budget: &mut Budget<'_>) -> Result<()> {
        let a = self.input;
        let b = self.output;
        for (output, row) in b.uses().iter().zip(self.rows.uses) {
            budget.charge_work(1)?;
            if output.coordinate != row.output {
                return Err(Error::Rule("complete output uses"));
            }
            let expected = match output.coordinate {
                Use::OperationOperand { operation, operand } => {
                    let output = index::operation(b, operation, budget)?;
                    Use::OperationOperand {
                        operation: a.operations()[self.origins[output]].coordinate,
                        operand,
                    }
                }
                Use::TerminatorOperand { block, operand } => {
                    Use::TerminatorOperand { block, operand }
                }
            };
            if row.input != expected {
                return Err(Error::Rule("exact positional use origin"));
            }
            let input = index::used(a, expected, budget)?;
            let definition = a.uses()[input].definition;
            if self.status[definition] != PROVED
                || self.descendants[definition] != output.definition
            {
                return Err(Error::Rule("unproved final operand transport"));
            }
        }
        for ((input, output), row) in a
            .edge_arguments()
            .iter()
            .zip(b.edge_arguments())
            .zip(self.rows.edge_arguments)
        {
            budget.charge_work(1)?;
            if input.coordinate != output.coordinate
                || row.input != input.coordinate
                || row.output != output.coordinate
                || self.status[input.incoming_definition] != PROVED
                || self.descendants[input.incoming_definition] != output.incoming_definition
                || self.descendants[input.target_definition] != output.target_definition
            {
                return Err(Error::Rule("exact edge argument transport"));
            }
        }
        Ok(())
    }
}

fn same_terminator(a: &Terminator, b: &Terminator, budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(1)?;
    Ok(match (a, b) {
        (Terminator::Branch { .. }, Terminator::Branch { .. })
        | (Terminator::ConditionalBranch { .. }, Terminator::ConditionalBranch { .. })
        | (Terminator::Unreachable, Terminator::Unreachable) => true,
        (Terminator::Return { values: a }, Terminator::Return { values: b }) => a.len() == b.len(),
        (Terminator::Switch { cases: a, .. }, Terminator::Switch { cases: b, .. }) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                budget.charge_work(1)?;
                if a.value != b.value {
                    return Ok(false);
                }
            }
            true
        }
        (
            Terminator::IntegerSwitch { cases: a, .. },
            Terminator::IntegerSwitch { cases: b, .. },
        ) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                budget.charge_work(1)?;
                if a.value != b.value {
                    return Ok(false);
                }
            }
            true
        }
        _ => false,
    })
}
