fn checked_byte_header_oracle_v48() -> usize {
    use fe2o3_kernel_ir::CheckedBinaryOperator;
    type Fields = ([usize; 2], [usize; 2], u32, bool, CheckedBinaryOperator);
    assert_eq!(size_of::<CheckedByteOperationV48>(), size_of::<Fields>());
    size_of::<Fields>()
        + size_of::<Result<Fields>>()
        + size_of::<(
            CheckedBinaryOperator,
            ScalarType,
            FormalIndexWidth,
            [usize; 4],
            u32,
            bool,
        )>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 2], std::fmt::Result)>()
}

fn checked_byte_module_v48(
    scalar: ScalarType,
    operator: fe2o3_kernel_ir::CheckedBinaryOperator,
    returns: u8,
) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(KirOperation::new(
        vec![
            ValueDef::new(ValueId(2), Type::Scalar(scalar)),
            ValueDef::new(ValueId(3), Type::BOOL),
        ],
        OperationKind::Binary {
            op: BinaryOp::Checked(operator),
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
    ));
    let mut types = Vec::new();
    let mut values = Vec::new();
    for (bit, id, ty) in [(1, 2, Type::Scalar(scalar)), (2, 3, Type::BOOL)] {
        if returns & bit != 0 {
            types.push(ty);
            values.push(ValueId(id));
        }
    }
    entry.terminator = Some(Terminator::Return { values });
    let mut module = Module::new("actual-checked-byte-pairs");
    module.functions.push(KirFunction::internal_helper(
        "checked",
        Signature::new(vec![Type::Scalar(scalar); 2], types),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    module
}

#[test]
fn actual_checked_byte_dispatch_binds_both_result_ordinals_even_when_one_is_unused() {
    use fe2o3_kernel_ir::CheckedBinaryOperator as Op;
    let scalars = [
        ScalarType::U8,
        ScalarType::I8,
        ScalarType::U16,
        ScalarType::I16,
        ScalarType::U32,
        ScalarType::I32,
        ScalarType::U64,
        ScalarType::I64,
        ScalarType::U128,
        ScalarType::I128,
    ];
    for scalar in scalars {
        for operator in [Op::Add, Op::Subtract, Op::Multiply] {
            for returned in [1, 2, 3] {
                with_inventory(
                    &checked_byte_module_v48(scalar, operator, returned),
                    |inventory, physical, floor| {
                        let allocations = NoAllocations(inventory.owner());
                        let text = run(floor, LIMIT, LIMIT, |out| {
                            let body = ByteFunctionV30::derive(
                                inventory,
                                physical,
                                Function(0),
                                ByteContext::native(FormalIndexWidth::Bits64),
                                &allocations,
                                out,
                            )?;
                            let ByteOperationV30::Checked(plan) = body.operations[0] else {
                                panic!("checked arithmetic must reach the actual typed dispatcher");
                            };
                            assert_eq!(plan.operands, [0, 1]);
                            assert_eq!(plan.results, [2, 3]);
                            assert_eq!(plan.operator, operator);
                            assert_eq!(plan.bits, u32::from(scalar.bit_width().unwrap()));
                            assert_eq!(plan.signed, scalar.is_signed_integer());
                            body.emit(480, out)
                        })
                        .0
                        .unwrap();
                        let symbol = match operator {
                            Op::Add => "+",
                            Op::Subtract => "-",
                            Op::Multiply => "*",
                        };
                        assert!(text.contains(&format!(
                            "checked_wide = checked_left {symbol} checked_right"
                        )));
                        assert!(text.contains(&format!(
                            "checked_modulus = memory_value_modulus_v30({})",
                            scalar.bit_width().unwrap() / 8
                        )));
                        assert!(text.contains(".update(2int, MemoryValueV30::Scalar(checked_wide % checked_modulus)).update(3int, MemoryValueV30::Scalar(if checked_in_range { 0int } else { 1int }))"));
                        assert!(text.contains("let valid = checked_ok;"));
                        assert!(text.contains("let memory = s.memory;"));
                        assert!(text.contains("let generations = s.generations;"));
                        assert!(text.contains("let frames = s.frames;"));
                        assert!(text.contains("MemoryOperationEffectV30::Pure"));
                        assert!(!text.contains("checked_ok && checked_in_range"));
                        assert_eq!(
                            text.contains("checked_left_bits - checked_modulus"),
                            scalar.is_signed_integer()
                        );
                    },
                );
            }
        }
    }
}

#[test]
fn actual_checked_byte_sparse_permuted_ids_use_dense_operand_and_result_ordinals() {
    use fe2o3_kernel_ir::CheckedBinaryOperator;
    for [left, right, value, overflow] in [[91, 7, 53, 11], [13, 89, 3, 67]] {
        let mut entry = BasicBlock::new(BlockId(41));
        entry.operations.push(KirOperation::new(
            vec![
                ValueDef::new(ValueId(value), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(overflow), Type::BOOL),
            ],
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                lhs: ValueId(left),
                rhs: ValueId(right),
            },
        ));
        entry.terminator = Some(Terminator::Return {
            values: vec![ValueId(value), ValueId(overflow)],
        });
        let mut module = Module::new("checked-sparse-ids");
        module.functions.push(KirFunction::internal_helper(
            "checked",
            Signature::new(
                vec![Type::Scalar(ScalarType::U32); 2],
                vec![Type::Scalar(ScalarType::U32), Type::BOOL],
            ),
            vec![ValueId(left), ValueId(right)],
            vec![entry],
        ));
        with_inventory(&module, |inventory, physical, floor| {
            run(floor, LIMIT, LIMIT, |out| {
                let allocations = NoAllocations(inventory.owner());
                let body = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )?;
                let ByteOperationV30::Checked(plan) = body.operations[0] else {
                    panic!("genuine Checked operation must retain both dense results");
                };
                assert_eq!(plan.operands, [0, 1]);
                assert_eq!(plan.results, [2, 3]);
                assert_eq!((plan.bits, plan.signed), (32, false));
                body.emit(298, out)
            })
            .0
            .unwrap();
        });
    }
}

