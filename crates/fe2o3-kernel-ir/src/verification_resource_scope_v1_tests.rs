use super::*;
use CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use CanonicalKernelIrWorkBudgetV1 as Work;

const ENTRY: usize = Budget::BOUNDED_SCRATCH_WORK_V1;
const FRAME: usize = Budget::BOUNDED_SCRATCH_STORAGE_V1;

fn exercise(budget: &mut Budget<'_>, work: usize, storage: usize) -> Result<(), Resource> {
    let ledger = budget.work_ledger_identity_v1();
    budget.with_bounded_scratch_v1(work, storage, |scope| {
        assert!(scope.work_ledger_identity_v1() == ledger);
        scope.charge_work(5)?;
        scope.reserve_storage(7)?;
        scope.release_storage(7)?;
        Ok(())
    })
}

#[test]
fn exact_and_one_short_limits_preserve_original_prefix_and_floor() {
    for (work, storage, success) in [
        (ENTRY + 5, FRAME + 7, true),
        (ENTRY + 4, FRAME + 7, false),
        (ENTRY + 5, FRAME + 6, false),
        (ENTRY - 1, FRAME + 7, false),
        (ENTRY + 5, FRAME - 1, false),
    ] {
        let mut original = Work::new(1000);
        let mut budget = Budget::new(&mut original, 10000);
        budget.charge_work(19).unwrap();
        budget.reserve_storage(31).unwrap();
        assert_eq!(exercise(&mut budget, work, storage).is_ok(), success);
        assert_eq!(budget.storage(), 31);
        assert_eq!(budget.work_limit_v1(), 1000);
        if success {
            assert_eq!(budget.work(), 19 + ENTRY + 5);
            assert_eq!(budget.peak_storage(), 31 + FRAME + 7);
        }
        budget.charge_work(100).unwrap();
        budget.reserve_storage(100).unwrap();
    }
}

