use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::mem::size_of;

const FLOOR: usize = 37;
fn cpc_headers() -> [usize; 3] {
    [
        size_of::<Cleanup<'_, '_>>(),
        size_of::<std::thread::Result<R<CsResultV1<()>>>>(),
        size_of::<std::thread::Result<CsResultV1<()>>>() + size_of::<std::thread::Result<()>>(),
    ]
}
fn cpc_scope_bytes() -> usize {
    cpc_headers().into_iter().sum()
}
fn index_header() -> usize {
    size_of::<Vec<Option<usize>>>()
}
fn index_rows(count: usize) -> usize {
    count * size_of::<Option<usize>>()
}

#[test]
fn empty_private_scope_exact_headers_and_all_first_denials_are_source_derived() {
    let mut prefix = FLOOR;
    for header in cpc_headers() {
        let denied = prefix + header;
        let mut work = Work::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, denied - 1);
        budget.reserve_storage(FLOOR).unwrap();
        let entered = std::cell::Cell::new(false);
        let result: CsResultV1<()> = cpc_scope_v1(&mut budget, |_| {
            entered.set(true);
            Ok(())
        });
        assert!(result.is_err());
        assert!(!entered.get());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (0, FLOOR, prefix)
        );
        assert_eq!(budget.failed_storage(), Some(denied));
        prefix = denied;
    }
    let mut work = Work::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, prefix);
    budget.reserve_storage(FLOOR).unwrap();
    cpc_scope_v1(&mut budget, |budget| {
        assert_eq!(budget.storage(), prefix);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, FLOOR, prefix)
    );
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn index_arithmetic_overflow_preserves_the_prior_header_only() {
    let mut work = Work::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(cpc_index_v1(usize::MAX, &mut budget).is_err());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, FLOOR + index_header(), FLOOR + index_header())
    );
    assert_eq!(budget.failed_storage(), None);
    budget.release_storage(index_header()).unwrap();
}

#[test]
fn temporary_index_header_and_requested_capacity_denials_have_exact_prefixes() {
    for after_header in [false, true] {
        let accepted = FLOOR + usize::from(after_header) * index_header();
        let denied = FLOOR + index_header() + usize::from(after_header) * index_rows(3);
        let mut work = Work::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, denied - 1);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(cpc_index_v1(3, &mut budget).is_err());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (0, accepted, accepted)
        );
        assert_eq!(budget.failed_storage(), Some(denied));
        budget.release_storage(accepted - FLOOR).unwrap();
    }
}

#[test]
fn index_fill_exact_work_and_one_short_keep_actual_capacity_paid() {
    // The allocator premise is separate from the engine and is not a cost sample.
    let mut premise = Vec::<Option<usize>>::new();
    premise.try_reserve_exact(3).unwrap();
    assert_eq!(
        premise.capacity(),
        3,
        "review exact-capacity premise on this allocator"
    );
    drop(premise);
    let paid = FLOOR + cpc_scope_bytes() + index_header() + index_rows(3);
    for short in [false, true] {
        let mut work = Work::new(3 - usize::from(short));
        {
            let mut budget = ArgumentBudgetV1::new(&mut work, paid);
            budget.reserve_storage(FLOOR).unwrap();
            let result: CsResultV1<()> = cpc_scope_v1(&mut budget, |budget| {
                let index = cpc_index_v1(3, budget)?;
                assert_eq!(index, [None, None, None]);
                assert_eq!(budget.storage(), paid);
                drop(index);
                Ok(())
            });
            assert_eq!(result.is_err(), short);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (if short { 0 } else { 3 }, FLOOR, paid)
            );
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(work.failed_work(), short.then_some(3));
    }
}

#[test]
fn temporary_index_retained_bytes_use_actual_capacity_and_drop_before_release() {
    let mut work = Work::new(3);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1 << 20);
    budget.reserve_storage(FLOOR).unwrap();
    let index = cpc_index_v1(3, &mut budget).unwrap();
    let paid = index_header() + index_rows(index.capacity());
    assert!(index.capacity() >= 3);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (3, FLOOR + paid, FLOOR + paid)
    );
    drop(index);
    budget.release_storage(paid).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn partial_index_failure_is_cleaned_without_erasing_storage_denial() {
    let peak = FLOOR + cpc_scope_bytes() + index_header();
    let mut work = Work::new(3);
    let mut budget = ArgumentBudgetV1::new(&mut work, peak + index_rows(3) - 1);
    budget.reserve_storage(FLOOR).unwrap();
    let result: CsResultV1<()> = cpc_scope_v1(&mut budget, |budget| {
        drop(cpc_index_v1(3, budget)?);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, FLOOR, peak)
    );
    assert_eq!(budget.failed_storage(), Some(peak + index_rows(3)));
}

#[test]
fn nested_recursive_payloads_are_drained_before_outer_scope_refund() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Trace {
        next: AtomicUsize,
        order: [AtomicUsize; 3],
    }
    struct Payload {
        trace: Arc<Trace>,
        ordinal: usize,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            let position = self.trace.next.fetch_add(1, Ordering::SeqCst) + 1;
            self.trace.order[self.ordinal].store(position, Ordering::SeqCst);
            if self.ordinal < 2 {
                std::panic::panic_any(Self {
                    trace: self.trace.clone(),
                    ordinal: self.ordinal + 1,
                });
            }
        }
    }
    let trace = Arc::new(Trace {
        next: AtomicUsize::new(0),
        order: std::array::from_fn(|_| AtomicUsize::new(0)),
    });
    let floor = FLOOR + 3 * size_of::<Payload>() + size_of::<Trace>();
    let outer = floor + cpc_scope_bytes() + 11;
    let peak = outer + cpc_scope_bytes() + 13;
    let mut work = Work::new(5);
    let mut budget = ArgumentBudgetV1::new(&mut work, peak);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(&budget) as usize;
    let intermediate = std::cell::Cell::new(None);
    let result: CsResultV1<()> = cpc_scope_v1(&mut budget, |budget| {
        budget.reserve_storage(11)?;
        budget.charge_work(2)?;
        let result: CsResultV1<()> = cpc_scope_v1(budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(3)?;
            std::panic::panic_any(Payload {
                trace: trace.clone(),
                ordinal: 0,
            });
        });
        intermediate.set(Some((
            matches!(
                result,
                Err(ProductionCanonicalScalarSourceErrorV1::Assertion(
                    Failure::Panicked
                ))
            ),
            budget.storage(),
            trace.next.load(Ordering::SeqCst),
        )));
        result
    });
    assert!(result.is_err());
    assert_eq!(intermediate.get(), Some((true, outer, 3)));
    for n in 0..3 {
        assert_eq!(trace.order[n].load(Ordering::SeqCst), n + 1);
    }
    assert_eq!(std::ptr::from_ref(&budget) as usize, slot);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (5, floor, peak)
    );
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn scope_accounting_failure_keeps_denial_and_drops_rejected_value() {
    struct Value<'a>(&'a std::cell::Cell<usize>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = std::cell::Cell::new(0);
    let mut work = Work::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1 << 20);
    budget.reserve_storage(FLOOR).unwrap();
    let result = cpc_scope_v1(&mut budget, |budget| {
        budget.release_storage(budget.storage() - FLOOR)?;
        Ok(Value(&drops))
    });
    assert!(result.is_err());
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), 0);
}
