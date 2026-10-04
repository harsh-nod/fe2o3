use super::*;
use crate::{PresburgerAffineExprV1 as Expr, PresburgerBoxV1 as BoxV1};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const FLOOR: usize = 73;
const INPUT: usize = 80_000;
const LIMIT: usize = 2_000_000;
thread_local! { static OBSERVED: Cell<usize> = const { Cell::new(0) }; }

fn singleton() -> PresburgerSetV1 {
    PresburgerSetV1::box_only(BoxV1::new(vec![0, 0], vec![1, 1]).unwrap())
}
fn owned_input(bytes: usize, budget: &mut Budget<'_>) -> Vec<u8> {
    budget.charge_work(bytes + 3).unwrap();
    budget.reserve_storage(bytes).unwrap();
    let mut backing = Vec::new();
    backing.try_reserve_exact(bytes).unwrap();
    budget.reserve_storage(backing.capacity() - bytes).unwrap();
    backing.resize(bytes, 7);
    backing
}
fn drop_input(backing: Vec<u8>, budget: &mut Budget<'_>) {
    let bytes = backing.capacity();
    drop(backing);
    budget.release_storage(bytes).unwrap();
}
fn query(
    scope: &mut PresburgerQueryScopeV4<'_>,
    set: &PresburgerSetV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scope.with_witness(set, budget, |decision, _| {
        assert!(matches!(decision, PresburgerQueryDecisionV2::Witness(point) if point == [0, 0]));
        Ok(())
    })
}
fn session_observer(_: &mut PresburgerQueryScopeV4<'_>, budget: &mut Budget<'_>) -> Result<()> {
    OBSERVED.with(|value| value.set(budget.storage()));
    Ok(())
}

#[test]
fn owned_session_headers_have_independent_exact_short_storage_and_local_caps() {
    type Consumer = fn(&mut PresburgerQueryScopeV4<'_>, &mut Budget<'_>) -> Result<()>;
    let headers = 2 * size_of::<std::thread::Result<Result<()>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<PresburgerQueryScopeV4<'_>>()
        + size_of::<Option<Consumer>>()
        + size_of::<Credit>()
        + size_of::<&mut Option<Consumer>>()
        + 2 * size_of::<(
            &mut PresburgerQueryScopeV4<'_>,
            &mut Budget<'_>,
            &mut Credit,
            &mut (),
            usize,
        )>();
    for (storage_short, local_short) in [(false, false), (true, false), (false, true)] {
        OBSERVED.with(|value| value.set(0));
        let mut work = Work::new(4);
        let mut budget = Budget::new(&mut work, FLOOR + headers - usize::from(storage_short));
        budget.reserve_storage(FLOOR).unwrap();
        let consume: Consumer = session_observer;
        let result = with_presburger_queries_v4(
            PresburgerQueryLimitsV2 {
                scratch_bytes: headers - usize::from(local_short),
                ..Default::default()
            },
            &mut budget,
            consume,
        );
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.storage(), FLOOR);
        if storage_short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == FLOOR + headers && error.limit() == FLOOR + headers - 1)
            );
        } else if local_short {
            assert_eq!(
                result,
                Err(Error::Limit {
                    resource: PresburgerQueryResourceV2::Scratch,
                    actual: headers,
                    limit: headers - 1
                })
            );
        } else {
            result.unwrap();
        }
        OBSERVED.with(|value| {
            assert_eq!(
                value.get(),
                if storage_short || local_short {
                    0
                } else {
                    FLOOR + headers
                }
            )
        });
    }
}

