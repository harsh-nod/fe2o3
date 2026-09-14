//! Complete coordinate framing, exact signatures, and observed retained anchors.
use super::{Budget, DescendantKind, Error, NONE, Origin, Result, State, index, payload};
use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Definition, OperationKind};

impl State<'_, '_, '_, '_> {
    pub(super) fn check_structure(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        self.check_module(budget)?;
        self.install_blocks(budget)?;
        self.install_operations(budget)?;
        self.install_definitions(budget)
    }

    fn check_module(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let input = self.input;
        let output = self.output;
        let rows = self.rows;
        let a = input.owner().module();
        let b = output.owner().module();
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
        for (ordinal, row) in rows.functions.iter().enumerate() {
            budget.charge_work(1)?;
            if output.functions()[ordinal].coordinate != row.output {
                return Err(Error::IncompleteRows);
            }
            let source = index::function(input, row.input, budget)?;
            if self.function_output[source] != NONE {
                return Err(Error::Rule("duplicate function association"));
            }
            let a = input.functions()[source].function;
            let b = output.functions()[ordinal].function;
            if !payload::bytes(a.id.as_str().as_bytes(), b.id.as_str().as_bytes(), budget)?
                || a.role != b.role
                || a.body.is_some() != b.body.is_some()
                || !payload::types(&a.signature.parameters, &b.signature.parameters, budget)?
                || !payload::types(&a.signature.results, &b.signature.results, budget)?
                || !payload::capabilities(
                    &a.required_capabilities,
                    &b.required_capabilities,
                    budget,
                )?
            {
                return Err(Error::Rule("exact function signature and declaration"));
            }
            self.function_input[ordinal] = source;
            self.function_output[source] = ordinal;
            for argument in 0..a.signature.parameters.len() {
                budget.charge_work(1)?;
                let input_definition = input.functions()[source]
                    .definitions
                    .start
                    .checked_add(argument)
                    .ok_or(Error::Arithmetic)?;
                let output_definition = output.functions()[ordinal]
                    .definitions
                    .start
                    .checked_add(argument)
                    .ok_or(Error::Arithmetic)?;
                self.anchors[output_definition] = input_definition;
            }
        }
        Ok(())
    }

