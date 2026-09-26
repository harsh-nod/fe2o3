use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{cell::Cell, mem::size_of};

#[test]
fn assertion_row_storage_exact_and_one_short_are_source_derived() {
    let count = 3;
    let payload = count * size_of::<Option<ProductionCanonicalAssertionV1>>();
    for shortage in [0, 1] {
        let mut work = Work::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, payload - shortage);
        let value = rows::<Option<ProductionCanonicalAssertionV1>>(count, &mut budget);
        if shortage == 0 {
            let value = value.unwrap();
            assert_eq!(value.capacity(), count);
            assert_eq!(budget.storage(), payload);
            drop(value);
            budget.release_storage(payload).unwrap();
        } else {
            assert!(matches!(
                value,
                Err(Failure::Policy(
                    ProductionCanonicalRankedPolicyErrorV1::Source(
                        ProductionCanonicalRankedSourceErrorV1::Resource(
                            ArgumentResourceV1::Storage(error)
                        )
                    )
                )) if error.actual() == payload && error.limit() == payload - 1
            ));
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.failed_storage(), Some(payload));
        }
        assert_eq!(budget.work(), 0);
    }
}
#[test]
fn assertion_scope_headers_are_paid_before_callback_exact_and_one_short() {
    let header = size_of::<Cleanup<'_, '_>>() + size_of::<std::thread::Result<R<()>>>();
    for shortage in [0, 1] {
        let mut work = Work::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, header - shortage);
        let entered = Cell::new(false);
        let result = scoped(&mut budget, |budget| {
            entered.set(true);
            assert_eq!(budget.storage(), header);
            Ok(())
        });
        assert_eq!(entered.get(), shortage == 0);
        assert_eq!(result.is_ok(), shortage == 0);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.work(), 0);
    }
}
#[test]
fn assertion_scope_failure_and_panic_drop_payload_before_refund() {
    struct Probe<'a, 'w> {
        drops: &'a Cell<usize>,
        budget: &'a ArgumentBudgetV1<'w>,
        paid: usize,
    }
    impl Drop for Probe<'_, '_> {
        fn drop(&mut self) {
            assert_eq!(self.budget.storage(), self.paid);
            self.drops.set(self.drops.get() + 1);
        }
    }
    for panic in [false, true] {
        let drops = Cell::new(0);
        let mut work = Work::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
        budget.reserve_storage(17).unwrap();
        let result: R<()> = scoped(&mut budget, |budget| {
            budget.reserve_storage(size_of::<Probe<'_, '_>>())?;
            let _probe = Probe {
                drops: &drops,
                paid: budget.storage(),
                budget,
            };
            if panic {
                panic!("paid scope unwind");
            }
            Err(binding(None, "scope refusal"))
        });
        assert!(result.is_err());
        assert_eq!(drops.get(), 1);
        assert_eq!(budget.storage(), 17);
    }
}
#[test]
fn actual_sparse_boolean_veto_does_not_treat_unknown_as_proof() {
    // Component semantics only: success here is absence of a veto, not Fact
    // construction. The public transaction independently requires a source Fact.
    for value in [
        CanonicalKirSparseValueV1::Unknown,
        CanonicalKirSparseValueV1::Dynamic,
        CanonicalKirSparseValueV1::Unreachable,
    ] {
        sparse_veto(value, true, 0).unwrap();
    }
}
#[test]
fn protected_scope_never_refunds_a_substituted_foreign_ledger() {
    let mut work = Work::new(100);
    let mut other = Work::new(100);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
    let mut foreign = ArgumentBudgetV1::new(&mut other, 4096);
    budget.reserve_storage(17).unwrap();
    foreign.reserve_storage(23).unwrap();
    let result: R<()> = scoped(&mut budget, |budget| {
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Failure::Policy(
            ProductionCanonicalRankedPolicyErrorV1::Source(
                ProductionCanonicalRankedSourceErrorV1::Resource(ArgumentResourceV1::Accounting)
            )
        ))
    ));
    assert_eq!(budget.storage(), 23);
    assert!(foreign.storage() > 17);
}