#[test]
fn owned_account_and_historical_denials_are_preserved() {
    let mut owned = CanonicalKernelIrOwnedVerificationResourceBudgetV1::new(Work::new(1000), 10000);
    owned.with_budget(|budget| {
        budget.charge_work(13).unwrap();
        budget.reserve_storage(23).unwrap();
        assert!(budget.charge_work(1000).is_err());
        assert!(budget.reserve_storage(10000).is_err());
        budget.reserve_storage(5000).unwrap();
        budget.release_storage(5000).unwrap();
        let account = budget.storage_account_identity_v1();
        budget
            .with_bounded_scratch_v1::<_, Resource>(ENTRY + 5, FRAME + 7, |scope| {
                assert_eq!(scope.storage_account_identity_v1(), account);
                scope.charge_work(5)?;
                scope.reserve_storage(7)?;
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.work(), 13 + ENTRY + 5);
        assert_eq!(budget.storage(), 23);
        assert_eq!(budget.peak_storage(), 5023);
        assert_eq!(budget.failed_work(), Some(1013));
        assert_eq!(budget.failed_storage(), Some(10023));
    });
    assert_eq!(owned.work_limit(), 1000);
    assert_eq!(owned.storage(), 23);
}

#[test]
fn scratch_scope_cannot_widen_an_outer_work_or_storage_limit() {
    let mut original = Work::new(ENTRY + 4);
    let mut budget = Budget::new(&mut original, FRAME + 6);
    assert!(matches!(exercise(&mut budget, ENTRY + 5, FRAME + 7),
        Err(Resource::Work(error)) if error.limit() == ENTRY + 4));
    assert_eq!(budget.storage(), 0);
    let mut original = Work::new(1000);
    let mut budget = Budget::new(&mut original, FRAME + 6);
    assert!(matches!(exercise(&mut budget, ENTRY + 5, FRAME + 7),
        Err(Resource::Storage(error)) if error.limit() == FRAME + 6));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn nested_queries_preserve_parent_frame_and_effective_limits() {
    let mut original = Work::new(1000);
    let mut budget = Budget::new(&mut original, 10000);
    budget.reserve_storage(31).unwrap();
    budget
        .with_bounded_scratch_v1::<_, Resource>(2 * ENTRY + 5, 2 * FRAME + 7, |outer| {
            let floor = outer.storage();
            assert!(outer.release_storage(1).is_err());
            exercise(outer, ENTRY + 5, FRAME + 7)?;
            assert_eq!(outer.storage(), floor);
            assert!(outer.charge_work(1).is_err());
            assert!(matches!(
                outer.with_bounded_scratch_v1::<_, Resource>(1000, 10000, |_| panic!(
                    "unpaid nested callback"
                )),
                Err(Resource::Work(_))
            ));
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.work(), 2 * ENTRY + 5);
    assert_eq!(budget.storage(), 31);
    assert_eq!(budget.peak_storage(), 31 + 2 * FRAME + 7);
    budget.charge_work(1).unwrap();
}

#[test]
fn nested_owned_storage_window_cannot_be_widened() {
    let mut owned = CanonicalKernelIrOwnedVerificationResourceBudgetV1::new(Work::new(1000), 10000);
    owned.with_budget(|budget| {
        budget.reserve_storage(31).unwrap();
        budget
            .with_additional_storage_window_v1::<_, Resource>(FRAME + 6, |outer| {
                assert!(matches!(exercise(outer, ENTRY + 5, FRAME + 7),
                Err(Resource::Storage(error)) if error.limit() == 31 + FRAME + 6));
                assert_eq!(outer.storage(), 31);
                Ok(())
            })
            .unwrap();
        exercise(budget, ENTRY + 5, FRAME + 7).unwrap();
        assert_eq!(budget.storage(), 31);
    });
}

#[test]
fn callback_error_and_unwind_restore_original_limits_and_scratch() {
    for panic in [false, true] {
        let mut original = Work::new(1000);
        let mut budget = Budget::new(&mut original, 10000);
        budget.reserve_storage(31).unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            budget.with_bounded_scratch_v1::<(), Resource>(ENTRY + 5, FRAME + 7, |scope| {
                scope.charge_work(5)?;
                scope.reserve_storage(7)?;
                if panic {
                    std::panic::panic_any(17_u32);
                }
                Err(Resource::Accounting)
            })
        }));
        if panic {
            assert_eq!(*outcome.unwrap_err().downcast::<u32>().unwrap(), 17);
        } else {
            assert_eq!(outcome.unwrap(), Err(Resource::Accounting));
        }
        assert_eq!(budget.storage(), 31);
        assert_eq!(budget.work(), ENTRY + 5);
        assert_eq!(budget.peak_storage(), 31 + FRAME + 7);
        assert_eq!(budget.work_limit_v1(), 1000);
        budget.charge_work(100).unwrap();
        exercise(&mut budget, ENTRY + 5, FRAME + 7).unwrap();
    }
}

#[test]
fn replacement_refuses_and_cleans_only_the_original_account() {
    let mut original = Work::new(1000);
    let mut budget = Budget::new(&mut original, 10000);
    // As in owned-ledger replacement tests, the replacement must outlive every
    // possible callback borrow. Production scopes allocate no accounting state.
    let mut foreign = Budget::new(Box::leak(Box::new(Work::new(2000))), 10000);
    foreign.charge_work(41).unwrap();
    foreign.reserve_storage(53).unwrap();
    budget.reserve_storage(31).unwrap();
    let result = budget.with_bounded_scratch_v1::<_, Resource>(ENTRY + 5, FRAME + 7, |scope| {
        scope.charge_work(5)?;
        scope.reserve_storage(7)?;
        *scope = foreign;
        assert_eq!(scope.storage(), 53);
        assert_eq!(scope.work(), 41);
        Ok(())
    });
    assert_eq!(result, Err(Resource::Accounting));
    assert_eq!(budget.storage(), 31);
    assert_eq!(budget.work(), ENTRY + 5);
    assert_eq!(budget.work_limit_v1(), 1000);
}

#[test]
fn arithmetic_refusal_never_enters_or_changes_the_account() {
    let mut original = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut original, usize::MAX);
    budget.charge_work(1).unwrap();
    budget.reserve_storage(1).unwrap();
    for (work, storage) in [(usize::MAX, 1), (1, usize::MAX)] {
        assert_eq!(
            budget.with_bounded_scratch_v1::<(), Resource>(work, storage, |_| panic!(
                "overflow callback"
            )),
            Err(Resource::Arithmetic)
        );
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.storage(), 1);
        assert_eq!(budget.work_limit_v1(), usize::MAX);
    }
}