fn unit_consume(_: &mut Budget<'_>) -> Result<()> {
    Ok(())
}
fn unit_build(_: &mut OwnedMeter<'_, '_>, _: &mut Budget<'_>) -> Result<()> {
    Ok(())
}
fn unit_visit(
    _: &(),
    consume: fn(&mut Budget<'_>) -> Result<()>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    consume(budget)?;
    OBSERVED.with(|value| value.set(budget.storage()));
    Ok(())
}

#[test]
fn owned_query_headers_have_independent_exact_and_one_short_cuts() {
    type C = fn(&mut Budget<'_>) -> Result<()>;
    type B = fn(&mut OwnedMeter<'_, '_>, &mut Budget<'_>) -> Result<()>;
    type V = fn(&(), C, &mut Budget<'_>) -> Result<()>;
    let headers = 2 * size_of::<std::thread::Result<Result<()>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<Option<C>>()
        + size_of::<Option<B>>()
        + size_of::<Option<V>>()
        + size_of::<()>()
        + size_of::<Result<()>>()
        + size_of::<Bounds>()
        + size_of::<PresburgerQueryDecisionV2<'_>>()
        + size_of::<PresburgerAffineNarrowingDecisionV3<'_>>()
        + size_of::<Credit>()
        + size_of::<OwnedMeter<'_, '_>>()
        + size_of::<SearchMeter<'_, '_, '_>>()
        + size_of::<(
            &PresburgerSetV1,
            &mut Option<C>,
            &mut Option<B>,
            &mut Option<V>,
        )>()
        + 2 * size_of::<(
            &mut PresburgerQueryScopeV4<'_>,
            &mut Budget<'_>,
            &mut Credit,
            &mut (),
            usize,
        )>();
    for short in [false, true] {
        let set = singleton();
        let completed = Cell::new(false);
        OBSERVED.with(|value| value.set(0));
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result = with_presburger_queries_v4(
            Default::default(),
            &mut budget,
            |scope, budget| {
                let floor = budget.storage();
                let padding = LIMIT - floor - headers + usize::from(short);
                budget.reserve_storage(padding).unwrap();
                let paid = budget.storage();
                let consume: C = unit_consume;
                let build: B = unit_build;
                let visit: V = unit_visit;
                let result = scope.query(&set, budget, consume, build, visit);
                assert_eq!(budget.storage(), paid);
                if short {
                    assert!(
                        matches!(&result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == LIMIT + 1 && error.limit() == LIMIT)
                    );
                    OBSERVED.with(|value| assert_eq!(value.get(), 0));
                } else {
                    assert!(result.is_ok());
                    OBSERVED.with(|value| assert_eq!(value.get(), LIMIT));
                }
                budget.release_storage(padding).unwrap();
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                result
            },
        );
        assert!(completed.get());
        assert_eq!(result.is_ok(), !short);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn owned_search_and_narrowing_preserve_large_paid_input_and_decisions() {
    let set = singleton();
    let narrow = PresburgerSetV1::new(
        BoxV1::new(vec![0], vec![1_i128 << 64]).unwrap(),
        vec![PresburgerConstraintV1::LessEqualZero(
            Expr::new(0, vec![1]).unwrap(),
        )],
    )
    .unwrap();
    let completed = Cell::new(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
        let session = budget.storage();
        for bytes in [INPUT, INPUT + 4096, INPUT] {
            let backing = owned_input(bytes, budget);
            let retained = budget.storage();
            query(scope, &set, budget)?;
            scope.with_affine_narrowing_v4(&narrow, budget, |decision, _| {
                assert!(matches!(decision, PresburgerAffineNarrowingDecisionV3::Bounds { lower, upper_exclusive } if lower == [0] && upper_exclusive == [1]));
                Ok(())
            })?;
            assert_eq!(budget.storage(), retained);
            assert_eq!(backing[bytes - 1], 7);
            drop_input(backing, budget);
            assert_eq!(budget.storage(), session);
        }
        let usage = scope.usage(budget)?;
        assert_eq!(usage.queries, 6);
        assert!(usage.peak_scratch_bytes < INPUT);
        completed.set(true);
        Ok(())
    }).unwrap();
    assert!(completed.get());
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn old_v2_local_scratch_policy_is_unchanged() {
    let set = singleton();
    let completed = Cell::new(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let error = with_presburger_queries_v2(Default::default(), &mut budget, |scope, budget| {
        let backing = owned_input(INPUT, budget);
        let result = scope.with_witness(&set, budget, |_, _| Ok(()));
        assert!(matches!(&result, Err(Error::Limit { resource: PresburgerQueryResourceV2::Scratch, actual, limit: 65_536 }) if *actual > INPUT));
        // V2's historical query refund leaves its own entry's borrowed backing.
        drop_input(backing, budget);
        completed.set(true);
        result
    }).unwrap_err();
    assert!(completed.get());
    assert!(matches!(
        error,
        Error::Limit {
            resource: PresburgerQueryResourceV2::Scratch,
            ..
        }
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn owned_repeated_queries_have_independent_exact_and_one_short_cumulative_work() {
    // Rank-two singleton: old entry/validation/search cost32, new entry adds4.
    const EXACT: usize = INPUT + 3 + 4 + 2 * 36;
    for short in [false, true] {
        let set = singleton();
        let completed = Cell::new(false);
        let mut work = Work::new(EXACT - usize::from(short));
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_presburger_queries_v4(
            Default::default(),
            &mut budget,
            |scope, budget| {
                let session = budget.storage();
                let backing = owned_input(INPUT, budget);
                query(scope, &set, budget)?;
                let result = query(scope, &set, budget);
                if short {
                    assert!(
                        matches!(&result, Err(Error::Resource(Resource::Work(error))) if error.actual() == EXACT && error.limit() == EXACT - 1)
                    );
                    assert_eq!(query(scope, &set, budget), result);
                } else {
                    assert!(result.is_ok());
                    assert_eq!(scope.state.work, 76);
                    assert_eq!(scope.state.queries, 2);
                }
                drop_input(backing, budget);
                assert_eq!(budget.storage(), session);
                completed.set(true);
                result
            },
        );
        assert!(completed.get());
        assert_eq!(result.is_ok(), !short);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn owned_query_count_and_work_caps_remain_cumulative_across_inputs() {
    for queries in [true, false] {
        let completed = Cell::new(false);
        let set = singleton();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let limits = PresburgerQueryLimitsV2 {
            queries: if queries { 1 } else { 2048 },
            work: if queries { LIMIT } else { 40 },
            ..Default::default()
        };
        let error = with_presburger_queries_v4(limits, &mut budget, |scope, budget| {
            let first = owned_input(INPUT, budget);
            query(scope, &set, budget)?;
            drop_input(first, budget);
            let second = owned_input(INPUT + 3, budget);
            let result = query(scope, &set, budget);
            assert_eq!(
                result,
                Err(Error::Limit {
                    resource: if queries {
                        PresburgerQueryResourceV2::Queries
                    } else {
                        PresburgerQueryResourceV2::Work
                    },
                    actual: if queries { 2 } else { 48 },
                    limit: if queries { 1 } else { 40 }
                })
            );
            assert_eq!(query(scope, &set, budget), result);
            drop_input(second, budget);
            completed.set(true);
            result
        })
        .unwrap_err();
        assert!(completed.get());
        assert!(matches!(error, Error::Limit { .. }));
        assert_eq!(budget.storage(), 0);
    }
}

fn measured(storage: usize) -> (Result<()>, usize, usize, bool) {
    let set = singleton();
    let completed = Cell::new(false);
    let attempted = Cell::new(false);
    let settled = Cell::new(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
        let backing = owned_input(INPUT, budget);
        attempted.set(true);
        let result = query(scope, &set, budget);
        drop_input(backing, budget);
        completed.set(result.is_ok());
        settled.set(true);
        result
    });
    assert!(attempted.get());
    assert!(settled.get());
    assert_eq!(budget.storage(), FLOOR);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        completed.get(),
    )
}
#[test]
fn owned_real_search_has_measured_exact_and_one_short_peak() {
    let (result, work, peak, completed) = measured(LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, exact_work, exact_peak, completed) = measured(peak);
    result.unwrap();
    assert!(completed);
    assert_eq!((exact_work, exact_peak), (work, peak));
    let (result, _, _, completed) = measured(peak - 1);
    assert!(!completed);
    assert!(
        matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.limit() == peak - 1)
    );
}

#[repr(align(64))]
struct PanicCapture<'a>(&'a Cell<usize>, [u8; 128]);
impl PanicCapture<'_> {
    fn touch(&self) -> u8 {
        self.1[0]
    }
}
impl Drop for PanicCapture<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        panic!("owned capture drop");
    }
}

#[test]
fn owned_entry_prior_denial_and_header_failure_drain_the_whole_capture() {
    for prior in [false, true] {
        let dropped = Cell::new(0);
        let entered = Cell::new(false);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, if prior { LIMIT } else { FLOOR });
        budget.reserve_storage(FLOOR).unwrap();
        if prior {
            assert!(budget.charge_work(LIMIT + 1).is_err());
        }
        let capture = PanicCapture(&dropped, [7; 128]);
        let entered_ref = &entered;
        let result = with_presburger_queries_v4(Default::default(), &mut budget, move |_, _| {
            entered_ref.set(capture.touch() == 7);
            Ok(())
        });
        assert!(!entered.get());
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), FLOOR);
        if prior {
            assert_eq!(
                result,
                Err(Error::PriorDenial {
                    work: Some(LIMIT + 1),
                    storage: None
                })
            );
        } else {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.limit() == FLOOR)
            );
        }
    }
}

