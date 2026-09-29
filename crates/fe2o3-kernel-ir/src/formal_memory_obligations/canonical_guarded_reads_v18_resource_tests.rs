use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn headers<T>() -> usize {
    size_of::<Facts<'_, VerifiedCanonicalKernelIrModuleV18>>()
        + size_of::<Accounting>()
        + size_of::<Cell<bool>>()
        + size_of::<CheckedCanonicalGuardedGlobalReadsV18<'_, '_>>()
        + 2 * size_of::<std::thread::Result<Result<T>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<&CanonicalGuardedGlobalReadFactV18<'_, '_>>()
        + size_of::<CanonicalGuardedReadIndexOriginV1>()
        + size_of::<ValueId>()
        + size_of::<(ValueId, ValueId)>()
}

#[test]
fn independent_v18_first_header_and_work_denials_preserve_paid_owner_floor() {
    let (graph, credit) = owner(&fixture(false));
    let floor = credit + 17;
    let header = headers::<()>();
    for allowed in [header - 1, header] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = Budget::new(&mut work, floor + allowed);
        budget.reserve_storage(floor).unwrap();
        let mut callbacks = 0;
        let result = with_canonical_guarded_global_reads_v18(
            &graph,
            Default::default(),
            &mut budget,
            |_, _| {
                callbacks += 1;
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
        assert_eq!(callbacks, 0);
        assert_eq!(budget.storage(), floor);
        if allowed < header {
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(budget.failed_storage(), Some(floor + header));
        } else {
            assert_eq!(budget.work(), 1);
            assert_eq!(budget.peak_storage(), floor + header);
            assert_eq!(
                budget.failed_storage(),
                Some(floor + header + size_of::<FunctionFacts<'_>>())
            );
        }
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(floor).unwrap();
    let result =
        with_canonical_guarded_global_reads_v18(&graph, Default::default(), &mut budget, |_, _| {
            Ok(())
        });
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Work(_)))
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.peak_storage(), floor + header);
    assert_eq!(budget.storage(), floor);
}

fn measured(
    graph: &VerifiedCanonicalKernelIrModuleV18,
    credit: usize,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_reads_v18(
        graph,
        Default::default(),
        &mut budget,
        |view, budget| {
            assert_eq!(view.function_count(budget)?, 3);
            assert_eq!(
                view.function_effects(FunctionCoordinate(1), budget)?,
                (1, 1, 1)
            );
            assert!(
                view.no_wrap_at(coordinate(0, 1, 2), coordinate(0, 0, 1), budget)?
                    .is_some()
            );
            Ok(())
        },
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn measured_v18_exact_and_one_short_work_and_peak_preserve_floor() {
    let (graph, credit) = owner(&fixture(true));
    let (result, work, peak) = measured(&graph, credit, 100_000_000, 100_000_000);
    result.unwrap();
    measured(&graph, credit, work, peak).0.unwrap();
    assert!(matches!(
        measured(&graph, credit, work - 1, peak).0,
        Err(Failure::Resource(ResourceError::Work(_)))
    ));
    assert!(matches!(
        measured(&graph, credit, work, peak - 1).0,
        Err(Failure::Resource(ResourceError::Storage { .. }))
    ));
}

#[test]
fn v18_foreign_ledger_and_moved_slot_fail_stickily() {
    let (graph, credit) = owner(&fixture(true));
    let result = run(&graph, credit, |view, budget| {
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut foreign = Budget::new(&mut foreign_work, 10_000_000);
        foreign.reserve_storage(budget.storage()).unwrap();
        assert!(matches!(
            view.owner(&mut foreign),
            Err(Failure::Resource(ResourceError::Accounting))
        ));
        assert!(matches!(
            view.function_count(budget),
            Err(Failure::Resource(ResourceError::Accounting))
        ));
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut replacement_work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    let mut replacement = Budget::new(&mut replacement_work, 100_000_000);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_reads_v18(
        &graph,
        Default::default(),
        &mut budget,
        |view, budget| {
            std::mem::swap(budget, &mut replacement);
            assert!(matches!(
                view.owner(&mut replacement),
                Err(Failure::Resource(ResourceError::Accounting))
            ));
            std::mem::swap(budget, &mut replacement);
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
    assert_eq!(budget.storage(), credit + 17);
    assert_eq!(replacement.storage(), 0);
}

#[test]
fn v18_ignored_coordinate_failure_and_leaked_scratch_are_rejected() {
    let (graph, credit) = owner(&fixture(true));
    let invalid = coordinate(0, 99, 0);
    let result = run(&graph, credit, |view, budget| {
        assert!(
            matches!(view.read_at(invalid, budget), Err(Failure::Coordinate(actual)) if actual == invalid)
        );
        Ok(())
    });
    assert!(matches!(result, Err(Failure::Coordinate(actual)) if actual == invalid));
    let result = run(&graph, credit, |view, budget| {
        budget.reserve_storage(19)?;
        assert_eq!(view.function_count(budget)?, 3);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
    run(&graph, credit, |view, budget| {
        budget.reserve_storage(19)?;
        assert_eq!(view.function_count(budget)?, 3);
        budget.release_storage(19)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn v18_nested_panic_payloads_drop_before_owner_floor_is_restored() {
    struct Payload {
        remaining: usize,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
            if self.remaining > 0 {
                std::panic::panic_any(Payload {
                    remaining: self.remaining - 1,
                    drops: self.drops.clone(),
                });
            }
        }
    }
    let (graph, credit) = owner(&fixture(true));
    let drops = Arc::new(AtomicUsize::new(0));
    let result: Result<()> = run(&graph, credit, |view, budget| {
        assert_eq!(view.function_count(budget)?, 3);
        std::panic::panic_any(Payload {
            remaining: 2,
            drops: drops.clone(),
        });
    });
    assert!(matches!(result, Err(Failure::Panicked)));
    assert_eq!(drops.load(Ordering::SeqCst), 3);
}

#[test]
fn v18_result_drops_before_leaked_credit_is_refunded() {
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (graph, credit) = owner(&fixture(true));
    let drops = Arc::new(AtomicUsize::new(0));
    let result = run(&graph, credit, |_, budget| {
        budget.reserve_storage(23)?;
        Ok(Payload(drops.clone()))
    });
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
