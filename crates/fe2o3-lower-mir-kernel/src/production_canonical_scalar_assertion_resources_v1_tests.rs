use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::mem::size_of;

const FLOOR: usize = 29;
fn header() -> usize {
    size_of::<CsaTransportV1>()
}
fn row_bytes(count: usize) -> usize {
    count * size_of::<ProductionCanonicalScalarAssertionV1>()
}
fn scope_header() -> usize {
    size_of::<Cleanup<'_, '_>>() + size_of::<std::thread::Result<R<CsResultV1<()>>>>()
}

#[test]
fn arithmetic_overflow_precedes_every_sidecar_debit() {
    let mut work = Work::new(10);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(csa_allocate_v1(usize::MAX, &mut budget).is_err());
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn zero_work_denies_before_sidecar_header_or_capacity() {
    let mut work = Work::new(0);
    {
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(csa_allocate_v1(1, &mut budget).is_err());
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
    assert_eq!(work.failed_work(), Some(1));
}

#[test]
fn empty_sidecar_exact_header_and_one_short_have_independent_first_denials() {
    for short in [false, true] {
        let mut work = Work::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + header() - usize::from(short));
        budget.reserve_storage(FLOOR).unwrap();
        let result = csa_allocate_v1(0, &mut budget);
        assert_eq!(budget.work(), 1);
        if short {
            assert!(result.is_err());
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.failed_storage(), Some(FLOOR + header()));
        } else {
            let rows = result.unwrap();
            assert_eq!(rows.storage, header());
            assert_eq!(rows.rows.capacity(), 0);
            assert_eq!(budget.storage(), FLOOR + header());
            drop(rows);
            budget.release_storage(header()).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.failed_storage(), None);
        }
    }
}

#[test]
fn requested_payload_one_short_fails_before_allocation_and_keeps_partial_header_paid() {
    let requested = row_bytes(3);
    let mut work = Work::new(1);
    let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + header() + requested - 1);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(csa_allocate_v1(3, &mut budget).is_err());
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.storage(), FLOOR + header());
    assert_eq!(budget.peak_storage(), FLOOR + header());
    assert_eq!(budget.failed_storage(), Some(FLOOR + header() + requested));
    budget.release_storage(header()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn actual_capacity_not_requested_count_is_the_retained_sidecar_contract() {
    let mut work = Work::new(1);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1 << 20);
    budget.reserve_storage(FLOOR).unwrap();
    let rows = csa_allocate_v1(3, &mut budget).unwrap();
    assert!(rows.rows.capacity() >= 3);
    assert_eq!(rows.storage, header() + row_bytes(rows.rows.capacity()));
    let paid = rows.storage;
    assert_eq!(budget.storage(), FLOOR + paid);
    assert_eq!(budget.work(), 1);
    drop(rows);
    budget.release_storage(paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn old_and_new_capacity_overlap_until_replacement_drops_old_backing() {
    let mut work = Work::new(2);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1 << 20);
    budget.reserve_storage(FLOOR).unwrap();
    let mut old = csa_allocate_v1(2, &mut budget).unwrap();
    let old_paid = old.storage;
    let next = csa_allocate_v1(3, &mut budget).unwrap();
    let next_paid = next.storage;
    assert_eq!(budget.storage(), FLOOR + old_paid + next_paid);
    assert_eq!(budget.peak_storage(), FLOOR + old_paid + next_paid);
    let retired = CsPairObserverV1::replace(&mut old, next);
    assert_eq!(retired, old_paid);
    assert_eq!(budget.storage(), FLOOR + old_paid + next_paid);
    budget.release_storage(retired).unwrap();
    assert_eq!(budget.storage(), FLOOR + next_paid);
    assert_eq!(budget.work(), 2);
    drop(old);
    budget.release_storage(next_paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn scoped_partial_allocation_restores_floor_without_resetting_denial_or_work() {
    let requested = row_bytes(3);
    let peak = FLOOR + scope_header() + header();
    let mut work = Work::new(1);
    let mut budget = ArgumentBudgetV1::new(&mut work, peak + requested - 1);
    budget.reserve_storage(FLOOR).unwrap();
    let result: CsResultV1<()> = csa_scope_v1(&mut budget, |budget| {
        let rows = csa_allocate_v1(3, budget)?;
        drop(rows);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), peak);
    assert_eq!(budget.failed_storage(), Some(peak + requested));
    assert_eq!(budget.work(), 1);
}

#[test]
fn assertion_scope_success_and_panic_both_preserve_sibling_payment() {
    for panic in [false, true] {
        let mut work = Work::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1 << 20);
        budget.reserve_storage(FLOOR).unwrap();
        let result: CsResultV1<()> = csa_scope_v1(&mut budget, |budget| {
            let rows = csa_allocate_v1(0, budget)?;
            if panic {
                panic!("resource scope panic");
            }
            drop(rows);
            Ok(())
        });
        assert_eq!(result.is_err(), panic);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + scope_header() + header());
        assert_eq!(budget.work(), 1);
    }
}