#[test]
fn owned_session_entry_work_has_exact_four_and_one_short_boundary() {
    for short in [false, true] {
        OBSERVED.with(|value| value.set(0));
        let mut work = Work::new(4 - usize::from(short));
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let consume: fn(&mut PresburgerQueryScopeV4<'_>, &mut Budget<'_>) -> Result<()> =
            session_observer;
        let result = with_presburger_queries_v4(Default::default(), &mut budget, consume);
        if short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Work(error))) if error.actual() == 4 && error.limit() == 3)
            );
            OBSERVED.with(|value| assert_eq!(value.get(), 0));
        } else {
            result.unwrap();
            OBSERVED.with(|value| assert!(value.get() > FLOOR));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn owned_session_error_preserves_new_caller_backing_until_its_owner_drops() {
    let mut backing = None;
    let completed = Cell::new(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_presburger_queries_v4(Default::default(), &mut budget, |_, budget| {
        backing = Some(owned_input(INPUT, budget));
        completed.set(true);
        Err::<(), _>(Error::CallbackRejected)
    });
    assert!(completed.get());
    assert_eq!(result, Err(Error::CallbackRejected));
    let backing = backing.unwrap();
    assert_eq!(backing[INPUT - 1], 7);
    assert_eq!(budget.storage(), FLOOR + backing.capacity());
    drop_input(backing, &mut budget);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn owned_query_header_failure_drains_capture_and_preserves_paid_input() {
    let dropped = Cell::new(0);
    let completed = Cell::new(false);
    let set = singleton();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let result = with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
        let session = budget.storage();
        let backing = owned_input(INPUT, budget);
        let padding = LIMIT - budget.storage();
        budget.reserve_storage(padding).unwrap();
        let capture = PanicCapture(&dropped, [9; 128]);
        let result = scope.with_witness(&set, budget, move |_, _| {
            assert_eq!(capture.touch(), 9);
            Ok(())
        });
        assert!(
            matches!(&result, Err(Error::Resource(Resource::Storage(error))) if error.limit() == LIMIT)
        );
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), LIMIT);
        assert_eq!(query(scope, &set, budget), result);
        budget.release_storage(padding).unwrap();
        drop_input(backing, budget);
        assert_eq!(budget.storage(), session);
        completed.set(true);
        result
    });
    assert!(completed.get());
    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn owned_callback_error_panic_and_panicking_result_keep_first_failure() {
    for mode in 0..3 {
        let completed = Cell::new(false);
        let dropped = Cell::new(0);
        let set = singleton();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result =
            with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
                let floor = budget.storage();
                let result = match mode {
                    0 => scope
                        .with_witness(&set, budget, |_, _| Err::<(), _>(Error::CallbackRejected)),
                    1 => scope.with_witness(&set, budget, |_, _| -> Result<()> {
                        panic!("callback panic")
                    }),
                    _ => scope
                        .with_witness(&set, budget, |_, budget| {
                            assert!(budget.charge_work(LIMIT + 1).is_err());
                            Ok(PanicCapture(&dropped, [0; 128]))
                        })
                        .map(drop),
                };
                assert_eq!(budget.storage(), floor);
                match mode {
                    0 => assert_eq!(result, Err(Error::CallbackRejected)),
                    1 => assert_eq!(result, Err(Error::Panicked)),
                    _ => {
                        assert!(matches!(
                            result,
                            Err(Error::PriorDenial {
                                work: Some(_),
                                storage: None
                            })
                        ));
                        assert_eq!(dropped.get(), 1);
                    }
                }
                assert_eq!(query(scope, &set, budget), result);
                completed.set(true);
                result
            });
        assert!(completed.get());
        assert!(result.is_err());
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn owned_surplus_is_refused_and_never_refunded_as_solver_storage() {
    for query_scope in [false, true] {
        let completed = Cell::new(false);
        let set = singleton();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
                let floor = budget.storage();
                if query_scope {
                    let result = scope.with_witness(&set, budget, |_, budget| {
                        budget.reserve_storage(31)?;
                        Ok(())
                    });
                    assert_eq!(result, Err(Resource::Accounting.into()));
                    assert_eq!(budget.storage(), floor + 31);
                    completed.set(true);
                    result
                } else {
                    budget.reserve_storage(31)?;
                    completed.set(true);
                    Ok(())
                }
            });
        assert!(completed.get());
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), FLOOR + 31);
        budget.release_storage(31).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn owned_foreign_slot_query_conservatively_suppresses_containing_refunds() {
    let completed = Cell::new(false);
    let retained = Cell::new(0);
    let set = singleton();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let result = with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
        let floor = budget.storage();
        let mut foreign_work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut foreign_work, LIMIT);
        let result = query(scope, &set, &mut foreign);
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(foreign.storage(), 0);
        assert_eq!(budget.storage(), floor);
        assert_eq!(query(scope, &set, budget), result);
        retained.set(floor);
        completed.set(true);
        result
    });
    assert!(completed.get());
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert_eq!(budget.storage(), retained.get());
    assert!(retained.get() > 0);
}

