#[test]
fn scoped_aggregate_memory_visits_the_actual_rewritten_chain_without_legacy_completion() {
    with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
        let chain = source.aggregate_output_v30(abi, budget)?;
        let floor = budget.storage();
        let mut called = false;
        chain.with_private_memory_v31::<_, ProductionAggregateSourceErrorV30, _>(
            budget,
            |view, budget| {
                called = true;
                assert!(std::ptr::eq(view.chain(budget)?, &chain));
                let output = chain.output(budget)?;
                view.check_final_owner(output.owner(), budget)?;
                let memory = view.memory(budget)?;
                assert!(memory.promotion_count() > 0);
                assert_eq!(memory.endpoint_count(), output.rounds().len() * 2 + 1);
                assert!(memory.endpoint_belongs_to(0, source.canonical(budget)?));
                for (round, row) in output.rounds().iter().enumerate() {
                    assert!(memory.endpoint_belongs_to(round * 2 + 1, row.scalar().owner()));
                    assert!(memory.endpoint_belongs_to(round * 2 + 2, row.aggregate().output()));
                }
                assert!(!view.runtime_requirements_are_discharged());
                assert!(!view.executed_memory_refinement_is_complete());
                assert!(!view.grants_artifact_or_launch_authority());
                Ok(())
            },
        )?;
        assert!(called);
        assert_eq!(budget.storage(), floor);
        chain.discard(budget)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn scoped_aggregate_memory_refuses_foreign_final_owner_in_the_real_callback() {
    let mut called = false;
    let result =
        with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
            let chain = source.aggregate_output_v30(abi, budget)?;
            let floor = budget.storage();
            let result = chain.with_private_memory_v31::<(), ProductionAggregateSourceErrorV30, _>(
                budget,
                |view, budget| {
                    called = true;
                    let original = source.canonical(budget)?;
                    view.check_final_owner(original, budget)?;
                    panic!("original graph was relabeled as the aggregate final owner");
                },
            );
            assert_eq!(budget.storage(), floor);
            let discarded = chain.discard(budget);
            result?;
            discarded?;
            Ok(())
        });
    assert!(called);
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding(
                "scoped aggregate memory final owner differs"
            )
        ))
    ));
}

#[test]
fn scoped_aggregate_memory_refuses_missing_duplicate_and_stale_stages_before_callback() {
    for fault in 0..5 {
        let mut called = false;
        let result =
            with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
                let chain = source.aggregate_output_v30(abi, budget)?;
                let floor = budget.storage();
                let result = chain
                    .with_private_memory_inner_v31::<(), ProductionAggregateSourceErrorV30, _>(
                        budget,
                        |_, _| {
                            called = true;
                            Ok(())
                        },
                        Some(fault),
                    );
                assert_eq!(budget.storage(), floor);
                let discarded = chain.discard(budget);
                result?;
                discarded?;
                Ok(())
            });
        assert!(!called, "fault {fault} reached the memory callback");
        assert!(
            matches!(
                result,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(_)
                ))
            ),
            "fault {fault}: {result:?}"
        );
    }
}

#[test]
fn scoped_aggregate_memory_ordinary_callback_refusal_settles_only_its_scoped_credit() {
    let mut called = false;
    let result =
        with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
            let chain = source.aggregate_output_v30(abi, budget)?;
            let floor = budget.storage();
            let result = chain.with_private_memory_v31::<(), ProductionAggregateSourceErrorV30, _>(
                budget,
                |view, budget| {
                    called = true;
                    assert!(view.memory(budget)?.promotion_count() > 0);
                    Err::<(), _>(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "scoped memory callback refusal",
                        )
                        .into(),
                    )
                },
            );
            assert_eq!(budget.storage(), floor);
            let discarded = chain.discard(budget);
            result?;
            discarded?;
            Ok(())
        });
    assert!(called);
    assert!(matches!(
        result,
        Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding("scoped memory callback refusal")
        ))
    ));
}

#[derive(Debug)]
enum MemoryVisitErrorV31 {
    Compiler(ProductionAggregateSourceErrorV30),
    Payload(Vec<u8>),
}

impl From<ProductionAggregateSourceErrorV30> for MemoryVisitErrorV31 {
    fn from(error: ProductionAggregateSourceErrorV30) -> Self {
        Self::Compiler(error)
    }
}

impl From<ProductionSourceOwnedViewErrorV18> for MemoryVisitErrorV31 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Compiler(error.into())
    }
}

fn memory_visit_payload_v31(budget: &mut ArgumentBudgetV1<'_>) -> Vec<u8> {
    budget.reserve_storage(37).unwrap();
    budget.charge_work(37).unwrap();
    let mut payload = Vec::new();
    payload.try_reserve_exact(37).unwrap();
    budget.reserve_storage(payload.capacity() - 37).unwrap();
    payload.resize(37, 0x31);
    payload
}

#[test]
fn scoped_aggregate_memory_preserves_paid_success_error_and_panic_payloads() {
    for phase in 0..3 {
        with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
            let chain = source.aggregate_output_v30(abi, budget)?;
            let floor = budget.storage();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                chain.with_private_memory_v31::<Vec<u8>, MemoryVisitErrorV31, _>(
                    budget,
                    |view, budget| {
                        assert!(view.memory(budget)?.promotion_count() > 0);
                        let payload = memory_visit_payload_v31(budget);
                        match phase {
                            0 => Ok(payload),
                            1 => Err(MemoryVisitErrorV31::Payload(payload)),
                            _ => std::panic::panic_any(payload),
                        }
                    },
                )
            }));
            let payload = match (phase, result) {
                (0, Ok(Ok(payload))) | (1, Ok(Err(MemoryVisitErrorV31::Payload(payload)))) => {
                    payload
                }
                (2, Err(payload)) => *payload.downcast::<Vec<u8>>().unwrap(),
                (_, other) => panic!("wrong paid payload disposition: {other:?}"),
            };
            assert_eq!(payload, vec![0x31; 37]);
            assert_eq!(budget.storage(), floor + payload.capacity());
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
fn scoped_aggregate_memory_lost_floor_or_foreign_ledger_cannot_refund_any_ancestor() {
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
            let result = chain.with_private_memory_v31::<(), ProductionAggregateSourceErrorV30, _>(
                budget,
                |view, budget| {
                    if foreign_query {
                        let mut foreign_work =
                            CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                        foreign.reserve_storage(budget.storage()).unwrap();
                        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                        assert!(matches!(
                            view.memory(&mut foreign),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!(
                            (foreign.work(), foreign.storage(), foreign.peak_storage()),
                            before
                        );
                    } else {
                        budget.release_storage(1).unwrap();
                    }
                    retained.set(budget.storage());
                    Ok(())
                },
            );
            assert!(matches!(
                result,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ))
            ));
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
        assert!(budget.storage() >= retained.get());
    }
}
