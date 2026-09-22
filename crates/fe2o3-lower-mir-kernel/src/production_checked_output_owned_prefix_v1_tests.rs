use super::super::{StoreError, StoreResult, store_scope};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use std::{cell::Cell, rc::Rc};

const FLOOR: usize = 137;

struct Tracked {
    identity: u64,
    drops: Rc<Cell<usize>>,
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
fn value(drops: &Rc<Cell<usize>>) -> Tracked {
    Tracked {
        identity: 0x123456789,
        drops: drops.clone(),
    }
}
fn fresh_capacity<T>(request: usize) -> usize {
    // Independent standard-library allocation reference, dropped before the
    // measured helper transaction. No producer receipt/work/peak is observed.
    let mut reference = Vec::<T>::new();
    reference.try_reserve_exact(request).unwrap();
    reference.capacity()
}
fn assert_storage(error: AssertOriginResourceV1, actual: usize, limit: usize) {
    let AssertOriginResourceV1::Storage(error) = error else {
        panic!("expected exact Storage, got {error:?}");
    };
    assert_eq!(error.actual(), actual);
    assert_eq!(error.limit(), limit);
}

#[test]
fn owning_prefix_singleton_has_exact_backing_and_preserves_inherited_credit() {
    let bytes = fresh_capacity::<Tracked>(1) * size_of::<Tracked>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + bytes);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let drops = Rc::new(Cell::new(0));
    let owner = OwnedPrefix::try_new(value(&drops), &mut budget).unwrap();
    assert_eq!(owner.get().identity, 0x123456789);
    assert_eq!(owner.values.len(), 1);
    assert_eq!(owner.retained_storage().unwrap(), bytes);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), FLOOR + bytes);
    assert_eq!(budget.peak_storage(), FLOOR + bytes);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(drops.get(), 0);
    drop(owner);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR + bytes);
    budget.release_storage(bytes).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn owning_prefix_work_boundaries_refuse_before_the_allocator() {
    let bytes = fresh_capacity::<Tracked>(1) * size_of::<Tracked>();
    for limit in [0, 1, 2] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + bytes);
        budget.reserve_storage(FLOOR).unwrap();
        let calls = Cell::new(0);
        let drops = Rc::new(Cell::new(0));
        let result = OwnedPrefix::try_new_with_reserve(value(&drops), &mut budget, |v| {
            calls.set(calls.get() + 1);
            v.try_reserve_exact(1)
        });
        if limit < 2 {
            let error = match result {
                Err(AssertOriginResourceV1::Work(error)) => error,
                _ => panic!("expected Work refusal"),
            };
            assert_eq!(error.actual(), 2);
            assert_eq!(error.limit(), limit);
            assert_eq!(calls.get(), 0);
            assert_eq!(drops.get(), 1);
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR);
        } else {
            let owner = result.unwrap();
            assert_eq!(calls.get(), 1);
            assert_eq!(budget.work(), 2);
            assert_eq!(budget.storage(), FLOOR + bytes);
            assert_eq!(budget.peak_storage(), FLOOR + bytes);
            drop(owner);
            assert_eq!(drops.get(), 1);
            budget.release_storage(bytes).unwrap();
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn owning_prefix_minimum_storage_denial_does_not_allocate_or_refund_floor() {
    let p = size_of::<Tracked>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + p - 1);
    budget.reserve_storage(FLOOR).unwrap();
    let calls = Cell::new(0);
    let drops = Rc::new(Cell::new(0));
    let result = OwnedPrefix::try_new_with_reserve(value(&drops), &mut budget, |v| {
        calls.set(calls.get() + 1);
        v.try_reserve_exact(1)
    });
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("expected minimum backing refusal"),
    };
    assert_storage(error, FLOOR + p, FLOOR + p - 1);
    assert_eq!(calls.get(), 0);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR);
    assert_eq!(budget.failed_storage(), Some(FLOOR + p));
}

