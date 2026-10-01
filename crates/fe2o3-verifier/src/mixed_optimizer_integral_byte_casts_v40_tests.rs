fn integral_cast_header_oracle_v40() -> usize {
    use fe2o3_kernel_ir::CastKind;
    type Fields = (usize, usize, u32, u32, CastKind);
    assert_eq!(size_of::<IntegralByteCastV40>(), size_of::<Fields>());
    size_of::<Fields>()
        + size_of::<Result<Fields>>()
        + size_of::<(CastKind, ScalarType, ScalarType, FormalIndexWidth, [u32; 2])>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 2], std::fmt::Result)>()
}

fn integral_cast_module_v40(from: ScalarType, to: ScalarType, copies: usize) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    let mut next = 1;
    for _ in 0..copies {
        let mut input = ValueId(0);
        for (kind, result) in fe2o3_kernel_ir::plan_integer_cast_v1(from, to)
            .unwrap()
            .into_iter()
            .flatten()
        {
            block.operations.push(KirOperation::effect_free(
                ValueDef::new(ValueId(next), Type::Scalar(result)),
                OperationKind::Cast {
                    kind,
                    value: input,
                    to: Type::Scalar(result),
                },
            ));
            input = ValueId(next);
            next += 1;
        }
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("general-integral-byte-casts");
    module.functions.push(KirFunction::internal_helper(
        "casts",
        Signature::new(vec![Type::Scalar(from)], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn byte_integral_casts_cover_all_fixed_width_signed_unsigned_and_bool_inputs() {
    use fe2o3_kernel_ir::CastKind;
    let types = [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::I128,
        ScalarType::U128,
    ];
    let mut count = 0;
    for from in types.into_iter().chain([ScalarType::Bool]) {
        for to in types {
            if from == to {
                continue;
            }
            let module = integral_cast_module_v40(from, to, 1);
            with_inventory(&module, |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                let text = run(floor, LIMIT, LIMIT, |out| {
                    let model = ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(FormalIndexWidth::Bits64),
                        &allocations,
                        out,
                    )?;
                    let ByteOperationV30::IntegralCast(plan) = &model.operations[0] else {
                        panic!("integral operation must use the complete byte dispatcher");
                    };
                    assert_eq!(plan.input_bits, u32::from(from.bit_width().unwrap()));
                    assert_eq!(plan.output_bits, u32::from(to.bit_width().unwrap()));
                    assert_eq!((plan.input, plan.result), (0, 1));
                    match from.bit_width().cmp(&to.bit_width()) {
                        std::cmp::Ordering::Greater => assert_eq!(plan.kind, CastKind::Truncate),
                        std::cmp::Ordering::Less if from.is_signed_integer() => {
                            assert_eq!(plan.kind, CastKind::SignExtend)
                        }
                        std::cmp::Ordering::Less => assert_eq!(plan.kind, CastKind::ZeroExtend),
                        std::cmp::Ordering::Equal => assert_eq!(plan.kind, CastKind::Bitcast),
                    }
                    model.emit(140, out)
                })
                .0
                .unwrap();
                assert!(
                    text.contains("MemoryValueV30::Scalar(v) => 0 <= v < integral_source_modulus")
                );
                assert!(text.contains("MemoryOperationEffectV30::Pure"));
                assert!(text.contains("let memory = s.memory"));
                assert!(!text.contains("MemoryValueV30::Pointer(p) =>"));
                if from == ScalarType::Bool {
                    assert!(text.contains("integral_source_modulus = 2int"));
                }
                if from.is_signed_integer() && from.bit_width() < to.bit_width() {
                    assert!(text.contains(
                        "integral_input + integral_target_modulus - integral_source_modulus"
                    ));
                }
                if to == ScalarType::U128 || to == ScalarType::I128 {
                    assert!(
                        text.contains("integral_target_modulus = memory_value_modulus_v30(16)")
                    );
                }
                count += 1;
            });
        }
    }
    assert_eq!(count, 100);
}

#[test]
fn byte_integral_index_bridges_require_explicit_actual_width_without_truncating_bitcasts() {
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
        for (from, to) in [
            (ScalarType::U32, ScalarType::Index),
            (ScalarType::U64, ScalarType::Index),
            (ScalarType::Index, ScalarType::U64),
            (ScalarType::U128, ScalarType::Index),
            (ScalarType::Index, ScalarType::I128),
        ] {
            with_inventory(
                &integral_cast_module_v40(from, to, 1),
                |inventory, physical, floor| {
                    let allocations = NoAllocations(inventory.owner());
                    let result = run(floor, LIMIT, LIMIT, |out| {
                        ByteFunctionV30::derive(
                            inventory,
                            physical,
                            Function(0),
                            ByteContext::native(width),
                            &allocations,
                            out,
                        )?
                        .emit(141, out)
                    })
                    .0;
                    if width == FormalIndexWidth::Bits32 && from != ScalarType::U32 {
                        assert!(matches!(
                            result,
                            Err(Error::Statement("actual integral byte cast is not modeled"))
                        ));
                    } else {
                        let text = result.unwrap();
                        assert!(text.contains("let integral_result = integral_input"));
                    }
                },
            );
        }
    }
}

#[test]
fn byte_integral_cast_derivation_has_independent_linear_resource_boundaries() {
    for copies in [1, 4, 16] {
        with_inventory(
            &integral_cast_module_v40(ScalarType::I64, ScalarType::I128, copies),
            |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                let derive = |out: &mut Writer<'_, '_>| {
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(FormalIndexWidth::Bits64),
                        &allocations,
                        out,
                    )
                    .map(|_| ())
                };
                // Owner1 + body4 + definitions(1+C) + control5. Each cast:
                // dispatch4, Alloca6, Storage6, pointer26+operand8, cast32, physical19.
                let work = 11 + 102 * copies;
                let storage = floor
                    + super::super::super::SOURCE_LIMIT
                    + headers::<NoAllocations<'_>>()
                    + copies * size_of::<ByteOperationV30<'_, '_>>();
                let exact = run(floor, work, storage, derive);
                assert!(exact.0.unwrap().is_empty());
                assert_eq!((exact.1, exact.2), (work, storage));
                assert!(matches!(run(floor, work - 1, storage, derive).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1 && error.actual() == work));
                assert!(matches!(run(floor, work, storage - 1, derive).0,
                Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1 && error.actual() == storage));
                let emit = |out: &mut Writer<'_, '_>| {
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(FormalIndexWidth::Bits64),
                        &allocations,
                        out,
                    )?
                    .emit(142, out)
                };
                let measured = run(floor, LIMIT, LIMIT, emit);
                let text = measured.0.unwrap();
                assert_eq!(run(floor, measured.1, measured.2, emit).0.unwrap(), text);
                assert!(matches!(
                    run(floor, measured.1 - 1, measured.2, emit).0,
                    Err(Error::Resource(Resource::Work(_)))
                ));
                assert!(matches!(
                    run(floor, measured.1, measured.2 - 1, emit).0,
                    Err(Error::Resource(Resource::Storage(_)))
                ));
            },
        );
    }
}

