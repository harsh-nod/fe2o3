use super::*;

fn checked_fixture(ty: ScalarType, operator: CheckedBinaryOperator, both: bool) -> Module {
    let mut module = fixture();
    let scalar = Type::Scalar(ty);
    let function = &mut module.functions[0];
    function.signature.parameters[1] = scalar.clone();
    function.signature.parameters[2] =
        Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let body = &mut function.body.as_mut().unwrap().blocks[2];
    body.operations[0] = Operation::checked_binary(
        ValueDef::new(ValueId(30), scalar.clone()),
        ValueDef::new(ValueId(33), Type::BOOL),
        operator,
        ValueId(1),
        ValueId(1),
    );
    body.operations[1] = op(
        31,
        scalar,
        if both {
            OperationKind::Select {
                condition: ValueId(33),
                true_value: ValueId(30),
                false_value: ValueId(1),
            }
        } else {
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(30),
            }
        },
    );
    body.operations[2] = Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(2),
            value: ValueId(31),
            access: MemoryAccess::new(AddressSpace::Global, 1),
        },
    );
    module
}

#[test]
fn checked_licm_pair_authenticates_both_actual_results_and_all_fixed_width_operators() {
    for ty in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for operator in [
            CheckedBinaryOperator::Add,
            CheckedBinaryOperator::Subtract,
            CheckedBinaryOperator::Multiply,
        ] {
            for both in [false, true] {
                let input = checked_fixture(ty, operator, both);
                let (output, rows) = prescribed(&input);
                relation(&input, &output, &rows).unwrap();
                let moved = &output.functions[0].body.as_ref().unwrap().blocks[0].operations[2];
                assert_eq!(
                    moved.results,
                    [
                        ValueDef::new(ValueId(30), Type::Scalar(ty)),
                        ValueDef::new(ValueId(33), Type::BOOL)
                    ]
                );
                assert_eq!(
                    moved.kind,
                    input.functions[0].body.as_ref().unwrap().blocks[2].operations[0].kind
                );
            }
        }
    }
}

#[test]
fn checked_licm_pair_rejects_operator_operand_and_overflow_definition_substitution() {
    let input = checked_fixture(ScalarType::U32, CheckedBinaryOperator::Add, true);
    let (output, rows) = prescribed(&input);
    for fault in 0..3 {
        let mut changed = output.clone();
        let blocks = &mut changed.functions[0].body.as_mut().unwrap().blocks;
        match fault {
            0 => {
                blocks[0].operations[2].kind = OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Multiply),
                    lhs: ValueId(1),
                    rhs: ValueId(1),
                }
            }
            1 => {
                blocks[0].operations[2].kind = OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    lhs: ValueId(1),
                    rhs: ValueId(11),
                }
            }
            2 => {
                blocks[0].operations[2].results[1].id = ValueId(99);
                let OperationKind::Select { condition, .. } = &mut blocks[0].operations[3].kind
                else {
                    panic!("overflow consumer")
                };
                *condition = ValueId(99);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            relation(&input, &changed, &rows),
            Err(Error::Mismatch("exact operation payload and ValueIds"))
        );
    }
}

#[test]
fn checked_licm_pair_retains_explicit_index_and_wide_integer_refusal() {
    for ty in [ScalarType::Index, ScalarType::I128, ScalarType::U128] {
        let input = checked_fixture(ty, CheckedBinaryOperator::Multiply, true);
        let (output, rows) = prescribed(&input);
        assert_eq!(
            relation(&input, &output, &rows),
            Err(Error::Mismatch(
                "eligible total operation and exact loop/preheader"
            ))
        );
    }
}
