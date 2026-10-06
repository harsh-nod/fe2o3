fn query_original_object_location_for_scratch_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    ordinal: usize,
    cell: SourceReferenceScalarCellV29,
    generation: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceStaticObjectLocationV29>, ProductionSemanticKirErrorV1> {
    let SourceBackingKindV29::Object(schema) = cell.kind else {
        panic!("selected original Object");
    };
    with_source_static_object_location_query_v29(plan.instances, plan, budget, |budget| {
        let storage = source_object_storage_v29(
            plan,
            ordinal,
            cell.instance,
            cell.local,
            generation,
            schema,
            budget,
        )?;
        assert!(
            matches!(storage, SemanticRetainedStorageV29::Object { cell: actual, schema: actual_schema, .. }
            if actual == ordinal && actual_schema == schema)
        );
        let layouts = plan
            .storage_root
            .as_ref()
            .unwrap()
            .source_layouts(plan.instances, budget)?;
        assert_eq!(
            layouts.visit_selected_components(
                plan.instances.owner(),
                cell.ty,
                schema,
                &[],
                budget,
                |_, _| panic!("empty original whole-object path has no components")
            )?,
            source_storage_v29::SourceSelectedProjectionV29::Physical {
                ty: cell.ty,
                schema
            }
        );
        // This query tests scratch custody only, not a physical pointer proof.
        Ok(Some(SourceStaticObjectLocationV29 {
            slot: ordinal,
            offset: 0,
            schema: Some(schema),
        }))
    })
}

