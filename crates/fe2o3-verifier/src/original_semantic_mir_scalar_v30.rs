//! Independent local-state semantics for the first original-MIR proof scope.
//! No source-to-target association or execution authority is constructed here.

use super::{Error, Resource, Result, Writer};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1 as Binary, SemanticConstantValueV1 as Constant,
    SemanticFunctionDeclV1 as Function, SemanticLocalRoleV1 as LocalRole,
    SemanticOperandV1 as Operand, SemanticPlaceV1 as Place, SemanticRvalueKindV1 as Rvalue,
    SemanticScalarTypeV1 as Scalar, SemanticStatementKindV1 as Statement,
    SemanticTerminatorKindV1 as Terminator, SemanticTypeDeclV1 as Type, SemanticTypeIdV1 as TypeId,
    SemanticTypeShapeV1 as Shape, SemanticUnaryOpV1 as Unary,
};
use std::{fmt::Write as _, mem::size_of};

#[path = "original_semantic_mir_canonical_v30.rs"]
mod canonical;

#[path = "original_semantic_mir_canonical_byte_v36.rs"]
mod canonical_byte;
pub(crate) use canonical_byte::{CanonicalByteScalarBodyV55, CanonicalByteScalarV30};

#[path = "original_semantic_mir_relation_v30.rs"]
mod relation;

#[path = "original_semantic_mir_boundary_v31.rs"]
mod boundary;

#[path = "original_semantic_mir_control_v31.rs"]
mod control;

#[path = "original_semantic_mir_control_pair_v31.rs"]
mod control_pair;

#[path = "original_semantic_mir_control_source_v31.rs"]
mod control_source;

#[path = "original_semantic_mir_invocations_v32.rs"]
mod invocations;

#[path = "original_semantic_mir_call_transfers_v33.rs"]
mod call_transfers;

#[path = "original_semantic_mir_invocation_body_v34.rs"]
mod invocation_body;

#[path = "original_semantic_mir_target_trace_v35.rs"]
mod target_trace;

#[path = "original_semantic_mir_control_generate_v31.rs"]
mod control_generate;
#[cfg(test)]
pub(crate) use control::tests::fixture as control_fixture_v31;
pub(crate) use control_generate::generate as generate_control_v31;
pub(crate) use invocation_body::generate_refinement_typed_v49 as generate_invocations_typed_v49;
pub(crate) use invocation_body::generate_refinement_typed_with_references_v69 as generate_invocations_typed_with_references_v69;
pub(crate) use invocation_body::generate_refinement_v36 as generate_invocations_v36;

#[path = "original_semantic_mir_source_v30.rs"]
mod source;
pub(crate) use source::generate;

#[cfg(test)]
#[path = "original_semantic_mir_scalar_v30_tests.rs"]
pub(crate) mod tests;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => {
        write!($out, $($arg)*).map_err(|_| $out.error())?
    };
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ScalarV30 {
    Unit,
    Bool,
    Integer { width: u32, signed: bool },
    Float { width: u32 },
}

impl ScalarV30 {
    fn from_source(types: &[Type], ty: TypeId) -> Result<Self> {
        match types.get(ty.index() as usize).map(Type::shape) {
            Some(Shape::Unit) => Ok(Self::Unit),
            Some(Shape::Scalar(Scalar::Bool)) => Ok(Self::Bool),
            Some(Shape::Scalar(Scalar::Float {
                bits: bits @ (32 | 64),
            })) => Ok(Self::Float {
                width: u32::from(*bits),
            }),
            Some(Shape::Scalar(Scalar::Integer { bits, signed }))
                if matches!(bits, 8 | 16 | 32 | 64) =>
            {
                Ok(Self::Integer {
                    width: u32::from(*bits),
                    signed: *signed,
                })
            }
            _ => Err(Error::Statement(
                "original MIR scalar proof type is not modeled",
            )),
        }
    }

