use super::*;

#[test]
fn invalid_read_or_alias_query_cannot_be_ignored_by_real_public_consumer() {
    for alias in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let owner = owner(SourceCase::Repeated, Order::Blocked, &mut budget);
        let floor = budget.storage();
        let error = consume(&owner, &mut budget, |checked, budget| {
            assert!(checked.read_count(budget)? > 0);
            if alias {
                assert!(checked.alias(usize::MAX, budget).is_err());
            } else {
                assert!(checked.read(usize::MAX, budget).is_err());
            }
            Ok(())
        })
        .unwrap_err();
        assert!(
            matches!(error, ReadError::Binding { obligation: None, reason } if reason == if alias { "alias ordinal" } else { "read ordinal" })
        );
        assert_eq!(budget.storage(), floor);
        release(owner, &mut budget);
    }
}

#[test]
fn paid_query_rejects_foreign_budget_without_touching_it() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let owner = owner(SourceCase::Repeated, Order::Striped, &mut budget);
    let floor = budget.storage();
    let error = consume(&owner, &mut budget, |checked, _| {
        let mut other_work = Work::new(LIMIT);
        let mut other = Budget::new(&mut other_work, LIMIT);
        other.reserve_storage(FLOOR)?;
        assert_eq!(
            checked.read_count(&mut other).unwrap_err(),
            ReadError::Resource(Resource::Accounting)
        );
        assert_eq!(other.storage(), FLOOR);
        assert_eq!(other.work(), 0);
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error, ReadError::Resource(Resource::Accounting));
    assert_eq!(budget.storage(), floor);
    release(owner, &mut budget);
}

#[test]
fn complete_source_alias_reader_rejects_untrusted_missing_duplicate_and_wrong_associations() {
    for fault in 0..4 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let owner = owner(SourceCase::Repeated, Order::Blocked, &mut budget);
        owner
            .with_checked_transport_v29(&mut budget, |transport, budget| {
                let result =
                    crate::tile_global_reads_v29::test_alias_fault_v29(transport, budget, fault);
                assert!(
                    matches!(result, Err(ReadError::Binding { .. })),
                    "fault={fault}: {result:?}"
                );
                Ok(())
            })
            .unwrap();
        release(owner, &mut budget);
        assert_eq!(budget.storage(), 0);
    }
}