#[test]
fn actual_checked_byte_index_width_is_explicit_and_noninteger_shapes_refuse() {
    use fe2o3_kernel_ir::CheckedBinaryOperator as Op;
    for index in [
        FormalIndexWidth::Bits32,
        FormalIndexWidth::Bits64,
        FormalIndexWidth::Unknown,
    ] {
        with_inventory(
            &checked_byte_module_v48(ScalarType::Index, Op::Multiply, 3),
            |inventory, _, floor| {
                let result = run(floor, LIMIT, LIMIT, |out| {
                    let plan = CheckedByteOperationV48::derive(inventory, 0, index, out)?;
                    assert_eq!(
                        plan.bits,
                        if index == FormalIndexWidth::Bits32 {
                            32
                        } else {
                            64
                        }
                    );
                    assert!(!plan.signed);
                    Ok(())
                })
                .0;
                if index == FormalIndexWidth::Unknown {
                    assert!(matches!(
                        result,
                        Err(Error::Statement(
                            "actual checked byte operation is not modeled"
                        ))
                    ));
                } else {
                    result.unwrap();
                }
            },
        );
    }
    for scalar in [
        ScalarType::Bool,
        ScalarType::F16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        assert!(checked::width(scalar, FormalIndexWidth::Bits64).is_err());
    }
    let module = integral_cast_module_v40(ScalarType::U32, ScalarType::U64, 1);
    with_inventory(&module, |inventory, _, floor| {
        assert!(matches!(
            run(floor, LIMIT, LIMIT, |out| {
                CheckedByteOperationV48::derive(inventory, 0, FormalIndexWidth::Bits64, out)
                    .map(|_| ())
            })
            .0,
            Err(Error::Statement(
                "actual checked byte operation is not modeled"
            ))
        ));
    });
}

