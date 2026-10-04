#[test]
fn retained_extension_custody_extra_floor_undercut_keeps_base_paid_and_denies_all_refunds() {
    for selected_binding in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
        let foreign = closed_owner_v1760(ClosedCaseV1760::Noop);
        let roots = fixture.roots();
        let held = std::cell::Cell::new(0);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let handoff = source.checked_closed_scalar_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let base = budget.storage();
            budget.reserve_storage(256)?;
            let required = budget.storage();
            if selected_binding {
                assert!(matches!(
                    handoff.check_original_source(&foreign, budget),
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "foreign original SSA owner"
                    ))
                ));
            }
            budget.release_storage(1)?;
            assert!(
                budget.storage() >= base,
                "original handoff remains fully paid"
            );
            let error = handoff
                .observe_retained_storage_v18(required, budget)
                .unwrap_err();
            if selected_binding {
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
                ));
            } else {
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
            }
            budget.reserve_storage(1)?;
            assert!(
                handoff
                    .observe_retained_storage_v18(required, budget)
                    .is_err(),
                "restoring the byte cannot restore linked cleanup authority"
            );
            let before = budget.storage();
            assert!(handoff.discard(budget).is_err());
            assert_eq!(budget.storage(), before);
            held.set(before);
            Err::<(), _>(ProductionClosedScalarHandoffErrorV18::from(error))
        });
        if selected_binding {
            assert!(matches!(
                result,
                Err(ProductionClosedScalarHandoffErrorV18::Check(
                    ProductionClosedScalarCheckErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
                    )
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionClosedScalarHandoffErrorV18::Check(
                    ProductionClosedScalarCheckErrorV18::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                    )
                ))
            ));
        }
        assert_eq!(budget.storage(), held.get());
        assert!(held.get() > MODULE_FLOOR);
    }
}

#[test]
fn retained_extension_custody_selected_binding_keeps_valid_cleanup_separate_from_query() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
    let foreign = closed_owner_v1760(ClosedCaseV1760::Noop);
    let roots = fixture.roots();
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let handoff = source.checked_closed_scalar_output_v18(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        let retained = handoff.retained_storage(budget)?;
        let base = budget.storage();
        budget.reserve_storage(256)?;
        let backing = Box::new([7_u8; 256]);
        let required = budget.storage();
        let error = handoff.check_original_source(&foreign, budget).unwrap_err();
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
        ));
        handoff.observe_retained_storage_v18(required, budget)?;
        assert!(
            handoff.output(budget).is_err(),
            "cleanup is not query authority"
        );
        drop(backing);
        budget.release_storage(256)?;
        assert_eq!(budget.storage(), base);
        assert!(matches!(
            handoff.discard(budget),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "foreign original SSA owner"
            ))
        ));
        assert_eq!(budget.storage(), base - retained);
        Err::<(), _>(ProductionClosedScalarHandoffErrorV18::from(error))
    });
    assert!(matches!(
        result,
        Err(ProductionClosedScalarHandoffErrorV18::Check(
            ProductionClosedScalarCheckErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Binding("foreign original SSA owner")
            )
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn retained_extension_custody_foreign_ledger_and_same_ledger_wrong_slot_never_refund() {
    for move_ledger in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut other_work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
        let roots = fixture.roots();
        let held = std::cell::Cell::new(0);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let handoff = source.checked_closed_scalar_output_v18(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            budget.reserve_storage(256)?;
            let required = budget.storage();
            other.reserve_storage(required)?;
            if move_ledger {
                std::mem::swap(budget, &mut other);
            }
            let before = (other.work(), other.storage());
            let error = handoff
                .observe_retained_storage_v18(required, &other)
                .unwrap_err();
            assert_eq!((other.work(), other.storage()), before);
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            ));
            if move_ledger {
                std::mem::swap(budget, &mut other);
            }
            let before = budget.storage();
            assert!(handoff.discard(budget).is_err());
            assert_eq!(budget.storage(), before);
            held.set(before);
            Err::<(), _>(ProductionClosedScalarHandoffErrorV18::from(error))
        });
        assert!(matches!(
            result,
            Err(ProductionClosedScalarHandoffErrorV18::Check(
                ProductionClosedScalarCheckErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                )
            ))
        ));
        assert_eq!(budget.storage(), held.get());
    }
}

#[test]
fn retained_extension_custody_zero_requirement_cannot_weaken_original_handoff_floor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(500_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) = closed_prepared_v1760(ClosedCaseV1760::Noop, true, &mut budget);
    let roots = fixture.roots();
    let held = std::cell::Cell::new(0);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let handoff = source.checked_closed_scalar_output_v18(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        budget.release_storage(1)?;
        let error = handoff.observe_retained_storage_v18(0, budget).unwrap_err();
        assert!(matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ));
        held.set(budget.storage());
        assert!(handoff.discard(budget).is_err());
        Err::<(), _>(ProductionClosedScalarHandoffErrorV18::from(error))
    });
    assert!(matches!(
        result,
        Err(ProductionClosedScalarHandoffErrorV18::Check(
            ProductionClosedScalarCheckErrorV18::Source(
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
            )
        ))
    ));
    assert_eq!(budget.storage(), held.get());
}
