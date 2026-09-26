// Isolated synthetic carrier census/resource controls. Fixture construction is
// outside these scopes. No canonical owner, source admission or nominal proof.
mod original_ledger_controls {
    use super::*;
    use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::PreparationResourcesV1;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };

    const LIMIT: usize = 8 * 1024 * 1024;
    const PREFIX_BYTES: usize = 27;
    const PREFIX_WORK: usize = 13;
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Finish {
        Success,
        Work,
        Storage,
        Unsupported(&'static str),
        Incomplete(&'static str),
    }
    #[derive(Clone, Copy, Debug)]
    struct Observed {
        finish: Finish,
        work: usize,
        peak: usize,
        owned: usize,
        local_work: Option<usize>,
        failed_work: bool,
        failed_storage: bool,
    }
    #[allow(clippy::too_many_arguments)]
    fn probe(
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[u8],
        local_limit: usize,
        work_limit: usize,
        storage_limit: usize,
        inspect: impl FnOnce(&Result<(Vec<Option<u32>>, usize), ProductionRankedProjectionErrorV1>),
    ) -> Observed {
        let mut work = Work::new(PREFIX_WORK + work_limit);
        let mut budget = Budget::new(&mut work, PREFIX_BYTES + storage_limit);
        budget.reserve_storage(PREFIX_BYTES).unwrap();
        budget.charge_work(PREFIX_WORK).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let mut owned = 0usize;
        let result = {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            exclusive_owner_carrier_v1::origins_with_limit_and_resources(
                callables,
                function,
                definitions,
                local_limit,
                &mut resources,
            )
        };
        inspect(&result);
        let finish = match &result {
            Ok(_) => Finish::Success,
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Work(_)),
            )) => Finish::Work,
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Storage(_)),
            )) => Finish::Storage,
            Err(ProductionRankedProjectionErrorV1::Unsupported(reason)) => {
                Finish::Unsupported(reason)
            }
            Err(ProductionRankedProjectionErrorV1::Incomplete(reason)) => {
                Finish::Incomplete(reason)
            }
            other => panic!("unexpected synthetic carrier result: {other:?}"),
        };
        let observed = Observed {
            finish,
            work: budget.work() - PREFIX_WORK,
            peak: budget.peak_storage() - PREFIX_BYTES,
            owned,
            local_work: result.as_ref().ok().map(|(_, work)| *work),
            failed_work: budget.failed_work().is_some(),
            failed_storage: budget.failed_storage().is_some(),
        };
        // Every accepted reservation belongs to the outer test scope. Scratch
        // has already dropped, but its conservative credits remain until here.
        assert_eq!(budget.storage(), PREFIX_BYTES + owned);
        assert!(budget.work_ledger_identity_v1() == identity);
        drop(result); // returned origin allocation dies BEFORE its owned refund
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), PREFIX_BYTES);
        assert_eq!(budget.work(), PREFIX_WORK + observed.work);
        assert_eq!(budget.peak_storage(), PREFIX_BYTES + observed.peak);
        observed
    }
    fn compare(function: &SemanticFunctionDeclV1, calls: &[SemanticCallableDeclV1]) -> Observed {
        let definitions = assertion_definition_inventory(function).unwrap().counts;
        let expected = exclusive_owner_carrier_v1::origins_with_limit(
            calls,
            function,
            &definitions,
            usize::MAX,
        )
        .unwrap();
        let result = probe(
            calls,
            function,
            &definitions,
            usize::MAX,
            LIMIT,
            LIMIT,
            |actual| {
                assert_eq!(actual.as_ref().unwrap(), &expected);
            },
        );
        assert_eq!(result.finish, Finish::Success);
        assert_eq!(result.local_work, Some(expected.1));
        assert!(result.work >= expected.1);
        assert_eq!(result.owned, result.peak);
        assert!(!result.failed_work && !result.failed_storage);
        result
    }

    #[test]
    fn synthetic_metered_copy_move_and_transitive_origins_match_legacy() {
        for value in [SemanticOperandV1::Copy(place(1)), operand(1)] {
            compare(
                &ordinary(vec![assign(2, SemanticRvalueKindV1::Use(value))]),
                &callables(),
            );
        }
        compare(
            &ordinary(vec![
                assign(7, SemanticRvalueKindV1::Use(operand(1))),
                assign(2, SemanticRvalueKindV1::Use(operand(7))),
            ]),
            &callables(),
        );
    }

    #[test]
    fn synthetic_metered_mutation_alias_and_unknown_receiver_predicates_match_legacy() {
        let mutations = [
            ordinary(vec![
                assign(2, SemanticRvalueKindV1::Use(operand(1))),
                assign(2, SemanticRvalueKindV1::Use(operand(6))),
            ]),
            ordinary(vec![
                assign(1, SemanticRvalueKindV1::Use(operand(6))),
                assign(2, SemanticRvalueKindV1::Use(operand(1))),
            ]),
            ordinary(vec![
                assign(7, SemanticRvalueKindV1::Use(operand(1))),
                assign(2, SemanticRvalueKindV1::Use(operand(7))),
                assign(
                    4,
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Mutable,
                        place: place(7),
                    },
                ),
            ]),
            function(
                vec![
                    assign(2, SemanticRvalueKindV1::Use(operand(1))),
                    borrow(4, 2),
                ],
                call(1, vec![operand(4)], 1),
                vec![borrow(3, 2)],
                vec![operand(3), constant(0)],
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            ),
            function(
                vec![
                    assign(2, SemanticRvalueKindV1::Use(operand(1))),
                    borrow(4, 2),
                    assign(7, SemanticRvalueKindV1::Use(operand(4))),
                ],
                call(0, vec![operand(4), constant(0)], 1),
                vec![borrow(3, 2)],
                vec![operand(3), constant(0)],
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            ),
        ];
        for value in mutations {
            compare(&value, &callables());
        }
    }

    #[test]
    fn synthetic_metered_receiver_ordinal_and_projection_are_unchanged() {
        for args in [vec![constant(0), operand(3)], vec![operand(3), operand(3)]] {
            let value = function(
                vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                vec![borrow(3, 2)],
                args,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            );
            compare(&value, &callables());
        }
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), POINTER_TYPE)
                    .unwrap(),
            ],
            POINTER_TYPE,
        )
        .unwrap();
        compare(
            &ordinary(vec![assign(
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
            )]),
            &callables(),
        );
        compare(
            &ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]),
            &[],
        );
    }

    #[test]
    fn synthetic_metered_no_owner_path_derives_none_from_actual_abi_and_keeps_local_count() {
        for ownership in [
            SemanticSourceArgumentOwnershipV1::RawPointer,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::Unspecified,
        ] {
            let value = function(
                vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                vec![borrow(3, 2)],
                vec![operand(3), constant(0)],
                ownership,
            );
            let calls = callables();
            let definitions = assertion_definition_inventory(&value).unwrap().counts;
            let local = value.locals().len() + value.abi().source_argument_ownership().len();
            let observed = probe(
                &calls,
                &value,
                &definitions,
                local,
                LIMIT,
                LIMIT,
                |result| {
                    let (origins, work) = result.as_ref().unwrap();
                    assert!(origins.iter().all(Option::is_none));
                    assert_eq!(*work, local);
                },
            );
            assert_eq!(observed.finish, Finish::Success);
            assert_eq!(observed.work, local + value.locals().len()); // exact fill work is additional
        }
    }

    #[test]
    fn synthetic_metered_exact_original_work_storage_and_one_short_boundaries() {
        let value = ordinary(vec![
            assign(7, SemanticRvalueKindV1::Use(operand(1))),
            assign(2, SemanticRvalueKindV1::Use(operand(7))),
        ]);
        let calls = callables();
        let definitions = assertion_definition_inventory(&value).unwrap().counts;
        let expected = exclusive_owner_carrier_v1::origins_with_limit(
            &calls,
            &value,
            &definitions,
            usize::MAX,
        )
        .unwrap();
        let measured = compare(&value, &calls);
        let exact = probe(
            &calls,
            &value,
            &definitions,
            expected.1,
            measured.work,
            measured.peak,
            |result| assert_eq!(result.as_ref().unwrap(), &expected),
        );
        assert_eq!(exact.finish, Finish::Success);
        assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
        let short_work = probe(
            &calls,
            &value,
            &definitions,
            expected.1,
            measured.work - 1,
            measured.peak,
            |result| assert!(result.is_err()),
        );
        assert_eq!(short_work.finish, Finish::Work);
        assert!(short_work.failed_work && !short_work.failed_storage);
        let short_storage = probe(
            &calls,
            &value,
            &definitions,
            expected.1,
            measured.work,
            measured.peak - 1,
            |result| assert!(result.is_err()),
        );
        assert_eq!(short_storage.finish, Finish::Storage);
        assert!(short_storage.failed_storage);
        assert!(short_storage.owned < measured.owned);
    }

    #[test]
    fn synthetic_metered_local_work_limit_and_foreign_definition_shape_still_refuse() {
        let value = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
        let calls = callables();
        let definitions = assertion_definition_inventory(&value).unwrap().counts;
        let (_, legacy_work) = exclusive_owner_carrier_v1::origins_with_limit(
            &calls,
            &value,
            &definitions,
            usize::MAX,
        )
        .unwrap();
        let short = probe(
            &calls,
            &value,
            &definitions,
            legacy_work - 1,
            LIMIT,
            LIMIT,
            |result| assert!(result.is_err()),
        );
        assert_eq!(
            short.finish,
            Finish::Unsupported("ExclusiveOwner carrier census exceeds the projection work limit")
        );
        assert!(!short.failed_work && !short.failed_storage);
        let foreign = probe(
            &calls,
            &value,
            &definitions[..definitions.len() - 1],
            usize::MAX,
            LIMIT,
            LIMIT,
            |result| assert!(result.is_err()),
        );
        assert_eq!(
            foreign.finish,
            Finish::Unsupported("ExclusiveOwner carrier definitions do not match the local table")
        );
        assert_eq!((foreign.work, foreign.owned, foreign.peak), (0, 0, 0));
    }

    #[test]
    fn synthetic_metered_zero_storage_denies_before_origin_allocation() {
        let value = ordinary(vec![assign(2, SemanticRvalueKindV1::Use(operand(1)))]);
        let definitions = assertion_definition_inventory(&value).unwrap().counts;
        let observed = probe(
            &callables(),
            &value,
            &definitions,
            usize::MAX,
            LIMIT,
            0,
            |result| assert!(result.is_err()),
        );
        assert_eq!(observed.finish, Finish::Storage);
        assert_eq!((observed.work, observed.owned, observed.peak), (0, 0, 0));
        assert!(observed.failed_storage);
    }

    #[test]
    fn synthetic_long_transparent_use_spine_is_prepaid_without_changing_local_census() {
        fn fixture(count: usize) -> SemanticFunctionDeclV1 {
            let projections = (0..count)
                .map(|index| {
                    SemanticProjectionV1::new(
                        if index % 2 == 0 {
                            SemanticProjectionKindV1::Field(0)
                        } else {
                            SemanticProjectionKindV1::OpaqueCast
                        },
                        POINTER_TYPE,
                    )
                    .unwrap()
                })
                .collect();
            let source =
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, POINTER_TYPE)
                    .unwrap();
            ordinary(vec![assign(
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)),
            )])
        }
        // All projections are transparent to the initial helper, forcing the
        // entire spine walk. Nonempty projected operands still cannot create
        // a whole-value carrier copy or confer an exclusive receiver origin.
        let calls = callables();
        let short = fixture(1);
        let long = fixture(257);
        let short_measured = compare(&short, &calls);
        let long_measured = compare(&long, &calls);
        assert_eq!(
            long_measured.local_work.unwrap() - short_measured.local_work.unwrap(),
            256
        );
        // One original-ledger charge for the helper walk plus the unchanged
        // census charge for scan.operand's later walk; no semantic counter bump.
        assert_eq!(long_measured.work - short_measured.work, 2 * 256);
        assert_eq!(long_measured.peak, short_measured.peak);
        let definitions = assertion_definition_inventory(&long).unwrap().counts;
        let expected =
            exclusive_owner_carrier_v1::origins_with_limit(&calls, &long, &definitions, usize::MAX)
                .unwrap();
        assert!(expected.0.iter().all(Option::is_none));
        let exact = probe(
            &calls,
            &long,
            &definitions,
            expected.1,
            long_measured.work,
            long_measured.peak,
            |result| assert_eq!(result.as_ref().unwrap(), &expected),
        );
        assert_eq!(exact.finish, Finish::Success);
        assert_eq!(exact.local_work, Some(expected.1));
        let short_work = probe(
            &calls,
            &long,
            &definitions,
            expected.1,
            long_measured.work - 1,
            long_measured.peak,
            |result| assert!(result.is_err()),
        );
        assert_eq!(short_work.finish, Finish::Work);
        assert!(short_work.failed_work && !short_work.failed_storage);
        let short_local = probe(
            &calls,
            &long,
            &definitions,
            expected.1 - 1,
            LIMIT,
            LIMIT,
            |result| assert!(result.is_err()),
        );
        assert_eq!(
            short_local.finish,
            Finish::Unsupported("ExclusiveOwner carrier census exceeds the projection work limit")
        );
        assert!(!short_local.failed_work && !short_local.failed_storage);
    }
}
