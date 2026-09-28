use super::super::{
    PresburgerAffineExprV1 as Expr, PresburgerBoxV1 as BoxV1, PresburgerSetDecisionV1,
};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const FLOOR: usize = 73;
const PREFIX: usize = 11;

thread_local! {
    // A thread-local observer leaves the exact callback-capture layouts intact.
    static COMPLETED_CHECKS: Cell<usize> = const { Cell::new(0) };
}
fn completed_checks() -> usize {
    COMPLETED_CHECKS.with(Cell::get)
}
fn complete_checks() {
    COMPLETED_CHECKS.with(|count| count.set(count.get() + 1));
}

fn box_set(rank: usize) -> PresburgerSetV1 {
    PresburgerSetV1::box_only(BoxV1::new(vec![0; rank], vec![1; rank]).unwrap())
}

fn query(
    scope: &mut PresburgerQueryScopeV2<'_>,
    set: &PresburgerSetV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scope.with_witness(set, budget, |decision, _| {
        assert!(matches!(decision, PresburgerQueryDecisionV2::Witness(point) if point.iter().all(|value| *value == 0)));
        Ok(())
    })
}

// Independent operation census for an unconstrained singleton box: session=4;
// each query=entry4+shape4+empty(2r+1)+two copies(2r+8)+nodes(r+1)+edges2r.
fn singleton_work(rank: usize, queries: usize) -> usize {
    4 + queries * (18 + 7 * rank)
}

fn singleton_scratch(rank: usize) -> usize {
    let result_frames =
        2 * size_of::<std::thread::Result<Result<()>>>() + size_of::<std::thread::Result<()>>();
    result_frames
        + size_of::<PresburgerQueryScopeV2<'_>>()
        + result_frames
        + size_of::<Option<Vec<i128>>>()
        + size_of::<Vec<i128>>()
        + size_of::<Meter<'_, '_, '_>>()
        + (rank + 1) * size_of::<(usize, i128, &mut [i128], Result<Option<Vec<i128>>>)>()
        + 2 * rank * size_of::<i128>()
}

#[test]
fn actual_search_matches_v1_witness_empty_pruning_and_congruence() {
    let cases = [
        (
            PresburgerSetV1::new(
                BoxV1::new(vec![0, 0], vec![4, 4]).unwrap(),
                vec![
                    PresburgerConstraintV1::EqualZero(Expr::new(-3, vec![1, 1]).unwrap()),
                    PresburgerConstraintV1::CongruentZero {
                        expression: Expr::new(-1, vec![1, 0]).unwrap(),
                        modulus: 2,
                    },
                ],
            )
            .unwrap(),
            Some(vec![1, 2]),
        ),
        (
            PresburgerSetV1::new(
                BoxV1::new(vec![0], vec![1_000_000]).unwrap(),
                vec![PresburgerConstraintV1::LessEqualZero(
                    Expr::new(3, vec![1]).unwrap(),
                )],
            )
            .unwrap(),
            None,
        ),
        (
            PresburgerSetV1::box_only(BoxV1::new(vec![3], vec![3]).unwrap()),
            None,
        ),
        (box_set(0), Some(vec![])),
    ];
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    with_presburger_queries_v2(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            for (set, expected) in &cases {
                match (set.find_witness(), expected) {
                    (PresburgerSetDecisionV1::Witness(actual), Some(expected)) => {
                        assert_eq!(actual.point(), expected)
                    }
                    (PresburgerSetDecisionV1::Empty, None) => {}
                    other => panic!("unexpected V1 result: {other:?}"),
                }
                scope.with_witness(set, budget, |decision, _| {
                    match (decision, expected) {
                        (PresburgerQueryDecisionV2::Witness(actual), Some(expected)) => {
                            assert_eq!(actual, expected)
                        }
                        (PresburgerQueryDecisionV2::Empty, None) => {}
                        other => panic!("unexpected V2 result: {other:?}"),
                    }
                    Ok(())
                })?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn cumulative_exact_and_one_short_shared_work_preserve_prefix_and_first_error() {
    let completed = completed_checks();
    let set = box_set(2);
    let exact = PREFIX + singleton_work(2, 2);
    for short in [false, true] {
        let mut work = Work::new(exact - usize::from(short));
        let mut budget = Budget::new(&mut work, 100_000);
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let callbacks = Cell::new(0);
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                for _ in 0..2 {
                    let before = budget.storage();
                    let result = scope.with_witness(&set, budget, |decision, _| {
                        assert_eq!(decision, PresburgerQueryDecisionV2::Witness(&[0, 0]));
                        callbacks.set(callbacks.get() + 1);
                        Ok(())
                    });
                    assert_eq!(budget.storage(), before);
                    if let Err(first) = result {
                        assert!(short);
                        let accepted = budget.work();
                        assert_eq!(query(scope, &set, budget), Err(first.clone()));
                        assert_eq!(budget.work(), accepted);
                        complete_checks();
                        return Ok(()); // Ignoring the denial must not unpoison the session.
                    }
                }
                complete_checks();
                Ok(())
            },
        );
        if short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Work(error))) if error.actual() == exact && error.limit() == exact - 1)
            );
            assert_eq!(callbacks.get(), 1);
            assert_eq!(budget.failed_work(), Some(exact));
            assert_eq!(budget.work(), exact - 6);
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(callbacks.get(), 2);
            assert_eq!(budget.work(), exact);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(completed_checks(), completed + 2);
}

