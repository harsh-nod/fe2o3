use super::*;

#[path = "production_source_wrapping_value_v23_tests.rs"]
mod wrapping_value_v23;

#[path = "production_source_unary_expression_v39_tests.rs"]
mod unary_expression_v39;

fn with_private_expression_result_v24(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> ProductionOptimizerTestResultV18 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
    let result = with_production_optimizer_result_v18(prepared, &mut budget, consume);
    assert_eq!(budget.storage(), MODULE_FLOOR);
    result
}

fn private_expression_selected_resource_v24(
    result: &ProductionOptimizerTestResultV18,
) -> Option<ArgumentResourceV1> {
    match result {
        Err(ProductionSourceOptimizationErrorV18::Source(
            ProductionSourceOwnedViewErrorV18::Resource(error),
        ))
        | Err(ProductionSourceOptimizationErrorV18::Adoption(
            fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                ProductionSourceOwnedViewErrorV18::Resource(error),
            ),
        ))
        | Err(ProductionSourceOptimizationErrorV18::Adoption(
            fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
        )) => Some(*error),
        _ => None,
    }
}

fn private_cross_block_owner_v22() -> ProductionSemanticSsaOwnerV1 {
    let base = private_entry_neutral_owner_v20();
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let prior = &functions[3];
    let mut locals = prior.locals().to_vec();
    locals.push(local(164, U32, SemanticLocalRoleV1::Temporary));
    let mut first = prior.blocks()[0].statements()[..2].to_vec();
    first.push(assign(
        place(4, U32),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: SemanticOperandV1::Copy(place(3, U32)),
            right: literal(0),
        },
    ));
    functions[3] = function(
        150,
        SemanticFunctionRoleV1::InternalHelper,
        prior.abi().clone(),
        locals,
        vec![
            block(
                170,
                first,
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            block(
                171,
                vec![
                    assign(
                        place(1, U32),
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::BitXor,
                            left: SemanticOperandV1::Copy(place(4, U32)),
                            right: literal(0),
                        },
                    ),
                    assign(
                        place(3, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
                    ),
                    prior.blocks()[0].statements().last().unwrap().clone(),
                ],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let callables = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
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
fn private_source_writes_check_cross_block_arithmetic_and_read_after_nonentry_store() {
    for factory in [
        private_cross_block_owner_v22 as fn() -> _,
        private_entry_non_neutral_owner_v20,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let before = private_entry_typed_memory_census_v20(source.canonical(budget)?);
                assert!(before.into_iter().all(|n| n > 0));
                let output = source.private_completed_integer_output_v20(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                assert_eq!(
                    private_entry_typed_memory_census_v20(output.output(budget)?.owner()),
                    before
                );
                assert!(!output.output(budget)?.grants_authority());
                output.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok::<_, ProductionPrivateSourceHandoffErrorV20>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn private_typed_read_names_retain_distinct_original_occurrences_and_legacy_namespace() {
    let completed = std::cell::Cell::new(false);
    with_entry_fixture_v18(
        private_cross_block_owner_v22,
        |original, optimized, budget| {
            original.with_optimized_source_scalar_leaves_v18(
                optimized,
                0,
                budget,
                |legacy, budget| {
                    assert!(
                        legacy
                            .original_leaves(budget)?
                            .leaves
                            .rows
                            .iter()
                            .all(|row| !row.typed_private)
                    );
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )?;
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let original = leaves.original_leaves(budget)?;
                    let rows = &original.leaves.rows;
                    assert!(rows.iter().filter(|row| row.typed_private).count() >= 2);
                    let mut pairs = 0;
                    for (i, left) in rows.iter().enumerate() {
                        for right in &rows[i + 1..] {
                            if left.typed_private
                                && right.typed_private
                                && left.instance == right.instance
                            {
                                assert_ne!(left.operation, right.operation);
                                assert_ne!(left.symbol, right.symbol);
                                assert_ne!(left.place, right.place);
                                pairs += 1;
                            }
                        }
                    }
                    assert!(pairs > 0);
                    completed.set(true);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )
        },
    )
    .unwrap();
    assert!(completed.get());
}

#[test]
fn private_source_writes_refuse_wrong_read_name_and_arithmetic_before_completion() {
    for fault in 0..3 {
        let completed = std::cell::Cell::new(false);
        let result = with_entry_fixture_v18(
            private_cross_block_owner_v22,
            |original, optimized, budget| {
                let floor = budget.storage();
                scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
                    budget.reserve_storage(private_source_completion_headers_v20()?)?;
                    let index = OriginalEntryIndexV20::build(original, budget)?;
                    original.with_optimized_scalar_leaf_namespace_v18(
                        optimized,
                        0,
                        &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                        budget,
                        |leaves, budget| {
                            leaves.with_checked_write_profile_v22(
                                true,
                                budget,
                                |request, budget| {
                                    if !request.row.source_write {
                                        let expression =
                                            index.expression(leaves, request, budget)?;
                                        return request.check_expression(&expression, budget);
                                    }
                                    let mut expression = index
                                        .source_write_expression_v22(leaves, request, budget)?;
                                    let ProductionSemanticExpressionV2::Binary {
                                        operation,
                                        lhs,
                                        rhs,
                                        ..
                                    } = &mut expression
                                    else {
                                        panic!("genuine source Binary");
                                    };
                                    match fault {
                                        0 => {
                                            *rhs =
                                                Box::new(ProductionSemanticExpressionV2::Constant {
                                                    scalar: request.row.scalar,
                                                    bits: 3,
                                                })
                                        }
                                        1 => *operation = ProductionSemanticBinaryOpV2::Multiply,
                                        _ => {
                                            let rows = &leaves.original.leaves.rows;
                                            let other = rows
                                                .iter()
                                                .rev()
                                                .find(|row| {
                                                    row.typed_private
                                                        && row.instance == request.row.instance
                                                })
                                                .unwrap();
                                            *lhs =
                                                Box::new(ProductionSemanticExpressionV2::Symbol {
                                                    symbol: other.symbol,
                                                    scalar: other.scalar,
                                                });
                                        }
                                    }
                                    let error =
                                        request.check_expression(&expression, budget).unwrap_err();
                                    assert!(!request.completed.get());
                                    assert!(
                                        matches!(
                                            error,
                                            ProductionSourceOwnedViewErrorV18::Binding(
                                                "actual scalar expression differs from its original source value"
                                            )
                                        ),
                                        "{error:?}"
                                    );
                                    completed.set(true);
                                    Err(error)
                                },
                                |_, _| panic!("counterfeit source expression completed"),
                            )
                        },
                    )
                })
            },
        );
        assert!(result.is_err());
        assert!(completed.get(), "all counterfeit checks must actually run");
    }
}

struct IdentityWorkV22 {
    remaining: usize,
}
impl CorrelationChargeV18 for IdentityWorkV22 {
    fn charge_many(&mut self, amount: usize) -> Option<()> {
        self.remaining = self.remaining.checked_sub(amount)?;
        Some(())
    }
}

fn normalized_private_binary_v22(
    operation: ProductionSemanticBinaryOpV2,
    scalar: ProductionSemanticScalarTypeV2,
    overflow: ProductionOverflowContractV2,
    neutral: u64,
    constant_left: bool,
) -> NormalizedScalarExpressionV1 {
    let symbol = NormalizedScalarExpressionV1::Symbol { symbol: 7, scalar };
    let constant = NormalizedScalarExpressionV1::Constant {
        scalar,
        bits: neutral,
    };
    let (left, right) = if constant_left {
        (constant, symbol)
    } else {
        (symbol, constant)
    };
    NormalizedScalarExpressionV1::Binary {
        operation,
        scalar,
        overflow,
        lhs: NormalizedScalarNodeV18::legacy(left),
        rhs: NormalizedScalarNodeV18::legacy(right),
    }
}

#[test]
fn private_source_integer_identities_have_exact_work_and_no_checked_or_float_equivalence() {
    use ProductionSemanticBinaryOpV2 as Op;
    for bits in [8, 16, 32, 64] {
        for signed in [false, true] {
            let scalar = ProductionSemanticScalarTypeV2::Integer { bits, signed };
            for (operation, neutral) in [
                (Op::Add, 0),
                (Op::Subtract, 0),
                (Op::Multiply, 1),
                (Op::BitAnd, u64::MAX >> (64 - bits)),
                (Op::BitOr, 0),
                (Op::BitXor, 0),
            ] {
                for left in [false, true] {
                    let mut expression = normalized_private_binary_v22(
                        operation,
                        scalar,
                        ProductionOverflowContractV2::Wrapping,
                        neutral,
                        left,
                    );
                    let mut charge = IdentityWorkV22 { remaining: 24 };
                    assert_eq!(
                        source_private_integer_identity_v22(&mut expression, 0, &mut charge),
                        Some(())
                    );
                    assert_eq!(charge.remaining, 0);
                    assert_eq!(
                        matches!(
                            expression,
                            NormalizedScalarExpressionV1::Symbol { symbol: 7, .. }
                        ),
                        !(left && operation == Op::Subtract)
                    );
                    let mut short = normalized_private_binary_v22(
                        operation,
                        scalar,
                        ProductionOverflowContractV2::Wrapping,
                        neutral,
                        left,
                    );
                    assert_eq!(
                        source_private_integer_identity_v22(
                            &mut short,
                            0,
                            &mut IdentityWorkV22 { remaining: 23 }
                        ),
                        None
                    );
                    assert!(matches!(short, NormalizedScalarExpressionV1::Binary { .. }));
                    let mut checked = normalized_private_binary_v22(
                        operation,
                        scalar,
                        ProductionOverflowContractV2::Checked,
                        neutral,
                        left,
                    );
                    source_private_integer_identity_v22(
                        &mut checked,
                        0,
                        &mut IdentityWorkV22 { remaining: 24 },
                    )
                    .unwrap();
                    assert!(matches!(
                        checked,
                        NormalizedScalarExpressionV1::Binary { .. }
                    ));
                }
            }
        }
    }
    for scalar in [
        ProductionSemanticScalarTypeV2::Integer {
            bits: 7,
            signed: false,
        },
        ProductionSemanticScalarTypeV2::Float { bits: 32 },
    ] {
        let mut expression = normalized_private_binary_v22(
            Op::Add,
            scalar,
            ProductionOverflowContractV2::Wrapping,
            0,
            false,
        );
        source_private_integer_identity_v22(
            &mut expression,
            0,
            &mut IdentityWorkV22 { remaining: 24 },
        )
        .unwrap();
        assert!(matches!(
            expression,
            NormalizedScalarExpressionV1::Binary { .. }
        ));
    }
    assert_eq!(private_binary_v22(SemanticBinaryOpV1::Divide), None);
    assert_eq!(private_binary_v22(SemanticBinaryOpV1::ShiftLeft), None);
    assert_eq!(private_binary_v22(SemanticBinaryOpV1::Offset), None);
}

#[test]
fn private_typed_read_profile_rejects_volatile_nonprivate_and_zero_alignment() {
    for space in [
        AddressSpace::Private,
        AddressSpace::Global,
        AddressSpace::Workgroup,
        AddressSpace::Generic,
    ] {
        for volatile in [false, true] {
            for alignment in [0, 1, 4] {
                let kind = OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue {
                    address: ValueId(1),
                    access: MemoryAccess {
                        address_space: space,
                        alignment,
                        volatile,
                    },
                });
                assert_eq!(
                    source_scalar_read_kind_v22(&kind, true),
                    space == AddressSpace::Private && !volatile && alignment != 0
                );
                assert!(!source_scalar_read_kind_v22(&kind, false));
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ExpressionCutV22 {
    Measure,
    Work { needed: usize, short: bool },
    Storage { needed: usize, short: bool },
}

fn expression_boundary_v22(cut: ExpressionCutV22) -> (usize, usize) {
    expression_boundary_for_v39(private_cross_block_owner_v22, 4, cut)
}

fn expression_boundary_for_v39(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    child_boxes: usize,
    cut: ExpressionCutV22,
) -> (usize, usize) {
    let completed = std::cell::Cell::new(false);
    let observed = std::cell::Cell::new(None);
    let selected_resource = std::cell::Cell::new(None);
    let result = with_private_expression_result_v24(factory, |original, optimized, budget| {
        let floor = budget.storage();
        scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
            budget.reserve_storage(private_source_completion_headers_v20()?)?;
            let index = OriginalEntryIndexV20::build(original, budget)?;
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    leaves.with_checked_write_profile_v22(
                        true,
                        budget,
                        |request, budget| {
                            if !request.row.source_write {
                                let expression = index.expression(leaves, request, budget)?;
                                return request.check_expression(&expression, budget);
                            }
                            let floor = budget.storage();
                            match cut {
                                ExpressionCutV22::Measure => {}
                                ExpressionCutV22::Work { needed, short } => {
                                    let room = needed - usize::from(short);
                                    budget.charge_work(
                                        OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - room,
                                    )?;
                                }
                                ExpressionCutV22::Storage { needed, short } => {
                                    let room = needed - usize::from(short);
                                    budget.reserve_storage(MODULE_LIMIT - floor - room)?;
                                }
                            }
                            let start = (budget.work(), budget.storage());
                            let expression =
                                index.source_write_expression_v22(leaves, request, budget);
                            let used = (budget.work() - start.0, budget.storage() - start.1);
                            let selected = match (cut, expression) {
                                (ExpressionCutV22::Work { short: true, .. }, Err(error)) => {
                                    assert!(
                                        matches!(
                                            error,
                                            ProductionSourceOwnedViewErrorV18::Resource(
                                                ArgumentResourceV1::Work(_)
                                            )
                                        ),
                                        "{error:?}"
                                    );
                                    error
                                }
                                (ExpressionCutV22::Storage { short: true, .. }, Err(error)) => {
                                    assert!(
                                        matches!(
                                            error,
                                            ProductionSourceOwnedViewErrorV18::Resource(
                                                ArgumentResourceV1::Storage(_)
                                            )
                                        ),
                                        "{error:?}"
                                    );
                                    error
                                }
                                (
                                    ExpressionCutV22::Work { short: true, .. }
                                    | ExpressionCutV22::Storage { short: true, .. },
                                    Ok(_),
                                ) => {
                                    panic!("one-short expression budget accepted")
                                }
                                (_, Err(error)) => {
                                    panic!("exact expression refused: {error:?}")
                                }
                                (_, Ok(expression)) => {
                                    // Each fixture states its independently authored tree size.
                                    assert_eq!(
                                        used.1,
                                        child_boxes * size_of::<ProductionSemanticExpressionV2>()
                                    );
                                    observed.set(Some(used));
                                    drop(expression);
                                    ProductionSourceOwnedViewErrorV18::Binding(
                                        "test stops after exact source expression boundary",
                                    )
                                }
                            };
                            if let ProductionSourceOwnedViewErrorV18::Resource(error) = &selected {
                                match error {
                                    ArgumentResourceV1::Work(limit) => {
                                        assert_eq!(limit.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                                    }
                                    ArgumentResourceV1::Storage(limit) => {
                                        assert_eq!(limit.limit(), MODULE_LIMIT);
                                    }
                                    other => {
                                        panic!("unexpected expression resource: {other:?}")
                                    }
                                }
                                selected_resource.set(Some(*error));
                            }
                            // Test-owned padding and fully dropped expression scratch are
                            // retired before the containing real attempt settles.
                            budget.release_storage(budget.storage() - floor)?;
                            assert_eq!(budget.storage(), floor);
                            completed.set(true);
                            Err(selected)
                        },
                        |_, _| panic!("boundary probe cannot complete a write request"),
                    )
                },
            )
        })
    });
    assert!(
        completed.get(),
        "boundary assertions and cleanup must complete"
    );
    if let Some(selected) = selected_resource.get() {
        assert_eq!(
            private_expression_selected_resource_v24(&result),
            Some(selected),
            "{result:?}"
        );
    } else {
        assert!(
            matches!(
                result,
                Err(ProductionSourceOptimizationErrorV18::Adoption(
                    fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "test stops after exact source expression boundary"
                        )
                    )
                ))
            ),
            "{result:?}"
        );
    }
    observed.get().unwrap_or((0, 0))
}

#[test]
fn private_source_expression_keeps_exact_and_one_short_original_ledger_boundaries() {
    let measured = expression_boundary_v22(ExpressionCutV22::Measure);
    assert!(measured.0 > 0);
    assert_eq!(measured.1, 4 * size_of::<ProductionSemanticExpressionV2>());
    assert_eq!(
        expression_boundary_v22(ExpressionCutV22::Work {
            needed: measured.0,
            short: false,
        }),
        measured
    );
    assert_eq!(
        expression_boundary_v22(ExpressionCutV22::Storage {
            needed: measured.1,
            short: false,
        }),
        measured
    );
    expression_boundary_v22(ExpressionCutV22::Work {
        needed: measured.0,
        short: true,
    });
    expression_boundary_v22(ExpressionCutV22::Storage {
        needed: measured.1,
        short: true,
    });
}
