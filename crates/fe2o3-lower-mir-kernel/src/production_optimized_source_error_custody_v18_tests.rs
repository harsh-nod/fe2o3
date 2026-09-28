#[derive(Debug)]
enum OwnedCallbackErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Payload(Vec<u64>),
}

impl From<ProductionSourceOwnedViewErrorV18> for OwnedCallbackErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

fn owned_callback_payload_v18(budget: &mut ArgumentBudgetV1<'_>) -> Vec<u64> {
    budget
        .reserve_storage(64 * std::mem::size_of::<u64>())
        .unwrap();
    let payload = vec![0x271_u64; 64];
    assert_eq!(payload.capacity(), 64);
    payload
}

#[test]
fn optimized_owned_error_preserves_adoption_residual_and_first_source_refusal() {
    for first_refusal in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result: Result<(), OwnedCallbackErrorV18> =
            prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                let result = source.with_retained_checked_optimization_v18(
                    budget,
                    |original, optimized, budget| {
                        assert!(std::ptr::eq(
                            original.inventory(budget)?,
                            optimized.input_inventory(budget)?
                        ));
                        if first_refusal {
                            let error = budget.charge_work(usize::MAX).unwrap_err();
                            let _ = original.retain_query_resource_error_v18(error);
                        }
                        let payload = owned_callback_payload_v18(budget);
                        reached.set(true);
                        Err::<((), usize), _>(OwnedCallbackErrorV18::Payload(payload))
                    },
                );
                match result {
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(error),
                    )) => Err(error),
                    Err(ProductionSourceOptimizationErrorV18::Source(error)) => Err(error.into()),
                    other => panic!(
                        "owned error did not traverse actual optimizer: {}",
                        other.is_ok()
                    ),
                }
            });
        assert!(reached.get());
        assert!(
            budget.storage() > MODULE_FLOOR + 64 * std::mem::size_of::<u64>(),
            "unbalanced adoption is conservative residual, not an error-transfer receipt"
        );
        match (first_refusal, result) {
            (false, Err(OwnedCallbackErrorV18::Payload(payload))) => {
                assert_eq!(payload, vec![0x271; 64]);
                drop(payload);
            }
            (
                true,
                Err(OwnedCallbackErrorV18::Source(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Work(error),
                ))),
            ) => assert_eq!(error.actual(), usize::MAX),
            other => panic!("first source error or selected owned error changed: {other:?}"),
        }
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.failed_work(), first_refusal.then_some(usize::MAX));
    }
}

#[test]
fn balanced_optimizer_success_selected_error_and_panic_keep_exact_cleanup() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let result =
                source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                    assert!(std::ptr::eq(
                        original.inventory(budget)?,
                        optimized.input_inventory(budget)?
                    ));
                    reached.set(true);
                    match mode {
                        0 => Ok(((), 0)),
                        1 => Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "selected balanced callback",
                        )),
                        _ => std::panic::resume_unwind(Box::new("balanced callback panic")),
                    }
                });
            match (mode, result) {
                (0, Ok((owner, (), receipt))) => {
                    assert!(!owner.grants_authority());
                    assert!(!receipt.grants_authority());
                    drop(owner);
                }
                (
                    1,
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "selected balanced callback",
                            ),
                        ),
                    )),
                ) => {}
                (
                    2,
                    Err(ProductionSourceOptimizationErrorV18::Adoption(
                        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Panicked,
                    )),
                ) => {}
                _ => panic!("balanced optimizer disposition changed"),
            }
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        });
        assert!(reached.get());
        if mode == 2 {
            assert!(result.is_err());
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
    }
}

#[test]
fn optimized_analysis_reclaims_its_cache_but_not_callback_success_error_or_panic_backing() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let result =
                source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                    let floor = budget.storage();
                    original.with_optimized_analysis_v18(
                        optimized,
                        budget,
                        |analysis, budget| {
                            analysis.with_projection_facts(budget, |_, _, _, _| {
                                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                            })
                        },
                    )?;
                    assert_eq!(
                        budget.storage(),
                        floor,
                        "balanced cache scope releases exactly its own storage"
                    );
                    for mode in 0..3 {
                        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            original.with_optimized_analysis_v18(
                                optimized,
                                budget,
                                |analysis, budget| {
                                    analysis.with_projection_facts(budget, |_, _, _, _| {
                                        Ok::<_, OwnedCallbackErrorV18>(())
                                    })?;
                                    let payload = owned_callback_payload_v18(budget);
                                    match mode {
                                        0 => Ok(payload),
                                        1 => Err(OwnedCallbackErrorV18::Payload(payload)),
                                        _ => {
                                            budget
                                                .reserve_storage(std::mem::size_of::<Vec<u64>>())
                                                .unwrap();
                                            std::panic::resume_unwind(Box::new(payload))
                                        }
                                    }
                                },
                            )
                        }));
                        let extra = if mode == 2 {
                            std::mem::size_of::<Vec<u64>>()
                        } else {
                            0
                        };
                        let backing = 64 * std::mem::size_of::<u64>() + extra;
                        assert_eq!(
                            budget.storage(),
                            floor + backing,
                            "dead analysis cache is not callback backing"
                        );
                        match (mode, caught) {
                            (0, Ok(Ok(payload)))
                            | (1, Ok(Err(OwnedCallbackErrorV18::Payload(payload)))) => {
                                assert_eq!(payload, vec![0x271; 64]);
                                drop(payload);
                            }
                            (2, Err(payload)) => {
                                let payload = payload.downcast::<Vec<u64>>().unwrap();
                                assert_eq!(*payload, vec![0x271; 64]);
                                drop(payload);
                            }
                            _ => panic!("analysis callback disposition changed"),
                        }
                        budget.release_storage(backing).unwrap();
                        assert_eq!(budget.storage(), floor);
                    }
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                });
            let (owner, (), _) = result
                .expect("same-candidate analysis controls remain balanced for optimizer transfer");
            drop(owner);
            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
        })
        .unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
}

#[test]
fn unbalanced_optimizer_panic_propagates_no_refund_before_source_settlement() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
    let retained = std::cell::Cell::new(0);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted = source
            .with_checked_optimization_v18::<(), ProductionSourceOwnedViewErrorV18>(
                budget,
                |_, _, budget| {
                    let payload = owned_callback_payload_v18(budget);
                    budget
                        .reserve_storage(std::mem::size_of::<Vec<u64>>())
                        .unwrap();
                    std::panic::resume_unwind(Box::new(payload))
                },
            );
        assert!(matches!(
            attempted,
            Err(ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Panicked
            ))
        ));
        assert!(source.cleanup.is_denied());
        retained.set(budget.storage());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "actual source optimizer adoption rejected"
        ))
    ));
    assert!(retained.get() > MODULE_FLOOR);
    assert_eq!(
        budget.storage(),
        retained.get(),
        "outer source cannot erase adoption's no-refund disposition"
    );
}
