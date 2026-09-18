use super::*;
use fe2o3_kernel_ir::{ComparePredicate, Constant};

fn floating_helper(ty: Type, op: BinaryOp) -> Function {
    let mut function = helper("floating", ty, None);
    let OperationKind::Binary { op: actual, .. } =
        &mut function.body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *actual = op;
    function
}

#[test]
fn strict_float_arithmetic_and_retained_nested_calls_are_raw_empty_not_pure() {
    for ty in [Type::F32, Type::F64] {
        for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
            let mut input = module(vec![
                helper("outer", ty.clone(), Some("floating")),
                floating_helper(ty.clone(), op),
            ]);
            // An uncalled output helper must receive the same checked census.
            input.functions.push(identity("orphan", ty.clone()));
            with_inventory(input, |inventory, budget| {
                let report = check(inventory, budget).unwrap();
                for i in 0..3 {
                    assert!(report.function(inventory, Coordinate(i), budget).unwrap());
                }
                assert!(report.call(inventory, 0, budget).unwrap());
                assert!(!report.call(inventory, 1, budget).unwrap());
                assert!(matches!(
                    inventory.operations()[0].operation.kind,
                    OperationKind::Call { .. }
                ));
                assert_eq!(inventory.functions().len(), 3);
            });
        }
    }
}

#[test]
fn float_constants_compare_and_select_preserve_exact_nan_and_signed_zero_bits() {
    for (ty, nan, zero) in [
        (
            Type::F32,
            Constant::F32Bits(0x7fc0_0042),
            Constant::F32Bits(0x8000_0000),
        ),
        (
            Type::F64,
            Constant::F64Bits(0x7ff8_0000_0000_0042),
            Constant::F64Bits(0x8000_0000_0000_0000),
        ),
    ] {
        let mut function = identity("bits", ty.clone());
        let block = &mut function.body.as_mut().unwrap().blocks[0];
        block.operations = vec![
            Operation::new(
                vec![ValueDef::new(ValueId(1), ty.clone())],
                OperationKind::Constant(nan.clone()),
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(2), ty.clone())],
                OperationKind::Constant(zero.clone()),
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(3), Type::BOOL)],
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(4), ty)],
                OperationKind::Select {
                    condition: ValueId(3),
                    true_value: ValueId(1),
                    false_value: ValueId(2),
                },
            ),
        ];
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(4)],
        });
        with_inventory(module(vec![function]), |inventory, budget| {
            let report = check(inventory, budget).unwrap();
            assert!(report.function(inventory, Coordinate(0), budget).unwrap());
            assert_eq!(
                inventory.operations()[0].operation.kind,
                OperationKind::Constant(nan)
            );
            assert_eq!(
                inventory.operations()[1].operation.kind,
                OperationKind::Constant(zero)
            );
            assert_eq!(inventory.operations().len(), 4);
        });
    }
}

#[test]
fn f64_divide_negation_float_remainder_and_unadmitted_casts_remain_closed() {
    for ty in [Type::F32, Type::F64] {
        for op in [BinaryOp::Divide, BinaryOp::Remainder] {
            if ty == Type::F32 && op == BinaryOp::Divide {
                continue;
            }
            expect_refusal(
                module(vec![floating_helper(ty.clone(), op)]),
                "closed scalar helper opcode",
            );
        }
        if ty == Type::F32 {
            continue;
        }
        let mut function = identity("negative", ty.clone());
        let block = &mut function.body.as_mut().unwrap().blocks[0];
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(1), ty)],
            OperationKind::Unary {
                op: UnaryOp::Negate,
                operand: ValueId(0),
            },
        ));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(1)],
        });
        expect_refusal(module(vec![function]), "closed scalar helper opcode");
    }
    for (input, output, kind) in [
        (Type::F32, Type::F64, CastKind::FloatExtend),
        (Type::F64, Type::F32, CastKind::FloatTruncate),
        (
            Type::Scalar(ScalarType::U32),
            Type::F64,
            CastKind::IntegerToFloat,
        ),
        (
            Type::F64,
            Type::Scalar(ScalarType::U32),
            CastKind::FloatToInteger,
        ),
        (Type::F32, Type::Scalar(ScalarType::U32), CastKind::Bitcast),
        (Type::Scalar(ScalarType::U32), Type::F32, CastKind::Bitcast),
    ] {
        let mut block = BasicBlock::new(BlockId(73));
        block.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(1), output.clone())],
            OperationKind::Cast {
                kind,
                value: ValueId(0),
                to: output.clone(),
            },
        ));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(1)],
        });
        expect_refusal(
            module(vec![Function::internal_helper(
                "cast",
                Signature::new(vec![input], vec![output]),
                vec![ValueId(0)],
                vec![block],
            )]),
            "closed scalar helper opcode",
        );
    }
}

#[test]
fn float_extension_does_not_open_plain_integer_arithmetic_or_other_float_widths() {
    for ty in [
        Type::Scalar(ScalarType::U32),
        Type::Scalar(ScalarType::U64),
        Type::Scalar(ScalarType::I32),
        Type::Scalar(ScalarType::I64),
    ] {
        for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
            expect_refusal(
                module(vec![floating_helper(ty.clone(), op)]),
                "closed scalar helper opcode",
            );
        }
    }
    for ty in [
        Type::Scalar(ScalarType::F16),
        Type::Scalar(ScalarType::Bf16),
    ] {
        expect_refusal(
            module(vec![identity("half", ty)]),
            "direct ordinary scalar signature",
        );
    }
}

