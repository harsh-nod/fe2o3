#[test]
fn original_object_lane_uses_selected_source_backing_not_candidate_operation_count() {
    let mut completed = false;
    with_selected_pointer_test_plan_v29(static_field_owner_v29(), |plan, budget| {
        let before = budget.storage();
        let objects: Vec<_> = plan
            .cells
            .rows
            .iter()
            .filter(|row| matches!(row.kind, SourceBackingKindV29::Object(_)))
            .collect();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].local.index(), 2);
        assert_eq!(source_physical_object_count_v29(plan, budget)?, 1);
        assert_eq!(budget.storage(), before);
        let error = scoped_raw_admission_v29::require_original_zero_raw_v29(
            plan.instances,
            Some(plan),
            budget,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "original raw source requires consuming expanded physical admission",
                ..
            }
        ));
        assert_eq!(budget.storage(), before);
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
    // The actual source-consuming candidate is independently required first.
    let (positive, _, _, finished) =
        run_static_field_admission_v29(0, 0, MODULE_LIMIT, MODULE_LIMIT);
    positive.unwrap();
    assert!(finished);
    let (changed, _, _, finished) =
        run_static_field_admission_v29(0, 3, MODULE_LIMIT, MODULE_LIMIT);
    assert!(STATIC_FIELD_MUTATED_V29.get() && STATIC_FIELD_OBSERVED_V29.get() > 0);
    assert!(!finished);
    assert!(
        matches!(
            changed,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "execution call parameters differ from their source instance",
                    }
                )
            ))
        ),
        "{changed:?}"
    );
}

#[test]
fn original_object_lane_keeps_no_object_sources_ordinary() {
    let mut completed = 0;
    with_selected_pointer_test_plan_v29(
        module_fixture_owner(ModuleFixture::Ordinary),
        |plan, budget| {
            assert!(plan.cells.rows.is_empty());
            let (work, storage) = (budget.work(), budget.storage());
            assert_eq!(source_physical_object_count_v29(plan, budget)?, 0);
            assert_eq!((budget.work() - work, budget.storage()), (5, storage));
            scoped_raw_admission_v29::require_original_zero_raw_v29(
                plan.instances,
                Some(plan),
                budget,
            )?;
            assert_eq!(budget.storage(), storage);
            completed += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(completed, 1);
}

#[test]
fn original_object_lane_preserves_owner_first_and_sticky_scratch_entry_denial() {
    for fault in 0..3 {
        let mut completed = false;
        let result =
            with_selected_pointer_test_plan_v29(static_field_owner_v29(), |plan, budget| {
                assert_eq!(source_physical_object_count_v29(plan, budget)?, 1);
                if fault == 0 {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                    let mut foreign = ArgumentBudgetV1::new(&mut work, 0);
                    assert!(matches!(
                        source_physical_object_count_v29(plan, &mut foreign),
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                } else if fault == 1 {
                    // Original owner authentication is five work. The next two
                    // work belong to scratch entry and must latch that denial.
                    budget.charge_work(20_000_000 - budget.work() - 5)?;
                } else {
                    budget.reserve_storage(64 * 1024 * 1024 - budget.storage())?;
                }
                let (work, storage) = (budget.work(), budget.storage());
                let error = source_physical_object_count_v29(plan, budget).unwrap_err();
                match (&error, fault) {
                    (
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting,
                        ),
                        0,
                    ) => {
                        assert_eq!(budget.work(), work);
                    }
                    (
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(error),
                        ),
                        1,
                    ) => {
                        assert_eq!((error.actual(), error.limit()), (20_000_002, 20_000_000));
                        assert_eq!(budget.work() - work, 5);
                    }
                    (
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(error),
                        ),
                        2,
                    ) => {
                        type Rows = Vec<Option<fe2o3_kernel_ir::StorageLayoutIdV1>>;
                        let header = std::mem::size_of::<Rows>()
                            + 2 * std::mem::size_of::<Result<Rows, ProductionSemanticKirErrorV1>>();
                        assert_eq!(
                            (error.actual(), error.limit()),
                            (64 * 1024 * 1024 + header, 64 * 1024 * 1024)
                        );
                        assert_eq!(budget.work() - work, 17);
                    }
                    _ => panic!("fault {fault}: {error:?}"),
                }
                assert_eq!(budget.storage(), storage);
                let work = budget.work();
                let repeated = source_physical_object_count_v29(plan, budget).unwrap_err();
                assert_eq!(format!("{repeated:?}"), format!("{error:?}"));
                assert_eq!((budget.work(), budget.storage()), (work, storage));
                completed = true;
                Err(error)
            });
        assert!(completed, "fault {fault}: {result:?}");
        assert!(matches!(
            (result, fault),
            (
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ),
                0
            ) | (
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                ),
                1
            ) | (
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                ),
                2
            )
        ));
    }
}