#[test]
fn actual_checked_byte_emitted_math_matches_exhaustive_eight_bit_value_and_overflow() {
    use fe2o3_kernel_ir::CheckedBinaryOperator as Op;
    for signed in [false, true] {
        for operator in [Op::Add, Op::Subtract, Op::Multiply] {
            let scalar = if signed {
                ScalarType::I8
            } else {
                ScalarType::U8
            };
            with_inventory(
                &checked_byte_module_v48(scalar, operator, 3),
                |inventory, _, floor| {
                    let text = run(floor, LIMIT, LIMIT, |out| {
                        CheckedByteOperationV48::derive(
                            inventory,
                            0,
                            FormalIndexWidth::Bits64,
                            out,
                        )?
                        .emit_step(
                            ByteMemoryStateNamesV30 {
                                values: "v",
                                memory: "m",
                                generations: "g",
                                frames: "f",
                                valid: "ok",
                            },
                            ByteMemoryStateNamesV30 {
                                values: "w",
                                memory: "n",
                                generations: "h",
                                frames: "q",
                                valid: "valid",
                            },
                            out,
                        )
                    })
                    .0
                    .unwrap();
                    let expression = text
                        .split("let checked_wide = ")
                        .nth(1)
                        .unwrap()
                        .split(';')
                        .next()
                        .unwrap();
                    let emitted_signed = text.contains("checked_left_bits - checked_modulus");
                    assert_eq!(emitted_signed, signed);
                    assert!(text.contains(if signed {
                        "-(checked_modulus / 2) <= checked_wide < checked_modulus / 2"
                    } else {
                        "0 <= checked_wide < checked_modulus"
                    }));
                    for a in 0..=255u16 {
                        for b in 0..=255u16 {
                            let decode = |v: u16| {
                                if emitted_signed && v >= 128 {
                                    i32::from(v) - 256
                                } else {
                                    i32::from(v)
                                }
                            };
                            let (left, right) = (decode(a), decode(b));
                            let wide = match expression {
                                "checked_left + checked_right" => left + right,
                                "checked_left - checked_right" => left - right,
                                "checked_left * checked_right" => left * right,
                                other => panic!("unrecognized emitted arithmetic {other}"),
                            };
                            let expected = if signed {
                                let (value, flag) = match operator {
                                    Op::Add => (a as i8).overflowing_add(b as i8),
                                    Op::Subtract => (a as i8).overflowing_sub(b as i8),
                                    Op::Multiply => (a as i8).overflowing_mul(b as i8),
                                };
                                (value as u8, flag)
                            } else {
                                match operator {
                                    Op::Add => (a as u8).overflowing_add(b as u8),
                                    Op::Subtract => (a as u8).overflowing_sub(b as u8),
                                    Op::Multiply => (a as u8).overflowing_mul(b as u8),
                                }
                            };
                            let range = if signed { -128..128 } else { 0..256 };
                            assert_eq!(
                                (wide.rem_euclid(256) as u8, !range.contains(&wide)),
                                expected
                            );
                        }
                    }
                },
            );
        }
    }
}

