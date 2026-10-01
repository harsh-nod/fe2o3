fn float_byte_header_oracle_v52() -> usize {
    #[allow(dead_code)]
    enum ActionFields {
        Constant(u64),
        Operator(u8),
        Select,
    }
    type Fields = (ActionFields, [usize; 3], usize, usize, u32, u32);
    assert_eq!(size_of::<FloatByteOperationV52>(), size_of::<Fields>());
    size_of::<Fields>()
        + size_of::<Result<Option<Fields>>>()
        + size_of::<(
            ActionFields,
            [Option<ValueId>; 3],
            [usize; 3],
            u32,
            u32,
            usize,
        )>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 2], std::fmt::Result)>()
}

fn float_byte_module_v52(scalar: ScalarType, kind: OperationKind, output: ScalarType) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(KirOperation::new(
        vec![ValueDef::new(ValueId(2), Type::Scalar(output))],
        kind,
    ));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut module = Module::new("exact-float-byte-operators");
    module.functions.push(KirFunction::internal_helper(
        "floating",
        Signature::new(vec![Type::Scalar(scalar); 2], vec![Type::Scalar(output)]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    module
}

#[test]
fn actual_float_values_preserve_constant_bits_and_closed_operator_keys() {
    use fe2o3_kernel_ir::{ComparePredicate, Constant, UnaryOp};
    for (scalar, bits) in [(ScalarType::F32, 32), (ScalarType::F64, 64)] {
        let binaries = [
            (BinaryOp::Add, 11),
            (BinaryOp::Subtract, 12),
            (BinaryOp::Multiply, 13),
            (BinaryOp::Divide, 14),
            (BinaryOp::Remainder, 15),
        ];
        let comparisons = [
            (ComparePredicate::Equal, 16),
            (ComparePredicate::NotEqual, 17),
            (ComparePredicate::LessThan, 18),
            (ComparePredicate::LessThanOrEqual, 19),
            (ComparePredicate::GreaterThan, 20),
            (ComparePredicate::GreaterThanOrEqual, 21),
        ];
        for (kind, code, output) in std::iter::once((
            OperationKind::Unary {
                op: UnaryOp::Negate,
                operand: ValueId(0),
            },
            10,
            scalar,
        ))
        .chain(binaries.into_iter().map(|(op, code)| {
            (
                OperationKind::Binary {
                    op,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
                code,
                scalar,
            )
        }))
        .chain(comparisons.into_iter().map(|(predicate, code)| {
            (
                OperationKind::Compare {
                    predicate,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
                code,
                ScalarType::Bool,
            )
        })) {
            with_inventory(
                &float_byte_module_v52(scalar, kind, output),
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
                        assert!(matches!(body.operations[0], ByteOperationV30::Float(_)));
                        body.emit(520, out)
                    })
                    .0
                    .unwrap();
                    let output_bits = if output == ScalarType::Bool { 1 } else { bits };
                    assert!(text.contains(&format!("byte_float_value_v52(s.frames, {code}int, {bits}int, {output_bits}int, s.values[0],")));
                    if code != 10 {
                        assert!(text.contains("s.values[0], s.values[1])"));
                    }
                    assert!(text.contains("let frames = s.frames;"));
                    assert!(text.contains("MemoryOperationEffectV30::Pure"));
                    assert!(!text.contains("assume("));
                },
            );
        }
        let patterns: &[u64] = if bits == 32 {
            &[
                0,
                0x8000_0000,
                0x7f80_0000,
                0x7fc0_0123,
                0x7f80_0042,
                0xffff_ffff,
            ]
        } else {
            &[
                0,
                0x8000_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0x7ff8_0000_0000_0123,
                0x7ff0_0000_0000_0042,
                u64::MAX,
            ]
        };
        for &pattern in patterns {
            let constant = if bits == 32 {
                Constant::F32Bits(pattern as u32)
            } else {
                Constant::F64Bits(pattern)
            };
            with_inventory(
                &float_byte_module_v52(scalar, OperationKind::Constant(constant), scalar),
                |inventory, physical, floor| {
                    let allocations = NoAllocations(inventory.owner());
                    let text = run(floor, LIMIT, LIMIT, |out| {
                        ByteFunctionV30::derive(
                            inventory,
                            physical,
                            Function(0),
                            ByteContext::native(FormalIndexWidth::Bits64),
                            &allocations,
                            out,
                        )?
                        .emit(521, out)
                    })
                    .0
                    .unwrap();
                    assert!(text.contains(&format!("MemoryValueV30::Scalar({pattern}int)")));
                    assert!(!text.contains("byte_float_value_v52(s.frames"));
                },
            );
        }
    }
}

#[test]
fn actual_float_opcode_width_and_operand_order_substitutions_remain_distinct() {
    use sha2::Digest as _;
    let mut hashes = [[0u8; 32]; 4];
    for (index, (scalar, op, lhs, rhs)) in [
        (ScalarType::F32, BinaryOp::Subtract, 0, 1),
        (ScalarType::F64, BinaryOp::Subtract, 0, 1),
        (ScalarType::F32, BinaryOp::Add, 0, 1),
        (ScalarType::F32, BinaryOp::Subtract, 1, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let kind = OperationKind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        };
        with_inventory(
            &float_byte_module_v52(scalar, kind, scalar),
            |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                let text = run(floor, LIMIT, LIMIT, |out| {
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(FormalIndexWidth::Bits64),
                        &allocations,
                        out,
                    )?
                    .emit(522, out)
                })
                .0
                .unwrap();
                hashes[index] = sha2::Sha256::digest(text.as_bytes()).into();
            },
        );
    }
    for index in 0..hashes.len() {
        for prior in 0..index {
            assert_ne!(hashes[index], hashes[prior]);
        }
    }
    // A genuine integer operation cannot acquire float semantics from width.
    with_inventory(
        &float_byte_module_v52(
            ScalarType::U32,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
            ScalarType::U32,
        ),
        |inventory, _, floor| {
            run(floor, LIMIT, LIMIT, |out| {
                assert!(FloatByteOperationV52::derive(inventory, 0, out)?.is_none());
                Ok(())
            })
            .0
            .unwrap();
        },
    );
}

#[test]
fn actual_float_header_is_independent_and_complete() {
    assert_eq!(floating::headers(), float_byte_header_oracle_v52());
}
