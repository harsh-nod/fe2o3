fn typed_entry_rhs_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    typed_entry_rhs_fixture_v18(false)
}

fn typed_root_entry_rhs_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    typed_entry_rhs_fixture_v18(true)
}

fn typed_entry_rhs_fixture_v18(root_backing: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = module_fixture_owner(ModuleFixture::Ordinary);
    let semantic = base.source_semantic();
    // The ordinary fixture's assertion-only Bool is outside this root closure.
    let mut types = semantic.types()[..2].to_vec();
    let pointer = reference(&mut types, U32, SemanticMutabilityV1::Immutable, true);
    let captured = || {
        vec![
            assign(
                place(2, pointer),
                SemanticRvalueKindV1::AddressOf {
                    place: place(1, U32),
                    mutability: SemanticMutabilityV1::Immutable,
                },
            ),
            assign(
                place(3, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
            ),
            assign(
                place(0, UNIT),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ),
        ]
    };
    let captured_locals = || {
        vec![
            local(70, UNIT, SemanticLocalRoleV1::Return),
            local(71, U32, SemanticLocalRoleV1::Argument(0)),
            local(72, pointer, SemanticLocalRoleV1::Temporary),
            local(73, U32, SemanticLocalRoleV1::Temporary),
        ]
    };
    let call = |argument, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![argument],
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let (locals, blocks) = if root_backing {
        (
            captured_locals(),
            vec![block(80, captured(), SemanticTerminatorKindV1::Return)],
        )
    } else {
        (
            vec![
                local(70, UNIT, SemanticLocalRoleV1::Return),
                local(71, U32, SemanticLocalRoleV1::Argument(0)),
                local(72, U32, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    80,
                    vec![assign(
                        place(2, U32),
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Add,
                            left: literal(7),
                            right: literal(9),
                        },
                    )],
                    call(SemanticOperandV1::Copy(place(1, U32)), 1),
                ),
                block(81, vec![], call(SemanticOperandV1::Move(place(2, U32)), 2)),
                block(82, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
    };
    let root = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        abi(61, true, &[U32]),
        locals,
        blocks,
    )
    .with_kernel_entry(semantic.functions()[0].kernel_entry().unwrap().clone());
    let mut functions = vec![root];
    if !root_backing {
        functions.push(function(
            100,
            SemanticFunctionRoleV1::InternalHelper,
            abi(101, false, &[U32]),
            captured_locals(),
            vec![block(110, captured(), SemanticTerminatorKindV1::Return)],
        ));
    }
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
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

// Test-only reconstruction of this fixture's source expression. The production
// bridge uses SourceScalarInstanceV18 and the existing full source resolver.
fn fixture_entry_expression_v18(
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    request: &ProductionSourceEntryWriteV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
    let (instance, function) = request.original(budget)?;
    let ProductionSourceScalarInputV18::EntryArgument { argument } =
        request.input_for(function, budget)?
    else {
        panic!("only exact whole entry requests");
    };
    let scalar = request.scalar(budget)?;
    let argument = match leaves
        .original_leaves(budget)?
        .original_argument(instance, function, argument, budget)?
    {
        ProductionSourceScalarArgumentV18::Root { argument } => argument,
        ProductionSourceScalarArgumentV18::Caller {
            instance: caller,
            function,
            block,
            operand,
        } => {
            assert_eq!(caller, 0);
            let SemanticTerminatorKindV1::Call(call) = function.blocks()[block.index() as usize]
                .terminator()
                .kind()
            else {
                panic!("exact original helper call");
            };
            assert!(std::ptr::eq(operand, &call.arguments()[0]));
            match operand {
                SemanticOperandV1::Copy(place) => {
                    assert_eq!(place.local().index(), 1);
                    let SemanticLocalRoleV1::Argument(argument) = function.locals()[1].role()
                    else {
                        panic!("original root scalar argument");
                    };
                    argument
                }
                SemanticOperandV1::Move(place) => {
                    assert_eq!(place.local().index(), 2);
                    let SemanticStatementKindV1::Assign(assignment) =
                        function.blocks()[0].statements()[0].kind()
                    else {
                        panic!("original expression assignment");
                    };
                    let SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left,
                        right,
                    } = assignment.value().kind()
                    else {
                        panic!("original 7 + 9 expression");
                    };
                    assert_eq!(left, &literal(7));
                    assert_eq!(right, &literal(9));
                    return Ok(ProductionSemanticExpressionV2::Constant { scalar, bits: 16 });
                }
                _ => panic!("fixture original Copy/Move only"),
            }
        }
    };
    Ok(ProductionSemanticExpressionV2::Symbol {
        symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 + argument,
        scalar,
    })
}

fn check_fixture_entry_rhs_v18(
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    request: &ProductionSourceEntryWriteV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let expression = fixture_entry_expression_v18(leaves, request, budget)?;
    request.check_expression(&expression, budget)
}

fn with_entry_fixture_v18(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    consume: impl for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(factory, &mut budget);
    let result = with_production_optimized_consumer_v18(prepared, &mut budget, consume);
    assert_eq!(budget.storage(), MODULE_FLOOR);
    result
}

#[test]
fn typed_entry_rhs_genuine_root_and_distinct_helper_copy_move_expression() {
    for (factory, expected) in [
        (typed_root_entry_rhs_owner_v18 as fn() -> _, 1),
        (typed_entry_rhs_owner_v18 as fn() -> _, 2),
    ] {
        with_entry_fixture_v18(factory, |original, optimized, budget| {
            let physical = original.source.root(0, budget)?.1;
            let recipe =
                scalar_leaf_collision_recipe_v18(original.inventory.functions()[physical].function);
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    let floor = budget.storage();
                    let visited = std::cell::Cell::new(0);
                    leaves.with_checked_entry_writes_v18(
                        budget,
                        |request, budget| {
                            visited.set(visited.get() + 1);
                            check_fixture_entry_rhs_v18(leaves, request, budget)
                        },
                        |checked, budget| {
                            checked.check_for(original, optimized, 0, budget)?;
                            assert_eq!(checked.rows.len(), expected);
                            assert_eq!(visited.get(), expected);
                            for row in checked.rows {
                                checked.require(
                                    row.instance,
                                    row.anchor,
                                    row.input,
                                    row.output,
                                    row.input_rhs,
                                    row.output_rhs,
                                    row.scalar,
                                    row.schema,
                                    budget,
                                )?;
                            }
                            if expected == 2 {
                                assert_ne!(checked.rows[0].instance, checked.rows[1].instance);
                                assert_ne!(checked.rows[0].input_rhs, checked.rows[1].input_rhs);
                            }
                            let output = optimized.output_inventory(budget)?;
                            assert!(
                                output.operations().iter().any(|row| matches!(
                                    row.operation.kind,
                                    OperationKind::Storage(
                                        ScopedObjectOperationV29::ReadValue { .. }
                                    )
                                )),
                                "actual typed read survives; entry writes are not a vacuous census"
                            );
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        },
                    )?;
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
        })
        .unwrap();
    }
}

#[test]
fn typed_entry_rhs_callback_success_cannot_skip_endpoint_checks() {
    let entered = std::cell::Cell::new(false);
    let result =
        with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
            let function =
                original.inventory.functions()[original.source.root(0, budget)?.1].function;
            let recipe = scalar_leaf_collision_recipe_v18(function);
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    leaves.with_checked_entry_writes_v18(
                        budget,
                        |_, _| {
                            entered.set(true);
                            Ok(())
                        },
                        |_, _| panic!("unchecked RHS roster escaped"),
                    )
                },
            )
        });
    assert!(entered.get());
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "typed entry request did not check both RHS endpoints"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn typed_entry_rhs_wrong_source_expression_never_completes() {
    let result =
        with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
            let function =
                original.inventory.functions()[original.source.root(0, budget)?.1].function;
            let recipe = scalar_leaf_collision_recipe_v18(function);
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    leaves.with_checked_entry_writes_v18(
                        budget,
                        |request, budget| {
                            check_fixture_entry_rhs_v18(leaves, request, budget)?;
                            // The first invocation receives a root-dependent value, not
                            // the independently authenticated second call's constant RHS.
                            request.check_expression(
                                &ProductionSemanticExpressionV2::Constant {
                                    scalar: request.scalar(budget)?,
                                    bits: 16,
                                },
                                budget,
                            )
                        },
                        |_, _| panic!("wrong caller expression admitted"),
                    )
                },
            )
        });
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "actual scalar expression differs from its original source value"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn typed_entry_rhs_context_rejects_each_same_typed_coordinate_substitution() {
    for fault in 0..8 {
        let result =
            with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
                let function =
                    original.inventory.functions()[original.source.root(0, budget)?.1].function;
                let recipe = scalar_leaf_collision_recipe_v18(function);
                original.with_optimized_scalar_leaves_v18(
                    optimized,
                    0,
                    &recipe,
                    budget,
                    |leaves, budget| {
                        leaves.with_checked_entry_writes_v18(
                            budget,
                            |request, budget| check_fixture_entry_rhs_v18(leaves, request, budget),
                            |checked, budget| {
                                let mut row = checked.rows[0];
                                let other = checked.rows[1];
                                match fault {
                                    0 => row.instance = other.instance,
                                    1 => row.anchor = usize::MAX,
                                    2 => row.input = other.input,
                                    3 => row.output = other.output,
                                    4 => row.input_rhs = other.input_rhs,
                                    5 => row.output_rhs = other.output_rhs,
                                    6 => {
                                        row.scalar = ProductionSemanticScalarTypeV2::Integer {
                                            signed: false,
                                            bits: 64,
                                        }
                                    }
                                    _ => row.schema = fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX),
                                }
                                checked.require(
                                    row.instance,
                                    row.anchor,
                                    row.input,
                                    row.output,
                                    row.input_rhs,
                                    row.output_rhs,
                                    row.scalar,
                                    row.schema,
                                    budget,
                                )
                            },
                        )
                    },
                )
            });
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "typed entry RHS has no exact completed request"
                ))
            ),
            "fault {fault}: {result:?}"
        );
    }
}