#[test]
fn original_object_location_queries_release_only_completed_query_storage() {
    use std::mem::size_of;
    type Error = ProductionSemanticKirErrorV1;
    let retained_headers = size_of::<SemanticRetainedStorageV29>()
        + 2 * size_of::<Result<SemanticRetainedStorageV29, Error>>();
    let projection_headers = 2 * size_of::<source_storage_v29::SourceSelectedProjectionV29>()
        + 2 * size_of::<Result<source_storage_v29::SourceSelectedProjectionV29, Error>>()
        + size_of::<source_storage_v29::SourceSelectedComponentV29<'_>>()
        + size_of::<Result<(), Error>>();
    // Owner 5 + scratch 2 + exact storage query 29 + layout lens 1
    // + two empty checked path walks 2+2 + visitor owner/finish 1+1.
    const WORK: usize = 43;
    const STOP: &str = "original location query exact work completed";
    for mode in 0..6 {
        let mut completed = false;
        let result = with_original_object_access_builder(|builder, budget| {
            builder.plan_scalar_cells(budget)?;
            let (ordinal, cell) = builder
                .plan
                .cells
                .rows
                .iter()
                .copied()
                .enumerate()
                .find(|(_, cell)| cell.local.index() == 2 && cell.generation == 5)
                .unwrap();
            if mode == 1 || mode == 2 {
                budget.charge_work(usize::MAX - budget.work() - WORK + usize::from(mode == 2))?;
            }
            let filler = if mode == 3 || mode == 4 {
                usize::MAX - budget.storage() - retained_headers - projection_headers
                    + usize::from(mode == 4)
            } else {
                0
            };
            budget.reserve_storage(filler)?;
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            let queried = query_original_object_location_for_scratch_v29(
                &builder.plan,
                ordinal,
                cell,
                if mode == 5 { u32::MAX } else { cell.generation },
                budget,
            );
            assert_eq!(
                budget.storage(),
                before.1,
                "no query owner may escape the unit cleanup scope"
            );
            assert_eq!(
                budget.work() - before.0,
                match mode {
                    2 => 42,
                    4 => 38,
                    5 => 29,
                    _ => WORK,
                }
            );
            assert_eq!(
                budget.peak_storage(),
                before.2.max(
                    before.1
                        + retained_headers
                        + if mode == 4 || mode == 5 {
                            0
                        } else {
                            projection_headers
                        }
                )
            );
            match mode {
                0 | 1 | 3 => assert!(
                    matches!(queried, Ok(Some(SourceStaticObjectLocationV29 { slot, offset: 0, .. })) if slot == ordinal)
                ),
                2 => assert!(matches!(
                    queried,
                    Err(Error::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    ))
                )),
                4 => assert!(matches!(
                    queried,
                    Err(Error::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    ))
                )),
                5 => assert!(matches!(
                    queried,
                    Err(Error::Unsupported {
                        detail: "typed allocation identity or representation requires its exact source contract",
                        ..
                    })
                )),
                _ => unreachable!(),
            }
            budget.release_storage(filler)?;
            if mode == 0 {
                for count in [1, 32, 128] {
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    for _ in 0..count {
                        assert_eq!(
                            query_original_object_location_for_scratch_v29(
                                &builder.plan,
                                ordinal,
                                cell,
                                cell.generation,
                                budget
                            )?,
                            queried.as_ref().unwrap().to_owned()
                        );
                    }
                    assert_eq!(
                        (
                            budget.work() - before.0,
                            budget.storage(),
                            budget.peak_storage()
                        ),
                        (count * WORK, before.1, before.2)
                    );
                }
            }
            completed = true;
            if mode == 1 {
                Err(source_reference_error_v29(STOP))
            } else {
                queried.map(|_| ())
            }
        });
        assert!(completed, "mode {mode}: {result:?}");
        match mode {
            0 | 3 => result.unwrap(),
            1 => assert!(matches!(
                result,
                Err(Error::Unsupported { detail: STOP, .. })
            )),
            2 => assert!(matches!(
                result,
                Err(Error::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                ))
            )),
            4 => assert!(matches!(
                result,
                Err(Error::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                ))
            )),
            5 => assert!(matches!(
                result,
                Err(Error::Unsupported {
                    detail: "typed allocation identity or representation requires its exact source contract",
                    ..
                })
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn original_object_location_query_custody_precedes_scratch_and_keeps_first_denial() {
    for foreign in [false, true] {
        let mut completed = false;
        let result = with_original_object_access_builder(|builder, budget| {
            builder.plan_scalar_cells(budget)?;
            let plan = &builder.plan;
            let mut entered = false;
            let result = if foreign {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                let mut other = ArgumentBudgetV1::new(&mut work, 0);
                let result = with_source_static_object_location_query_v29(
                    plan.instances,
                    plan,
                    &mut other,
                    |_| {
                        entered = true;
                        Ok(None)
                    },
                );
                assert_eq!((other.work(), other.storage()), (0, 0));
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                result
            } else {
                // Owner passes with one work unit left: scratch entry must
                // deny its two units and retain that exact first failure.
                budget.charge_work(usize::MAX - budget.work() - 6)?;
                let before = (budget.work(), budget.storage());
                let result = with_source_static_object_location_query_v29(
                    plan.instances,
                    plan,
                    budget,
                    |_| {
                        entered = true;
                        Ok(None)
                    },
                );
                assert_eq!((budget.work() - before.0, budget.storage()), (5, before.1));
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit)))
                    if (limit.actual(), limit.limit()) == (usize::MAX, usize::MAX))
                );
                result
            };
            assert!(!entered);
            let before = (budget.work(), budget.storage());
            let replay =
                with_source_static_object_location_query_v29(plan.instances, plan, budget, |_| {
                    entered = true;
                    Ok(None)
                });
            assert!(!entered);
            assert_eq!(format!("{replay:?}"), format!("{result:?}"));
            assert_eq!((budget.work(), budget.storage()), before);
            completed = true;
            result.map(|_| ())
        });
        assert!(completed, "foreign={foreign}: {result:?}");
        match (foreign, result) {
            (
                true,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting,
                )),
            )
            | (
                false,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_),
                )),
            ) => {}
            other => panic!("exact outer first denial: {other:?}"),
        }
    }
}

#[test]
fn original_object_location_query_error_panic_and_empty_result_preserve_the_caller_floor() {
    let mut completed = false;
    with_original_object_access_builder(|builder, budget| {
        builder.plan_scalar_cells(budget)?;
        let plan = &builder.plan;
        let floor = budget.storage();
        assert_eq!(
            with_source_static_object_location_query_v29(plan.instances, plan, budget, |budget| {
                budget.reserve_storage(37)?;
                Ok(None)
            })?,
            None
        );
        assert_eq!(budget.storage(), floor);
        let refused =
            with_source_static_object_location_query_v29(plan.instances, plan, budget, |budget| {
                budget.reserve_storage(37)?;
                Err(source_reference_error_v29(
                    "original location query semantic control",
                ))
            });
        assert!(matches!(
            refused,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "original location query semantic control",
                ..
            })
        ));
        assert_eq!(budget.storage(), floor);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_source_static_object_location_query_v29(
                plan.instances,
                plan,
                budget,
                |budget| {
                    budget.reserve_storage(37)?;
                    panic!("original location query panic control");
                },
            );
        }));
        assert!(panic.is_err());
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            with_source_static_object_location_query_v29(plan.instances, plan, budget, |_| Ok(
                None
            ))?,
            None
        );
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
}
