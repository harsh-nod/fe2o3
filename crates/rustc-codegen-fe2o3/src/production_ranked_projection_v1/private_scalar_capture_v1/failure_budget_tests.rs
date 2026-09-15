use super::*;

#[test]
fn failure_observation_work_preserves_exact_charge_boundary() {
    let mut budget = Budget::new(2);
    budget.charge(2).unwrap();
    assert_eq!(budget.failure(), None);
    assert!(!budget.work_exhausted);
    assert!(budget.charge(1).is_err());
    assert_eq!(budget.remaining, 0);
    assert!(budget.work_exhausted);
    assert_eq!(
        budget.failure(),
        Some(ResourceFailure::Work {
            remaining: 0,
            requested: 1
        })
    );
}

#[test]
fn failure_observation_storage_check_retains_attempt_before_drop() {
    let budget = Budget::new(123);
    let first = budget.reserve(MAX_STORAGE - 1).unwrap();
    let last = budget.reserve(1).unwrap();
    assert_eq!(budget.failure(), None);
    assert!(budget.reserve(1).is_err());
    let failure = Some(ResourceFailure::Storage {
        operation: StorageOperation::Check,
        retained: MAX_STORAGE,
        replaced: 0,
        requested: Some(1),
        attempted: Some(MAX_STORAGE + 1),
        limit: MAX_STORAGE,
    });
    assert_eq!(budget.failure(), failure);
    drop(first);
    drop(last);
    assert_eq!(budget.storage.get(), 0);
    assert_eq!(budget.failure(), failure);
    assert_eq!(budget.remaining, 123);
    assert!(!budget.work_exhausted);
}

#[test]
fn failure_observation_resize_is_distinct_and_keeps_old_reservation() {
    let budget = Budget::new(123);
    let mut retained = budget.reserve(1).unwrap();
    retained.resize(MAX_STORAGE).unwrap();
    assert!(retained.resize(MAX_STORAGE + 1).is_err());
    let failure = Some(ResourceFailure::Storage {
        operation: StorageOperation::Resize,
        retained: MAX_STORAGE,
        replaced: MAX_STORAGE,
        requested: Some(MAX_STORAGE + 1),
        attempted: Some(MAX_STORAGE + 1),
        limit: MAX_STORAGE,
    });
    assert_eq!(budget.failure(), failure);
    assert_eq!(retained.nodes, MAX_STORAGE);
    retained.resize(2).unwrap();
    assert_eq!(budget.failure(), failure);
    drop(retained);
    assert_eq!(budget.storage.get(), 0);
}

#[test]
fn failure_observation_storage_check_overflow_is_not_wrapped_to_a_small_total() {
    let budget = Budget::new(123);
    let _retained = budget.reserve(1).unwrap();
    assert!(budget.check_storage(&[usize::MAX]).is_err());
    assert_eq!(
        budget.failure(),
        Some(ResourceFailure::Storage {
            operation: StorageOperation::Check,
            retained: 1,
            replaced: 0,
            requested: None,
            attempted: None,
            limit: MAX_STORAGE,
        })
    );
    assert_eq!(budget.storage.get(), 1);
}

#[test]
fn failure_observation_storage_resize_overflow_preserves_both_reservations() {
    let budget = Budget::new(123);
    let _other = budget.reserve(1).unwrap();
    let mut retained = budget.reserve(1).unwrap();
    assert!(retained.resize(usize::MAX).is_err());
    assert_eq!(
        budget.failure(),
        Some(ResourceFailure::Storage {
            operation: StorageOperation::Resize,
            retained: 2,
            replaced: 1,
            requested: Some(usize::MAX),
            attempted: None,
            limit: MAX_STORAGE,
        })
    );
    assert_eq!(budget.storage.get(), 2);
    assert_eq!(retained.nodes, 1);
}

#[test]
fn failure_observation_first_failure_is_not_replaced_by_a_later_category() {
    let mut budget = Budget::new(0);
    assert!(budget.check_storage(&[MAX_STORAGE, 1]).is_err());
    let first = budget.failure();
    assert!(budget.charge(1).is_err());
    assert!(budget.work_exhausted);
    assert_eq!(budget.failure(), first);
    let independent = Budget::new(0);
    assert_eq!(independent.failure(), None);
}

#[test]
fn failure_observation_scratch_parts_use_existing_combined_ceiling() {
    let budget = Budget::new(123);
    let retained = budget.reserve(MAX_STORAGE - 7).unwrap();
    budget.check_storage(&[3, 4]).unwrap();
    assert!(budget.check_storage(&[3, 5]).is_err());
    assert_eq!(
        budget.failure(),
        Some(ResourceFailure::Storage {
            operation: StorageOperation::Check,
            retained: MAX_STORAGE - 7,
            replaced: 0,
            requested: Some(8),
            attempted: Some(MAX_STORAGE + 1),
            limit: MAX_STORAGE,
        })
    );
    assert_eq!(budget.storage.get(), MAX_STORAGE - 7);
    drop(retained);
}
