use super::super::tests::{SPACE, WORK, fixture, input, observe};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[test]
fn callback_error_panic_underpayment_and_changed_floor_are_never_adopted() {
    for mode in 0..4 {
        let input = input(&fixture());
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(19 + input.storage).unwrap();
        let observed = observe(&input, &mut budget);
        let visited = Cell::new(false);
        let residual = Cell::new(0);
        let result = observed.try_check_and_finish_with_v18(&mut budget, |_, budget| {
            visited.set(true);
            match mode {
                0 => Err("source rejection"),
                1 => panic!("source callback panic"),
                2 => Ok((99usize, 0)),
                3 => {
                    budget.reserve_storage(1).unwrap();
                    residual.set(budget.storage());
                    Ok((99usize, size_of::<usize>()))
                }
                _ => unreachable!(),
            }
        });
        assert!(visited.get());
        match mode {
            0 => assert!(matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::Origin(
                    "source rejection"
                ))
            )),
            1 => assert!(matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::Panicked)
            )),
            _ => assert!(matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::OriginAccounting)
            )),
        }
        if mode == 3 {
            assert!(residual.get() > 19 + input.storage);
            assert_eq!(budget.storage(), residual.get());
        } else {
            assert_eq!(budget.storage(), 19 + input.storage);
        }
    }
}

struct OriginDrop(Arc<AtomicUsize>);
impl Drop for OriginDrop {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn changed_callback_floor_never_refunds_on_success_error_or_unwind() {
    for surplus in [false, true] {
        for exit in 0..3 {
            let input = input(&fixture());
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, SPACE);
            budget.reserve_storage(23 + input.storage).unwrap();
            let observed = observe(&input, &mut budget);
            let residual = Cell::new(0);
            let drops = Arc::new(AtomicUsize::new(0));
            let payload = OriginDrop(drops.clone());
            let result = observed.try_check_and_finish_with_v18(&mut budget, |_, budget| {
                if surplus {
                    budget.reserve_storage(1).unwrap();
                } else {
                    budget.release_storage(1).unwrap();
                }
                residual.set(budget.storage());
                match exit {
                    0 => Ok((payload, size_of::<OriginDrop>())),
                    1 => {
                        drop(payload);
                        Err("selected origin error")
                    }
                    2 => std::panic::panic_any(payload),
                    _ => unreachable!(),
                }
            });
            match exit {
                0 => assert!(matches!(
                    result,
                    Err(KirCheckedNeutralOptimizationErrorV1::OriginAccounting)
                )),
                1 => assert!(matches!(
                    result,
                    Err(KirCheckedNeutralOptimizationErrorV1::Origin(
                        "selected origin error"
                    ))
                )),
                2 => assert!(matches!(
                    result,
                    Err(KirCheckedNeutralOptimizationErrorV1::Panicked)
                )),
                _ => unreachable!(),
            }
            assert!(residual.get() > 23 + input.storage);
            assert_eq!(
                budget.storage(),
                residual.get(),
                "surplus={surplus}, exit={exit}"
            );
            assert_eq!(drops.load(Ordering::SeqCst), 1);
        }
    }
}

#[test]
fn changed_callback_floor_preserves_work_peak_and_first_storage_failure() {
    let input = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(31 + input.storage).unwrap();
    let observed = observe(&input, &mut budget);
    let state = Cell::new(None);
    let result = observed.try_check_and_finish_with_v18::<(), _, _>(&mut budget, |_, budget| {
        let rejected = budget.storage() + SPACE;
        assert!(budget.reserve_storage(SPACE).is_err());
        assert_eq!(budget.failed_storage(), Some(rejected));
        budget.charge_work(3).unwrap();
        budget.release_storage(1).unwrap();
        state.set(Some((
            budget.storage(),
            budget.work(),
            budget.peak_storage(),
            rejected,
        )));
        Err("selected origin error")
    });
    assert!(matches!(
        result,
        Err(KirCheckedNeutralOptimizationErrorV1::Origin(
            "selected origin error"
        ))
    ));
    let (storage, work, peak, rejected) = state.get().expect("callback was entered");
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (storage, work, peak)
    );
    assert_eq!(budget.failed_storage(), Some(rejected));
}

struct Payload {
    remaining: usize,
    drops: Arc<AtomicUsize>,
}
impl Drop for Payload {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
        if self.remaining != 0 {
            std::panic::panic_any(Payload {
                remaining: self.remaining - 1,
                drops: self.drops.clone(),
            });
        }
    }
}

#[test]
fn bounded_nested_panic_payload_destruction_restores_the_original_floor() {
    for remaining in 0..4 {
        let input = input(&fixture());
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(43 + input.storage).unwrap();
        let observed = observe(&input, &mut budget);
        let drops = Arc::new(AtomicUsize::new(0));
        let payload = Payload {
            remaining,
            drops: drops.clone(),
        };
        let result = observed
            .try_check_and_finish_with_v18::<(), (), _>(&mut budget, move |_, _| {
                std::panic::panic_any(payload)
            });
        assert!(matches!(
            result,
            Err(KirCheckedNeutralOptimizationErrorV1::Panicked)
        ));
        assert_eq!(drops.load(Ordering::SeqCst), remaining + 1);
        assert_eq!(budget.storage(), 43 + input.storage);
    }
}
