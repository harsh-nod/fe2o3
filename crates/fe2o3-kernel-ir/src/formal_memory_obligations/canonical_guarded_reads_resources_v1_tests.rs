use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn headers<T>() -> usize {
    size_of::<Facts<'_>>()
        + size_of::<Accounting>()
        + size_of::<CheckedCanonicalGuardedGlobalReadsV1<'_, '_>>()
        + size_of::<std::thread::Result<Result<T>>>()
        + size_of::<std::thread::Result<Result<T>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<&CanonicalGuardedGlobalReadFactV1<'_, '_>>()
        + size_of::<CanonicalGuardedReadIndexOriginV1>()
        + size_of::<ValueId>()
        + size_of::<(ValueId, ValueId)>()
}

#[test]
fn entry_header_exact_and_one_short_are_independent_first_cuts() {
    let graph = owner(fixture());
    let header = headers::<()>();
    for limit in [header - 1, header] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = Budget::new(&mut work, limit);
        let mut calls = 0;
        let result = with_canonical_guarded_global_reads_v1(
            &graph,
            Default::default(),
            &mut budget,
            |_, _| {
                calls += 1;
                Ok(())
            },
        );
        assert!(matches!(
            result,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
        assert_eq!(calls, 0);
        assert_eq!(budget.storage(), 0);
        if limit < header {
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.peak_storage(), 0);
            assert_eq!(budget.failed_storage(), Some(header));
        } else {
            assert_eq!(budget.work(), 1);
            assert_eq!(budget.peak_storage(), header);
            assert_eq!(
                budget.failed_storage(),
                Some(header + size_of::<FunctionFacts<'_>>())
            );
        }
    }
}

#[test]
fn initial_work_denial_retains_paid_header_prefix_only() {
    let graph = owner(fixture());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, 1 << 20);
    let result =
        with_canonical_guarded_global_reads_v1(&graph, Default::default(), &mut budget, |_, _| {
            Ok(())
        });
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Work(_)))
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.peak_storage(), headers::<()>());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_derived_empty_origin_construction_work_is_forty() {
    // Exactly twenty reserve calls, each charging2. All loops/sorts/fills are
    // empty; only the explicit DFS stack reserves one tuple. No SUT preflight.
    for limit in [39, 40] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 1 << 20);
        let result = {
            let mut meter = LiveGuardMeter::new(&mut budget, 1000, 1 << 20, 1000);
            origins::resolve(&mut meter, &[], &[])
        };
        if limit == 39 {
            assert!(matches!(result, Err(ResourceError::Work(_))));
            assert_eq!(budget.work(), 38);
        } else {
            assert_eq!(result.unwrap(), Vec::<Option<ValueId>>::new());
            assert_eq!(budget.work(), 40);
        }
        // This component intentionally leaves scratch credit with its caller;
        // the actual public scope drops/refunds the complete paid candidate.
        assert_eq!(
            budget.peak_storage(),
            20 * size_of::<Vec<()>>() + size_of::<(usize, usize)>()
        );
        budget.release_storage(budget.storage()).unwrap();
    }
}

#[test]
fn local_work_denial_precedes_external_debit() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = Budget::new(&mut work, 100);
    let mut meter = LiveGuardMeter::new(&mut budget, 0, 100, 100);
    assert!(matches!(meter.charge(1), Err(ResourceError::Work(_))));
    assert_eq!(meter.budget.work(), 0);
}

#[test]
fn public_nested_payload_destructors_complete_before_floor_return() {
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
    let graph = owner(fixture());
    let drops = Arc::new(AtomicUsize::new(0));
    let result: Result<()> = run(&graph, |view, budget| {
        assert!(matches!(
            view.read_at(coordinate(1, 2), budget)?,
            CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(_)
        ));
        std::panic::panic_any(Payload {
            remaining: 2,
            drops: drops.clone(),
        });
    });
    assert!(matches!(result, Err(Failure::Panicked)));
    assert_eq!(drops.load(Ordering::SeqCst), 3);
}

#[test]
fn callback_scratch_is_allowed_but_leaked_reservations_are_not() {
    let graph = owner(fixture());
    run(&graph, |view, budget| {
        budget.reserve_storage(31)?;
        assert_eq!(view.function_count(budget)?, 1);
        budget.release_storage(31)?;
        Ok(())
    })
    .unwrap();
    let result = run(&graph, |view, budget| {
        budget.reserve_storage(31)?;
        assert_eq!(view.function_count(budget)?, 1);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Failure::Resource(ResourceError::Accounting))
    ));
}