#[test]
fn repeated_query_storage_exact_and_one_short_refund_without_accumulation() {
    let exact = FLOOR + singleton_scratch(2);
    for short in [false, true] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, exact - usize::from(short));
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                let set = box_set(2);
                let entry = budget.storage();
                for _ in 0..3 {
                    query(scope, &set, budget)?;
                    assert_eq!(budget.storage(), entry);
                }
                Ok(())
            },
        );
        if short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == exact && error.limit() == exact - 1)
            );
            assert_eq!(budget.failed_storage(), Some(exact));
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(budget.peak_storage(), exact);
            assert_eq!(budget.work(), singleton_work(2, 3));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn local_work_storage_and_query_caps_are_cumulative_and_never_empty_proofs() {
    let completed = completed_checks();
    let set = box_set(1);
    for (limits, kind, actual, limit) in [
        (
            PresburgerQueryLimitsV2 {
                work: singleton_work(1, 2) - 1,
                ..Default::default()
            },
            PresburgerQueryResourceV2::Work,
            singleton_work(1, 2),
            singleton_work(1, 2) - 1,
        ),
        (
            PresburgerQueryLimitsV2 {
                queries: 1,
                ..Default::default()
            },
            PresburgerQueryResourceV2::Queries,
            2,
            1,
        ),
    ] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let expected = Error::Limit {
            resource: kind,
            actual,
            limit,
        };
        let result = with_presburger_queries_v2(limits, &mut budget, |scope, budget| {
            for _ in 0..2 {
                if let Err(first) = query(scope, &set, budget) {
                    assert_eq!(first, expected);
                    assert_eq!(query(scope, &set, budget), Err(first));
                    complete_checks();
                    return Ok(());
                }
            }
            panic!("a local cap must refuse");
        });
        assert_eq!(result, Err(expected));
        assert_eq!(budget.failed_work(), None);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.storage(), FLOOR);
    }
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let limits = PresburgerQueryLimitsV2 {
        scratch_bytes: singleton_scratch(1) - 1,
        ..Default::default()
    };
    let result = with_presburger_queries_v2(limits, &mut budget, |scope, budget| {
        let first = query(scope, &box_set(1), budget).unwrap_err();
        assert_eq!(
            first,
            Error::Limit {
                resource: PresburgerQueryResourceV2::Scratch,
                actual: singleton_scratch(1),
                limit: singleton_scratch(1) - 1
            }
        );
        assert_eq!(query(scope, &box_set(1), budget), Err(first));
        complete_checks();
        Ok(())
    });
    assert_eq!(
        result,
        Err(Error::Limit {
            resource: PresburgerQueryResourceV2::Scratch,
            actual: singleton_scratch(1),
            limit: singleton_scratch(1) - 1
        })
    );
    assert_eq!((budget.storage(), budget.failed_storage()), (FLOOR, None));
    assert_eq!(completed_checks(), completed + 3);
}

