use super::*;

#[derive(Debug, PartialEq)]
enum Error {
    Resource(Resource),
    Panicked,
    Refused,
}
fn owned<'work, T>(
    budget: &mut Budget<'work>,
    consumed: usize,
    run: impl FnOnce(&mut Budget<'work>) -> std::result::Result<(T, usize), Error>,
) -> std::result::Result<T, Error> {
    resources::owned(budget, consumed, Error::Resource, || Error::Panicked, run)
}

#[test]
fn constructor_scope_success_error_panic_and_extra_storage_cleanup() {
    for mode in 0..4 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR + 41).unwrap();
        let result = owned(&mut budget, 41, |budget| {
            budget.charge_work(7).map_err(Error::Resource)?;
            let rows = resources::table::<u64>(9, budget).map_err(Error::Resource)?;
            let retained =
                41 + resources::bytes::<u64>(rows.capacity()).map_err(Error::Resource)?;
            match mode {
                0 => Ok((rows, retained)),
                1 => Err(Error::Refused),
                2 => panic_any("guard construction fixture"),
                _ => {
                    budget.reserve_storage(1).map_err(Error::Resource)?;
                    Ok((rows, retained))
                }
            }
        });
        match result {
            Ok(rows) => {
                assert_eq!(mode, 0);
                let retained = 41 + resources::bytes::<u64>(rows.capacity()).unwrap();
                assert_eq!(budget.storage(), FLOOR + retained);
                drop(rows);
                budget.release_storage(retained).unwrap();
            }
            Err(Error::Refused) => assert_eq!(mode, 1),
            Err(Error::Panicked) => assert_eq!(mode, 2),
            Err(Error::Resource(Resource::Accounting)) => assert_eq!(mode, 3),
            _ => panic!("unexpected constructor outcome"),
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 16);
    }
}

#[test]
fn private_scope_never_credits_foreign_ledger_or_undercut_floor() {
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: std::result::Result<(), Error> = owned(&mut budget, 0, |budget| {
        budget.charge_work(3).map_err(Error::Resource)?;
        let mut replacement = Budget::new(&mut foreign_work, STORAGE);
        replacement.reserve_storage(FLOOR).unwrap();
        *budget = replacement;
        Ok(((), 0))
    });
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!((budget.work(), budget.storage()), (0, FLOOR));
    assert_eq!(work.work(), 3);
    assert_eq!(foreign_work.work(), 0);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result: std::result::Result<(), Error> = owned(&mut budget, 0, |budget| {
        budget.release_storage(1).map_err(Error::Resource)?;
        Err(Error::Refused)
    });
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), FLOOR - 1);
}

#[test]
fn requested_capacity_and_name_are_prepaid_with_exact_and_one_short_controls() {
    let run = |limit, capacity| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, capacity);
        budget.reserve_storage(FLOOR).unwrap();
        let result = owned(&mut budget, 0, |budget| {
            let mut rows = resources::table::<u64>(7, budget).map_err(Error::Resource)?;
            let name = resources::name("generic_root", budget).map_err(Error::Resource)?;
            let retained = resources::bytes::<u64>(rows.capacity()).map_err(Error::Resource)?
                + name.capacity();
            assert_eq!(budget.storage(), FLOOR + retained);
            rows.extend(0..7);
            Ok(((rows, name), retained))
        });
        let spent = budget.work();
        let peak = budget.peak_storage();
        let result = result.map(|(rows, name)| {
            let retained = resources::bytes::<u64>(rows.capacity()).unwrap() + name.capacity();
            drop((rows, name));
            budget.release_storage(retained).unwrap();
        });
        assert_eq!(budget.storage(), FLOOR);
        (result, spent, peak)
    };
    let (result, spent, peak) = run(WORK, STORAGE);
    result.unwrap();
    run(spent, peak).0.unwrap();
    assert!(matches!(
        run(spent - 1, peak).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(spent, peak - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(
        resources::bytes::<u64>(usize::MAX),
        Err(Resource::Arithmetic)
    );
}

#[test]
fn rejected_result_and_original_panic_payload_drop_only_after_valid_cleanup() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic_any("payload destructor");
        }
    }
    for original in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let result: std::result::Result<Bomb, Error> = owned(&mut budget, 0, |budget| {
                budget.reserve_storage(91).map_err(Error::Resource)?;
                if original {
                    panic_any(Bomb);
                }
                Ok((Bomb, 90))
            });
            // A rejected candidate's destructor is caught by the scope; its
            // new string payload drops normally after cleanup.
            assert_eq!(result.err(), Some(Error::Resource(Resource::Accounting)));
        }));
        assert_eq!(outcome.is_err(), original);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn panicking_diagnostic_conversion_happens_after_owned_cleanup() {
    for panic_mapper in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _: std::result::Result<(), Error> = resources::owned(
                &mut budget,
                0,
                |_| panic_any("resource diagnostic"),
                || panic_any("panic diagnostic"),
                |budget| {
                    budget.reserve_storage(91).unwrap();
                    if panic_mapper {
                        panic_any("construction");
                    }
                    Ok(((), 90))
                },
            );
        }));
        assert!(outcome.is_err());
        assert_eq!(budget.storage(), FLOOR);
    }
}
