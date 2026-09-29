use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticCheckedBinaryRvalueV1,
    SemanticLayoutIdentityV1, SemanticPaddingV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

#[test]
fn ordinary_wrapping_values_bind_both_actual_owners_without_relabeling_checked_flags() {
    for factory in [
        private_cross_block_owner_v22 as fn() -> _,
        private_entry_non_neutral_owner_v20,
    ] {
        let completed = std::cell::Cell::new(false);
        with_entry_fixture_v18(factory, |original, optimized, budget| {
            original.with_optimized_source_scalar_leaves_v18(
                optimized,
                0,
                budget,
                |legacy, _| {
                    assert!(legacy.original.leaves.wrapping.is_empty());
                    assert!(legacy.wrapping.is_empty());
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )?;
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let source = leaves.original.leaves;
                    let root = original.source.root(0, budget)?.1;
                    let function = original.inventory.functions()[root].function;
                    assert!(!source.wrapping.is_empty());
                    for row in &source.wrapping {
                        let actual =
                            source_operation_row_v18(original.inventory, row.operation, budget)?
                                .operation;
                        assert_eq!(
                            normalize_kir_binary_v1(
                                BinaryOp::Checked(row.operator),
                                actual,
                                row.value
                            )
                            .unwrap()
                            .1,
                            ProductionOverflowContractV2::Checked
                        );
                        assert_eq!(
                            source.wrapping_value_v23(
                                function,
                                row.value,
                                ProductionOverflowContractV2::Checked,
                                budget
                            )?,
                            ProductionOverflowContractV2::Wrapping
                        );
                        let flag = actual.results[1].id;
                        assert!(
                            normalize_kir_binary_v1(BinaryOp::Checked(row.operator), actual, flag)
                                .is_none()
                        );
                        assert_eq!(
                            source.wrapping_value_v23(
                                function,
                                flag,
                                ProductionOverflowContractV2::Checked,
                                budget
                            )?,
                            ProductionOverflowContractV2::Checked
                        );
                    }
                    let scratch = budget.storage();
                    let arguments = SourceRootArgumentsV18::build(original, 0, budget)?;
                    let normalizer = OptimizedSourceScalarNormalizationV18 {
                        leaves,
                        arguments: &arguments,
                    };
                    for row in leaves.wrapping {
                        assert_eq!(
                            normalizer.binary_overflow(
                                leaves.function.function,
                                row.value,
                                ProductionOverflowContractV2::Checked,
                                budget
                            )?,
                            ProductionOverflowContractV2::Wrapping
                        );
                        let output = leaves.optimized.output_inventory(budget)?;
                        let actual =
                            source_operation_row_v18(output, row.operation, budget)?.operation;
                        assert_eq!(
                            normalizer.binary_overflow(
                                leaves.function.function,
                                actual.results[1].id,
                                ProductionOverflowContractV2::Checked,
                                budget
                            )?,
                            ProductionOverflowContractV2::Checked
                        );
                    }
                    drop(normalizer);
                    drop(arguments);
                    budget.release_storage(budget.storage() - scratch)?;
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        })
        .unwrap();
        assert!(completed.get());
    }
}

#[test]
fn wrapping_value_source_contract_excludes_checked_tuple_and_noninteger_arithmetic() {
    let owner = private_entry_non_neutral_owner_v20();
    let types = owner.source_semantic().types();
    let expected = ProductionSemanticScalarTypeV2::Integer {
        bits: 32,
        signed: false,
    };
    for (operation, checked) in [
        (SemanticBinaryOpV1::Add, CheckedBinaryOperator::Add),
        (
            SemanticBinaryOpV1::Subtract,
            CheckedBinaryOperator::Subtract,
        ),
        (
            SemanticBinaryOpV1::Multiply,
            CheckedBinaryOperator::Multiply,
        ),
    ] {
        let statement = assign(
            place(1, U32),
            SemanticRvalueKindV1::Binary {
                operation,
                left: literal(7),
                right: literal(3),
            },
        );
        assert_eq!(
            source_wrapping_contract_v23(&statement, types),
            Some((checked, expected))
        );
    }
    for kind in [
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Add,
            literal(7),
            literal(3),
        )),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Divide,
            left: literal(7),
            right: literal(3),
        },
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::BitXor,
            left: literal(7),
            right: literal(3),
        },
        SemanticRvalueKindV1::Use(literal(7)),
    ] {
        // A classifier control, not an admitted source tuple. Genuine owner
        // tests above separately authenticate the real emitted value/flag.
        assert_eq!(
            source_wrapping_contract_v23(&assign(place(1, U32), kind), types),
            None
        );
    }
}