#[test]
fn coefficient_rows_are_prepaid_before_arithmetic_and_unknown_is_sticky() {
    let completed = completed_checks();
    let set = PresburgerSetV1::new(
        BoxV1::new(vec![1], vec![2]).unwrap(),
        vec![PresburgerConstraintV1::EqualZero(
            Expr::new(i128::MAX, vec![1]).unwrap(),
        )],
    )
    .unwrap();
    assert_eq!(
        set.find_witness(),
        PresburgerSetDecisionV1::Incomplete(PresburgerFailureV1::ArithmeticOverflow)
    );
    // Before interval: session4+entry4+shape(4+3)+empty3+copy5+node1+constraint1.
    let before_interval = 25;
    for short in [false, true] {
        let limit = if short {
            before_interval + 20 - 1
        } else {
            100_000
        };
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 100_000);
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                let error = scope
                    .with_witness::<()>(&set, budget, |_, _| {
                        panic!("incomplete cannot call consumer")
                    })
                    .unwrap_err();
                assert_eq!(query(scope, &box_set(1), budget), Err(error.clone()));
                complete_checks();
                Err::<(), _>(error)
            },
        );
        if short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Work(error))) if error.actual() == before_interval + 20)
            );
            assert_eq!(budget.work(), before_interval);
        } else {
            assert_eq!(
                result,
                Err(Error::Model(PresburgerFailureV1::ArithmeticOverflow))
            );
            assert_eq!(budget.work(), before_interval + 20);
        }
        assert_eq!(budget.storage(), 0);
    }
    assert_eq!(completed_checks(), completed + 2);
}

#[test]
fn invalid_private_model_shapes_refuse_before_search_or_callback() {
    let mut mismatched = box_set(1);
    mismatched.domain.upper_exclusive.clear();
    let mut constraints = box_set(1);
    constraints.constraints = vec![PresburgerConstraintV1::EqualZero(
        Expr::new(0, vec![]).unwrap(),
    )];
    let mut modulus = box_set(1);
    modulus.constraints = vec![PresburgerConstraintV1::CongruentZero {
        expression: Expr::new(0, vec![1]).unwrap(),
        modulus: 0,
    }];
    for set in [mismatched, constraints, modulus] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                scope.with_witness::<()>(&set, budget, |_, _| {
                    panic!("invalid input reached callback")
                })
            },
        );
        assert!(matches!(
            result,
            Err(Error::Model(PresburgerFailureV1::InvalidModel { .. }))
        ));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn callback_error_and_panic_restore_floor_and_poison_retries() {
    let completed = completed_checks();
    for panic in [false, true] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let expected = if panic {
            Error::Panicked
        } else {
            Error::CallbackRejected
        };
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                let before = budget.storage();
                let error = scope
                    .with_witness::<()>(&box_set(1), budget, |_, budget| {
                        budget.reserve_storage(31)?;
                        if panic {
                            panic!("expected query callback unwind")
                        }
                        Err(Error::CallbackRejected)
                    })
                    .unwrap_err();
                assert_eq!(error, expected);
                assert_eq!(budget.storage(), before);
                assert_eq!(query(scope, &box_set(1), budget), Err(error));
                complete_checks();
                Ok(())
            },
        );
        assert_eq!(result, Err(expected));
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(completed_checks(), completed + 2);
}

#[test]
fn rejected_result_destructor_unwind_is_drained_and_query_credit_refunded() {
    let completed = completed_checks();
    struct Bomb<'a>(&'a Cell<usize>);
    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("expected rejected-result destructor unwind");
        }
    }
    let dropped = Cell::new(0);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            let before = budget.storage();
            let result = scope.with_witness(&box_set(1), budget, |_, budget| {
                budget.reserve_storage(1)?;
                Ok(Bomb(&dropped))
            });
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            assert_eq!(dropped.get(), 1);
            assert_eq!(budget.storage(), before);
            complete_checks();
            Ok(())
        },
    );
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(completed_checks(), completed + 1);
}

#[test]
fn swallowed_direct_work_and_storage_denials_poison_outer_success() {
    let completed = completed_checks();
    for storage in [false, true] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                let result = scope.with_witness(&box_set(1), budget, |_, budget| {
                    if storage {
                        assert!(budget.reserve_storage(100_001).is_err());
                    } else {
                        assert!(budget.charge_work(100_001).is_err());
                    }
                    Ok(())
                });
                assert!(matches!(result, Err(Error::PriorDenial { .. })));
                assert_eq!(query(scope, &box_set(1), budget), result);
                complete_checks();
                Ok(())
            },
        );
        assert!(
            matches!(
                result,
                Err(Error::PriorDenial {
                    work: None,
                    storage: Some(_)
                })
            ) == storage
        );
        assert!(
            matches!(
                result,
                Err(Error::PriorDenial {
                    work: Some(_),
                    storage: None
                })
            ) != storage
        );
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(completed_checks(), completed + 2);
}

