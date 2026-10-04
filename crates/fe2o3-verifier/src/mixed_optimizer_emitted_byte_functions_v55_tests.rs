#[test]
fn emitted_byte_functions_reuse_exact_memory_and_control_models_with_resource_boundaries() {
    with_inventory(&memory_module(2), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let context = ByteContext::native(FormalIndexWidth::Bits64);
        let emit = |out: &mut Writer<'_, '_>| {
            let mut emitted =
                EmittedByteFunctionsV55::new(inventory, physical, &allocations, context, out)?;
            for function in 0..2 {
                let function_floor = out.budget.storage();
                let model = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(function),
                    context,
                    &allocations,
                    out,
                )?;
                emitted.emit(&model, 50 + function as usize, out)?;
                drop(model);
                out.budget
                    .release_storage(out.budget.storage() - function_floor)?;
            }
            emit!(out, "mod reused {{\nuse super::*;\n");
            for function in 0..2 {
                // Distinct wrapper identity is permitted only after checking
                // the exact retained model and each captured allocation site.
                let wrapper = NoAllocations(inventory.owner());
                let model = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(function),
                    context,
                    &wrapper,
                    out,
                )?;
                emitted.emit_parent_aliases(&model, function as usize, out)?;
            }
            emit!(out, "}}\n");
            Ok(())
        };
        let measured = run(floor, LIMIT, LIMIT, emit);
        let source = measured.0.unwrap();
        let reused = source.split_once("mod reused {").unwrap().1;
        assert!(!reused.contains("open spec fn"));
        for function in 0..2 {
            let source_namespace = 50 + function;
            for operation in (function * 3)..(function * 3 + 3) {
                assert_eq!(
                    source
                        .matches(&format!(
                            "open spec fn byte_operation_{source_namespace}_{operation}_v30("
                        ))
                        .count(),
                    1
                );
                assert!(reused.contains(&format!("byte_operation_{source_namespace}_{operation}_v30 as byte_operation_{function}_{operation}_v30,")));
            }
            for name in ["micro_begin", "micro_step", "micro_finish", "block_step"] {
                assert!(reused.contains(&format!(
                    "byte_{name}_{source_namespace}_v30 as byte_{name}_{function}_v30,"
                )));
            }
            assert!(reused.contains(&format!("byte_control_{source_namespace}_{function}_v30 as byte_control_{function}_{function}_v30,")));
            assert!(reused.contains(&format!("byte_block_{source_namespace}_{function}_v30 as byte_block_{function}_{function}_v30,")));
        }
        assert_eq!(source.matches("MemoryOperationEffectV30::Read").count(), 2);
        assert_eq!(source.matches("MemoryOperationEffectV30::Write").count(), 2);
        let exact = run(floor, measured.1, measured.2, emit);
        assert_eq!(exact.0.unwrap(), source);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(matches!(run(floor, measured.1 - 1, measured.2, emit).0,
            Err(Error::Resource(Resource::Work(error)))
                if error.limit() == measured.1 - 1 && error.actual() == measured.1));
        assert!(matches!(run(floor, measured.1, measured.2 - 1, emit).0,
            Err(Error::Resource(Resource::Storage(error)))
                if error.limit() == measured.2 - 1 && error.actual() == measured.2));
    });
}

