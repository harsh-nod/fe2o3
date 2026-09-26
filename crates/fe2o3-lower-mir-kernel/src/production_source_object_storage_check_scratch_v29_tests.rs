#[test]
fn original_object_storage_checks_have_exact_temporary_boundaries() {
    use std::mem::size_of;
    type Error = ProductionSemanticKirErrorV1;
    let headers = size_of::<SemanticRetainedStorageV29>()
        + 2 * size_of::<Result<SemanticRetainedStorageV29, Error>>();
    // Authenticated owner 5 + unit scratch 2 + original storage query 29
    // + independent cell/schema/extent comparison 4.
    const WORK: usize = 40;
    const STOP: &str = "exact storage-check work completed";
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
                .find(|(_, row)| row.local.index() == 2 && row.generation == 5)
                .unwrap();
            let SourceBackingKindV29::Object(schema) = cell.kind else {
                panic!("original object");
            };
            if mode == 1 || mode == 2 {
                budget.charge_work(usize::MAX - budget.work() - WORK + usize::from(mode == 2))?;
            }
            let filler = if mode == 3 || mode == 4 {
                usize::MAX - budget.storage() - headers + usize::from(mode == 4)
            } else {
                0
            };
            budget.reserve_storage(filler)?;
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            let queried = budget.source_object_storage_matches_v29(
                &builder.plan,
                ordinal,
                cell.instance,
                cell.local,
                if mode == 5 { u32::MAX } else { cell.generation },
                schema,
                Some((1, 1)),
            );
            assert_eq!(budget.storage(), before.1);
            assert_eq!(
                budget.work() - before.0,
                match mode {
                    2 => 36,
                    4 => 17,
                    5 => 29,
                    _ => WORK,
                }
            );
            assert_eq!(
                budget.peak_storage(),
                before.2.max(before.1 + if mode == 4 { 0 } else { headers })
            );
            match mode {
                0 | 1 | 3 => assert!(matches!(queried, Ok(true))),
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
            if mode == 5 {
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                assert!(budget.source_object_storage_matches_v29(&builder.plan,
                    ordinal, cell.instance, cell.local, cell.generation, schema, Some((1, 1)))?);
                assert_eq!((budget.work() - before.0, budget.storage(), budget.peak_storage()),
                    (WORK, before.1, before.2));
            }
            if mode == 0 {
                for count in [1, 32, 128] {
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    for index in 0..count {
                        assert!(budget.source_object_storage_matches_v29(
                            &builder.plan,
                            ordinal,
                            cell.instance,
                            cell.local,
                            cell.generation,
                            schema,
                            (index % 2 == 0).then_some((1, 1))
                        )?);
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
                for wrong in [(2, 1), (1, 2)] {
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    assert!(!budget.source_object_storage_matches_v29(
                        &builder.plan,
                        ordinal,
                        cell.instance,
                        cell.local,
                        cell.generation,
                        schema,
                        Some(wrong)
                    )?);
                    assert_eq!(
                        (
                            budget.work() - before.0,
                            budget.storage(),
                            budget.peak_storage()
                        ),
                        (WORK, before.1, before.2)
                    );
                }
                // The ordinary storage-returning API still retains its actual
                // envelope until its caller drops the representation.
                let floor = budget.storage();
                let retained = budget.source_object_storage_v29(
                    &builder.plan,
                    ordinal,
                    cell.instance,
                    cell.local,
                    cell.generation,
                    schema,
                )?;
                assert_eq!(budget.storage(), floor + headers);
                assert!(
                    matches!(&retained, SemanticRetainedStorageV29::Object { cell: actual, .. } if *actual == ordinal)
                );
                drop(retained);
                budget.release_storage(headers)?;
                assert_eq!(budget.storage(), floor);
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
fn original_object_storage_check_custody_and_first_denial_precede_scratch() {
    for mode in 0..3 {
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
                .find(|(_, row)| row.local.index() == 2 && row.generation == 5)
                .unwrap();
            let SourceBackingKindV29::Object(schema) = cell.kind else {
                panic!("original object");
            };
            let plan = &builder.plan;
            let result = match mode {
                0 => {
                    let before = (budget.work(), budget.storage());
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                    let mut other = ArgumentBudgetV1::new(&mut work, 53);
                    other.reserve_storage(53)?;
                    let result = other.source_object_storage_matches_v29(
                        plan,
                        ordinal,
                        cell.instance,
                        cell.local,
                        cell.generation,
                        schema,
                        None,
                    );
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((other.work(), other.storage()), (0, 53));
                    assert_eq!((budget.work(), budget.storage()), before);
                    result
                }
                1 => {
                    budget.charge_work(usize::MAX - budget.work() - 6)?;
                    let before = (budget.work(), budget.storage());
                    let result = budget.source_object_storage_matches_v29(
                        plan,
                        ordinal,
                        cell.instance,
                        cell.local,
                        cell.generation,
                        schema,
                        None,
                    );
                    assert_eq!((budget.work() - before.0, budget.storage()), (5, before.1));
                    assert!(
                        matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(limit)))
                        if (limit.actual(), limit.limit()) == (usize::MAX, usize::MAX))
                    );
                    result
                }
                2 => {
                    let filler = usize::MAX - budget.storage();
                    budget.reserve_storage(filler)?;
                    let result = budget.source_object_storage_matches_v29(
                        plan,
                        ordinal,
                        cell.instance,
                        cell.local,
                        cell.generation,
                        schema,
                        None,
                    );
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            )
                        )
                    ));
                    budget.release_storage(filler)?;
                    budget.charge_work(usize::MAX - budget.work())?;
                    result
                }
                _ => unreachable!(),
            };
            let before = (budget.work(), budget.storage());
            let replay = budget.source_object_storage_matches_v29(
                plan,
                ordinal,
                cell.instance,
                cell.local,
                cell.generation,
                schema,
                None,
            );
            assert_eq!(format!("{replay:?}"), format!("{result:?}"));
            assert_eq!((budget.work(), budget.storage()), before);
            completed = true;
            result.map(|_| ())
        });
        assert!(completed, "mode {mode}: {result:?}");
        match (mode, result) {
            (
                0,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting,
                )),
            )
            | (
                1,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_),
                )),
            )
            | (
                2,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_),
                )),
            ) => {}
            other => panic!("exact outer first denial: {other:?}"),
        }
    }
}

