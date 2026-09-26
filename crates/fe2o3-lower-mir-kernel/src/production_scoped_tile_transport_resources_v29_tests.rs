use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct NestedPayload {
    remaining: usize,
    drops: Arc<AtomicUsize>,
}
impl Drop for NestedPayload {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
        if self.remaining != 0 {
            std::panic::panic_any(Self {
                remaining: self.remaining - 1,
                drops: Arc::clone(&self.drops),
            });
        }
    }
}
#[test]
fn public_callback_drains_one_and_two_nested_payloads_before_floor_restore() {
    for depth in [1, 2] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
        let floor = budget.storage();
        let drops = Arc::new(AtomicUsize::new(0));
        let result: Result<(), Error> =
            owner.with_checked_transport_v29(&mut budget, |view, budget| {
                assert!(view.pending_obligation_count(budget)? > 0);
                std::panic::panic_any(NestedPayload {
                    remaining: depth,
                    drops: Arc::clone(&drops),
                });
            });
        assert_eq!(result, Err(Error::Panicked));
        assert_eq!(drops.load(Ordering::SeqCst), depth + 1);
        assert_eq!(budget.storage(), floor);
        check_complete(&owner, &mut budget);
        drop_owner(owner, &mut budget);
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    }
}
#[test]
fn rejected_return_value_and_its_nested_payload_preserve_accounting_precedence() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let floor = budget.storage();
    let drops = Arc::new(AtomicUsize::new(0));
    let result = owner.with_checked_transport_v29(&mut budget, |view, budget| {
        budget.release_storage(1)?;
        assert_eq!(
            view.root_count(budget),
            Err(Error::Resource(ArgumentResourceV1::Accounting))
        );
        budget.reserve_storage(1)?;
        Ok(NestedPayload {
            remaining: 2,
            drops: Arc::clone(&drops),
        })
    });
    assert!(matches!(
        result,
        Err(Error::Resource(ArgumentResourceV1::Accounting))
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    assert_eq!(budget.storage(), floor);
    drop_owner(owner, &mut budget);
}

#[test]
fn sticky_accounting_survives_a_later_callback_panic() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let floor = budget.storage();
    let drops = Arc::new(AtomicUsize::new(0));
    let result: Result<(), Error> =
        owner.with_checked_transport_v29(&mut budget, |view, budget| {
            budget.release_storage(1)?;
            assert_eq!(
                view.root_count(budget),
                Err(Error::Resource(ArgumentResourceV1::Accounting))
            );
            budget.reserve_storage(1)?;
            std::panic::panic_any(NestedPayload {
                remaining: 1,
                drops: Arc::clone(&drops),
            });
        });
    assert_eq!(result, Err(Error::Resource(ArgumentResourceV1::Accounting)));
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    assert_eq!(budget.storage(), floor);
    drop_owner(owner, &mut budget);
}
#[test]
fn caller_scratch_is_allowed_during_queries_but_not_returned_as_unowned_credit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let floor = budget.storage();
    owner
        .with_checked_transport_v29(&mut budget, |view, budget| {
            budget.reserve_storage(29)?;
            assert!(view.source_alias_count(budget)? > 0);
            budget.release_storage(29)?;
            Ok(())
        })
        .unwrap();
    let result = owner.with_checked_transport_v29(&mut budget, |view, budget| {
        budget.reserve_storage(29)?;
        assert!(view.source_alias_count(budget)? > 0);
        Ok(())
    });
    assert_eq!(result, Err(Error::Resource(ArgumentResourceV1::Accounting)));
    assert_eq!(budget.storage(), floor);
    drop_owner(owner, &mut budget);
}
#[test]
fn each_paid_query_and_postflight_debit_exactly_one_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let mut last = 0;
    owner
        .with_checked_transport_v29(&mut budget, |view, budget| {
            let before = budget.work();
            view.source_identity(budget)?;
            assert_eq!(budget.work(), before + 1);
            view.root_count(budget)?;
            assert_eq!(budget.work(), before + 2);
            view.piece_alias(0, budget)?;
            assert_eq!(budget.work(), before + 3);
            last = budget.work();
            Ok(())
        })
        .unwrap();
    assert_eq!(
        budget.work(),
        last + 1,
        "one postflight query, no duplicated debit"
    );
    drop_owner(owner, &mut budget);
}