#[test]
fn byte_integral_cast_consumes_actual_u128_discriminant_results_without_new_authority() {
    use fe2o3_kernel_ir::CastKind;
    let mut module = storage_view_module_v39(64, None, true, 1);
    module.functions[0].blocks[0]
        .operations
        .push(KirOperation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
            OperationKind::Cast {
                kind: CastKind::Truncate,
                value: ValueId(1),
                to: Type::Scalar(ScalarType::U32),
            },
        ));
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let text = run(floor, LIMIT, LIMIT, |out| {
            let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
            contracts.emit(1, out)?;
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                ByteContext::classified(FormalIndexWidth::Bits64, &contracts, 1),
                &allocations,
                out,
            )?
            .emit(143, out)
        })
        .0
        .unwrap();
        assert!(text.contains(".discriminants[variant]"));
        assert!(text.contains("MemoryTagReadPurposeV39::DiscriminantRead"));
        assert!(text.contains("integral_source_modulus = memory_value_modulus_v30(16)"));
        assert!(text.contains("integral_target_modulus = memory_value_modulus_v30(4)"));
        assert!(text.contains("integral_input % integral_target_modulus"));
        assert!(!text.contains("assume("));
    });
}

#[test]
fn byte_integral_cast_keeps_float_conversions_explicitly_unsupported() {
    let mut module = integral_cast_module_v40(ScalarType::U32, ScalarType::U64, 1);
    module.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::F32);
    module.functions[0].blocks[0].operations[0] = KirOperation::effect_free(
        ValueDef::new(ValueId(1), Type::Scalar(ScalarType::F64)),
        OperationKind::Cast {
            kind: fe2o3_kernel_ir::CastKind::FloatExtend,
            value: ValueId(0),
            to: Type::Scalar(ScalarType::F64),
        },
    );
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        assert!(matches!(
            run(floor, LIMIT, LIMIT, |out| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )
                .map(|_| ())
            })
            .0,
            Err(Error::Statement("actual integral byte cast is not modeled"))
        ));
    });
}