#[test]
fn foreign_ledger_query_leaves_foreign_account_untouched_and_retains_first_refusal() {
    let completed = completed_checks();
    let mut work = Work::new(100_000);
    let mut foreign_work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let mut foreign = Budget::new(&mut foreign_work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            assert_eq!(
                query(scope, &box_set(1), &mut foreign),
                Err(Error::Resource(Resource::Accounting))
            );
            assert_eq!(
                query(scope, &box_set(1), budget),
                Err(Error::Resource(Resource::Accounting))
            );
            complete_checks();
            Ok(())
        },
    );
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!((foreign.work(), foreign.storage()), (0, 0));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(completed_checks(), completed + 1);
}

#[test]
fn moved_original_budget_slot_is_rejected_even_with_identical_ledger() {
    let completed = completed_checks();
    let mut work = Work::new(100_000);
    let mut spare_work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let mut spare = Budget::new(&mut spare_work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            let original_ledger = budget.work_ledger_identity_v1();
            std::mem::swap(budget, &mut spare);
            assert_eq!(spare.work_ledger_identity_v1(), original_ledger);
            assert_eq!(
                query(scope, &box_set(1), &mut spare),
                Err(Error::Resource(Resource::Accounting))
            );
            std::mem::swap(budget, &mut spare);
            assert_eq!(
                query(scope, &box_set(1), budget),
                Err(Error::Resource(Resource::Accounting))
            );
            complete_checks();
            Ok(())
        },
    );
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!((spare.work(), spare.storage()), (0, 0));
    assert_eq!(completed_checks(), completed + 1);
}

#[test]
fn caller_floor_undercut_is_not_repaired_with_fabricated_credit() {
    let completed = completed_checks();
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            budget.release_storage(budget.storage()).unwrap();
            assert_eq!(
                query(scope, &box_set(1), budget),
                Err(Error::Resource(Resource::Accounting))
            );
            complete_checks();
            Ok(())
        },
    );
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), 0);
    assert_eq!(completed_checks(), completed + 1);
}

#[test]
fn prior_ledger_denial_and_zero_local_caps_never_enter_session_callback() {
    for prior in [false, true] {
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, 100_000);
        if prior {
            assert!(budget.charge_work(11).is_err());
        }
        let limits = PresburgerQueryLimitsV2 {
            work: 0,
            ..Default::default()
        };
        let result = with_presburger_queries_v2::<()>(limits, &mut budget, |_, _| {
            panic!("refused entry callback")
        });
        if prior {
            assert_eq!(
                result,
                Err(Error::PriorDenial {
                    work: Some(11),
                    storage: None
                })
            );
        } else {
            assert_eq!(
                result,
                Err(Error::Limit {
                    resource: PresburgerQueryResourceV2::Work,
                    actual: 4,
                    limit: 0
                })
            );
        }
        assert_eq!((budget.work(), budget.storage()), (0, 0));
    }
}

