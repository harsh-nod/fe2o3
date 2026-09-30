#[test]
fn source_only_scalar_namespace_preserves_complete_census_and_ranked_collisions() {
    with_entry_fixture_v18(
        scalar_read_store_owner_v18,
        |original, optimized, budget| {
            let function =
                original.inventory.functions()[original.source.root(0, budget)?.1].function;
            let recipe = scalar_leaf_collision_recipe_v18(function);
            let floor = budget.storage();
            original.with_source_scalar_leaves_v18(0, budget, |source, budget| {
                assert!(!source.leaves.rows.is_empty());
                original.with_scalar_leaves_v18(0, &recipe, budget, |ranked, _| {
                    assert_eq!(source.leaves.rows.len(), ranked.leaves.rows.len());
                    for (ordinal, (left, right)) in source
                        .leaves
                        .rows
                        .iter()
                        .zip(&ranked.leaves.rows)
                        .enumerate()
                    {
                        assert_eq!(
                            source_leaf_original_order_v18(left),
                            source_leaf_original_order_v18(right)
                        );
                        assert_eq!(left.scalar, right.scalar);
                        assert_eq!(left.symbol as usize, ordinal);
                        assert!(![0, 2].contains(&right.symbol));
                        assert!(left.symbol < PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2);
                    }
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })
            })?;
            assert_eq!(budget.storage(), floor);
            original.with_optimized_source_scalar_leaves_v18(
                optimized,
                0,
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
                            panic!("original scalar memory operand");
                        };
                        let expression = leaves
                            .original_leaves(budget)?
                            .original_place(instance, function, place, budget)?
                            .expect("complete source Load name");
                        request.check_expression(&expression, budget)?;
                        reads += 1;
                        Ok(())
                    })?;
                    assert!(reads >= 2);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                },
            )?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn source_only_scalar_namespace_checks_root_and_distinct_helper_entry_rhs() {
    for (factory, expected) in [
        (typed_root_entry_rhs_owner_v18 as fn() -> _, 1),
        (typed_entry_rhs_owner_v18 as fn() -> _, 2),
    ] {
        with_entry_fixture_v18(factory, |original, optimized, budget| {
            let floor = budget.storage();
            original.with_optimized_source_scalar_leaves_v18(
                optimized,
                0,
                budget,
                |leaves, budget| {
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
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        },
                    )
                },
            )?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn source_only_scalar_namespace_does_not_admit_ranked_symbols() {
    let reached = std::cell::Cell::new(false);
    let result =
        with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
            original.with_optimized_source_scalar_leaves_v18(
                optimized,
                0,
                budget,
                |leaves, budget| {
                    leaves.with_checked_entry_writes_v18(
                        budget,
                        |request, budget| {
                            check_fixture_entry_rhs_v18(leaves, request, budget)?;
                            reached.set(true);
                            request.check_expression(
                                &ProductionSemanticExpressionV2::Symbol {
                                    symbol: fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2,
                                    scalar: request.scalar(budget)?,
                                },
                                budget,
                            )
                        },
                        |_, _| panic!("ranked symbol cannot complete source-only entry checks"),
                    )
                },
            )
        });
    assert!(reached.get());
    assert!(result.is_err());
}

#[test]
fn source_only_scalar_namespace_wrong_root_never_enters_callback() {
    for optimized_scope in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result =
            with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
                if optimized_scope {
                    original.with_optimized_source_scalar_leaves_v18(
                        optimized,
                        usize::MAX,
                        budget,
                        |_, _| {
                            reached.set(true);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        },
                    )
                } else {
                    original.with_source_scalar_leaves_v18(usize::MAX, budget, |_, _| {
                        reached.set(true);
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                    })
                }
            });
        assert!(!reached.get());
        assert!(result.is_err());
    }
}

#[test]
fn source_only_scalar_factor_preserves_ranked_root_name_refusal() {
    let result =
        with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
            let foreign = fe2o3_pliron::ProductionRankedKernelV1::new(
                "foreign_root",
                0,
                vec![fe2o3_pliron::ProductionRankedBlockV1::new(
                    vec![],
                    fe2o3_pliron::ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            original.with_optimized_scalar_leaves_v18(optimized, 0, &foreign, budget, |_, _| {
                panic!("wrong ranked function name cannot enter source scope")
            })
        });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "scalar leaf ranked root differs"
        ))
    ));
}

