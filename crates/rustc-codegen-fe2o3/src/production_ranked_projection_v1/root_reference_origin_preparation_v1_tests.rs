// Inert component fixtures only: no nominal owner or actual GuardedAccess roster.
mod reference_origin_preparation_controls {
    use super::super::bf16_nominal_preparation_resources_v1::PreparationResourcesV1;
    use super::super::root_reference_origin_preparation_v1::*;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};
    const LIMIT: usize = 16 * 1024 * 1024;
    const FLOOR: usize = 37;

    fn shared_statement(
        destination: u32,
        transparent: usize,
    ) -> fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1 {
        let mut projections =
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE)
                    .unwrap();
                transparent
            ];
        projections.push(
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 0,
                    minimum_length: 4,
                    from_end: false,
                },
                SCALAR_TYPE,
            )
            .unwrap(),
        );
        typed_assignment(
            destination,
            POINTER_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(0),
                    projections,
                    SCALAR_TYPE,
                )
                .unwrap(),
            },
        )
    }
    fn shared_function(transparent: usize) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![block(
                61,
                vec![
                    shared_statement(1, transparent),
                    typed_assignment(
                        2,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(typed_operand(1, POINTER_TYPE)),
                    ),
                    typed_assignment(
                        3,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(typed_operand(2, POINTER_TYPE)),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            (0..4)
                .map(|i| {
                    local(
                        61 + i,
                        if i == 0 { SCALAR_TYPE } else { POINTER_TYPE },
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
    fn aliases(function: &SemanticFunctionDeclV1) -> Vec<Vec<CapabilityEdgeV1>> {
        let mut graph = vec![Vec::new(); function.locals().len()];
        graph[1].push(CapabilityEdgeV1 {
            destination: 2,
            use_block: 0,
            kind: CapabilityEdgeKindV1::Alias,
        });
        graph[2].push(CapabilityEdgeV1 {
            destination: 3,
            use_block: 0,
            kind: CapabilityEdgeKindV1::AuthenticatedOptionPayload,
        });
        graph
    }
    fn guarded(
        with_shared: bool,
    ) -> (
        SemanticFunctionDeclV1,
        Vec<SemanticOptionProducerV1>,
        Vec<SemanticCallableDeclV1>,
    ) {
        let (base, producers) = option_dominance_chain(2);
        let function = if with_shared {
            let mut blocks = base.blocks().to_vec();
            blocks[0] = block(
                0,
                vec![shared_statement(5, 0)],
                base.blocks()[0].terminator().kind().clone(),
            );
            let mut locals = base.locals().to_vec();
            locals.push(local(6, POINTER_TYPE, SemanticLocalRoleV1::Temporary));
            projection_function_with_locals(blocks, locals)
        } else {
            base
        };
        let calls = vec![compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                disjoint_slice: POINTER_TYPE,
                index_witness: SCALAR_TYPE,
                element: SCALAR_TYPE,
                raw_index: SCALAR_TYPE,
            },
        )];
        (function, producers, calls)
    }
    fn tables(
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
    ) -> (SemanticOptionDominanceV1, SemanticEnumPayloadDominanceV1) {
        (
            SemanticOptionDominanceV1::analyze(function, producers).unwrap(),
            SemanticEnumPayloadDominanceV1::analyze(function, &projection_types()).unwrap(),
        )
    }

    #[test]
    fn ordinary_origins_match_frozen_algorithm_and_source_order() {
        let function = shared_function(0);
        let graph = aliases(&function);
        let (options, enums) = tables(&function, &[]);
        let old =
            frozen_checked_reference_origins(&function, &[], 0, &graph, &options, &enums).unwrap();
        let new = checked_reference_origins(&function, &[], 0, &graph, &options, &enums).unwrap();
        assert_eq!(old, new);
        assert_eq!(old.capacity(), new.capacity());
        for index in 1..4 {
            assert_eq!(
                new[index].unwrap().source,
                CheckedReferenceSourceV1::ProjectedSharedBorrow
            );
        }
        let (function, producers, calls) = guarded(true);
        let graph = vec![Vec::new(); function.locals().len()];
        let (options, enums) = tables(&function, &producers);
        let old = frozen_checked_reference_origins(&function, &calls, 2, &graph, &options, &enums)
            .unwrap();
        let new =
            checked_reference_origins(&function, &calls, 2, &graph, &options, &enums).unwrap();
        assert_eq!(old, new);
        assert_eq!(
            new[1].unwrap().source,
            CheckedReferenceSourceV1::GuardedAccess(0)
        );
        assert_eq!(
            new[3].unwrap().source,
            CheckedReferenceSourceV1::GuardedAccess(1)
        );
        assert_eq!(
            new[5].unwrap().source,
            CheckedReferenceSourceV1::ProjectedSharedBorrow
        );
    }

    #[test]
    fn paid_fifo_seeds_shared_borrows_before_source_order_guarded_calls() {
        let (function, producers, calls) = guarded(true);
        let (options, enums) = tables(&function, &producers);
        let graph = vec![Vec::new(); function.locals().len()];
        let expected =
            frozen_checked_reference_origins(&function, &calls, 2, &graph, &options, &enums)
                .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut pending = PendingUnjoinedReferenceOriginsV1::new();
        prepare_unjoined_reference_origins_v1(
            &function,
            &calls,
            2,
            &graph,
            &options,
            &enums,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        let payload = pending.payload_for_test().unwrap();
        assert_eq!(payload.origins, expected);
        assert_eq!(payload.fifo, vec![5, 1, 3]);
        assert_eq!(payload.cursor, 3);
        assert!(pending.completed_for_test());
        assert_eq!(budget.storage(), FLOOR + owned);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }

    #[test]
    fn inventory_mismatch_precedes_fifo_and_fifo_preserves_first_semantic_refusal() {
        let (function, producers, calls) = guarded(true);
        let (options, enums) = tables(&function, &producers);
        let mut graph = vec![Vec::new(); function.locals().len()];
        graph[5].push(CapabilityEdgeV1 {
            destination: 999,
            use_block: 0,
            kind: CapabilityEdgeKindV1::Alias,
        });
        graph[1].push(CapabilityEdgeV1 {
            destination: 0,
            use_block: 0,
            kind: CapabilityEdgeKindV1::Alias,
        });
        for expected_count in [1, 2] {
            let old = frozen_checked_reference_origins(
                &function,
                &calls,
                expected_count,
                &graph,
                &options,
                &enums,
            );
            let new = checked_reference_origins(
                &function,
                &calls,
                expected_count,
                &graph,
                &options,
                &enums,
            );
            assert_eq!(format!("{old:?}"), format!("{new:?}"));
            let reason = if expected_count == 1 {
                "checked disjoint access inventory changed during projection"
            } else {
                "a checked reference destination without one exact definition"
            };
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut pending = PendingUnjoinedReferenceOriginsV1::new();
            let result = prepare_unjoined_reference_origins_v1(
                &function,
                &calls,
                expected_count,
                &graph,
                &options,
                &enums,
                &mut pending,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            );
            assert!(
                matches!(&result, Err(ProductionRankedProjectionErrorV1::Incomplete(s)
                | ProductionRankedProjectionErrorV1::Unsupported(s)) if *s == reason)
            );
            let payload = pending.payload_for_test().unwrap();
            assert_eq!(payload.fifo, vec![5, 1, 3]);
            assert_eq!(payload.cursor, usize::from(expected_count == 2));
            assert!(!pending.completed_for_test());
            drop(result);
            drop(pending);
            budget.release_storage(owned).unwrap();
        }
    }

    #[test]
    fn availability_refusal_precedes_destination_count_and_conflicting_origins_are_retained() {
        let (function, producers, calls) = guarded(false);
        let (options, enums) = tables(&function, &producers);
        for hostile in [false, true] {
            let mut graph = vec![Vec::new(); function.locals().len()];
            graph[1].push(CapabilityEdgeV1 {
                destination: if hostile { 999 } else { 2 },
                use_block: if hostile { 0 } else { 4 },
                kind: CapabilityEdgeKindV1::Alias,
            });
            if !hostile {
                graph[3].push(CapabilityEdgeV1 {
                    destination: 2,
                    use_block: 4,
                    kind: CapabilityEdgeKindV1::Alias,
                });
            }
            let old =
                frozen_checked_reference_origins(&function, &calls, 2, &graph, &options, &enums);
            let new = checked_reference_origins(&function, &calls, 2, &graph, &options, &enums);
            assert_eq!(format!("{old:?}"), format!("{new:?}"));
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut pending = PendingUnjoinedReferenceOriginsV1::new();
            let result = prepare_unjoined_reference_origins_v1(
                &function,
                &calls,
                2,
                &graph,
                &options,
                &enums,
                &mut pending,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            );
            assert_eq!(format!("{new:?}"), format!("{result:?}"));
            if !hostile {
                assert_eq!(pending.payload_for_test().unwrap().fifo, vec![1, 3, 2]);
                assert_eq!(pending.payload_for_test().unwrap().cursor, 2);
            }
            drop(pending);
            budget.release_storage(owned).unwrap();
        }
    }

    fn definition_function() -> SemanticFunctionDeclV1 {
        let project = |local| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64_TYPE)
                        .unwrap(),
                ],
                U64_TYPE,
            )
            .unwrap()
        };
        let deref = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(6),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U64_TYPE).unwrap(),
            ],
            U64_TYPE,
        )
        .unwrap();
        let statements = vec![
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                project(1),
                typed_constant(U64_TYPE, 1, 8),
                SemanticVolatilityV1::NonVolatile,
                None,
            ))),
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                project(2),
                SemanticRvalueV1::new(
                    U64_TYPE,
                    SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 1, 8)),
                ),
            ))),
            statement(SemanticStatementKindV1::AtomicRmw(
                SemanticAtomicRmwV1::new(
                    typed_place(3, U64_TYPE),
                    typed_place(3, U64_TYPE),
                    typed_constant(U64_TYPE, 1, 8),
                    SemanticAtomicRmwOpV1::Exchange,
                    atomic_access(),
                ),
            )),
            statement(SemanticStatementKindV1::AtomicCompareExchange(
                SemanticAtomicCompareExchangeV1::new(
                    typed_place(4, U64_TYPE),
                    typed_place(4, U64_TYPE),
                    typed_constant(U64_TYPE, 0, 8),
                    typed_constant(U64_TYPE, 1, 8),
                    atomic_access(),
                    SemanticAtomicOrderingV1::Relaxed,
                    false,
                ),
            )),
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: project(5),
                variant_index: 0,
            }),
            statement(SemanticStatementKindV1::Deinitialize(typed_place(
                5, U64_TYPE,
            ))),
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                deref,
                typed_constant(U64_TYPE, 1, 8),
                SemanticVolatilityV1::NonVolatile,
                None,
            ))),
            statement(SemanticStatementKindV1::Nop),
        ];
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                project(6),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        projection_function_with_locals(
            vec![
                block(70, statements, SemanticTerminatorKindV1::Call(call)),
                block(71, vec![], SemanticTerminatorKindV1::Return),
            ],
            (0..7)
                .map(|i| {
                    local(
                        70 + i,
                        U64_TYPE,
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

    #[test]
    fn exact_definition_visitor_counts_projected_stores_atomics_discriminants_and_calls() {
        let function = definition_function();
        let old = frozen_local_definition_counts(&function);
        let new = local_definition_counts(&function);
        assert_eq!(new, vec![0, 1, 1, 2, 2, 2, 1]);
        assert_eq!(new, old);
        assert_eq!(new.capacity(), old.capacity());
        let scalar = assertion_definition_inventory(&function).unwrap();
        // Actual source correction: scalar COUNTS share the same visitor;
        // assignment-site and deduplicated block metadata are different facts.
        assert_eq!(scalar.counts, new);
        assert!(scalar.assignments.iter().all(Option::is_none));
        assert_eq!(scalar.blocks[0].iter().filter(|&&id| id == 3).count(), 1);
        let (options, enums) = tables(&function, &[]);
        let graph = vec![Vec::new(); function.locals().len()];
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut pending = PendingUnjoinedReferenceOriginsV1::new();
        prepare_unjoined_reference_origins_v1(
            &function,
            &[],
            0,
            &graph,
            &options,
            &enums,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        assert_eq!(pending.payload_for_test().unwrap().definitions, old);
        drop(pending);
        budget.release_storage(owned).unwrap();
    }

    #[test]
    fn exact_definition_counts_saturate_and_preserve_inventory_failure_domain_difference() {
        let function = projection_function_with_locals(
            vec![block(
                74,
                (0..260)
                    .map(|_| {
                        typed_assignment(
                            1,
                            U64_TYPE,
                            SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 0, 8)),
                        )
                    })
                    .collect(),
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(74, U64_TYPE, SemanticLocalRoleV1::Return),
                local(75, U64_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        assert_eq!(
            local_definition_counts(&function),
            frozen_local_definition_counts(&function)
        );
        assert_eq!(local_definition_counts(&function), vec![0, 255]);
        assert_eq!(
            assertion_definition_inventory(&function).unwrap().counts,
            vec![0, 255]
        );
        let (options, enums) = tables(&function, &[]);
        let graph = vec![Vec::new(); function.locals().len()];
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut pending = PendingUnjoinedReferenceOriginsV1::new();
        prepare_unjoined_reference_origins_v1(
            &function,
            &[],
            0,
            &graph,
            &options,
            &enums,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        assert_eq!(
            pending.payload_for_test().unwrap().definitions,
            vec![0, 255]
        );
        drop(pending);
        budget.release_storage(owned).unwrap();
        let malformed = projection_function_with_locals(
            vec![block(
                76,
                vec![typed_assignment(
                    99,
                    U64_TYPE,
                    SemanticRvalueKindV1::Use(typed_constant(U64_TYPE, 0, 8)),
                )],
                SemanticTerminatorKindV1::Return,
            )],
            vec![local(76, U64_TYPE, SemanticLocalRoleV1::Return)],
        );
        assert_eq!(
            local_definition_counts(&malformed),
            frozen_local_definition_counts(&malformed)
        );
        assert_eq!(local_definition_counts(&malformed), vec![0]);
        assert!(matches!(
            assertion_definition_inventory(&malformed),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an assertion proof assignment is outside the semantic local table"
            ))
        ));
    }

    #[test]
    fn guarded_definition_refusal_precedes_missing_availability_and_inventory_mismatch() {
        let (base, _producers, calls) = guarded(false);
        for duplicate in [true, false] {
            let function = if duplicate {
                let mut blocks = base.blocks().to_vec();
                blocks[0] = block(
                    0,
                    vec![typed_assignment(
                        1,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(typed_constant(POINTER_TYPE, 0, 8)),
                    )],
                    base.blocks()[0].terminator().kind().clone(),
                );
                projection_function_with_locals(blocks, base.locals().to_vec())
            } else {
                base.clone()
            };
            let (options, enums) = tables(&function, &[]);
            let graph = vec![Vec::new(); function.locals().len()];
            let expected = if duplicate {
                "a checked disjoint result without one exact definition"
            } else {
                "a checked disjoint result without exact Option Some availability"
            };
            let old =
                frozen_checked_reference_origins(&function, &calls, 999, &graph, &options, &enums);
            let new = checked_reference_origins(&function, &calls, 999, &graph, &options, &enums);
            assert_eq!(format!("{old:?}"), format!("{new:?}"));
            assert!(
                matches!(new, Err(ProductionRankedProjectionErrorV1::Incomplete(s)) if s == expected)
            );
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let mut pending = PendingUnjoinedReferenceOriginsV1::new();
            let result = prepare_unjoined_reference_origins_v1(
                &function,
                &calls,
                999,
                &graph,
                &options,
                &enums,
                &mut pending,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            );
            assert!(
                matches!(&result, Err(ProductionRankedProjectionErrorV1::Incomplete(s)) if *s == expected)
            );
            assert!(pending.payload_for_test().unwrap().fifo.is_empty());
            assert!(!pending.completed_for_test());
            assert_eq!(budget.storage(), owned);
            assert_eq!(
                (budget.failed_work(), budget.failed_storage()),
                (None, None)
            );
            drop(result);
            drop(pending);
            budget.release_storage(owned).unwrap();
        }
    }

    #[test]
    fn unmetered_adapter_cannot_prepare_paid_unjoined_payload() {
        let function = shared_function(0);
        let graph = aliases(&function);
        let (options, enums) = tables(&function, &[]);
        let mut pending = PendingUnjoinedReferenceOriginsV1::new();
        let result = prepare_unjoined_reference_origins_v1(
            &function,
            &[],
            0,
            &graph,
            &options,
            &enums,
            &mut pending,
            &mut PreparationResourcesV1::unmetered(),
        );
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "unjoined reference data requires an original ledger"
            ))
        ));
        assert!(
            pending.payload_for_test().is_none()
                && !pending.has_ledger_for_test()
                && !pending.completed_for_test()
        );
    }

    fn measured(
        transparent: usize,
        work_limit: usize,
        storage_limit: usize,
    ) -> (bool, usize, usize, bool, bool) {
        let function = shared_function(transparent);
        let graph = aliases(&function);
        let (options, enums) = tables(&function, &[]);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let mut pending = PendingUnjoinedReferenceOriginsV1::new();
        let result = prepare_unjoined_reference_origins_v1(
            &function,
            &[],
            0,
            &graph,
            &options,
            &enums,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        let observation = (
            result.is_ok(),
            budget.work(),
            budget.peak_storage(),
            budget.failed_work().is_some(),
            budget.failed_storage().is_some(),
        );
        assert_eq!(budget.storage(), FLOOR + owned);
        drop(result);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        observation
    }
    #[test]
    fn paid_reference_work_storage_boundaries_and_projection_spines_are_charged() {
        let passed = measured(0, LIMIT, LIMIT);
        assert!(passed.0);
        assert!(measured(0, passed.1, passed.2).0);
        assert!(measured(0, passed.1 - 1, LIMIT).3);
        assert!(measured(0, LIMIT, passed.2 - 1).4);
        assert!(measured(0, 0, LIMIT).3);
        assert!(measured(0, LIMIT, FLOOR).4);
        let projected = measured(17, LIMIT, LIMIT);
        assert!(projected.0 && projected.1 >= passed.1 + 17 * 4);
    }

    #[test]
    fn paid_reference_outer_lifetime_survives_error_panic_and_preserves_foreign_surplus() {
        for panics in [false, true] {
            let function = shared_function(0);
            let graph = aliases(&function);
            let (options, enums) = tables(&function, &[]);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let mut owned = 0;
            let mut pending = PendingUnjoinedReferenceOriginsV1::new();
            let result = catch_unwind(AssertUnwindSafe(|| {
                prepare_unjoined_reference_origins_v1(
                    &function,
                    &[],
                    0,
                    &graph,
                    &options,
                    &enums,
                    &mut pending,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned),
                )?;
                if panics {
                    panic!("reference preparation component panic");
                }
                Err::<(), _>(ProductionRankedProjectionErrorV1::Incomplete(
                    "reference preparation component refusal",
                ))
            }));
            assert!(
                pending.completed_for_test() && pending.payload_for_test().unwrap().fifo.len() == 3
            );
            assert_eq!(budget.storage(), FLOOR + owned);
            budget.reserve_storage(23).unwrap();
            drop(result);
            drop(pending);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), FLOOR + 23);
        }
    }

    #[test]
    fn occupied_pending_refuses_retry_and_foreign_ledger_without_replacing_payload() {
        let function = shared_function(0);
        let graph = aliases(&function);
        let (options, enums) = tables(&function, &[]);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut pending = PendingUnjoinedReferenceOriginsV1::new();
        prepare_unjoined_reference_origins_v1(
            &function,
            &[],
            0,
            &graph,
            &options,
            &enums,
            &mut pending,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        let before = owned;
        assert!(
            prepare_unjoined_reference_origins_v1(
                &function,
                &[],
                0,
                &graph,
                &options,
                &enums,
                &mut pending,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        assert_eq!(owned, before);
        let mut other_work = Work::new(LIMIT);
        let mut other_budget = Budget::new(&mut other_work, LIMIT);
        let mut other_owned = 0;
        assert!(
            prepare_unjoined_reference_origins_v1(
                &function,
                &[],
                0,
                &graph,
                &options,
                &enums,
                &mut pending,
                &mut PreparationResourcesV1::new(&mut other_budget, &mut other_owned)
            )
            .is_err()
        );
        assert_eq!(other_owned, 0);
        assert_eq!(pending.payload_for_test().unwrap().fifo, vec![1, 2, 3]);
        drop(pending);
        budget.release_storage(owned).unwrap();
    }

    // Frozen pre-extraction algorithms, renamed only. Not new helper calls.
    fn frozen_checked_reference_origins(
        function: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
        guarded_access_count: usize,
        edges_by_source: &[Vec<CapabilityEdgeV1>],
        option_dominance: &SemanticOptionDominanceV1,
        enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    ) -> Result<Vec<Option<CheckedReferenceOriginV1>>, ProductionRankedProjectionErrorV1> {
        let definitions = frozen_local_definition_counts(function);
        let mut origins = vec![None; function.locals().len()];
        let mut worklist = VecDeque::new();
        for block in function.blocks() {
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let destination = assignment.destination();
                if !destination.projections().is_empty()
                    || definitions
                        .get(destination.local().index() as usize)
                        .copied()
                        != Some(1)
                {
                    continue;
                }
                let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
                    continue;
                };
                if !matches!(kind, SemanticBorrowKindV1::Shared)
                    || !place.projections().iter().any(|projection| {
                        matches!(
                            projection.kind(),
                            SemanticProjectionKindV1::Index(_)
                                | SemanticProjectionKindV1::ConstantIndex { .. }
                        )
                    })
                {
                    continue;
                }
                let destination = destination.local().index() as usize;
                origins[destination] = Some(CheckedReferenceOriginV1 {
                    source: CheckedReferenceSourceV1::ProjectedSharedBorrow,
                    availability: None,
                });
                worklist.push_back(destination);
            }
        }

        let mut access = 0_usize;
        for block in function.blocks() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if !matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. }
                        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut { .. }
                        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut { .. },
                    ..
                })
            ) {
                continue;
            }
            let destination = simple_call_destination(call)?;
            if definitions.get(destination.index() as usize).copied() != Some(1) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked disjoint result without one exact definition",
                ));
            }
            let destination = destination.index() as usize;
            let availability = option_dominance
                .availability(SemanticLocalIdV1::from_index(destination as u32))
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked disjoint result without exact Option Some availability",
                ))?;
            if origins[destination].is_some() {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a checked disjoint result with a conflicting reference origin",
                ));
            }
            origins[destination] = Some(CheckedReferenceOriginV1 {
                source: CheckedReferenceSourceV1::GuardedAccess(access),
                availability: Some(CapabilityAvailabilityV1::Option(availability)),
            });
            worklist.push_back(destination);
            access += 1;
        }
        if access != guarded_access_count {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "checked disjoint access inventory changed during projection",
            ));
        }
        while let Some(source) = worklist.pop_front() {
            let Some(origin) = origins[source] else {
                continue;
            };
            let edges = edges_by_source.get(source).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "a checked reference source outside the capability graph",
                ),
            )?;
            for edge in edges {
                if !matches!(
                    edge.kind,
                    CapabilityEdgeKindV1::Alias
                        | CapabilityEdgeKindV1::AuthenticatedOptionPayload
                        | CapabilityEdgeKindV1::AuthenticatedEnumPayload { .. }
                ) {
                    continue;
                }
                let authorization_block = match edge.kind {
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                        construction_block, ..
                    } => construction_block,
                    _ => edge.use_block,
                };
                if !origin.availability.is_none_or(|availability| {
                    capability_availability_allows(
                        option_dominance,
                        enum_payload_dominance,
                        availability,
                        SemanticBlockIdV1::from_index(authorization_block as u32),
                    )
                }) {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a checked reference is transported outside its authenticated payload region",
                    ));
                }
                if definitions.get(edge.destination).copied() != Some(1) {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked reference destination without one exact definition",
                    ));
                }
                let projected = match edge.kind {
                    CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } => {
                        CheckedReferenceOriginV1 {
                            availability: Some(CapabilityAvailabilityV1::EnumPayload(availability)),
                            ..origin
                        }
                    }
                    _ => origin,
                };
                let slot = origins.get_mut(edge.destination).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a checked reference destination outside the semantic local table",
                    ),
                )?;
                if slot.is_none() {
                    *slot = Some(projected);
                    worklist.push_back(edge.destination);
                } else if *slot != Some(projected) {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked disjoint reference with conflicting origins",
                    ));
                }
            }
        }
        Ok(origins)
    }

    fn frozen_local_definition_counts(function: &SemanticFunctionDeclV1) -> Vec<u8> {
        let mut definitions = vec![0_u8; function.locals().len()];
        let mut record = |place: &SemanticPlaceV1| {
            if let Some(slot) =
                local_definition_index(place).and_then(|local| definitions.get_mut(local))
            {
                *slot = slot.saturating_add(1);
            }
        };
        for block in function.blocks() {
            for statement in block.statements() {
                visit_statement_definition_places(statement.kind(), &mut record);
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
            {
                record(destination.place());
            }
        }
        definitions
    }
}