#[test]
fn separate_sessions_still_share_cumulative_work_and_usage_observations_are_paid() {
    let exact = 2 * (singleton_work(1, 1) + 1);
    let mut work = Work::new(exact);
    let mut budget = Budget::new(&mut work, 100_000);
    for _ in 0..2 {
        with_presburger_queries_v2(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                query(scope, &box_set(1), budget)?;
                let usage = scope.usage(budget)?;
                assert_eq!(usage.work, singleton_work(1, 1) + 1);
                assert_eq!(usage.queries, 1);
                assert_eq!(usage.peak_scratch_bytes, singleton_scratch(1));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
    assert_eq!(budget.work(), exact);
    assert!(matches!(
        with_presburger_queries_v2::<()>(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |_, _| panic!("exhausted session")
        ),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn typed_session_and_query_headers_refuse_before_their_callbacks() {
    let completed = completed_checks();
    type Large = [u8; 1024];
    let session_header = 2 * size_of::<std::thread::Result<Result<Large>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<PresburgerQueryScopeV2<'_>>();
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, session_header - 1);
    let result = with_presburger_queries_v2::<Large>(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |_, _| panic!("unpaid session"),
    );
    assert!(
        matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == session_header)
    );
    assert_eq!((budget.work(), budget.storage()), (4, 0));

    let unit_session = 2 * size_of::<std::thread::Result<Result<()>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<PresburgerQueryScopeV2<'_>>();
    let query_header = 2 * size_of::<std::thread::Result<Result<Large>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<Option<Vec<i128>>>()
        + size_of::<Vec<i128>>()
        + size_of::<Meter<'_, '_, '_>>();
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, unit_session + query_header - 1);
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            let unit_session = 2 * size_of::<std::thread::Result<Result<()>>>()
                + size_of::<std::thread::Result<()>>()
                + size_of::<PresburgerQueryScopeV2<'_>>();
            let query_header = 2 * size_of::<std::thread::Result<Result<Large>>>()
                + size_of::<std::thread::Result<()>>()
                + size_of::<Option<Vec<i128>>>()
                + size_of::<Vec<i128>>()
                + size_of::<Meter<'_, '_, '_>>();
            let error = scope
                .with_witness::<Large>(&box_set(1), budget, |_, _| panic!("unpaid query"))
                .unwrap_err();
            assert!(
                matches!(error, Error::Resource(Resource::Storage(error)) if error.actual() == unit_session + query_header)
            );
            assert_eq!(query(scope, &box_set(1), budget), Err(error));
            complete_checks();
            Ok(())
        },
    );
    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
    assert_eq!((budget.work(), budget.storage()), (8, 0));
    assert_eq!(completed_checks(), completed + 1);
}

#[test]
fn outer_callback_error_panic_and_panicking_payload_restore_caller_floor() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("expected panic-payload destructor unwind");
        }
    }
    for mode in 0..3 {
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_presburger_queries_v2::<()>(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |_, budget| {
                budget.reserve_storage(31)?;
                match mode {
                    0 => Err(Error::CallbackRejected),
                    1 => panic!("expected outer callback unwind"),
                    _ => std::panic::panic_any(Payload(dropped.clone())),
                }
            },
        );
        assert_eq!(
            result,
            Err(if mode == 0 {
                Error::CallbackRejected
            } else {
                Error::Panicked
            })
        );
        assert_eq!(dropped.load(Ordering::SeqCst), usize::from(mode == 2));
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn earlier_direct_denial_precedes_later_consumer_rejection() {
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let result = with_presburger_queries_v2::<()>(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            scope.with_witness(&box_set(1), budget, |_, budget| {
                assert!(budget.charge_work(100_001).is_err());
                Err(Error::CallbackRejected)
            })
        },
    );
    assert_eq!(
        result,
        Err(Error::PriorDenial {
            work: budget.failed_work(),
            storage: None
        })
    );
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn recorded_same_ledger_denial_precedes_later_floor_undercut() {
    let completed = completed_checks();
    for storage in [false, true] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_presburger_queries_v2::<()>(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                let first = scope
                    .with_witness(&box_set(1), budget, |_, budget| {
                        if storage {
                            assert!(budget.reserve_storage(100_001).is_err());
                        } else {
                            assert!(budget.charge_work(100_001).is_err());
                        }
                        budget.release_storage(budget.storage()).unwrap();
                        Ok(())
                    })
                    .unwrap_err();
                assert_eq!(
                    first,
                    Error::PriorDenial {
                        work: budget.failed_work(),
                        storage: budget.failed_storage()
                    }
                );
                assert_eq!(query(scope, &box_set(1), budget), Err(first));
                complete_checks();
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err(Error::PriorDenial {
                work: budget.failed_work(),
                storage: budget.failed_storage()
            })
        );
        assert_eq!(budget.storage(), 0, "never recreate undercut credit");
    }
    assert_eq!(completed_checks(), completed + 2);
}

