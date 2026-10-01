fn descriptor_length_leaf_run_v40(
    metadata: bool,
    same_parameter: bool,
    fault: u8,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(self.0);
        }
    }
    let _restore = Restore(DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.replace(None));
    let owner = descriptor_length_only_owner_v30(metadata, same_parameter);
    let abi = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
    let reached = std::cell::Cell::new(false);
    let (result, work, peak) = run_descriptor_role_owner_with_abi_v18(
        owner,
        abi,
        work,
        storage,
        |original, optimized, budget| {
            let floor = budget.storage();
            let result = original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let source = leaves.original_leaves(budget)?;
                    let function = source.original_function(0, budget)?;
                    let statements = function.blocks()[0].statements();
                    let mut symbols = Vec::new();
                    source_scalar_normalization_scratch_v18(
                        original.source.cleanup,
                        budget,
                        0,
                        |budget| {
                            let index = OriginalEntryIndexV20::build(original, budget)?;
                            for statement in 0..2u32 {
                                let SemanticStatementKindV1::Assign(assignment) =
                                    statements[statement as usize].kind()
                                else {
                                    panic!("genuine descriptor metadata assignment");
                                };
                                let scalar = ProductionSemanticScalarTypeV2::Integer {
                                    signed: false,
                                    bits: 64,
                                };
                                let ty = assignment.value().result_type();
                                let mut remaining =
                                    fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                                let expression = index.private_expression_v22(
                                    source,
                                    0,
                                    ty,
                                    scalar,
                                    OriginalPrivateInputV22::Rvalue {
                                        block: 0,
                                        statement,
                                        value: assignment.value(),
                                    },
                                    0,
                                    &mut remaining,
                                    budget,
                                )?;
                                let ProductionSemanticExpressionV2::Symbol {
                                    symbol,
                                    scalar: actual,
                                } = expression
                                else {
                                    panic!("descriptor length must have an original-owned name");
                                };
                                assert_eq!(actual, scalar);
                                assert!(symbol < PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2);
                                assert!(
                                    source
                                        .leaves
                                        .descriptor_length_symbol_v40(symbol, scalar, budget)?
                                );
                                symbols.push(symbol);
                                // Metadata's source u64 result is the exact Index
                                // endpoint of SliceLength, not a scalar-program row.
                                let (locator, archived) = original.assignment_result_row_v30(
                                    0,
                                    0,
                                    SemanticBlockIdV1::from_index(0),
                                    statement,
                                    budget,
                                )?;
                                assert!(std::ptr::eq(archived, assignment));
                                let SourceRvalueEndpointV30::Scalar {
                                    value,
                                    scalar: ScalarType::Index,
                                } = locator.endpoint
                                else {
                                    panic!("exact descriptor length endpoint");
                                };
                                let physical = original.source.root(0, budget)?.1;
                                let input = original
                                    .inventory
                                    .definition_for_value(
                                        original.inventory.functions()[physical].coordinate,
                                        value,
                                        budget,
                                    )
                                    .map_err(source_pointer_inventory_error_v18)?
                                    .unwrap();
                                assert_eq!(input.ty, &Type::INDEX);
                                let SliceDefinition::Result {
                                    operation,
                                    result: 0,
                                } = input.coordinate
                                else {
                                    panic!("exact metadata result coordinate");
                                };
                                let operation = source_operation_row_v18(
                                    original.inventory,
                                    operation,
                                    budget,
                                )?
                                .operation;
                                let OperationKind::SliceLength { slice: receiver } = operation.kind
                                else {
                                    panic!("exact metadata operation");
                                };
                                assert_eq!(operation.results.len(), 1);
                                let receiver_type = original
                                    .inventory
                                    .definition_for_value(
                                        original.inventory.functions()[physical].coordinate,
                                        receiver,
                                        budget,
                                    )
                                    .map_err(source_pointer_inventory_error_v18)?
                                    .unwrap()
                                    .ty;
                                assert!(matches!(receiver_type, Type::Slice(slice)
                                    if slice.element.as_ref() == &Type::Scalar(ScalarType::U32)));
                                original
                                    .check_descriptor_operand_v30(0, locator, receiver, budget)?;
                                let expected =
                                    Some(NormalizedScalarExpressionV1::Symbol { symbol, scalar });
                                assert_eq!(
                                    source.leaves.descriptor_length_value_v40(
                                        input.value.unwrap(),
                                        budget
                                    )?,
                                    expected
                                );
                                let descendants =
                                    optimized.definition_descendants(input.coordinate, budget)?;
                                assert!(!descendants.is_empty(), "live dynamic length");
                                for descendant in descendants {
                                    let actual = optimized_source_definition_row_v18(
                                        optimized.output_inventory(budget)?,
                                        descendant.output,
                                        budget,
                                    )?
                                    .value
                                    .unwrap();
                                    assert_eq!(
                                        leaves.read(leaves.function.function, actual, budget)?,
                                        expected
                                    );
                                }
                            }
                            drop(index);
                            Ok(())
                        },
                    )?;
                    assert_eq!(symbols[0] == symbols[1], same_parameter);
                    reached.set(true);
                    let SemanticStatementKindV1::Assign(assignment) = statements[0].kind() else {
                        unreachable!();
                    };
                    let scalar = ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 64,
                    };
                    match fault {
                        0 => Ok(()),
                        1 => source
                            .leaves
                            .descriptor_length_expression_v40(
                                0,
                                0,
                                0,
                                &assignment.value().clone(),
                                scalar,
                                budget,
                            )
                            .map(drop),
                        2 => source
                            .leaves
                            .descriptor_length_expression_v40(
                                0,
                                0,
                                1,
                                assignment.value(),
                                scalar,
                                budget,
                            )
                            .map(drop),
                        3 => source
                            .leaves
                            .descriptor_length_expression_v40(
                                0,
                                0,
                                0,
                                assignment.value(),
                                ProductionSemanticScalarTypeV2::Bool,
                                budget,
                            )
                            .map(drop),
                        4 => source
                            .leaves
                            .descriptor_length_expression_v40(
                                1,
                                0,
                                0,
                                assignment.value(),
                                scalar,
                                budget,
                            )
                            .map(drop),
                        5 => {
                            let before = (budget.work(), budget.storage());
                            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(work);
                            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, storage);
                            let refused = source.leaves.descriptor_length_symbol_v40(
                                symbols[0],
                                scalar,
                                &mut foreign,
                            );
                            assert!(matches!(
                                refused,
                                Err(ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting
                                ))
                            ));
                            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                            let retry = source
                                .leaves
                                .descriptor_length_symbol_v40(symbols[0], scalar, budget);
                            assert!(matches!(
                                retry,
                                Err(ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Accounting
                                ))
                            ));
                            assert_eq!((budget.work(), budget.storage()), before);
                            retry.map(drop)
                        }
                        _ => unreachable!(),
                    }
                },
            );
            if original.source.cleanup.is_denied() {
                DESCRIPTOR_ROLE_RETAINED_FLOOR_V18.set(Some(budget.storage()));
            } else {
                assert_eq!(budget.storage(), floor);
            }
            result
        },
    );
    (result, work, peak, reached.get())
}

