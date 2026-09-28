// Synthetic component controls. Fixture/observer allocations are not claimed
// as an authenticated paid source route. All physical owner fields are dropped
// before the original credit counter is refunded.
mod direct_comparison_preparation_controls {
    use super::super::root_direct_comparison_preparation_v1 as direct;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAssignmentV1, SemanticConstantV1, SemanticProjectionKindV1, SemanticProjectionV1,
        SemanticRvalueV1, SemanticScalarValueV1,
    };
    const LIMIT: usize = 32 * 1024 * 1024;
    type Error = ProductionRankedProjectionErrorV1;
    type Pair = (ProductionRankedValueV1, ProductionRankedValueV1);
    // BEGIN ORIGINAL UNIFORM ORACLE
    fn original_uniform(
        operand: &SemanticOperandV1,
        constants: &[Option<u64>],
        stable_argument_origins: &[Option<u32>],
        arguments: &mut [Option<u32>],
        next_argument: &mut usize,
        operations: &mut Vec<ProductionRankedOperationV1>,
        next_value: &mut u32,
    ) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
        if let Some(value) = constant_operand_value(operand, constants) {
            reserve_operation(operations)?;
            let result = next_value_id(next_value)?;
            operations.push(ProductionRankedOperationV1::IndexConstant { result, value });
            return Ok(Some(ProductionRankedValueV1::Local(result)));
        }
        let Some(local) = simple_operand_local(operand) else {
            return Ok(None);
        };
        let local_index = local.index() as usize;
        let origin = stable_argument_origins.get(local_index).copied().flatten();
        let Some(origin) = origin.map(|origin| origin as usize) else {
            return Ok(None);
        };
        let slot =
            arguments
                .get_mut(origin)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a uniform switch argument origin outside the semantic local table",
                ))?;
        let argument = match *slot {
            Some(argument) => argument,
            None => {
                let argument = u32::try_from(*next_argument).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "too many uniform switch ranked arguments",
                    )
                })?;
                *next_argument = next_argument.checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "uniform switch ranked argument count overflow",
                    ),
                )?;
                *slot = Some(argument);
                argument
            }
        };
        Ok(Some(ProductionRankedValueV1::Argument(argument)))
    }
    // END ORIGINAL UNIFORM ORACLE
    struct OriginalScalarInventory<'a> {
        address_escaped: &'a [bool],
    }
    #[allow(clippy::too_many_arguments, clippy::needless_borrow)]
    fn original_loop(
        function: &SemanticFunctionDeclV1,
        constants: &[Option<u64>],
        stable_argument_origins: &[Option<u32>],
        local_definitions: &[u8],
        address_escaped: &[bool],
        mut runtime_index_arguments: &mut [Option<u32>],
        mut next_runtime_argument: &mut usize,
        operations: &mut Vec<ProductionRankedOperationV1>,
        next_value: &mut u32,
    ) -> Result<Vec<Option<GuardPredicateV1>>, Error> {
        let local_count = function.locals().len();
        let scalar_inventory = OriginalScalarInventory { address_escaped };
        // BEGIN FROZEN ORIGINAL LOOP (operand callee renamed to frozen oracle)
        let mut direct_switch_predicates = vec![None; local_count];
        for block in function.blocks() {
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty() {
                    continue;
                }
                let SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left,
                    right,
                } = assignment.value().kind()
                else {
                    continue;
                };
                let destination = assignment.destination().local().index() as usize;
                if local_definitions.get(destination).copied() != Some(1)
                    || scalar_inventory.address_escaped.get(destination).copied() != Some(false)
                {
                    continue;
                }
                let Some(lhs) = original_uniform(
                    left,
                    constants,
                    &stable_argument_origins,
                    &mut runtime_index_arguments,
                    &mut next_runtime_argument,
                    operations,
                    next_value,
                )?
                else {
                    continue;
                };
                let Some(rhs) = original_uniform(
                    right,
                    constants,
                    &stable_argument_origins,
                    &mut runtime_index_arguments,
                    &mut next_runtime_argument,
                    operations,
                    next_value,
                )?
                else {
                    continue;
                };
                retain_identical_direct_switch_predicate_v1(
                    direct_switch_predicates.get_mut(destination).ok_or(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "a direct switch predicate outside the semantic local table",
                        ),
                    )?,
                    GuardPredicateV1 {
                        comparisons: vec![(lhs, rhs)],
                    },
                )?;
            }
        }
        // END FROZEN ORIGINAL LOOP
        Ok(direct_switch_predicates)
    }

    fn operand(local: u32, projections: usize) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                (0..projections)
                    .map(|_| {
                        SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, SCALAR_TYPE)
                            .unwrap()
                    })
                    .collect(),
                SCALAR_TYPE,
            )
            .unwrap(),
        )
    }
    fn constant(value: u64) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(u128::from(value), 8).unwrap(),
            ),
        ))
    }
    fn comparison(
        destination: u32,
        left: SemanticOperandV1,
        right: SemanticOperandV1,
    ) -> SemanticStatementV1 {
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(destination),
                    vec![],
                    SCALAR_TYPE,
                )
                .unwrap(),
                SemanticRvalueV1::new(
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left,
                        right,
                    },
                ),
            )),
        )
    }
    fn function(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
        scalar_summary_argument_helper(
            61,
            vec![block(62, statements, SemanticTerminatorKindV1::Return)],
        )
    }
    fn duplicate_function() -> SemanticFunctionDeclV1 {
        function(vec![
            comparison(0, operand(1, 0), operand(2, 0)),
            comparison(0, operand(1, 0), operand(2, 0)),
        ])
    }
    #[derive(Default)]
    struct Inputs {
        slots: Vec<Option<u32>>,
        next_argument: usize,
        operations: Vec<ProductionRankedOperationV1>,
        next_value: u32,
    }
    fn inputs() -> Inputs {
        Inputs {
            slots: vec![None; 3],
            next_argument: 7,
            next_value: 41,
            ..Default::default()
        }
    }
    fn snapshot(i: &Inputs) -> (Vec<Option<u32>>, usize, String, u32) {
        (
            i.slots.clone(),
            i.next_argument,
            format!("{:?}", i.operations),
            i.next_value,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn paid(
        owner: &mut direct::DirectComparisonPreparationV1,
        function: &SemanticFunctionDeclV1,
        constants: &[Option<u64>],
        origins: &[Option<u32>],
        counts: &[u8],
        escaped: &[bool],
        i: &mut Inputs,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        owner.prepare(
            function,
            constants,
            origins,
            counts,
            escaped,
            &mut i.slots,
            &mut i.next_argument,
            &mut i.operations,
            &mut i.next_value,
            resources,
        )
    }
    fn run(
        owner: &mut direct::DirectComparisonPreparationV1,
        function: &SemanticFunctionDeclV1,
        i: &mut Inputs,
        resources: &mut PreparationResourcesV1<'_, '_>,
    ) -> Result<(), Error> {
        paid(
            owner,
            function,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            i,
            resources,
        )
    }
    fn physical(owner: &direct::DirectComparisonPreparationV1, i: &Inputs) -> usize {
        let (_, roster, _pending, duplicates) = direct::test_access::observe(owner);
        let (roster_cap, pending_cap, duplicates_cap) = direct::test_access::capacities(owner);
        roster_cap * std::mem::size_of::<Option<GuardPredicateV1>>()
            + pending_cap * std::mem::size_of::<Pair>()
            + duplicates_cap * std::mem::size_of::<GuardPredicateV1>()
            + roster
                .iter()
                .flatten()
                .map(|g| g.comparisons.len() * std::mem::size_of::<Pair>())
                .sum::<usize>()
            + duplicates
                .iter()
                .map(|g| g.comparisons.len() * std::mem::size_of::<Pair>())
                .sum::<usize>()
            + i.operations.capacity() * std::mem::size_of::<ProductionRankedOperationV1>()
    }
    fn parity(
        function: &SemanticFunctionDeclV1,
        constants: &[Option<u64>],
        origins: &[Option<u32>],
        counts: &[u8],
        escaped: &[bool],
        make: fn() -> Inputs,
    ) {
        let mut old = make();
        let mut new = make();
        let expected = original_loop(
            function,
            constants,
            origins,
            counts,
            escaped,
            &mut old.slots,
            &mut old.next_argument,
            &mut old.operations,
            &mut old.next_value,
        );
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let actual = paid(
            &mut owner,
            function,
            constants,
            origins,
            counts,
            escaped,
            &mut new,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        );
        assert_eq!(
            format!("{:?}", actual.as_ref().map(|_| ())),
            format!("{:?}", expected.as_ref().map(|_| ()))
        );
        assert_eq!(snapshot(&new), snapshot(&old));
        match expected {
            Ok(roster) => {
                assert_eq!(
                    owner
                        .view(
                            function,
                            &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                        )
                        .unwrap(),
                    roster
                );
                assert_eq!(direct::test_access::observe(&owner).0, 2);
            }
            Err(_) => assert_eq!(direct::test_access::observe(&owner).0, 1),
        }
        assert_eq!(budget.storage(), owned);
        assert!(physical(&owner, &new) <= owned);
        drop(owner);
        drop(new);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn direct_comparison_original_loop_oracle_arguments_constants_and_prefix_state() {
        let f = function(vec![
            comparison(0, operand(1, 0), operand(2, 0)),
            comparison(2, constant(4), constant(9)),
        ]);
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            inputs,
        );
        fn existing() -> Inputs {
            Inputs {
                slots: vec![Some(19), Some(23), None],
                next_argument: 29,
                next_value: 53,
                operations: vec![],
            }
        }
        parity(
            &f,
            &[None, None, Some(12)],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            existing,
        );
    }
    #[test]
    fn direct_comparison_original_loop_oracle_skips_and_missing_origins() {
        let f = function(vec![
            SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            ),
            comparison(0, operand(1, 0), operand(2, 0)),
        ]);
        for counts in [&[0u8; 3][..], &[2u8; 3][..], &[][..]] {
            parity(
                &f,
                &[],
                &[Some(0), Some(1), Some(2)],
                counts,
                &[false; 3],
                inputs,
            );
        }
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[true; 3],
            inputs,
        );
        parity(&f, &[], &[], &[1; 3], &[false; 3], inputs);
    }
    #[test]
    fn direct_comparison_original_loop_oracle_left_then_right_partial_mutations() {
        for left in [operand(1, 0), constant(11)] {
            let f = function(vec![comparison(0, left, operand(2, 64))]);
            parity(
                &f,
                &[],
                &[Some(0), Some(1), Some(2)],
                &[1; 3],
                &[false; 3],
                inputs,
            );
        }
        let f = function(vec![comparison(0, operand(1, 0), operand(2, 0))]);
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(99)],
            &[1; 3],
            &[false; 3],
            inputs,
        );
    }
    #[test]
    fn direct_comparison_original_loop_oracle_destination_error_follows_both_operands() {
        let f = function(vec![comparison(7, operand(1, 0), operand(2, 0))]);
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 8],
            &[false; 8],
            inputs,
        );
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(matches!(
            paid(
                &mut owner,
                &f,
                &[],
                &[Some(0), Some(1), Some(2)],
                &[1; 8],
                &[false; 8],
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            ),
            Err(Error::Unsupported(
                "a direct switch predicate outside the semantic local table"
            ))
        ));
        assert_eq!(i.slots, [None, Some(7), Some(8)]);
        assert_eq!(i.next_argument, 9);
        assert!(direct::test_access::observe(&owner).2.is_none());
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_duplicates_keep_both_physical_payloads_and_exact_work() {
        let f = duplicate_function();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        run(
            &mut owner,
            &f,
            &mut i,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        let d = direct::direct_frame_v1().unwrap();
        let u = root_uniform_operand_preparation_v1::operand_frame_v1().unwrap();
        assert_eq!(budget.work(), d + 4 * u + 40);
        assert_eq!(
            owned,
            d + 4 * u
                + 3 * std::mem::size_of::<Option<GuardPredicateV1>>()
                + 2 * std::mem::size_of::<Pair>()
                + std::mem::size_of::<GuardPredicateV1>()
        );
        let (_, roster, pending, duplicates) = direct::test_access::observe(&owner);
        assert_eq!(duplicates.len(), 1);
        assert!(pending.is_none());
        assert_eq!(roster[0].as_ref(), duplicates.first());
        let pointers = direct::test_access::payload_pointers(&owner);
        assert_eq!(pointers.len(), 2);
        assert_ne!(pointers[0], pointers[1]);
        assert!(physical(&owner, &i) <= owned);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn direct_comparison_original_loop_oracle_duplicate_and_conflict() {
        parity(
            &duplicate_function(),
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            inputs,
        );
        let f = function(vec![
            comparison(0, operand(1, 0), operand(2, 0)),
            comparison(0, operand(1, 0), operand(0, 0)),
        ]);
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            inputs,
        );
    }
    #[test]
    fn direct_comparison_conflict_remains_pending_and_owner_is_terminal() {
        let f = function(vec![
            comparison(0, operand(1, 0), operand(2, 0)),
            comparison(0, operand(1, 0), operand(0, 0)),
        ]);
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(matches!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            ),
            Err(Error::Incomplete(
                "one comparison local has conflicting source definitions"
            ))
        ));
        let (phase, roster, pending, duplicates) = direct::test_access::observe(&owner);
        assert_eq!(phase, 1);
        assert!(roster[0].is_some());
        assert!(pending.is_some());
        assert!(duplicates.is_empty());
        assert_ne!(roster[0], pending);
        assert_eq!(i.next_argument, 10);
        let before = (budget.work(), budget.storage(), snapshot(&i));
        assert!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        assert_eq!(before, (budget.work(), budget.storage(), snapshot(&i)));
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_duplicate_reserve_denial_keeps_pending_allocation() {
        let f = duplicate_function();
        let d = direct::direct_frame_v1().unwrap();
        let u = root_uniform_operand_preparation_v1::operand_frame_v1().unwrap();
        let limit = d
            + 4 * u
            + 3 * std::mem::size_of::<Option<GuardPredicateV1>>()
            + 2 * std::mem::size_of::<Pair>();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        let mut owned = 0;
        assert!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        let (phase, roster, pending, duplicates) = direct::test_access::observe(&owner);
        assert_eq!(phase, 1);
        assert_eq!(roster[0], pending);
        assert!(duplicates.is_empty());
        assert_eq!(direct::test_access::capacities(&owner), (3, 1, 0));
        assert_eq!(owned, limit);
        assert!(budget.failed_storage().is_some());
        assert!(physical(&owner, &i) <= owned);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn direct_comparison_candidate_allocation_denial_keeps_pending_header() {
        let f = function(vec![comparison(0, operand(1, 0), operand(2, 0))]);
        let d = direct::direct_frame_v1().unwrap();
        let u = root_uniform_operand_preparation_v1::operand_frame_v1().unwrap();
        let limit = d + 2 * u + 3 * std::mem::size_of::<Option<GuardPredicateV1>>();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        let mut owned = 0;
        assert!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        let (phase, roster, pending, duplicates) = direct::test_access::observe(&owner);
        assert_eq!(phase, 1);
        assert!(roster.iter().all(Option::is_none));
        assert!(pending.unwrap().comparisons.is_empty());
        assert!(duplicates.is_empty());
        assert_eq!(direct::test_access::capacities(&owner), (3, 0, 0));
        assert_eq!(owned, limit);
        assert_eq!(i.next_argument, 9);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_work_denial_boundaries_preserve_attached_storage() {
        let f = duplicate_function();
        let d = direct::direct_frame_v1().unwrap();
        let u = root_uniform_operand_preparation_v1::operand_frame_v1().unwrap();
        for limit in [
            0,
            d - 1,
            d,
            d + 2,
            d + 3,
            d + 4,
            d + 5,
            d + 6,
            d + 7,
            d + 7 + u + 5,
            d + 7 + u + 6,
            d + 2 * u + 19,
            d + 2 * u + 22,
            d + 4 * u + 39,
        ] {
            let mut i = inputs();
            let mut owner = direct::DirectComparisonPreparationV1::new();
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            let mut owned = 0;
            assert!(
                run(
                    &mut owner,
                    &f,
                    &mut i,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err(),
                "{limit}"
            );
            assert_eq!(direct::test_access::observe(&owner).0, 1);
            assert!(budget.failed_work().is_some());
            assert_eq!(budget.storage(), owned);
            assert!(physical(&owner, &i) <= owned);
            let before = budget.work();
            assert!(
                owner
                    .view(
                        &f,
                        &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                    )
                    .is_err()
            );
            assert_eq!(before, budget.work());
            drop(owner);
            drop(i);
            budget.release_storage(owned).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }
    #[test]
    fn direct_comparison_header_and_roster_storage_boundaries_are_terminal() {
        let f = duplicate_function();
        let d = direct::direct_frame_v1().unwrap();
        for limit in [
            0,
            d - 1,
            d,
            d + 3 * std::mem::size_of::<Option<GuardPredicateV1>>() - 1,
        ] {
            let mut i = inputs();
            let mut owner = direct::DirectComparisonPreparationV1::new();
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, limit);
            let mut owned = 0;
            assert!(
                run(
                    &mut owner,
                    &f,
                    &mut i,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
            );
            assert_eq!(direct::test_access::observe(&owner).0, 1);
            assert_eq!(direct::test_access::capacities(&owner), (0, 0, 0));
            assert_eq!(snapshot(&i), snapshot(&inputs()));
            assert_eq!(owned, budget.storage());
            drop(owner);
            drop(i);
            budget.release_storage(owned).unwrap();
        }
    }
    #[test]
    fn direct_comparison_ssa_error_retains_original_operations_capacity() {
        let f = function(vec![comparison(0, constant(7), operand(2, 0))]);
        fn invalid() -> Inputs {
            let mut i = inputs();
            i.next_value = u32::MAX;
            i
        }
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            invalid,
        );
        let mut i = invalid();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        assert_eq!(i.operations.capacity(), 1);
        assert!(i.operations.is_empty());
        assert!(direct::test_access::observe(&owner).2.is_none());
        assert!(physical(&owner, &i) <= owned);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_refuses_unmetered_and_prior_denial_without_prefix_mutation() {
        let f = duplicate_function();
        let mut i = inputs();
        let before = snapshot(&i);
        let mut owner = direct::DirectComparisonPreparationV1::new();
        assert!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::unmetered()
            )
            .is_err()
        );
        assert_eq!(snapshot(&i), before);
        assert_eq!(direct::test_access::observe(&owner).0, 1);
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        assert!(
            PreparationResourcesV1::new(&mut budget, &mut owned)
                .work(1)
                .is_err()
        );
        assert!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        assert_eq!(snapshot(&i), before);
        assert_eq!(owned, 0);
    }
    #[test]
    fn direct_comparison_successful_owner_reentry_refuses_without_reallocating() {
        let f = duplicate_function();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        run(
            &mut owner,
            &f,
            &mut i,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        let before = (
            budget.work(),
            budget.storage(),
            snapshot(&i),
            direct::test_access::capacities(&owner),
        );
        assert!(
            run(
                &mut owner,
                &f,
                &mut i,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        assert_eq!(
            before,
            (
                budget.work(),
                budget.storage(),
                snapshot(&i),
                direct::test_access::capacities(&owner)
            )
        );
        assert_eq!(direct::test_access::observe(&owner).0, 1);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_foreign_same_content_source_refuses_and_poison_is_sticky() {
        let f = duplicate_function();
        let foreign = f.clone();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        run(
            &mut owner,
            &f,
            &mut i,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        let before = budget.work();
        assert!(
            owner
                .view(
                    &foreign,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert!(
            owner
                .view(
                    &f,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert_eq!(before, budget.work());
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_foreign_ledger_view_does_not_debit_or_detach() {
        let f = duplicate_function();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        run(
            &mut owner,
            &f,
            &mut i,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        let mut other_work = Work::new(LIMIT);
        let mut other_budget = Budget::new(&mut other_work, LIMIT);
        let mut other_owned = 0;
        assert!(
            owner
                .view(
                    &f,
                    &mut PreparationResourcesV1::new(&mut other_budget, &mut other_owned)
                )
                .is_err()
        );
        assert_eq!(other_budget.work(), 0);
        assert_eq!(other_owned, 0);
        assert_eq!(budget.storage(), owned);
        assert_eq!(direct::test_access::observe(&owner).3.len(), 1);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_expected_unwind_after_conflict_keeps_failed_owner() {
        let f = function(vec![
            comparison(0, operand(1, 0), operand(2, 0)),
            comparison(0, operand(1, 0), operand(0, 0)),
        ]);
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert!(
                run(
                    &mut owner,
                    &f,
                    &mut i,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
            );
            panic!("fixed direct comparison post-refusal unwind");
        }));
        assert!(unwind.is_err());
        let (phase, roster, pending, _) = direct::test_access::observe(&owner);
        assert_eq!(phase, 1);
        assert!(roster[0].is_some());
        assert!(pending.is_some());
        assert_eq!(budget.storage(), owned);
        assert!(physical(&owner, &i) <= owned);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    #[test]
    fn direct_comparison_input_rosters_are_data_not_authenticated_predecessor_state() {
        let f = function(vec![comparison(0, operand(1, 0), operand(2, 0))]);
        fn supplied() -> Inputs {
            Inputs {
                slots: vec![None, Some(100), Some(200)],
                next_argument: 211,
                operations: vec![],
                next_value: 73,
            }
        }
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            supplied,
        );
    }

    #[test]
    fn direct_comparison_projected_destination_and_non_less_than_are_skipped() {
        let place = match operand(0, 1) {
            SemanticOperandV1::Copy(p) => p,
            _ => unreachable!(),
        };
        let projected = SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place,
                SemanticRvalueV1::new(
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: constant(1),
                        right: constant(2),
                    },
                ),
            )),
        );
        let add = SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], SCALAR_TYPE)
                    .unwrap(),
                SemanticRvalueV1::new(
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: constant(3),
                        right: constant(4),
                    },
                ),
            )),
        );
        parity(
            &function(vec![projected, add]),
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            inputs,
        );
    }
    #[test]
    fn direct_comparison_multiple_blocks_preserve_original_source_order() {
        let f = scalar_summary_argument_helper(
            67,
            vec![
                block(
                    68,
                    vec![comparison(0, operand(2, 0), operand(1, 0))],
                    SemanticTerminatorKindV1::Return,
                ),
                block(
                    69,
                    vec![comparison(1, operand(0, 0), operand(1, 0))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
        parity(
            &f,
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            inputs,
        );
    }
    #[test]
    fn direct_comparison_owned_counter_identity_is_explicitly_not_authenticated() {
        let f = duplicate_function();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut original_owned = 0;
        run(
            &mut owner,
            &f,
            &mut i,
            &mut PreparationResourcesV1::new(&mut budget, &mut original_owned),
        )
        .unwrap();
        let mut other_counter = 0;
        // Same Budget/Work pair passes the lexical view check. This is exactly
        // why only the outer authenticated owner may supply the real counter.
        assert!(
            owner
                .view(
                    &f,
                    &mut PreparationResourcesV1::new(&mut budget, &mut other_counter)
                )
                .is_ok()
        );
        assert_eq!(other_counter, 0);
        assert_eq!(budget.storage(), original_owned);
        drop(owner);
        drop(i);
        budget.release_storage(original_owned).unwrap();
    }
    #[test]
    fn direct_comparison_view_work_denial_preserves_storage_and_is_terminal() {
        let f = duplicate_function();
        let mut i = inputs();
        let mut owner = direct::DirectComparisonPreparationV1::new();
        let d = direct::direct_frame_v1().unwrap();
        let u = root_uniform_operand_preparation_v1::operand_frame_v1().unwrap();
        let mut work = Work::new(d + 4 * u + 40);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        run(
            &mut owner,
            &f,
            &mut i,
            &mut PreparationResourcesV1::new(&mut budget, &mut owned),
        )
        .unwrap();
        assert!(
            owner
                .view(
                    &f,
                    &mut PreparationResourcesV1::new(&mut budget, &mut owned)
                )
                .is_err()
        );
        assert_eq!(direct::test_access::observe(&owner).0, 1);
        assert!(budget.failed_work().is_some());
        assert_eq!(budget.storage(), owned);
        assert_eq!(direct::test_access::observe(&owner).3.len(), 1);
        drop(owner);
        drop(i);
        budget.release_storage(owned).unwrap();
    }
    #[test]
    fn direct_comparison_empty_source_does_not_synthesize_argument_state() {
        parity(
            &function(vec![]),
            &[],
            &[Some(0), Some(1), Some(2)],
            &[1; 3],
            &[false; 3],
            inputs,
        );
    }
    #[test]
    fn direct_comparison_model_accessors_have_distinct_typed_frames() {
        direct::test_access::source_accessor_frames();
    }
    #[test]
    fn direct_comparison_explicit_frame_rows_and_checked_overflow() {
        direct::test_access::frames();
    }
}
