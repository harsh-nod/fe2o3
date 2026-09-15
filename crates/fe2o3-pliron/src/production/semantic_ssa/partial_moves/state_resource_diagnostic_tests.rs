use super::*;

#[test]
fn failed_tree_allocation_records_live_before_temporaries_unwind() {
    let path = vec![PathElement::Field(0)];
    let temporary =
        PATH_SET_WORDS + path_words(&path).unwrap() + PARTIAL_WORDS + PARTIAL_ENTRY_WORDS;
    let base = 17;
    let budget = Budget::new(base, 0, base + temporary, 100_000).unwrap();
    let mut state = State::default();
    let error = state.mark(0, path, &budget).unwrap_err();
    assert_eq!(
        error,
        Error::Limit {
            resource: Resource::Storage,
            required: base + temporary + NODE_WORDS,
            limit: base + temporary,
            storage: Some(StorageFailure {
                live: temporary,
                peak: temporary,
                requested: NODE_WORDS,
            }),
        }
    );
    assert_eq!(budget.0.live.get(), 0);
    assert_eq!(budget.peak(), temporary);
    assert!(state.root.is_none());
}

#[test]
fn rejected_reservation_preserves_prior_peak_and_does_not_charge_work() {
    let budget = Budget::new(7, 3, 27, 100).unwrap();
    drop(budget.reserve(14).unwrap());
    let held = budget.reserve(6).unwrap();
    let work = budget.work_units();
    assert!(matches!(
        budget.reserve(15),
        Err(Error::Limit {
            resource: Resource::Storage,
            required: 28,
            limit: 27,
            storage: Some(StorageFailure {
                live: 6,
                peak: 14,
                requested: 15
            }),
        })
    ));
    assert_eq!(budget.0.live.get(), 6);
    assert_eq!(budget.peak(), 14);
    assert_eq!(budget.work_units(), work);
    drop(held);
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn base_rejection_records_zero_state_and_no_new_allocation() {
    assert!(matches!(
        Budget::new(18, 0, 17, 100),
        Err(Error::Limit {
            resource: Resource::Storage,
            required: 18,
            limit: 17,
            storage: Some(StorageFailure {
                live: 0,
                peak: 0,
                requested: 0
            }),
        })
    ));
}

#[test]
fn work_and_overflow_errors_are_not_storage_observations() {
    let budget = Budget::new(0, 3, 100, 4).unwrap();
    assert_eq!(
        budget.work(2),
        Err(Error::Limit {
            resource: Resource::Work,
            required: 5,
            limit: 4,
            storage: None,
        })
    );
    let budget = Budget::new(usize::MAX, 0, usize::MAX, 100).unwrap();
    assert!(matches!(budget.reserve(1), Err(Error::Overflow)));
    assert_eq!(budget.0.live.get(), 0);
    assert_eq!(budget.peak(), 0);
}
