// Component-only controls. These fixtures grant no actual-source or bounds authority.
mod bounds_source_scan_controls {
    use super::super::root_bounds_source_scan_v1::{self as scan, BoundsSourceStorageV1};
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    const LIMIT: usize = 16 * 1024 * 1024;
    type Error = ProductionRankedProjectionErrorV1;
    type Original<'s> = (
        Vec<BoundsLocalDefinitionV1<'s>>,
        Vec<Vec<usize>>,
        Vec<Option<ProductionRankedValueV1>>,
        Vec<Option<ProductionRankedValueV1>>,
    );

    // BEGIN FROZEN ORIGINAL SCAN AND INITIALIZATION
    #[allow(unused_mut, unused_variables)]
    fn original<'s>(
        function: &'s SemanticFunctionDeclV1,
        first_argument: usize,
        known_indices: &[Option<ProjectedDisjointIndexV1>],
        ordinary_indices: &[Option<ProjectedOrdinaryIndexV1>],
    ) -> Result<Original<'s>, Error> {
        let mut definitions = vec![BoundsLocalDefinitionV1::default(); function.locals().len()];
        let mut argument_count = first_argument;
        let mut predecessors = vec![Vec::new(); function.blocks().len()];
        for (block_index, block) in function.blocks().iter().enumerate() {
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty() {
                    continue;
                }
                let definition = definitions
                    .get_mut(assignment.destination().local().index() as usize)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "a Rust bounds-check definition outside the semantic local table",
                    ))?;
                definition.count = definition.count.saturating_add(1);
                definition.value = Some(assignment.value());
                definition.length_source = match assignment.value().kind() {
                    SemanticRvalueKindV1::Length(place) => Some(
                        if place.projections().iter().any(|projection| {
                            matches!(projection.kind(), SemanticProjectionKindV1::Field(_))
                        }) {
                            ProjectedBoundsExtentSourceV1::CanonicalSlice
                        } else {
                            ProjectedBoundsExtentSourceV1::Slice(place.local())
                        },
                    ),
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand,
                    } => match operand {
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                            Some(if place.projections().is_empty() {
                                ProjectedBoundsExtentSourceV1::Slice(place.local())
                            } else {
                                ProjectedBoundsExtentSourceV1::CanonicalSlice
                            })
                        }
                        SemanticOperandV1::Constant(_) => None,
                    },
                    _ => None,
                };
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && destination.place().projections().is_empty()
            {
                let definition = definitions
                    .get_mut(destination.place().local().index() as usize)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "a Rust bounds-check call result outside the semantic local table",
                    ))?;
                definition.count = definition.count.saturating_add(1);
                definition.length_source = None;
                definition.value = None;
            }
            block
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionRankedProjectionErrorV1>(|edge| {
                    let target = edge.target().index() as usize;
                    let target_predecessors = predecessors.get_mut(target).ok_or(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "a Rust bounds-check CFG edge outside the semantic block table",
                        ),
                    )?;
                    target_predecessors.push(block_index);
                    Ok(())
                })?;
        }

        if !ordinary_indices.is_empty() && ordinary_indices.len() != function.locals().len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "ordinary intrinsic index facts do not match the semantic local table",
            ));
        }
        let mut local_values = if known_indices.is_empty() {
            vec![None; function.locals().len()]
        } else {
            if known_indices.len() != function.locals().len() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "intrinsic index facts do not match the semantic local table",
                ));
            }
            known_indices
                .iter()
                .map(|index| index.map(|index| index.value))
                .collect()
        };
        // Separate MIR `Len` temporaries for one stable slice describe one ranked extent.
        let mut slice_extents = vec![None; function.locals().len()];
        Ok((definitions, predecessors, local_values, slice_extents))
    }

    // END FROZEN ORIGINAL SCAN AND INITIALIZATION

    fn ordinary() -> SemanticFunctionDeclV1 {
        bounds_check_function(SemanticBinaryOpV1::LessThan, true, false, false)
    }
    fn place(local: u32, projections: Vec<SemanticProjectionV1>) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            projections,
            SCALAR_TYPE,
        )
        .unwrap()
    }
    fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(SCALAR_TYPE, value),
        )))
    }
    fn narrow(
        statements: Vec<SemanticStatementV1>,
        terminator: SemanticTerminatorKindV1,
    ) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(150, statements, terminator),
                block(151, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(150, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(151, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(152, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }
    fn equal_definition(
        actual: BoundsLocalDefinitionV1<'_>,
        expected: BoundsLocalDefinitionV1<'_>,
    ) {
        assert_eq!(actual.count, expected.count);
        assert_eq!(actual.length_source, expected.length_source);
        match (actual.value, expected.value) {
            (None, None) => (),
            (Some(a), Some(b)) => assert!(std::ptr::eq(a, b)),
            _ => panic!("definition source/value differs"),
        }
    }
    fn parity(function: &SemanticFunctionDeclV1, known: &[Option<ProjectedDisjointIndexV1>]) {
        let expected = original(function, 3, known, &[]).unwrap();
        let mut definitions = vec![BoundsLocalDefinitionV1::default(); function.locals().len()];
        let mut predecessors = vec![Vec::new(); function.blocks().len()];
        scan::scan_legacy_v1(function, &mut definitions, &mut predecessors).unwrap();
        let values = scan::initial_values_legacy_v1(function, known, &[]).unwrap();
        for (a, b) in definitions.iter().zip(&expected.0) {
            equal_definition(*a, *b);
        }
        assert_eq!(predecessors, expected.1);
        assert_eq!(values.0, expected.2);
        assert_eq!(values.1, expected.3);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut storage = BoundsSourceStorageV1::new();
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(function, &mut resources).unwrap();
            storage
                .initialize_values(function, known, &[], &mut resources)
                .unwrap();
            let view = storage.view(function, &mut resources).unwrap();
            for (local, row) in expected.0.iter().enumerate() {
                equal_definition(view.definition(local, &mut resources).unwrap(), *row);
            }
            for (block, row) in expected.1.iter().enumerate() {
                assert_eq!(view.predecessors(block, &mut resources).unwrap(), row);
            }
            assert_eq!(view.local_values(&mut resources).unwrap(), expected.2);
            assert_eq!(view.slice_extents(&mut resources).unwrap(), expected.3);
        }
        assert_eq!(budget.storage(), owned);
        drop(storage);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn bounds_scan_original_fixture_rows_match_both_routes() {
        parity(&ordinary(), &[]);
        parity(&repeated_bounds_check_function(true, false), &[]);
        parity(&repeated_bounds_check_function(false, true), &[]);
        parity(&literal_bounds_check_function(1, 4), &[]);
        parity(
            &branch_bounds_check_function(BranchBoundsCheckOptionsV1::default()),
            &[],
        );
        parity(&option_dominance_chain(3).0, &[]);
    }
    #[test]
    fn bounds_scan_known_slots_preserve_value_ids() {
        let function = ordinary();
        let mut indices = vec![None; function.locals().len()];
        indices[4] = Some(ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(19),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        });
        parity(&function, &indices);
    }
    #[test]
    fn bounds_scan_assignment_then_call_clears_value_and_extent() {
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(1, vec![]),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = narrow(
            vec![assign(
                place(1, vec![]),
                SemanticRvalueKindV1::Length(place(2, vec![])),
            )],
            SemanticTerminatorKindV1::Call(call),
        );
        let old = original(&function, 0, &[], &[]).unwrap();
        assert_eq!(old.0[1].count, 2);
        assert!(old.0[1].value.is_none() && old.0[1].length_source.is_none());
        parity(&function, &[]);
    }
    #[test]
    fn bounds_scan_projected_destinations_are_skipped() {
        let field =
            || SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR_TYPE).unwrap();
        let function = narrow(
            vec![assign(
                place(1, vec![field()]),
                SemanticRvalueKindV1::Length(place(2, vec![])),
            )],
            SemanticTerminatorKindV1::Return,
        );
        assert_eq!(original(&function, 0, &[], &[]).unwrap().0[1].count, 0);
        parity(&function, &[]);
    }
    #[test]
    fn bounds_scan_length_and_pointer_metadata_distinguish_field_deref_and_plain() {
        for projection in [
            None,
            Some(SemanticProjectionKindV1::Field(0)),
            Some(SemanticProjectionKindV1::Dereference),
        ] {
            for metadata in [false, true] {
                let p = place(
                    2,
                    projection
                        .into_iter()
                        .map(|p| SemanticProjectionV1::new(p, SCALAR_TYPE).unwrap())
                        .collect(),
                );
                let value = if metadata {
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: SemanticOperandV1::Copy(p),
                    }
                } else {
                    SemanticRvalueKindV1::Length(p)
                };
                let function = narrow(
                    vec![assign(place(1, vec![]), value)],
                    SemanticTerminatorKindV1::Return,
                );
                parity(&function, &[]);
            }
        }
        let function = narrow(
            vec![assign(
                place(1, vec![]),
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: constant(0),
                },
            )],
            SemanticTerminatorKindV1::Return,
        );
        parity(&function, &[]);
    }
    #[test]
    fn bounds_scan_duplicate_edges_preserve_order_and_multiplicity() {
        let function = narrow(
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: constant(0),
                targets: SemanticSwitchTargetsV1::new(
                    vec![
                        SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        ),
                        SemanticSwitchTargetV1::new(
                            1,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        ),
                    ],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            },
        );
        assert_eq!(
            original(&function, 0, &[], &[]).unwrap().1[1],
            vec![0, 0, 0]
        );
        parity(&function, &[]);
    }
    #[test]
    fn bounds_scan_legacy_first_error_and_partial_mutation_order() {
        let function = ordinary();
        let mut definitions = vec![BoundsLocalDefinitionV1::default(); 5];
        let mut predecessors = vec![];
        assert!(matches!(
            scan::scan_legacy_v1(&function, &mut definitions, &mut predecessors),
            Err(Error::Unsupported(
                "a Rust bounds-check definition outside the semantic local table"
            ))
        ));
        assert_eq!(definitions[4].count, 1);
        let mut definitions = vec![BoundsLocalDefinitionV1::default(); function.locals().len()];
        assert!(matches!(
            scan::scan_legacy_v1(&function, &mut definitions, &mut predecessors),
            Err(Error::Unsupported(
                "a Rust bounds-check CFG edge outside the semantic block table"
            ))
        ));
        assert_eq!(definitions[2].count, 1);
    }
    #[test]
    fn bounds_scan_call_result_range_error_precedes_cfg_error() {
        let function = option_dominance_chain(1).0;
        let mut definitions = vec![];
        let mut predecessors = vec![];
        assert!(matches!(
            scan::scan_legacy_v1(&function, &mut definitions, &mut predecessors),
            Err(Error::Unsupported(
                "a Rust bounds-check call result outside the semantic local table"
            ))
        ));
    }
    #[test]
    fn bounds_scan_ordinary_shape_refusal_precedes_known_shape_refusal() {
        let function = ordinary();
        assert!(matches!(
            scan::initial_values_legacy_v1(&function, &[None], &[None]),
            Err(Error::Unsupported(
                "ordinary intrinsic index facts do not match the semantic local table"
            ))
        ));
        assert!(matches!(
            scan::initial_values_legacy_v1(&function, &[None], &[]),
            Err(Error::Unsupported(
                "intrinsic index facts do not match the semantic local table"
            ))
        ));
    }
    #[test]
    fn bounds_scan_refuses_unmetered_and_repeated_owner() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        assert!(
            storage
                .scan(&function, &mut PreparationResourcesV1::unmetered())
                .is_err()
        );
        assert!(!scan::test_access::state(&storage).0);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            assert!(storage.scan(&function, &mut resources).is_err());
            assert!(storage.view(&function, &mut resources).is_err());
            storage
                .initialize_values(&function, &[], &[], &mut resources)
                .unwrap();
            assert!(
                storage
                    .initialize_values(&function, &[], &[], &mut resources)
                    .is_err()
            );
        }
        drop(storage);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn bounds_scan_capacity_only_owners_are_occupied() {
        for vector in 0..4 {
            let mut storage = BoundsSourceStorageV1::new();
            scan::test_access::occupy_capacity(&mut storage, vector);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            assert!(
                storage
                    .scan(
                        &ordinary(),
                        &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                    )
                    .is_err()
            );
            assert_eq!(owned, 0);
        }
    }
    #[test]
    fn bounds_scan_equal_content_foreign_source_refuses() {
        let function = ordinary();
        let foreign = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            assert!(
                storage
                    .initialize_values(&foreign, &[], &[], &mut resources)
                    .is_err()
            );
            storage
                .initialize_values(&function, &[], &[], &mut resources)
                .unwrap();
            assert!(storage.view(&foreign, &mut resources).is_err());
        }
        drop(storage);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn bounds_scan_foreign_ledger_refuses_without_borrowing_original_budget() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        storage
            .scan(
                &function,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
        let mut other_work = Work::new(LIMIT);
        let mut other_budget = Budget::new(&mut other_work, LIMIT);
        let mut other_owned = 0;
        assert!(
            storage
                .initialize_values(
                    &function,
                    &[],
                    &[],
                    &mut PreparationResourcesV1::new(&mut other_budget, &mut other_owned)
                )
                .is_err()
        );
        assert_eq!(other_owned, 0);
        drop(storage);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn bounds_scan_missing_scan_and_failed_shape_cannot_be_retried() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            assert!(
                storage
                    .initialize_values(&function, &[], &[], &mut resources)
                    .is_err()
            );
            storage.scan(&function, &mut resources).unwrap();
            assert!(
                storage
                    .initialize_values(&function, &[None], &[], &mut resources)
                    .is_err()
            );
            assert!(
                storage
                    .initialize_values(&function, &[], &[], &mut resources)
                    .is_err()
            );
            assert!(storage.view(&function, &mut resources).is_err());
        }
        drop(storage);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn bounds_scan_invalid_or_wrong_local_coordinates_refuse() {
        for (block, statement) in [(usize::MAX, 0), (0, usize::MAX), (0, 1)] {
            let function = ordinary();
            let mut storage = BoundsSourceStorageV1::new();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            {
                let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
                storage.scan(&function, &mut resources).unwrap();
                storage
                    .initialize_values(&function, &[], &[], &mut resources)
                    .unwrap();
                scan::test_access::coordinate(&mut storage, 4, block, statement);
                assert!(
                    storage
                        .view(&function, &mut resources)
                        .unwrap()
                        .definition(4, &mut resources)
                        .is_err()
                );
            }
            drop(storage);
            budget.release_storage(owned).unwrap();
        }
    }
    #[test]
    fn bounds_scan_queries_recheck_original_ledger_and_ranges() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            storage
                .initialize_values(&function, &[], &[], &mut resources)
                .unwrap();
            let view = storage.view(&function, &mut resources).unwrap();
            assert!(view.definition(usize::MAX, &mut resources).is_err());
            assert!(view.predecessors(usize::MAX, &mut resources).is_err());
            assert!(
                view.local_values(&mut PreparationResourcesV1::unmetered())
                    .is_err()
            );
        }
        drop(storage);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn bounds_scan_count_saturates_and_frame_arithmetic_refuses() {
        scan::test_access::saturated_assignment_and_call(&ordinary());
        scan::test_access::checked_frame_arithmetic();
    }
    #[test]
    fn bounds_scan_header_one_short_work_marks_owner_without_storage() {
        let frame = scan::scan_frame_v1().unwrap();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(frame - 1);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(
            storage
                .scan(
                    &ordinary(),
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert!(budget.failed_work().is_some());
        assert_eq!(owned, 0);
        assert_eq!(
            scan::test_access::state(&storage),
            (true, false, false, false, 0, 0)
        );
    }
    #[test]
    fn bounds_scan_header_one_short_storage_keeps_poisoned_owner() {
        let frame = scan::scan_frame_v1().unwrap();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, frame - 1);
        let mut owned = 0;
        assert!(
            storage
                .scan(
                    &ordinary(),
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert!(budget.failed_storage().is_some());
        assert_eq!(owned, 0);
        assert!(scan::test_access::state(&storage).0);
    }
    #[test]
    fn bounds_scan_partial_allocation_remains_owned_until_outer_cleanup() {
        let function = ordinary();
        let mut reference = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        reference
            .scan(
                &function,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
        let exact = owned;
        drop(reference);
        budget.release_storage(owned).unwrap();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, exact - 1);
        let mut owned = 0;
        assert!(
            storage
                .scan(
                    &function,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert!(budget.failed_storage().is_some());
        assert!(scan::test_access::state(&storage).4 > 0);
        assert_eq!(budget.storage(), owned);
        assert!(!scan::test_access::state(&storage).1);
        drop(storage);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn bounds_scan_expected_unwind_retains_completed_owner_until_postflight() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            storage
                .scan(
                    &function,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned),
                )
                .unwrap();
            panic!("fixed component unwind after retained scan");
        }));
        assert!(result.is_err());
        assert_eq!(budget.storage(), owned);
        assert!(scan::test_access::state(&storage).1);
        drop(storage);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn bounds_scan_paid_initialization_preserves_ordinary_first_error() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            assert!(matches!(
                storage.initialize_values(&function, &[None], &[None], &mut resources),
                Err(Error::Unsupported(
                    "ordinary intrinsic index facts do not match the semantic local table"
                ))
            ));
        }
        let state = scan::test_access::state(&storage);
        assert!(state.0 && state.1 && state.2 && !state.3);
        assert_eq!(budget.storage(), owned);
        drop(storage);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn bounds_scan_value_extent_failure_retains_initialized_local_slots() {
        let function = ordinary();
        let mut reference = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            reference.scan(&function, &mut resources).unwrap();
            reference
                .initialize_values(&function, &[], &[], &mut resources)
                .unwrap();
        }
        let total = owned;
        drop(reference);
        budget.release_storage(owned).unwrap();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, total - 1);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            assert!(
                storage
                    .initialize_values(&function, &[], &[], &mut resources)
                    .is_err()
            );
            assert!(storage.view(&function, &mut resources).is_err());
        }
        assert!(budget.failed_storage().is_some());
        assert_eq!(
            scan::test_access::value_lengths(&storage),
            (function.locals().len(), 0)
        );
        assert_eq!(budget.storage(), owned);
        drop(storage);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn bounds_scan_nested_growth_overflow_retains_existing_edge_payload() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            scan::test_access::nested_growth_overflow(&mut storage, &mut resources);
        }
        assert_eq!(budget.storage(), owned);
        drop(storage);
        budget.release_storage(owned).unwrap();
    }

    #[test]
    fn bounds_scan_paid_source_errors_preserve_first_error_and_partial_rows() {
        // Raw component declarations, deliberately not admitted semantic requests.
        for mode in 0..3 {
            let mut statements = vec![assign(
                place(1, vec![]),
                SemanticRvalueKindV1::Use(constant(0)),
            )];
            if mode == 0 {
                statements.push(assign(
                    place(99, vec![]),
                    SemanticRvalueKindV1::Use(constant(1)),
                ));
            }
            let terminator = if mode == 1 {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(0),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            place(99, vec![]),
                            cfg_edge(SemanticEdgeRoleV1::CallReturn, 99),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            } else {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 99))
            };
            let function = narrow(statements, terminator);
            let expected = original(&function, 0, &[], &[]);
            let mut storage = BoundsSourceStorageV1::new();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            let actual = storage.scan(
                &function,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            );
            match (expected, actual) {
                (Err(Error::Unsupported(a)), Err(Error::Unsupported(b))) => assert_eq!(a, b),
                _ => panic!("source error ordering changed"),
            }
            assert_eq!(scan::test_access::definition_count(&storage, 1), 1);
            assert!(!scan::test_access::state(&storage).1);
            assert_eq!(budget.storage(), owned);
            drop(storage);
            budget.release_storage(owned).unwrap();
        }
    }

    #[test]
    fn bounds_scan_explicit_frame_rosters_cover_nested_vertices_and_overflow() {
        scan::test_access::frame_rosters_control();
        scan::test_access::query_frame_overflow_control();
    }

    #[test]
    fn bounds_scan_standalone_queries_reuse_one_retained_query_envelope() {
        let function = ordinary();
        let expected = original(&function, 0, &[], &[]).unwrap();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            storage
                .initialize_values(&function, &[], &[], &mut resources)
                .unwrap();
        }
        let retained = owned;
        assert!(retained >= scan::scan_frame_v1().unwrap() + scan::value_frame_v1().unwrap());
        let before_work = budget.work();
        let before_storage = budget.storage();
        for _ in 0..3 {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            let view = storage.view(&function, &mut resources).unwrap();
            view.require_same_source_v1(&function, &mut resources)
                .unwrap();
            for (local, row) in expected.0.iter().enumerate() {
                equal_definition(view.definition(local, &mut resources).unwrap(), *row);
            }
            for (block, row) in expected.1.iter().enumerate() {
                assert_eq!(view.predecessors(block, &mut resources).unwrap(), row);
            }
            assert_eq!(view.local_values(&mut resources).unwrap(), expected.2);
            assert_eq!(view.slice_extents(&mut resources).unwrap(), expected.3);
        }
        // Exactly the old per-query work: view, source check, two slices,
        // one per local/block, and the reached source-coordinate lookups.
        let coordinate_reads = expected.0.iter().filter(|row| row.value.is_some()).count();
        let per_pass = 4 + expected.0.len() + coordinate_reads + expected.1.len();
        assert_eq!(budget.work() - before_work, 3 * per_pass);
        assert_eq!(budget.storage(), before_storage);
        assert_eq!(owned, retained);
        drop(storage);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn bounds_scan_standalone_invalid_queries_do_not_refund_or_readmit() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            storage.scan(&function, &mut resources).unwrap();
            storage
                .initialize_values(&function, &[], &[], &mut resources)
                .unwrap();
        }
        let retained = owned;
        let before = budget.work();
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            let view = storage.view(&function, &mut resources).unwrap();
            assert!(view.definition(usize::MAX, &mut resources).is_err());
            assert!(view.predecessors(usize::MAX, &mut resources).is_err());
            // These source errors do not poison the work/storage ledger.
            assert_eq!(
                view.local_values(&mut resources).unwrap().len(),
                function.locals().len()
            );
            assert_eq!(
                view.slice_extents(&mut resources).unwrap().len(),
                function.locals().len()
            );
        }
        assert_eq!(budget.work() - before, 5);
        assert_eq!(owned, retained);
        assert_eq!(budget.storage(), retained);
        drop(storage);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn bounds_scan_query_envelope_is_required_before_any_scan_payload() {
        let total = scan::scan_frame_v1().unwrap();
        let query = scan::test_access::standalone_query_frame_v1();
        assert!(query > 0 && total > query);
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        // Enough for all the new scan rows except the one query envelope.
        let mut budget = Budget::new(&mut work, total - query);
        let mut owned = 0;
        let function = ordinary();
        assert!(
            storage
                .scan(
                    &function,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert!(budget.failed_storage().is_some());
        assert_eq!(owned, 0);
        assert_eq!(
            scan::test_access::state(&storage),
            (true, false, false, false, 0, 0)
        );
        assert!(
            storage
                .scan(
                    &function,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert_eq!(owned, 0);
    }

    #[test]
    fn bounds_scan_value_envelope_one_short_stays_partial_and_nonreusable() {
        let function = ordinary();
        let mut storage = BoundsSourceStorageV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        storage
            .scan(
                &function,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
        let retained = owned;
        let frame = scan::value_frame_v1().unwrap();
        let burn = LIMIT - budget.storage() - (frame - 1);
        budget.reserve_storage(burn).unwrap();
        assert!(
            storage
                .initialize_values(
                    &function,
                    &[],
                    &[],
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert!(budget.failed_storage().is_some());
        assert_eq!(owned, retained);
        assert_eq!(scan::test_access::value_lengths(&storage), (0, 0));
        let state = scan::test_access::state(&storage);
        assert!(state.0 && state.1 && state.2 && !state.3);
        assert!(
            storage
                .initialize_values(
                    &function,
                    &[],
                    &[],
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert_eq!(owned, retained);
        drop(storage);
        budget.release_storage(owned).unwrap();
        budget.release_storage(burn).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
