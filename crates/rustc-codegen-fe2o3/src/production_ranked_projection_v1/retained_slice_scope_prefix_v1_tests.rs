//! Inert resource-only slice owner controls, not source/SSA or slice producer completion.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 17;
fn trial(count: usize, work_limit: usize, extra: usize) -> (bool, usize, usize, bool) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, FLOOR + extra);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut value = RetainedSliceScopePrefixV1::new();
    let result = value.prepare_into(count, &mut Prep::new(&mut budget, &mut owned));
    assert_eq!(budget.storage(), FLOOR + owned);
    assert_eq!(value.count, Some(count));
    let before = (budget.work(), budget.storage(), owned, value.retained);
    assert!(
        value
            .prepare_into(count, &mut Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    assert_eq!(
        before,
        (budget.work(), budget.storage(), owned, value.retained)
    );
    let outcome = (result.is_ok(), budget.work(), owned, value.entry.is_some());
    drop(value);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    outcome
}
#[test]
fn retained_pre_writer_slice_exact_original_storage_formula_and_work() {
    for count in [0, 3, 29] {
        let result = trial(count, LIMIT, LIMIT);
        assert!(result.0);
        assert_eq!(result.1, 4);
        assert_eq!(result.2, storage([count; 5]).unwrap());
    }
}
#[test]
fn retained_pre_writer_slice_work_refusal_precedes_floor_and_reservation() {
    for cut in 0..4 {
        let result = trial(3, cut, LIMIT);
        assert!(!result.0);
        assert_eq!(result.2, 0);
        assert!(!result.3);
    }
}
#[test]
fn retained_pre_writer_slice_every_storage_cut_retains_original_floor() {
    let required = storage([3; 5]).unwrap();
    for cut in 0..required {
        let result = trial(3, LIMIT, cut);
        assert!(!result.0);
        assert_eq!(result.1, 4);
        assert_eq!(result.2, 0);
        assert!(result.3);
    }
}
#[test]
fn retained_pre_writer_slice_size_overflow_after_work_before_floor() {
    let result = trial(usize::MAX, LIMIT, LIMIT);
    assert!(!result.0);
    assert_eq!(result.1, 4);
    assert!(!result.3);
}
#[test]
fn retained_pre_writer_slice_foreign_counter_and_lower_held_floor_refuse() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut value = RetainedSliceScopePrefixV1::new();
    value
        .prepare_into(3, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let mut foreign = owned;
    assert!(
        value
            .before_writers(3, &Prep::new(&mut budget, &mut foreign))
            .is_err()
    );
    assert!(
        value
            .before_writers(4, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    budget.release_storage(1).unwrap();
    assert!(
        value
            .before_writers(3, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    budget.reserve_storage(1).unwrap();
    value
        .before_writers(3, &Prep::new(&mut budget, &mut owned))
        .unwrap();
    let held = owned;
    owned -= 1;
    budget.reserve_storage(19).unwrap();
    assert!(
        value
            .before_writers(3, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    owned = held;
    budget.release_storage(19).unwrap();
    drop(value);
    budget.release_storage(owned).unwrap();
}