#[test]
fn actual_checked_byte_wider_extrema_preserve_value_and_overflow_independently() {
    use fe2o3_kernel_ir::CheckedBinaryOperator as Op;
    macro_rules! check_width {
        ($unsigned:ty, $signed:ty, $u:expr, $i:expr) => {
            for (scalar, signed) in [($u, false), ($i, true)] {
                for operator in [Op::Add, Op::Subtract, Op::Multiply] {
                    with_inventory(
                        &checked_byte_module_v48(scalar, operator, 3),
                        |inventory, _, floor| {
                            let text = run(floor, LIMIT, LIMIT, |out| {
                                CheckedByteOperationV48::derive(
                                    inventory,
                                    0,
                                    FormalIndexWidth::Bits64,
                                    out,
                                )?
                                .emit_step(
                                    ByteMemoryStateNamesV30 {
                                        values: "v",
                                        memory: "m",
                                        generations: "g",
                                        frames: "f",
                                        valid: "ok",
                                    },
                                    ByteMemoryStateNamesV30 {
                                        values: "w",
                                        memory: "n",
                                        generations: "h",
                                        frames: "q",
                                        valid: "valid",
                                    },
                                    out,
                                )
                            })
                            .0
                            .unwrap();
                            let expression = text
                                .split("let checked_wide = ")
                                .nth(1)
                                .unwrap()
                                .split(';')
                                .next()
                                .unwrap();
                            let bits = <$unsigned>::BITS;
                            let modulus = 1u128 << bits;
                            let sign = modulus / 2;
                            let values = [0, 1, sign - 1, sign, sign + 1, modulus - 2, modulus - 1];
                            for a in values {
                                for b in values {
                                    let observed = if signed {
                                        let decode = |value: u128| {
                                            if value >= sign {
                                                value as i128 - modulus as i128
                                            } else {
                                                value as i128
                                            }
                                        };
                                        let (left, right) = (decode(a), decode(b));
                                        let wide = match expression {
                                            "checked_left + checked_right" => left + right,
                                            "checked_left - checked_right" => left - right,
                                            "checked_left * checked_right" => left * right,
                                            other => panic!("unrecognized checked formula {other}"),
                                        };
                                        (
                                            wide.rem_euclid(modulus as i128) as u128,
                                            wide < -(sign as i128) || wide >= sign as i128,
                                        )
                                    } else {
                                        match expression {
                                            "checked_left + checked_right" => {
                                                ((a + b) % modulus, a + b >= modulus)
                                            }
                                            "checked_left - checked_right" if a < b => {
                                                (modulus - (b - a), true)
                                            }
                                            "checked_left - checked_right" => (a - b, false),
                                            "checked_left * checked_right" => {
                                                ((a * b) % modulus, a * b >= modulus)
                                            }
                                            other => panic!("unrecognized checked formula {other}"),
                                        }
                                    };
                                    let expected = if signed {
                                        let (value, overflow) = match operator {
                                            Op::Add => (a as $signed).overflowing_add(b as $signed),
                                            Op::Subtract => {
                                                (a as $signed).overflowing_sub(b as $signed)
                                            }
                                            Op::Multiply => {
                                                (a as $signed).overflowing_mul(b as $signed)
                                            }
                                        };
                                        (value as $unsigned as u128, overflow)
                                    } else {
                                        let (value, overflow) = match operator {
                                            Op::Add => {
                                                (a as $unsigned).overflowing_add(b as $unsigned)
                                            }
                                            Op::Subtract => {
                                                (a as $unsigned).overflowing_sub(b as $unsigned)
                                            }
                                            Op::Multiply => {
                                                (a as $unsigned).overflowing_mul(b as $unsigned)
                                            }
                                        };
                                        (value as u128, overflow)
                                    };
                                    assert_eq!(
                                        observed, expected,
                                        "{scalar:?} {operator:?} {a} {b}"
                                    );
                                }
                            }
                        },
                    );
                }
            }
        };
    }
    check_width!(u16, i16, ScalarType::U16, ScalarType::I16);
    check_width!(u32, i32, ScalarType::U32, ScalarType::I32);
    check_width!(u64, i64, ScalarType::U64, ScalarType::I64);
}

#[test]
fn actual_checked_byte_queries_and_complete_emission_keep_exact_resource_boundaries() {
    use fe2o3_kernel_ir::CheckedBinaryOperator as Op;
    with_inventory(
        &checked_byte_module_v48(ScalarType::I64, Op::Multiply, 3),
        |inventory, physical, floor| {
            let query = |out: &mut Writer<'_, '_>| {
                CheckedByteOperationV48::derive(inventory, 0, FormalIndexWidth::Bits64, out)
                    .map(|_| ())
            };
            let storage = floor + super::super::super::SOURCE_LIMIT;
            let measured = run(floor, 48, storage, query);
            measured.0.unwrap();
            assert_eq!((measured.1, measured.2), (48, storage));
            assert!(matches!(run(floor, 47, storage, query).0,
            Err(Error::Resource(Resource::Work(error))) if error.limit() == 47 && error.actual() == 48));
            let allocations = NoAllocations(inventory.owner());
            let generate = |out: &mut Writer<'_, '_>| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )?
                .emit(481, out)
            };
            let measured = run(floor, LIMIT, LIMIT, generate);
            let text = measured.0.unwrap();
            let exact = run(floor, measured.1, measured.2, generate);
            assert_eq!(exact.0.unwrap(), text);
            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
            assert!(matches!(run(floor, measured.1 - 1, measured.2, generate).0,
            Err(Error::Resource(Resource::Work(error))) if error.limit() == measured.1 - 1 && error.actual() == measured.1));
            assert!(matches!(run(floor, measured.1, measured.2 - 1, generate).0,
            Err(Error::Resource(Resource::Storage(error))) if error.limit() == measured.2 - 1 && error.actual() == measured.2));
        },
    );
    assert_eq!(checked::headers(), checked_byte_header_oracle_v48());
}