#[test]
fn owned_same_slot_foreign_ledger_stays_failed_after_original_ledger_restore() {
    let completed = Cell::new(false);
    let retained = Cell::new(0);
    let set = singleton();
    let mut work = Work::new(LIMIT);
    let mut other_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let result = with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
        let floor = budget.storage();
        let original = budget.work_ledger_identity_v1();
        std::mem::swap(budget, &mut other);
        assert!(budget.work_ledger_identity_v1() != original);
        let result = query(scope, &set, budget);
        assert_eq!(result, Err(Resource::Accounting.into()));
        std::mem::swap(budget, &mut other);
        assert!(budget.work_ledger_identity_v1() == original);
        assert_eq!(budget.storage(), floor);
        assert_eq!(query(scope, &set, budget), result);
        retained.set(floor);
        completed.set(true);
        result
    });
    assert!(completed.get());
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert_eq!(budget.storage(), retained.get());
    assert!(retained.get() > 0);
    assert_eq!(other.storage(), 0);
}

#[test]
fn owned_query_undercut_keeps_prior_denial_and_blocks_containing_refunds() {
    for prior in [false, true] {
        let completed = Cell::new(false);
        let inner = Cell::new(false);
        let remaining = Cell::new(0);
        let expected = Cell::new(None);
        let set = singleton();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result =
            with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
                let result = scope.with_witness(&set, budget, |_, budget| {
                    if prior {
                        assert!(budget.charge_work(LIMIT + 1).is_err());
                        expected.set(budget.failed_work());
                    }
                    let before = budget.storage();
                    budget.release_storage(1).unwrap();
                    assert_eq!(budget.storage(), before - 1);
                    inner.set(true);
                    Ok(())
                });
                assert!(inner.get());
                assert_eq!(
                    result,
                    if prior {
                        Err(Error::PriorDenial {
                            work: expected.get(),
                            storage: None,
                        })
                    } else {
                        Err(Resource::Accounting.into())
                    }
                );
                assert_eq!(query(scope, &set, budget), result);
                remaining.set(budget.storage());
                completed.set(true);
                result
            });
        assert!(completed.get());
        assert!(result.is_err());
        assert_eq!(budget.storage(), remaining.get());
        assert!(remaining.get() > 0);
    }
}