#[test]
fn typed_entry_rhs_request_rejects_same_content_foreign_function() {
    let result =
        with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
            let function =
                original.inventory.functions()[original.source.root(0, budget)?.1].function;
            let recipe = scalar_leaf_collision_recipe_v18(function);
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    leaves.with_checked_entry_writes_v18(
                        budget,
                        |request, budget| {
                            let (_, function) = request.original(budget)?;
                            let substituted = function.clone();
                            request.input_for(&substituted, budget).map(|_| ())
                        },
                        |_, _| panic!("foreign declaration admitted"),
                    )
                },
            )
        });
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "typed entry resolver substituted original declaration"
            ))
        ),
        "{result:?}"
    );
}

#[test]
fn typed_entry_rhs_each_endpoint_rejects_another_invocations_same_typed_rhs() {
    for output_endpoint in [false, true] {
        let entered = std::cell::Cell::new(false);
        let result =
            with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
                let function =
                    original.inventory.functions()[original.source.root(0, budget)?.1].function;
                let recipe = scalar_leaf_collision_recipe_v18(function);
                original.with_optimized_scalar_leaves_v18(
                    optimized,
                    0,
                    &recipe,
                    budget,
                    |leaves, budget| {
                        let mut second = None;
                        visit_optimized_source_objects_v18(
                            original,
                            optimized,
                            0,
                            budget,
                            |row, budget| {
                                if let Some(row) =
                                    source_entry_write_row_v18(original, 0, &row, budget)?
                                {
                                    if row.instance == 2 {
                                        second = Some(row);
                                    }
                                }
                                Ok(())
                            },
                        )?;
                        let other = second.expect("second original helper invocation");
                        leaves.with_checked_entry_writes_v18(
                            budget,
                            |request, budget| {
                                assert_ne!(request.row.instance, other.instance);
                                check_fixture_entry_rhs_v18(leaves, request, budget)?;
                                let expression =
                                    fixture_entry_expression_v18(leaves, request, budget)?;
                                entered.set(true);
                                // These are independent normalizer substitution controls,
                                // not forged source owners or claimed compiler mutations.
                                if output_endpoint {
                                    let normalizer = OptimizedSourceScalarNormalizationV18 {
                                        leaves,
                                        arguments: request.arguments,
                                    };
                                    optimized_source_scalar_expression_endpoint_v18(
                                        leaves.original.leaves,
                                        optimized,
                                        optimized.output_inventory(budget)?,
                                        leaves.function.coordinate,
                                        &normalizer,
                                        request.output_origins,
                                        request.output_inline,
                                        &expression,
                                        other.output_rhs,
                                        budget,
                                    )
                                } else {
                                    source_scalar_expression_value_v18(
                                        leaves.original.leaves,
                                        request.arguments,
                                        request.input_origins,
                                        request.input_inline,
                                        &expression,
                                        other.input_rhs,
                                        budget,
                                    )
                                }
                            },
                            |_, _| panic!("same-typed substituted endpoint admitted"),
                        )
                    },
                )
            });
        assert!(entered.get());
        let expected = if output_endpoint {
            "actual optimized scalar expression differs from its original source value"
        } else {
            "actual scalar expression differs from its original source value"
        };
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Binding(detail)) if detail == expected),
            "{result:?}"
        );
    }
}

