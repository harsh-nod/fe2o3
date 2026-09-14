//! Monotone typed alias and scalar facts used only to check candidate substitutions.
//! No candidate row is assumed true while deriving these facts.
use super::{Budget, Error, Literal, NONE, Origin, Result, State, index, payload};
use crate::{
    CanonicalKirSparseConstantV1 as SparseConstant, CanonicalKirSparseValueV1 as SparseValue,
    canonical_kir_sparse_scalar_v1 as scalar,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as Use,
    Constant, OperationKind, ScalarType, UnaryOp, scalar_ops_v2 as math,
};

pub(super) fn literal(value: &Constant) -> Literal {
    let (ty, bits) = match *value {
        Constant::Bool(value) => (ScalarType::Bool, u128::from(value)),
        Constant::I8(value) => (ScalarType::I8, u128::from(value as u8)),
        Constant::I16(value) => (ScalarType::I16, u128::from(value as u16)),
        Constant::I32(value) => (ScalarType::I32, u128::from(value as u32)),
        Constant::I64(value) => (ScalarType::I64, u128::from(value as u64)),
        Constant::U8(value) => (ScalarType::U8, u128::from(value)),
        Constant::U16(value) => (ScalarType::U16, u128::from(value)),
        Constant::U32(value) => (ScalarType::U32, u128::from(value)),
        Constant::U64(value) => (ScalarType::U64, u128::from(value)),
        Constant::Index(value) => (ScalarType::Index, u128::from(value)),
        Constant::F16Bits(value) => (ScalarType::F16, u128::from(value)),
        Constant::Bf16Bits(value) => (ScalarType::Bf16, u128::from(value)),
        Constant::F32Bits(value) => (ScalarType::F32, u128::from(value)),
        Constant::F64Bits(value) => (ScalarType::F64, u128::from(value)),
    };
    Literal { ty, bits }
}

impl State<'_, '_, '_, '_> {
    pub(super) fn solve_values(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let input = self.input;
        let output = self.output;
        let rows = self.rows;
        for operation in input.operations() {
            budget.charge_work(1)?;
            if let OperationKind::Constant(value) = &operation.operation.kind {
                if operation.results.len() != 1 {
                    return Err(Error::Rule("constant result arity"));
                }
                self.publish_literal(operation.results.start, literal(value), budget)?;
            }
        }
        loop {
            budget.charge_work(1)?;
            self.mark_reachable(budget)?;
            let mut changed = self.phi_aliases(budget)?;
            for (ordinal, operation) in input.operations().iter().enumerate() {
                budget.charge_work(1)?;
                let block = index::block(input, operation.coordinate.block, budget)?;
                if self.reachable[block] == 0 {
                    continue;
                }
                changed |= self.select_alias(ordinal, budget)?;
                changed |= self.scalar_facts(ordinal, budget)?;
            }
            // Candidate pairs are obligations, not union premises. Only an
            // independently matching pure expression can add a CSE equality.
            for (definition, row) in rows.definitions.iter().enumerate() {
                budget.charge_work(1)?;
                for descendant in &rows.definition_outputs
                    [index::range(row.outputs, rows.definition_outputs.len(), budget)?]
                {
                    budget.charge_work(1)?;
                    let target = index::definition(output, descendant.output, budget)?;
                    let anchor = self.anchors[target];
                    if anchor == NONE {
                        return Err(Error::IncompleteRows);
                    }
                    if self.expression_equal(definition, anchor, budget)? {
                        changed |= self.union(definition, anchor, budget)?;
                    }
                }
            }
            if !changed {
                return Ok(());
            }
        }
    }