#[test]
fn descriptor_length_scalar_leaves_bind_both_original_forms_and_exact_optimized_parameters() {
    for metadata in [false, true] {
        for same in [false, true] {
            let (result, _, _, reached) = descriptor_length_leaf_run_v40(
                metadata,
                same,
                0,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
            );
            result.unwrap();
            assert!(reached);
        }
    }
}

#[test]
fn descriptor_length_scalar_leaves_reject_cloned_rvalue_wrong_site_type_and_instance() {
    for metadata in [false, true] {
        for fault in 1..=4 {
            let (result, _, _, reached) = descriptor_length_leaf_run_v40(
                metadata,
                false,
                fault,
                OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                MODULE_LIMIT,
            );
            assert!(
                reached,
                "original leaf admission must precede the negative query"
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(_))
            ));
        }
    }
}

#[test]
fn descriptor_length_scalar_leaves_reject_foreign_budget_and_latch_retry() {
    let (result, _, _, reached) = descriptor_length_leaf_run_v40(
        true,
        false,
        5,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    assert!(reached);
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
}

#[test]
fn descriptor_length_scalar_leaves_have_exact_and_one_short_whole_resources() {
    let run = |work, storage| descriptor_length_leaf_run_v40(true, false, 0, work, storage);
    let (result, work, storage, reached) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(reached);
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, storage, true));
    for (work_limit, storage_limit, is_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (result, accepted_work, accepted_storage, _) = run(work_limit, storage_limit);
        match (
            is_work,
            source_slot_tests::original_repeated_source_resource_v29(result.unwrap_err()),
        ) {
            (true, ArgumentResourceV1::Work(error)) => {
                assert_eq!(error.limit(), work_limit);
                assert_eq!(error.actual(), work);
            }
            (false, ArgumentResourceV1::Storage(error)) => {
                assert_eq!(error.limit(), storage_limit);
                assert_eq!(error.actual(), storage);
            }
            other => panic!("exact descriptor leaf resource refusal: {other:?}"),
        }
        assert!(accepted_work <= work_limit && accepted_storage <= storage_limit);
    }
}
