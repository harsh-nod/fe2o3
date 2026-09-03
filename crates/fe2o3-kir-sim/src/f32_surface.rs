use fe2o3_kernel_ir::{BinaryOp, ComparePredicate, F32MathFunction, UnaryOp};

/// One core scalar operation whose operands and non-boolean result are binary32.
///
/// Casts and cross-width float operations are intentionally a separate surface.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum F32ScalarOperationV1 {
    Unary(UnaryOp),
    Binary(BinaryOp),
    Compare(ComparePredicate),
    Math(F32MathFunction),
}

/// Ordered source of truth for the core scalar binary32 operations admitted by
/// simulator preflight.
///
/// The differential V3 corpus checks this exact ordered roster. Preflight also
/// consults it for every binary32 unary, binary, comparison, and math operation,
/// so extending admission requires extending this roster and its conformance
/// corpus together.
pub const F32_SCALAR_OPERATION_ROSTER_V1: &[F32ScalarOperationV1] = &[
    F32ScalarOperationV1::Unary(UnaryOp::Negate),
    F32ScalarOperationV1::Binary(BinaryOp::Add),
    F32ScalarOperationV1::Binary(BinaryOp::Subtract),
    F32ScalarOperationV1::Binary(BinaryOp::Multiply),
    F32ScalarOperationV1::Binary(BinaryOp::Divide),
    F32ScalarOperationV1::Binary(BinaryOp::Remainder),
    F32ScalarOperationV1::Compare(ComparePredicate::Equal),
    F32ScalarOperationV1::Compare(ComparePredicate::NotEqual),
    F32ScalarOperationV1::Compare(ComparePredicate::LessThan),
    F32ScalarOperationV1::Compare(ComparePredicate::LessThanOrEqual),
    F32ScalarOperationV1::Compare(ComparePredicate::GreaterThan),
    F32ScalarOperationV1::Compare(ComparePredicate::GreaterThanOrEqual),
    F32ScalarOperationV1::Math(F32MathFunction::FusedMultiplyAdd),
    F32ScalarOperationV1::Math(F32MathFunction::Floor),
    F32ScalarOperationV1::Math(F32MathFunction::Ceil),
    F32ScalarOperationV1::Math(F32MathFunction::Truncate),
    F32ScalarOperationV1::Math(F32MathFunction::RoundTiesEven),
    F32ScalarOperationV1::Math(F32MathFunction::Abs),
];

impl F32ScalarOperationV1 {
    pub const fn family(self) -> &'static str {
        match self {
            Self::Unary(_) => "unary",
            Self::Binary(_) => "binary",
            Self::Compare(_) => "compare",
            Self::Math(_) => "f32_math",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Unary(UnaryOp::Negate) => "negate",
            Self::Unary(UnaryOp::Not) => "not",
            Self::Binary(BinaryOp::Add) => "add",
            Self::Binary(BinaryOp::Subtract) => "subtract",
            Self::Binary(BinaryOp::Multiply) => "multiply",
            Self::Binary(BinaryOp::Divide) => "divide",
            Self::Binary(BinaryOp::Remainder) => "remainder",
            Self::Binary(BinaryOp::BitAnd) => "bit_and",
            Self::Binary(BinaryOp::BitOr) => "bit_or",
            Self::Binary(BinaryOp::BitXor) => "bit_xor",
            Self::Binary(BinaryOp::ShiftLeft) => "shift_left",
            Self::Binary(BinaryOp::ShiftRight) => "shift_right",
            Self::Binary(BinaryOp::Checked(operator)) => match operator {
                fe2o3_kernel_ir::CheckedBinaryOperator::Add => "checked_add",
                fe2o3_kernel_ir::CheckedBinaryOperator::Subtract => "checked_subtract",
                fe2o3_kernel_ir::CheckedBinaryOperator::Multiply => "checked_multiply",
            },
            Self::Compare(ComparePredicate::Equal) => "compare_equal",
            Self::Compare(ComparePredicate::NotEqual) => "compare_not_equal",
            Self::Compare(ComparePredicate::LessThan) => "compare_less_than",
            Self::Compare(ComparePredicate::LessThanOrEqual) => "compare_less_than_or_equal",
            Self::Compare(ComparePredicate::GreaterThan) => "compare_greater_than",
            Self::Compare(ComparePredicate::GreaterThanOrEqual) => "compare_greater_than_or_equal",
            Self::Math(F32MathFunction::Sqrt) => "sqrt",
            Self::Math(F32MathFunction::FusedMultiplyAdd) => "fused_multiply_add",
            Self::Math(F32MathFunction::Floor) => "floor",
            Self::Math(F32MathFunction::Ceil) => "ceil",
            Self::Math(F32MathFunction::Truncate) => "truncate",
            Self::Math(F32MathFunction::RoundTiesEven) => "round_ties_even",
            Self::Math(F32MathFunction::Sin) => "sin",
            Self::Math(F32MathFunction::Cos) => "cos",
            Self::Math(F32MathFunction::Exp) => "exp",
            Self::Math(F32MathFunction::Exp2) => "exp2",
            Self::Math(F32MathFunction::Ln) => "ln",
            Self::Math(F32MathFunction::Log2) => "log2",
            Self::Math(F32MathFunction::Log10) => "log10",
        }
    }
}

pub(crate) fn admits_f32_scalar_operation(operation: F32ScalarOperationV1) -> bool {
    F32_SCALAR_OPERATION_ROSTER_V1.contains(&operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admitted_roster_is_unique_and_names_every_operation() {
        assert_eq!(F32_SCALAR_OPERATION_ROSTER_V1.len(), 17);
        for (index, operation) in F32_SCALAR_OPERATION_ROSTER_V1.iter().enumerate() {
            assert!(
                !F32_SCALAR_OPERATION_ROSTER_V1[..index].contains(operation),
                "duplicate operation {}",
                operation.name()
            );
            assert!(!operation.name().is_empty());
            assert!(!operation.family().is_empty());
        }
    }
}
