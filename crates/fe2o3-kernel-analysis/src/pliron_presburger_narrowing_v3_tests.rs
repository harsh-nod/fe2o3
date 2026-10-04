use super::super::super::{
    PresburgerAffineExprV1 as Expr, PresburgerBoxV1 as BoxV1, PresburgerSetDecisionV1,
};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const FLOOR: usize = 73;
const PREFIX: usize = 11;
const U64_END: i128 = 1_i128 << 64;
thread_local! {
    static COMPLETED: Cell<usize> = const { Cell::new(0) };
}
fn completed() -> usize {
    COMPLETED.get()
}
fn complete() {
    COMPLETED.set(completed() + 1);
}
fn set(
    lower: &[i128],
    upper: &[i128],
    constraints: Vec<PresburgerConstraintV1>,
) -> PresburgerSetV1 {
    PresburgerSetV1::new(
        BoxV1::new(lower.to_vec(), upper.to_vec()).unwrap(),
        constraints,
    )
    .unwrap()
}
fn le(constant: i128, coefficients: &[i128]) -> PresburgerConstraintV1 {
    PresburgerConstraintV1::LessEqualZero(Expr::new(constant, coefficients.to_vec()).unwrap())
}
fn eq(constant: i128, coefficients: &[i128]) -> PresburgerConstraintV1 {
    PresburgerConstraintV1::EqualZero(Expr::new(constant, coefficients.to_vec()).unwrap())
}
fn observe(input: &PresburgerSetV1) -> Result<Option<(Vec<i128>, Vec<i128>)>> {
    let mut work = Work::new(2_000_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let observed = with_presburger_queries_v2(Default::default(), &mut budget, |scope, budget| {
        scope.with_affine_narrowing_v3(input, budget, |decision, _| {
            Ok(match decision {
                PresburgerAffineNarrowingDecisionV3::Empty => None,
                PresburgerAffineNarrowingDecisionV3::Bounds {
                    lower,
                    upper_exclusive,
                } => Some((lower.to_vec(), upper_exclusive.to_vec())),
            })
        })
    });
    assert_eq!(budget.storage(), FLOOR);
    observed
}
fn query(
    scope: &mut PresburgerQueryScopeV2<'_>,
    input: &PresburgerSetV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scope.with_affine_narrowing_v3(input, budget, |decision, _| {
        assert!(matches!(
            decision,
            PresburgerAffineNarrowingDecisionV3::Bounds { .. }
        ));
        complete();
        Ok(())
    })
}

// Independent census: session4; entry4 + validation4 + empty(r+2),
// two copies2(r+4), one fixed-point sweep1 per unconstrained query.
fn box_work(rank: usize, queries: usize) -> usize {
    4 + queries * (19 + 3 * rank)
}
fn query_header<C>(_: &C) -> usize {
    let frames =
        2 * size_of::<std::thread::Result<Result<()>>>() + size_of::<std::thread::Result<()>>();
    frames
        + size_of::<Option<C>>()
        + size_of::<Option<Bounds>>()
        + size_of::<Result<Option<Bounds>>>()
        + size_of::<Bounds>()
        + size_of::<PresburgerAffineNarrowingDecisionV3<'_>>()
        + size_of::<(
            &PresburgerSetV1,
            &mut PresburgerQueryScopeV2<'_>,
            &mut Budget<'_>,
        )>()
}
fn session_header<C>(_: &C) -> usize {
    2 * size_of::<std::thread::Result<Result<()>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<PresburgerQueryScopeV2<'_>>()
        + size_of::<C>()
}

#[test]
fn affine_v3_signed_rounding_is_exact_for_both_divisor_signs() {
    for numerator in -24_i128..=24 {
        for denominator in (-9_i128..=9).filter(|value| *value != 0) {
            let candidates: Vec<i128> = (-32_i128..=32).collect();
            let floor = *candidates
                .iter()
                .filter(|&&q| {
                    if denominator > 0 {
                        q * denominator <= numerator
                    } else {
                        q * denominator >= numerator
                    }
                })
                .max()
                .unwrap();
            let ceil = *candidates
                .iter()
                .filter(|&&q| {
                    if denominator > 0 {
                        q * denominator >= numerator
                    } else {
                        q * denominator <= numerator
                    }
                })
                .min()
                .unwrap();
            assert_eq!(floor_div(numerator, denominator), Ok(floor));
            assert_eq!(ceil_div(numerator, denominator), Ok(ceil));
        }
    }
    for (numerator, denominator) in [(i128::MIN, -1), (7, 0)] {
        assert_eq!(
            floor_div(numerator, denominator),
            Err(Error::Model(PresburgerFailureV1::ArithmeticOverflow))
        );
        assert_eq!(
            ceil_div(numerator, denominator),
            Err(Error::Model(PresburgerFailureV1::ArithmeticOverflow))
        );
    }
}

#[test]
fn affine_v3_full_u64_endpoints_and_signed_extrema_never_wrap() {
    assert_eq!(
        observe(&set(&[0], &[U64_END], vec![])).unwrap(),
        Some((vec![0], vec![U64_END]))
    );
    assert_eq!(
        observe(&set(&[0], &[U64_END], vec![eq(-(U64_END - 1), &[1])])).unwrap(),
        Some((vec![U64_END - 1], vec![U64_END]))
    );
    assert_eq!(
        observe(&set(&[0], &[U64_END], vec![le(U64_END, &[-1])])).unwrap(),
        None
    );
    assert_eq!(
        observe(&set(&[i128::MIN], &[i128::MIN + 1], vec![])).unwrap(),
        Some((vec![i128::MIN], vec![i128::MIN + 1]))
    );
    assert_eq!(
        observe(&set(&[i128::MAX - 1], &[i128::MAX], vec![])).unwrap(),
        Some((vec![i128::MAX - 1], vec![i128::MAX]))
    );
}

#[test]
fn affine_v3_constant_rank_zero_and_original_empty_domains_are_distinct() {
    assert_eq!(
        observe(&set(&[], &[], vec![])).unwrap(),
        Some((vec![], vec![]))
    );
    assert_eq!(observe(&set(&[], &[], vec![le(1, &[])])).unwrap(), None);
    assert_eq!(observe(&set(&[], &[], vec![eq(-1, &[])])).unwrap(), None);
    assert_eq!(observe(&set(&[7], &[7], vec![])).unwrap(), None);
}

#[test]
fn affine_v3_congruence_is_not_misreported_as_satisfied_or_empty() {
    let input = set(
        &[0],
        &[1],
        vec![PresburgerConstraintV1::CongruentZero {
            expression: Expr::new(1, vec![2]).unwrap(),
            modulus: 2,
        }],
    );
    assert_eq!(input.find_witness(), PresburgerSetDecisionV1::Empty);
    assert_eq!(observe(&input).unwrap(), Some((vec![0], vec![1])));
}

#[test]
fn affine_v3_small_exhaustive_oracle_preserves_every_original_solution() {
    for a in -3..=3 {
        for b in -3..=3 {
            for constant in -4..=4 {
                for equality in [false, true] {
                    let constraint = if equality {
                        eq(constant, &[a, b])
                    } else {
                        le(constant, &[a, b])
                    };
                    let input = set(&[-4, -4], &[5, 5], vec![constraint, le(-2, &[1, -1])]);
                    let result = observe(&input).unwrap();
                    let mut witnesses = 0;
                    for x in -4_i128..5 {
                        for y in -4_i128..5 {
                            let value = constant + a * x + b * y;
                            if (if equality { value == 0 } else { value <= 0 }) && x - y <= 2 {
                                witnesses += 1;
                                let (lower, upper) = result
                                    .as_ref()
                                    .expect("a genuine solution cannot be narrowed away");
                                assert!(lower[0] <= x && x < upper[0]);
                                assert!(lower[1] <= y && y < upper[1]);
                            }
                        }
                    }
                    if result.is_none() {
                        assert_eq!(witnesses, 0);
                    }
                }
            }
        }
    }
}

#[test]
fn affine_v3_extent_two_cannot_authorize_extent_four_or_unchecked_axes() {
    let bounded = set(&[0], &[2], vec![le(2, &[-1])]);
    let actual = set(&[0], &[4], vec![le(2, &[-1])]);
    assert_eq!(observe(&bounded).unwrap(), None);
    assert_eq!(observe(&actual).unwrap(), Some((vec![2], vec![4])));
    let multidimensional = set(&[0, 0], &[U64_END, 4], vec![le(0, &[1, 0])]);
    assert_eq!(
        observe(&multidimensional).unwrap(),
        Some((vec![0, 0], vec![1, 4]))
    );
    let two_writers = set(
        &[0, 0, 0, 0],
        &[U64_END, 4, U64_END, 4],
        vec![
            le(0, &[1, 0, 0, 0]),
            le(0, &[0, 0, 1, 0]),
            le(1, &[0, 1, 0, -1]),
        ],
    );
    assert!(observe(&two_writers).unwrap().is_some());
    assert!(matches!(
        two_writers.find_witness(),
        PresburgerSetDecisionV1::Witness(_)
    ));
}

#[test]
fn affine_v3_checked_contradictions_and_fixed_point_chain_are_general() {
    let input = set(
        &[0, 0, 0],
        &[U64_END, U64_END, U64_END],
        vec![le(0, &[1, -1, 0]), le(0, &[0, 1, -1]), le(0, &[0, 0, 1])],
    );
    assert_eq!(
        observe(&input).unwrap(),
        Some((vec![0, 0, 0], vec![1, 1, 1]))
    );
    let contradiction = set(
        &[0, 0],
        &[U64_END, U64_END],
        vec![eq(0, &[1, -1]), le(1, &[-1, 1])],
    );
    // A valid contradiction need not be discovered within the admitted work.
    // This slow propagator must refuse rather than return an invented Empty.
    assert!(matches!(
        observe(&contradiction),
        Err(Error::Limit {
            resource: PresburgerQueryResourceV2::Work,
            ..
        })
    ));
}

#[test]
fn affine_v3_arithmetic_incompleteness_and_malformed_models_are_sticky() {
    let mut malformed = set(&[0], &[1], vec![]);
    malformed
        .constraints
        .push(PresburgerConstraintV1::CongruentZero {
            expression: Expr::new(0, vec![1]).unwrap(),
            modulus: 0,
        });
    let inputs = [
        (
            set(&[2], &[3], vec![le(0, &[i128::MAX])]),
            Error::Model(PresburgerFailureV1::ArithmeticOverflow),
        ),
        (
            set(&[0], &[1], vec![eq(i128::MIN, &[0])]),
            Error::Model(PresburgerFailureV1::ArithmeticOverflow),
        ),
        (
            malformed,
            Error::Model(PresburgerFailureV1::InvalidModel {
                detail: "V2 constraint rank or modulus differs",
            }),
        ),
    ];
    let start = completed();
    for (input, expected) in inputs {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let result =
            with_presburger_queries_v2(Default::default(), &mut budget, |scope, budget| {
                assert_eq!(query(scope, &input, budget), Err(expected.clone()));
                let accepted = budget.work();
                assert_eq!(
                    query(scope, &set(&[0], &[1], vec![]), budget),
                    Err(expected.clone())
                );
                assert_eq!(budget.work(), accepted);
                complete();
                Ok(())
            });
        assert_eq!(result, Err(expected));
        assert_eq!(budget.storage(), 0);
    }
    assert_eq!(completed(), start + 3);
}

#[test]
fn affine_v3_rejected_query_entry_drains_owned_capture_without_masking_limit() {
    struct Hostile;
    impl Drop for Hostile {
        fn drop(&mut self) {
            complete();
            panic!("intentional rejected query capture destructor");
        }
    }
    let start = completed();
    let input = set(&[0], &[1], vec![]);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let expected = Error::Limit {
        resource: PresburgerQueryResourceV2::Queries,
        actual: 1,
        limit: 0,
    };
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2 {
            queries: 0,
            ..Default::default()
        },
        &mut budget,
        |scope, budget| {
            let capture = Hostile;
            let result: Result<()> = scope.with_affine_narrowing_v3(&input, budget, move |_, _| {
                drop(capture);
                panic!("refused query cannot run the callback")
            });
            assert_eq!(result, Err(expected.clone()));
            assert_eq!(query(scope, &input, budget), Err(expected.clone()));
            complete();
            Ok(())
        },
    );
    assert_eq!(result, Err(expected));
    assert_eq!(completed(), start + 2);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn affine_v3_exact_and_one_short_cumulative_work_never_reset_the_ledger() {
    let start = completed();
    let input = set(&[0, 0], &[1, 1], vec![]);
    let exact = PREFIX + box_work(2, 2);
    for short in [false, true] {
        let before = completed();
        let mut work = Work::new(exact - usize::from(short));
        let mut budget = Budget::new(&mut work, 100_000);
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            with_presburger_queries_v2(Default::default(), &mut budget, |scope, budget| {
                for _ in 0..2 {
                    let floor = budget.storage();
                    if let Err(first) = query(scope, &input, budget) {
                        assert!(short);
                        assert_eq!(budget.storage(), floor);
                        let accepted = budget.work();
                        assert_eq!(query(scope, &input, budget), Err(first));
                        assert_eq!(budget.work(), accepted);
                        complete();
                        return Ok(());
                    }
                    assert_eq!(budget.storage(), floor);
                }
                complete();
                Ok(())
            });
        if short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Work(error))) if error.actual() == exact && error.limit() == exact - 1)
            );
            assert_eq!(budget.failed_work(), Some(exact));
            assert_eq!(completed(), before + 2);
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(budget.work(), exact);
            assert_eq!(completed(), before + 3);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(completed(), start + 5);
}