#[test]
fn alternate_meter_cannot_lend_storage_check_authority() {
    struct Alternate<'a, 'work>(&'a mut ArgumentBudgetV1<'work>);
    impl SemanticEmissionBudgetV1 for Alternate<'_, '_> {
        fn work_ledger_identity_v1(
            &self,
        ) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
            self.0.work_ledger_identity_v1()
        }
        fn charge_work(&mut self, n: usize) -> Result<(), ProductionSemanticKirErrorV1> {
            self.0.charge_work(n).map_err(Into::into)
        }
        fn reserve_storage(&mut self, n: usize) -> Result<(), ProductionSemanticKirErrorV1> {
            self.0.reserve_storage(n).map_err(Into::into)
        }
        fn release_storage(&mut self, n: usize) -> Result<(), ProductionSemanticKirErrorV1> {
            self.0.release_storage(n).map_err(Into::into)
        }
        fn storage(&self) -> usize {
            self.0.storage()
        }
    }
    let mut completed = false;
    let result = with_original_object_access_builder(|builder, budget| {
        let before = (budget.work(), budget.storage());
        let query = Alternate(budget).source_object_storage_matches_v29(
            &builder.plan,
            0,
            builder.plan.root,
            SemanticLocalIdV1::from_index(0),
            0,
            fe2o3_kernel_ir::StorageLayoutIdV1(0),
            None,
        );
        assert!(matches!(
            query,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        assert!(matches!(
            builder.plan.check_owner(builder.plan.instances, budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        completed = true;
        query.map(|_| ())
    });
    assert!(completed);
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}
