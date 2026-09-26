use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn actual_public_callback_balanced_scratch_and_sticky_undercut_preserve_owner_floor() {
    for undercut in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = owner(SourceCase::Repeated, Order::Blocked, &mut budget);
        let floor = budget.storage();
        let result = consume(&owner, &mut budget, |view, budget| {
            assert!(view.read_count(budget)? > 0);
            if undercut {
                budget.release_storage(1)?;
                assert_eq!(
                    view.read_count(budget).unwrap_err(),
                    ReadError::Resource(Resource::Accounting)
                );
                budget.reserve_storage(1)?;
            } else {
                budget.reserve_storage(31)?;
                assert!(view.alias_count(budget)? > 0);
                budget.release_storage(31)?;
            }
            Ok(())
        });
        if undercut {
            assert_eq!(
                result.unwrap_err(),
                ReadError::Resource(Resource::Accounting)
            );
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), floor);
        release(owner, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn real_nested_panic_payload_backing_is_dropped_before_original_floor_returns() {
    struct Payload {
        next: Option<Box<Payload>>,
        backing: Vec<u8>,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
            if let Some(next) = self.next.take() {
                std::panic::panic_any(next);
            }
        }
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = owner(SourceCase::Repeated, Order::Striped, &mut budget);
    let floor = budget.storage();
    let drops = Arc::new(AtomicUsize::new(0));
    let result: Result<(), ReadError> = consume(&owner, &mut budget, |view, budget| {
        assert!(view.read_count(budget)? > 0);
        let mut next = None;
        for size in [3, 5, 7] {
            budget.reserve_storage(std::mem::size_of::<Payload>() + size)?;
            let backing = vec![0_u8; size];
            budget.reserve_storage(backing.capacity() - size)?;
            next = Some(Box::new(Payload {
                next,
                backing,
                drops: drops.clone(),
            }));
        }
        assert_eq!(next.as_ref().unwrap().backing.len(), 7);
        std::panic::panic_any(next.unwrap());
    });
    assert_eq!(result.unwrap_err(), ReadError::Panicked);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    assert_eq!(budget.storage(), floor);
    release(owner, &mut budget);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn rejected_callback_value_is_destroyed_without_returning_unpaid_backing() {
    struct Paid {
        backing: Vec<u8>,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Paid {
        fn drop(&mut self) {
            assert_eq!(self.backing.len(), 13);
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let owner = owner(SourceCase::Repeated, Order::Blocked, &mut budget);
    let floor = budget.storage();
    let drops = Arc::new(AtomicUsize::new(0));
    let result = consume(&owner, &mut budget, |view, budget| {
        assert!(view.read_count(budget)? > 0);
        budget.reserve_storage(13)?;
        let backing = vec![0_u8; 13];
        budget.reserve_storage(backing.capacity() - 13)?;
        Ok(Paid {
            backing,
            drops: drops.clone(),
        })
    });
    assert!(matches!(
        result,
        Err(ReadError::Resource(Resource::Accounting))
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), floor);
    release(owner, &mut budget);
}

#[test]
fn source_graph_and_transport_postflights_each_charge_one_live_work_unit() {
    // Instrumentation consumes the unused budget explicitly. This checks the
    // three source-derived final queries, NOT whole-entry expected work or a
    // preflight-derived boundary. No successful total is reused as an oracle.
    for remaining in [2, 3] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let owner = owner(SourceCase::Repeated, Order::Striped, &mut budget);
        let floor = budget.storage();
        let result = consume(&owner, &mut budget, |view, budget| {
            assert!(view.read_count(budget)? > 0);
            budget.charge_work(
                LIMIT
                    .checked_sub(budget.work())
                    .unwrap()
                    .checked_sub(remaining)
                    .unwrap(),
            )?;
            Ok(())
        });
        if remaining == 3 {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(ReadError::Transport(
                    crate::ProductionTileScalarTransportErrorV29::Resource(Resource::Work(_))
                ))
            ));
        }
        assert_eq!(budget.work(), LIMIT);
        assert_eq!(budget.storage(), floor);
        release(owner, &mut budget);
    }
}

#[test]
fn first_source_query_failure_survives_foreign_budget_then_nested_callback_panic() {
    struct Payload {
        next: Option<Box<Payload>>,
        backing: Vec<u8>,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            assert_eq!(self.backing.len(), 5);
            self.drops.fetch_add(1, Ordering::SeqCst);
            if let Some(next) = self.next.take() {
                std::panic::panic_any(next);
            }
        }
    }
    for binding_first in [false, true] {
        for nested in [1, 2] {
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(FLOOR).unwrap();
            let owner = owner(SourceCase::Repeated, Order::Striped, &mut budget);
            let floor = budget.storage();
            let drops = Arc::new(AtomicUsize::new(0));
            let expected = if binding_first {
                ReadError::Binding {
                    obligation: None,
                    reason: "read ordinal",
                }
            } else {
                ReadError::Resource(Resource::Accounting)
            };
            let result: Result<(), ReadError> = consume(&owner, &mut budget, |view, budget| {
                assert!(view.read_count(budget)? > 0);
                let mut payload = None;
                for _ in 0..=nested {
                    budget.reserve_storage(std::mem::size_of::<Payload>() + 5)?;
                    let backing = vec![0_u8; 5];
                    budget.reserve_storage(backing.capacity() - 5)?;
                    payload = Some(Box::new(Payload {
                        next: payload,
                        backing,
                        drops: drops.clone(),
                    }));
                }
                if binding_first {
                    assert_eq!(view.read(usize::MAX, budget).err().unwrap(), expected);
                }
                let before = budget.work();
                let mut foreign_work = Work::new(LIMIT);
                let mut foreign = Budget::new(&mut foreign_work, LIMIT);
                foreign.reserve_storage(7).unwrap();
                assert_eq!(
                    view.read_count(&mut foreign),
                    Err(ReadError::Resource(Resource::Accounting))
                );
                assert_eq!(foreign.work(), 0);
                assert_eq!(foreign.storage(), 7);
                assert_eq!(budget.work(), before);
                std::panic::panic_any(payload.unwrap());
            });
            assert_eq!(result, Err(expected));
            assert_eq!(drops.load(Ordering::SeqCst), nested + 1);
            assert_eq!(budget.storage(), floor);
            release(owner, &mut budget);
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}
