fn tagged_select_header_oracle_v55() -> usize {
    type Fields<'a> = ([usize; 3], usize, &'a Type, FormalIndexWidth);
    assert_eq!(size_of::<TaggedSelectV55<'_>>(), size_of::<Fields<'_>>());
    size_of::<Fields<'static>>()
        + size_of::<Result<Option<Fields<'static>>>>()
        + size_of::<([ValueId; 3], [usize; 3], [&'static Type; 3])>()
        + size_of::<(
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
        )>()
        + size_of::<([usize; 8], [&(); 8], [Result<()>; 3], std::fmt::Result)>()
}

fn tagged_select_module_v55(ty: Type, swapped: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(KirOperation::effect_free(
        ValueDef::new(ValueId(3), ty.clone()),
        OperationKind::Select {
            condition: ValueId(0),
            true_value: ValueId(if swapped { 2 } else { 1 }),
            false_value: ValueId(if swapped { 1 } else { 2 }),
        },
    ));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut module = Module::new("tagged-select-actual-operands");
    module
        .storage_layouts
        .push(fe2o3_kernel_ir::StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::U32),
        });
    module.functions.push(KirFunction::internal_helper(
        "select",
        Signature::new(vec![Type::BOOL, ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry],
    ));
    module
}

#[test]
fn tagged_select_transports_exact_pointer_slice_vector_and_unit_values_in_both_arm_orders() {
    let mut types = vec![
        Type::Unit,
        Type::Vector(fe2o3_kernel_ir::FixedVectorTypeV12::new(
            ScalarType::U32,
            4,
            fe2o3_kernel_ir::VectorLayoutV12::Interleaved { factor: 2 },
        )),
    ];
    for space in [
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Generic,
    ] {
        for access in [
            AccessMode::ReadOnly,
            AccessMode::ReadWrite,
            AccessMode::WriteOnly,
        ] {
            for element in [
                Type::Scalar(ScalarType::U32),
                Type::F32,
                Type::F64,
                Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(0)),
            ] {
                types.push(Type::pointer(element, space, access));
            }
            types.push(Type::slice(Type::Scalar(ScalarType::U32), space, access));
        }
    }
    for ty in types {
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
            for swapped in [false, true] {
                with_inventory(
                    &tagged_select_module_v55(ty.clone(), swapped),
                    |inventory, physical, floor| {
                        let allocations = NoAllocations(inventory.owner());
                        let text = run(floor, LIMIT, LIMIT, |out| {
                            let model = ByteFunctionV30::derive(
                                inventory,
                                physical,
                                Function(0),
                                ByteContext::native(width),
                                &allocations,
                                out,
                            )?;
                            assert!(matches!(
                                model.operations[0],
                                ByteOperationV30::TaggedSelect(_)
                            ));
                            model.emit(55, out)
                        })
                        .0
                        .unwrap();
                        let (first, second) = if swapped { (2, 1) } else { (1, 2) };
                        assert!(text.contains(&format!("else if s.values[0] == MemoryValueV30::Scalar(1int) {{ s.values[{first}] }} else {{ s.values[{second}] }}")));
                        assert!(text.contains("s.values.update(3int, tagged_select_value)"));
                        assert!(text.contains("let memory = s.memory;"));
                        assert!(text.contains("let generations = s.generations;"));
                        assert!(text.contains("let frames = s.frames;"));
                        assert!(text.contains("MemoryOperationEffectV30::Pure"));
                        assert!(!text.contains("memory_allocate"));
                        assert!(!text.contains("MemoryPointerV30 {"));
                        assert!(!text.contains("ieee_operators("));
                        assert!(!text.contains("canonical_scalar_result_"));
                    },
                );
            }
        }
    }
}

#[test]
fn tagged_select_only_validates_the_selected_runtime_value_without_dereferencing_it() {
    with_inventory(
        &tagged_select_module_v55(pointer(), false),
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
                .emit(55, out)
            })
            .0
            .unwrap();
            let begin = text.find(" let tagged_select_inputs_ok =").unwrap();
            let end = text[begin..].find(" let effect =").unwrap() + begin;
            let step = &text[begin..end];
            assert!(step.contains("s.values[0] == MemoryValueV30::Scalar(0int) || s.values[0] == MemoryValueV30::Scalar(1int)"));
            assert!(step.contains("if !tagged_select_inputs_ok { MemoryValueV30::Undefined }"));
            assert!(step.contains(
                "match tagged_select_value { MemoryValueV30::Pointer(p) => byte_pointer_type_v30(p,"
            ));
            assert!(!step.contains("match s.values[1]"));
            assert!(!step.contains("match s.values[2]"));
            assert!(!step.contains("byte_range_live"));
            assert!(!step.contains("byte_range_initialized"));
            assert!(step.contains("else { s.values };"));
        },
    );
}

