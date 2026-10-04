use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    panic::panic_any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn closed_scope_drops_partial_backing_before_exact_floor_cleanup() {
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
                1 => Err(Error::LoadRowExtent),
                _ => panic!("test-only decoded-input construction panic"),
            }
        });
        match exit {
            0 => assert_eq!(result.unwrap(), 11),
            1 => assert!(matches!(result, Err(Error::LoadRowExtent))),
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
fn closed_scope_refuses_foreign_ledger_and_undercut_without_refunding_them() {
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
        panic!("test-only hostile payload destructor");
    }
}

#[test]
fn panic_payload_destruction_is_deferred_through_owned_drop_and_valid_cleanup() {
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
        panic!("test-only rejected result destructor");
    }
}

#[test]
fn rejected_success_drop_is_contained_after_accounting_refusal() {
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
fn cleanup_preserves_first_denial_and_accepted_work_on_all_normal_exits() {
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
                Err(Error::LoadRowExtent)
            }),
            Err(Error::LoadRowExtent)
        ));
        assert_eq!(budget.storage(), 13);
        (budget.work(), budget.failed_storage())
    };
    assert_eq!(
        (accepted, failed_storage, work.failed_work()),
        (14, Some(usize::MAX), Some(usize::MAX))
    );
}

#[test]
fn little_endian_p5_coordinate_copy_preserves_all_six_words() {
    let bytes: Vec<_> = [7u32, 11, 19, 23, 29, u32::MAX]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let first = coordinate(&bytes[..12]);
    let second = coordinate(&bytes[12..]);
    assert_eq!(
        (first.block.function.0, first.block.block, first.operation),
        (7, 11, 19)
    );
    assert_eq!(
        (
            second.block.function.0,
            second.block.block,
            second.operation
        ),
        (23, 29, u32::MAX)
    );
}