#[test]
fn affine_v3_exact_and_one_short_typed_capacity_headers_are_independent() {
    let input = set(&[0, 0], &[1, 1], vec![]);
    for short in [false, true] {
        let callback = |decision: PresburgerAffineNarrowingDecisionV3<'_>, _: &mut Budget<'_>| {
            assert!(matches!(
                decision,
                PresburgerAffineNarrowingDecisionV3::Bounds { .. }
            ));
            complete();
            Ok(())
        };
        let header = query_header(&callback);
        let outer = |scope: &mut PresburgerQueryScopeV2<'_>, budget: &mut Budget<'_>| {
            scope.with_affine_narrowing_v3(&input, budget, callback)
        };
        let exact = FLOOR + session_header(&outer) + header + 4 * size_of::<i128>();
        let before = completed();
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, exact - usize::from(short));
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_presburger_queries_v2(Default::default(), &mut budget, outer);
        if short {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == exact && error.limit() == exact - 1)
            );
            assert_eq!(completed(), before);
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(budget.peak_storage(), exact);
            assert_eq!(completed(), before + 1);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn affine_v3_query_limit_is_shared_with_unchanged_v2_witness_order() {
    let input = set(&[0, 0], &[4, 4], vec![eq(-3, &[1, 1])]);
    assert!(
        matches!(input.find_witness(), PresburgerSetDecisionV1::Witness(point) if point.point() == [0, 3])
    );
    let start = completed();
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let expected = Error::Limit {
        resource: PresburgerQueryResourceV2::Queries,
        actual: 3,
        limit: 2,
    };
    let result = with_presburger_queries_v2(
        PresburgerQueryLimitsV2 {
            queries: 2,
            ..Default::default()
        },
        &mut budget,
        |scope, budget| {
            query(scope, &input, budget)?;
            scope.with_witness(&input, budget, |decision, _| {
                assert_eq!(decision, PresburgerQueryDecisionV2::Witness(&[0, 3]));
                complete();
                Ok(())
            })?;
            assert_eq!(query(scope, &input, budget), Err(expected.clone()));
            complete();
            Ok(())
        },
    );
    assert_eq!(result, Err(expected));
    assert_eq!(completed(), start + 3);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn affine_v3_swallowed_denial_precedes_later_undercut_and_retry() {
    let input = set(&[0], &[1], vec![]);
    let start = completed();
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_presburger_queries_v2(Default::default(), &mut budget, |scope, budget| {
        let first = scope
            .with_affine_narrowing_v3(&input, budget, |_, budget| {
                assert!(budget.charge_work(100_001).is_err());
                budget.release_storage(1).unwrap();
                complete();
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            first,
            Error::PriorDenial {
                work: Some(_),
                storage: None
            }
        ));
        assert_eq!(query(scope, &input, budget), Err(first.clone()));
        complete();
        Ok(())
    });
    assert!(matches!(
        result,
        Err(Error::PriorDenial {
            work: Some(_),
            storage: None
        })
    ));
    assert_eq!(completed(), start + 2);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn affine_v3_foreign_budget_and_floor_custody_fail_before_decisions() {
    let input = set(&[0], &[1], vec![]);
    let start = completed();
    for foreign in [false, true] {
        let mut work = Work::new(100_000);
        let mut spare_work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        let mut spare = Budget::new(&mut spare_work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            with_presburger_queries_v2(Default::default(), &mut budget, |scope, budget| {
                let floor = budget.storage();
                if foreign {
                    assert_eq!(
                        query(scope, &input, &mut spare),
                        Err(Resource::Accounting.into())
                    );
                    assert_eq!(spare.work(), 0);
                    assert_eq!(spare.storage(), 0);
                } else {
                    budget.release_storage(1).unwrap();
                    assert_eq!(
                        query(scope, &input, budget),
                        Err(Resource::Accounting.into())
                    );
                    budget.reserve_storage(1).unwrap();
                    assert_eq!(budget.storage(), floor);
                }
                assert_eq!(
                    query(scope, &input, budget),
                    Err(Resource::Accounting.into())
                );
                complete();
                Ok(())
            });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(completed(), start + 2);
}

#[test]
fn affine_v3_callback_panics_and_rejection_cannot_be_swallowed() {
    let input = set(&[0], &[1], vec![]);
    let start = completed();
    for panics in [false, true] {
        let expected = if panics {
            Error::Panicked
        } else {
            Error::CallbackRejected
        };
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            with_presburger_queries_v2(Default::default(), &mut budget, |scope, budget| {
                assert_eq!(
                    scope.with_affine_narrowing_v3(&input, budget, |_, _| -> Result<()> {
                        if panics {
                            panic!("intentional V3 callback panic");
                        }
                        Err(Error::CallbackRejected)
                    }),
                    Err(expected.clone())
                );
                assert_eq!(query(scope, &input, budget), Err(expected.clone()));
                complete();
                Ok(())
            });
        assert_eq!(result, Err(expected));
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(completed(), start + 2);
}

#[test]
fn affine_v3_large_owned_capture_is_paid_before_callback_and_dropped_on_refusal() {
    #[repr(align(4096))]
    struct Capture([u8; 4096]);
    impl Drop for Capture {
        fn drop(&mut self) {
            complete();
        }
    }
    let input = set(&[0], &[1], vec![]);
    for mode in 0..3 {
        let before = completed();
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let capture = Capture([7; 4096]);
        let callback = move |_: PresburgerAffineNarrowingDecisionV3<'_>, _: &mut Budget<'_>| {
            assert_eq!(capture.0[0], 7);
            assert_eq!(capture.0[4095], 7);
            drop(capture);
            complete();
            Ok(())
        };
        let needed = query_header(&callback);
        let outer = |scope: &mut PresburgerQueryScopeV2<'_>, budget: &mut Budget<'_>| {
            let result = scope.with_affine_narrowing_v3(&input, budget, callback);
            complete();
            result
        };
        let header = session_header(&outer) + needed;
        let cap = match mode {
            0 => header - 1,
            1 => header,
            _ => header + 2 * size_of::<i128>(),
        };
        let result = with_presburger_queries_v2(
            PresburgerQueryLimitsV2 {
                scratch_bytes: cap,
                ..Default::default()
            },
            &mut budget,
            outer,
        );
        if mode < 2 {
            let actual = header + usize::from(mode == 1) * size_of::<i128>();
            assert_eq!(
                result,
                Err(Error::Limit {
                    resource: PresburgerQueryResourceV2::Scratch,
                    actual,
                    limit: cap
                })
            );
            assert_eq!(completed(), before + 2);
        } else {
            assert_eq!(result, Ok(()));
            assert_eq!(completed(), before + 3);
            assert_eq!(budget.peak_storage(), FLOOR + cap);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}