#[test]
fn no_selected_read_census_uses_one_source_derived_local_work_limit() {
    for operations in [0_u32, 3] {
        let mut module = fixture();
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks.truncate(1);
        body.blocks[0].operations = (0..operations)
            .map(|ordinal| {
                op(
                    4 + ordinal,
                    Type::BOOL,
                    OperationKind::Constant(Constant::Bool(true)),
                )
            })
            .collect();
        body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
        let graph = owner(module);
        // Selection: 32 + 2 per block + 2 per operation. Census: 1 per
        // block + 2 per effect-free operation. No read/effect rows are built.
        let exact = 35 + 4 * operations as usize;
        for limit in [exact - 1, exact] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut budget = Budget::new(&mut work, 1 << 20);
            budget.reserve_storage(17).unwrap();
            let mut callbacks = 0;
            let result = with_canonical_guarded_global_reads_v1(
                &graph,
                CanonicalGuardedGlobalReadLimitsV1 {
                    per_function_work: limit,
                    ..Default::default()
                },
                &mut budget,
                |view, budget| {
                    callbacks += 1;
                    assert_eq!(
                        view.function_effects(FunctionCoordinate(0), budget)?,
                        (0, 0, 0)
                    );
                    Ok(())
                },
            );
            if limit == exact {
                result.unwrap();
                assert_eq!(callbacks, 1);
            } else {
                let Err(Failure::Resource(ResourceError::Work(error))) = result else {
                    panic!("the combined local selection/census meter must deny");
                };
                assert_eq!(error.actual(), exact);
                assert_eq!(error.limit(), exact - 1);
                assert_eq!(callbacks, 0);
            }
            assert_eq!(budget.storage(), 17);
            // The local denial happens before the external ledger debit.
            assert_eq!(work.failed_work(), None);
        }
    }
}

#[test]
fn unselected_owned_meter_header_has_an_independent_local_byte_cut() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks.truncate(1);
    body.blocks[0].operations.clear();
    body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
    let graph = owner(module);
    let header = size_of::<GuardedControlCollectionV1<LiveGuardMeter<'_, '_>>>();
    // The unselected meter header is followed by the reused effect-census
    // row and (conditions, reason) carriers, even when no read rows are built.
    // The enclosing getter bundle is external credit, not this local cap.
    let collector = size_of::<ReadRow>()
        + size_of::<(
            Option<runtime_slice_read_v1::RuntimeSliceReadConditionsV1>,
            CanonicalGuardedGlobalReadReasonV1,
        )>();
    let exact = header + collector;
    for (limit, attempted) in [
        (header - 1, Some(header)),
        (header, Some(exact)),
        (exact - 1, Some(exact)),
        (exact, None),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1 << 20);
        budget.reserve_storage(17).unwrap();
        let mut callbacks = 0;
        let result = with_canonical_guarded_global_reads_v1(
            &graph,
            CanonicalGuardedGlobalReadLimitsV1 {
                per_function_work: 35,
                per_function_new_bytes: limit,
                ..Default::default()
            },
            &mut budget,
            |_, _| {
                callbacks += 1;
                Ok(())
            },
        );
        if let Some(actual) = attempted {
            assert_eq!(
                result,
                Err(Failure::Resource(ResourceError::Storage {
                    actual,
                    limit,
                }))
            );
            assert_eq!(callbacks, 0);
        } else {
            result.unwrap();
            assert_eq!(callbacks, 1);
        }
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn first_query_failure_survives_foreign_budget_then_nested_callback_panic() {
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
    let graph = owner(fixture());
    for coordinate_first in [false, true] {
        for nested in [1, 2] {
            let drops = Arc::new(AtomicUsize::new(0));
            let expected = if coordinate_first {
                Failure::Coordinate(coordinate(99, 99))
            } else {
                Failure::Resource(ResourceError::Accounting)
            };
            let result: Result<()> = run(&graph, |view, budget| {
                assert!(std::ptr::eq(view.owner(budget)?, &graph));
                let mut payload = None;
                for _ in 0..=nested {
                    budget.reserve_storage(size_of::<Payload>() + 5)?;
                    let backing = vec![0_u8; 5];
                    budget.reserve_storage(backing.capacity() - 5)?;
                    payload = Some(Box::new(Payload {
                        next: payload,
                        backing,
                        drops: drops.clone(),
                    }));
                }
                if coordinate_first {
                    assert_eq!(
                        view.read_at(coordinate(99, 99), budget).err().unwrap(),
                        expected
                    );
                }
                let before = budget.work();
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100);
                let mut foreign = Budget::new(&mut foreign_work, 100);
                foreign.reserve_storage(7).unwrap();
                assert!(matches!(
                    view.owner(&mut foreign),
                    Err(Failure::Resource(ResourceError::Accounting))
                ));
                assert_eq!(foreign.work(), 0);
                assert_eq!(foreign.storage(), 7);
                assert_eq!(budget.work(), before);
                std::panic::panic_any(payload.unwrap());
            });
            assert_eq!(result, Err(expected));
            assert_eq!(drops.load(Ordering::SeqCst), nested + 1);
        }
    }
}