#[test]
fn source_only_scalar_constructor_keeps_first_storage_refusal() {
    for optimized_scope in [false, true] {
        for attempt_header in [true, false] {
            let reached = std::cell::Cell::new(false);
            let result =
                with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
                    let floor = budget.storage();
                    // The optimized adapter moves both borrowed subjects, the
                    // root ordinal, and the guarded ZST user continuation.
                    type Adapter<'a, 's> = (
                        &'a ProductionSourceCorrespondenceV18<'s>,
                        &'a ProductionOptimizedSourceCorrespondenceV18<'s>,
                        usize,
                        Option<()>,
                    );
                    let headers = std::mem::size_of::<ProductionSourceScalarLeavesV18<'_>>()
                        + std::mem::size_of::<SourceScalarNamespaceV18<'_>>()
                        + std::mem::size_of::<&SourceScalarNamespaceV18<'_>>()
                        + std::mem::size_of::<
                            std::thread::Result<Result<(), ProductionSourceOwnedViewErrorV18>>,
                        >()
                        + source_owned_finish_header_oracle_v26::<
                            (),
                            ProductionSourceOwnedViewErrorV18,
                        >()
                        + if optimized_scope {
                            std::mem::size_of::<Option<Adapter<'_, '_>>>()
                                + std::mem::align_of::<Option<Adapter<'_, '_>>>()
                        } else {
                            std::mem::size_of::<Option<()>>() + std::mem::align_of::<Option<()>>()
                        };
                    type Capture<'a, 's, F> = (
                        Option<F>,
                        &'a ProductionSourceCorrespondenceV18<'s>,
                        &'a usize,
                        &'a SourceScalarNamespaceV18<'s>,
                        &'a usize,
                        &'a usize,
                    );
                    fn envelope<F>() -> usize {
                        scoped_source_attempt_header_oracle_v29::<
                            ((SourceScalarLeavesV18<'_, '_>, usize), Option<F>),
                            ProductionSourceOwnedViewErrorV18,
                            Capture<'_, '_, F>,
                        >()
                    }
                    let envelope = if optimized_scope {
                        envelope::<Adapter<'_, '_>>()
                    } else {
                        envelope::<()>()
                    };
                    let target = envelope + if attempt_header { 0 } else { headers };
                    let padding = MODULE_LIMIT - floor - target + 1;
                    budget.reserve_storage(padding)?;
                    let first = if optimized_scope {
                        original.with_optimized_source_scalar_leaves_v18(
                            optimized,
                            0,
                            budget,
                            |_, _| panic!("short header cannot enter callback"),
                        )
                    } else {
                        original.with_source_scalar_leaves_v18(0, budget, |_, _| {
                            panic!("short header cannot enter callback")
                        })
                    };
                    let Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Storage(bound),
                    )) = first.as_ref()
                    else {
                        panic!("exact constructor header cut: {first:?}");
                    };
                    assert_eq!(
                        (bound.actual(), bound.limit()),
                        (MODULE_LIMIT + 1, MODULE_LIMIT)
                    );
                    assert_eq!(budget.storage(), floor + padding);
                    budget.release_storage(padding)?;
                    let before = (budget.work(), budget.storage());
                    let replay: SourceOwnedResultV18<()> =
                        original.with_source_scalar_leaves_v18(0, budget, |_, _| {
                            panic!("first resource failure remains sticky")
                        });
                    assert_eq!(format!("{first:?}"), format!("{replay:?}"));
                    assert_eq!((budget.work(), budget.storage()), before);
                    reached.set(true);
                    first
                });
            assert!(reached.get());
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Storage(_)
                ))
            ));
        }
    }
}

#[test]
fn optimized_scalar_inner_attempt_header_exact_and_short_keep_first_refusal() {
    for short in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result =
            with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
                let outer_floor = budget.storage();
                type Capture<'a, 's> = (
                    &'a ProductionSourceCorrespondenceV18<'s>,
                    &'a ProductionOptimizedSourceCorrespondenceV18<'s>,
                    &'a usize,
                    &'a ProductionSourceScalarLeavesV18<'s>,
                );
                let helper = scoped_source_attempt_header_oracle_v29::<
                    (
                        Vec<OptimizedSourceScalarReadV18>,
                        Vec<SourceWrappingValueV23>,
                        OptimizedSourceScalarBoundariesV31,
                        Vec<OptimizedIssuedPresenceV31>,
                        &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
                        usize,
                    ),
                    ProductionSourceOwnedViewErrorV18,
                    Capture<'_, '_>,
                >();
                let explicit = size_of::<ProductionOptimizedSourceScalarLeavesV18<'_>>()
                    + size_of::<Vec<OptimizedSourceScalarReadV18>>()
                    + size_of::<Vec<SourceWrappingValueV23>>()
                    + size_of::<SourceWrappingValueV23>()
                    + optimized_source_boundary_headers_v31()?
                    + slice_view_v1::optimized_presence_headers_v31()?
                    + size_of::<std::thread::Result<SourceOwnedResultV18<()>>>()
                    + source_reference_cleanup_headers_v29()?;
                OPTIMIZED_SCALAR_ATTEMPT_PROBE_V18.set(Some((
                    helper - usize::from(short),
                    0,
                    0,
                    0,
                )));
                let first: SourceOwnedResultV18<()> = original
                    .with_optimized_source_scalar_leaves_v18(optimized, 0, budget, |_, _| {
                        panic!("the optimized inner constructor must refuse")
                    });
                let (_, padding, inner_floor, calls) =
                    OPTIMIZED_SCALAR_ATTEMPT_PROBE_V18.replace(None).unwrap();
                assert_eq!(calls, 1, "the original scalar scope must have succeeded");
                assert!(inner_floor > outer_floor);
                let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(
                    bound,
                ))) = first.as_ref()
                else {
                    panic!("the actual optimized inner attempt must refuse: {first:?}");
                };
                let limit = budget.storage_limit();
                assert_eq!(
                    (bound.actual(), bound.limit()),
                    (limit + if short { 1 } else { explicit }, limit)
                );
                assert_eq!(budget.storage(), outer_floor + padding);
                budget.release_storage(padding)?;
                let before = (budget.work(), budget.storage());
                let replay: SourceOwnedResultV18<()> = original
                    .with_optimized_source_scalar_leaves_v18(optimized, 0, budget, |_, _| {
                        panic!("retained inner failure forbids retry")
                    });
                assert_eq!(format!("{first:?}"), format!("{replay:?}"));
                assert_eq!((budget.work(), budget.storage()), before);
                reached.set(true);
                first
            });
        assert!(reached.get(), "{result:?}");
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage(_)
            ))
        ));
    }
}

