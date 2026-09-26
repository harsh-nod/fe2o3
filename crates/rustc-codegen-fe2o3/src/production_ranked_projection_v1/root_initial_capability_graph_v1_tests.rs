// Component source/data controls only; never manufactured nominal authority.
mod initial_capability_graph_controls {
    use super::super::bf16_nominal_preparation_resources_v1::PreparationResourcesV1;
    use super::super::root_initial_capability_graph_v1::{
        InitialGraphStorageV1, populate_initial_graph_v1,
    };
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};

    const LIMIT: usize = 16 * 1024 * 1024;
    const FLOOR: usize = 37;

    fn aliases(projected: bool) -> SemanticFunctionDeclV1 {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            if projected {
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE)
                        .unwrap();
                    17
                ]
            } else {
                vec![]
            },
            SCALAR_TYPE,
        )
        .unwrap();
        projection_function_with_locals(
            vec![block(
                31,
                vec![
                    typed_assignment(
                        2,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)),
                    ),
                    typed_assignment(
                        3,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(2),
                                vec![],
                                SCALAR_TYPE,
                            )
                            .unwrap(),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            (0..4)
                .map(|i| {
                    local(
                        31 + i,
                        SCALAR_TYPE,
                        if i == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                    )
                })
                .collect(),
        )
    }

    fn enums(duplicate: bool) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
        let (types, _) = ordinary_component_payload_fixture();
        let store = |source| {
            typed_assignment(
                1,
                ENUM_TYPE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::EnumVariant(1),
                        vec![typed_operand(source, SCALAR_TYPE)],
                    )
                    .unwrap(),
                ),
            )
        };
        let mut statements = vec![store(0)];
        if duplicate {
            statements.push(store(3));
        }
        statements.push(typed_assignment(
            2,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(1),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), ENUM_TYPE)
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR_TYPE)
                            .unwrap(),
                    ],
                    SCALAR_TYPE,
                )
                .unwrap(),
            )),
        ));
        (
            types,
            projection_function_with_locals(
                vec![block(34, statements, SemanticTerminatorKindV1::Return)],
                vec![
                    local(34, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(35, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                    local(36, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                    local(37, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            ),
        )
    }
    fn empty_graph(function: &SemanticFunctionDeclV1) -> InitialGraphStorageV1 {
        let mut result = InitialGraphStorageV1::empty();
        result.edges = vec![Vec::new(); function.locals().len()];
        result
    }
    fn snapshot(graph: &InitialGraphStorageV1) -> String {
        format!(
            "{:?}/{:?}/{:?}/{:?}/{:?}",
            graph.edges, graph.edge_count, graph.stores, graph.loads, graph.borrowed
        )
    }
    fn run(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        graph: &mut InitialGraphStorageV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
        old: bool,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let options = SemanticOptionDominanceV1::analyze(function, &[]).unwrap();
        let enums = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
        let counts = assertion_definition_inventory(function).unwrap().counts;
        if old {
            return old_initial_graph(&[], function, None, &counts, &options, &enums, graph);
        }
        populate_initial_graph_v1(
            &[],
            function,
            None,
            &counts,
            &options,
            &enums,
            &mut graph.edges,
            &mut graph.edge_count,
            &mut graph.stores,
            &mut graph.loads,
            &mut graph.borrowed,
            resources,
        )
    }

    #[test]
    fn shared_legacy_initial_graph_matches_frozen_source_loop_and_partial_refusals() {
        let (enum_types, enum_function) = enums(false);
        let (duplicate_types, duplicate_function) = enums(true);
        for (types, function) in [
            (projection_types(), aliases(false)),
            (projection_types(), aliases(true)),
            (enum_types, enum_function),
            (duplicate_types, duplicate_function),
        ] {
            let mut old = empty_graph(&function);
            let mut new = empty_graph(&function);
            let a = run(
                &function,
                &types,
                &mut old,
                &mut PreparationResourcesV1::unmetered(),
                true,
            );
            let b = run(
                &function,
                &types,
                &mut new,
                &mut PreparationResourcesV1::unmetered(),
                false,
            );
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
            assert_eq!(snapshot(&old), snapshot(&new));
            assert_eq!(
                old.edges.iter().map(Vec::capacity).collect::<Vec<_>>(),
                new.edges.iter().map(Vec::capacity).collect::<Vec<_>>()
            );
            assert_eq!(
                (
                    old.stores.capacity(),
                    old.loads.capacity(),
                    old.borrowed.capacity()
                ),
                (
                    new.stores.capacity(),
                    new.loads.capacity(),
                    new.borrowed.capacity()
                )
            );
        }
    }

    #[test]
    fn metered_initial_graph_preserves_source_edge_order_and_enum_ambiguity() {
        for duplicate in [false, true] {
            let (types, function) = enums(duplicate);
            let mut old = empty_graph(&function);
            let expected = run(
                &function,
                &types,
                &mut old,
                &mut PreparationResourcesV1::unmetered(),
                true,
            );
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            let mut new = InitialGraphStorageV1::empty();
            let actual = {
                let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
                new.edges = resources.nested(function.locals().len()).unwrap();
                run(&function, &types, &mut new, &mut resources, false)
            };
            assert_eq!(format!("{expected:?}"), format!("{actual:?}"));
            assert_eq!(snapshot(&old), snapshot(&new));
            if duplicate {
                assert!(matches!(
                    actual,
                    Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "an enum payload has multiple candidate capability stores"
                    ))
                ));
            } else {
                assert_eq!(new.edge_count, 2); // Transparent carrier edge, then joined payload edge.
                assert_eq!(new.edges[0][0].destination, 2);
                assert_eq!(new.edges[1][0].destination, 2);
            }
            assert!(owned > 0 && budget.work() > 0);
            assert_eq!(budget.storage(), FLOOR + owned);
            drop(actual);
            drop(new);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    fn probe(
        projected: bool,
        work_limit: usize,
        storage_limit: usize,
    ) -> (bool, usize, usize, bool, bool) {
        let function = aliases(projected);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut graph = InitialGraphStorageV1::empty();
        let result = (|| {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            resources.work(function.locals().len())?;
            resources.reserve(&mut graph.edges, function.locals().len())?;
            graph.edges.resize_with(function.locals().len(), Vec::new);
            run(
                &function,
                &projection_types(),
                &mut graph,
                &mut resources,
                false,
            )
        })();
        let observation = (
            result.is_ok(),
            budget.work(),
            budget.peak_storage(),
            budget.failed_work().is_some(),
            budget.failed_storage().is_some(),
        );
        drop(result);
        drop(graph);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        observation
    }
    #[test]
    fn initial_graph_exact_work_and_storage_boundaries_refuse_without_refunding_live_values() {
        let passed = probe(false, LIMIT, LIMIT);
        assert!(passed.0);
        assert!(probe(false, passed.1, passed.2).0);
        assert!(probe(false, passed.1 - 1, LIMIT).3);
        assert!(probe(false, LIMIT, passed.2 - 1).4);
        assert!(probe(false, 0, LIMIT).3);
        assert!(probe(false, LIMIT, FLOOR).4);
    }
    #[test]
    fn every_transparency_projection_has_additional_original_work() {
        let plain = probe(false, LIMIT, LIMIT);
        let projected = probe(true, LIMIT, LIMIT);
        assert!(plain.0 && projected.0);
        assert!(projected.1 >= plain.1 + 17 * 4);
    }
    #[test]
    fn edge_cap_refuses_before_a_new_edge_allocation() {
        let function = aliases(false);
        let mut graph = empty_graph(&function);
        graph.edge_count = MAX_PROJECTED_OPERATIONS_V1;
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let result = run(
            &function,
            &projection_types(),
            &mut graph,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            false,
        );
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "capability def-use edges exceed the charged projection limit"
            ))
        ));
        assert_eq!(owned, 0);
        assert!(graph.edges.iter().all(Vec::is_empty));
    }
    #[test]
    fn initial_graph_partial_payload_and_error_or_panic_drop_before_own_refund() {
        for panic in [false, true] {
            let function = aliases(false);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            let mut pending = InitialGraphStorageV1::empty();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
                pending.edges = resources.nested(function.locals().len())?;
                run(
                    &function,
                    &projection_types(),
                    &mut pending,
                    &mut resources,
                    false,
                )?;
                if panic {
                    panic!("initial graph component panic");
                }
                Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                    "component callback refusal",
                ))
            }));
            assert!(pending.edge_count > 0);
            assert_eq!(budget.storage(), FLOOR + owned);
            budget.reserve_storage(23).unwrap(); // Foreign surplus remains foreign.
            drop(outcome);
            drop(pending);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR + 23);
        }
    }

    #[test]
    fn source_and_destination_bounds_refuse_before_edge_storage() {
        let function = aliases(false);
        for slots in [1, 2] {
            let mut graph = InitialGraphStorageV1::empty();
            graph.edges.resize_with(slots, Vec::new);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let result = run(
                &function,
                &projection_types(),
                &mut graph,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
                false,
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a capability def-use edge outside the semantic local table"
                ))
            ));
            assert_eq!(owned, 0);
            assert!(graph.edges.iter().all(Vec::is_empty));
        }
    }

    #[test]
    fn ordinary_intrinsic_alias_edge_is_shared_but_not_a_strict_profile_extension() {
        let terminal = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![typed_operand(1, SCALAR_TYPE)],
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], SCALAR_TYPE)
                        .unwrap(),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
        let function = projection_function_with_locals(
            vec![
                block(38, vec![], terminal),
                block(39, vec![], SemanticTerminatorKindV1::Return),
            ],
            (0..3)
                .map(|i| {
                    local(
                        38 + i,
                        SCALAR_TYPE,
                        if i == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                    )
                })
                .collect(),
        );
        let callables = [compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness: SCALAR_TYPE,
                raw_index: SCALAR_TYPE,
            },
        )];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let counts = assertion_definition_inventory(&function).unwrap().counts;
        let mut old = empty_graph(&function);
        old_initial_graph(
            &callables, &function, None, &counts, &options, &enums, &mut old,
        )
        .unwrap();
        let mut new = empty_graph(&function);
        populate_initial_graph_v1(
            &callables,
            &function,
            None,
            &counts,
            &options,
            &enums,
            &mut new.edges,
            &mut new.edge_count,
            &mut new.stores,
            &mut new.loads,
            &mut new.borrowed,
            &mut PreparationResourcesV1::unmetered(),
        )
        .unwrap();
        assert_eq!(snapshot(&old), snapshot(&new));
        assert_eq!(new.edge_count, 1);
        assert_eq!(new.edges[1][0].destination, 2);
    }

    #[test]
    fn accepted_fanout_preserves_statement_order_within_each_source_row() {
        let function = projection_function_with_locals(
            vec![block(
                40,
                vec![
                    typed_assignment(
                        3,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Use(typed_operand(1, SCALAR_TYPE)),
                    ),
                    typed_assignment(
                        2,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Use(typed_operand(1, SCALAR_TYPE)),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            (0..4)
                .map(|i| {
                    local(
                        40 + i,
                        SCALAR_TYPE,
                        if i == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                    )
                })
                .collect(),
        );
        let mut old = empty_graph(&function);
        run(
            &function,
            &projection_types(),
            &mut old,
            &mut PreparationResourcesV1::unmetered(),
            true,
        )
        .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut graph = InitialGraphStorageV1::empty();
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            graph.edges = resources.nested(function.locals().len()).unwrap();
            run(
                &function,
                &projection_types(),
                &mut graph,
                &mut resources,
                false,
            )
            .unwrap();
        }
        assert_eq!(snapshot(&old), snapshot(&graph));
        assert_eq!(
            graph.edges[1]
                .iter()
                .map(|edge| edge.destination)
                .collect::<Vec<_>>(),
            vec![3, 2]
        );
        drop(graph);
        budget.release_storage(owned).unwrap();
    }

    // Frozen pre-extraction source loop, with only bindings changed to borrowed
    // graph fields. No resource helper or new algorithm is used by this oracle.
    #[allow(clippy::too_many_arguments)]
    fn old_initial_graph(
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        linear_launch_upper_bound: Option<u64>,
        local_definitions: &[u8],
        option_dominance: &SemanticOptionDominanceV1,
        enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
        graph: &mut InitialGraphStorageV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let InitialGraphStorageV1 {
            edges: edges_by_source,
            edge_count,
            stores: enum_payload_stores,
            loads: enum_payload_loads,
            borrowed: borrowed_locals,
        } = graph;

        for (block_index, block) in function.blocks().iter().enumerate() {
            for (statement_index, statement) in block.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty() {
                    continue;
                }
                if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                    && let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind()
                    && let [operand] = aggregate.operands()
                    && let Some(source) = transparent_operand_place(operand)
                {
                    if enum_payload_stores.len() == MAX_PROJECTED_OPERATIONS_V1 {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "single-payload enum stores exceed the charged projection limit",
                        ));
                    }
                    enum_payload_stores.try_reserve(1).map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "single-payload enum store storage cannot be reserved",
                        )
                    })?;
                    enum_payload_stores.push(PendingEnumPayloadStoreV1 {
                        carrier: assignment.destination().local().index() as usize,
                        variant: *variant,
                        source: source.local().index() as usize,
                        construction_block: block_index,
                        statement: statement_index,
                    });
                    continue;
                }
                if let SemanticRvalueKindV1::Use(operand) = assignment.value().kind()
                    && let Some(place) = raw_operand_place(operand)
                    && let Some((carrier, variant)) = enum_payload_projection(place)
                {
                    if enum_payload_loads.len() == MAX_PROJECTED_OPERATIONS_V1 {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "single-payload enum loads exceed the charged projection limit",
                        ));
                    }
                    enum_payload_loads.try_reserve(1).map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "single-payload enum load storage cannot be reserved",
                        )
                    })?;
                    enum_payload_loads.push(PendingEnumPayloadLoadV1 {
                        carrier,
                        variant,
                        destination: assignment.destination().local().index() as usize,
                        use_block: block_index,
                        statement: statement_index,
                    });
                }
                let (source, borrowed) = match assignment.value().kind() {
                    SemanticRvalueKindV1::Use(operand) => {
                        (transparent_operand_place(operand), false)
                    }
                    SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. }
                        if place.projections().is_empty() =>
                    {
                        (Some(place), true)
                    }
                    _ => (None, false),
                };
                let Some(source) = source else {
                    continue;
                };
                let source = source.local().index() as usize;
                let destination = assignment.destination().local().index() as usize;
                if borrowed {
                    if borrowed_locals.len() == MAX_PROJECTED_OPERATIONS_V1 {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "borrowed capability uses exceed the charged projection limit",
                        ));
                    }
                    borrowed_locals.try_reserve(1).map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "borrowed capability use storage cannot be reserved",
                        )
                    })?;
                    borrowed_locals.push((source, block_index));
                }
                push_capability_edge(
                    edges_by_source,
                    edge_count,
                    source,
                    CapabilityEdgeV1 {
                        destination,
                        use_block: block_index,
                        kind: CapabilityEdgeKindV1::Alias,
                    },
                )?;
            }

            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
                callables.get(call.callee().index() as usize)
            else {
                continue;
            };
            let kind = match operation {
                SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. } => {
                    CapabilityEdgeKindV1::Alias
                }
                SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                    index_space,
                    ..
                } => CapabilityEdgeKindV1::IntoDisjoint {
                    mapping: *index_space,
                },
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                    output_space,
                    offset,
                    ..
                }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                    output_space,
                    offset,
                    ..
                } => {
                    let destination = simple_call_destination(call)?;
                    let availability = option_dominance.availability(destination).ok_or(
                        ProductionRankedProjectionErrorV1::Incomplete(
                            "a checked shift lacks authenticated Option Some availability",
                        ),
                    )?;
                    CapabilityEdgeKindV1::CheckedShift {
                        mapping: *output_space,
                        offset: *offset,
                        availability,
                    }
                }
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                    output_space,
                    lanes_per_block,
                    elements_per_lane,
                    ..
                } => {
                    let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                        lanes_per_block: *lanes_per_block,
                        elements_per_lane: *elements_per_lane,
                    };
                    if *output_space != expected
                        || *lanes_per_block == 0
                        || *elements_per_lane == 0
                        || lanes_per_block.checked_mul(*elements_per_lane).is_none()
                    {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "a malformed blocked mapping reached ranked projection",
                        ));
                    }
                    if !blocked_mapping_fits_launch_v1(
                        linear_launch_upper_bound,
                        *lanes_per_block,
                        *elements_per_lane,
                    ) {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "a multi-lane blocked mapping requires an authenticated finite rank-1 launch extent whose full blocked index range fits u64",
                        ));
                    }
                    let destination = simple_call_destination(call)?;
                    let availability = option_dominance.availability(destination).ok_or(
                        ProductionRankedProjectionErrorV1::Incomplete(
                            "a checked block lacks authenticated Option Some availability",
                        ),
                    )?;
                    CapabilityEdgeKindV1::CheckedBlock {
                        mapping: expected,
                        lanes_per_block: *lanes_per_block,
                        elements_per_lane: *elements_per_lane,
                        availability,
                    }
                }
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                    output_space,
                    lanes_per_tile,
                    tile_rows,
                    tile_columns,
                    elements_per_lane,
                    ..
                } => {
                    let expected = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                        lanes_per_tile: *lanes_per_tile,
                        tile_rows: *tile_rows,
                        tile_columns: *tile_columns,
                        elements_per_lane: *elements_per_lane,
                    };
                    if *output_space != expected
                        || !tiled_2d_geometry_valid_v1(
                            *lanes_per_tile,
                            *tile_rows,
                            *tile_columns,
                            *elements_per_lane,
                        )
                    {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "a malformed tiled-2d mapping reached ranked projection",
                        ));
                    }
                    let destination = simple_call_destination(call)?;
                    let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked tiled-2d witness lacks authenticated Option Some availability",
                    ),
                )?;
                    CapabilityEdgeKindV1::CheckedTiled2d {
                        mapping: expected,
                        availability,
                    }
                }
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
                    output_space,
                    lanes_per_row,
                    elements_per_lane,
                    ..
                } => {
                    let expected = SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                        lanes_per_row: *lanes_per_row,
                        elements_per_lane: *elements_per_lane,
                    };
                    if *output_space != expected
                        || !row_striped_2d_geometry_valid_v1(*lanes_per_row, *elements_per_lane)
                    {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "a malformed row-striped-2d mapping reached ranked projection",
                        ));
                    }
                    let destination = simple_call_destination(call)?;
                    let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked row-striped-2d witness lacks authenticated Option Some availability",
                    ),
                )?;
                    CapabilityEdgeKindV1::CheckedRowStriped2d {
                        mapping: expected,
                        availability,
                    }
                }
                _ => continue,
            };
            let destination = simple_call_destination(call)?.index() as usize;
            let source = call
                .arguments()
                .first()
                .and_then(simple_operand_local)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "an index capability transform without one exact input local",
                ))?
                .index() as usize;
            push_capability_edge(
                edges_by_source,
                edge_count,
                source,
                CapabilityEdgeV1 {
                    destination,
                    use_block: block_index,
                    kind,
                },
            )?;
        }

        enum_payload_stores.sort_unstable_by_key(|store| (store.carrier, store.variant));
        for load in enum_payload_loads.iter().copied() {
            let key = (load.carrier, load.variant);
            let first =
                enum_payload_stores.partition_point(|store| (store.carrier, store.variant) < key);
            let end =
                enum_payload_stores.partition_point(|store| (store.carrier, store.variant) <= key);
            let matches = &enum_payload_stores[first..end];
            let Some(store) = matches.first() else {
                continue;
            };
            if matches.len() != 1 {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an enum payload has multiple candidate capability stores",
                ));
            }
            let kind =
                if store.construction_block == load.use_block && store.statement < load.statement {
                    CapabilityEdgeKindV1::Alias
                } else {
                    if local_definitions.get(load.carrier).copied() != Some(1) {
                        continue;
                    }
                    let Some(availability) = enum_payload_dominance.availability(
                        SemanticLocalIdV1::from_index(load.carrier as u32),
                        load.variant,
                    ) else {
                        continue;
                    };
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                        construction_block: store.construction_block,
                        availability,
                    }
                };
            push_capability_edge(
                edges_by_source,
                edge_count,
                store.source,
                CapabilityEdgeV1 {
                    destination: load.destination,
                    use_block: load.use_block,
                    kind,
                },
            )?;
        }

        Ok(())
    }
}