#[test]
fn owned_model_overflow_and_empty_decisions_match_the_shared_algorithms() {
    let empty = PresburgerSetV1::box_only(BoxV1::new(vec![1], vec![1]).unwrap());
    let overflow = PresburgerSetV1::new(
        BoxV1::new(vec![2], vec![3]).unwrap(),
        vec![PresburgerConstraintV1::LessEqualZero(
            Expr::new(0, vec![i128::MAX]).unwrap(),
        )],
    )
    .unwrap();
    let completed = Cell::new(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let result = with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
        scope.with_witness(&empty, budget, |decision, _| {
            assert_eq!(decision, PresburgerQueryDecisionV2::Empty);
            Ok(())
        })?;
        let error = scope.with_affine_narrowing_v4(&overflow, budget, |_, _| {
            panic!("overflow must not become an observation")
        });
        assert_eq!(
            error,
            Err::<(), _>(Error::Model(PresburgerFailureV1::ArithmeticOverflow))
        );
        assert_eq!(scope.with_witness(&empty, budget, |_, _| Ok(())), error);
        completed.set(true);
        error
    });
    assert!(completed.get());
    assert_eq!(
        result,
        Err(Error::Model(PresburgerFailureV1::ArithmeticOverflow))
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn owned_usage_foreign_slot_and_ledger_restore_cannot_restore_refund_permission() {
    for same_slot in [false, true] {
        let completed = Cell::new(false);
        let retained = Cell::new(0);
        let mut work = Work::new(LIMIT);
        let mut other_work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut other = Budget::new(&mut other_work, LIMIT);
        let result =
            with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
                let floor = budget.storage();
                let original = budget.work_ledger_identity_v1();
                let error = if same_slot {
                    std::mem::swap(budget, &mut other);
                    assert!(budget.work_ledger_identity_v1() != original);
                    let error = scope.usage(budget).unwrap_err();
                    std::mem::swap(budget, &mut other);
                    error
                } else {
                    scope.usage(&mut other).unwrap_err()
                };
                assert_eq!(error, Resource::Accounting.into());
                assert!(budget.work_ledger_identity_v1() == original);
                assert_eq!(budget.storage(), floor);
                assert_eq!(other.storage(), 0);
                assert_eq!(scope.usage(budget), Err(error.clone()));
                retained.set(floor);
                completed.set(true);
                // Deliberately swallow the refusal: settlement must still reject.
                Ok(())
            });
        assert!(completed.get());
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), retained.get());
        assert!(retained.get() > 0);
    }
}

