include!("production_source_entry_rhs_v18_tests.rs");
include!("production_source_scalar_constant_fold_v18_tests.rs");
include!("production_source_only_scalar_v18_tests.rs");

#[test]
fn actual_folded_store_uses_selected_output_value_not_first_descendant() {
    for (factory, retained_divide) in [
        (folding_source_owner_v18 as fn() -> _, false),
        (division_source_owner_v18 as fn() -> _, true),
    ] {
        run_production_optimized_consumer_v18(factory, |original, optimized, budget| {
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.with_optimized_scalar_leaves_v18(optimized, 0, &recipe, budget, |leaves, budget| {
            let mut folded = 0;
            leaves.visit_store_inputs(budget, |disposition, budget| {
                let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
                    else { return Ok::<_, ProductionSourceOwnedViewErrorV18>(()); };
                let (_, function) = request.original(budget)?;
                let _original_input = request.input_for(function, budget)?;
                let input = optimized.input_inventory(budget)?;
                let original_row = source_operation_row_v18(input, request.original.operation, budget)?;
                let source_use = &input.uses()[original_row.operands.start + 1];
                let source_definition = input.definitions()[source_use.definition].coordinate;
                let OptimizedDefinition::Result { operation: producer, result: 0 } = source_definition
                    else { return Ok(()); };
                let producer = source_operation_row_v18(input, producer, budget)?;
                let expected = if retained_divide { fe2o3_kernel_ir::BinaryOp::Divide } else {
                    fe2o3_kernel_ir::BinaryOp::Checked(fe2o3_kernel_ir::CheckedBinaryOperator::Add)
                };
                if !matches!(producer.operation.kind, OperationKind::Binary { op, .. } if op == expected) {
                    return Ok(());
                }
                let scalar = request.scalar(budget)?;
                request.check_expression(&ProductionSemanticExpressionV2::Constant { scalar, bits: 16 }, budget)?;
                let output = optimized.output_inventory(budget)?;
                let actual = source_operation_row_v18(output, request.output, budget)?;
                assert!(matches!(actual.operation.kind, OperationKind::Store { value, .. } if value == request.value));
                let input_use = OptimizedUse::OperationOperand { operation: request.original.operation, operand: 1 };
                let selected = optimized.operand(input_use, budget)?.expect("retained Store use");
                let selected_row = optimized_source_definition_row_v18(output, selected.definition, budget)?;
                assert_eq!(selected_row.value, Some(request.value));
                let descendants = optimized.definition_descendants(source_definition, budget)?;
                assert!(!descendants.is_empty());
                let mut selected_found = false;
                let mut retained = 0;
                let mut constants = 0;
                for descendant in descendants {
                    selected_found |= descendant.output == selected.definition;
                    let OptimizedDefinition::Result { operation, result: 0 } = descendant.output
                        else { panic!("folded value cannot select the overflow result"); };
                    match source_operation_row_v18(output, operation, budget)?.operation.kind {
                        OperationKind::Binary { op: fe2o3_kernel_ir::BinaryOp::Divide, .. } => retained += 1,
                        OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(16)) => constants += 1,
                        _ => panic!("folded scalar descendant changed physical value"),
                    }
                }
                assert!(selected_found);
                assert_eq!(retained, usize::from(retained_divide));
                assert!(constants > 0);
                assert!(matches!(optimized_source_definition_row_v18(output, selected.definition, budget)?.value,
                    Some(value) if value == request.value));
                assert!(matches!(source_operation_row_v18(output,
                    match selected.definition { OptimizedDefinition::Result { operation, result: 0 } => operation,
                        _ => panic!("Store selected a non-value result"), }, budget)?.operation.kind,
                    OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(16))));
                folded += 1;
                Ok(())
            })?;
            let input = optimized.input_inventory(budget)?;
            let mut expected = 0;
            for row in input.operations() {
                if row.coordinate.block.function.0 as usize != root { continue; }
                if !matches!(row.operation.kind, OperationKind::Store { .. }) { continue; }
                let value = &input.definitions()[input.uses()[row.operands.start + 1].definition];
                let OptimizedDefinition::Result { operation, result: 0 } = value.coordinate else { continue; };
                if matches!(source_operation_row_v18(input, operation, budget)?.operation.kind,
                    OperationKind::Binary { op: fe2o3_kernel_ir::BinaryOp::Divide
                        | fe2o3_kernel_ir::BinaryOp::Checked(fe2o3_kernel_ir::CheckedBinaryOperator::Add), .. }) {
                    expected += 1;
                }
            }
            assert!(expected > 0);
            assert_eq!(folded, expected, "every actual arithmetic Store occurrence is checked");
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
        });
    }
}