#[test]
fn typed_entry_rhs_headers_have_an_independent_exact_and_one_short_equation() {
    type Prepared<'a> = (
        Vec<SourceEntryWriteRowV18>,
        SourceRootArgumentsV18<'a, 'a>,
        Gfx942InlineScalarCorrespondenceV30<'a>,
        Gfx942InlineScalarCorrespondenceV30<'a>,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        usize,
    );
    let expected = size_of::<Prepared<'_>>()
        + 2 * size_of::<SourceOwnedResultV18<Prepared<'_>>>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<Prepared<'_>>>>()
        + size_of::<std::panic::AssertUnwindSafe<SourceOwnedResultV18<Prepared<'_>>>>()
        + size_of::<Vec<SourceEntryWriteRowV18>>()
        + size_of::<SourceRootArgumentsV18<'_, '_>>()
        + 2 * size_of::<Gfx942InlineScalarCorrespondenceV30<'_>>()
        + size_of::<ProductionSourceEntryWriteV18<'_>>()
        + size_of::<ProductionCheckedSourceEntryWritesV18<'_>>()
        + size_of::<OptimizedSourceScalarNormalizationV18<'_>>()
        + size_of::<Option<SourceEntryWriteRowV18>>()
        + size_of::<SourcePhysicalObjectV18<'_>>()
        + size_of::<OptimizedSourceObjectV18<'_>>()
        + size_of::<ScopedObjectPayloadV29>()
        + size_of::<[Option<ValueId>; 2]>()
        + size_of::<SourceEntryWriteRowV18>()
        + size_of::<SourceOwnedResultV18<Option<SourceEntryWriteRowV18>>>()
        + size_of::<SourceOwnedResultV18<SourcePhysicalObjectV18<'_>>>()
        + size_of::<SourceOwnedResultV18<ScopedObjectPayloadV29>>()
        + size_of::<ProductionOptimizedSourceOperationV18>()
        + size_of::<SourceOwnedResultV18<ProductionOptimizedSourceOperationV18>>()
        + size_of::<SourceOwnedResultV18<&SemanticFunctionDeclV1>>()
        + size_of::<SourceOwnedResultV18<(usize, &SemanticFunctionDeclV1)>>()
        + size_of::<SourceOwnedResultV18<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>>(
        )
        + size_of::<SourceOwnedResultV18<&AdmittedInertSemanticMirV1>>()
        + size_of::<Option<&SemanticFunctionDeclV1>>()
        + size_of::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>()
        + size_of::<Option<SemanticLocalRoleV1>>()
        + size_of::<ProductionSourceScalarInputV18<'_>>()
        + size_of::<SourceOwnedResultV18<ProductionSourceScalarInputV18<'_>>>()
        + size_of::<SourceOwnedResultV18<ProductionSemanticScalarTypeV2>>()
        + size_of::<Type>()
        + size_of::<Result<Type, ProductionSemanticKirErrorV1>>()
        + size_of::<Option<&SourceEntryWriteRowV18>>()
        + 2 * size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Option<SourceOwnedQueryFailureV18>>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<std::cell::Cell<bool>>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<()>>>()
        + source_reference_cleanup_headers_v29().unwrap();
    assert_eq!(
        source_entry_write_headers_v18::<(), ProductionSourceOwnedViewErrorV18>().unwrap(),
        expected
    );
    assert_eq!(
        source_entry_write_callback_headers_v18(17, 64, 31, 128).unwrap(),
        17 + 31 + 2 * 64 + 2 * 128
    );
    assert!(matches!(
        source_entry_write_callback_headers_v18(0, usize::MAX, 0, 1),
        Err(ArgumentResourceV1::Arithmetic)
    ));
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let result = budget.reserve_storage(expected);
        assert_eq!(result.is_err(), short);
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(_))));
            assert_eq!(budget.storage(), 0);
        } else {
            assert_eq!(budget.storage(), expected);
            budget.release_storage(expected).unwrap();
        }
    }
}

