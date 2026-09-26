use super::*;

#[test]
fn unauthenticated_omission_duplicate_and_foreign_ranges_never_reach_consumer() {
    use transport_impl::TestFault::*;
    for fault in [
        OmittedOrigin,
        OriginRange,
        OmittedPiece,
        PieceComponent,
        PieceStage,
        DuplicateOperation,
        OmittedUse,
        UseDefinition,
        OmittedPayload,
        PayloadOrdinal,
        OmittedAttachment,
        OmittedSourceAlias,
        SourceInstance,
        PendingOmission,
        ReadAssociation,
        MissingAlias,
        DuplicatedAlias,
        ForeignAliasRange,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let mut owner = tile_owner(SourceCase::Slots, Order::Blocked, &mut budget);
        let floor = budget.storage();
        transport_impl::inject_fault(&mut owner, fault);
        let result: Result<(), Error> = owner.with_checked_transport_v29(&mut budget, |_, _| {
            panic!("hostile row reached checked consumer: {fault:?}");
        });
        assert!(
            matches!(result, Err(Error::Binding(_))),
            "{fault:?}: {result:?}"
        );
        assert_eq!(budget.storage(), floor);
        drop_owner(owner, &mut budget);
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn separately_admitted_same_byte_owner_is_not_an_inventory_subject() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let first = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let second = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let first_identity = first
        .with_checked_transport_v29(&mut budget, |view, budget| view.current_identity(budget))
        .unwrap();
    let second_identity = second
        .with_checked_transport_v29(&mut budget, |view, budget| view.current_identity(budget))
        .unwrap();
    assert_eq!(first_identity, second_identity);
    let floor = budget.storage();
    assert_eq!(
        transport_impl::foreign_inventory(&first, &second, &mut budget),
        Err(Error::Binding("foreign current owner"))
    );
    assert_eq!(budget.storage(), floor);
    drop_owner(second, &mut budget);
    drop_owner(first, &mut budget);
}
#[test]
fn missing_and_foreign_donors_refuse_before_transfer() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    assert!(matches!(
        Owner::try_from_pending_with_budget_v29(&mut None, Order::Blocked, &mut budget),
        Err(Error::MissingDonor)
    ));
    let (pending, _) = pending_source(SourceCase::Repeated, false, None, &mut budget);
    let identity = *pending.pending_identity();
    let retained = pending.adopted_storage();
    let mut donor = Some(pending);
    let mut other_work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut other = ArgumentBudgetV1::new(&mut other_work, SCHEDULE_LIMIT);
    other.reserve_storage(retained).unwrap();
    assert!(matches!(
        Owner::try_from_pending_with_budget_v29(&mut donor, Order::Blocked, &mut other),
        Err(Error::Resource(ArgumentResourceV1::Accounting))
    ));
    assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
    assert_eq!(other.storage(), retained);
    let owner =
        Owner::try_from_pending_with_budget_v29(&mut donor, Order::Blocked, &mut budget).unwrap();
    check_complete(&owner, &mut budget);
    drop_owner(owner, &mut budget);
}
#[test]
fn query_ordinal_error_is_sticky_even_when_ignored_by_callback() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Striped, &mut budget);
    let floor = budget.storage();
    let error = owner
        .with_checked_transport_v29(&mut budget, |view, budget| {
            let error = view.piece_alias(usize::MAX, budget).unwrap_err();
            assert_eq!(view.operation_count(budget).unwrap_err(), error);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error, Error::Binding("piece alias ordinal"));
    assert_eq!(budget.storage(), floor);
    drop_owner(owner, &mut budget);
}
#[test]
fn foreign_and_moved_query_ledgers_are_sticky_without_foreign_credit() {
    for moved in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, SCHEDULE_LIMIT);
        foreign.reserve_storage(91).unwrap();
        let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
        let floor = budget.storage();
        let result = owner.with_checked_transport_v29(&mut budget, |view, budget| {
            if moved {
                std::mem::swap(budget, &mut foreign);
                assert!(matches!(
                    view.root_count(&mut foreign),
                    Err(Error::Resource(ArgumentResourceV1::Accounting))
                ));
                std::mem::swap(budget, &mut foreign);
            } else {
                assert!(matches!(
                    view.root_count(&mut foreign),
                    Err(Error::Resource(ArgumentResourceV1::Accounting))
                ));
            }
            Ok(())
        });
        assert_eq!(result, Err(Error::Resource(ArgumentResourceV1::Accounting)));
        assert_eq!(foreign.storage(), 91);
        assert_eq!(budget.storage(), floor);
        drop_owner(owner, &mut budget);
    }
}
#[test]
fn restored_floor_cannot_erase_an_observed_undercut() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let floor = budget.storage();
    let result = owner.with_checked_transport_v29(&mut budget, |view, budget| {
        budget.release_storage(1)?;
        assert_eq!(
            view.origin_count(budget),
            Err(Error::Resource(ArgumentResourceV1::Accounting))
        );
        budget.reserve_storage(1)?;
        Ok(())
    });
    assert_eq!(result, Err(Error::Resource(ArgumentResourceV1::Accounting)));
    assert_eq!(budget.storage(), floor);
    drop_owner(owner, &mut budget);
}
#[test]
fn storage_denial_restores_exact_donor_and_allows_same_ledger_recovery() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let (pending, _) = pending_source(SourceCase::Repeated, false, None, &mut budget);
    let identity = *pending.pending_identity();
    let before = budget.storage();
    let pressure = budget.storage_limit() - before;
    budget.reserve_storage(pressure).unwrap();
    let mut donor = Some(pending);
    assert!(matches!(
        Owner::try_from_pending_with_budget_v29(&mut donor, Order::Blocked, &mut budget),
        Err(Error::Resource(ArgumentResourceV1::Storage { .. }))
    ));
    assert_eq!(donor.as_ref().unwrap().pending_identity(), &identity);
    assert_eq!(budget.storage(), before + pressure);
    let denial = budget.failed_storage().expect("real denied storage debit");
    budget.release_storage(pressure).unwrap();
    let owner =
        Owner::try_from_pending_with_budget_v29(&mut donor, Order::Blocked, &mut budget).unwrap();
    assert_eq!(budget.failed_storage(), Some(denial));
    check_complete(&owner, &mut budget);
    drop_owner(owner, &mut budget);
    assert_eq!(budget.storage(), 0);
}
