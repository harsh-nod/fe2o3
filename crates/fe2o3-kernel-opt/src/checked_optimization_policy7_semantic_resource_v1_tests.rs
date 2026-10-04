use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn local_scope_preserves_prefix_work_and_storage_on_success_error_and_panic() {
    for mode in 0..3 {
        let mut work = Work::new(17);
        let mut budget = Budget::new(&mut work, 41);
        budget.charge_work(2).unwrap();
        budget.reserve_storage(11).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(30)?;
            budget.charge_work(3)?;
            match mode {
                0 => Ok(()),
                1 => Err(Error::Rows),
                _ => std::panic::panic_any("scope test"),
            }
        });
        assert_eq!(budget.storage(), 11);
        assert_eq!(budget.work(), 5);
        assert_eq!(budget.peak_storage(), 41);
        assert!(budget.work_ledger_identity_v1() == ledger);
        match mode {
            0 => result.unwrap(),
            1 => assert!(matches!(result, Err(Error::Rows))),
            _ => assert!(matches!(result, Err(Error::Panicked))),
        }
    }
}

#[test]
fn local_scope_never_releases_a_foreign_work_ledger_or_undercut_floor() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 11);
    budget.reserve_storage(11).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 10);
    let foreign_work = Box::leak(Box::new(Work::new(10)));
    let result = scoped(&mut budget, |budget| {
        let mut foreign = Budget::new(foreign_work, 19);
        foreign.reserve_storage(19)?;
        *budget = foreign;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 19);
}

#[test]
fn hostile_panic_payload_destruction_happens_after_scope_cleanup() {
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            std::panic::panic_any("hostile payload destructor");
        }
    }
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 41);
    budget.reserve_storage(11).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.reserve_storage(30)?;
            budget.charge_work(3)?;
            std::panic::panic_any(Payload)
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 11);
    assert_eq!(budget.work(), 3);
}