#[test]
fn typed_entry_rhs_constructor_header_refusal_is_sticky_and_releases_no_foreign_credit() {
    let entered = std::cell::Cell::new(false);
    let result =
        with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
            let function =
                original.inventory.functions()[original.source.root(0, budget)?.1].function;
            let recipe = scalar_leaf_collision_recipe_v18(function);
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    let floor = budget.storage();
                    let headers =
                        source_entry_write_headers_v18::<(), ProductionSourceOwnedViewErrorV18>()?;
                    let padding = MODULE_LIMIT - floor - headers + 1;
                    budget.reserve_storage(padding)?;
                    let first = leaves.with_checked_entry_writes_v18(
                        budget,
                        |_, _| panic!("one-short header must refuse before request"),
                        |_, _| Ok::<_, ProductionSourceOwnedViewErrorV18>(()),
                    );
                    assert!(matches!(
                        first,
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Storage(_)
                        ))
                    ));
                    assert_eq!(budget.storage(), floor + padding);
                    budget.release_storage(padding)?;
                    let before = (budget.work(), budget.storage());
                    let replay = leaves.with_checked_entry_writes_v18(
                        budget,
                        |_, _| panic!("sticky refusal cannot visit"),
                        |_, _| Ok::<_, ProductionSourceOwnedViewErrorV18>(()),
                    );
                    assert_eq!(format!("{first:?}"), format!("{replay:?}"));
                    assert_eq!((budget.work(), budget.storage()), before);
                    entered.set(true);
                    first
                },
            )
        });
    assert!(entered.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Storage(_)
        ))
    ));
}