#[test]
fn assertion_nested_scopes_restore_original_ledger_during_recursive_payload_drop() {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind, panic_any, resume_unwind},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    struct Trace {
        next: AtomicUsize,
        counts: [AtomicUsize; 3],
        order: [AtomicUsize; 3],
    }
    struct Payload {
        trace: Arc<Trace>,
        token: usize,
        generation: usize,
        terminal: usize,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.trace.counts[self.generation].fetch_add(1, Ordering::SeqCst);
            let order = self.trace.next.fetch_add(1, Ordering::SeqCst) + 1;
            self.trace.order[self.generation].store(order, Ordering::SeqCst);
            if self.generation < self.terminal {
                panic_any(Payload {
                    trace: Arc::clone(&self.trace),
                    token: self.token,
                    generation: self.generation + 1,
                    terminal: self.terminal,
                });
            }
        }
    }
    struct PaidProbe<'a, 'w> {
        budget: &'a ArgumentBudgetV1<'w>,
        storage: &'a Cell<usize>,
        drops: &'a Cell<usize>,
    }
    impl Drop for PaidProbe<'_, '_> {
        fn drop(&mut self) {
            self.storage.set(self.budget.storage());
            self.drops.set(self.drops.get() + 1);
        }
    }

    const CALLER: usize = 17;
    const SIBLING: usize = 23;
    const OUTER_SCRATCH: usize = 19;
    const INNER_SCRATCH: usize = 23;
    let header = size_of::<Cleanup<'_, '_>>() + size_of::<std::thread::Result<R<()>>>();
    let probe = size_of::<PaidProbe<'_, '_>>();
    // Panic payloads are caller-owned; neither wrapper may refund their floor.
    let floor = CALLER + SIBLING + 3 * size_of::<Payload>();
    let outer_paid = floor + header + OUTER_SCRATCH;
    let peak = outer_paid + header + INNER_SCRATCH + probe;

    for terminal in 0..=2 {
        let trace = Arc::new(Trace {
            next: AtomicUsize::new(0),
            counts: std::array::from_fn(|_| AtomicUsize::new(0)),
            order: std::array::from_fn(|_| AtomicUsize::new(0)),
        });
        let token = 0x271_b11 + terminal;
        let entered = Cell::new(None);
        let intermediate = Cell::new(None);
        let intermediate_payload = Cell::new(false);
        let probe_storage = Cell::new(0);
        let probe_drops = Cell::new(0);
        let mut work = Work::new(10);
        let mut budget = ArgumentBudgetV1::new(&mut work, peak);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(5).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&budget) as usize;

        let outcome: std::thread::Result<R<()>> = catch_unwind(AssertUnwindSafe(|| {
            scoped(&mut budget, |budget| {
                budget.reserve_storage(OUTER_SCRATCH)?;
                budget.charge_work(2)?;
                let outer_scratch = [0x31_u8; OUTER_SCRATCH];
                let inner: std::thread::Result<R<()>> = catch_unwind(AssertUnwindSafe(|| {
                    scoped(budget, |budget| {
                        budget.reserve_storage(INNER_SCRATCH)?;
                        budget.reserve_storage(probe)?;
                        budget.charge_work(3)?;
                        let inner_scratch = [0x42_u8; INNER_SCRATCH];
                        entered.set(Some((
                            budget.storage(),
                            std::ptr::from_ref(&*budget) as usize == slot
                                && budget.work_ledger_identity_v1() == ledger,
                        )));
                        let _probe = PaidProbe {
                            budget,
                            storage: &probe_storage,
                            drops: &probe_drops,
                        };
                        std::hint::black_box(inner_scratch);
                        panic_any(Payload {
                            trace: Arc::clone(&trace),
                            token,
                            generation: 0,
                            terminal,
                        });
                    })
                }));
                // Observe the inner refund before the outer guard can mask it.
                intermediate.set(Some((
                    budget.storage(),
                    std::ptr::from_ref(&*budget) as usize == slot
                        && budget.work_ledger_identity_v1() == ledger,
                )));
                std::hint::black_box(outer_scratch);
                match inner {
                    Ok(result) => result,
                    Err(payload) => {
                        intermediate_payload.set(payload.downcast_ref::<Payload>().is_some_and(
                            |value| {
                                Arc::ptr_eq(&value.trace, &trace)
                                    && value.token == token
                                    && value.generation == 1
                                    && value.terminal == terminal
                            },
                        ));
                        // Resume this exact payload; do not replace its destructor panic.
                        resume_unwind(payload)
                    }
                }
            })
        }));

        // Assertions are outside the production catches so failures stay failures.
        assert_eq!(entered.get(), Some((peak, true)));
        assert_eq!(intermediate.get(), Some((outer_paid, true)));
        assert_eq!(intermediate_payload.get(), terminal > 0);
        assert_eq!(probe_drops.get(), 1);
        assert_eq!(probe_storage.get(), peak);
        assert_eq!(std::ptr::from_ref(&budget) as usize, slot);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), peak);
        assert_eq!(budget.work(), 10);
        assert_eq!(budget.failed_storage(), None);
        match outcome {
            Ok(result) => {
                assert!(terminal < 2);
                assert!(matches!(result, Err(Failure::Panicked)));
            }
            Err(payload) => {
                assert_eq!(terminal, 2);
                let value = payload.downcast_ref::<Payload>().expect("terminal payload");
                assert!(Arc::ptr_eq(&value.trace, &trace));
                assert_eq!(
                    (value.token, value.generation, value.terminal),
                    (token, 2, 2)
                );
                assert_eq!(trace.counts[2].load(Ordering::SeqCst), 0);
                // Both real Cleanup::drop paths have run before terminal retirement.
                drop(payload);
            }
        }
        for generation in 0..3 {
            assert_eq!(
                trace.counts[generation].load(Ordering::SeqCst),
                usize::from(generation <= terminal)
            );
            assert_eq!(
                trace.order[generation].load(Ordering::SeqCst),
                if generation <= terminal {
                    generation + 1
                } else {
                    0
                }
            );
        }
        assert_eq!(trace.next.load(Ordering::SeqCst), terminal + 1);
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor - SIBLING).unwrap();
        assert_eq!(budget.storage(), SIBLING);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}
