use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CastKind, CheckedBinaryOperator, MemoryAccess,
};

fn pointer(ty: Type) -> Type {
    Type::pointer(ty, AddressSpace::Global, AccessMode::ReadWrite)
}
fn store(address: u32, value: u32, alignment: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(address),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Global, alignment),
        },
    )
}

#[test]
fn both_checked_results_have_independent_exact_constant_descendants() {
    let u8_type = Type::Scalar(ScalarType::U8);
    let input = module(
        vec![],
        vec![u8_type.clone(), Type::BOOL],
        vec![],
        vec![returning(
            17,
            vec![
                constant(1, Constant::U8(255)),
                constant(2, Constant::U8(1)),
                Operation::checked_binary(
                    ValueDef::new(ValueId(3), u8_type.clone()),
                    ValueDef::new(ValueId(4), Type::BOOL),
                    CheckedBinaryOperator::Add,
                    ValueId(1),
                    ValueId(2),
                ),
            ],
            &[3, 4],
        )],
    );
    for overflow in [true, false] {
        let output = module(
            vec![],
            vec![u8_type.clone(), Type::BOOL],
            vec![],
            vec![returning(
                17,
                vec![
                    constant(30, Constant::U8(0)),
                    constant(40, Constant::Bool(overflow)),
                ],
                &[30, 40],
            )],
        );
        inspect(
            input.clone(),
            output,
            |a, b| {
                Plan {
                    chains: vec![vec![(0, None)]],
                    operations: vec![
                        Origin::ConstantFrom(result(0, 2, 0)),
                        Origin::ConstantFrom(result(0, 2, 1)),
                    ],
                    relations: vec![(3, 30, S), (4, 40, S)],
                    uses: vec![term(0, 0), term(0, 1)],
                    ..Plan::default()
                }
                .rows(a, b)
            },
            |a, b, rows, floor| {
                if overflow {
                    accepted(a, b, rows, floor);
                    rows.definition_outputs[0].kind = R;
                    assert_eq!(
                        rejected(a, b, rows, floor),
                        Error::Rule("retained definition duplication")
                    );
                } else {
                    assert_eq!(
                        rejected(a, b, rows, floor),
                        Error::Rule("constant synthesis bits")
                    );
                }
            },
        );
    }
}

#[test]
fn ordinary_integer_fold_keeps_original_and_replacement_while_transporting_store() {
    let parameters = vec![pointer(U32)];
    let add = value(
        3,
        U32,
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(1),
            rhs: ValueId(2),
        },
    );
    let input = module(
        parameters.clone(),
        vec![U32],
        vec![99],
        vec![returning(
            9,
            vec![
                constant(1, Constant::U32(10)),
                constant(2, Constant::U32(20)),
                add.clone(),
                store(99, 3, 4),
            ],
            &[3],
        )],
    );
    let output = module(
        parameters,
        vec![U32],
        vec![99],
        vec![returning(
            9,
            vec![
                constant(1, Constant::U32(10)),
                constant(2, Constant::U32(20)),
                constant(30, Constant::U32(30)),
                add,
                store(99, 30, 4),
            ],
            &[30],
        )],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)]],
                operations: vec![
                    Origin::Retained(op(0, 0)),
                    Origin::Retained(op(0, 1)),
                    Origin::ConstantFrom(result(0, 2, 0)),
                    Origin::Retained(op(0, 2)),
                    Origin::Retained(op(0, 3)),
                ],
                relations: vec![(99, 99, R), (1, 1, R), (2, 2, R), (3, 3, R), (3, 30, S)],
                uses: vec![
                    operand(0, 2, 0),
                    operand(0, 2, 1),
                    operand(0, 3, 0),
                    operand(0, 3, 1),
                    term(0, 0),
                ],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            // A second old result descendant does not relabel the surviving original.
            let original = rows
                .definition_outputs
                .iter_mut()
                .find(|row| row.output == result(0, 3, 0))
                .unwrap();
            original.kind = S;
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("complete final definition anchors")
            );
        },
    );
}

