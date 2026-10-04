fn float_reuse_module_v56(scalar: ScalarType, count: usize, changed: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    for index in 0..count {
        entry.operations.push(KirOperation::effect_free(
            ValueDef::new(ValueId(index as u32 + 2), Type::Scalar(scalar)),
            OperationKind::Binary {
                op: if changed && index == count / 2 {
                    BinaryOp::Multiply
                } else {
                    BinaryOp::Add
                },
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ));
    }
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("complete-float-body-sharing");
    module.functions.push(KirFunction::internal_helper(
        "floating",
        Signature::new(vec![Type::Scalar(scalar); 2], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    module
}

#[test]
fn emitted_float_transitions_share_complete_bodies_without_losing_output_guards() {
    for scalar in [ScalarType::F32, ScalarType::F64] {
        for count in [1, 17, 96] {
            with_inventory(
                &float_reuse_module_v56(scalar, count, false),
                |input, physical, input_floor| {
                    with_inventory(
                        &float_reuse_module_v56(scalar, count, true),
                        |output, output_physical, output_floor| {
                            let allocations = NoAllocations(input.owner());
                            let output_allocations = NoAllocations(output.owner());
                            let context = ByteContext::native(FormalIndexWidth::Bits64);
                            let emit = |out: &mut Writer<'_, '_>| {
                                let mut emitted = EmittedByteFunctionsV55::new(
                                    input,
                                    physical,
                                    &allocations,
                                    context,
                                    out,
                                )?;
                                let before = ByteFunctionV30::derive(
                                    input,
                                    physical,
                                    Function(0),
                                    context,
                                    &allocations,
                                    out,
                                )?;
                                emitted.emit(&before, 50, out)?;
                                emit!(out, "mod output {{\nuse super::*;\n");
                                let after = ByteFunctionV30::derive(
                                    output,
                                    output_physical,
                                    Function(0),
                                    context,
                                    &output_allocations,
                                    out,
                                )?;
                                emitted.emit_output_reusing_bodies_v56(&after, 1, out)?;
                                emit!(out, "}}\n");
                                Ok(())
                            };
                            let floor = input_floor + output_floor;
                            let measured = run(floor, LIMIT, LIMIT, emit);
                            let source = measured.0.unwrap();
                            let output_text = source.split_once("mod output {").unwrap().1;
                            assert_eq!(
                                source.matches("open spec fn byte_transition_body_").count(),
                                count
                            );
                            assert_eq!(
                                output_text
                                    .matches("use super::byte_transition_body_")
                                    .count(),
                                count - 1
                            );
                            assert_eq!(
                                output_text.matches("open spec fn byte_operation_").count(),
                                count
                            );
                            assert_eq!(
                                output_text
                                    .matches("s.pc != 0 || !byte_inputs_1_v55(s, little_endian)")
                                    .count(),
                                count
                            );
                            assert!(!output_text.contains(&format!(
                                " as byte_transition_body_1_{}_v56;",
                                count / 2
                            )));
                            assert!(output_text.contains("byte_float_value_v52(s.frames, 13int,"));
                            assert!(!source.contains("assume("));
                            let inline = run(output_floor, LIMIT, LIMIT, |out| {
                                ByteFunctionV30::derive(
                                    output,
                                    output_physical,
                                    Function(0),
                                    context,
                                    &output_allocations,
                                    out,
                                )?
                                .emit(1, out)
                            })
                            .0
                            .unwrap();
                            if count > 1 {
                                assert!(
                                    output_text.len() * 3 < inline.len() * 2,
                                    "complete output size must shrink without omitting operations"
                                );
                            }
                            let exact = run(floor, measured.1, measured.2, emit);
                            assert_eq!(exact.0.unwrap(), source);
                            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
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
                },
            );
        }
    }
}

#[test]
fn emitted_transitions_cover_memory_checked_index_cast_select_and_trap_models() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let modules = [
        memory_module(1),
        pointer_storage_module_v37(false, 2),
        pointer_storage_module_v37(true, 2),
        checked_byte_module_v48(
            ScalarType::I32,
            fe2o3_kernel_ir::CheckedBinaryOperator::Add,
            3,
        ),
        index_module_v37(1),
        integral_cast_module_v40(ScalarType::I8, ScalarType::U64, 1),
        tagged_select_module_v55(pointer, false),
        tagged_select_module_v55(Type::Unit, false),
        trap_module_v40(3),
    ];
    for module in &modules {
        with_inventory(module, |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
                let context = ByteContext::native(width);
                let source = run(floor, LIMIT, LIMIT, |out| {
                    let mut emitted = EmittedByteFunctionsV55::new(
                        inventory,
                        physical,
                        &allocations,
                        context,
                        out,
                    )?;
                    let model = ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        context,
                        &allocations,
                        out,
                    )?;
                    assert!(
                        model
                            .operations
                            .iter()
                            .all(|operation| !matches!(operation, ByteOperationV30::Scalar(_)))
                    );
                    emitted.emit(&model, 55, out)?;
                    emit!(out, "mod output {{\nuse super::*;\n");
                    emitted.emit_output_reusing_bodies_v56(&model, 1, out)?;
                    emit!(out, "}}\n");
                    Ok(())
                })
                .0
                .unwrap();
                let count = inventory.functions()[0].operations.len();
                let output = source.split_once("mod output {").unwrap().1;
                assert_eq!(
                    source.matches("open spec fn byte_transition_body_").count(),
                    count
                );
                assert_eq!(
                    output.matches("use super::byte_transition_body_").count(),
                    count
                );
                assert_eq!(
                    output.matches("open spec fn byte_operation_").count(),
                    count
                );
                assert!(output.contains("open spec fn byte_micro_step_1_v30"));
                assert!(output.contains("open spec fn byte_block_step_1_v30"));
            }
        });
    }
}

#[test]
fn emitted_transition_descriptors_do_not_share_changed_effects_operands_or_index_context() {
    let mut memory = memory_module(1);
    let operations = &mut memory.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let OperationKind::Load { access, .. } = &mut operations[1].kind else {
        panic!("load");
    };
    access.alignment = 1;
    let OperationKind::Store { access, .. } = &mut operations[2].kind else {
        panic!("store");
    };
    access.alignment = 2;
    let mut index = index_module_v37(1);
    index.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Y,
            },
            Type::INDEX,
        ));
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    let cases = [
        (memory_module(1), memory, vec![1, 2]),
        (index_module_v37(1), index, vec![0]),
        (
            pointer_storage_module_v37(false, 1),
            pointer_storage_module_v37(true, 1),
            vec![1],
        ),
        (
            tagged_select_module_v55(pointer.clone(), false),
            tagged_select_module_v55(pointer, true),
            vec![0],
        ),
        (
            checked_byte_module_v48(
                ScalarType::U64,
                fe2o3_kernel_ir::CheckedBinaryOperator::Add,
                3,
            ),
            checked_byte_module_v48(
                ScalarType::U64,
                fe2o3_kernel_ir::CheckedBinaryOperator::Subtract,
                3,
            ),
            vec![0],
        ),
    ];
    for (before, after, changed) in cases {
        with_inventory(&before, |input, physical, input_floor| {
            with_inventory(&after, |output, output_physical, output_floor| {
                let allocations = NoAllocations(input.owner());
                let output_allocations = NoAllocations(output.owner());
                let context = ByteContext::native(FormalIndexWidth::Bits64);
                let source = run(input_floor + output_floor, LIMIT, LIMIT, |out| {
                    let mut emitted =
                        EmittedByteFunctionsV55::new(input, physical, &allocations, context, out)?;
                    let model = ByteFunctionV30::derive(
                        input,
                        physical,
                        Function(0),
                        context,
                        &allocations,
                        out,
                    )?;
                    emitted.emit(&model, 55, out)?;
                    let model = ByteFunctionV30::derive(
                        output,
                        output_physical,
                        Function(0),
                        context,
                        &output_allocations,
                        out,
                    )?;
                    emit!(out, "mod output {{\nuse super::*;\n");
                    emitted.emit_output_reusing_bodies_v56(&model, 1, out)?;
                    emit!(out, "}}\n");
                    Ok(())
                })
                .0
                .unwrap();
                let output_text = source.split_once("mod output {").unwrap().1;
                assert_eq!(
                    output_text
                        .matches("use super::byte_transition_body_")
                        .count(),
                    input.functions()[0].operations.len() - changed.len()
                );
                for operation in changed {
                    assert!(
                        !output_text
                            .contains(&format!(" as byte_transition_body_1_{operation}_v56;"))
                    );
                    assert!(
                        output_text
                            .contains(&format!("open spec fn byte_operation_1_{operation}_v30"))
                    );
                }
            });
        });
    }
}

