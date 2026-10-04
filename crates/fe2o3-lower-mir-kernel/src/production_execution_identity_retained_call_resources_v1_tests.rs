fn effect_resource_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = lifecycle_owner(false);
    let semantic = original.source_semantic();
    let root = &semantic.functions()[0];
    let replacement = function(
        80,
        root.role(),
        root.abi().clone(),
        root.locals()[..2].to_vec(),
        vec![block(85, vec![], SemanticTerminatorKindV1::Return)],
    )
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    assert_eq!(replacement.abi(), root.abi());
    retained_rebuild(
        semantic.types()[..2].to_vec(),
        vec![replacement],
        vec![SemanticCallableDeclV1::defined(ROOT)],
    )
}

fn independent_effect_headers() -> usize {
    use std::mem::size_of;
    let fields = size_of::<&ExecutionInstancesV29<'_>>()
        + size_of::<Vec<bool>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 2 * size_of::<usize>();
    assert_eq!(fields, size_of::<ExecutionRetainedCallEffectsV1<'_, '_>>());
    fields
        + size_of::<Result<ExecutionRetainedCallEffectsV1<'_, '_>, ProductionSemanticKirErrorV1>>()
}

#[test]
fn original_effect_closure_has_independent_exact_and_one_short_work_storage() {
    for cut in 0..3 {
        for prior in [false, true] {
            let expected = if prior {
                Some(LIMIT + 23)
            } else if cut == 1 {
                Some(LIMIT + 1)
            } else {
                None
            };
            let mut observed = false;
            with_effect_index(effect_resource_owner(), expected, |index, budget| {
                assert_eq!(index.instances.instances().len(), 1);
                let source = index.instances.instances()[0].declaration();
                assert_eq!(source.blocks().len(), 1);
                assert!(source.blocks()[0].statements().is_empty());
                assert!(matches!(source.blocks()[0].terminator().kind(), SemanticTerminatorKindV1::Return));
                // Vector prepay3 + initialize1 + instance3 + source block2,
                // original active-block lookup1 + original terminator1.
                let exact_work = 3 + 1 + 3 + 2 + 1 + 1;
                let exact_storage = independent_effect_headers() + std::mem::size_of::<bool>();
                let remaining_work = exact_work - usize::from(cut == 1);
                let remaining_storage = exact_storage - usize::from(cut == 2);
                budget.charge_work(LIMIT - budget.work() - remaining_work)?;
                budget.reserve_storage(LIMIT - budget.storage() - remaining_storage)?;
                let floor = budget.storage();
                let before = budget.work();
                if prior {
                    assert!(budget.charge_work(remaining_work + 23).is_err());
                    assert!(budget.reserve_storage(remaining_storage + 29).is_err());
                }
                let result = scoped_slot_attempt_v29(budget, |budget| ExecutionRetainedCallEffectsV1::derive(index, budget));
                match cut {
                    0 => {
                        let effects = result?;
                        assert_eq!(effects.closed, [true]);
                        assert_eq!(effects.closed.capacity(), 1);
                        assert_eq!(budget.work(), before + exact_work);
                        assert_eq!(budget.storage(), floor + exact_storage);
                        assert_eq!(budget.peak_storage(), LIMIT);
                        effects.discard(budget)?;
                    }
                    1 => {
                        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
                            if error.actual() == LIMIT + 1 && error.limit() == LIMIT));
                        assert_eq!(budget.work(), before + exact_work - 1);
                    }
                    2 => {
                        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error)))
                            if error.actual() == LIMIT + 1 && error.limit() == LIMIT));
                        assert_eq!(budget.work(), before + 3, "payload denial precedes initialization/body scans");
                    }
                    _ => unreachable!(),
                }
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), if prior { Some(LIMIT + 29) } else if cut == 2 { Some(LIMIT + 1) } else { None });
                observed = true;
                Ok(())
            }).unwrap();
            assert!(observed);
        }
    }
}

#[test]
fn effect_queries_check_exact_owner_ledger_floor_and_inclusive_work() {
    for short in [false, true] {
        let mut observed = false;
        with_effect_index(effect_resource_owner(), short.then_some(LIMIT + 1), |index, budget| {
            let effects = ExecutionRetainedCallEffectsV1::derive(index, budget)?;
            let root = index.instances.root();
            let remaining = 7 - usize::from(short);
            budget.charge_work(LIMIT - budget.work() - remaining)?;
            let start = budget.work();
            let result = effects.accepts(index.instances, root, budget);
            if short {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
                    if error.actual() == LIMIT + 1));
                assert_eq!(budget.work(), start);
            } else {
                assert!(result?);
                assert_eq!(budget.work(), start + 7);
            }
            effects.discard(budget)?;
            observed = true;
            Ok(())
        }).unwrap();
        assert!(observed);
    }
    let mut observed = 0;
    with_effect_index(effect_resource_owner(), None, |index, budget| {
        let effects = ExecutionRetainedCallEffectsV1::derive(index, budget)?;
        let root = index.instances.root();
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, LIMIT);
        foreign.reserve_storage(effects.floor)?;
        assert!(matches!(
            effects.accepts(index.instances, root, &mut foreign),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(foreign.work(), 7);
        assert_eq!(foreign.storage(), effects.floor);
        drop(foreign);
        observed += 1;
        budget.release_storage(1)?;
        assert!(matches!(
            effects.accepts(index.instances, root, budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        budget.reserve_storage(1)?;
        effects.discard(budget)?;
        observed += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(observed, 2);
}

#[test]
fn effect_construction_settlement_preserves_selected_error_panic_and_prior_denial() {
    for panic in [false, true] {
        for prior in [false, true] {
            let mut completed = false;
            with_effect_index(
                effect_resource_owner(),
                prior.then_some(LIMIT + 23),
                |index, budget| {
                    let floor = budget.storage();
                    assert!(
                        floor > 37,
                        "the caller and original instance/index owners remain paid"
                    );
                    let before = budget.work();
                    if prior {
                        assert!(budget.charge_work(LIMIT - before + 23).is_err());
                        assert!(budget.reserve_storage(LIMIT - floor + 29).is_err());
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        scoped_slot_attempt_v29(budget, |budget| {
                            let effects = ExecutionRetainedCallEffectsV1::derive(index, budget)?;
                            assert_eq!(effects.closed, [true]);
                            assert_eq!(budget.storage(), floor + independent_effect_headers() + 1);
                            budget.reserve_storage(13)?;
                            if panic {
                                std::panic::panic_any(1222usize);
                            }
                            Err::<(), _>(
                                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                    ArgumentResourceV1::Arithmetic,
                                ),
                            )
                        })
                    }));
                    if panic {
                        assert_eq!(result.unwrap_err().downcast_ref::<usize>(), Some(&1222));
                    } else {
                        assert!(matches!(
                            result.unwrap(),
                            Err(
                                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                    ArgumentResourceV1::Arithmetic
                                )
                            )
                        ));
                    }
                    assert_eq!(budget.work(), before + 11);
                    assert_eq!(
                        budget.storage(),
                        floor,
                        "the dropped certificate and scratch, not caller storage, are refunded"
                    );
                    assert_eq!(budget.failed_storage(), prior.then_some(LIMIT + 29));
                    completed = true;
                    Ok(())
                },
            )
            .unwrap();
            assert!(
                completed,
                "assertions after the unwind boundary must execute"
            );
        }
    }
}