#[test]
fn safe_float_helper_does_not_hide_unsafe_orphan_or_recursive_closure() {
    let mut input = module(vec![floating_helper(Type::F32, BinaryOp::Add)]);
    let mut orphan = floating_helper(Type::F64, BinaryOp::Divide);
    orphan.id = "orphan".into();
    input.functions.push(orphan);
    expect_refusal(input, "closed scalar helper opcode");
    expect_refusal(
        module(vec![helper("cycle", Type::F32, Some("cycle"))]),
        "complete empty helper closure",
    );
    expect_refusal(
        module(vec![
            helper("a", Type::F64, Some("b")),
            helper("b", Type::F64, Some("a")),
        ]),
        "complete empty helper closure",
    );
}

#[test]
fn floating_storage_and_pointer_arguments_do_not_become_raw_empty() {
    for ty in [Type::F32, Type::F64] {
        let width = ty.as_scalar().unwrap().bit_width().unwrap() / 8;
        let mut function = identity("private", ty.clone());
        function.body.as_mut().unwrap().blocks[0].operations = vec![
            Operation::new(
                vec![ValueDef::new(
                    ValueId(1),
                    Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                )],
                OperationKind::Alloca {
                    element: ty.clone(),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: width.into(),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Private, width.into()),
                },
            ),
        ];
        expect_refusal(module(vec![function]), "ordinary scalar helper definitions");
        expect_refusal(
            module(vec![identity(
                "global",
                Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly),
            )]),
            "direct ordinary scalar signature",
        );
    }
}

#[test]
fn float_report_cannot_be_transplanted_to_another_identical_inventory() {
    with_inventory(
        module(vec![floating_helper(Type::F32, BinaryOp::Add)]),
        |a, budget| {
            let report = check(a, budget).unwrap();
            with_inventory(
                module(vec![floating_helper(Type::F32, BinaryOp::Add)]),
                |b, other| {
                    assert!(matches!(
                        report.function(b, Coordinate(0), other),
                        Err(E::Unsupported {
                            phase: "scalar helpers",
                            detail: "same borrowed inventory"
                        })
                    ));
                    assert!(matches!(
                        report.call(b, 0, other),
                        Err(E::Unsupported {
                            phase: "scalar helpers",
                            detail: "same borrowed inventory"
                        })
                    ));
                },
            );
        },
    );
}

#[test]
fn registered_float_intrinsic_empty_effects_do_not_authorize_ordinary_helper_calls() {
    use fe2o3_kernel_ir::{F32MathFunction, FloatOperation};
    let sqrt = FloatOperation::F32Math {
        function: F32MathFunction::Sqrt,
        implementation: F32MathFunction::Sqrt.required_implementation(),
        arguments: vec![ValueId(0)],
    };
    let mut function = identity("math", Type::F32);
    let block = &mut function.body.as_mut().unwrap().blocks[0];
    block.operations.push(sqrt.operation(ValueId(1)));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    with_inventory(
        module(vec![function, sqrt.declaration()]),
        |inventory, budget| {
            let (effects, storage) = CanonicalKirCallEffectsV1::derive(inventory, budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_eq!(
                effects.decision(Coordinate(0), budget).unwrap(),
                CanonicalKirCallEffectDecisionV1::CompleteEmpty
            );
            drop(effects);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert!(matches!(
                check(inventory, budget),
                Err(E::Unsupported {
                    phase: "scalar helpers",
                    detail: "internal scalar helper callees only",
                })
            ));
        },
    );
}

#[test]
fn return_only_float_helper_keeps_literal_twenty_six_work_boundary() {
    for ty in [Type::F32, Type::F64] {
        with_inventory(
            module(vec![identity("identity", ty)]),
            |inventory, setup| {
                let floor = setup.storage();
                for limit in [25, 26] {
                    let mut work = Work::new(limit);
                    let mut budget = Budget::new(&mut work, STORAGE);
                    budget.reserve_storage(floor).unwrap();
                    {
                        let result = check(inventory, &mut budget);
                        if limit == 26 {
                            assert!(result.is_ok());
                        } else {
                            assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
                        }
                    }
                    assert_eq!(budget.work(), limit);
                    budget.release_storage(budget.storage() - floor).unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(
                        work.failed_work(),
                        if limit == 26 { None } else { Some(26) }
                    );
                }
            },
        );
    }
}

#[test]
fn expanded_float_opcode_accounting_has_exact_and_one_short_work_and_storage() {
    with_inventory(
        module(vec![floating_helper(Type::F64, BinaryOp::Multiply)]),
        |inventory, setup| {
            let floor = setup.storage();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            drop(check(inventory, &mut budget).unwrap());
            let exact_work = budget.work();
            let exact_storage = budget.peak_storage();
            budget.release_storage(budget.storage() - floor).unwrap();
            for (work_limit, storage_limit, succeeds) in [
                (exact_work, exact_storage, true),
                (exact_work - 1, exact_storage, false),
                (exact_work, exact_storage - 1, false),
            ] {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                {
                    let result = check(inventory, &mut budget);
                    if succeeds {
                        assert!(result.is_ok());
                    } else if work_limit < exact_work {
                        assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
                    } else {
                        assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
                    }
                }
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
                if succeeds {
                    assert_eq!(budget.work(), exact_work);
                    assert_eq!(budget.peak_storage(), exact_storage);
                }
            }
        },
    );
}