    fn width(self) -> u32 {
        match self {
            Self::Unit => 0,
            Self::Bool => 1,
            Self::Integer { width, .. } | Self::Float { width } => width,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum OperatorV30 {
    WrappingAdd,
    WrappingSubtract,
    WrappingMultiply,
    And,
    Or,
    Xor,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl OperatorV30 {
    fn from_source(op: Binary) -> Result<Self> {
        Ok(match op {
            Binary::BitAnd => Self::And,
            Binary::BitOr => Self::Or,
            Binary::BitXor => Self::Xor,
            Binary::Equal => Self::Equal,
            Binary::NotEqual => Self::NotEqual,
            Binary::LessThan => Self::Less,
            Binary::LessOrEqual => Self::LessEqual,
            Binary::GreaterThan => Self::Greater,
            Binary::GreaterOrEqual => Self::GreaterEqual,
            _ => {
                return Err(Error::Statement(
                    "original MIR arithmetic/effect contract is not modeled",
                ));
            }
        })
    }

    fn comparison(self) -> bool {
        !matches!(
            self,
            Self::WrappingAdd
                | Self::WrappingSubtract
                | Self::WrappingMultiply
                | Self::And
                | Self::Or
                | Self::Xor
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ExpressionV30 {
    Argument(u32),
    Constant(u128),
    Not(usize),
    Select {
        condition: usize,
        true_value: usize,
        false_value: usize,
    },
    Binary {
        operation: OperatorV30,
        left: usize,
        right: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct NodeV30 {
    pub scalar: ScalarV30,
    pub expression: ExpressionV30,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AssignmentV30 {
    pub statement: u32,
    pub destination: u32,
    pub value: usize,
}

pub(super) struct SourceProgramV30 {
    pub nodes: Vec<NodeV30>,
    pub assignments: Vec<AssignmentV30>,
    pub arguments: usize,
    pub returned: Option<usize>,
    pub statements: usize,
    locals: Vec<Option<usize>>,
}

fn vector<T>(count: usize, out: &mut Writer<'_, '_>) -> Result<Vec<T>> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    out.budget.reserve_storage(bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    if values.capacity() > count {
        out.budget.reserve_storage(
            (values.capacity() - count)
                .checked_mul(size_of::<T>())
                .ok_or(Resource::Arithmetic)?,
        )?;
    }
    Ok(values)
}

fn emit_graph_v30<I: Iterator<Item = Result<Option<usize>>>>(
    nodes: &[NodeV30],
    arguments: usize,
    root: usize,
    label: &'static str,
    observations: I,
    returned: Option<usize>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(
        size_of::<I>()
            + size_of::<Option<Result<Option<usize>>>>()
            + size_of::<(&[NodeV30], &mut Writer<'_, '_>, &str, &NodeV30)>()
            + size_of::<(usize, usize, usize, Option<usize>, ScalarV30, &str)>()
            + size_of::<Result<()>>(),
    )?;
    emit!(
        out,
        "spec fn original_{label}_trace_{root}_v30(base: Seq<int>) -> Seq<int>\n recommends base.len() == {arguments},\n{{\n"
    );
    for (index, node) in nodes.iter().enumerate() {
        out.budget.charge_work(2)?;
        emit!(out, " let m{index}: int = ");
        match node.expression {
            ExpressionV30::Argument(argument) => emit!(out, "base[{argument}]"),
            ExpressionV30::Constant(bits) => emit!(out, "{bits}int"),
            ExpressionV30::Select {
                condition,
                true_value,
                false_value,
            } => {
                out.budget.charge_work(4)?;
                if condition >= index
                    || true_value >= index
                    || false_value >= index
                    || nodes[condition].scalar != ScalarV30::Bool
                    || nodes[true_value].scalar != node.scalar
                    || nodes[false_value].scalar != node.scalar
                    || node.scalar == ScalarV30::Unit
                {
                    return Err(Error::Statement(
                        "original MIR select types or dependency order differ",
                    ));
                }
                emit!(
                    out,
                    "if m{condition} == 1int {{ m{true_value} }} else {{ m{false_value} }}"
                );
            }
            ExpressionV30::Not(input) => {
                if node.scalar == ScalarV30::Bool {
                    emit!(out, "if m{input} == 1int {{ 0int }} else {{ 1int }}");
                } else {
                    let width = node.scalar.width();
                    emit!(out, "(!(m{input} as u{width})) as int");
                }
            }
            ExpressionV30::Binary {
                operation,
                left,
                right,
            } => {
                let operand = nodes
                    .get(left)
                    .ok_or(Error::Statement("original MIR expression left edge"))?
                    .scalar;
                if right >= index || left >= index || nodes[right].scalar != operand {
                    return Err(Error::Statement(
                        "original MIR expression dependency order differs",
                    ));
                }
                match operation {
                    OperatorV30::WrappingAdd
                    | OperatorV30::WrappingSubtract
                    | OperatorV30::WrappingMultiply => {
                        let ScalarV30::Integer { width, .. } = operand else {
                            return Err(Error::Statement(
                                "original MIR wrapping expression requires integer operands",
                            ));
                        };
                        if !matches!(width, 8 | 16 | 32 | 64) || node.scalar != operand {
                            return Err(Error::Statement(
                                "original MIR wrapping expression width or result differs",
                            ));
                        }
                        let symbol = match operation {
                            OperatorV30::WrappingAdd => "+",
                            OperatorV30::WrappingSubtract => "-",
                            OperatorV30::WrappingMultiply => "*",
                            _ => unreachable!(),
                        };
                        // Raw two's-complement carriers use the same positive
                        // modulus for signed and unsigned wrapping operations.
                        let modulus = 1u128 << width;
                        emit!(out, "(m{left} {symbol} m{right}) % {modulus}int");
                    }
                    OperatorV30::And | OperatorV30::Or | OperatorV30::Xor => {
                        let width = if operand == ScalarV30::Bool {
                            8
                        } else {
                            operand.width()
                        };
                        let symbol = match operation {
                            OperatorV30::And => "&",
                            OperatorV30::Or => "|",
                            _ => "^",
                        };
                        emit!(
                            out,
                            "((m{left} as u{width}) {symbol} (m{right} as u{width})) as int"
                        );
                    }
                    _ => {
                        let symbol = match operation {
                            OperatorV30::Equal => "==",
                            OperatorV30::NotEqual => "!=",
                            OperatorV30::Less => "<",
                            OperatorV30::LessEqual => "<=",
                            OperatorV30::Greater => ">",
                            OperatorV30::GreaterEqual => ">=",
                            _ => unreachable!(),
                        };
                        if let ScalarV30::Integer {
                            width,
                            signed: true,
                        } = operand
                        {
                            let modulus = 1u128 << width;
                            emit!(
                                out,
                                "if source_signed_v30(m{left}, {modulus}) {symbol} source_signed_v30(m{right}, {modulus}) {{ 1int }} else {{ 0int }}"
                            );
                        } else {
                            emit!(
                                out,
                                "if m{left} {symbol} m{right} {{ 1int }} else {{ 0int }}"
                            );
                        }
                    }
                }
            }
        }
        emit!(out, ";\n");
    }
    emit!(out, " seq![");
    for observation in observations {
        out.budget.charge_work(1)?;
        match observation? {
            Some(value) if value < nodes.len() => emit!(out, "m{value},"),
            Some(_) => return Err(Error::Statement("original MIR trace endpoint is absent")),
            None => emit!(out, "0int,"),
        }
    }
    if let Some(value) = returned {
        if value >= nodes.len() {
            return Err(Error::Statement("original MIR trace return is absent"));
        }
        emit!(out, "m{value},");
    }
    emit!(out, "]\n}}\n");
    Ok(())
}

impl SourceProgramV30 {
    /// The trace contains every assignment result, even when the destination
    /// is later overwritten or unused, followed by the actual return value.
    pub(super) fn emit_trace(&self, root: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        emit_graph_v30(
            &self.nodes,
            self.arguments,
            root,
            "mir",
            self.assignments
                .iter()
                .map(|assignment| Ok(Some(assignment.value))),
            self.returned,
            out,
        )
    }

    pub(super) fn derive(
        types: &[Type],
        function: &Function,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(
            function
                .locals()
                .len()
                .checked_add(4)
                .ok_or(Resource::Arithmetic)?,
        )?;
        if function.blocks().len() != 1
            || function.entry().index() != 0
            || !matches!(function.blocks()[0].terminator().kind(), Terminator::Return)
        {
            return Err(Error::Statement(
                "original MIR source control is not modeled",
            ));
        }
        let statements = function.blocks()[0].statements();
        let arguments = function.abi().source_input_types().len();
        let capacity = statements
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(arguments))
            .ok_or(Resource::Arithmetic)?;
        out.budget.reserve_storage(
            size_of::<Self>() + size_of::<Result<Self>>() + interpreter_headers_v31(),
        )?;
        let mut program = Self {
            nodes: vector(capacity, out)?,
            assignments: vector(statements.len(), out)?,
            arguments,
            returned: None,
            statements: statements.len(),
            locals: vector(function.locals().len(), out)?,
        };
        program.locals.resize(function.locals().len(), None);
        out.budget.reserve_storage(size_of::<Vec<bool>>())?;
        let mut arguments_seen = vector(arguments, out)?;
        arguments_seen.resize(arguments, false);
        out.budget.charge_work(arguments)?;
        for (local, declaration) in function.locals().iter().enumerate() {
            if let LocalRole::Argument(argument) = declaration.role() {
                out.budget.charge_work(4)?;
                if function.abi().source_input_types().get(argument as usize)
                    != Some(&declaration.ty())
                    || arguments_seen.get(argument as usize) != Some(&false)
                {
                    return Err(Error::Statement(
                        "original MIR scalar argument roster differs",
                    ));
                }
                arguments_seen[argument as usize] = true;
                let scalar = ScalarV30::from_source(types, declaration.ty())?;
                if scalar == ScalarV30::Unit {
                    return Err(Error::Statement(
                        "original MIR zero-sized argument ABI is not modeled",
                    ));
                }
                let node = program.push(
                    NodeV30 {
                        scalar,
                        expression: ExpressionV30::Argument(argument),
                    },
                    out,
                )?;
                program.locals[local] = Some(node);
            }
        }
        if program.nodes.len() != arguments {
            return Err(Error::Statement("original MIR scalar argument is missing"));
        }
        for (ordinal, statement) in statements.iter().enumerate() {
            program.statement(types, function, ordinal, statement.kind(), out)?;
        }
        program.return_value(types, function, out)?;
        Ok(program)
    }

    fn statement(
        &mut self,
        types: &[Type],
        function: &Function,
        ordinal: usize,
        statement: &Statement,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(3)?;
        match statement {
            Statement::Assign(assignment) => {
                let destination = assignment.destination();
                self.local(function, destination)?;
                let scalar = ScalarV30::from_source(types, destination.ty())?;
                if assignment.value().result_type() != destination.ty() {
                    return Err(Error::Statement(
                        "original MIR assignment result type differs",
                    ));
                }
                let value = self.rvalue(types, function, assignment.value().kind(), scalar, out)?;
                self.locals[destination.local().index() as usize] = Some(value);
                if self.assignments.len() == self.assignments.capacity() {
                    return Err(Resource::Accounting.into());
                }
                self.assignments.push(AssignmentV30 {
                    statement: u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?,
                    destination: destination.local().index(),
                    value,
                });
            }
            Statement::StorageLive(local) | Statement::StorageDead(local) => {
                *self
                    .locals
                    .get_mut(local.index() as usize)
                    .ok_or(Error::Statement("original MIR storage local is absent"))? = None;
            }
            Statement::Deinitialize(place) => {
                let local = self.local(function, place)?;
                self.locals[local] = None;
            }
            Statement::Nop => {}
            _ => {
                return Err(Error::Statement(
                    "original MIR source statement is not modeled",
                ));
            }
        }
        Ok(())
    }

    fn return_value(
        &mut self,
        types: &[Type],
        function: &Function,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let return_type = function.abi().return_type();
        if !matches!(
            types.get(return_type.index() as usize).map(Type::shape),
            Some(Shape::Unit)
        ) {
            let scalar = ScalarV30::from_source(types, return_type)?;
            let mut returned = None;
            for (local, declaration) in function.locals().iter().enumerate() {
                out.budget.charge_work(1)?;
                if declaration.role() == LocalRole::Return {
                    if returned.is_some() || declaration.ty() != return_type {
                        return Err(Error::Statement("original MIR return local differs"));
                    }
                    returned = Some(
                        self.locals[local]
                            .ok_or(Error::Statement("original MIR return is undefined"))?,
                    );
                }
            }
            let returned =
                returned.ok_or(Error::Statement("original MIR return local is missing"))?;
            if self.nodes[returned].scalar != scalar {
                return Err(Error::Statement("original MIR return scalar type differs"));
            }
            self.returned = Some(returned);
        }
        Ok(())
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

    fn local(&self, function: &Function, place: &Place) -> Result<usize> {
        let local = place.local().index() as usize;
        if !place.projections().is_empty()
            || function
                .locals()
                .get(local)
                .is_none_or(|declaration| declaration.ty() != place.ty())
        {
            return Err(Error::Statement(
                "original MIR projected place is not modeled",
            ));
        }
        Ok(local)
    }

    fn operand(
        &mut self,
        types: &[Type],
        function: &Function,
        operand: &Operand,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        out.budget.charge_work(3)?;
        let scalar = ScalarV30::from_source(types, operand.ty())?;
        let value = match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                let local = self.local(function, place)?;
                let value = self.locals[local]
                    .ok_or(Error::Statement("original MIR scalar use is undefined"))?;
                if matches!(operand, Operand::Move(_)) {
                    self.locals[local] = None;
                }
                value
            }
            Operand::Constant(constant) => {
                let bits = match constant.value() {
                    Constant::ZeroSized if scalar == ScalarV30::Unit => 0,
                    Constant::Scalar(value) if scalar != ScalarV30::Unit => value.bits(),
                    _ => {
                        return Err(Error::Statement(
                            "original MIR nonscalar constant is not modeled",
                        ));
                    }
                };
                if bits >= (1u128 << scalar.width()) {
                    return Err(Error::Statement("original MIR constant bit width differs"));
                }
                self.push(
                    NodeV30 {
                        scalar,
                        expression: ExpressionV30::Constant(bits),
                    },
                    out,
                )?
            }
        };
        if self.nodes[value].scalar != scalar {
            return Err(Error::Statement("original MIR scalar operand type differs"));
        }
        Ok(value)
    }

    fn rvalue(
        &mut self,
        types: &[Type],
        function: &Function,
        value: &Rvalue,
        scalar: ScalarV30,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        let value = match value {
            Rvalue::Use(operand) => self.operand(types, function, operand, out)?,
            Rvalue::Unary {
                operation: Unary::Not,
                operand,
            } => {
                let input = self.operand(types, function, operand, out)?;
                if matches!(scalar, ScalarV30::Unit | ScalarV30::Float { .. })
                    || self.nodes[input].scalar != scalar
                {
                    return Err(Error::Statement("original MIR unary scalar type differs"));
                }
                self.push(
                    NodeV30 {
                        scalar,
                        expression: ExpressionV30::Not(input),
                    },
                    out,
                )?
            }
            Rvalue::Binary {
                operation,
                left,
                right,
            } => {
                let operation = OperatorV30::from_source(*operation)?;
                let left = self.operand(types, function, left, out)?;
                let right = self.operand(types, function, right, out)?;
                let input = self.nodes[left].scalar;
                if matches!(input, ScalarV30::Unit | ScalarV30::Float { .. })
                    || self.nodes[right].scalar != input
                    || scalar
                        != if operation.comparison() {
                            ScalarV30::Bool
                        } else {
                            input
                        }
                {
                    return Err(Error::Statement("original MIR binary scalar types differ"));
                }
                self.push(
                    NodeV30 {
                        scalar,
                        expression: ExpressionV30::Binary {
                            operation,
                            left,
                            right,
                        },
                    },
                    out,
                )?
            }
            _ => {
                return Err(Error::Statement(
                    "original MIR scalar rvalue is not modeled",
                ));
            }
        };
        if self.nodes[value].scalar != scalar {
            return Err(Error::Statement(
                "original MIR assignment scalar type differs",
            ));
        }
        Ok(value)
    }
}

// Reusable statement/return frames, prepaid once for the complete interpreter.
// They hold no heap graph and are shared by the historical scalar and CFG paths.
fn interpreter_headers_v31() -> usize {
    size_of::<(
        &mut SourceProgramV30,
        &[Type],
        &Function,
        usize,
        &Statement,
        &mut Writer<'_, '_>,
        &Place,
        ScalarV30,
        usize,
        AssignmentV30,
        Result<()>,
    )>() + size_of::<(
        &mut SourceProgramV30,
        &[Type],
        &Function,
        &mut Writer<'_, '_>,
        usize,
        Option<usize>,
        Result<()>,
    )>()
}