#[test]
fn owning_prefix_actual_excess_capacity_is_paid_and_exactly_refused() {
    let p = size_of::<Tracked>();
    let reference_bytes = fresh_capacity::<Tracked>(2) * p;
    assert!(reference_bytes >= 2 * p);
    for limit in [FLOOR + p, FLOOR + reference_bytes] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
        let mut budget = AssertOriginBudgetV1::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let calls = Cell::new(0);
        let allocated_capacity = Cell::new(0);
        let drops = Rc::new(Cell::new(0));
        let result = OwnedPrefix::try_new_with_reserve(value(&drops), &mut budget, |v| {
            calls.set(calls.get() + 1);
            v.try_reserve_exact(2)?;
            allocated_capacity.set(v.capacity());
            Ok(())
        });
        assert_eq!(allocated_capacity.get() * p, reference_bytes);
        assert_eq!(calls.get(), 1);
        assert_eq!(budget.work(), 2);
        if limit == FLOOR + p {
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("expected excess backing refusal"),
            };
            assert_storage(error, FLOOR + reference_bytes, limit);
            assert_eq!(drops.get(), 1);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + p);
            assert_eq!(budget.failed_storage(), Some(FLOOR + reference_bytes));
        } else {
            let owner = result.unwrap();
            assert_eq!(owner.retained_storage().unwrap(), reference_bytes);
            assert_eq!(budget.storage(), FLOOR + reference_bytes);
            assert_eq!(budget.peak_storage(), FLOOR + reference_bytes);
            assert_eq!(budget.failed_storage(), None);
            assert_eq!(drops.get(), 0);
            drop(owner);
            assert_eq!(drops.get(), 1);
            budget.release_storage(reference_bytes).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn owning_prefix_maps_genuine_try_reserve_error_and_releases_only_new_backing() {
    let p = size_of::<Tracked>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + p);
    budget.reserve_storage(FLOOR).unwrap();
    let calls = Cell::new(0);
    let drops = Rc::new(Cell::new(0));
    let result = OwnedPrefix::try_new_with_reserve(value(&drops), &mut budget, |v| {
        calls.set(calls.get() + 1);
        v.try_reserve_exact(usize::MAX)
    });
    assert!(matches!(result, Err(AssertOriginResourceV1::Allocation)));
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + p);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn owning_prefix_checks_extent_arithmetic_and_private_singleton_shape() {
    assert_eq!(
        capacity_bytes::<[u8; 2]>(usize::MAX),
        Err(AssertOriginResourceV1::Arithmetic)
    );
    let empty = OwnedPrefix::<u64> { values: Vec::new() };
    assert_eq!(
        empty.retained_storage(),
        Err(AssertOriginResourceV1::Accounting)
    );
    let p = size_of::<Tracked>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + p);
    budget.reserve_storage(FLOOR).unwrap();
    let drops = Rc::new(Cell::new(0));
    let result = OwnedPrefix::try_new_with_reserve(value(&drops), &mut budget, |_| Ok(()));
    assert!(matches!(result, Err(AssertOriginResourceV1::Accounting)));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + p);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn owning_prefix_zero_sized_value_has_no_fabricated_backing() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = OwnedPrefix::try_new((), &mut budget).unwrap();
    assert_eq!(*owner.get(), ());
    assert_eq!(owner.values.len(), 1);
    assert_eq!(owner.retained_storage().unwrap(), 0);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn owning_prefix_panicking_allocator_preserves_scope_floor_and_drops_input() {
    let p = size_of::<Tracked>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + p);
    budget.reserve_storage(FLOOR).unwrap();
    let drops = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let result: StoreResult<OwnedPrefix<Tracked>> = store_scope(FLOOR, &mut budget, |budget| {
        Ok(OwnedPrefix::try_new_with_reserve(
            value(&drops),
            budget,
            |v| {
                calls.set(calls.get() + 1);
                v.try_reserve_exact(1)?;
                panic!("private test reserve panic before insertion")
            },
        )?)
    });
    assert!(matches!(result, Err(StoreError::Panicked)));
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + p);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn owning_prefix_success_transfers_exact_unreserved_added_backing() {
    let bytes = fresh_capacity::<Tracked>(1) * size_of::<Tracked>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
    let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + bytes);
    budget.reserve_storage(FLOOR).unwrap();
    let drops = Rc::new(Cell::new(0));
    let result: StoreResult<_> = store_scope(FLOOR, &mut budget, |budget| {
        let owner = OwnedPrefix::try_new(value(&drops), budget)?;
        let receipt = owner.retained_storage()?;
        Ok((owner, receipt))
    });
    let (owner, receipt) = result.unwrap();
    assert_eq!(receipt, bytes);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + bytes);
    assert_eq!(drops.get(), 0);
    budget.reserve_storage(receipt).unwrap();
    drop(owner);
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR + bytes);
    budget.release_storage(receipt).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[derive(Clone, Copy)]
