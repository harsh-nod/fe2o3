use super::super::super::OperatorV30 as Operator;
use super::*;

fn wrapping_module(ty: Type, operator: BinaryOp) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), ty.clone()),
        OperationKind::Binary {
            op: operator,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut module = Module::new("canonical-wrapping-contract");
    module.functions.push(Function::internal_helper(
        "wrapping",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

fn types() -> Vec<(Type, FormalIndexWidth, u32, bool)> {
    let mut values = Vec::new();
    for (unsigned, signed, width) in [
        (ScalarType::U8, ScalarType::I8, 8),
        (ScalarType::U16, ScalarType::I16, 16),
        (ScalarType::U32, ScalarType::I32, 32),
        (ScalarType::U64, ScalarType::I64, 64),
    ] {
        values.push((
            Type::Scalar(unsigned),
            FormalIndexWidth::Bits64,
            width,
            false,
        ));
        values.push((Type::Scalar(signed), FormalIndexWidth::Bits64, width, true));
    }
    values.push((Type::INDEX, FormalIndexWidth::Bits32, 32, false));
    values.push((Type::INDEX, FormalIndexWidth::Bits64, 64, false));
    values
}

fn operators() -> [(BinaryOp, Operator, &'static str); 3] {
    [
        (BinaryOp::Add, Operator::WrappingAdd, "+"),
        (BinaryOp::Subtract, Operator::WrappingSubtract, "-"),
        (BinaryOp::Multiply, Operator::WrappingMultiply, "*"),
    ]
}

#[test]
fn canonical_wrapping_actual_typed_operations_emit_modular_target_contracts() {
    for (ty, width, bits, signed) in types() {
        for (operator, expected, symbol) in operators() {
            with_module(
                &wrapping_module(ty.clone(), operator),
                |inventory, floor| {
                    let text = run(inventory, floor, LIMIT, LIMIT, |out| {
                        let scalar = CanonicalByteScalarV30::derive(inventory, 0, width, out)?;
                        assert_eq!(
                            scalar.nodes[2].scalar,
                            ScalarV30::Integer {
                                width: bits,
                                signed
                            }
                        );
                        assert_eq!(
                            scalar.nodes[2].expression,
                            ExpressionV30::Binary {
                                operation: expected,
                                left: 0,
                                right: 1
                            }
                        );
                        scalar.emit_definition(0, out)?;
                        let (before, after) = names();
                        scalar.emit_step(0, before, after, out)
                    })
                    .0
                    .unwrap();
                    assert!(text.contains(&format!("(m0 {symbol} m1) % {}int", 1u128 << bits)));
                    assert!(text.contains("let m1 = m0;"));
                    assert!(text.contains("let g1 = g0;"));
                    assert!(text.contains("let f1 = f0;"));
                    assert!(text.contains("v0.update(2int, MemoryValueV30::Scalar("));
                },
            );
        }
    }
}

#[test]
fn canonical_wrapping_modulus_matches_signed_and_unsigned_boundary_oracles() {
    for (_, _, bits, signed) in types() {
        let modulus = 1u128 << bits;
        let mask = modulus - 1;
        let sign = modulus / 2;
        for (operator, _, _) in operators() {
            for (left, right) in [
                (0, 1),
                (mask, 1),
                (sign, mask),
                (sign - 1, 1),
                (mask, mask),
                (sign, sign),
            ] {
                let actual = match operator {
                    BinaryOp::Add => left.wrapping_add(right) & mask,
                    BinaryOp::Subtract => left.wrapping_sub(right) & mask,
                    BinaryOp::Multiply => left.wrapping_mul(right) & mask,
                    _ => unreachable!(),
                };
                let signed_value = |value: u128| {
                    if signed && value >= sign {
                        value as i128 - modulus as i128
                    } else {
                        value as i128
                    }
                };
                let expected = if signed {
                    let lhs = signed_value(left);
                    let rhs = signed_value(right);
                    let raw = match operator {
                        BinaryOp::Add => lhs + rhs,
                        BinaryOp::Subtract => lhs - rhs,
                        BinaryOp::Multiply => lhs * rhs,
                        _ => unreachable!(),
                    };
                    raw.rem_euclid(modulus as i128) as u128
                } else {
                    match operator {
                        BinaryOp::Add => (left + right) % modulus,
                        BinaryOp::Subtract => (left + modulus - right) % modulus,
                        BinaryOp::Multiply => (left * right) % modulus,
                        _ => unreachable!(),
                    }
                };
                assert_eq!(actual, expected);
            }
        }
    }
}

#[test]
fn canonical_wrapping_refuses_other_contracts_and_noninteger_types() {
    use fe2o3_kernel_ir::CheckedBinaryOperator;
    let scalar = ScalarV30::Integer {
        width: 32,
        signed: false,
    };
    let nodes = [NodeV30 {
        scalar,
        expression: ExpressionV30::Argument(0),
    }; 2];
    for operator in [
        BinaryOp::Checked(CheckedBinaryOperator::Add),
        BinaryOp::Checked(CheckedBinaryOperator::Subtract),
        BinaryOp::Checked(CheckedBinaryOperator::Multiply),
        BinaryOp::Divide,
        BinaryOp::Remainder,
        BinaryOp::ShiftLeft,
        BinaryOp::ShiftRight,
    ] {
        let kind = OperationKind::Binary {
            op: operator,
            lhs: ValueId(0),
            rhs: ValueId(1),
        };
        assert!(canonical::operation_expression(&kind, &[0, 1], scalar, &nodes).is_err());
    }
    for (operator, _, _) in operators() {
        let kind = OperationKind::Binary {
            op: operator,
            lhs: ValueId(0),
            rhs: ValueId(1),
        };
        for scalar in [
            ScalarV30::Unit,
            ScalarV30::Bool,
            ScalarV30::Float { width: 32 },
            ScalarV30::Integer {
                width: 1,
                signed: false,
            },
            ScalarV30::Integer {
                width: 128,
                signed: true,
            },
        ] {
            let nodes = [NodeV30 {
                scalar,
                expression: ExpressionV30::Argument(0),
            }; 2];
            assert!(canonical::operation_expression(&kind, &[0, 1], scalar, &nodes).is_err());
        }
        let mut wrong = nodes;
        wrong[1].scalar = ScalarV30::Integer {
            width: 32,
            signed: true,
        };
        assert!(canonical::operation_expression(&kind, &[0, 1], scalar, &wrong).is_err());
    }
    with_module(
        &wrapping_module(Type::INDEX, BinaryOp::Add),
        |inventory, floor| {
            assert!(
                run(inventory, floor, LIMIT, LIMIT, |out| {
                    CanonicalByteScalarV30::derive(inventory, 0, FormalIndexWidth::Unknown, out)
                        .map(|_| ())
                })
                .0
                .is_err()
            );
        },
    );
}

#[test]
fn canonical_wrapping_has_exact_and_one_short_resource_boundaries() {
    for (operator, _, _) in operators() {
        with_module(
            &wrapping_module(Type::Scalar(ScalarType::I64), operator),
            |inventory, floor| {
                let body = |out: &mut Writer<'_, '_>| {
                    let scalar = CanonicalByteScalarV30::derive(
                        inventory,
                        0,
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                    scalar.emit_definition(0, out)?;
                    let (before, after) = names();
                    scalar.emit_step(0, before, after, out)
                };
                let baseline = run(inventory, floor, LIMIT, LIMIT, body);
                baseline.0.unwrap();
                run(inventory, floor, baseline.1, baseline.2, body)
                    .0
                    .unwrap();
                assert!(
                    matches!(run(inventory, floor, baseline.1 - 1, baseline.2, body).0,
                Err(Error::Resource(Resource::Work(error))) if error.actual() == baseline.1 && error.limit() == baseline.1 - 1)
                );
                assert!(
                    matches!(run(inventory, floor, baseline.1, baseline.2 - 1, body).0,
                Err(Error::Resource(Resource::Storage(error))) if error.actual() == baseline.2 && error.limit() == baseline.2 - 1)
                );
            },
        );
    }
}
