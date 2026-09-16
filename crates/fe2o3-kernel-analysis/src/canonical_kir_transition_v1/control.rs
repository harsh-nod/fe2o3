//! Exact successor occurrences, merge connectors, and ordered effect transport.
use super::{Budget, Error, INTERNAL, NONE, Result, State, index, payload};
use fe2o3_kernel_ir::{CanonicalKirUseCoordinateV1 as Use, ScalarType, Terminator};

fn selector_prefix(terminator: &Terminator) -> usize {
    usize::from(matches!(
        terminator,
        Terminator::ConditionalBranch { .. }
            | Terminator::Switch { .. }
            | Terminator::IntegerSwitch { .. }
    ))
}

impl State<'_, '_, '_, '_> {
    pub(super) fn edge_possible(&self, edge: usize, budget: &mut Budget<'_>) -> Result<bool> {
        budget.charge_work(1)?;
        let row = &self.input.edges()[edge];
        let block = &self.input.blocks()[index::block(self.input, row.coordinate.source, budget)?];
        if !matches!(block.terminator, Terminator::ConditionalBranch { .. }) {
            return Ok(true);
        }
        let condition = self.input.uses()[block.terminator_uses.start].definition;
        Ok(match self.literal(condition, budget)? {
            Some(super::Literal {
                ty: ScalarType::Bool,
                bits: 1,
            }) => row.coordinate.successor == 0,
            Some(super::Literal {
                ty: ScalarType::Bool,
                bits: 0,
            }) => row.coordinate.successor == 1,
            _ => true,
        })
    }