#[test]
fn tagged_select_keeps_scalar_and_ieee_adapters_closed_and_distinct() {
    for (scalar, floating) in [
        (ScalarType::U32, false),
        (ScalarType::F32, true),
        (ScalarType::F64, true),
    ] {
        with_inventory(
            &tagged_select_module_v55(Type::Scalar(scalar), false),
            |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                run(floor, LIMIT, LIMIT, |out| {
                    let model = ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(FormalIndexWidth::Bits64),
                        &allocations,
                        out,
                    )?;
                    if floating {
                        assert!(matches!(model.operations[0], ByteOperationV30::Float(_)));
                    } else {
                        assert!(matches!(model.operations[0], ByteOperationV30::Scalar(_)));
                    }
                    Ok(())
                })
                .0
                .unwrap();
            },
        );
    }
}

#[test]
fn tagged_select_rejects_wrong_condition_rights_space_pointee_and_width() {
    let exact = pointer();
    for (condition, first, second) in [
        (Type::INDEX, exact.clone(), exact.clone()),
        (
            Type::BOOL,
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadOnly,
            ),
            exact.clone(),
        ),
        (
            Type::BOOL,
            exact.clone(),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        (
            Type::BOOL,
            exact.clone(),
            Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
        ),
        (
            Type::BOOL,
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
            exact.clone(),
        ),
    ] {
        assert!(
            run(0, LIMIT, LIMIT, |out| {
                super::super::select_types(&condition, &first, &second, &exact, out)
            })
            .0
            .is_err()
        );
    }
    with_inventory(
        &tagged_select_module_v55(exact, false),
        |inventory, _, floor| {
            for (operation, width) in [
                (usize::MAX, FormalIndexWidth::Bits64),
                (0, FormalIndexWidth::Unknown),
            ] {
                let result = run(floor, LIMIT, LIMIT, |out| {
                    out.budget.reserve_storage(tagged_select::headers())?;
                    let error = TaggedSelectV55::derive(inventory, operation, width, out)
                        .err()
                        .expect("invalid tagged Select must refuse");
                    assert!(out.text.is_empty());
                    Err(error)
                });
                assert!(result.0.is_err());
            }
        },
    );
}

#[test]
fn tagged_select_has_independent_exact_derive_work_and_full_emission_bounds() {
    let ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    with_inventory(
        &tagged_select_module_v55(ty, false),
        |inventory, physical, floor| {
            // Dispatch2 + fixed16 + three exact uses8 each + Select4 + three
            // two-node type comparisons3 per node. No type-depth-free equality.
            let work = 2 + 16 + 3 * 8 + 4 + 3 * 2 * 3;
            let storage =
                floor + super::super::super::SOURCE_LIMIT + tagged_select_header_oracle_v55();
            let derive = |out: &mut Writer<'_, '_>| {
                out.budget.reserve_storage(tagged_select::headers())?;
                assert!(
                    TaggedSelectV55::derive(inventory, 0, FormalIndexWidth::Bits64, out)?.is_some()
                );
                Ok(())
            };
            let measured = run(floor, LIMIT, LIMIT, derive);
            measured.0.unwrap();
            assert_eq!((measured.1, measured.2), (work, storage));
            run(floor, work, storage, derive).0.unwrap();
            assert!(matches!(
                run(floor, work - 1, storage, derive).0,
                Err(Error::Resource(_))
            ));
            assert!(matches!(
                run(floor, work, storage - 1, derive).0,
                Err(Error::Resource(_))
            ));
            let allocations = NoAllocations(inventory.owner());
            let emit = |out: &mut Writer<'_, '_>| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )?
                .emit(55, out)
            };
            let measured = run(floor, LIMIT, LIMIT, emit);
            let text = measured.0.unwrap();
            assert_eq!(run(floor, measured.1, measured.2, emit).0.unwrap(), text);
            assert!(matches!(
                run(floor, measured.1 - 1, measured.2, emit).0,
                Err(Error::Resource(_))
            ));
            assert!(matches!(
                run(floor, measured.1, measured.2 - 1, emit).0,
                Err(Error::Resource(_))
            ));
        },
    );
}
