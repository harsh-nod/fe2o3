use super::*;
use fe2o3_kernel_ir::ValueDef;

fn run(
    arguments: &[CheckedU32PrefixArgumentV1],
    operations: &[Operation],
) -> Result<Origin, CheckedU32PrefixErrorV1> {
    let operand = u32::MAX;
    let value = u32::MAX - 1;
    let overflow = u32::MAX - 2;
    let literal = 17;
    checked_u32_prefix_kernel_assemble_body_v1!(
        ordinary_exec,
        arguments,
        operations,
        operand,
        value,
        overflow,
        literal,
        origins,
        argument,
        [],
        [],
        index,
        [],
        []
    )
}

fn argument(argument: usize, id: u32) -> CheckedU32PrefixArgumentV1 {
    CheckedU32PrefixArgumentV1 {
        argument,
        semantic_local: argument as u32,
        kernel_ir_value: ValueId(id),
    }
}

fn constant(id: u32, value: u32) -> Operation {
    Operation {
        results: vec![ValueDef {
            id: ValueId(id),
            ty: Type::Scalar(ScalarType::U32),
        }],
        kind: OperationKind::Constant(Constant::U32(value)),
    }
}

fn terminal(rhs: u32) -> Operation {
    Operation {
        results: vec![
            ValueDef {
                id: ValueId(u32::MAX - 1),
                ty: Type::Scalar(ScalarType::U32),
            },
            ValueDef {
                id: ValueId(u32::MAX - 2),
                ty: Type::Scalar(ScalarType::Bool),
            },
        ],
        kind: OperationKind::Binary {
            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
            lhs: ValueId(u32::MAX),
            rhs: ValueId(rhs),
        },
    }
}

#[test]
fn sparse_ids_preserve_argument_order_and_constant_versions() {
    let args = [argument(0, 42), argument(1, u32::MAX)];
    assert_eq!(
        run(&args, &[constant(0, 17), terminal(0)]),
        Ok(Origin::Argument(1))
    );
    assert_eq!(
        run(&[], &[constant(u32::MAX, 91), constant(0, 17), terminal(0)]),
        Ok(Origin::Constant(91))
    );
    assert_eq!(
        run(&[], &[constant(u32::MAX, 92), constant(0, 17), terminal(0)]),
        Ok(Origin::Constant(92))
    );
}

#[test]
fn bounds_reject_before_indexing_and_accept_exact_capacity() {
    let args = [argument(0, u32::MAX)];
    for operations in [vec![], vec![terminal(0)], vec![constant(0, 17); 258]] {
        assert_eq!(
            run(&args, &operations),
            Err(CheckedU32PrefixErrorV1::Kernel)
        );
    }
    let mut args: Vec<_> = (0..128)
        .map(|index| argument(index, index as u32 + 1000))
        .collect();
    args[127].kernel_ir_value = ValueId(u32::MAX);
    let mut operations: Vec<_> = (0..256).map(|id| constant(id, 17)).collect();
    operations.push(terminal(255));
    assert_eq!(run(&args, &operations), Ok(Origin::Argument(127)));
    args.push(argument(128, 999));
    assert_eq!(
        run(&args, &operations),
        Err(CheckedU32PrefixErrorV1::Kernel)
    );
}

#[test]
fn duplicate_and_nonpositional_arguments_reject() {
    for args in [
        vec![argument(1, u32::MAX)],
        vec![argument(0, u32::MAX), argument(1, u32::MAX)],
    ] {
        assert_eq!(
            run(&args, &[constant(0, 17), terminal(0)]),
            Err(CheckedU32PrefixErrorV1::Kernel)
        );
    }
}

#[test]
fn constant_shapes_and_shadowed_ids_reject() {
    let args = [argument(0, u32::MAX)];
    for mutation in 0..7 {
        let mut ops = vec![constant(7, 88), constant(0, 17), terminal(0)];
        match mutation {
            0 => ops[0].results[0].id = ValueId(u32::MAX),
            1 => ops[0].results[0].id = ValueId(0),
            2 => ops[0].results[0].ty = Type::Scalar(ScalarType::I32),
            3 => ops[0].results.clear(),
            4 => {
                let extra = ops[1].results[0].clone();
                ops[0].results.push(extra);
            }
            5 => ops[0].kind = OperationKind::Constant(Constant::U64(88)),
            _ => ops[0].kind = terminal(0).kind,
        }
        assert_eq!(
            run(&args, &ops),
            Err(CheckedU32PrefixErrorV1::Kernel),
            "mutation {mutation}"
        );
    }
}

#[test]
fn terminal_exact_operator_types_order_freshness_and_literal_reject() {
    let args = [argument(0, u32::MAX)];
    for mutation in 0..15 {
        let mut ops = vec![constant(7, 17), constant(0, 17), terminal(0)];
        match mutation {
            0 => ops[2].results.clear(),
            1 => {
                ops[2].results.pop();
            }
            2 => ops[2].results.swap(0, 1),
            3 => ops[2].results[0].id = ValueId(u32::MAX - 2),
            4 => ops[2].results[0].id = ValueId(7),
            5 => ops[2].results[1].id = ValueId(u32::MAX),
            6 => ops[2].results[0].ty = Type::Scalar(ScalarType::U64),
            7 => ops[2].results[1].ty = Type::Scalar(ScalarType::U32),
            8 => ops[1].kind = OperationKind::Constant(Constant::U32(18)),
            9 => ops[2] = terminal(7),
            10 => ops[2] = terminal(123),
            11 => {
                if let OperationKind::Binary { ref mut lhs, .. } = ops[2].kind {
                    *lhs = ValueId(7);
                }
            }
            12 => {
                if let OperationKind::Binary { ref mut op, .. } = ops[2].kind {
                    *op = BinaryOp::Add;
                }
            }
            13 => {
                if let OperationKind::Binary { ref mut op, .. } = ops[2].kind {
                    *op = BinaryOp::Checked(CheckedBinaryOperator::Subtract);
                }
            }
            _ => ops[2].kind = OperationKind::Constant(Constant::U32(17)),
        }
        assert_eq!(
            run(&args, &ops),
            Err(CheckedU32PrefixErrorV1::Kernel),
            "mutation {mutation}"
        );
    }
    assert_eq!(
        run(&[], &[constant(0, 17), terminal(0)]),
        Err(CheckedU32PrefixErrorV1::ValueMismatch)
    );
}