    pub(super) fn mark_reachable(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let input = self.input;
        budget.charge_work(self.reachable.len())?;
        self.reachable.fill(0);
        let mut length = 0_usize;
        for function in input.functions() {
            budget.charge_work(1)?;
            if !function.blocks.is_empty() {
                let entry = function.blocks.start;
                self.reachable[entry] = 1;
                *self.pending.get_mut(length).ok_or(Error::IncompleteRows)? = entry;
                length = length.checked_add(1).ok_or(Error::Arithmetic)?;
            }
        }
        while length != 0 {
            budget.charge_work(1)?;
            length -= 1;
            let block = self.pending[length];
            for edge in input.blocks()[block].edges.clone() {
                budget.charge_work(1)?;
                if !self.edge_possible(edge, budget)? {
                    continue;
                }
                let target = self.edge_target[edge];
                if self.reachable[target] == 0 {
                    self.reachable[target] = 1;
                    *self.pending.get_mut(length).ok_or(Error::IncompleteRows)? = target;
                    length = length.checked_add(1).ok_or(Error::Arithmetic)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn check_control(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        self.check_connectors(budget)?;
        for (ordinal, row) in self.rows.edges.iter().enumerate() {
            budget.charge_work(1)?;
            let output = &self.output.edges()[ordinal];
            if row.output != output.coordinate {
                return Err(Error::IncompleteRows);
            }
            let input = index::edge(self.input, row.input, budget)?;
            let source = index::block(self.output, output.coordinate.source, budget)?;
            let target = index::block(self.output, output.target, budget)?;
            if self.edge_output[input] != NONE
                || self.input.edges()[input].coordinate.source
                    != self.input.blocks()[self.block_tail[source]].coordinate
                || self.edge_target[input] != self.block_head[target]
            {
                return Err(Error::Rule("outgoing edge origin or destination"));
            }
            self.edge_output[input] = ordinal;
        }
        for (ordinal, block) in self.input.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            if self.reachable[ordinal] == 0 {
                continue;
            }
            if self.block_output[ordinal] == NONE {
                return Err(Error::Rule("reachable block removed"));
            }
            for edge in block.edges.clone() {
                budget.charge_work(1)?;
                if self.edge_possible(edge, budget)? && self.edge_output[edge] == NONE {
                    return Err(Error::Rule("executable edge occurrence removed"));
                }
            }
        }
        for block in 0..self.output.blocks().len() {
            self.check_terminator(block, budget)?;
        }
        self.check_edge_arguments(budget)
    }

    fn check_connectors(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let rows = self.rows;
        for row in rows.blocks {
            budget.charge_work(1)?;
            let segments = &rows.segments[index::range(row.segments, rows.segments.len(), budget)?];
            for pair in segments.windows(2) {
                budget.charge_work(1)?;
                let source = index::block(self.input, pair[0].input, budget)?;
                let target = index::block(self.input, pair[1].input, budget)?;
                let edge = index::edge(
                    self.input,
                    pair[0].connector.ok_or(Error::IncompleteRows)?,
                    budget,
                )?;
                let function = index::function(self.input, pair[1].input.function, budget)?;
                if self.input.edges()[edge].coordinate.source != pair[0].input
                    || self.edge_target[edge] != target
                    || self.edge_output[edge] != NONE
                    || self.input.functions()[function].blocks.start == target
                    || !self.edge_possible(edge, budget)?
                {
                    return Err(Error::Rule("merge connector"));
                }
                // Includes a default-only Switch/IntegerSwitch. A constant
                // conditional may first select its exact surviving occurrence.
                for candidate in self.input.blocks()[source].edges.clone() {
                    budget.charge_work(1)?;
                    if candidate != edge && self.edge_possible(candidate, budget)? {
                        return Err(Error::Rule("merge source has another executable edge"));
                    }
                }
                let mut incoming = self.incoming_head[target];
                while incoming != NONE {
                    budget.charge_work(1)?;
                    let predecessor = index::block(
                        self.input,
                        self.input.edges()[incoming].coordinate.source,
                        budget,
                    )?;
                    if incoming != edge
                        && self.reachable[predecessor] != 0
                        && self.edge_possible(incoming, budget)?
                    {
                        return Err(Error::Rule(
                            "merge target has another executable predecessor",
                        ));
                    }
                    incoming = self.incoming_next[incoming];
                }
                self.edge_output[edge] = INTERNAL;
            }
        }
        Ok(())
    }

    fn check_terminator(&self, output: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let block = &self.output.blocks()[output];
        let tail = &self.input.blocks()[self.block_tail[output]];
        let folded = matches!(
            (tail.terminator, block.terminator),
            (
                Terminator::ConditionalBranch { .. },
                Terminator::Branch { .. }
            )
        );
        let same = match (tail.terminator, block.terminator) {
            (Terminator::Branch { .. }, Terminator::Branch { .. })
            | (Terminator::ConditionalBranch { .. }, Terminator::ConditionalBranch { .. })
            | (Terminator::Unreachable, Terminator::Unreachable) => true,
            (Terminator::Return { values: a }, Terminator::Return { values: b }) => {
                a.len() == b.len()
            }
            (Terminator::Switch { cases: a, .. }, Terminator::Switch { cases: b, .. }) => {
                if a.len() != b.len() {
                    false
                } else {
                    let mut equal = true;
                    for (a, b) in a.iter().zip(b) {
                        budget.charge_work(1)?;
                        equal &= a.value == b.value;
                    }
                    equal
                }
            }
            (
                Terminator::IntegerSwitch { cases: a, .. },
                Terminator::IntegerSwitch { cases: b, .. },
            ) => {
                if a.len() != b.len() {
                    false
                } else {
                    let mut equal = true;
                    for (a, b) in a.iter().zip(b) {
                        budget.charge_work(1)?;
                        equal &= a.value == b.value;
                    }
                    equal
                }
            }
            _ => false,
        };
        if !same && !folded {
            return Err(Error::Rule("unsupported terminator transformation"));
        }
        if same && tail.edges.len() != block.edges.len() {
            return Err(Error::Rule("successor occurrence count"));
        }
        for (ordinal, edge) in block.edges.clone().enumerate() {
            budget.charge_work(1)?;
            let origin = index::edge(self.input, self.rows.edges[edge].input, budget)?;
            if folded {
                if block.edges.len() != 1 || !self.edge_possible(origin, budget)? {
                    return Err(Error::Rule("selected successor"));
                }
                for other in tail.edges.clone() {
                    budget.charge_work(1)?;
                    if other != origin && self.edge_possible(other, budget)? {
                        return Err(Error::Rule("unproved branch selection"));
                    }
                }
            } else if origin
                != tail
                    .edges
                    .start
                    .checked_add(ordinal)
                    .ok_or(Error::Arithmetic)?
            {
                return Err(Error::Rule("successor occurrence order"));
            }
        }
        let prefix = if matches!(block.terminator, Terminator::Return { .. }) {
            block.terminator_uses.len()
        } else {
            selector_prefix(block.terminator)
        };
        for operand in 0..prefix {
            budget.charge_work(1)?;
            let output = block
                .terminator_uses
                .start
                .checked_add(operand)
                .ok_or(Error::Arithmetic)?;
            let operand = u32::try_from(operand).map_err(|_| Error::Arithmetic)?;
            if self.rows.uses[output].input
                != (Use::TerminatorOperand {
                    block: tail.coordinate,
                    operand,
                })
            {
                return Err(Error::Rule("selector or return operand occurrence"));
            }
        }
        Ok(())
    }

    fn check_edge_arguments(&self, budget: &mut Budget<'_>) -> Result<()> {
        for (ordinal, row) in self.rows.edge_arguments.iter().enumerate() {
            budget.charge_work(1)?;
            let output = &self.output.edge_arguments()[ordinal];
            if output.coordinate != row.output {
                return Err(Error::IncompleteRows);
            }
            let source =
                &self.input.edge_arguments()[index::edge_argument(self.input, row.input, budget)?];
            let output_edge = index::edge(self.output, row.output.edge, budget)?;
            if row.input.edge != self.rows.edges[output_edge].input
                || self.anchors[output.target_definition] != source.target_definition
                || self.retained_anchors[output.target_definition] != 1
                || !self.has_descendant(
                    source.incoming_definition,
                    output.incoming_definition,
                    budget,
                )?
            {
                return Err(Error::Rule("edge argument parameter transport"));
            }
            let input_edge = index::edge(self.input, row.input.edge, budget)?;
            let input_use = self.edge_argument_use(true, input_edge, row.input.argument, budget)?;
            let output_use =
                self.edge_argument_use(false, output_edge, row.output.argument, budget)?;
            if self.rows.uses[output_use].input != self.input.uses()[input_use].coordinate {
                return Err(Error::Rule("edge argument use occurrence"));
            }
        }
        Ok(())
    }

    fn edge_argument_use(
        &self,
        input: bool,
        edge: usize,
        argument: u32,
        budget: &mut Budget<'_>,
    ) -> Result<usize> {
        budget.charge_work(1)?;
        let inventory = if input { self.input } else { self.output };
        let edge = &inventory.edges()[edge];
        let block = &inventory.blocks()[index::block(inventory, edge.coordinate.source, budget)?];
        let first = inventory.edges()[block.edges.start].bindings.start;
        let offset = edge
            .bindings
            .start
            .checked_sub(first)
            .ok_or(Error::Arithmetic)?;
        let argument = usize::try_from(argument).map_err(|_| Error::Arithmetic)?;
        if argument >= edge.bindings.len() {
            return Err(Error::InvalidCoordinate);
        }
        let index = block
            .terminator_uses
            .start
            .checked_add(selector_prefix(block.terminator))
            .and_then(|start| start.checked_add(offset))
            .and_then(|start| start.checked_add(argument))
            .ok_or(Error::Arithmetic)?;
        if index >= block.terminator_uses.end {
            return Err(Error::InvalidCoordinate);
        }
        Ok(index)
    }

    pub(super) fn check_order(&self, budget: &mut Budget<'_>) -> Result<()> {
        for (ordinal, operation) in self.input.operations().iter().enumerate() {
            budget.charge_work(1)?;
            let block = index::block(self.input, operation.coordinate.block, budget)?;
            if self.reachable[block] != 0
                && !payload::pure(&operation.operation.kind)
                && self.operation_output[ordinal] == NONE
                && !self.total_integer_identity(ordinal, budget)?
            {
                return Err(Error::Rule("executable ordered operation removed"));
            }
        }
        for block in self.output.blocks() {
            budget.charge_work(1)?;
            let mut previous = None;
            for operation in block.operations.clone() {
                budget.charge_work(1)?;
                if payload::pure(&self.output.operations()[operation].operation.kind) {
                    continue;
                }
                let origin = self.operation_input[operation];
                if origin == NONE {
                    return Err(Error::Rule("ordered operation synthesized"));
                }
                let coordinate = self.input.operations()[origin].coordinate;
                let source = index::block(self.input, coordinate.block, budget)?;
                let key = (self.block_position[source], coordinate.operation);
                if previous.is_some_and(|previous| previous >= key) {
                    return Err(Error::Rule("ordered operation order"));
                }
                previous = Some(key);
            }
        }
        Ok(())
    }
}