    fn phi_aliases(&mut self, budget: &mut Budget<'_>) -> Result<bool> {
        let input = self.input;
        let mut changed = false;
        for (block_index, block) in input.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            if self.reachable[block_index] == 0 || block.parameters.is_empty() {
                continue;
            }
            let function = index::function(input, block.coordinate.function, budget)?;
            // The initial entry value is not supplied by a CFG predecessor.
            // Backedges alone cannot establish its alias or constant value.
            if input.functions()[function].blocks.start == block_index {
                continue;
            }
            for (ordinal, parameter) in block.parameters.clone().enumerate() {
                budget.charge_work(1)?;
                let mut representative = NONE;
                let mut compatible = true;
                let mut incoming = self.incoming_head[block_index];
                while incoming != NONE {
                    budget.charge_work(1)?;
                    let edge = &input.edges()[incoming];
                    let source = index::block(input, edge.coordinate.source, budget)?;
                    if self.reachable[source] != 0 && self.edge_possible(incoming, budget)? {
                        let binding = edge
                            .bindings
                            .start
                            .checked_add(ordinal)
                            .ok_or(Error::Arithmetic)?;
                        if binding >= edge.bindings.end {
                            return Err(Error::Rule("input phi arity"));
                        }
                        let value = input.edge_arguments()[binding].incoming_definition;
                        if self.root(value, budget)? != self.root(parameter, budget)? {
                            if representative == NONE {
                                representative = value;
                            } else if !self.equal(representative, value, budget)? {
                                compatible = false;
                            }
                        }
                    }
                    incoming = self.incoming_next[incoming];
                }
                if compatible && representative != NONE {
                    changed |= self.union(parameter, representative, budget)?;
                }
            }
        }
        Ok(changed)
    }

    fn select_alias(&mut self, operation: usize, budget: &mut Budget<'_>) -> Result<bool> {
        let input = self.input;
        let row = &input.operations()[operation];
        if !matches!(row.operation.kind, OperationKind::Select { .. }) {
            return Ok(false);
        }
        if row.operands.len() != 3 || row.results.len() != 1 {
            return Err(Error::Rule("select shape"));
        }
        let condition = self.input.uses()[row.operands.start].definition;
        let yes = self.input.uses()[row.operands.start + 1].definition;
        let no = self.input.uses()[row.operands.start + 2].definition;
        let selected = match self.literal(condition, budget)? {
            Some(Literal {
                ty: ScalarType::Bool,
                bits: 1,
            }) => Some(yes),
            Some(Literal {
                ty: ScalarType::Bool,
                bits: 0,
            }) => Some(no),
            _ if self.equal(yes, no, budget)? => Some(yes),
            _ => None,
        };
        match selected {
            Some(selected) => self.union(row.results.start, selected, budget),
            None => Ok(false),
        }
    }

    fn scalar_facts(&mut self, operation: usize, budget: &mut Budget<'_>) -> Result<bool> {
        let input = self.input;
        let row = &input.operations()[operation];
        let eligible = matches!(
            row.operation.kind,
            OperationKind::Unary {
                op: UnaryOp::Not,
                ..
            } | OperationKind::Binary { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Cast { .. }
        );
        if !eligible {
            return Ok(false);
        }
        if row.operands.len() > 3 || row.results.len() > 2 {
            return Err(Error::Rule("scalar fold arity"));
        }
        let mut arguments = [scalar::Input {
            value: SparseValue::Dynamic,
            ty: None,
        }; 3];
        for (slot, use_index) in row.operands.clone().enumerate() {
            budget.charge_work(1)?;
            let definition = input.uses()[use_index].definition;
            let Some(value) = self.literal(definition, budget)? else {
                return Ok(false);
            };
            arguments[slot] = scalar::Input {
                value: SparseValue::Constant(SparseConstant {
                    ty: value.ty,
                    bits: value.bits,
                }),
                ty: Some(value.ty),
            };
        }
        let result_type = input
            .definitions()
            .get(row.results.start)
            .and_then(|definition| definition.ty.as_scalar());
        budget.charge_work(1)?;
        if let OperationKind::Binary { op, .. } = row.operation.kind {
            if arguments[0].ty.and_then(ScalarType::bit_width)
                != arguments[1].ty.and_then(ScalarType::bit_width)
            {
                return Ok(false);
            }
            // APInt's one-bit Boolean fold is distinct from ordinary integer
            // arithmetic and from target-sized Index values.
            if arguments[0].ty == Some(ScalarType::Bool) {
                let (SparseValue::Constant(a), SparseValue::Constant(b)) =
                    (arguments[0].value, arguments[1].value)
                else {
                    return Ok(false);
                };
                let bits = match op {
                    BinaryOp::BitAnd => a.bits & b.bits,
                    BinaryOp::BitOr => a.bits | b.bits,
                    BinaryOp::BitXor => a.bits ^ b.bits,
                    _ => return Ok(false),
                };
                return self.publish_literal(
                    row.results.start,
                    Literal {
                        ty: ScalarType::Bool,
                        bits,
                    },
                    budget,
                );
            }
            if matches!(op, BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply) {
                let Some(ty) = arguments[0].ty.and_then(scalar::fixed) else {
                    return Ok(false);
                };
                let (SparseValue::Constant(a), SparseValue::Constant(b)) =
                    (arguments[0].value, arguments[1].value)
                else {
                    return Ok(false);
                };
                let operator = match op {
                    BinaryOp::Add => math::IntBinary::Add,
                    BinaryOp::Subtract => math::IntBinary::Sub,
                    _ => math::IntBinary::Mul,
                };
                budget.charge_work(1)?;
                if !matches!(
                    math::evaluate_integer_binary(
                        ty,
                        operator,
                        math::IntMode::Overflowing,
                        a.bits,
                        b.bits
                    ),
                    Some(
                        math::IntOutcome::Overflowing {
                            overflowed: false,
                            ..
                        } | math::IntOutcome::Value(_)
                    )
                ) {
                    return Ok(false);
                }
            }
        }
        let transfer = scalar::transfer(&row.operation.kind, result_type, arguments);
        if transfer.exceptional {
            return Ok(false);
        }
        let mut changed = false;
        for (ordinal, definition) in row.results.clone().enumerate() {
            budget.charge_work(1)?;
            if let SparseValue::Constant(value) = transfer.values[ordinal] {
                changed |= self.publish_literal(
                    definition,
                    Literal {
                        ty: value.ty,
                        bits: value.bits,
                    },
                    budget,
                )?;
            }
        }
        Ok(changed)
    }

    fn expression_equal(&self, a: usize, b: usize, budget: &mut Budget<'_>) -> Result<bool> {
        if self.equal(a, b, budget)? {
            return Ok(true);
        }
        let (
            Definition::Result {
                operation: a_op,
                result: a_result,
            },
            Definition::Result {
                operation: b_op,
                result: b_result,
            },
        ) = (
            self.input.definitions()[a].coordinate,
            self.input.definitions()[b].coordinate,
        )
        else {
            return Ok(false);
        };
        if a_result != b_result || a_op.block.function != b_op.block.function {
            return Ok(false);
        }
        let a = &self.input.operations()[index::operation(self.input, a_op, budget)?];
        let b = &self.input.operations()[index::operation(self.input, b_op, budget)?];
        if !payload::pure(&a.operation.kind)
            || !payload::pure(&b.operation.kind)
            || a.results.len() != b.results.len()
            || a.operands.len() != b.operands.len()
            || !payload::operation(&a.operation.kind, &b.operation.kind, budget)?
        {
            return Ok(false);
        }
        for (a, b) in a.results.clone().zip(b.results.clone()) {
            if !payload::ty(
                self.input.definitions()[a].ty,
                self.input.definitions()[b].ty,
                budget,
            )? {
                return Ok(false);
            }
        }
        for (a, b) in a.operands.clone().zip(b.operands.clone()) {
            budget.charge_work(1)?;
            if !self.equal(
                self.input.uses()[a].definition,
                self.input.uses()[b].definition,
                budget,
            )? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn check_values_and_uses(&self, budget: &mut Budget<'_>) -> Result<()> {
        for (definition, row) in self.rows.definitions.iter().enumerate() {
            budget.charge_work(1)?;
            for descendant in &self.rows.definition_outputs
                [index::range(row.outputs, self.rows.definition_outputs.len(), budget)?]
            {
                budget.charge_work(1)?;
                let output = index::definition(self.output, descendant.output, budget)?;
                if !self.equal(definition, self.anchors[output], budget)? {
                    return Err(Error::Rule("unproved typed substitution"));
                }
            }
        }
        for (ordinal, row) in self.rows.operations.iter().enumerate() {
            budget.charge_work(1)?;
            if let Origin::ConstantFrom(source) = row.origin {
                let source = index::definition(self.input, source, budget)?;
                let OperationKind::Constant(value) =
                    &self.output.operations()[ordinal].operation.kind
                else {
                    return Err(Error::Rule("nonconstant synthesis"));
                };
                if self.literal(source, budget)? != Some(literal(value)) {
                    return Err(Error::Rule("constant synthesis bits"));
                }
            }
        }
        for (ordinal, row) in self.rows.uses.iter().enumerate() {
            budget.charge_work(1)?;
            let input = index::used(self.input, row.input, budget)?;
            let output = &self.output.uses()[ordinal];
            if output.coordinate != row.output {
                return Err(Error::IncompleteRows);
            }
            let source_definition = self.input.uses()[input].definition;
            if !self.has_descendant(source_definition, output.definition, budget)? {
                return Err(Error::Rule("final operand has no exact descendant"));
            }
            if let Use::OperationOperand { operation, operand } = row.output {
                let operation = index::operation(self.output, operation, budget)?;
                let origin = self.operation_input[operation];
                if origin == NONE
                    || row.input
                        != (Use::OperationOperand {
                            operation: self.input.operations()[origin].coordinate,
                            operand,
                        })
                {
                    return Err(Error::Rule("final operation operand origin"));
                }
            }
        }
        Ok(())
    }

    pub(super) fn has_descendant(
        &self,
        input: usize,
        output: usize,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        let row = self
            .rows
            .definitions
            .get(input)
            .ok_or(Error::InvalidCoordinate)?;
        for descendant in &self.rows.definition_outputs
            [index::range(row.outputs, self.rows.definition_outputs.len(), budget)?]
        {
            budget.charge_work(1)?;
            if descendant.output == self.output.definitions()[output].coordinate {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