#[test]
fn overflow_and_invalid_shift_cannot_manufacture_constant_synthesis() {
    for (operator, left, right, claimed) in [
        (BinaryOp::Add, u32::MAX, 1, 0),
        (BinaryOp::Divide, 1, 0, 0),
        (BinaryOp::ShiftLeft, 1, 32, 0),
    ] {
        let arithmetic = value(
            3,
            U32,
            OperationKind::Binary {
                op: operator,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        );
        let input = module(
            vec![],
            vec![U32],
            vec![],
            vec![returning(
                9,
                vec![
                    constant(1, Constant::U32(left)),
                    constant(2, Constant::U32(right)),
                    arithmetic.clone(),
                ],
                &[3],
            )],
        );
        let output = module(
            vec![],
            vec![U32],
            vec![],
            vec![returning(
                9,
                vec![
                    constant(1, Constant::U32(left)),
                    constant(2, Constant::U32(right)),
                    constant(30, Constant::U32(claimed)),
                    arithmetic,
                ],
                &[30],
            )],
        );
        inspect(
            input,
            output,
            |a, b| {
                Plan {
                    chains: vec![vec![(0, None)]],
                    operations: vec![
                        Origin::Retained(op(0, 0)),
                        Origin::Retained(op(0, 1)),
                        Origin::ConstantFrom(result(0, 2, 0)),
                        Origin::Retained(op(0, 2)),
                    ],
                    relations: vec![(1, 1, R), (2, 2, R), (3, 3, R), (3, 30, S)],
                    uses: vec![operand(0, 2, 0), operand(0, 2, 1), term(0, 0)],
                    ..Plan::default()
                }
                .rows(a, b)
            },
            |a, b, rows, floor| {
                assert_eq!(
                    rejected(a, b, rows, floor),
                    Error::Rule("constant synthesis bits")
                )
            },
        );
    }
}

#[test]
fn float_select_retains_nan_payload_and_signed_zero_bits_next_to_physical_effects() {
    for (selected, other, alignment) in [
        (
            Constant::F32Bits(0x7fc0_0123),
            Constant::F32Bits(0x7fc0_0456),
            4,
        ),
        (
            Constant::F64Bits(0x8000_0000_0000_0000),
            Constant::F64Bits(0),
            8,
        ),
    ] {
        let ty = selected.ty();
        let parameters = vec![pointer(ty.clone())];
        let input = module(
            parameters.clone(),
            vec![ty.clone()],
            vec![99],
            vec![returning(
                1,
                vec![
                    constant(1, Constant::Bool(true)),
                    constant(2, selected.clone()),
                    constant(3, other.clone()),
                    value(
                        4,
                        ty.clone(),
                        OperationKind::Select {
                            condition: ValueId(1),
                            true_value: ValueId(2),
                            false_value: ValueId(3),
                        },
                    ),
                    store(99, 4, alignment),
                ],
                &[4],
            )],
        );
        for correct in [true, false] {
            let output = module(
                parameters.clone(),
                vec![ty.clone()],
                vec![99],
                vec![returning(
                    1,
                    vec![
                        constant(
                            44,
                            if correct {
                                selected.clone()
                            } else {
                                other.clone()
                            },
                        ),
                        store(99, 44, alignment),
                    ],
                    &[44],
                )],
            );
            inspect(
                input.clone(),
                output,
                |a, b| {
                    Plan {
                        chains: vec![vec![(0, None)]],
                        operations: vec![
                            Origin::ConstantFrom(result(0, 3, 0)),
                            Origin::Retained(op(0, 4)),
                        ],
                        relations: vec![(99, 99, R), (4, 44, S)],
                        uses: vec![operand(0, 4, 0), operand(0, 4, 1), term(0, 0)],
                        ..Plan::default()
                    }
                    .rows(a, b)
                },
                |a, b, rows, floor| {
                    if correct {
                        accepted(a, b, rows, floor);
                    } else {
                        assert_eq!(
                            rejected(a, b, rows, floor),
                            Error::Rule("constant synthesis bits")
                        );
                    }
                },
            );
        }
    }
}

#[test]
fn pure_float_bitcast_cse_preserves_exact_typed_result_and_operand_origin() {
    let f32_type = Type::Scalar(ScalarType::F32);
    let cast = |id| {
        value(
            id,
            f32_type.clone(),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(8),
                to: f32_type.clone(),
            },
        )
    };
    let parameters = vec![pointer(f32_type.clone()), U32];
    let input = module(
        parameters.clone(),
        vec![f32_type.clone()],
        vec![99, 8],
        vec![returning(8, vec![cast(1), cast(2), store(99, 2, 4)], &[2])],
    );
    let output = module(
        parameters,
        vec![f32_type.clone()],
        vec![99, 8],
        vec![returning(8, vec![cast(1), store(99, 1, 4)], &[1])],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)]],
                operations: vec![Origin::Retained(op(0, 0)), Origin::Retained(op(0, 2))],
                relations: vec![(99, 99, R), (8, 8, R), (1, 1, R), (2, 1, S)],
                uses: vec![
                    operand(0, 0, 0),
                    operand(0, 2, 0),
                    operand(0, 2, 1),
                    term(0, 0),
                ],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            // Both casts consume the same integer, but retained slot identity is exact.
            rows.uses[0].input = operand(0, 1, 0);
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("final operation operand origin")
            );
        },
    );
}

