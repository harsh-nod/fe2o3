fn aggregate_initial_paid_payload_v30(budget: &mut ArgumentBudgetV1<'_>) -> Vec<u8> {
    budget.reserve_storage(43).unwrap();
    budget.charge_work(43).unwrap();
    let mut payload = Vec::new();
    payload.try_reserve_exact(43).unwrap();
    budget.reserve_storage(payload.capacity() - 43).unwrap();
    payload.resize(43, 0x30);
    payload
}

#[test]
fn aggregate_initial_callback_preserves_paid_success_and_panic_payloads() {
    for panic in [false, true] {
        with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
            let chain = source.aggregate_output_v30(abi, budget)?;
            let floor = budget.storage();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_aggregate_initial_source_v30(
                    source,
                    chain.output(budget)?,
                    budget,
                    |original, optimized, budget| {
                        original.global_expression_entry_v23(optimized, budget)?;
                        let payload = aggregate_initial_paid_payload_v30(budget);
                        if panic {
                            std::panic::panic_any(payload);
                        }
                        Ok(payload)
                    },
                )
            }));
            let payload = match (panic, result) {
                (false, Ok(Ok(payload))) => payload,
                (true, Err(payload)) => *payload.downcast::<Vec<u8>>().unwrap(),
                (_, other) => panic!("wrong initial-source payload disposition: {other:?}"),
            };
            assert_eq!(payload, vec![0x30; 43]);
            assert_eq!(budget.storage(), floor + payload.capacity());
            assert!(!source.cleanup.is_denied());
            let credit = payload.capacity();
            drop(payload);
            budget.release_storage(credit)?;
            assert_eq!(budget.storage(), floor);
            chain.discard(budget)?;
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn aggregate_initial_callback_refusal_preserves_owned_error_side_payload() {
    let result =
        with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
            let chain = source.aggregate_output_v30(abi, budget)?;
            let floor = budget.storage();
            // This helper's error type is the fixed compiler enum. A caller may
            // nevertheless retain paid diagnostic/output backing on that error.
            let diagnostic = std::cell::RefCell::new(None);
            let result = with_aggregate_initial_source_v30(
                source,
                chain.output(budget)?,
                budget,
                |original, optimized, budget| {
                    original.global_expression_entry_v23(optimized, budget)?;
                    *diagnostic.borrow_mut() = Some(aggregate_initial_paid_payload_v30(budget));
                    Err::<(), _>(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "first initial-source callback refusal",
                        )
                        .into(),
                    )
                },
            );
            assert!(matches!(
                &result,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "first initial-source callback refusal"
                    )
                ))
            ));
            let payload = diagnostic.into_inner().unwrap();
            assert_eq!(budget.storage(), floor + payload.capacity());
            assert!(!source.cleanup.is_denied());
            let credit = payload.capacity();
            drop(payload);
            budget.release_storage(credit)?;
            assert_eq!(budget.storage(), floor);
            assert!(chain.discard(budget).is_err());
            result
        });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding("first initial-source callback refusal")
        ))
    ));
}

#[test]
fn aggregate_initial_callback_lost_floor_and_foreign_ledger_veto_all_refunds() {
    for foreign_query in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) =
            integer_handoff_prepared_v18(private_entry_phi_owner_v20, &mut budget);
        let roots = fixture.roots();
        let retained = std::cell::Cell::new(0);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let chain = source.aggregate_output_v30(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let result = with_aggregate_initial_source_v30(
                source,
                chain.output(budget)?,
                budget,
                |original, optimized, budget| {
                    if foreign_query {
                        let mut work =
                            CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                        let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                        foreign.reserve_storage(budget.storage()).unwrap();
                        let before = (foreign.work(), foreign.storage());
                        assert!(matches!(
                            original.global_expression_entry_v23(optimized, &mut foreign),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!((foreign.work(), foreign.storage()), before);
                    } else {
                        budget.release_storage(1).unwrap();
                    }
                    retained.set(budget.storage());
                    Ok(())
                },
            );
            assert!(source.cleanup.is_denied());
            assert_eq!(budget.storage(), retained.get());
            drop(chain);
            result
        });
        assert!(matches!(
            result,
            Err(ProductionAggregateSourceErrorV30::Source(
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ))
        ));
        assert!(retained.get() > MODULE_FLOOR);
        assert_eq!(budget.storage(), retained.get());
    }
}

#[test]
fn aggregate_initial_callback_preflight_refusal_survives_panicking_capture_drop() {
    struct PanicDrop;
    impl Drop for PanicDrop {
        fn drop(&mut self) {
            panic!("unused initial-source capture destructor");
        }
    }
    let result =
        with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
            let chain = source.aggregate_output_v30(abi, budget)?;
            let output = chain.output(budget)?;
            let floor = budget.storage();
            let _ = source.retain_aggregate_result_v30::<()>(Err(
                ProductionSourceOwnedViewErrorV18::Binding("initial preflight first refusal")
                    .into(),
            ));
            let capture = PanicDrop;
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_aggregate_initial_source_v30(source, output, budget, move |_, _, _| {
                    drop(capture);
                    Ok(())
                })
            }));
            assert_eq!(budget.storage(), floor);
            assert!(!source.cleanup.is_denied());
            let result = caught.expect("capture destruction cannot replace first refusal");
            assert!(matches!(
                &result,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Binding("initial preflight first refusal")
                ))
            ));
            assert!(chain.discard(budget).is_err());
            result
        });
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding("initial preflight first refusal")
        ))
    ));
}