    fn install_blocks(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let input = self.input;
        let output = self.output;
        let rows = self.rows;
        let mut consumed = 0;
        for (ordinal, row) in rows.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            if output.blocks()[ordinal].coordinate != row.output {
                return Err(Error::IncompleteRows);
            }
            let range = index::range(row.segments, rows.segments.len(), budget)?;
            if range.start != consumed || range.is_empty() {
                return Err(Error::IncompleteRows);
            }
            consumed = range.end;
            let output_function = index::function(output, row.output.function, budget)?;
            for (position, segment) in rows.segments[range.clone()].iter().enumerate() {
                budget.charge_work(1)?;
                let block = index::block(input, segment.input, budget)?;
                let function = index::function(input, segment.input.function, budget)?;
                if self.function_output[function] != output_function
                    || self.block_output[block] != NONE
                {
                    return Err(Error::Rule("block chain owner or duplicate"));
                }
                if segment.connector.is_some() != (position + 1 < range.len()) {
                    return Err(Error::Rule("block chain connector framing"));
                }
                self.block_output[block] = ordinal;
                self.block_position[block] = position;
                if position == 0 {
                    self.block_head[ordinal] = block;
                }
                self.block_tail[ordinal] = block;
            }
        }
        if consumed != rows.segments.len() {
            return Err(Error::IncompleteRows);
        }
        for (output_function, source) in self.function_input.iter().copied().enumerate() {
            budget.charge_work(1)?;
            let a = &input.functions()[source].blocks;
            let b = &output.functions()[output_function].blocks;
            if a.is_empty() != b.is_empty()
                || (!a.is_empty() && self.block_head[b.start] != a.start)
            {
                return Err(Error::Rule("function entry chain"));
            }
        }
        Ok(())
    }

    fn install_operations(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let input = self.input;
        let output = self.output;
        let rows = self.rows;
        for (ordinal, row) in rows.operations.iter().enumerate() {
            budget.charge_work(1)?;
            let operation = &output.operations()[ordinal];
            if operation.coordinate != row.output {
                return Err(Error::IncompleteRows);
            }
            let output_block = index::block(output, row.output.block, budget)?;
            match row.origin {
                Origin::Retained(source) => {
                    let source = index::operation(input, source, budget)?;
                    let source_operation = &input.operations()[source];
                    let source_block =
                        index::block(input, source_operation.coordinate.block, budget)?;
                    if self.operation_output[source] != NONE
                        || self.block_output[source_block] != output_block
                    {
                        return Err(Error::Rule("retained operation owner or duplication"));
                    }
                    if source_operation.results.len() != operation.results.len()
                        || source_operation.operands.len() != operation.operands.len()
                        || !payload::operation(
                            &source_operation.operation.kind,
                            &operation.operation.kind,
                            budget,
                        )?
                    {
                        return Err(Error::Rule("retained operation payload"));
                    }
                    for (a, b) in source_operation
                        .results
                        .clone()
                        .zip(operation.results.clone())
                    {
                        if !payload::ty(
                            input.definitions()[a].ty,
                            output.definitions()[b].ty,
                            budget,
                        )? {
                            return Err(Error::Rule("retained result type"));
                        }
                        self.anchors[b] = a;
                    }
                    self.operation_output[source] = ordinal;
                    self.operation_input[ordinal] = source;
                }
                Origin::ConstantFrom(source) => {
                    let source = index::definition(input, source, budget)?;
                    if !matches!(operation.operation.kind, OperationKind::Constant(_))
                        || operation.results.len() != 1
                        || !operation.operands.is_empty()
                    {
                        return Err(Error::Rule("constant-only synthesis"));
                    }
                    let target = operation.results.start;
                    if !payload::ty(
                        input.definitions()[source].ty,
                        output.definitions()[target].ty,
                        budget,
                    )? {
                        return Err(Error::Rule("synthesized constant type"));
                    }
                    let source_function = index::function(
                        input,
                        index::definition_function(input.definitions()[source].coordinate),
                        budget,
                    )?;
                    let output_function =
                        index::function(output, row.output.block.function, budget)?;
                    if self.function_output[source_function] != output_function {
                        return Err(Error::Rule("synthesized constant function"));
                    }
                    self.anchors[target] = source;
                    self.retained_anchors[target] = 2;
                }
            }
        }
        Ok(())
    }

    fn install_definitions(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let input = self.input;
        let output = self.output;
        let rows = self.rows;
        let mut consumed = 0;
        for (source, row) in rows.definitions.iter().enumerate() {
            budget.charge_work(1)?;
            if input.definitions()[source].coordinate != row.input {
                return Err(Error::IncompleteRows);
            }
            let range = index::range(row.outputs, rows.definition_outputs.len(), budget)?;
            if range.start != consumed {
                return Err(Error::IncompleteRows);
            }
            consumed = range.end;
            let mut previous = None;
            for descendant in &rows.definition_outputs[range] {
                budget.charge_work(1)?;
                if previous.is_some_and(|value| value >= descendant.output) {
                    return Err(Error::Rule("descendant order or duplication"));
                }
                previous = Some(descendant.output);
                let target = index::definition(output, descendant.output, budget)?;
                let source_function =
                    index::function(input, index::definition_function(row.input), budget)?;
                let target_function = index::function(
                    output,
                    index::definition_function(descendant.output),
                    budget,
                )?;
                if self.function_output[source_function] != target_function
                    || !payload::ty(
                        input.definitions()[source].ty,
                        output.definitions()[target].ty,
                        budget,
                    )?
                {
                    return Err(Error::Rule("descendant owner or type"));
                }
                if descendant.kind == DescendantKind::Retained {
                    if self.retained_anchors[target] != 0 {
                        return Err(Error::Rule("retained definition duplication"));
                    }
                    if self.anchors[target] == NONE {
                        let (
                            Definition::BlockArgument { block: a, .. },
                            Definition::BlockArgument { block: b, .. },
                        ) = (row.input, descendant.output)
                        else {
                            return Err(Error::Rule("unanchored retained definition"));
                        };
                        let a = index::block(input, a, budget)?;
                        let b = index::block(output, b, budget)?;
                        if self.block_head[b] != a {
                            return Err(Error::Rule("retained block parameter origin"));
                        }
                        self.anchors[target] = source;
                    }
                    if self.anchors[target] != source {
                        return Err(Error::Rule("retained definition identity"));
                    }
                    self.retained_anchors[target] = 1;
                }
            }
        }
        if consumed != rows.definition_outputs.len() {
            return Err(Error::IncompleteRows);
        }
        for target in 0..output.definitions().len() {
            budget.charge_work(1)?;
            if self.anchors[target] == NONE
                || self.retained_anchors[target] == 0
                || !self.has_descendant(self.anchors[target], target, budget)?
            {
                return Err(Error::Rule("complete final definition anchors"));
            }
        }
        for block in output.blocks() {
            budget.charge_work(1)?;
            let mut previous = None;
            for parameter in block.parameters.clone() {
                budget.charge_work(1)?;
                let Definition::BlockArgument { argument, .. } =
                    input.definitions()[self.anchors[parameter]].coordinate
                else {
                    return Err(Error::Rule("final block parameter identity"));
                };
                if previous.is_some_and(|value| value >= argument) {
                    return Err(Error::Rule("retained parameter order"));
                }
                previous = Some(argument);
            }
        }
        Ok(())
    }
}