struct PaidDropSnapshot {
    storage: usize,
    peak: usize,
    work: usize,
    failed_storage: Option<usize>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

struct BudgetDropProbe<'work> {
    budget: Rc<std::cell::RefCell<Option<AssertOriginBudgetV1<'work>>>>,
    observed: Rc<Cell<Option<PaidDropSnapshot>>>,
}
impl Drop for BudgetDropProbe<'_> {
    fn drop(&mut self) {
        let slot = self.budget.borrow();
        let budget = slot.as_ref().expect("actual budget installed before Drop");
        assert!(
            self.observed.get().is_none(),
            "source element dropped twice"
        );
        self.observed.set(Some(PaidDropSnapshot {
            storage: budget.storage(),
            peak: budget.peak_storage(),
            work: budget.work(),
            failed_storage: budget.failed_storage(),
            ledger: budget.work_ledger_identity_v1(),
        }));
    }
}

#[test]
fn owning_prefix_drop_reads_the_actual_paid_ledger_and_preserves_prior_sentinel() {
    for transferred in [false, true] {
        let bytes = fresh_capacity::<BudgetDropProbe<'_>>(1) * size_of::<BudgetDropProbe<'_>>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(5);
        let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + bytes);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(3).unwrap();
        assert_storage(
            budget.reserve_storage(bytes + 1).unwrap_err(),
            FLOOR + bytes + 1,
            FLOOR + bytes,
        );
        let ledger = budget.work_ledger_identity_v1();
        let watched = Rc::new(std::cell::RefCell::new(None));
        let observed = Rc::new(Cell::new(None));
        let value = BudgetDropProbe {
            budget: watched.clone(),
            observed: observed.clone(),
        };
        let owner = if transferred {
            let result: StoreResult<_> = store_scope(FLOOR, &mut budget, |budget| {
                Ok(OwnedPrefix::try_new(value, budget)?)
            });
            let owner = result.unwrap();
            assert_eq!(budget.storage(), FLOOR);
            budget.reserve_storage(bytes).unwrap();
            owner
        } else {
            OwnedPrefix::try_new(value, &mut budget).unwrap()
        };
        assert!(observed.get().is_none());
        // The helper/scope borrow has ended. Move the SAME actual Budget, not a
        // copied snapshot, into the empty observer; its slot is allowed to move.
        *watched.borrow_mut() = Some(budget);
        drop(owner);
        let snapshot = observed.get().expect("actual element Drop observed");
        assert_eq!(snapshot.storage, FLOOR + bytes);
        assert_eq!(snapshot.peak, FLOOR + bytes);
        assert_eq!(snapshot.work, 5);
        assert_eq!(snapshot.failed_storage, Some(FLOOR + bytes + 1));
        assert!(snapshot.ledger == ledger);
        let mut budget = watched.borrow_mut().take().unwrap();
        assert_eq!(budget.storage(), FLOOR + bytes);
        budget.release_storage(bytes).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 5);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), Some(FLOOR + bytes + 1));
    }
}