#[test]
fn typed_entry_rhs_completed_query_keeps_first_work_refusal() {
    let entered = std::cell::Cell::new(false);
    let result =
        with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
            let function =
                original.inventory.functions()[original.source.root(0, budget)?.1].function;
            let recipe = scalar_leaf_collision_recipe_v18(function);
            original.with_optimized_scalar_leaves_v18(
                optimized,
                0,
                &recipe,
                budget,
                |leaves, budget| {
                    leaves.with_checked_entry_writes_v18(
                        budget,
                        |request, budget| check_fixture_entry_rhs_v18(leaves, request, budget),
                        |checked, budget| {
                            let row = checked.rows[0];
                            budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work())?;
                            let first = checked.require(
                                row.instance,
                                row.anchor,
                                row.input,
                                row.output,
                                row.input_rhs,
                                row.output_rhs,
                                row.scalar,
                                row.schema,
                                budget,
                            );
                            assert!(matches!(
                                first,
                                Err(ProductionSourceOwnedViewErrorV18::Resource(
                                    ArgumentResourceV1::Work(_)
                                ))
                            ));
                            let before = (budget.work(), budget.storage());
                            let replay = checked.check_for(original, optimized, usize::MAX, budget);
                            assert_eq!(format!("{first:?}"), format!("{replay:?}"));
                            assert_eq!((budget.work(), budget.storage()), before);
                            entered.set(true);
                            first
                        },
                    )
                },
            )
        });
    assert!(entered.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Work(_)
        ))
    ));
}

#[test]
fn typed_entry_rhs_scope_keeps_owned_success_error_and_panic_backing() {
    with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
        let function = original.inventory.functions()[original.source.root(0, budget)?.1].function;
        let recipe = scalar_leaf_collision_recipe_v18(function);
        original.with_optimized_scalar_leaves_v18(
            optimized,
            0,
            &recipe,
            budget,
            |leaves, budget| {
                let floor = budget.storage();
                for mode in 0..3 {
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        leaves.with_checked_entry_writes_v18(
                            budget,
                            |request, budget| {
                                check_fixture_entry_rhs_v18(leaves, request, budget)
                                    .map_err(OwnedCallbackErrorV18::from)
                            },
                            |checked, budget| {
                                assert_eq!(checked.rows.len(), 2);
                                let payload = owned_callback_payload_v18(budget);
                                match mode {
                                    0 => Ok(payload),
                                    1 => Err(OwnedCallbackErrorV18::Payload(payload)),
                                    _ => {
                                        budget.reserve_storage(size_of::<Vec<u64>>()).unwrap();
                                        std::panic::resume_unwind(Box::new(payload))
                                    }
                                }
                            },
                        )
                    }));
                    let backing =
                        64 * size_of::<u64>() + if mode == 2 { size_of::<Vec<u64>>() } else { 0 };
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
                        _ => panic!("typed entry context changed selected callback disposition"),
                    }
                    budget.release_storage(backing)?;
                }
                Ok(())
            },
        )
    })
    .unwrap();
}