#[test]
fn emitted_storage_view_transitions_keep_the_exact_interpretation_descriptor() {
    with_inventory(
        &storage_view_module_v39(8, Some(ScalarType::U8), false, 1),
        |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            for changed in [false, true] {
                let source = run(floor, LIMIT, LIMIT, |out| {
                    let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                    let other = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
                    let context = ByteContext::classified(FormalIndexWidth::Bits64, &contracts, 9);
                    let next = ByteContext::classified(
                        FormalIndexWidth::Bits64,
                        if changed { &other } else { &contracts },
                        if changed { 10 } else { 9 },
                    );
                    let mut emitted = EmittedByteFunctionsV55::new(
                        inventory,
                        physical,
                        &allocations,
                        context,
                        out,
                    )?;
                    let model = ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        context,
                        &allocations,
                        out,
                    )?;
                    assert!(
                        model
                            .operations
                            .iter()
                            .all(|operation| matches!(operation, ByteOperationV30::View(_)))
                    );
                    emitted.emit(&model, 55, out)?;
                    let model = ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        next,
                        &allocations,
                        out,
                    )?;
                    emit!(out, "mod output {{\nuse super::*;\n");
                    emitted.emit_output_reusing_bodies_v56(&model, 1, out)?;
                    emit!(out, "}}\n");
                    Ok(())
                })
                .0
                .unwrap();
                let output = source.split_once("mod output {").unwrap().1;
                assert_eq!(
                    output.matches("use super::byte_transition_body_").count(),
                    if changed {
                        0
                    } else {
                        inventory.operations().len()
                    }
                );
                assert!(output.contains(&format!(
                    "byte_target_view_contracts_match_{}_v38(s.memory, little_endian)",
                    if changed { 10 } else { 9 }
                )));
            }
        },
    );
}

