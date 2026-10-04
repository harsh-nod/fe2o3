// A bounded expression recipe for the overflow result of checked arithmetic.
// The source and canonical adapters independently authenticate their operands.
// This recipe does not assume that an overflow assertion succeeds.
enum CheckedArithmeticNodeV41<T> {
    Operand(usize),
    Constant {
        scalar: ProductionSemanticScalarTypeV2,
        bits: u64,
    },
    Binary {
        operation: ProductionSemanticBinaryOpV2,
        scalar: ProductionSemanticScalarTypeV2,
        lhs: T,
        rhs: T,
    },
    Compare {
        operation: ProductionSemanticComparisonV2,
        scalar: ProductionSemanticScalarTypeV2,
        lhs: T,
        rhs: T,
    },
    Select {
        condition: T,
        when_true: T,
        when_false: T,
    },
}

#[derive(Clone, Copy)]
enum CheckedArithmeticInstructionV41 {
    Left,
    Right,
    Zero,
    Minimum,
    MinusOne,
    False,
    True,
    Arithmetic,
    Divide,
    Less,
    Equal,
    NotEqual,
    BoolEqual,
    BoolNotEqual,
    Select,
}

const CHECKED_ARITHMETIC_STACK_V41: usize = 8;
const CHECKED_ARITHMETIC_DEPTH_V41: usize = 8;

fn checked_arithmetic_scalar_v41(scalar: ProductionSemanticScalarTypeV2) -> Option<(bool, u16)> {
    match scalar {
        ProductionSemanticScalarTypeV2::Integer {
            signed,
            bits: bits @ (8 | 16 | 32 | 64),
        } => Some((signed, bits)),
        _ => None,
    }
}

fn checked_arithmetic_operator_v41(
    operation: CheckedBinaryOperator,
) -> ProductionSemanticBinaryOpV2 {
    match operation {
        CheckedBinaryOperator::Add => ProductionSemanticBinaryOpV2::Add,
        CheckedBinaryOperator::Subtract => ProductionSemanticBinaryOpV2::Subtract,
        CheckedBinaryOperator::Multiply => ProductionSemanticBinaryOpV2::Multiply,
    }
}

fn checked_arithmetic_recipe_v41(
    operation: CheckedBinaryOperator,
    signed: bool,
) -> &'static [CheckedArithmeticInstructionV41] {
    use CheckedArithmeticInstructionV41::*;
    match (operation, signed) {
        (CheckedBinaryOperator::Add, false) => &[Left, Right, Arithmetic, Left, Less],
        (CheckedBinaryOperator::Subtract, false) => &[Left, Right, Less],
        (CheckedBinaryOperator::Multiply, false) => &[
            Right, Zero, Equal, False, Left, Right, Arithmetic, Right, Divide, Left, NotEqual,
            Select,
        ],
        (CheckedBinaryOperator::Add, true) => &[
            Left,
            Zero,
            Less,
            Right,
            Zero,
            Less,
            BoolEqual,
            Left,
            Right,
            Arithmetic,
            Zero,
            Less,
            Left,
            Zero,
            Less,
            BoolNotEqual,
            False,
            Select,
        ],
        (CheckedBinaryOperator::Subtract, true) => &[
            Left,
            Zero,
            Less,
            Right,
            Zero,
            Less,
            BoolNotEqual,
            Left,
            Right,
            Arithmetic,
            Zero,
            Less,
            Left,
            Zero,
            Less,
            BoolNotEqual,
            False,
            Select,
        ],
        (CheckedBinaryOperator::Multiply, true) => &[
            Right, Zero, Equal, False, Left, Minimum, Equal, Right, MinusOne, Equal, False, Select,
            True, Left, Right, Arithmetic, Right, Divide, Left, NotEqual, Select, Select,
        ],
    }
}

fn checked_arithmetic_headers_v41<T>() -> usize {
    std::mem::size_of::<[Option<T>; CHECKED_ARITHMETIC_STACK_V41]>()
        + std::mem::size_of::<CheckedArithmeticNodeV41<T>>()
        + 3 * std::mem::size_of::<T>()
        + std::mem::size_of::<Option<T>>()
        + std::mem::size_of::<&[CheckedArithmeticInstructionV41]>()
        + std::mem::size_of::<std::slice::Iter<'_, CheckedArithmeticInstructionV41>>()
        + std::mem::size_of::<CheckedArithmeticInstructionV41>()
        + std::mem::size_of::<(
            CheckedBinaryOperator,
            ProductionSemanticScalarTypeV2,
            usize,
            bool,
            u16,
            u64,
        )>()
}

