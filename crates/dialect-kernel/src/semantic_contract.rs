use std::{error::Error, fmt};

use pliron::{
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        op_interfaces::{NRegionsInterface, NResultsInterface},
    },
    combine::{Parser, count_min_max, parser::char::hex_digit},
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_op, pliron_type},
    op::Op,
    operation::Operation,
    parsable::{Parsable, ParseResult, StateStream},
    printable::{self, Printable},
    result::Result as PlironResult,
    r#type::Typed,
    value::Value,
    verify_err, verify_err_noloc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticContractError {
    MalformedOperation,
    ForeignScalarOperand { operand: usize },
}

impl fmt::Display for SemanticContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedOperation => {
                formatter.write_str("semantic-contract operation has a malformed closed payload")
            }
            Self::ForeignScalarOperand { operand } => write!(
                formatter,
                "semantic-contract operand {operand} is not kernel.semantic_scalar"
            ),
        }
    }
}

impl Error for SemanticContractError {}

#[pliron_type(
    name = "kernel.semantic_scalar",
    format,
    generate_get = true,
    verifier = "succ"
)]
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct SemanticScalarType;

#[pliron_attr(name = "kernel.semantic_symbol", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticSymbolAttr(pub u32);

#[pliron_attr(name = "kernel.semantic_constant", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticConstantAttr(pub u64);

/// Opaque commitment to a compiler-retained typed semantic transcript.
///
/// This attribute authenticates no typing or numerical fact by itself. The
/// production ranked owner must retain and validate the committed transcript.
#[pliron_attr(name = "kernel.semantic_expression_commitment")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SemanticExpressionCommitmentAttr([u64; 4]);

impl SemanticExpressionCommitmentAttr {
    pub const fn new(words: [u64; 4]) -> Self {
        Self(words)
    }

    pub const fn words(&self) -> [u64; 4] {
        self.0
    }

    const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
    }
}

impl Verify for SemanticExpressionCommitmentAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.is_zero() {
            return verify_err_noloc!(
                "kernel.semantic_expression_commitment cannot be the reserved all-zero identity"
            );
        }
        Ok(())
    }
}

impl Printable for SemanticExpressionCommitmentAttr {
    fn fmt(
        &self,
        _context: &Context,
        _state: &printable::State,
        formatter: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(
            formatter,
            "{:016x}{:016x}{:016x}{:016x}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl Parsable for SemanticExpressionCommitmentAttr {
    type Arg = ();
    type Parsed = Self;

    fn parse<'a>(
        state_stream: &mut StateStream<'a>,
        _arg: Self::Arg,
    ) -> ParseResult<'a, Self::Parsed> {
        let word = || {
            count_min_max::<String, _, _>(16, 16, hex_digit())
                .and_then(|digits| u64::from_str_radix(&digits, 16))
        };
        word()
            .and(word())
            .and(word())
            .and(word())
            .map(|(((first, second), third), fourth)| Self([first, second, third, fourth]))
            .parse_stream(state_stream)
            .into()
    }
}

#[pliron_attr(name = "kernel.semantic_binary_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SemanticBinaryKindAttr {
    Add,
    Multiply,
}

#[pliron_op(
    name = "kernel.semantic_symbol",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (kernel_semantic_symbol: SemanticSymbolAttr)
)]
pub struct SemanticSymbolOp;

impl SemanticSymbolOp {
    pub fn new(context: &mut Context, symbol: u32) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![SemanticScalarType::get(context).into()],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_semantic_symbol(context, SemanticSymbolAttr(symbol));
        op
    }

    pub fn symbol(&self, context: &Context) -> Option<u32> {
        self.get_attr_kernel_semantic_symbol(context)
            .map(|value| value.0)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl Verify for SemanticSymbolOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed(self, context, 0, 1, 1)?;
        if self.symbol(context).is_none() || !is_semantic_scalar(self.result(context), context) {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_constant",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (kernel_semantic_constant: SemanticConstantAttr)
)]
pub struct SemanticConstantOp;

impl SemanticConstantOp {
    pub fn new(context: &mut Context, value: u64) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![SemanticScalarType::get(context).into()],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_semantic_constant(context, SemanticConstantAttr(value));
        op
    }

    pub fn value(&self, context: &Context) -> Option<u64> {
        self.get_attr_kernel_semantic_constant(context)
            .map(|value| value.0)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl Verify for SemanticConstantOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed(self, context, 0, 1, 1)?;
        if self.value(context).is_none() || !is_semantic_scalar(self.result(context), context) {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_expression_commitment",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (kernel_semantic_expression_commitment: SemanticExpressionCommitmentAttr)
)]
pub struct SemanticExpressionCommitmentOp;