#[test]
fn denied_external_debit_preserves_history_and_accepts_smaller_suffix() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    owner
        .with_checked_transport_v29(&mut budget, |_, budget| {
            budget.charge_work(SCHEDULE_LIMIT - budget.work() - 2)?;
            assert!(budget.charge_work(3).is_err());
            assert_eq!(budget.work(), SCHEDULE_LIMIT - 2);
            budget.charge_work(1)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.work(), SCHEDULE_LIMIT);
    drop_owner(owner, &mut budget);
    assert_eq!(work.failed_work(), Some(SCHEDULE_LIMIT + 1));
}
#[test]
fn exact_and_one_short_postflight_drop_returned_backing_without_reclassifying_failure() {
    for available in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
        let floor = budget.storage();
        let drops = Arc::new(AtomicUsize::new(0));
        let result = owner.with_checked_transport_v29(&mut budget, |_, budget| {
            budget.charge_work(SCHEDULE_LIMIT - budget.work() - available)?;
            Ok(NestedPayload {
                remaining: 0,
                drops: Arc::clone(&drops),
            })
        });
        if available == 0 {
            assert!(matches!(
                result,
                Err(Error::Resource(ArgumentResourceV1::Work(_)))
            ));
            assert_eq!(drops.load(Ordering::SeqCst), 1);
        } else {
            let value = result.ok().expect("one exact postflight debit");
            assert_eq!(drops.load(Ordering::SeqCst), 0);
            drop(value);
            assert_eq!(drops.load(Ordering::SeqCst), 1);
        }
        assert_eq!(budget.work(), SCHEDULE_LIMIT);
        assert_eq!(budget.storage(), floor);
        drop_owner(owner, &mut budget);
    }
}
#[test]
fn scoped_capacity_oracle_has_source_derived_exact_and_one_short_peaks() {
    let header = transport_impl::callback_headers::<()>();
    // Pinned allocator contract for this fixture: try_reserve_exact(3) stores three u64s.
    // No work is charged by the allocation helper; pushes are a separate debit.
    for short in [0, 1] {
        let required = SCHEDULE_FLOOR + header + 3 * size_of::<u64>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, required - short);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let result = transport_impl::allocation_boundary::<u64>(3, &mut budget);
        assert_eq!(result.is_ok(), short == 0);
        if short == 0 {
            assert_eq!(budget.peak_storage(), required);
        } else {
            assert!(matches!(
                result,
                Err(Error::Resource(ArgumentResourceV1::Storage { .. }))
            ));
            assert_eq!(budget.failed_storage(), Some(required));
            assert_eq!(budget.peak_storage(), SCHEDULE_FLOOR + header);
        }
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    }
}
#[test]
fn capacity_arithmetic_overflow_refuses_without_allocating_or_stealing_floor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    assert_eq!(
        transport_impl::allocation_boundary::<u64>(usize::MAX, &mut budget),
        Err(Error::Resource(ArgumentResourceV1::Arithmetic))
    );
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.failed_storage(), None);
}
#[test]
fn public_callback_fixed_header_one_short_refuses_before_replay() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let owner = tile_owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let floor = budget.storage();
    let header = transport_impl::callback_headers::<()>();
    let pressure = budget.storage_limit() - budget.storage() - (header - 1);
    budget.reserve_storage(pressure).unwrap();
    let before_work = budget.work();
    let result = owner.with_checked_transport_v29(&mut budget, |_, _| -> Result<(), Error> {
        panic!("fixed-header denial reached callback")
    });
    assert!(matches!(
        result,
        Err(Error::Resource(ArgumentResourceV1::Storage { .. }))
    ));
    assert_eq!(budget.work(), before_work);
    assert_eq!(budget.storage(), floor + pressure);
    budget.release_storage(pressure).unwrap();
    check_complete(&owner, &mut budget);
    drop_owner(owner, &mut budget);
}
