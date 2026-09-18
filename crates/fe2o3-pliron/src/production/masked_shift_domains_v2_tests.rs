use super::*;

type Expr = ProductionSemanticExpressionV2;
type Scalar = ProductionSemanticScalarTypeV2;
type Op = ProductionSemanticBinaryOpV2;
type Overflow = ProductionOverflowContractV2;
type Cast = ProductionSemanticCastV2;
type Error = ProductionSemanticExpressionErrorV2;

fn integer(signed: bool, bits: u16) -> Scalar {
    Scalar::Integer { signed, bits }
}

fn symbol(scalar: Scalar) -> Expr {
    Expr::Symbol { symbol: 7, scalar }
}

fn constant(scalar: Scalar, bits: u64) -> Expr {
    Expr::Constant { scalar, bits }
}

fn binary(operation: Op, scalar: Scalar, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        operation,
        scalar,
        overflow: Overflow::Wrapping,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn masked(shifted_width: u16, count: Scalar) -> Expr {
    binary(
        Op::BitAnd,
        count,
        symbol(count),
        constant(count, u64::from(shifted_width) - 1),
    )
}

fn shift(operation: Op, scalar: Scalar, count: Expr) -> Expr {
    binary(operation, scalar, symbol(scalar), count)
}

fn refused(expression: Expr) {
    expression.validate().unwrap();
    assert_eq!(
        expression.validate_static_domains(),
        Err(Error::IncompleteDomain)
    );
}

#[test]
fn exact_positional_masks_define_both_shifts_for_every_integer_and_count_type() {
    for signed in [false, true] {
        for width in [8, 16, 32, 64] {
            let scalar = integer(signed, width);
            for count_signed in [false, true] {
                for count_width in [8, 16, 32, 64] {
                    let count = integer(count_signed, count_width);
                    for operation in [Op::ShiftLeft, Op::ShiftRight] {
                        let expression = shift(operation, scalar, masked(width, count));
                        expression.validate().unwrap();
                        expression.validate_static_domains().unwrap();
                    }
                }
            }
        }
    }
}

#[test]
fn integer_cast_inside_the_actual_mask_keeps_the_mask_domain() {
    for width in [8, 16, 32, 64] {
        for signed in [false, true] {
            let scalar = integer(signed, width);
            let count = integer(false, 32);
            let operand = Expr::Cast {
                kind: Cast::Integer,
                source: scalar,
                target: count,
                operand: Box::new(symbol(scalar)),
            };
            let rhs = binary(
                Op::BitAnd,
                count,
                operand,
                constant(count, u64::from(width) - 1),
            );
            let expression = shift(Op::ShiftRight, scalar, rhs);
            expression.validate().unwrap();
            expression.validate_static_domains().unwrap();
        }
    }
}

#[test]
fn raw_wrong_mask_operator_position_and_outer_cast_do_not_define_a_shift() {
    let scalar = integer(false, 64);
    let count = integer(false, 32);
    refused(shift(Op::ShiftLeft, scalar, symbol(count)));
    for mask in [0, 31, 62, 64, 127, u32::MAX.into()] {
        refused(shift(
            Op::ShiftLeft,
            scalar,
            binary(Op::BitAnd, count, symbol(count), constant(count, mask)),
        ));
    }
    for operation in [Op::BitOr, Op::BitXor, Op::Add] {
        refused(shift(
            Op::ShiftRight,
            scalar,
            binary(operation, count, symbol(count), constant(count, 63)),
        ));
    }
    refused(shift(
        Op::ShiftLeft,
        scalar,
        binary(Op::BitAnd, count, constant(count, 63), symbol(count)),
    ));
    refused(shift(
        Op::ShiftLeft,
        scalar,
        Expr::Cast {
            kind: Cast::Integer,
            source: count,
            target: scalar,
            operand: Box::new(masked(64, count)),
        },
    ));
    let mismatched = shift(
        Op::ShiftLeft,
        scalar,
        binary(Op::BitAnd, count, symbol(scalar), constant(count, 63)),
    );
    assert_eq!(mismatched.validate(), Err(Error::TypeMismatch));
    assert_eq!(
        mismatched.validate_static_domains(),
        Err(Error::IncompleteDomain)
    );
    let mismatched_literal = shift(
        Op::ShiftLeft,
        scalar,
        binary(Op::BitAnd, count, symbol(count), constant(scalar, 63)),
    );
    assert_eq!(mismatched_literal.validate(), Err(Error::TypeMismatch));
    assert_eq!(
        mismatched_literal.validate_static_domains(),
        Err(Error::IncompleteDomain)
    );
}

#[test]
fn a_mask_does_not_hide_undefined_children_and_literal_boundaries_are_unchanged() {
    let scalar = integer(true, 32);
    let undefined = binary(Op::Divide, scalar, symbol(scalar), constant(scalar, 0));
    refused(shift(
        Op::ShiftLeft,
        scalar,
        binary(Op::BitAnd, scalar, undefined, constant(scalar, 31)),
    ));
    for width in [8, 16, 32, 64] {
        let scalar = integer(false, width);
        for count in [0, u64::from(width) - 1] {
            let expression = shift(Op::ShiftRight, scalar, constant(scalar, count));
            expression.validate().unwrap();
            expression.validate_static_domains().unwrap();
        }
        refused(shift(Op::ShiftLeft, scalar, constant(scalar, width.into())));
    }
}
