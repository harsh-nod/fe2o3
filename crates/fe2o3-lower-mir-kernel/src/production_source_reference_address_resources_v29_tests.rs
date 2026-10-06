fn first_epoch_union_storage() -> usize {
    // Two virtual inputs, one merge cursor, nested/caller return envelopes, two members
    // in the first four-element capacity, one four-row arena capacity, the
    // two-level BTree split allowance, and a four-index object bucket.
    2 * size_of::<SourceReferenceEpochAtomsV29>()
        + size_of::<SourceReferenceEpochUnionV29>()
        + 2 * size_of::<Result<SourceReferenceEpochAtomsV29, ProductionSemanticKirErrorV1>>()
        + 4 * size_of::<Result<Option<u32>, ProductionSemanticKirErrorV1>>()
        + 2 * size_of::<Option<u32>>()
        + 2 * size_of::<Result<u32, ProductionSemanticKirErrorV1>>()
        + 4 * size_of::<u32>()
        + 4 * size_of::<SourceReferenceEpochSetV29>()
        + 64 * size_of::<((usize, u32), Vec<usize>, usize)>()
        + 4 * size_of::<usize>()
}

#[test]
fn original_epoch_union_has_independent_exact_and_one_short_work() {
    // 3 limit + 4 atom headers + 32 source-block lookup + 3 source checks;
    // 32 object lookup; 9 merge steps; 7 member pushes; 5 row push;
    // 32 second lookup + 32 map insertion + 5 object-index push.
    const WORK: usize = 164;
    for available in [WORK, WORK - 1] {
        let mut completed = false;
        let result = with_address_builder(|builder, budget| {
            let before_storage = budget.storage();
            budget.charge_work(ADDRESS_TEST_LIMIT - budget.work() - available)?;
            let before_work = budget.work();
            let result = builder.join_storage_epochs(
                builder.plan.root,
                SemanticLocalIdV1::from_index(2),
                0,
                1,
                budget,
            );
            if available == WORK {
                assert!(result.is_ok(), "{result:?}");
                assert_eq!(budget.work() - before_work, WORK);
                assert_eq!(
                    budget.storage() - before_storage,
                    first_epoch_union_storage()
                );
                assert_eq!(builder.plan.epoch_members, [0, 1]);
            } else {
                assert!(
                    matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ),
                    "{result:?}"
                );
                assert!(
                    builder
                        .plan
                        .epoch_objects
                        .get(&(builder.plan.root.index(), 2))
                        .is_none_or(Vec::is_empty)
                );
            }
            completed = true;
            Ok(())
        });
        assert!(result.is_ok(), "{result:?}");
        assert!(completed, "resource assertions did not complete");
    }
}

#[test]
fn original_epoch_union_has_independent_exact_and_one_short_storage() {
    for short in [0, 1] {
        let mut completed = false;
        let result = with_address_builder(|builder, budget| {
            let expected = first_epoch_union_storage();
            budget.reserve_storage(ADDRESS_TEST_LIMIT - budget.storage() - (expected - short))?;
            let before = budget.storage();
            let result = builder.join_storage_epochs(
                builder.plan.root,
                SemanticLocalIdV1::from_index(2),
                0,
                1,
                budget,
            );
            if short == 0 {
                assert!(result.is_ok(), "{result:?}");
                assert_eq!(budget.storage() - before, expected);
                assert_eq!(budget.storage(), ADDRESS_TEST_LIMIT);
            } else {
                assert!(
                    matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            )
                        )
                    ),
                    "{result:?}"
                );
                assert!(
                    builder
                        .plan
                        .epoch_objects
                        .get(&(builder.plan.root.index(), 2))
                        .is_none_or(Vec::is_empty)
                );
            }
            completed = true;
            Ok(())
        });
        assert!(result.is_ok(), "{result:?}");
        assert!(completed, "resource assertions did not complete");
    }
}

#[test]
fn raw_virtual_union_one_short_work_cannot_publish_an_expiry_step() {
    let mut completed = false;
    with_address_plan(AddressFlow::Read, |plan, _| {
        for available in [3, 2] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(available);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let input = SourceReferenceRawChoicesV29::Singleton(SourceReferenceRawChoiceV29 {
                origin: 0,
                expired: false,
            });
            let mut union = SourceReferenceRawUnionV29 {
                left: input,
                right: input,
                left_index: 0,
                right_index: 0,
                expire: true,
            };
            let result = plan.next_raw_union(&mut union, &mut budget);
            if available == 3 {
                assert_eq!(
                    result.unwrap(),
                    Some(SourceReferenceRawChoiceV29 {
                        origin: 0,
                        expired: true
                    })
                );
                assert_eq!((union.left_index, union.right_index), (1, 1));
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert_eq!((union.left_index, union.right_index), (0, 0));
            }
            assert_eq!(budget.storage(), 0);
        }
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
}