#[test]
fn emitted_alloca_transitions_replay_captured_generation_site_recipes() {
    struct Sites<'a> {
        owner: &'a Owner,
        site: Cell<ByteAllocationSiteV30>,
    }
    impl ByteAllocationResolverV30 for Sites<'_> {
        fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
            out.budget.charge_work(1)?;
            if std::ptr::eq(owner, self.owner) {
                Ok(())
            } else {
                Err(mismatch())
            }
        }
        fn site(&self, _: Operation, out: &mut Writer<'_, '_>) -> Result<ByteAllocationSiteV30> {
            out.budget.charge_work(1)?;
            Ok(self.site.get())
        }
    }
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(KirOperation::effect_free(
        ValueDef::new(
            ValueId(0),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("emitted-transition-allocation-recipe-custody");
    module.functions.push(KirFunction::internal_helper(
        "body",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    with_inventory(&module, |inventory, physical, floor| {
        run(floor, LIMIT, LIMIT, |out| {
            let original = ByteAllocationSiteV30 {
                original: inventory.operations()[0].coordinate,
                physical_root_owner: 0,
            };
            let source = Sites {
                owner: inventory.owner(),
                site: Cell::new(original),
            };
            let output = Sites {
                owner: inventory.owner(),
                site: Cell::new(original),
            };
            let context = ByteContext::native(FormalIndexWidth::Bits64);
            let mut emitted =
                EmittedByteFunctionsV55::new(inventory, physical, &source, context, out)?;
            let before =
                ByteFunctionV30::derive(inventory, physical, Function(0), context, &source, out)?;
            emitted.emit(&before, 55, out)?;
            let after =
                ByteFunctionV30::derive(inventory, physical, Function(0), context, &output, out)?;
            let start = out.text.len();
            emitted.emit_output_reusing_bodies_v56(&after, 1, out)?;
            assert!(out.text[start..].contains(
                "use super::byte_transition_body_55_0_v56 as byte_transition_body_1_0_v56;"
            ));
            for change in 0..4 {
                let mut wrong = original;
                match change {
                    0 => wrong.physical_root_owner += 1,
                    1 => wrong.original.block.function.0 += 1,
                    2 => wrong.original.block.block += 1,
                    _ => wrong.original.operation += 1,
                }
                output.site.set(wrong);
                assert!(matches!(
                    emitted.emit_output_reusing_bodies_v56(&after, 2, out),
                    Err(Error::Statement(_))
                ));
                source.site.set(wrong);
                assert!(matches!(
                    emitted.emit_output_reusing_bodies_v56(&after, 2, out),
                    Err(Error::Statement(_))
                ));
                output.site.set(original);
                assert!(matches!(
                    emitted.emit_output_reusing_bodies_v56(&after, 2, out),
                    Err(Error::Statement(_))
                ));
                source.site.set(original);
                output.site.set(wrong);
                let changed = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    context,
                    &output,
                    out,
                )?;
                let start = out.text.len();
                emitted.emit_output_reusing_bodies_v56(&changed, 3, out)?;
                let independent = &out.text[start..];
                assert!(!independent.contains("use super::byte_transition_body_"));
                assert!(independent.contains("allocation_site_v30 = MemoryPrivateSiteV30"));
                assert!(independent.contains("allocation_generation_v30 + 1"));
                assert!(independent.contains("!s.memory.live.contains_key(allocation_v30)"));
                output.site.set(original);
            }
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn emitted_tagged_select_transitions_compare_the_complete_type() {
    let before = tagged_select_module_v55(
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        false,
    );
    for changed in [
        Type::pointer(
            Type::Scalar(ScalarType::I32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Generic,
            AccessMode::ReadOnly,
        ),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
    ] {
        with_inventory(&before, |input, physical, input_floor| {
            with_inventory(
                &tagged_select_module_v55(changed, false),
                |output, output_physical, output_floor| {
                    let allocations = NoAllocations(input.owner());
                    let output_allocations = NoAllocations(output.owner());
                    let context = ByteContext::native(FormalIndexWidth::Bits64);
                    run(input_floor + output_floor, LIMIT, LIMIT, |out| {
                        let mut emitted = EmittedByteFunctionsV55::new(
                            input,
                            physical,
                            &allocations,
                            context,
                            out,
                        )?;
                        let model = ByteFunctionV30::derive(
                            input,
                            physical,
                            Function(0),
                            context,
                            &allocations,
                            out,
                        )?;
                        emitted.emit(&model, 55, out)?;
                        let after = ByteFunctionV30::derive(
                            output,
                            output_physical,
                            Function(0),
                            context,
                            &output_allocations,
                            out,
                        )?;
                        let start = out.text.len();
                        emitted.emit_output_reusing_bodies_v56(&after, 1, out)?;
                        assert!(!out.text[start..].contains("use super::byte_transition_body_"));
                        assert!(out.text[start..].contains("open spec fn byte_operation_1_0_v30"));
                        Ok(())
                    })
                    .0
                    .unwrap();
                },
            );
        });
    }
}

#[test]
fn emitted_float_transition_reuse_retains_ledger_and_storage_failure() {
    with_inventory(
        &float_reuse_module_v56(ScalarType::F32, 2, false),
        |inventory, physical, floor| {
            for foreign in [false, true] {
                let allocations = NoAllocations(inventory.owner());
                let context = ByteContext::native(FormalIndexWidth::Bits64);
                run(floor, LIMIT, LIMIT, |out| {
                    let before = out.budget.storage();
                    let mut emitted = EmittedByteFunctionsV55::new(
                        inventory,
                        physical,
                        &allocations,
                        context,
                        out,
                    )?;
                    let model = ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        context,
                        &allocations,
                        out,
                    )?;
                    emitted.emit(&model, 55, out)?;
                    if foreign {
                        let mut work = Work::new(LIMIT);
                        let mut budget = Budget::new(&mut work, LIMIT);
                        budget.reserve_storage(out.budget.storage())?;
                        let mut other = Writer::new(&mut budget)?;
                        assert!(matches!(
                            emitted.emit_output_reusing_bodies_v56(&model, 1, &mut other),
                            Err(Error::Resource(Resource::Accounting))
                        ));
                    } else {
                        out.budget.release_storage(out.budget.storage() - before)?;
                    }
                    assert!(matches!(
                        emitted.emit_output_reusing_bodies_v56(&model, 1, out),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    Ok(())
                })
                .0
                .unwrap();
            }
        },
    );
}