impl SemanticExpressionCommitmentOp {
    pub fn new(context: &mut Context, identity: [u64; 4]) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![SemanticScalarType::get(context).into()],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_semantic_expression_commitment(
            context,
            SemanticExpressionCommitmentAttr::new(identity),
        );
        op
    }

    pub fn identity(&self, context: &Context) -> Option<[u64; 4]> {
        self.get_attr_kernel_semantic_expression_commitment(context)
            .map(|identity| identity.words())
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl Verify for SemanticExpressionCommitmentOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed(self, context, 0, 1, 1)?;
        if self.identity(context).is_none() || !is_semantic_scalar(self.result(context), context) {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        Ok(())
    }
}

#[pliron_op(
    name = "kernel.semantic_binary",
    format,
    interfaces = [NResultsInterface<1>, NRegionsInterface<0>],
    attributes = (kernel_semantic_binary_kind: SemanticBinaryKindAttr)
)]
pub struct SemanticBinaryOp;

impl SemanticBinaryOp {
    pub fn new(
        context: &mut Context,
        kind: SemanticBinaryKindAttr,
        lhs: Value,
        rhs: Value,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![SemanticScalarType::get(context).into()],
            vec![lhs, rhs],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_semantic_binary_kind(context, kind);
        op
    }

    pub fn kind(&self, context: &Context) -> Option<SemanticBinaryKindAttr> {
        self.get_attr_kernel_semantic_binary_kind(context)
            .map(|value| *value)
    }

    pub fn lhs(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }

    pub fn rhs(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(1)
    }

    pub fn result(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_result(0)
    }
}

impl Verify for SemanticBinaryOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed(self, context, 2, 1, 1)?;
        if self.kind(context).is_none() || !is_semantic_scalar(self.result(context), context) {
            return verify_err!(self.loc(context), SemanticContractError::MalformedOperation);
        }
        require_scalar(self, context, 0)?;
        require_scalar(self, context, 1)
    }
}

#[pliron_op(
    name = "kernel.require_equivalent",
    format,
    interfaces = [NResultsInterface<0>, NRegionsInterface<0>]
)]
pub struct RequireEquivalentOp;

impl RequireEquivalentOp {
    pub fn new(context: &mut Context, actual: Value, expected: Value) -> Self {
        Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![actual, expected],
            vec![],
            0,
        ))
    }

    pub fn actual(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(0)
    }

    pub fn expected(&self, context: &Context) -> Value {
        self.get_operation().deref(context).get_operand(1)
    }
}

impl Verify for RequireEquivalentOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        verify_closed(self, context, 2, 0, 0)?;
        require_scalar(self, context, 0)?;
        require_scalar(self, context, 1)
    }
}

fn is_semantic_scalar(value: Value, context: &Context) -> bool {
    value
        .get_type(context)
        .deref(context)
        .is::<SemanticScalarType>()
}

fn require_scalar(operation: &dyn Op, context: &Context, operand: usize) -> PlironResult<()> {
    if !is_semantic_scalar(
        operation
            .get_operation()
            .deref(context)
            .get_operand(operand),
        context,
    ) {
        return verify_err!(
            operation.loc(context),
            SemanticContractError::ForeignScalarOperand { operand }
        );
    }
    Ok(())
}

fn verify_closed(
    operation: &dyn Op,
    context: &Context,
    operands: usize,
    results: usize,
    attributes: usize,
) -> PlironResult<()> {
    let raw = operation.get_operation().deref(context);
    let closed_attributes = raw.attributes.0.keys().all(|key| {
        key == &*ATTR_KEY_DEBUG_INFO
            || matches!(
                key.as_ref(),
                "kernel_semantic_symbol"
                    | "kernel_semantic_constant"
                    | "kernel_semantic_expression_commitment"
                    | "kernel_semantic_binary_kind"
            )
    });
    if raw.get_num_operands() != operands
        || raw.get_num_results() != results
        || raw.get_num_successors() != 0
        || raw.num_regions() != 0
        || raw
            .attributes
            .0
            .keys()
            .filter(|key| *key != &*ATTR_KEY_DEBUG_INFO)
            .count()
            != attributes
        || !closed_attributes
    {
        return verify_err!(
            operation.loc(context),
            SemanticContractError::MalformedOperation
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliron::{context::Context, dialect::DialectName};

    #[test]
    fn semantic_expression_commitment_is_closed_and_non_authoritative() {
        let mut context = Context::new();
        crate::register_dialect(
            &mut context,
            &DialectName::try_new(crate::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let operation = SemanticExpressionCommitmentOp::new(&mut context, [1, 2, 3, 4]);
        operation.verify(&context).unwrap();
        assert_eq!(operation.identity(&context), Some([1, 2, 3, 4]));
    }
}
