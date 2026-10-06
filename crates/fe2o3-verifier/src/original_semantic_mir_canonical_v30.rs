//! The target model reads only actual canonical operations, independently of MIR.

use super::super::{Error, Inventory, Resource, Result, Writer};
use super::{ExpressionV30, NodeV30, OperatorV30, ScalarV30, vector};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirDefinitionCoordinateV1 as Definition, ComparePredicate,
    OperationKind as Kind, ScalarType, Terminator, Type, UnaryOp,
};
use std::mem::size_of;

#[path = "original_semantic_mir_target_control_v31.rs"]
pub(super) mod control;

pub(super) struct CanonicalProgramV30 {
    pub nodes: Vec<NodeV30>,
    pub definitions: Vec<Option<usize>>,
    pub definition_start: usize,
    pub returned: Option<usize>,
    pub arguments: usize,
}

pub(super) fn scalar(ty: &Type) -> Result<ScalarV30> {
    if *ty == Type::BOOL {
        return Ok(ScalarV30::Bool);
    }
    let (width, signed) = match ty.as_scalar() {
        Some(ScalarType::U8) => (8, false),
        Some(ScalarType::I8) => (8, true),
        Some(ScalarType::U16) => (16, false),
        Some(ScalarType::I16) => (16, true),
        Some(ScalarType::U32) => (32, false),
        Some(ScalarType::I32) => (32, true),
        Some(ScalarType::U64) => (64, false),
        Some(ScalarType::I64) => (64, true),
        _ => {
            return Err(Error::Statement(
                "original MIR target scalar type is not modeled",
            ));
        }
    };
    Ok(ScalarV30::Integer { width, signed })
}

impl CanonicalProgramV30 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        function: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let function = inventory
            .functions()
            .get(function)
            .ok_or(Error::Statement("original MIR target function is absent"))?;
        let arguments = function.function.signature.parameters.len();
        let count = function
            .operations
            .len()
            .checked_add(arguments)
            .ok_or(Resource::Arithmetic)?;
        out.budget.reserve_storage(
            size_of::<Self>()
                + size_of::<Result<Self>>()
                + size_of::<[usize; 3]>()
                + operation_headers_v31(),
        )?;
        let mut result = Self {
            nodes: vector(count, out)?,
            definitions: vector(function.definitions.len(), out)?,
            definition_start: function.definitions.start,
            returned: None,
            arguments,
        };
        out.budget.charge_work(function.definitions.len())?;
        result.definitions.resize(function.definitions.len(), None);
        for ordinal in 0..arguments {
            out.budget.charge_work(3)?;
            let index = function
                .definitions
                .start
                .checked_add(ordinal)
                .ok_or(Resource::Arithmetic)?;
            let definition = inventory
                .definitions()
                .get(index)
                .ok_or(Error::Statement("original MIR target parameter is absent"))?;
            if definition.coordinate
                != (Definition::FunctionArgument {
                    function: function.coordinate,
                    argument: u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                })
            {
                return Err(Error::Statement(
                    "original MIR target argument coordinate differs",
                ));
            }
            let node = result.push(
                NodeV30 {
                    scalar: scalar(definition.ty)?,
                    expression: ExpressionV30::Argument(ordinal as u32),
                },
                out,
            )?;
            result.definitions[ordinal] = Some(node);
        }
        let mut visited: Vec<bool> = vector(function.blocks.len(), out)?;
        out.budget.charge_work(function.blocks.len())?;
        visited.resize(function.blocks.len(), false);
        let mut block = function.blocks.start;
        loop {
            out.budget.charge_work(4)?;
            if block >= function.blocks.end || block < function.blocks.start {
                return Err(Error::Statement("original MIR target block is foreign"));
            }
            let seen = &mut visited[block - function.blocks.start];
            if *seen {
                return Err(Error::Statement(
                    "original MIR target cyclic control is not modeled",
                ));
            }
            *seen = true;
            let row = &inventory.blocks()[block];
            if !row.parameters.is_empty() {
                return Err(Error::Statement("original MIR target phi is not modeled"));
            }
            for operation in row.operations.clone() {
                out.budget.charge_work(8)?;
                let operation = &inventory.operations()[operation];
                if operation.results.len() != 1 || !operation.effects.is_empty() {
                    return Err(Error::Statement(
                        "original MIR target effect is not modeled",
                    ));
                }
                let ty = scalar(inventory.definitions()[operation.results.start].ty)?;
                let mut input = [0usize; 3];
                if operation.operands.len() > input.len() {
                    return Err(Error::Statement("original MIR target scalar arity differs"));
                }
                for (position, at) in operation.operands.clone().enumerate() {
                    if position == 2 {
                        out.budget.charge_work(1)?;
                    }
                    input[position] = result.value(inventory.uses()[at].definition)?;
                }
                let expression = operation_expression(
                    &operation.operation.kind,
                    &input[..operation.operands.len()],
                    ty,
                    &result.nodes,
                )?;
                let node = result.push(
                    NodeV30 {
                        scalar: ty,
                        expression,
                    },
                    out,
                )?;
                let slot = operation
                    .results
                    .start
                    .checked_sub(result.definition_start)
                    .ok_or(Resource::Accounting)?;
                if result
                    .definitions
                    .get_mut(slot)
                    .ok_or(Resource::Accounting)?
                    .replace(node)
                    .is_some()
                {
                    return Err(Error::Statement(
                        "original MIR target definition is repeated",
                    ));
                }
            }
            match row.terminator {
                Terminator::Branch { arguments, .. }
                    if arguments.is_empty() && row.edges.len() == 1 =>
                {
                    let target = inventory.edges()[row.edges.start].target;
                    if target.function != function.coordinate {
                        return Err(Error::Statement(
                            "original MIR target successor function differs",
                        ));
                    }
                    block = function
                        .blocks
                        .start
                        .checked_add(target.block as usize)
                        .ok_or(Resource::Arithmetic)?;
                }
                Terminator::Return { values } if values.len() <= 1 && row.edges.is_empty() => {
                    if values.len() != row.terminator_uses.len() {
                        return Err(Error::Statement("original MIR target return arity differs"));
                    }
                    if let Some(operand) = inventory
                        .uses()
                        .get(row.terminator_uses.start)
                        .filter(|_| !values.is_empty())
                    {
                        result.returned = Some(result.value(operand.definition)?);
                    }
                    break;
                }
                _ => {
                    return Err(Error::Statement(
                        "original MIR target control is not modeled",
                    ));
                }
            }
        }
        out.budget.charge_work(visited.len())?;
        if visited.iter().any(|seen| !seen) {
            return Err(Error::Statement(
                "original MIR target block coverage is incomplete",
            ));
        }
        Ok(result)
    }

    fn push(&mut self, node: NodeV30, out: &mut Writer<'_, '_>) -> Result<usize> {
        out.budget.charge_work(1)?;
        if self.nodes.len() == self.nodes.capacity() {
            return Err(Resource::Accounting.into());
        }
        let index = self.nodes.len();
        self.nodes.push(node);
        Ok(index)
    }

    pub(super) fn value(&self, definition: usize) -> Result<usize> {
        definition
            .checked_sub(self.definition_start)
            .and_then(|index| self.definitions.get(index))
            .copied()
            .flatten()
            .ok_or(Error::Statement(
                "original MIR target scalar definition is unavailable",
            ))
    }
}