#[test]
fn foreign_failed_ledger_cannot_supply_the_selected_denial() {
    let completed = completed_checks();
    let mut work = Work::new(100_000);
    let mut foreign_work = Work::new(0);
    let mut budget = Budget::new(&mut work, 100_000);
    let mut foreign = Budget::new(&mut foreign_work, 0);
    assert!(foreign.charge_work(1).is_err());
    assert!(foreign.reserve_storage(1).is_err());
    let result = with_presburger_queries_v2::<()>(
        PresburgerQueryLimitsV2::default(),
        &mut budget,
        |scope, budget| {
            assert_eq!(
                query(scope, &box_set(1), &mut foreign),
                Err(Error::Resource(Resource::Accounting))
            );
            assert_eq!(
                query(scope, &box_set(1), budget),
                Err(Error::Resource(Resource::Accounting))
            );
            complete_checks();
            Ok(())
        },
    );
    assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(
        (
            budget.failed_work(),
            budget.failed_storage(),
            budget.storage()
        ),
        (None, None, 0)
    );
    assert_eq!(
        (foreign.failed_work(), foreign.failed_storage()),
        (Some(1), Some(1))
    );
    assert_eq!(completed_checks(), completed + 1);
}

#[test]
fn aligned_owned_session_callback_capture_is_paid_before_invocation() {
    #[repr(align(64))]
    struct Capture([u8; 4096]);
    for short in [false, true] {
        let called = Cell::new(false);
        let capture = Capture([7; 4096]);
        let observed = &called;
        let callback = move |_: &mut PresburgerQueryScopeV2<'_>, _: &mut Budget<'_>| {
            let capture = std::hint::black_box(capture);
            assert_eq!(capture.0[4095], 7);
            observed.set(true);
            Ok(())
        };
        let captured = std::mem::size_of_val(&callback);
        assert!(captured >= 4096 && captured % 64 == 0);
        let exact = FLOOR
            + captured
            + 2 * size_of::<std::thread::Result<Result<()>>>()
            + size_of::<std::thread::Result<()>>()
            + size_of::<PresburgerQueryScopeV2<'_>>();
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, exact - usize::from(short));
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            with_presburger_queries_v2(PresburgerQueryLimitsV2::default(), &mut budget, callback);
        if short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == exact && error.limit() == exact - 1)
            );
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(budget.peak_storage(), exact);
        }
        assert_eq!(called.get(), !short);
        assert_eq!((budget.work(), budget.storage()), (4, FLOOR));
    }
}

#[test]
fn aligned_owned_query_callback_capture_is_paid_before_search() {
    let completed = completed_checks();
    #[repr(align(64))]
    struct Capture([u8; 4096]);
    for short in [false, true] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 20_000);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_presburger_queries_v2::<()>(
            PresburgerQueryLimitsV2::default(),
            &mut budget,
            |scope, budget| {
                let called = Cell::new(false);
                let capture = Capture([7; 4096]);
                let observed = &called;
                let callback = move |_: PresburgerQueryDecisionV2<'_>, _: &mut Budget<'_>| {
                    let capture = std::hint::black_box(capture);
                    assert_eq!(capture.0[4095], 7);
                    observed.set(true);
                    Ok(())
                };
                let captured = std::mem::size_of_val(&callback);
                assert!(captured >= 4096 && captured % 64 == 0);
                let header = captured
                    + 2 * size_of::<std::thread::Result<Result<()>>>()
                    + size_of::<std::thread::Result<()>>()
                    + size_of::<Option<Vec<i128>>>()
                    + size_of::<Vec<i128>>()
                    + size_of::<Meter<'_, '_, '_>>();
                let floor = budget.storage();
                let tail = 2 * size_of::<(usize, i128, &mut [i128], Result<Option<Vec<i128>>>)>()
                    + 2 * size_of::<i128>();
                let padding =
                    budget.storage_limit() - floor - if short { header - 1 } else { header + tail };
                budget.reserve_storage(padding).unwrap();
                let result = scope.with_witness(&box_set(1), budget, callback);
                assert_eq!(budget.storage(), floor + padding);
                budget.release_storage(padding).unwrap();
                if short {
                    let first = result.unwrap_err();
                    assert!(
                        matches!(first, Error::Resource(Resource::Storage(error)) if error.actual() == budget.storage_limit() + 1)
                    );
                    assert!(!called.get());
                    assert_eq!(
                        budget.work(),
                        8,
                        "no set, coefficient or node traversal yet"
                    );
                    assert_eq!(query(scope, &box_set(1), budget), Err(first));
                } else {
                    result.unwrap();
                    assert!(called.get());
                    assert_eq!(budget.work(), singleton_work(1, 1));
                    assert_eq!(budget.peak_storage(), budget.storage_limit());
                }
                complete_checks();
                Ok(())
            },
        );
        if short {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
        } else {
            assert_eq!(result, Ok(()));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(completed_checks(), completed + 2);
}