#[test]
fn typed_entry_rhs_swallowed_expression_and_work_errors_never_publish_context() {
    for resource in [false, true] {
        let swallowed = std::cell::Cell::new(false);
        let result = with_entry_fixture_v18(
            typed_entry_rhs_owner_v18,
            |original, optimized, budget| {
                let function =
                    original.inventory.functions()[original.source.root(0, budget)?.1].function;
                let recipe = scalar_leaf_collision_recipe_v18(function);
                original.with_optimized_scalar_leaves_v18(optimized, 0, &recipe, budget, |leaves, budget| {
                leaves.with_checked_entry_writes_v18(budget, |request, budget| {
                    // First succeed, so a stale completion bit alone would
                    // incorrectly admit the subsequent swallowed failure.
                    check_fixture_entry_rhs_v18(leaves, request, budget)?;
                    let scalar = request.scalar(budget)?;
                    if resource { budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work())?; }
                    let refused = request.check_expression(&ProductionSemanticExpressionV2::Constant {
                        scalar, bits: 0xa11ce,
                    }, budget);
                    if resource {
                        assert!(matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(_)))));
                    } else {
                        assert!(matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "actual scalar expression differs from its original source value"))));
                    }
                    swallowed.set(true);
                    Ok(())
                }, |_, _| panic!("swallowed request failure published context"))
            })
            },
        );
        assert!(swallowed.get());
        assert!(matches!(
            (resource, result),
            (
                true,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Work(_)
                ))
            ) | (
                false,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "actual scalar expression differs from its original source value"
                ))
            )
        ));
    }
}

#[test]
fn typed_entry_rhs_context_rejects_wrong_root_and_genuine_other_relation_before_debit() {
    for foreign in [false, true] {
        let result =
            with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
                let function =
                    original.inventory.functions()[original.source.root(0, budget)?.1].function;
                let recipe = scalar_leaf_collision_recipe_v18(function);
                original.with_optimized_scalar_leaves_v18(
                    optimized,
                    0,
                    &recipe,
                    budget,
                    |leaves, budget| {
                        leaves.with_checked_entry_writes_v18(
                            budget,
                            |request, budget| check_fixture_entry_rhs_v18(leaves, request, budget),
                            |checked, budget| {
                                if foreign {
                                    original.source.with_ranked_correspondence_v18(
                                        original.inventory,
                                        budget,
                                        |other, budget| {
                                            assert!(!std::ptr::eq(original, other));
                                            let before = (budget.work(), budget.storage());
                                            let refused =
                                                checked.check_for(other, optimized, 0, budget);
                                            assert_eq!((budget.work(), budget.storage()), before);
                                            refused
                                        },
                                    )
                                } else {
                                    let before = (budget.work(), budget.storage());
                                    let refused =
                                        checked.check_for(original, optimized, usize::MAX, budget);
                                    assert_eq!((budget.work(), budget.storage()), before);
                                    refused
                                }
                            },
                        )
                    },
                )
            });
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "typed entry context changed exact owners or root"
                ))
            ),
            "{result:?}"
        );
    }
}

#[test]
fn typed_entry_rhs_foreign_ledger_and_higher_live_floor_veto_refunds() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(typed_entry_rhs_owner_v18, &mut budget);
        let observed = std::cell::Cell::new(false);
        let result = with_production_optimized_consumer_v18(
            prepared,
            &mut budget,
            |original, optimized, budget| {
                let function =
                    original.inventory.functions()[original.source.root(0, budget)?.1].function;
                let recipe = scalar_leaf_collision_recipe_v18(function);
                original.with_optimized_scalar_leaves_v18(
                    optimized,
                    0,
                    &recipe,
                    budget,
                    |leaves, budget| {
                        leaves.with_checked_entry_writes_v18(
                            budget,
                            |request, budget| check_fixture_entry_rhs_v18(leaves, request, budget),
                            |checked, budget| {
                                let before = (budget.work(), budget.storage());
                                let refused = if foreign {
                                    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(
                                        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
                                    );
                                    let mut other =
                                        ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                                    other.reserve_storage(budget.storage())?;
                                    let other_before = (other.work(), other.storage());
                                    let refused =
                                        checked.check_for(original, optimized, 0, &mut other);
                                    assert_eq!((other.work(), other.storage()), other_before);
                                    assert_eq!((budget.work(), budget.storage()), before);
                                    refused
                                } else {
                                    budget.release_storage(1)?;
                                    assert!(budget.storage() >= leaves.floor);
                                    checked.check_for(original, optimized, 0, budget)
                                };
                                assert!(original.source.cleanup.is_denied());
                                observed.set(true);
                                refused
                            },
                        )
                    },
                )
            },
        );
        assert!(observed.get());
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ),
            "{result:?}"
        );
        assert!(
            budget.storage() > MODULE_FLOOR,
            "denied custody cannot refund source or lexical credits"
        );
    }
}
