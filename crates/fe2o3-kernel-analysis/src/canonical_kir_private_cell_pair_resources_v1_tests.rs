use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

#[test]
fn owned_error_panic_extra_and_undercut_preserve_exact_accounting() {
    for mode in 0..4 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut live = 0;
        let result: Result<()> = scoped(&mut budget, |meter| {
            let (mut rows, _) = meter.table::<u64>(16)?;
            meter.push(&mut rows, 1)?;
            live = meter.budget_for_test().storage() - FLOOR;
            match mode {
                0 => Err(Error::Mismatch("test refusal")),
                1 => panic_any("test unwind"),
                2 => {
                    meter.budget_for_test().reserve_storage(11)?;
                    Ok(())
                }
                _ => {
                    meter.budget_for_test().release_storage(FLOOR + 1)?;
                    Ok(())
                }
            }
        });
        assert!(result.is_err());
        assert_eq!(
            budget.storage(),
            match mode {
                0 | 1 => FLOOR,
                2 => FLOOR + 11,
                _ => live - 1,
            }
        );
        assert!(budget.work() > 0);
    }
}

#[test]
fn foreign_equal_storage_ledger_is_never_charged_or_credited() {
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    {
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut replacement_floor = 0;
        let result: Result<()> = scoped(&mut budget, |meter| {
            let (_rows, _) = meter.table::<u64>(16)?;
            replacement_floor = meter.budget_for_test().storage();
            let mut foreign = Budget::new(&mut foreign_work, STORAGE);
            foreign.reserve_storage(replacement_floor)?;
            *meter.budget_for_test() = foreign;
            meter.work(1)
        });
        assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
        assert_eq!(budget.storage(), replacement_floor);
        assert_eq!(budget.work(), 0);
    }
    assert!(work.work() > 0);
    assert_eq!(foreign_work.work(), 0);
}

#[test]
fn nested_derive_panic_adopts_only_unwound_same_ledger_scratch_and_stops() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: Result<()> = scoped(&mut budget, |meter| {
        let nested: Result<()> = meter.derive(|budget| {
            budget.charge_work(13)?;
            budget.reserve_storage(97)?;
            panic_any("nested inventory setup");
        });
        assert_eq!(nested, Err(Error::Panicked));
        assert_eq!(meter.work(1), Err(Error::Panicked));
        assert_eq!(meter.derive(|_| Ok(())), Err(Error::Panicked));
        nested
    });
    assert_eq!(result, Err(Error::Panicked));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), 13);
}

#[test]
fn hostile_panic_payload_destructors_run_after_cleanup_in_both_scopes() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic_any("payload destructor");
        }
    }
    for nested in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = scoped(&mut budget, |meter| {
                let (_rows, _) = meter.table::<u64>(16)?;
                if nested {
                    meter.derive(|budget| {
                        budget.reserve_storage(91)?;
                        panic_any(Bomb);
                    })
                } else {
                    panic_any(Bomb);
                }
            });
        }));
        assert!(result.is_err());
        assert_eq!(budget.storage(), FLOOR);
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: Result<Bomb> = scoped(&mut budget, |meter| {
        let (_rows, _) = meter.table::<u64>(16)?;
        meter.budget_for_test().reserve_storage(11)?;
        Ok(Bomb)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), FLOOR + 11);
}

#[test]
fn capacity_arithmetic_and_excess_are_prepaid_before_initialization() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = scoped(&mut budget, |meter| meter.table::<u64>(usize::MAX));
    assert!(matches!(result, Err(Error::Resource(Resource::Arithmetic))));
    assert_eq!(budget.storage(), FLOOR);
    // These exercise the actual capacity helper, not an allocator-overallocation claim.
    let result = scoped(&mut budget, |meter| {
        meter.reserve(3 * size_of::<u64>())?;
        meter.capacity::<u64>(3, 5)
    });
    assert_eq!(result, Ok(40));
    assert_eq!(budget.storage(), FLOOR);
    let peak = budget.peak_storage();
    for (limit, ok) in [(peak, true), (peak - 1, false)] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = scoped(&mut budget, |meter| {
            meter.reserve(24)?;
            meter.capacity::<u64>(3, 5)
        });
        assert_eq!(result.is_ok(), ok);
        assert_eq!(budget.storage(), FLOOR);
    }
    let result = scoped(&mut budget, |meter| {
        meter.reserve(24)?;
        meter.capacity::<u64>(3, 2)
    });
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), FLOOR);
}