fn scalar_reuse_module_v55(changed: bool, count: u32) -> Module {
    let mut module = Module::new("scalar-body-output-sharing");
    for function in 0..2 {
        let mut block = BasicBlock::new(BlockId(0));
        for operation in 0..count {
            block.operations.push(KirOperation::effect_free(
                ValueDef::new(ValueId(operation), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(
                    operation + u32::from(changed && operation == count / 2),
                )),
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(KirFunction::internal_helper(
            format!("body{function}"),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
    }
    module
}

#[test]
fn emitted_scalar_bodies_keep_output_predicates_and_exact_changed_operations() {
    let count = 16;
    with_inventory(
        &scalar_reuse_module_v55(false, count),
        |input, physical, input_floor| {
            with_inventory(
                &scalar_reuse_module_v55(true, count),
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
                        for function in 0..2 {
                            let model = ByteFunctionV30::derive(
                                input,
                                physical,
                                Function(function),
                                context,
                                &allocations,
                                out,
                            )?;
                            emitted.emit(&model, 50 + function as usize, out)?;
                        }
                        emit!(out, "mod output {{\nuse super::*;\n");
                        for function in 0..2 {
                            let model = ByteFunctionV30::derive(
                                output,
                                output_physical,
                                Function(function),
                                context,
                                &output_allocations,
                                out,
                            )?;
                            emitted.emit_output_reusing_bodies_v56(
                                &model,
                                function as usize * 2 + 1,
                                out,
                            )?;
                        }
                        emit!(out, "}}\n");
                        Ok(())
                    };
                    let floor = input_floor + output_floor;
                    let measured = run(floor, LIMIT, LIMIT, emit);
                    let source = measured.0.unwrap();
                    let output_text = source.split_once("mod output {").unwrap().1;
                    assert_eq!(
                        source.matches("open spec fn byte_scalar_body_").count(),
                        2 * count as usize
                    );
                    assert_eq!(
                        output_text.matches("use super::byte_scalar_body_").count(),
                        2 * (count as usize - 1)
                    );
                    assert_eq!(
                        output_text.matches("open spec fn byte_operation_").count(),
                        2 * count as usize
                    );
                    assert_eq!(
                        output_text
                            .matches("open spec fn original_canonical_byte_scalar_trace_")
                            .count(),
                        2
                    );
                    for function in 0..2 {
                        let namespace = function * 2 + 1;
                        assert!(output_text.contains(&format!(
                            "s.pc != {function} || !byte_inputs_{namespace}_v55(s, little_endian)"
                        )));
                        let changed = function * count as usize + count as usize / 2;
                        assert!(
                            !output_text.contains(&format!(
                                " as byte_scalar_body_{namespace}_{changed}_v55;"
                            ))
                        );
                    }
                    assert!(!source.contains("assume("));
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

#[test]
fn emitted_scalar_bodies_refuse_missing_width_and_sticky_foreign_ledger() {
    with_inventory(
        &scalar_reuse_module_v55(false, 2),
        |inventory, physical, floor| {
            run(floor, LIMIT, LIMIT, |out| {
                let allocations = NoAllocations(inventory.owner());
                let context = ByteContext::native(FormalIndexWidth::Bits64);
                let mut emitted =
                    EmittedByteFunctionsV55::new(inventory, physical, &allocations, context, out)?;
                let model = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    context,
                    &allocations,
                    out,
                )?;
                assert!(matches!(
                    emitted.emit_output_reusing_bodies_v56(&model, 1, out),
                    Err(Error::Statement(_))
                ));
                assert!(out.text.is_empty());
                emitted.emit(&model, 50, out)?;
                let wrong_width = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits32),
                    &allocations,
                    out,
                )?;
                let bytes = out.text.len();
                assert!(matches!(
                    emitted.emit_output_reusing_bodies_v56(&wrong_width, 1, out),
                    Err(Error::Statement(_))
                ));
                assert_eq!(out.text.len(), bytes);
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut foreign = Writer::new(&mut budget)?;
                assert!(matches!(
                    emitted.emit_output_reusing_bodies_v56(&model, 1, &mut foreign),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!(foreign.budget.work(), 0);
                assert!(foreign.text.is_empty());
                let work = out.budget.work();
                assert!(matches!(
                    emitted.emit_output_reusing_bodies_v56(&model, 1, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!(out.budget.work(), work);
                assert_eq!(out.text.len(), bytes);
                Ok(())
            })
            .0
            .unwrap();
        },
    );
}

#[test]
fn emitted_byte_functions_keep_large_complete_census_without_duplicate_interpreters() {
    let count = 1024_u32;
    let mut entry = BasicBlock::new(BlockId(0));
    for operation in 0..count {
        entry.operations.push(KirOperation::effect_free(
            ValueDef::new(ValueId(operation), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(operation)),
        ));
    }
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("complete-emitted-byte-model-reuse");
    module.functions.push(KirFunction::internal_helper(
        "body",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let context = ByteContext::native(FormalIndexWidth::Bits64);
        let source = run(floor, LIMIT, LIMIT, |out| {
            let mut emitted =
                EmittedByteFunctionsV55::new(inventory, physical, &allocations, context, out)?;
            let model = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                context,
                &allocations,
                out,
            )?;
            emitted.emit(&model, 55, out)?;
            emit!(out, "mod reused {{\nuse super::*;\n");
            emitted.emit_parent_aliases(&model, 0, out)?;
            emit!(out, "}}\n");
            Ok(())
        })
        .0
        .unwrap();
        assert!(source.len() < super::super::super::SOURCE_LIMIT);
        assert_eq!(super::super::super::SOURCE_LIMIT, 2 * 1024 * 1024);
        assert_eq!(
            source.matches("open spec fn byte_operation_55_").count(),
            count as usize
        );
        assert_eq!(
            source.matches(" as byte_operation_0_").count(),
            count as usize
        );
        for operation in 0..count {
            assert!(source.contains(&format!(
                "byte_operation_55_{operation}_v30 as byte_operation_0_{operation}_v30,"
            )));
            assert!(source.contains(&format!(
                "m.next_operation == {operation} && m.observations.len() == {operation}"
            )));
            assert!(source.contains(&format!("function: 0, block: 0, operation: {operation}")));
        }
        assert!(!source.contains("open spec fn byte_operation_0_"));
    });
}

#[test]
fn emitted_byte_functions_refuse_missing_duplicate_wrong_width_owner_and_census() {
    with_inventory(&memory_module(2), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let context = ByteContext::native(FormalIndexWidth::Bits64);
        run(floor, LIMIT, LIMIT, |out| {
            let mut emitted =
                EmittedByteFunctionsV55::new(inventory, physical, &allocations, context, out)?;
            let model = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                context,
                &allocations,
                out,
            )?;
            assert!(matches!(
                emitted.emit_parent_aliases(&model, 0, out),
                Err(Error::Statement(_))
            ));
            assert!(out.text.is_empty());
            emitted.emit(&model, 55, out)?;
            let length = out.text.len();
            assert!(matches!(
                emitted.emit(&model, 56, out),
                Err(Error::Statement(_))
            ));
            let second = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(1),
                context,
                &allocations,
                out,
            )?;
            for namespace in [54, 55] {
                assert!(matches!(
                    emitted.emit(&second, namespace, out),
                    Err(Error::Statement(_))
                ));
            }
            let wrong_width = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                ByteContext::native(FormalIndexWidth::Bits32),
                &allocations,
                out,
            )?;
            assert!(matches!(
                emitted.emit_parent_aliases(&wrong_width, 0, out),
                Err(Error::Statement(_))
            ));
            let mut truncated = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                context,
                &allocations,
                out,
            )?;
            truncated.operations.pop();
            assert!(matches!(
                emitted.emit_parent_aliases(&truncated, 0, out),
                Err(Error::Statement(_))
            ));
            assert_eq!(out.text.len(), length);
            emitted.emit(&second, 56, out)?;
            emitted.emit_parent_aliases(&model, 0, out)
        })
        .0
        .unwrap();
        with_inventory(
            &memory_module(2),
            |foreign, foreign_physical, foreign_floor| {
                run(floor + foreign_floor, LIMIT, LIMIT, |out| {
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
                    let foreign_allocations = NoAllocations(foreign.owner());
                    let foreign_model = ByteFunctionV30::derive(
                        foreign,
                        foreign_physical,
                        Function(0),
                        context,
                        &foreign_allocations,
                        out,
                    )?;
                    assert!(matches!(
                        emitted.emit_parent_aliases(&foreign_model, 0, out),
                        Err(Error::Statement(_))
                    ));
                    Ok(())
                })
                .0
                .unwrap();
            },
        );
    });
}

#[test]
fn emitted_byte_functions_refuse_substituted_view_registry_and_truncated_source() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        run(floor, LIMIT, LIMIT, |out| {
            let contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
            let other_contracts = Contracts::derive(inventory, FormalIndexWidth::Bits64, out)?;
            let context = ByteContext::classified(FormalIndexWidth::Bits64, &contracts, 9);
            let mut emitted =
                EmittedByteFunctionsV55::new(inventory, physical, &allocations, context, out)?;
            let model = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                context,
                &allocations,
                out,
            )?;
            emitted.emit(&model, 55, out)?;
            for context in [
                ByteContext::native(FormalIndexWidth::Bits64),
                ByteContext::classified(FormalIndexWidth::Bits64, &contracts, 10),
                ByteContext::classified(FormalIndexWidth::Bits64, &other_contracts, 9),
            ] {
                let substituted = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    context,
                    &allocations,
                    out,
                )?;
                assert!(matches!(
                    emitted.emit_parent_aliases(&substituted, 0, out),
                    Err(Error::Statement(_))
                ));
            }
            out.text.clear();
            assert!(matches!(
                emitted.emit_parent_aliases(&model, 0, out),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert!(matches!(
                emitted.emit_parent_aliases(&model, 0, out),
                Err(Error::Resource(Resource::Accounting))
            ));
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn emitted_byte_functions_refuse_substituted_or_mutated_allocation_recipes() {
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
    let mut module = Module::new("emitted-allocation-recipe-custody");
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
            let context = ByteContext::native(FormalIndexWidth::Bits64);
            let mut emitted =
                EmittedByteFunctionsV55::new(inventory, physical, &source, context, out)?;
            let model =
                ByteFunctionV30::derive(inventory, physical, Function(0), context, &source, out)?;
            emitted.emit(&model, 55, out)?;
            let wrapper = Sites {
                owner: inventory.owner(),
                site: Cell::new(original),
            };
            let matching =
                ByteFunctionV30::derive(inventory, physical, Function(0), context, &wrapper, out)?;
            emitted.emit_parent_aliases(&matching, 0, out)?;
            for change in 0..4 {
                let mut wrong = original;
                match change {
                    0 => wrong.physical_root_owner += 1,
                    1 => wrong.original.block.function.0 += 1,
                    2 => wrong.original.block.block += 1,
                    _ => wrong.original.operation += 1,
                }
                wrapper.site.set(wrong);
                let substituted = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    context,
                    &wrapper,
                    out,
                )?;
                assert!(matches!(
                    emitted.emit_parent_aliases(&substituted, 0, out),
                    Err(Error::Statement(_))
                ));
                // Even two matching *current* resolvers cannot substitute the
                // recipe captured in the already emitted operation body.
                source.site.set(wrong);
                assert!(matches!(
                    emitted.emit_parent_aliases(&substituted, 0, out),
                    Err(Error::Statement(_))
                ));
                source.site.set(original);
            }
            wrapper.site.set(original);
            emitted.emit_parent_aliases(&matching, 0, out)
        })
        .0
        .unwrap();
    });
}

#[test]
fn emitted_byte_functions_refuse_foreign_ledger_and_released_retention() {
    with_inventory(&memory_module(1), |inventory, physical, floor| {
        for foreign in [false, true] {
            let allocations = NoAllocations(inventory.owner());
            let context = ByteContext::native(FormalIndexWidth::Bits64);
            run(floor, LIMIT, LIMIT, |out| {
                let before = out.budget.storage();
                let mut emitted =
                    EmittedByteFunctionsV55::new(inventory, physical, &allocations, context, out)?;
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
                        emitted.emit_parent_aliases(&model, 0, &mut other),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                } else {
                    out.budget.release_storage(out.budget.storage() - before)?;
                }
                assert!(matches!(
                    emitted.emit_parent_aliases(&model, 0, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                Ok(())
            })
            .0
            .unwrap();
        }
    });
}