#[test]
fn ordinary_float_operation_cannot_disappear_as_if_native_pure() {
    let ty = Type::Scalar(ScalarType::F32);
    let input = module(
        vec![ty.clone(), ty.clone()],
        vec![],
        vec![1, 2],
        vec![returning(
            4,
            vec![value(
                3,
                ty.clone(),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(1),
                    rhs: ValueId(2),
                },
            )],
            &[],
        )],
    );
    let output = module(
        vec![ty.clone(), ty],
        vec![],
        vec![1, 2],
        vec![returning(4, vec![], &[])],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)]],
                relations: vec![(1, 1, R), (2, 2, R)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            assert_eq!(
                rejected(a, b, rows, floor),
                Error::Rule("executable ordered operation removed")
            )
        },
    );
}

#[test]
fn ordered_physical_operations_cannot_be_reordered_or_omitted() {
    let parameters = vec![pointer(U32), U32, U32];
    let input = module(
        parameters.clone(),
        vec![],
        vec![99, 1, 2],
        vec![returning(3, vec![store(99, 1, 4), store(99, 2, 4)], &[])],
    );
    for omitted in [false, true] {
        let output = module(
            parameters.clone(),
            vec![],
            vec![99, 1, 2],
            vec![returning(
                3,
                if omitted {
                    vec![store(99, 1, 4)]
                } else {
                    vec![store(99, 2, 4), store(99, 1, 4)]
                },
                &[],
            )],
        );
        inspect(
            input.clone(),
            output,
            |a, b| {
                Plan {
                    chains: vec![vec![(0, None)]],
                    operations: if omitted {
                        vec![Origin::Retained(op(0, 0))]
                    } else {
                        vec![Origin::Retained(op(0, 1)), Origin::Retained(op(0, 0))]
                    },
                    relations: vec![(99, 99, R), (1, 1, R), (2, 2, R)],
                    uses: if omitted {
                        vec![operand(0, 0, 0), operand(0, 0, 1)]
                    } else {
                        vec![
                            operand(0, 1, 0),
                            operand(0, 1, 1),
                            operand(0, 0, 0),
                            operand(0, 0, 1),
                        ]
                    },
                    ..Plan::default()
                }
                .rows(a, b)
            },
            |a, b, rows, floor| {
                assert_eq!(
                    rejected(a, b, rows, floor),
                    Error::Rule(if omitted {
                        "executable ordered operation removed"
                    } else {
                        "ordered operation order"
                    })
                );
            },
        );
    }
}

#[test]
fn same_width_mixed_signedness_shift_matches_native_fold_and_keeps_producer() {
    let signed = Type::Scalar(ScalarType::I32);
    let shift = value(
        3,
        signed.clone(),
        OperationKind::Binary {
            op: BinaryOp::ShiftRight,
            lhs: ValueId(1),
            rhs: ValueId(2),
        },
    );
    let input = module(
        vec![],
        vec![signed.clone()],
        vec![],
        vec![returning(
            9,
            vec![
                constant(1, Constant::I32(-8)),
                constant(2, Constant::U32(1)),
                shift.clone(),
            ],
            &[3],
        )],
    );
    let output = module(
        vec![],
        vec![signed],
        vec![],
        vec![returning(
            9,
            vec![
                constant(1, Constant::I32(-8)),
                constant(2, Constant::U32(1)),
                constant(30, Constant::I32(-4)),
                shift,
            ],
            &[30],
        )],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)]],
                operations: vec![
                    Origin::Retained(op(0, 0)),
                    Origin::Retained(op(0, 1)),
                    Origin::ConstantFrom(result(0, 2, 0)),
                    Origin::Retained(op(0, 2)),
                ],
                relations: vec![(1, 1, R), (2, 2, R), (3, 3, R), (3, 30, S)],
                uses: vec![operand(0, 2, 0), operand(0, 2, 1), term(0, 0)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| accepted(a, b, rows, floor),
    );
}