pub(super) fn operation_headers_v31() -> usize {
    size_of::<(
        &Kind,
        &[usize],
        ScalarV30,
        &[NodeV30],
        ExpressionV30,
        Result<ExpressionV30>,
    )>()
}

pub(super) fn operation_expression(
    kind: &Kind,
    input: &[usize],
    ty: ScalarV30,
    nodes: &[NodeV30],
) -> Result<ExpressionV30> {
    Ok(match kind {
        Kind::Constant(value) if input.is_empty() => {
            ExpressionV30::Constant(super::super::bits(value))
        }
        Kind::Select { .. } if input.len() == 3 => {
            if nodes.get(input[0]).map(|node| node.scalar) != Some(ScalarV30::Bool)
                || nodes.get(input[1]).map(|node| node.scalar) != Some(ty)
                || nodes.get(input[2]).map(|node| node.scalar) != Some(ty)
                || ty == ScalarV30::Unit
            {
                return Err(Error::Statement(
                    "original MIR target select scalar types differ",
                ));
            }
            ExpressionV30::Select {
                condition: input[0],
                true_value: input[1],
                false_value: input[2],
            }
        }
        Kind::Unary {
            op: UnaryOp::Not, ..
        } if input.len() == 1 => {
            if nodes[input[0]].scalar != ty {
                return Err(Error::Statement(
                    "original MIR target unary scalar type differs",
                ));
            }
            ExpressionV30::Not(input[0])
        }
        Kind::Binary { op, .. } if input.len() == 2 => {
            let operation = match op {
                BinaryOp::BitAnd => OperatorV30::And,
                BinaryOp::BitOr => OperatorV30::Or,
                BinaryOp::BitXor => OperatorV30::Xor,
                _ => {
                    return Err(Error::Statement(
                        "original MIR target arithmetic/effect contract is not modeled",
                    ));
                }
            };
            if nodes[input[0]].scalar != ty || nodes[input[1]].scalar != ty {
                return Err(Error::Statement(
                    "original MIR target binary scalar types differ",
                ));
            }
            ExpressionV30::Binary {
                operation,
                left: input[0],
                right: input[1],
            }
        }
        Kind::Compare { predicate, .. } if input.len() == 2 => {
            if ty != ScalarV30::Bool || nodes[input[0]].scalar != nodes[input[1]].scalar {
                return Err(Error::Statement(
                    "original MIR target comparison scalar types differ",
                ));
            }
            let operation = match predicate {
                ComparePredicate::Equal => OperatorV30::Equal,
                ComparePredicate::NotEqual => OperatorV30::NotEqual,
                ComparePredicate::LessThan => OperatorV30::Less,
                ComparePredicate::LessThanOrEqual => OperatorV30::LessEqual,
                ComparePredicate::GreaterThan => OperatorV30::Greater,
                ComparePredicate::GreaterThanOrEqual => OperatorV30::GreaterEqual,
            };
            ExpressionV30::Binary {
                operation,
                left: input[0],
                right: input[1],
            }
        }
        _ => {
            return Err(Error::Statement(
                "original MIR target operation is not modeled",
            ));
        }
    })
}
