// Isolated component fixtures only; these tests never create a source owner.
mod nominal_preparation_component_tests {
    use super::super::bf16_nominal_source_algorithms_v1 as algorithms;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    fn fixture(escape: bool, redefine: bool) -> SemanticFunctionDeclV1 {
        let place = |index, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap()
        };
        let assign = |destination, ty, kind| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination, ty),
                SemanticRvalueV1::new(ty, kind),
            )))
        };
        let mut statements = vec![assign(
            2,
            SCALAR_TYPE,
            SemanticRvalueKindV1::Use(constant(17)),
        )];
        if redefine {
            statements.push(assign(
                2,
                SCALAR_TYPE,
                SemanticRvalueKindV1::Use(constant(19)),
            ));
        }
        if escape {
            statements.push(assign(
                3,
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(2, SCALAR_TYPE),
                },
            ));
        }
        projection_function(vec![block(
            231,
            statements,
            SemanticTerminatorKindV1::Return,
        )])
    }
    type Components = (
        AssertionDefinitionInventoryV1,
        LocalProvenanceV1,
        Vec<Option<AllocationContractV1>>,
        Vec<Option<u64>>,
    );
    fn prepare_component(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<Components, ProductionRankedProjectionErrorV1> {
        let inventory =
            algorithms::assertion_definition_inventory_with_resources_v1(function, resources)?;
        let origins = algorithms::local_provenance_with_resources_v1(
            &[],
            types,
            function,
            &inventory.counts,
            &inventory.address_escaped,
            resources,
        )?;
        let allocations = algorithms::local_allocation_contracts_with_resources_v1(
            types,
            function,
            &origins.allocation_origins,
            resources,
        )?;
        let constants = algorithms::constant_locals_with_resources_v1(function, resources)?;
        Ok((inventory, origins, allocations, constants))
    }
    #[test]
    fn exact_shared_source_predicates_match_legacy_without_source_authority() {
        for (escape, redefine) in [(false, false), (true, false), (false, true)] {
            let function = fixture(escape, redefine);
            let types = projection_types();
            let (old, origins, allocations, constants) =
                prepare_component(&function, &types, &mut PreparationResourcesV1::unmetered())
                    .unwrap();
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            budget.reserve_storage(29).unwrap();
            let mut owned = 0;
            let result = prepare_component(
                &function,
                &types,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
            assert_eq!(result.0.counts, old.counts);
            assert_eq!(result.0.blocks, old.blocks);
            assert_eq!(result.0.assignments, old.assignments);
            assert_eq!(result.0.address_escaped, old.address_escaped);
            assert_eq!(result.1, origins);
            assert_eq!(result.2, allocations);
            assert_eq!(result.3, constants);
            assert_eq!(
                result.3[2],
                if escape || redefine { None } else { Some(17) }
            );
            assert_eq!(budget.storage(), 29 + owned);
            assert!(owned > 4096 && budget.work() > 0);
            drop(result);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), 29);
        }
    }
    #[test]
    fn preparation_exact_and_one_short_boundaries_preserve_owned_prefix() {
        let function = fixture(true, false);
        let types = projection_types();
        let probe = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(11).unwrap();
            budget.reserve_storage(31).unwrap();
            let mut owned = 0;
            let result = prepare_component(
                &function,
                &types,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            );
            let ok = result.is_ok();
            let denied = (
                budget.failed_work().is_some(),
                budget.failed_storage().is_some(),
            );
            drop(result); // ALL values drop before exact accepted-owned refund.
            assert_eq!(budget.storage(), 31 + owned);
            let observed = (ok, budget.work(), budget.peak_storage(), denied);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), 31);
            observed
        };
        let measured = probe(100_000, 1_000_000);
        assert!(measured.0);
        assert_eq!(probe(measured.1, measured.2), measured);
        let short_work = probe(measured.1 - 1, measured.2);
        assert!(!short_work.0 && short_work.3.0 && !short_work.3.1);
        let short_storage = probe(measured.1, measured.2 - 1);
        assert!(!short_storage.0 && !short_storage.3.0 && short_storage.3.1);
    }
    #[test]
    fn metered_origin_conflict_is_not_an_empty_or_default_origin() {
        let edges = vec![vec![2], vec![2], vec![]];
        let mut origins = vec![Some(0u32), Some(1), None];
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, 100_000);
        let mut owned = 0;
        let result = algorithms::propagate_exact_local_origins_with_resources_v1(
            &mut origins,
            &edges,
            "test origin conflict",
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "test origin conflict"
            ))
        ));
        assert_eq!(origins, vec![Some(0), Some(1), Some(0)]);
        drop(result);
        assert_eq!(budget.storage(), owned);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn metered_constant_alias_and_cycle_preserve_exact_values() {
        for definitions in [
            vec![
                ConstantDefinitionV1::Direct(64),
                ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(0)),
            ],
            vec![
                ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(1)),
                ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(0)),
            ],
        ] {
            let mut legacy_states = [0; 2];
            let mut legacy_values = [None; 2];
            let old = resolve_constant_iterative(
                1,
                &definitions,
                &mut legacy_states,
                &mut legacy_values,
                &mut Vec::new(),
            );
            let mut states = [0; 2];
            let mut values = [None; 2];
            let mut path = Vec::new();
            let mut work = Work::new(1000);
            let mut budget = Budget::new(&mut work, 100_000);
            let mut owned = 0;
            let result = algorithms::resolve_constant_iterative_with_resources_v1(
                1,
                &definitions,
                &mut states,
                &mut values,
                &mut path,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
            assert_eq!(result, old);
            assert_eq!(states, legacy_states);
            assert_eq!(values, legacy_values);
            drop(path);
            budget.release_storage(owned).unwrap();
        }
    }
    #[test]
    fn explicit_sort_admission_preserves_sorted_unique_set() {
        for mut values in [vec![4, 1, 3, 1, 0], vec![], vec![0, 0, 0]] {
            let mut old = values.clone();
            old.sort_unstable();
            old.dedup();
            let mut work = Work::new(1000);
            let mut budget = Budget::new(&mut work, 100_000);
            let mut owned = 0;
            PreparationResourcesV1::new(&mut budget, &mut owned)
                .sort_unique_indices(&mut values)
                .unwrap();
            assert_eq!(values, old);
            assert_eq!(owned, 0, "sort allocates no payload");
        }
    }
    #[test]
    fn resource_adapter_preserves_arithmetic_and_first_denial() {
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, 16);
        let mut owned = 0;
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        assert!(matches!(
            resources.reserve::<u64>(&mut Vec::new(), usize::MAX),
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
            ))
        ));
        resources.work(1).unwrap();
        assert!(matches!(
            resources.work(1),
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Work(_))
            ))
        ));
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.failed_work(), Some(2));
        assert_eq!(budget.storage(), 0);
        assert_eq!(owned, 0);
    }
    #[test]
    fn transparency_spines_are_prepaid_before_shared_provenance_scans() {
        let length = 64usize;
        let place = || {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE,)
                        .unwrap();
                    length
                ],
                SCALAR_TYPE,
            )
            .unwrap()
        };
        let operand = || SemanticOperandV1::Copy(place());
        let cases = [
            (SemanticRvalueKindV1::Use(operand()), 2),
            (
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: operand(),
                },
                2,
            ),
            (
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: operand(),
                },
                1,
            ),
            (
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Offset,
                    left: operand(),
                    right: constant(1),
                },
                1,
            ),
            (
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(),
                },
                1,
            ),
        ];
        for (kind, scans) in cases {
            let required = length * scans;
            for limit in [required - 1, required] {
                let mut work = Work::new(limit + 7);
                let mut budget = Budget::new(&mut work, 1_000_000);
                budget.charge_work(7).unwrap();
                let mut owned = 0;
                let result = algorithms::prepay_provenance_spines_v1(
                    &kind,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned),
                );
                if limit < required {
                    assert!(matches!(
                        result,
                        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                            CanonicalAssertionErrorV1::Resource(Resource::Work(_))
                        ))
                    ));
                    assert_eq!(budget.work(), 7, "no spine work admitted");
                    assert_eq!(budget.failed_work(), Some(7 + required));
                } else {
                    result.unwrap();
                    assert_eq!(budget.work(), 7 + required);
                    assert_eq!(budget.failed_work(), None);
                }
                assert_eq!(owned, 0, "preflight neither allocates nor grants authority");
            }
        }
    }

    #[test]
    fn actual_provenance_source_path_charges_both_transparency_walks() {
        let measure = |length| {
            let source = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(0),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE,)
                        .unwrap();
                    length
                ],
                SCALAR_TYPE,
            )
            .unwrap();
            let destination =
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], SCALAR_TYPE)
                    .unwrap();
            let assignment = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)),
                ),
            )));
            let function = projection_function(vec![block(
                232,
                vec![assignment],
                SemanticTerminatorKindV1::Return,
            )]);
            let types = projection_types();
            let legacy =
                prepare_component(&function, &types, &mut PreparationResourcesV1::unmetered())
                    .unwrap();
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 1_000_000);
            let mut owned = 0;
            let metered = prepare_component(
                &function,
                &types,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned),
            )
            .unwrap();
            assert_eq!(metered.0.counts, legacy.0.counts);
            assert_eq!(metered.0.address_escaped, legacy.0.address_escaped);
            assert_eq!(metered.1, legacy.1);
            assert_eq!(metered.2, legacy.2);
            assert_eq!(metered.3, legacy.3);
            assert_eq!(metered.3[2], None);
            drop(metered);
            let observed = (budget.work(), owned);
            budget.release_storage(owned).unwrap();
            observed
        };
        let short = measure(1);
        let long = measure(64);
        assert_eq!(long.0 - short.0, 2 * (64 - 1));
        assert_eq!(long.1, short.1, "projection spine remains borrowed source");
    }
}