#[test]
fn wrapping_value_original_rows_reject_wrong_site_result_and_operator() {
    for fault in 0..5 {
        let completed = std::cell::Cell::new(false);
        let result = with_private_expression_result_v24(
            private_entry_non_neutral_owner_v20,
            |original, optimized, budget| {
                original.with_optimized_scalar_leaf_namespace_v18(optimized, 0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22, budget, |leaves, budget| {
                    let source = leaves.original.leaves;
                    let mut row = source.wrapping[0];
                    let reason = match fault {
                        0 => { row.statement = u32::MAX; "wrapping scalar source assignment differs" }
                        1 => { row.operator = CheckedBinaryOperator::Multiply; "wrapping scalar source assignment differs" }
                        2 => {
                            let actual = source_operation_row_v18(original.inventory, row.operation, budget)?.operation;
                            row.value = actual.results[1].id;
                            "wrapping scalar original occurrence differs"
                        }
                        3 => { row.operation.operation = u32::MAX; "wrapping scalar original occurrence differs" }
                        _ => {
                            let actual = original.inventory.operations().iter().find(|other|
                                other.coordinate.block.function != row.operation.block.function
                                && source_wrapping_result_v23(other.operation, row.operator, row.scalar).is_some()).unwrap();
                            row.operation = actual.coordinate;
                            row.value = actual.operation.results[0].id;
                            "wrapping scalar original occurrence differs"
                        }
                    };
                    // Check membership before actual-operation lookup for a
                    // forged coordinate, so no unrelated inventory refusal wins.
                    let error = source.check_wrapping_row_v23(&row, budget).unwrap_err();
                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(actual) if actual == reason), "{error:?}");
                    completed.set(true);
                    Err::<(), _>(error)
                })
            },
        );
        assert!(completed.get());
        assert!(result.is_err());
    }
}

#[test]
fn wrapping_value_overflow_edges_keep_checked_contract_distinct_for_every_fixed_width() {
    use ProductionSemanticBinaryOpV2 as Binary;
    for bits in [8, 16, 32, 64] {
        for signed in [false, true] {
            let scalar = ProductionSemanticScalarTypeV2::Integer { bits, signed };
            let mask = u64::MAX >> (64 - bits);
            let maximum = if signed { mask >> 1 } else { mask };
            let minimum = if signed { 1u64 << (bits - 1) } else { 0 };
            for (operation, left, right, expected) in [
                (Binary::Add, maximum, 1, maximum.wrapping_add(1) & mask),
                (Binary::Subtract, minimum, 1, minimum.wrapping_sub(1) & mask),
                (Binary::Multiply, maximum, 2, maximum.wrapping_mul(2) & mask),
            ] {
                let lhs = NormalizedScalarExpressionV1::Constant { scalar, bits: left };
                let rhs = NormalizedScalarExpressionV1::Constant {
                    scalar,
                    bits: right,
                };
                assert_eq!(
                    source_scalar_binary_constant_v18(
                        operation,
                        scalar,
                        ProductionOverflowContractV2::Wrapping,
                        &lhs,
                        &rhs
                    ),
                    Some(expected)
                );
                assert_eq!(
                    source_scalar_binary_constant_v18(
                        operation,
                        scalar,
                        ProductionOverflowContractV2::Checked,
                        &lhs,
                        &rhs
                    ),
                    None
                );
            }
        }
    }
}

fn explicit_checked_owner_v23() -> ProductionSemanticSsaOwnerV1 {
    let base = private_entry_non_neutral_owner_v20();
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([241; 32]),
        SemanticLayoutIdentityV1::from_sha256([241; 32]),
        SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let pair = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([242; 32]),
        SemanticLayoutIdentityV1::from_sha256([242; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, boolean]).unwrap()),
    ));
    let mut functions = semantic.functions().to_vec();
    let old = &functions[3];
    let mut locals = old.locals().to_vec();
    let output = locals.len() as u32;
    locals.push(local(164, pair, SemanticLocalRoleV1::Temporary));
    let mut statements = old.blocks()[0].statements().to_vec();
    let unit = statements.pop().unwrap();
    statements.push(assign(
        place(output, pair),
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Add,
            SemanticOperandV1::Copy(place(3, U32)),
            literal(1),
        )),
    ));
    statements.push(unit);
    functions[3] = function(
        150,
        SemanticFunctionRoleV1::InternalHelper,
        old.abi().clone(),
        locals,
        vec![block(170, statements, SemanticTerminatorKindV1::Return)],
    );
    let callables = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn explicit_checked_source_tuple_and_both_results_never_enter_wrapping_index() {
    let completed = std::cell::Cell::new(false);
    with_entry_fixture_v18(explicit_checked_owner_v23, |original, optimized, budget| {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
            budget,
            |leaves, budget| {
                let source = leaves.original.leaves;
                let mut seen = 0;
                for instance in &original.source.root_row(0)?.coordinates.sources.rows {
                    if instance.function.index() != 3 {
                        continue;
                    }
                    original.visit_source_operations(
                        0,
                        instance.instance.index(),
                        SemanticBlockIdV1::from_index(0),
                        Some(3),
                        budget,
                        |mapped, budget| {
                            if let ProductionSourceOperationV18::Operation(coordinate) = mapped {
                                let actual = source_operation_row_v18(
                                    original.inventory,
                                    coordinate,
                                    budget,
                                )?
                                .operation;
                                if matches!(
                                    actual.kind,
                                    OperationKind::Binary {
                                        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                                        ..
                                    }
                                ) {
                                    assert_eq!(actual.results.len(), 2);
                                    let function = original.inventory.functions()
                                        [coordinate.block.function.0 as usize]
                                        .function;
                                    for result in &actual.results {
                                        assert_eq!(
                                            source.wrapping_value_v23(
                                                function,
                                                result.id,
                                                ProductionOverflowContractV2::Checked,
                                                budget
                                            )?,
                                            ProductionOverflowContractV2::Checked
                                        );
                                    }
                                    assert!(
                                        source
                                            .wrapping
                                            .iter()
                                            .all(|row| row.operation != coordinate)
                                    );
                                    seen += 1;
                                }
                            }
                            Ok(())
                        },
                    )?;
                }
                assert_eq!(seen, 1);
                completed.set(true);
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            },
        )
    })
    .unwrap();
    assert!(completed.get());
}