#[test]
fn output_scalar_read_keeps_original_occurrence_symbol_after_fixed_passes() {
    run_production_optimized_consumer_v18(
        scalar_read_store_owner_v18,
        |original, optimized, budget| {
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    let mut reads = 0;
                    leaves.visit_store_inputs(budget, |disposition, budget| {
                        let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) =
                            disposition
                        else {
                            return Ok::<_, ProductionSourceOwnedViewErrorV18>(());
                        };
                        if !matches!(
                            request.original.source,
                            ScopedMemoryStoreSourceV29::Operand {
                                source: ScopedMemoryOperandSourceV29::Memory { .. },
                                ..
                            }
                        ) {
                            return Ok(());
                        }
                        let (instance, function) = request.original(budget)?;
                        let ProductionSourceScalarInputV18::Operand {
                            operand: SemanticOperandV1::Copy(place),
                            ..
                        } = request.input_for(function, budget)?
                        else {
                            panic!("original read operand");
                        };
                        let expression = leaves
                            .original_leaves(budget)?
                            .original_place(instance, function, place, budget)?
                            .expect("authenticated original read symbol");
                        request.check_expression(&expression, budget)?;
                        reads += 1;
                        Ok(())
                    })?;
                    assert!(reads >= 2);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    );
}
#[test]
fn scalar_leaf_scopes_preserve_owned_success_error_and_panic_backing() {
    run_production_optimized_consumer_v18(
        folding_source_owner_v18,
        |original, optimized, budget| {
            let root = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
            let floor = budget.storage();
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    assert!(
                        leaves.visit_store_inputs(budget, |_, _| Ok::<
                            _,
                            ProductionSourceOwnedViewErrorV18,
                        >(()))?
                            > 0
                    );
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )?;
            assert_eq!(
                budget.storage(),
                floor,
                "same candidate passes before owned payload controls"
            );
            for mode in 0..3 {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    original.with_optimized_scalar_leaves_v18(
                        optimized,
                        0,
                        &recipe,
                        budget,
                        |_, budget| {
                            let payload = owned_callback_payload_v18(budget);
                            match mode {
                                0 => Ok(payload),
                                1 => Err(OwnedCallbackErrorV18::Payload(payload)),
                                _ => {
                                    budget
                                        .reserve_storage(std::mem::size_of::<Vec<u64>>())
                                        .unwrap();
                                    std::panic::resume_unwind(Box::new(payload))
                                }
                            }
                        },
                    )
                }));
                let backing = 64 * std::mem::size_of::<u64>()
                    + if mode == 2 {
                        std::mem::size_of::<Vec<u64>>()
                    } else {
                        0
                    };
                assert_eq!(budget.storage(), floor + backing);
                match (mode, caught) {
                    (0, Ok(Ok(payload)))
                    | (1, Ok(Err(OwnedCallbackErrorV18::Payload(payload)))) => {
                        assert_eq!(payload, vec![0x271; 64]);
                        drop(payload);
                    }
                    (2, Err(payload)) => {
                        let payload = payload.downcast::<Vec<u64>>().unwrap();
                        assert_eq!(*payload, vec![0x271; 64]);
                        drop(payload);
                    }
                    _ => panic!("scalar callback disposition changed"),
                }
                budget.release_storage(backing).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            Ok(())
        },
    );
}

#[test]
fn scalar_store_visitors_release_only_original_and_optimized_origin_caches() {
    for optimized_visit in [false, true] {
        run_production_optimized_consumer_v18(
            folding_source_owner_v18,
            |original, optimized, budget| {
                let root = original.source.root(0, budget)?.1;
                let recipe =
                    scalar_leaf_collision_recipe_v18(original.inventory.functions()[root].function);
                let floor = budget.storage();
                original.with_optimized_scalar_leaves_v18(
                    optimized,
                    0,
                    &recipe,
                    budget,
                    |leaves, budget| {
                        let count = if optimized_visit {
                            leaves.visit_store_inputs(budget, |_, _| {
                                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                            })?
                        } else {
                            leaves
                                .original_leaves(budget)?
                                .visit_store_inputs(budget, |_, _| {
                                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                                })?
                        };
                        assert!(count > 0);
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    },
                )?;
                assert_eq!(budget.storage(), floor);
                for panic in [false, true] {
                    let reached = std::cell::Cell::new(false);
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        original.with_optimized_scalar_leaves_v18(optimized, 0, &recipe, budget, |leaves, budget| {
                        let consume = |budget: &mut ArgumentBudgetV1<'_>| -> Result<(), OwnedCallbackErrorV18> {
                            assert!(!reached.replace(true), "visitor continued after selected error");
                            let payload = owned_callback_payload_v18(budget);
                            if panic {
                                budget.reserve_storage(std::mem::size_of::<Vec<u64>>()).unwrap();
                                std::panic::resume_unwind(Box::new(payload));
                            }
                            Err(OwnedCallbackErrorV18::Payload(payload))
                        };
                        if optimized_visit {
                            leaves.visit_store_inputs(budget, |_, budget| consume(budget))
                        } else {
                            leaves.original_leaves(budget)?.visit_store_inputs(budget, |_, budget| consume(budget))
                        }
                    })
                    }));
                    assert!(reached.get());
                    let backing = 64 * std::mem::size_of::<u64>()
                        + if panic {
                            std::mem::size_of::<Vec<u64>>()
                        } else {
                            0
                        };
                    assert_eq!(budget.storage(), floor + backing);
                    match (panic, caught) {
                        (false, Ok(Err(OwnedCallbackErrorV18::Payload(payload)))) => {
                            assert_eq!(payload, vec![0x271; 64]);
                            drop(payload);
                        }
                        (true, Err(payload)) => {
                            let payload = payload.downcast::<Vec<u64>>().unwrap();
                            assert_eq!(*payload, vec![0x271; 64]);
                            drop(payload);
                        }
                        _ => panic!("Store visitor changed selected owned payload"),
                    }
                    budget.release_storage(backing).unwrap();
                    assert_eq!(budget.storage(), floor);
                }
                Ok(())
            },
        );
    }
}