#[test]
fn source_only_scalar_scopes_preserve_owned_success_error_and_panic_backing() {
    with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, optimized, budget| {
        let floor = budget.storage();
        for mode in 0..3 {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                original.with_optimized_source_scalar_leaves_v18(
                    optimized,
                    0,
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
                                std::panic::resume_unwind(Box::new(payload));
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
                (0, Ok(Ok(payload))) | (1, Ok(Err(OwnedCallbackErrorV18::Payload(payload)))) => {
                    assert_eq!(payload, vec![0x271; 64]);
                    drop(payload);
                }
                (2, Err(payload)) => {
                    let payload = payload.downcast::<Vec<u64>>().unwrap();
                    assert_eq!(*payload, vec![0x271; 64]);
                    drop(payload);
                }
                _ => panic!("source-only callback disposition changed"),
            }
            budget.release_storage(backing)?;
            assert_eq!(budget.storage(), floor);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_only_scalar_scopes_refuse_foreign_ledger_and_lost_live_floor() {
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
                original.with_optimized_source_scalar_leaves_v18(
                    optimized,
                    0,
                    budget,
                    |leaves, budget| {
                        let before = (budget.work(), budget.storage());
                        let refused = if foreign {
                            let mut other_work =
                                CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                            let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                            other.reserve_storage(budget.storage())?;
                            let other_before = (other.work(), other.storage());
                            let refused = leaves.check(&mut other);
                            assert_eq!((other.work(), other.storage()), other_before);
                            assert_eq!((budget.work(), budget.storage()), before);
                            refused
                        } else {
                            budget.release_storage(1)?;
                            leaves.check(budget)
                        };
                        assert!(original.source.cleanup.is_denied());
                        assert!(matches!(
                            refused,
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        let before = (budget.work(), budget.storage());
                        let replay = leaves.check(budget);
                        assert_eq!(format!("{refused:?}"), format!("{replay:?}"));
                        assert_eq!((budget.work(), budget.storage()), before);
                        observed.set(true);
                        refused
                    },
                )
            },
        );
        assert!(observed.get());
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(budget.storage() > MODULE_FLOOR);
    }
}
#[test]
fn source_slice_initial_query_refusal_protects_uncalled_capture() {
    let dropped = std::cell::Cell::new(0);
    let reached = std::cell::Cell::new(false);
    let result = with_entry_fixture_v18(typed_entry_rhs_owner_v18, |original, _, budget| {
        let selected = original.retain_query::<()>(Err(ArgumentResourceV1::Accounting.into()));
        assert!(matches!(
            selected,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        let capture = SourceQueryDropV1751 {
            drops: &dropped,
            deny: None,
            payload: Some(Box::new(0x1797_0030_u64)),
        };
        let before = (budget.work(), budget.storage());
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            original.with_checked_slice_access_v18(
                0,
                0,
                ProductionSliceAccessSiteV1::new(
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(0),
                    SemanticBlockIdV1::from_index(0),
                    None,
                    0,
                    SemanticBlockIdV1::from_index(0),
                ),
                budget,
                move |_, _| -> Result<(), ProductionSemanticKirErrorV1> {
                    std::hint::black_box(&capture);
                    panic!("sticky source refusal invoked slice consumer");
                },
            )
        }));
        assert_eq!(dropped.get(), 1);
        let result = caught.expect("uncalled capture panic replaced source refusal");
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        reached.set(true);
        result
    });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
}