#[test]
fn wrapping_value_result_shape_never_grants_flag_or_counterfeit_checked_contract() {
    let completed = std::cell::Cell::new(false);
    with_entry_fixture_v18(
        private_entry_non_neutral_owner_v20,
        |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let source = leaves.original.leaves;
                    let row = source.wrapping[0];
                    let actual =
                        source_operation_row_v18(original.inventory, row.operation, budget)?
                            .operation;
                    for fault in 0..6 {
                        let mut other = actual.clone();
                        match fault {
                            0 => other.results.swap(0, 1),
                            1 => other.results[1].ty = Type::Scalar(ScalarType::U32),
                            2 => other.results[0].ty = Type::Scalar(ScalarType::F32),
                            3 => other.results[1].id = other.results[0].id,
                            4 => {
                                other.results.pop();
                            }
                            _ => {
                                let OperationKind::Binary { op, .. } = &mut other.kind else {
                                    unreachable!()
                                };
                                *op = BinaryOp::Add;
                            }
                        }
                        assert_eq!(
                            source_wrapping_result_v23(&other, row.operator, row.scalar),
                            None
                        );
                    }
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .unwrap();
    assert!(completed.get());
}

fn wrapping_query_cut_v23(cut: Option<(bool, usize, bool)>) -> usize {
    let completed = std::cell::Cell::new(false);
    let measured = std::cell::Cell::new(0);
    let selected = std::cell::Cell::new(None);
    let result = with_private_expression_result_v24(
        private_entry_non_neutral_owner_v20,
        |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let source = leaves.original.leaves;
                    let row = source.wrapping[0];
                    let function =
                        original.inventory.functions()[original.source.root(0, budget)?.1].function;
                    let floor = budget.storage();
                    if let Some((storage, needed, short)) = cut {
                        let room = needed - usize::from(short);
                        if storage {
                            budget.reserve_storage(MODULE_LIMIT - floor - room)?;
                        } else {
                            budget.charge_work(
                                OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - room,
                            )?;
                        }
                    }
                    let start = budget.work();
                    let value = source.wrapping_value_v23(
                        function,
                        row.value,
                        ProductionOverflowContractV2::Checked,
                        budget,
                    );
                    measured.set(budget.work() - start);
                    let error = match (cut, value) {
                        (
                            Some((storage, _, true)),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(error)),
                        ) => {
                            match error {
                                ArgumentResourceV1::Storage(limit) if storage => {
                                    assert_eq!(limit.limit(), MODULE_LIMIT)
                                }
                                ArgumentResourceV1::Work(limit) if !storage => {
                                    assert_eq!(limit.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18)
                                }
                                _ => panic!("wrong query boundary: {error:?}"),
                            }
                            selected.set(Some(error));
                            ProductionSourceOwnedViewErrorV18::Resource(error)
                        }
                        (Some((_, _, true)), other) => {
                            panic!("one-short query accepted: {other:?}")
                        }
                        (_, Ok(value)) => {
                            assert_eq!(value, ProductionOverflowContractV2::Wrapping);
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "test stops after wrapping query",
                            )
                        }
                        (_, Err(error)) => panic!("wrapping query refused: {error:?}"),
                    };
                    budget.release_storage(budget.storage() - floor)?;
                    assert_eq!(budget.storage(), floor);
                    completed.set(true);
                    Err::<(), _>(error)
                },
            )
        },
    );
    assert!(completed.get());
    if let Some(error) = selected.get() {
        assert_eq!(
            private_expression_selected_resource_v24(&result),
            Some(error),
            "{result:?}"
        );
    } else {
        assert!(matches!(
            result,
            Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                    ProductionSourceOwnedViewErrorV18::Binding("test stops after wrapping query")
                )
            ))
        ));
    }
    measured.get()
}

#[test]
fn wrapping_value_exact_and_one_short_query_work_and_visitor_header() {
    let work = wrapping_query_cut_v23(None);
    assert!(work > 0);
    wrapping_query_cut_v23(Some((false, work, false)));
    wrapping_query_cut_v23(Some((false, work, true)));
    // check_wrapping_row's callback captures two shared references: the row
    // and mutable found count. The actual closure is measured in production.
    let header = 2 * size_of::<usize>();
    wrapping_query_cut_v23(Some((true, header, false)));
    wrapping_query_cut_v23(Some((true, header, true)));
}
