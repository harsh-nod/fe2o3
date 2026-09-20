use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    panic::panic_any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[test]
fn pool_vector_accounts_observed_capacity_before_initializing_entries() {
    for count in [0, 1, 6] {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 10_000);
        budget.reserve_storage(13).unwrap();
        let values = entries(count, &mut budget).unwrap();
        assert!(values.is_empty());
        assert!(values.capacity() >= count);
        let backing = values.capacity() * size_of::<Entry>();
        assert_eq!(budget.storage(), 13 + backing);
        drop(values);
        budget.release_storage(backing).unwrap();
        assert_eq!(budget.storage(), 13);
        assert_eq!(budget.work(), 0);
        if count > 0 {
            let mut work = Work::new(0);
            let mut budget = Budget::new(&mut work, 13 + count * size_of::<Entry>() - 1);
            budget.reserve_storage(13).unwrap();
            assert!(matches!(
                entries(count, &mut budget),
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.storage(), 13);
            assert_eq!(
                budget.failed_storage(),
                Some(13 + count * size_of::<Entry>())
            );
        }
    }
}

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn scope_restores_exact_floor_and_work_after_owned_scratch_drops() {
    for exit in 0..3 {
        let mut work = Work::new(9);
        let mut budget = Budget::new(&mut work, 53);
        budget.reserve_storage(13).unwrap();
        budget.charge_work(2).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let dropped = Arc::new(AtomicBool::new(false));
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(40)?;
            budget.charge_work(7)?;
            let _owned = Dropped(dropped.clone());
            match exit {
                0 => Ok(11),
                1 => Err(Error::Pool),
                _ => panic!("test-only graph-pool construction panic"),
            }
        });
        match exit {
            0 => assert_eq!(result.unwrap(), 11),
            1 => assert!(matches!(result, Err(Error::Pool))),
            _ => assert!(matches!(result, Err(Error::Panicked))),
        }
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(
            (budget.storage(), budget.peak_storage(), budget.work()),
            (13, 53, 9)
        );
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn scope_refuses_undercut_and_foreign_work_without_foreign_refund() {
    let mut first = Work::new(20);
    let mut second = Work::new(20);
    let mut budget = Budget::new(&mut first, 100);
    let mut foreign = Budget::new(&mut second, 100);
    budget.reserve_storage(13).unwrap();
    foreign.reserve_storage(29).unwrap();
    let original = budget.work_ledger_identity_v1();
    let other = foreign.work_ledger_identity_v1();
    let result = scoped(&mut budget, |budget| {
        budget.reserve_storage(17)?;
        budget.charge_work(3)?;
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(budget.work_ledger_identity_v1() == other);
    assert!(foreign.work_ledger_identity_v1() == original);
    assert_eq!(
        (budget.storage(), foreign.storage(), foreign.work()),
        (29, 30, 3)
    );
    std::mem::swap(&mut budget, &mut foreign);
    budget.release_storage(17).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 12);
}

struct HostilePayload(Arc<AtomicBool>);
impl Drop for HostilePayload {
    fn drop(&mut self) {
        assert!(self.0.load(Ordering::SeqCst));
        panic!("test-only hostile panic payload destructor");
    }
}

#[test]
fn hostile_payload_drops_after_local_owners_and_exact_floor_cleanup() {
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(13).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.reserve_storage(40)?;
            budget.charge_work(7)?;
            let _owned = Dropped(dropped.clone());
            panic_any(HostilePayload(dropped.clone()))
        });
    }));
    assert!(result.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(
        (budget.storage(), budget.peak_storage(), budget.work()),
        (13, 53, 7)
    );
}

struct Rejected(Arc<AtomicBool>);
impl Drop for Rejected {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
        panic!("test-only rejected success destructor");
    }
}

#[test]
fn rejected_success_destructor_is_contained_after_accounting_refusal() {
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(13).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        budget.charge_work(7)?;
        Ok(Rejected(dropped.clone()))
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!((budget.storage(), budget.work()), (12, 7));
}

#[test]
fn scope_keeps_first_denial_history_on_success_and_error() {
    let mut work = Work::new(20);
    let (accepted, failed_storage) = {
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(13).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        assert_eq!(
            scoped(&mut budget, |budget| {
                budget.reserve_storage(17)?;
                budget.charge_work(7)?;
                Ok(11)
            })
            .unwrap(),
            11
        );
        assert!(matches!(
            scoped::<()>(&mut budget, |budget| {
                budget.reserve_storage(17)?;
                budget.charge_work(7)?;
                Err(Error::Pool)
            }),
            Err(Error::Pool)
        ));
        assert_eq!(budget.storage(), 13);
        (budget.work(), budget.failed_storage())
    };
    assert_eq!(
        (accepted, failed_storage, work.failed_work()),
        (14, Some(usize::MAX), Some(usize::MAX))
    );
}