#[test]
fn owned_usage_undercut_restore_keeps_first_denial_and_refund_suppression() {
    for prior in [false, true] {
        let completed = Cell::new(false);
        let retained = Cell::new(0);
        let expected = std::cell::RefCell::new(None);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result =
            with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
                let floor = budget.storage();
                if prior {
                    assert!(budget.charge_work(LIMIT + 1).is_err());
                }
                let selected = if prior {
                    Error::PriorDenial {
                        work: budget.failed_work(),
                        storage: None,
                    }
                } else {
                    Resource::Accounting.into()
                };
                budget.release_storage(1).unwrap();
                assert_eq!(budget.storage(), floor - 1);
                assert_eq!(scope.usage(budget), Err(selected.clone()));
                budget.reserve_storage(1).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(scope.usage(budget), Err(selected.clone()));
                *expected.borrow_mut() = Some(selected);
                retained.set(floor);
                completed.set(true);
                Ok(())
            });
        assert!(completed.get());
        assert_eq!(result, Err(expected.into_inner().unwrap()));
        assert_eq!(budget.storage(), retained.get());
        assert!(retained.get() > 0);
    }
}

#[test]
fn owned_prequery_session_undercut_is_not_a_new_borrowed_entry_floor() {
    for narrowing in [false, true] {
        let completed = Cell::new(false);
        let entered = Cell::new(false);
        let retained = Cell::new(0);
        let set = singleton();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result =
            with_presburger_queries_v4(Default::default(), &mut budget, |scope, budget| {
                let floor = budget.storage();
                budget.release_storage(1).unwrap();
                let result = if narrowing {
                    scope.with_affine_narrowing_v4(&set, budget, |_, _| {
                        entered.set(true);
                        Ok(())
                    })
                } else {
                    scope.with_witness(&set, budget, |_, _| {
                        entered.set(true);
                        Ok(())
                    })
                };
                assert_eq!(result, Err(Resource::Accounting.into()));
                assert!(!entered.get());
                assert_eq!(budget.storage(), floor - 1);
                budget.reserve_storage(1).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(query(scope, &set, budget), result);
                retained.set(floor);
                completed.set(true);
                Ok(())
            });
        assert!(completed.get());
        assert!(!entered.get());
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), retained.get());
        assert!(retained.get() > 0);
    }
}