fn checked_overflow_expression_v41<T, E>(
    operation: CheckedBinaryOperator,
    scalar: ProductionSemanticScalarTypeV2,
    emit: &mut impl FnMut(CheckedArithmeticNodeV41<T>) -> Result<T, E>,
    invalid: impl Fn() -> E,
) -> Result<T, E> {
    use CheckedArithmeticInstructionV41 as I;
    use CheckedArithmeticNodeV41 as N;
    let (signed, bits) = checked_arithmetic_scalar_v41(scalar).ok_or_else(&invalid)?;
    let mut stack: [Option<T>; CHECKED_ARITHMETIC_STACK_V41] = std::array::from_fn(|_| None);
    let mut count = 0usize;
    for instruction in checked_arithmetic_recipe_v41(operation, signed) {
        let node = match instruction {
            I::Left => N::Operand(0),
            I::Right => N::Operand(1),
            I::Zero | I::Minimum | I::MinusOne | I::False | I::True => N::Constant {
                scalar: if matches!(instruction, I::False | I::True) {
                    ProductionSemanticScalarTypeV2::Bool
                } else {
                    scalar
                },
                bits: match instruction {
                    I::Minimum => 1u64 << (bits - 1),
                    I::MinusOne => u64::MAX >> (64 - bits),
                    I::True => 1,
                    _ => 0,
                },
            },
            I::Select => {
                if count < 3 {
                    return Err(invalid());
                }
                count -= 3;
                N::Select {
                    condition: stack[count].take().ok_or_else(&invalid)?,
                    when_true: stack[count + 1].take().ok_or_else(&invalid)?,
                    when_false: stack[count + 2].take().ok_or_else(&invalid)?,
                }
            }
            _ => {
                if count < 2 {
                    return Err(invalid());
                }
                count -= 2;
                let lhs = stack[count].take().ok_or_else(&invalid)?;
                let rhs = stack[count + 1].take().ok_or_else(&invalid)?;
                match instruction {
                    I::Arithmetic | I::Divide => N::Binary {
                        operation: if matches!(instruction, I::Divide) {
                            ProductionSemanticBinaryOpV2::Divide
                        } else {
                            checked_arithmetic_operator_v41(operation)
                        },
                        scalar,
                        lhs,
                        rhs,
                    },
                    _ => N::Compare {
                        operation: match instruction {
                            I::Less => ProductionSemanticComparisonV2::LessThan,
                            I::Equal | I::BoolEqual => ProductionSemanticComparisonV2::Equal,
                            I::NotEqual | I::BoolNotEqual => {
                                ProductionSemanticComparisonV2::NotEqual
                            }
                            _ => return Err(invalid()),
                        },
                        scalar: if matches!(instruction, I::BoolEqual | I::BoolNotEqual) {
                            ProductionSemanticScalarTypeV2::Bool
                        } else {
                            scalar
                        },
                        lhs,
                        rhs,
                    },
                }
            }
        };
        let result = emit(node)?;
        *stack.get_mut(count).ok_or_else(&invalid)? = Some(result);
        count += 1;
    }
    if count != 1 {
        return Err(invalid());
    }
    stack[0].take().ok_or_else(invalid)
}

fn normalize_checked_overflow_v41(
    operation: CheckedBinaryOperator,
    scalar: ProductionSemanticScalarTypeV2,
    budget: &mut dyn CorrelationChargeV18,
    mut operand: impl FnMut(
        Option<usize>,
        &mut dyn CorrelationChargeV18,
    ) -> Option<Option<NormalizedScalarExpressionV1>>,
) -> Option<NormalizedScalarExpressionV1> {
    let headers = checked_arithmetic_headers_v41::<NormalizedScalarExpressionV1>()
        .checked_add(std::mem::size_of_val(&operand))?
        .checked_add(std::mem::align_of_val(&operand))?;
    budget.reserve_private_array_scratch(headers)?;
    let result = checked_overflow_expression_v41(
        operation,
        scalar,
        &mut |node| {
            budget.charge().ok_or(())?;
            budget.normalized_node().ok_or(())?;
            if operand(None, budget).ok_or(())?.is_some() {
                return Err(());
            }
            Ok(match node {
                CheckedArithmeticNodeV41::Operand(index) => {
                    operand(Some(index), budget).ok_or(())?.ok_or(())?
                }
                CheckedArithmeticNodeV41::Constant { scalar, bits } => {
                    NormalizedScalarExpressionV1::Constant { scalar, bits }
                }
                CheckedArithmeticNodeV41::Binary {
                    operation,
                    scalar,
                    lhs,
                    rhs,
                } => NormalizedScalarExpressionV1::Binary {
                    operation,
                    scalar,
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs: normalized_scalar_box_v18(lhs, budget).ok_or(())?,
                    rhs: normalized_scalar_box_v18(rhs, budget).ok_or(())?,
                },
                CheckedArithmeticNodeV41::Compare {
                    operation,
                    scalar,
                    lhs,
                    rhs,
                } => NormalizedScalarExpressionV1::Compare {
                    operation,
                    operand_scalar: scalar,
                    lhs: normalized_scalar_box_v18(lhs, budget).ok_or(())?,
                    rhs: normalized_scalar_box_v18(rhs, budget).ok_or(())?,
                },
                CheckedArithmeticNodeV41::Select {
                    condition,
                    when_true,
                    when_false,
                } => NormalizedScalarExpressionV1::Select {
                    scalar: ProductionSemanticScalarTypeV2::Bool,
                    condition: normalized_scalar_box_v18(condition, budget).ok_or(())?,
                    when_true: normalized_scalar_box_v18(when_true, budget).ok_or(())?,
                    when_false: normalized_scalar_box_v18(when_false, budget).ok_or(())?,
                },
            })
        },
        || (),
    );
    drop(operand);
    budget.release_private_array_scratch(headers)?;
    result.ok()
}
