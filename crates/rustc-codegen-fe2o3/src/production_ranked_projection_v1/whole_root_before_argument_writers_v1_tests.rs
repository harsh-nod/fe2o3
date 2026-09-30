//! Inert destination-owner controls; no admitted source or canonical proof.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 23;
fn trial(work_limit: usize, extra: usize) -> (bool, usize, usize, usize, usize, bool) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, FLOOR + extra);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut value = RetainedBeforeArgumentWritersV1::new();
    let result = value.prepare_into(3, &mut Prep::new(&mut budget, &mut owned));
    let outcome = (
        result.is_ok(),
        budget.work(),
        owned,
        value.runtime_index_arguments.len(),
        value.runtime_slice_extent_arguments.len(),
        value.initialized,
    );
    assert_eq!(budget.storage(), FLOOR + owned);
    assert_eq!(value.borrowed_locals.capacity(), 0);
    assert_eq!(
        value.next_runtime_argument,
        if value.initialized { 1 } else { 0 }
    );
    let before = (
        budget.work(),
        budget.storage(),
        owned,
        value.runtime_index_arguments.capacity(),
        value.runtime_slice_extent_arguments.capacity(),
    );
    assert!(
        value
            .prepare_into(3, &mut Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    assert_eq!(
        before,
        (
            budget.work(),
            budget.storage(),
            owned,
            value.runtime_index_arguments.capacity(),
            value.runtime_slice_extent_arguments.capacity()
        )
    );
    drop(value);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    outcome
}
#[test]
fn retained_pre_writer_argument_original_initial_values_and_order() {
    let good = trial(LIMIT, LIMIT);
    assert!(good.0 && good.5);
    assert_eq!((good.1, good.3, good.4), (12, 3, 3));
}
#[test]
fn retained_pre_writer_argument_every_work_cut_retains_partial_rows() {
    let good = trial(LIMIT, LIMIT);
    let mut first = false;
    for cut in 0..good.1 {
        let value = trial(cut, LIMIT);
        assert!(!value.0);
        first |= value.3 == 3 && value.4 == 0;
    }
    assert!(first);
}
#[test]
fn retained_pre_writer_argument_every_storage_cut_retains_partial_rows() {
    let good = trial(LIMIT, LIMIT);
    let mut first = false;
    for cut in 0..good.2 {
        let value = trial(LIMIT, cut);
        assert!(!value.0);
        first |= value.3 == 3 && value.4 == 0;
    }
    assert!(first);
}
#[test]
fn retained_pre_writer_argument_empty_still_sets_next_one_after_both_rows() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut value = RetainedBeforeArgumentWritersV1::new();
    value
        .prepare_into(0, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    assert_eq!(value.next_runtime_argument, 1);
    assert!(value.initialized);
    assert_eq!(value.runtime_index_arguments.capacity(), 0);
    assert_eq!(value.runtime_slice_extent_arguments.capacity(), 0);
    assert_eq!(owned, 0);
}
#[test]
fn retained_pre_writer_argument_unwind_keeps_completed_outer_destinations() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut value = RetainedBeforeArgumentWritersV1::new();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        value
            .prepare_into(3, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        std::panic::panic_any(());
    }));
    assert!(caught.is_err());
    assert_eq!(value.runtime_index_arguments.len(), 3);
    assert_eq!(value.runtime_slice_extent_arguments.len(), 3);
    assert_eq!(budget.storage(), owned);
    drop(value);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
