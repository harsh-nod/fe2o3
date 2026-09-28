// Inert component source/data tests only; no nominal owner or actual root namespace.
mod invocation_index_preparation_controls {
    use super::super::bf16_nominal_preparation_resources_v1::PreparationResourcesV1;
    use super::super::root_invocation_index_preparation_v1::*;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};
    const LIMIT: usize = 16 * 1024 * 1024;
    const FLOOR: usize = 37;
    type Outcome = std::result::Result<(), ProductionRankedProjectionErrorV1>;

    struct Inputs<'a> {
        function: &'a SemanticFunctionDeclV1,
        calls: &'a [SemanticCallableDeclV1],
        definitions: &'a [u8],
        escaped: &'a [bool],
        options: &'a SemanticOptionDominanceV1,
        enums: &'a SemanticEnumPayloadDominanceV1,
        edges: &'a [Vec<CapabilityEdgeV1>],
    }
    #[derive(Debug)]
    struct State {
        indices: Vec<Option<ProjectedDisjointIndexV1>>,
        grids: Vec<Option<ProjectedGridLeaderV1>>,
        predicates: Vec<Option<GuardPredicateV1>>,
        index_queue: VecDeque<usize>,
        grid_queue: VecDeque<usize>,
        operations: Vec<ProductionRankedOperationV1>,
        next_value: u32,
        text: String,
        processed: usize,
    }
    impl State {
        fn new(locals: usize) -> Self {
            Self {
                indices: vec![None; locals],
                grids: vec![None; locals],
                predicates: vec![None; locals],
                index_queue: VecDeque::new(),
                grid_queue: VecDeque::new(),
                operations: Vec::new(),
                next_value: 0,
                text: String::new(),
                processed: 0,
            }
        }
        fn capacities(&self) -> [usize; 6] {
            [
                self.indices.capacity(),
                self.grids.capacity(),
                self.predicates.capacity(),
                self.index_queue.capacity(),
                self.operations.capacity(),
                self.text.capacity(),
            ]
        }
    }
    fn seed(input: &Inputs<'_>, state: &mut State, old: bool) -> Outcome {
        if old {
            return frozen_seed(input, state);
        }
        seed_invocation_values_legacy_v1(
            input.calls,
            input.function,
            input.definitions,
            input.escaped,
            input.options,
            &mut state.indices,
            &mut state.grids,
            &mut state.predicates,
            &mut state.index_queue,
            &mut state.grid_queue,
            0,
            &mut state.operations,
            &mut state.next_value,
            &mut state.text,
        )
    }
    fn propagate(input: &Inputs<'_>, state: &mut State, old: bool) -> Outcome {
        if old {
            return frozen_propagate(input, state);
        }
        propagate_index_values_legacy_v1(
            input.definitions,
            input.escaped,
            input.options,
            input.enums,
            input.edges,
            &mut state.indices,
            &state.grids,
            &mut state.predicates,
            &mut state.index_queue,
            &mut state.processed,
            &mut state.operations,
            &mut state.next_value,
            &mut state.text,
        )
    }
    fn compare(
        input: &Inputs<'_>,
        initialize: impl Fn(&mut State),
    ) -> (std::result::Result<(), String>, State) {
        let mut old = State::new(input.function.locals().len());
        let mut new = State::new(input.function.locals().len());
        initialize(&mut old);
        initialize(&mut new);
        let a = seed(input, &mut old, true)
            .and_then(|()| propagate(input, &mut old, true))
            .map_err(|error| format!("{error:?}"));
        let b = seed(input, &mut new, false)
            .and_then(|()| propagate(input, &mut new, false))
            .map_err(|error| format!("{error:?}"));
        assert_eq!(a, b);
        assert_eq!(format!("{old:?}"), format!("{new:?}"));
        assert_eq!(old.capacities(), new.capacities());
        assert_eq!(old.grid_queue.capacity(), new.grid_queue.capacity());
        assert_eq!(
            old.predicates
                .iter()
                .map(|p| p.as_ref().map(|p| p.comparisons.capacity()))
                .collect::<Vec<_>>(),
            new.predicates
                .iter()
                .map(|p| p.as_ref().map(|p| p.comparisons.capacity()))
                .collect::<Vec<_>>()
        );
        (b, new)
    }
    fn fixture(destinations: &[u32]) -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
        let mut blocks = Vec::new();
        for (index, destination) in destinations.iter().copied().enumerate() {
            blocks.push(block(
                50 + index as u8,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            typed_place(destination, SCALAR_TYPE),
                            cfg_edge(SemanticEdgeRoleV1::CallReturn, index as u32 + 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ));
        }
        let statements = [(2, 1), (3, 1), (4, 2), (5, 3)]
            .into_iter()
            .filter(|(destination, _)| !destinations.contains(destination))
            .map(|(destination, source)| {
                typed_assignment(
                    destination,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(typed_operand(source, SCALAR_TYPE)),
                )
            })
            .collect();
        blocks.push(block(
            50 + destinations.len() as u8,
            statements,
            SemanticTerminatorKindV1::Return,
        ));
        let function = projection_function_with_locals(
            blocks,
            (0..6)
                .map(|index| {
                    local(
                        50 + index,
                        SCALAR_TYPE,
                        if index == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                    )
                })
                .collect(),
        );
        (
            function,
            vec![compiler_intrinsic_callable(
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: SCALAR_TYPE,
                    raw_index: SCALAR_TYPE,
                },
            )],
        )
    }
    fn graph(function: &SemanticFunctionDeclV1) -> Vec<Vec<CapabilityEdgeV1>> {
        let mut edges = vec![vec![]; function.locals().len()];
        let block = function.blocks().len() - 1;
        for (source, destination) in [(1, 2), (1, 3), (2, 4), (3, 5)] {
            edges[source].push(CapabilityEdgeV1 {
                destination,
                use_block: block,
                kind: CapabilityEdgeKindV1::Alias,
            });
        }
        edges
    }
    fn paid(
        input: &Inputs<'_>,
        pending: &mut PendingUnjoinedInvocationIndicesV1,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Outcome {
        prepare_unjoined_invocation_indices_v1(
            input.calls,
            input.function,
            input.definitions,
            input.escaped,
            input.options,
            input.enums,
            input.edges,
            pending,
            resources,
        )
    }

    #[test]
    fn ordinary_index_seed_and_fifo_match_frozen_algorithm_with_real_existing_namespace() {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        let (result, state) = compare(&input, |state| {
            state
                .operations
                .push(ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(80),
                    value: 17,
                });
            state.next_value = 81;
            state.text.push_str("already-emitted operation\n");
        });
        assert!(result.is_ok());
        assert_eq!(state.next_value, 82);
        assert_eq!(state.operations.len(), 2);
        assert_eq!(state.processed, 4);
        assert!(state.index_queue.is_empty());
        for row in &state.indices[1..] {
            assert_eq!(
                row.unwrap().value,
                ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(81))
            );
        }
    }

    #[test]
    fn ordinary_seed_preserves_emission_before_custody_and_text_or_value_refusal_order() {
        let (function, calls) = fixture(&[1]);
        let mut definitions = local_definition_counts(&function);
        let mut escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        for which in 0..4 {
            definitions[1] = if which == 0 { 0 } else { 1 };
            escaped[1] = which == 1;
            let input = Inputs {
                function: &function,
                calls: &calls,
                definitions: &definitions,
                escaped: &escaped,
                options: &options,
                enums: &enums,
                edges: &edges,
            };
            let (result, state) = compare(&input, |state| {
                if which == 2 {
                    state.next_value = u32::MAX;
                }
                if which == 3 {
                    state.text = "x".repeat(MAX_PROJECTED_RANKED_IR_BYTES_V1);
                }
            });
            assert!(result.is_err());
            assert!(state.indices[1].is_none());
            assert_eq!(state.operations.len(), usize::from(which != 2));
            assert_eq!(state.next_value, if which == 2 { u32::MAX } else { 1 });
            if which < 2 {
                assert!(result.unwrap_err().contains("mutable or address-escaped"));
                assert!(!state.text.is_empty());
            } else if which == 2 {
                assert!(result.unwrap_err().contains("too many ranked SSA values"));
            } else {
                assert!(result.unwrap_err().contains("diagnostic text limit"));
            }
        }
    }

    #[test]
    fn ordinary_duplicate_seed_and_conflicting_alias_preserve_partial_fifo_and_operations() {
        for duplicate in [false, true] {
            let (function, calls) = fixture(if duplicate { &[1, 1] } else { &[1, 2] });
            // Deliberately supplied component custody input to isolate the duplicate
            // capability check; this is not a source-authenticated scalar inventory.
            let definitions = vec![1; function.locals().len()];
            let escaped = vec![false; function.locals().len()];
            let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
            let enums =
                SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
            let mut edges = vec![vec![]; function.locals().len()];
            for source in [1, 2] {
                edges[source].push(CapabilityEdgeV1 {
                    destination: 3,
                    use_block: 2,
                    kind: CapabilityEdgeKindV1::Alias,
                });
            }
            let input = Inputs {
                function: &function,
                calls: &calls,
                definitions: &definitions,
                escaped: &escaped,
                options: &options,
                enums: &enums,
                edges: &edges,
            };
            let (result, state) = compare(&input, |_| {});
            assert!(result.is_err());
            assert_eq!(state.operations.len(), if duplicate { 1 } else { 2 });
            assert_eq!(
                state.index_queue.iter().copied().collect::<Vec<_>>(),
                if duplicate { vec![1] } else { vec![3] }
            );
        }
    }

    #[test]
    fn ordinary_grid_seed_and_all_transform_arms_match_frozen_source_but_paid_stays_narrow() {
        let (function, producers) = option_dominance_chain(1);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &producers).unwrap();
        let no_options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let availability = options
            .availability(SemanticLocalIdV1::from_index(1))
            .unwrap();
        let grid_calls = vec![compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {
                grid_leader: SCALAR_TYPE,
            },
        )];
        let no_edges = vec![vec![]; function.locals().len()];
        for has_availability in [false, true] {
            let input = Inputs {
                function: &function,
                calls: &grid_calls,
                definitions: &definitions,
                escaped: &escaped,
                options: if has_availability {
                    &options
                } else {
                    &no_options
                },
                enums: &enums,
                edges: &no_edges,
            };
            let (result, state) = compare(&input, |_| {});
            assert_eq!(result.is_ok(), has_availability);
            assert_eq!(state.operations.len(), if has_availability { 2 } else { 1 });
            if has_availability {
                assert_eq!(
                    state.grid_queue.iter().copied().collect::<Vec<_>>(),
                    vec![1]
                );
            }
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut pending = PendingUnjoinedInvocationIndicesV1::new();
            assert!(
                paid(
                    &input,
                    &mut pending,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
            );
            assert!(pending.payload_for_test().unwrap().operations.is_empty());
            drop(pending);
            budget.release_storage(owned).unwrap();
        }
        let calls = vec![compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: SCALAR_TYPE,
                raw_index: SCALAR_TYPE,
            },
        )];
        for kind in [
            CapabilityEdgeKindV1::IntoDisjoint {
                mapping: SemanticDisjointIndexSpaceV1::Index1d,
            },
            CapabilityEdgeKindV1::CheckedShift {
                mapping: SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 0 },
                offset: 0,
                availability,
            },
            CapabilityEdgeKindV1::CheckedShift {
                mapping: SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 7 },
                offset: 7,
                availability,
            },
            CapabilityEdgeKindV1::CheckedBlock {
                mapping: SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: 1,
                    elements_per_lane: 4,
                },
                lanes_per_block: 1,
                elements_per_lane: 4,
                availability,
            },
            CapabilityEdgeKindV1::CheckedBlock {
                mapping: SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: 2,
                    elements_per_lane: 4,
                },
                lanes_per_block: 2,
                elements_per_lane: 4,
                availability,
            },
            CapabilityEdgeKindV1::CheckedTiled2d {
                mapping: SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                    lanes_per_tile: 4,
                    tile_rows: 2,
                    tile_columns: 2,
                    elements_per_lane: 1,
                },
                availability,
            },
            CapabilityEdgeKindV1::CheckedRowStriped2d {
                mapping: SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                    lanes_per_row: 2,
                    elements_per_lane: 4,
                },
                availability,
            },
            CapabilityEdgeKindV1::AuthenticatedOptionPayload,
        ] {
            let mut edges = vec![vec![]; function.locals().len()];
            edges[1].push(CapabilityEdgeV1 {
                destination: 2,
                use_block: 2,
                kind,
            });
            let input = Inputs {
                function: &function,
                calls: &calls,
                definitions: &definitions,
                escaped: &escaped,
                options: &options,
                enums: &enums,
                edges: &edges,
            };
            assert!(compare(&input, |_| {}).0.is_ok());
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut pending = PendingUnjoinedInvocationIndicesV1::new();
            assert!(
                paid(
                    &input,
                    &mut pending,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
            );
            assert!(pending.payload_for_test().unwrap().operations.is_empty());
            drop(pending);
            budget.release_storage(owned).unwrap();
        }
    }

    #[test]
    fn paid_narrow_data_matches_ordinary_rows_operations_and_retains_exact_fifo_order() {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        let (result, ordinary) = compare(&input, |_| {});
        assert!(result.is_ok());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut pending = PendingUnjoinedInvocationIndicesV1::new();
        paid(
            &input,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        assert!(pending.completed_for_test());
        let data = pending.payload_for_test().unwrap();
        assert_eq!(data.indices, ordinary.indices);
        assert_eq!(data.grids, ordinary.grids);
        assert_eq!(data.predicates, ordinary.predicates);
        assert_eq!(
            format!("{:?}", data.operations),
            format!("{:?}", ordinary.operations)
        );
        assert_eq!(data.next_value, ordinary.next_value);
        assert_eq!(data.processed_edges, ordinary.processed);
        assert_eq!(data.index_fifo, vec![1, 2, 3, 4, 5]);
        assert_eq!(data.index_cursor, data.index_fifo.len());
        assert!(data.grid_fifo.is_empty());
        assert_eq!(data.grid_cursor, 0);
        assert_eq!(budget.storage(), FLOOR + owned);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }

    fn probe(work_limit: usize, storage_limit: usize) -> (bool, usize, usize, bool, bool, usize) {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut pending = PendingUnjoinedInvocationIndicesV1::new();
        let result = paid(
            &input,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert_eq!(budget.storage(), FLOOR + owned);
        let retained = pending
            .payload_for_test()
            .map_or(0, |data| data.operations.len() + data.index_fifo.len());
        let observation = (
            result.is_ok(),
            budget.work(),
            budget.peak_storage(),
            budget.failed_work().is_some(),
            budget.failed_storage().is_some(),
            retained,
        );
        drop(result);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        observation
    }

    #[test]
    fn paid_exact_work_storage_boundaries_and_partial_denial_keep_outer_payload_live() {
        let success = probe(LIMIT, LIMIT);
        assert!(success.0);
        assert!(probe(success.1, success.2).0);
        let work_denied = probe(success.1 - 1, LIMIT);
        assert!(!work_denied.0 && work_denied.3 && work_denied.5 > 0);
        let storage_denied = probe(LIMIT, success.2 - 1);
        assert!(!storage_denied.0 && storage_denied.4 && storage_denied.5 > 0);
        assert!(!probe(0, LIMIT).0);
        assert!(!probe(LIMIT, FLOOR).0);
    }

    #[test]
    fn paid_semantic_failure_retains_emitted_operation_and_refuses_same_or_fresh_ledger_retry() {
        let (function, calls) = fixture(&[1]);
        let mut definitions = local_definition_counts(&function);
        definitions[1] = 2;
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        let mut pending = PendingUnjoinedInvocationIndicesV1::new();
        assert!(
            paid(
                &input,
                &mut pending,
                &mut PreparationResourcesV1::unmetered()
            )
            .is_err()
        );
        assert!(pending.payload_for_test().is_none());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let error = paid(
            &input,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert!(matches!(
            error,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an index capability local has mutable or address-escaped value semantics"
            ))
        ));
        assert!(!pending.completed_for_test());
        assert_eq!(pending.payload_for_test().unwrap().operations.len(), 1);
        assert_eq!(pending.payload_for_test().unwrap().next_value, 1);
        assert!(pending.payload_for_test().unwrap().index_fifo.is_empty());
        let held = owned;
        let same = paid(
            &input,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert!(matches!(
            same,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "unjoined index pending owner cannot be replaced or retried"
            ))
        ));
        assert_eq!(owned, held);
        let mut other_work = Work::new(LIMIT);
        let mut other = Budget::new(&mut other_work, LIMIT);
        let mut other_owned = 0;
        assert!(
            paid(
                &input,
                &mut pending,
                &mut PreparationResourcesV1::new(&mut other, &mut other_owned)
            )
            .is_err()
        );
        assert_eq!(other_owned, 0);
        assert_eq!(other.storage(), 0);
        assert_eq!(pending.payload_for_test().unwrap().operations.len(), 1);
        drop(pending);
        budget.release_storage(owned).unwrap();
    }

    #[test]
    fn paid_outer_data_survives_error_and_panic_until_its_own_final_disposal() {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        for panic in [false, true] {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            let mut pending = PendingUnjoinedInvocationIndicesV1::new();
            let result = catch_unwind(AssertUnwindSafe(|| -> Outcome {
                paid(
                    &input,
                    &mut pending,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned),
                )?;
                if panic {
                    panic!("unjoined index component unwind");
                }
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "component consumer refusal",
                ))
            }));
            assert_eq!(result.is_err(), panic);
            assert!(pending.completed_for_test());
            assert!(!pending.payload_for_test().unwrap().operations.is_empty());
            assert_eq!(budget.storage(), FLOOR + owned);
            drop(result);
            drop(pending);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn enum_payload_transport_preserves_availability_and_rejects_uses_on_the_wrong_branch() {
        let (types, base) = ordinary_component_payload_fixture();
        let carrier = typed_assignment(
            1,
            ENUM_TYPE,
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::EnumVariant(1),
                    vec![typed_operand(4, SCALAR_TYPE)],
                )
                .unwrap(),
            ),
        );
        let discriminant = enum_discriminant(
            SemanticLocalIdV1::from_index(1),
            SemanticLocalIdV1::from_index(2),
        );
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                typed_place(4, SCALAR_TYPE),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let mut locals = base.locals().to_vec();
        locals.push(local(144, SCALAR_TYPE, SemanticLocalRoleV1::Temporary));
        locals.push(local(145, SCALAR_TYPE, SemanticLocalRoleV1::Temporary));
        let function = projection_function_with_locals(
            vec![
                block(139, vec![], SemanticTerminatorKindV1::Call(call)),
                block(
                    140,
                    vec![carrier, discriminant],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: typed_operand(2, SCALAR_TYPE),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![
                                SemanticSwitchTargetV1::new(
                                    0,
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                                ),
                                SemanticSwitchTargetV1::new(
                                    1,
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                                ),
                            ],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    141,
                    vec![
                        base.blocks()[1].statements()[0].clone(),
                        typed_assignment(
                            5,
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Use(typed_operand(3, SCALAR_TYPE)),
                        ),
                    ],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(142, vec![], SemanticTerminatorKindV1::Return),
                block(143, vec![], SemanticTerminatorKindV1::Unreachable),
            ],
            locals,
        );
        let calls = vec![compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: SCALAR_TYPE,
                raw_index: SCALAR_TYPE,
            },
        )];
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        let availability = enums
            .availability(SemanticLocalIdV1::from_index(1), 1)
            .unwrap();
        for use_block in [2, 3] {
            let mut edges = vec![vec![]; function.locals().len()];
            edges[1].push(CapabilityEdgeV1 {
                destination: 3,
                use_block: 2,
                kind: CapabilityEdgeKindV1::Alias,
            });
            edges[4].push(CapabilityEdgeV1 {
                destination: 3,
                use_block: 2,
                kind: CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                    construction_block: 1,
                    availability,
                },
            });
            edges[3].push(CapabilityEdgeV1 {
                destination: 5,
                use_block,
                kind: CapabilityEdgeKindV1::Alias,
            });
            let input = Inputs {
                function: &function,
                calls: &calls,
                definitions: &definitions,
                escaped: &escaped,
                options: &options,
                enums: &enums,
                edges: &edges,
            };
            let (expected, state) = compare(&input, |_| {});
            assert_eq!(expected.is_ok(), use_block == 2);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut pending = PendingUnjoinedInvocationIndicesV1::new();
            let actual = paid(
                &input,
                &mut pending,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .map_err(|error| format!("{error:?}"));
            assert_eq!(actual, expected);
            assert_eq!(pending.payload_for_test().unwrap().indices, state.indices);
            assert_eq!(pending.completed_for_test(), use_block == 2);
            assert_eq!(
                pending.payload_for_test().unwrap().indices[3]
                    .unwrap()
                    .availability,
                Some(CapabilityAvailabilityV1::EnumPayload(availability))
            );
            drop(pending);
            budget.release_storage(owned).unwrap();
        }
    }

    #[test]
    fn paid_bounds_and_shape_refusals_occupy_the_owner_without_minting_any_values() {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        for which in 0..5 {
            let mut edges = graph(&function);
            if which == 2 {
                edges[1][0].destination = function.locals().len();
            }
            if which == 3 {
                edges[1][0].use_block = function.blocks().len();
            }
            if which == 4 {
                edges[1][0].kind = CapabilityEdgeKindV1::AuthenticatedOptionPayload;
            }
            let input = Inputs {
                function: &function,
                calls: if which == 1 { &[] } else { &calls },
                definitions: if which == 0 { &[] } else { &definitions },
                escaped: &escaped,
                options: &options,
                enums: &enums,
                edges: &edges,
            };
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut pending = PendingUnjoinedInvocationIndicesV1::new();
            assert!(
                paid(
                    &input,
                    &mut pending,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
            );
            assert!(!pending.completed_for_test());
            assert!(pending.payload_for_test().unwrap().operations.is_empty());
            assert!(pending.payload_for_test().unwrap().indices.is_empty());
            assert!(owned > 0);
            assert_eq!(
                (budget.failed_work(), budget.failed_storage()),
                (None, None)
            );
            assert!(
                paid(
                    &input,
                    &mut pending,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
            );
            drop(pending);
            budget.release_storage(owned).unwrap();
        }
    }

    #[test]
    fn ordinary_operation_cap_and_processed_overflow_preserve_prior_state() {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        for cap in [false, true] {
            let (result, state) = compare(&input, |state| {
                state.next_value = 81;
                if cap {
                    for _ in 0..MAX_PROJECTED_OPERATIONS_V1 {
                        state
                            .operations
                            .push(ProductionRankedOperationV1::IndexConstant {
                                result: ProductionRankedValueIdV1::new(80),
                                value: 17,
                            });
                    }
                } else {
                    state.processed = usize::MAX;
                }
            });
            let error = result.unwrap_err();
            if cap {
                assert!(error.contains("ranked operation limit"));
                assert_eq!(state.next_value, 81);
                assert_eq!(state.operations.len(), MAX_PROJECTED_OPERATIONS_V1);
                assert!(state.indices.iter().all(Option::is_none));
            } else {
                assert!(error.contains("capability work accounting overflowed"));
                assert_eq!(state.next_value, 82);
                assert_eq!(state.operations.len(), 1);
                assert_eq!(state.processed, usize::MAX);
                assert!(state.indices[1].is_some());
                assert!(state.indices[2].is_none());
            }
            assert!(state.index_queue.is_empty());
        }
    }

    #[test]
    fn ordinary_checked_predicate_refusal_occurs_after_transform_emission() {
        let (function, producers) = option_dominance_chain(1);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &producers).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let availability = options
            .availability(SemanticLocalIdV1::from_index(1))
            .unwrap();
        let calls = vec![compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: SCALAR_TYPE,
                raw_index: SCALAR_TYPE,
            },
        )];
        let mut edges = vec![vec![]; function.locals().len()];
        edges[1].push(CapabilityEdgeV1 {
            destination: 2,
            use_block: 2,
            kind: CapabilityEdgeKindV1::CheckedShift {
                mapping: SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 7 },
                offset: 7,
                availability,
            },
        });
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        let (result, state) = compare(&input, |state| {
            state.predicates[2] = Some(GuardPredicateV1 {
                comparisons: vec![],
            });
        });
        assert!(result.unwrap_err().contains("multiple checked predicates"));
        assert_eq!(state.operations.len(), 4);
        assert_eq!(state.next_value, 4);
        assert_eq!(state.processed, 1);
        assert!(state.indices[2].is_none());
        assert!(state.index_queue.is_empty());
        assert!(state.text.contains("index_binary Add"));
    }
    // Exact pinned original body below. No calls to the extracted seed or paid helpers.
    struct FrozenScalarCustody<'a> {
        address_escaped: &'a [bool],
    }
    fn frozen_seed(input: &Inputs<'_>, state: &mut State) -> Outcome {
        let callables = input.calls;
        let function = input.function;
        let local_count = function.locals().len();
        let local_definitions = input.definitions;
        let scalar_inventory = FrozenScalarCustody {
            address_escaped: input.escaped,
        };
        let option_dominance = input.options;
        let index_values = &mut state.indices;
        let grid_leaders = &mut state.grids;
        let option_predicates = &mut state.predicates;
        let index_worklist = &mut state.index_queue;
        let grid_worklist = &mut state.grid_queue;
        let launch_extent = 0;
        let operations = &mut state.operations;
        let next_value = &mut state.next_value;
        let ranked_ir = &mut state.text;
        // FROZEN-SEED-BEGIN
        for block in function.blocks() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
                callables.get(call.callee().index() as usize)
            else {
                continue;
            };
            if !matches!(
                operation,
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                    | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
            ) {
                continue;
            }
            let destination = simple_call_destination(call)?;
            let destination = destination.index() as usize;
            if destination >= local_count {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an invocation-capability destination outside the semantic local table",
                ));
            }
            if index_values[destination].is_some() || grid_leaders[destination].is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "multiple invocation capabilities for one semantic local",
                ));
            }
            reserve_operation(operations)?;
            let result = next_value_id(next_value)?;
            operations.push(ProductionRankedOperationV1::InvocationIndex {
                result,
                dimension: 0,
                launch_extent,
            });
            push_ranked_ir(
                ranked_ir,
                &format!(
                    "  %{} = kernel.invocation_index <0, dynamic>\n",
                    result.get()
                ),
            )?;
            match operation {
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. } => {
                    require_index_scalar_custody_v1(
                        destination,
                        &local_definitions,
                        &scalar_inventory.address_escaped,
                    )?;
                    index_values[destination] = Some(ProjectedDisjointIndexV1 {
                        value: ProductionRankedValueV1::Local(result),
                        mapping: SemanticDisjointIndexSpaceV1::Index1d,
                        precondition: None,
                        availability: None,
                    });
                    index_worklist.push_back(destination);
                }
                SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader } => {
                    let availability = option_dominance
                        .availability(SemanticLocalIdV1::from_index(destination as u32))
                        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                            "a grid-leader capability lacks authenticated Option Some availability",
                        ))?;
                    reserve_operation(operations)?;
                    let one = next_value_id(next_value)?;
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: one,
                        value: 1,
                    });
                    push_ranked_ir(
                        ranked_ir,
                        &format!("  %{} = kernel.index_constant 1\n", one.get()),
                    )?;
                    option_predicates[destination] = Some(GuardPredicateV1 {
                        comparisons: vec![(
                            ProductionRankedValueV1::Local(result),
                            ProductionRankedValueV1::Local(one),
                        )],
                    });
                    grid_leaders[destination] = Some(ProjectedGridLeaderV1 {
                        grid_leader: *grid_leader,
                        precondition: (
                            ProductionRankedValueV1::Local(result),
                            ProductionRankedValueV1::Local(one),
                        ),
                        availability: CapabilityAvailabilityV1::Option(availability),
                    });
                    grid_worklist.push_back(destination);
                }
                _ => unreachable!(),
            }
        }

        // FROZEN-SEED-END
        Ok(())
    }

    fn frozen_propagate(input: &Inputs<'_>, state: &mut State) -> Outcome {
        let local_definitions = input.definitions;
        let scalar_inventory = FrozenScalarCustody {
            address_escaped: input.escaped,
        };
        let option_dominance = input.options;
        let enum_payload_dominance = input.enums;
        let edges_by_source = input.edges;
        let mut index_values = &mut state.indices;
        let grid_leaders = &state.grids;
        let option_predicates = &mut state.predicates;
        let mut index_worklist = &mut state.index_queue;
        let operations = &mut state.operations;
        let next_value = &mut state.next_value;
        let ranked_ir = &mut state.text;
        let mut processed_edges = state.processed;
        let result = (|| -> Outcome {
            // FROZEN-PROPAGATION-BEGIN
            while let Some(source) = index_worklist.pop_front() {
                require_index_scalar_custody_v1(
                    source,
                    &local_definitions,
                    &scalar_inventory.address_escaped,
                )?;
                let input =
                    index_values[source].ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "the capability worklist lost an index value",
                    ))?;
                for edge in &edges_by_source[source] {
                    processed_edges = processed_edges.checked_add(1).ok_or(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "capability work accounting overflowed",
                        ),
                    )?;
                    let authorization_block = match edge.kind {
                        CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                            construction_block,
                            ..
                        } => construction_block,
                        _ => edge.use_block,
                    };
                    if !input.availability.is_none_or(|availability| {
                        capability_availability_allows(
                            &option_dominance,
                            &enum_payload_dominance,
                            availability,
                            SemanticBlockIdV1::from_index(authorization_block as u32),
                        )
                    }) {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "an index capability is used outside its authenticated Some edge",
                        ));
                    }
                    let projected = match edge.kind {
                        CapabilityEdgeKindV1::Alias
                        | CapabilityEdgeKindV1::AuthenticatedOptionPayload => input,
                        CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } => {
                            ProjectedDisjointIndexV1 {
                                availability: Some(CapabilityAvailabilityV1::EnumPayload(
                                    availability,
                                )),
                                ..input
                            }
                        }
                        CapabilityEdgeKindV1::IntoDisjoint { mapping } => {
                            ProjectedDisjointIndexV1 { mapping, ..input }
                        }
                        CapabilityEdgeKindV1::CheckedShift {
                            mapping,
                            offset,
                            availability,
                        } => {
                            reserve_operation(operations)?;
                            let offset_value = next_value_id(next_value)?;
                            operations.push(ProductionRankedOperationV1::IndexConstant {
                                result: offset_value,
                                value: offset,
                            });
                            reserve_operation(operations)?;
                            let shifted = next_value_id(next_value)?;
                            operations.push(ProductionRankedOperationV1::IndexBinary {
                                result: shifted,
                                kind: IndexBinaryKindAttr::Add,
                                lhs: input.value,
                                rhs: ProductionRankedValueV1::Local(offset_value),
                            });
                            push_ranked_ir(
                                ranked_ir,
                                &format!(
                                    "  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Add {}, %{}\n",
                                    offset_value.get(),
                                    offset,
                                    shifted.get(),
                                    ranked_value_text_v1(input.value),
                                    offset_value.get(),
                                ),
                            )?;
                            let precondition = if offset == 0 {
                                input.precondition
                            } else {
                                reserve_operation(operations)?;
                                let upper = next_value_id(next_value)?;
                                operations.push(ProductionRankedOperationV1::IndexConstant {
                                    result: upper,
                                    value: u64::MAX - offset + 1,
                                });
                                push_ranked_ir(
                                    ranked_ir,
                                    &format!(
                                        "  %{} = kernel.index_constant {}\n",
                                        upper.get(),
                                        u64::MAX - offset + 1,
                                    ),
                                )?;
                                Some((input.value, ProductionRankedValueV1::Local(upper)))
                            };
                            ProjectedDisjointIndexV1 {
                                value: ProductionRankedValueV1::Local(shifted),
                                mapping,
                                precondition,
                                availability: Some(CapabilityAvailabilityV1::Option(availability)),
                            }
                        }
                        CapabilityEdgeKindV1::CheckedBlock {
                            mapping,
                            lanes_per_block,
                            elements_per_lane,
                            availability,
                        } => {
                            if lanes_per_block == 1 {
                                let maximum_raw =
                                    (u64::MAX - (elements_per_lane - 1)) / elements_per_lane;
                                reserve_operation(operations)?;
                                let upper = next_value_id(next_value)?;
                                operations.push(ProductionRankedOperationV1::IndexConstant {
                                    result: upper,
                                    value: maximum_raw + 1,
                                });
                                push_ranked_ir(
                                    ranked_ir,
                                    &format!(
                                        "  %{} = kernel.index_constant {}\n",
                                        upper.get(),
                                        maximum_raw + 1,
                                    ),
                                )?;
                                ProjectedDisjointIndexV1 {
                                    mapping,
                                    precondition: Some((
                                        input.value,
                                        ProductionRankedValueV1::Local(upper),
                                    )),
                                    availability: Some(CapabilityAvailabilityV1::Option(
                                        availability,
                                    )),
                                    ..input
                                }
                            } else {
                                ProjectedDisjointIndexV1 {
                                    mapping,
                                    availability: Some(CapabilityAvailabilityV1::Option(
                                        availability,
                                    )),
                                    ..input
                                }
                            }
                        }
                        CapabilityEdgeKindV1::CheckedTiled2d {
                            mapping,
                            availability,
                        } => ProjectedDisjointIndexV1 {
                            mapping,
                            availability: Some(CapabilityAvailabilityV1::Option(availability)),
                            ..input
                        },
                        CapabilityEdgeKindV1::CheckedRowStriped2d {
                            mapping,
                            availability,
                        } => ProjectedDisjointIndexV1 {
                            mapping,
                            availability: Some(CapabilityAvailabilityV1::Option(availability)),
                            ..input
                        },
                    };
                    if matches!(
                        edge.kind,
                        CapabilityEdgeKindV1::CheckedShift { .. }
                            | CapabilityEdgeKindV1::CheckedBlock { .. }
                            | CapabilityEdgeKindV1::CheckedTiled2d { .. }
                            | CapabilityEdgeKindV1::CheckedRowStriped2d { .. }
                    ) {
                        let predicate = option_predicates.get_mut(edge.destination).ok_or(
                            ProductionRankedProjectionErrorV1::Unsupported(
                                "a checked capability destination outside the semantic local table",
                            ),
                        )?;
                        if predicate.is_some() {
                            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                                "multiple checked predicates for one semantic local",
                            ));
                        }
                        *predicate =
                            Some(GuardPredicateV1::from_precondition(projected.precondition));
                    }
                    frozen_assign_index_capability(
                        edge.destination,
                        projected,
                        &mut index_values,
                        &grid_leaders,
                        &mut index_worklist,
                    )?;
                }
            }

            // FROZEN-PROPAGATION-END
            Ok(())
        })();
        state.processed = processed_edges;
        result
    }
    // FROZEN-ASSIGN-BEGIN
    fn frozen_assign_index_capability(
        destination: usize,
        projected: ProjectedDisjointIndexV1,
        index_values: &mut [Option<ProjectedDisjointIndexV1>],
        grid_leaders: &[Option<ProjectedGridLeaderV1>],
        worklist: &mut VecDeque<usize>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if destination >= index_values.len() || grid_leaders[destination].is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an index capability escaped the semantic local table or changed capability kind",
            ));
        }
        match index_values[destination] {
            None => {
                index_values[destination] = Some(projected);
                worklist.push_back(destination);
            }
            Some(existing) if existing == projected => {}
            Some(_) => {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "multiple index capabilities reach one semantic local",
                ));
            }
        }
        Ok(())
    }

    // FROZEN-ASSIGN-END
    // New actual-namespace component controls use the existing inert fixture and
    // frozen ordinary oracle, not an invented source owner or ready constructor.
    #[test]
    fn actual_namespace_index_component_preserves_nonzero_prefix_and_all_frozen_rows() {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let input = Inputs {
            function: &function,
            calls: &calls,
            definitions: &definitions,
            escaped: &escaped,
            options: &options,
            enums: &enums,
            edges: &edges,
        };
        let prior = ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(80),
            value: 17,
        };
        let mut expected = State::new(function.locals().len());
        expected.operations.push(prior.clone());
        expected.next_value = 81;
        frozen_seed(&input, &mut expected).unwrap();
        frozen_propagate(&input, &mut expected).unwrap();
        let mut operations = vec![prior];
        let mut next = 81;
        let mut rows = RootInvocationIndexStorageV1::empty();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(1024).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut owned = 0;
        prepare_root_namespace_indices_v1(
            &calls,
            &function,
            &definitions,
            &escaped,
            &options,
            &enums,
            &edges,
            4,
            &mut rows,
            &mut operations,
            &mut next,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        assert_eq!(operations, expected.operations);
        assert_eq!(next, expected.next_value);
        assert_eq!(rows.indices, expected.indices);
        assert_eq!(format!("{:?}", rows.grids), format!("{:?}", expected.grids));
        assert_eq!(
            format!("{:?}", rows.predicates),
            format!("{:?}", expected.predicates)
        );
        assert_eq!(rows.index_fifo, vec![1, 2, 3, 4, 5]);
        assert_eq!(rows.index_cursor, 5);
        assert_eq!(rows.processed_edges, expected.processed);
        assert!(rows.grid_fifo.is_empty());
        assert!(budget.work_ledger_identity_v1() == identity);
        assert!(budget.storage() >= 1024 + owned);
        drop(rows);
        drop(operations);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 1024);
    }
    #[test]
    fn actual_namespace_component_count_mismatch_occupies_without_reseeding_or_emitting() {
        let (function, calls) = fixture(&[1]);
        let definitions = local_definition_counts(&function);
        let escaped = vec![false; function.locals().len()];
        let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let enums =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let edges = graph(&function);
        let mut operations = vec![ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(80),
            value: 17,
        }];
        let mut next = 81;
        let mut rows = RootInvocationIndexStorageV1::empty();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let wrong = prepare_root_namespace_indices_v1(
            &calls,
            &function,
            &definitions,
            &escaped,
            &options,
            &enums,
            &edges,
            5,
            &mut rows,
            &mut operations,
            &mut next,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert!(matches!(
            wrong,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "actual complete graph edge storage/count differs"
            ))
        ));
        assert_eq!(operations.len(), 1);
        assert_eq!(next, 81);
        assert!(rows.indices.is_empty());
        let retry = prepare_root_namespace_indices_v1(
            &calls,
            &function,
            &definitions,
            &escaped,
            &options,
            &enums,
            &edges,
            4,
            &mut rows,
            &mut operations,
            &mut next,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert!(matches!(
            retry,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "actual root index preparation cannot be replaced or retried"
            ))
        ));
        assert_eq!(operations.len(), 1);
        assert_eq!(next, 81);
        drop(rows);
        drop(operations);
        budget.release_storage(owned).unwrap();
    }
}
