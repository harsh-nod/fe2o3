use super::super::{
    MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2, ReferenceCastKindV1, ReferenceGuardClauseV1,
    ReferenceUnaryOpV1, reference_boolean_guard_atom_v1,
};
use super::*;

fn point() -> X {
    X::PointCoordinate { axis: 0 }
}
fn number(bits: u128) -> X {
    X::Constant(C::Scalar {
        scalar: T::Usize,
        bits,
    })
}
fn op(operation: B, lhs: X, rhs: X) -> X {
    X::Binary {
        operation,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        checked: false,
    }
}
fn coordinate(divisor: u128, stride: u128) -> X {
    op(
        B::Add,
        op(
            B::Multiply,
            op(B::Divide, point(), number(divisor)),
            number(stride),
        ),
        op(B::Remainder, point(), number(divisor)),
    )
}
fn condition(divisor: u128, threshold: u128) -> X {
    op(
        B::LessThan,
        op(B::Remainder, point(), number(divisor)),
        number(threshold),
    )
}
fn bound(divisor: u128, threshold: u128) -> A {
    reference_boolean_guard_atom_v1(condition(divisor, threshold), true)
}
fn predicate(clauses: Vec<Vec<A>>) -> ReferencePathPredicateV1 {
    ReferencePathPredicateV1 {
        clauses: clauses
            .into_iter()
            .map(|atoms| ReferenceGuardClauseV1 {
                atoms: atoms.into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}
fn guard(divisor: u128, threshold: u128) -> ReferencePathPredicateV1 {
    predicate(vec![vec![bound(divisor, threshold)]])
}
fn recognize(expression: &X, guard: &ReferencePathPredicateV1) -> Option<CompactRowsUsize1D> {
    CompactRowsUsize1D::from_expression(expression, guard, &mut Work::default()).unwrap()
}
fn at_mut<'a>(mut expression: &'a mut X, path: &[usize]) -> &'a mut X {
    for side in path {
        let X::Binary { lhs, rhs, .. } = expression else {
            panic!("binary path")
        };
        expression = match side {
            0 => lhs,
            1 => rhs,
            _ => panic!("binary side"),
        };
    }
    expression
}
fn assert_resource<T: std::fmt::Debug>(result: Result<T, Error>) {
    let error = result.unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "reference symbolic execution exceeds {MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2} cumulative expression work nodes"
        )
    );
}

#[test]
fn compact_row_coordinate_stage_roundtrip_keeps_complete_guard() {
    let expression = coordinate(64, 16);
    let unrelated = reference_boolean_guard_atom_v1(
        op(
            B::Equal,
            X::InputLength {
                reference_argument: 2,
            },
            number(256),
        ),
        true,
    );
    let guard = predicate(vec![vec![unrelated, bound(64, 16)]]);
    let before_expression = expression.clone();
    let before_guard = guard.clone();
    let mut work = Work::default();
    let descriptor = CompactRowsUsize1D::from_expression(&expression, &guard, &mut work)
        .unwrap()
        .unwrap();
    assert_eq!(
        (descriptor.axis(), descriptor.divisor(), descriptor.stride()),
        (0, 64, 16)
    );
    let spent = work.charged_nodes;
    assert_eq!(descriptor.expression(&mut work).unwrap(), expression);
    assert_eq!(work.charged_nodes - spent, 9);
    assert_eq!(expression, before_expression);
    assert_eq!(guard, before_guard);
}

#[test]
fn compact_row_coordinate_accepts_only_stronger_strict_thresholds() {
    for threshold in [0, 1, 15, 16] {
        assert!(recognize(&coordinate(64, 16), &guard(64, threshold)).is_some());
    }
    for threshold in [17, 63, 64, u128::from(u64::MAX), u128::from(u64::MAX) + 1] {
        assert_eq!(
            recognize(&coordinate(64, 16), &guard(64, threshold)),
            None,
            "{threshold}"
        );
    }
}

#[test]
fn compact_row_coordinate_checks_each_disjunct_and_does_not_join_witnesses() {
    let good = bound(64, 16);
    let two = predicate(vec![vec![good.clone()], vec![bound(64, 12)]]);
    assert!(recognize(&coordinate(64, 16), &two).is_some());
    for atoms in [
        vec![],
        vec![bound(64, 17)],
        vec![bound(32, 16)],
        vec![A::Assert {
            condition: condition(64, 16),
            expected: true,
        }],
    ] {
        for clauses in [
            vec![vec![good.clone()], atoms.clone()],
            vec![atoms, vec![good.clone()]],
        ] {
            let bad = predicate(clauses);
            let original = bad.clone();
            assert_eq!(recognize(&coordinate(64, 16), &bad), None);
            assert_eq!(bad, original);
        }
    }
}

#[test]
fn compact_row_coordinate_no_vacuous_empty_domain_or_missing_guard() {
    assert_eq!(recognize(&coordinate(64, 16), &predicate(vec![])), None);
    assert_eq!(
        recognize(&coordinate(64, 16), &predicate(vec![vec![]])),
        None
    );
    assert_eq!(recognize(&coordinate(64, 64), &predicate(vec![])), None);
    for divisor in [1, 2, 64, u64::MAX] {
        let expression = coordinate(u128::from(divisor), u128::from(divisor));
        let descriptor = recognize(&expression, &predicate(vec![vec![]])).unwrap();
        assert_eq!(descriptor.stride(), divisor);
    }
}

#[test]
fn compact_row_coordinate_requires_canonical_positive_switch_not_assert() {
    let positive = bound(64, 16);
    assert!(
        matches!(&positive, A::SwitchValueSet { values, inside_set: false, .. } if values.as_ref() == [0])
    );
    for atom in [
        reference_boolean_guard_atom_v1(condition(64, 16), false),
        A::Assert {
            condition: condition(64, 16),
            expected: true,
        },
        A::Assert {
            condition: condition(64, 16),
            expected: false,
        },
        A::SwitchValueSet {
            discriminant: condition(64, 16),
            values: vec![1].into_boxed_slice(),
            inside_set: true,
        },
        A::SwitchValueSet {
            discriminant: condition(64, 16),
            values: vec![0, 0].into_boxed_slice(),
            inside_set: false,
        },
        A::SwitchValueSet {
            discriminant: condition(64, 16),
            values: vec![0, 1].into_boxed_slice(),
            inside_set: false,
        },
        A::SwitchValueSet {
            discriminant: condition(64, 16),
            values: Box::default(),
            inside_set: false,
        },
    ] {
        assert_eq!(
            recognize(&coordinate(64, 16), &predicate(vec![vec![atom]])),
            None
        );
    }
    // An unrelated assertion is preserved, never used as the remainder witness.
    let with_assert = predicate(vec![vec![
        A::Assert {
            condition: point(),
            expected: true,
        },
        positive,
    ]]);
    assert!(recognize(&coordinate(64, 16), &with_assert).is_some());
}

#[test]
fn compact_row_coordinate_guard_requires_exact_point_divisor_type_and_operation() {
    let mut changes = Vec::new();
    for path in [&[0, 0][..], &[0, 1][..], &[1][..]] {
        let mut changed = condition(64, 16);
        *at_mut(&mut changed, path) = match path {
            [0, 0] => X::PointCoordinate { axis: 1 },
            [0, 1] => number(32),
            _ => X::Constant(C::Scalar {
                scalar: T::U64,
                bits: 16,
            }),
        };
        changes.push(changed);
    }
    for operation in [B::LessEqual, B::Equal, B::GreaterThan, B::GreaterEqual] {
        changes.push(op(
            operation,
            op(B::Remainder, point(), number(64)),
            number(16),
        ));
    }
    changes.push(op(
        B::LessThan,
        number(16),
        op(B::Remainder, point(), number(64)),
    ));
    changes.push(op(
        B::LessThan,
        op(B::BitAnd, point(), number(63)),
        number(16),
    ));
    changes.push(X::Unary {
        operation: ReferenceUnaryOpV1::Not,
        operand: Box::new(condition(64, 16)),
    });
    for path in [&[][..], &[0][..]] {
        let mut changed = condition(64, 16);
        let X::Binary { checked, .. } = at_mut(&mut changed, path) else {
            unreachable!()
        };
        *checked = true;
        changes.push(changed);
    }
    for changed in changes {
        assert_eq!(
            recognize(
                &coordinate(64, 16),
                &predicate(vec![vec![reference_boolean_guard_atom_v1(changed, true)]])
            ),
            None
        );
    }
}

#[test]
fn compact_row_coordinate_rejects_zero_inconsistent_and_out_of_width_constants() {
    for (divisor, stride) in [
        (0, 0),
        (0, 1),
        (1, 0),
        (64, 0),
        (16, 17),
        (u128::from(u64::MAX) + 1, 16),
        (64, u128::from(u64::MAX) + 1),
    ] {
        assert_eq!(
            recognize(&coordinate(divisor, stride), &guard(divisor, stride)),
            None
        );
    }
    for replacement in [0, 32, 65, u128::from(u64::MAX) + 1] {
        let mut expression = coordinate(64, 16);
        *at_mut(&mut expression, &[1, 1]) = number(replacement);
        assert_eq!(recognize(&expression, &guard(64, 16)), None);
    }
}

#[test]
fn compact_row_coordinate_rejects_signed_wrong_width_bool_float_and_zst_literals() {
    for path in [&[0, 0, 1][..], &[0, 1][..], &[1, 1][..]] {
        for scalar in [
            T::Bool,
            T::U8,
            T::U16,
            T::U32,
            T::U64,
            T::I8,
            T::I16,
            T::I32,
            T::I64,
            T::Isize,
            T::F32,
            T::F64,
        ] {
            let mut expression = coordinate(64, 16);
            let X::Constant(C::Scalar { scalar: actual, .. }) = at_mut(&mut expression, path)
            else {
                unreachable!()
            };
            *actual = scalar;
            assert_eq!(
                recognize(&expression, &guard(64, 16)),
                None,
                "{path:?}/{scalar:?}"
            );
        }
        let mut expression = coordinate(64, 16);
        *at_mut(&mut expression, path) = X::Constant(C::ZeroSized);
        assert_eq!(recognize(&expression, &guard(64, 16)), None);
    }
    for scalar in [T::Bool, T::U64, T::I64, T::F32] {
        let mut condition = condition(64, 16);
        *at_mut(&mut condition, &[1]) = X::Constant(C::Scalar { scalar, bits: 16 });
        assert_eq!(
            recognize(
                &coordinate(64, 16),
                &predicate(vec![vec![reference_boolean_guard_atom_v1(condition, true)]])
            ),
            None
        );
    }
}

#[test]
fn compact_row_coordinate_rejects_reordered_checked_or_different_operations() {
    for path in [&[][..], &[0][..], &[0, 0][..], &[1][..]] {
        let mut swapped = coordinate(64, 16);
        let X::Binary { lhs, rhs, .. } = at_mut(&mut swapped, path) else {
            unreachable!()
        };
        std::mem::swap(lhs, rhs);
        assert_eq!(
            recognize(&swapped, &guard(64, 16)),
            None,
            "swapped {path:?}"
        );
        let mut checked = coordinate(64, 16);
        let X::Binary { checked: flag, .. } = at_mut(&mut checked, path) else {
            unreachable!()
        };
        *flag = true;
        assert_eq!(
            recognize(&checked, &guard(64, 16)),
            None,
            "checked {path:?}"
        );
        let mut changed = coordinate(64, 16);
        let X::Binary { operation, .. } = at_mut(&mut changed, path) else {
            unreachable!()
        };
        *operation = B::Subtract;
        assert_eq!(
            recognize(&changed, &guard(64, 16)),
            None,
            "operation {path:?}"
        );
    }
}

#[test]
fn compact_row_coordinate_rejects_casts_loads_aliases_and_nonzero_axes() {
    for path in [&[0, 0, 0][..], &[1, 0][..]] {
        for replacement in [
            X::PointCoordinate { axis: 1 },
            X::KernelScalarArgument { argument: 0 },
            X::InputLength {
                reference_argument: 0,
            },
            X::InputLoad {
                reference_argument: 0,
                index: Box::new(point()),
            },
            X::Cast {
                kind: ReferenceCastKindV1::Integer,
                source: T::Usize,
                target: T::Usize,
                operand: Box::new(point()),
            },
            X::Unary {
                operation: ReferenceUnaryOpV1::Not,
                operand: Box::new(point()),
            },
        ] {
            let mut expression = coordinate(64, 16);
            *at_mut(&mut expression, path) = replacement;
            assert_eq!(recognize(&expression, &guard(64, 16)), None);
        }
    }
    let wrapped = X::Cast {
        kind: ReferenceCastKindV1::Integer,
        source: T::Usize,
        target: T::Usize,
        operand: Box::new(coordinate(64, 16)),
    };
    assert_eq!(recognize(&wrapped, &guard(64, 16)), None);
}

fn evaluate(expression: &X, point: u64) -> Option<u64> {
    match expression {
        X::PointCoordinate { axis: 0 } => Some(point),
        X::Constant(C::Scalar {
            scalar: T::Usize,
            bits,
        }) => u64::try_from(*bits).ok(),
        X::Binary {
            operation,
            lhs,
            rhs,
            checked: false,
        } => {
            let lhs = evaluate(lhs, point)?;
            let rhs = evaluate(rhs, point)?;
            match operation {
                B::Add => lhs.checked_add(rhs),
                B::Multiply => lhs.checked_mul(rhs),
                B::Divide => lhs.checked_div(rhs),
                B::Remainder => lhs.checked_rem(rhs),
                _ => None,
            }
        }
        _ => None,
    }
}

#[test]
fn compact_row_coordinate_finite_differential_injective_and_invertible() {
    for divisor in 1_u64..=32 {
        for stride in 1..=divisor {
            let original = coordinate(u128::from(divisor), u128::from(stride));
            let descriptor =
                recognize(&original, &guard(u128::from(divisor), u128::from(stride))).unwrap();
            let materialized = descriptor.expression(&mut Work::default()).unwrap();
            let mut image = std::collections::BTreeMap::new();
            for point in 0_u64..1024 {
                let actual =
                    evaluate(&materialized, point).expect("no arithmetic overflow or zero divisor");
                let expected = (u128::from(point) / u128::from(divisor)) * u128::from(stride)
                    + u128::from(point) % u128::from(divisor);
                assert_eq!(u128::from(actual), expected);
                assert!(actual <= point);
                if point % divisor < stride {
                    assert_eq!(
                        image.insert(actual, point),
                        None,
                        "D={divisor},S={stride},p={point}"
                    );
                    let inverse = u128::from(actual / stride) * u128::from(divisor)
                        + u128::from(actual % stride);
                    assert_eq!(inverse, u128::from(point));
                }
            }
        }
    }
}

#[test]
fn compact_row_coordinate_word_boundaries_are_total_before_lane_guard() {
    for (divisor, stride) in [
        (1, 1),
        (2, 1),
        (64, 16),
        (u64::MAX, 1),
        (u64::MAX, u64::MAX - 1),
        (u64::MAX, u64::MAX),
        (1_u64 << 63, (1_u64 << 63) - 1),
    ] {
        let descriptor = recognize(
            &coordinate(u128::from(divisor), u128::from(stride)),
            &guard(u128::from(divisor), u128::from(stride)),
        )
        .unwrap();
        let expression = descriptor.expression(&mut Work::default()).unwrap();
        for point in [0, 1, divisor - 1, divisor, u64::MAX - 1, u64::MAX] {
            let actual = evaluate(&expression, point).expect("total even for inactive lanes");
            let expected = (u128::from(point) / u128::from(divisor)) * u128::from(stride)
                + u128::from(point) % u128::from(divisor);
            assert_eq!(u128::from(actual), expected);
            assert!(actual <= point);
            if point % divisor < stride {
                assert_eq!(
                    u128::from(actual / stride) * u128::from(divisor) + u128::from(actual % stride),
                    u128::from(point)
                );
            }
        }
    }
}

#[test]
fn compact_row_coordinate_collision_without_guard_is_not_a_descriptor() {
    let expression = coordinate(64, 16);
    assert_eq!(evaluate(&expression, 16), Some(16));
    assert_eq!(evaluate(&expression, 64), Some(16));
    assert_eq!(recognize(&expression, &predicate(vec![vec![]])), None);
    assert_eq!(recognize(&expression, &guard(64, 17)), None);
    assert_eq!(
        recognize(&expression, &predicate(vec![vec![bound(64, 16)], vec![]])),
        None
    );
    let compact = recognize(&expression, &guard(64, 16)).unwrap();
    let wider = recognize(&coordinate(64, 32), &guard(64, 32)).unwrap();
    let other_divisor = recognize(&coordinate(32, 16), &guard(32, 16)).unwrap();
    assert_ne!(compact, wider);
    assert_ne!(compact, other_divisor);
}

#[test]
fn compact_row_coordinate_shared_budget_exact_boundary_and_no_refund() {
    let expression = coordinate(64, 16);
    let predicate = guard(64, 16);
    let mut measured = Work::default();
    assert!(
        CompactRowsUsize1D::from_expression(&expression, &predicate, &mut measured)
            .unwrap()
            .is_some()
    );
    let cost = measured.charged_nodes;
    assert_eq!(
        cost, 17,
        "nine shape nodes, predicate, clause, atom and five condition nodes"
    );
    let mut exact = Work::default();
    exact
        .charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - cost)
        .unwrap();
    assert!(
        CompactRowsUsize1D::from_expression(&expression, &predicate, &mut exact)
            .unwrap()
            .is_some()
    );
    assert_eq!(exact.charged_nodes, MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2);
    assert_resource(CompactRowsUsize1D::from_expression(
        &expression,
        &predicate,
        &mut exact,
    ));
    assert_eq!(
        exact.charged_nodes,
        MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 + 1
    );
    let mut insufficient = Work::default();
    insufficient
        .charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - cost + 1)
        .unwrap();
    assert_resource(CompactRowsUsize1D::from_expression(
        &expression,
        &predicate,
        &mut insufficient,
    ));
    assert_eq!(
        insufficient.charged_nodes,
        MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 + 1
    );
}

#[test]
fn compact_row_coordinate_later_clause_exhaustion_cannot_reuse_first_witness() {
    let expression = coordinate(64, 16);
    let two = predicate(vec![vec![bound(64, 16)], vec![bound(64, 16)]]);
    let original = two.clone();
    let mut work = Work::default();
    work.charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - 18)
        .unwrap();
    assert_resource(CompactRowsUsize1D::from_expression(
        &expression,
        &two,
        &mut work,
    ));
    assert_eq!(work.charged_nodes, MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 + 1);
    assert_eq!(two, original);
}

#[test]
fn compact_row_coordinate_materialization_is_precharged_and_never_resets_owner() {
    let descriptor = recognize(&coordinate(64, 16), &guard(64, 16)).unwrap();
    let mut work = Work::default();
    work.charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - 9)
        .unwrap();
    assert_eq!(
        descriptor.expression(&mut work).unwrap(),
        coordinate(64, 16)
    );
    assert_eq!(work.charged_nodes, MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2);
    assert_resource(descriptor.expression(&mut work));
    assert_eq!(work.charged_nodes, MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 + 9);
    let mut insufficient = Work::default();
    insufficient
        .charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - 8)
        .unwrap();
    assert_resource(descriptor.expression(&mut insufficient));
    assert_eq!(
        insufficient.charged_nodes,
        MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 + 1
    );
}

#[test]
fn compact_row_coordinate_structural_misses_are_none_but_exhaustion_is_error() {
    let predicate = guard(64, 16);
    let mut work = Work::default();
    assert_eq!(
        CompactRowsUsize1D::from_expression(&point(), &predicate, &mut work).unwrap(),
        None
    );
    assert_eq!(work.charged_nodes, 1);
    work.charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 - 1)
        .unwrap();
    assert_resource(CompactRowsUsize1D::from_expression(
        &point(),
        &predicate,
        &mut work,
    ));
    assert_eq!(work.charged_nodes, MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2 + 1);
    let mut overflow = Work {
        charged_nodes: usize::MAX,
    };
    let error =
        CompactRowsUsize1D::from_expression(&point(), &predicate, &mut overflow).unwrap_err();
    assert_eq!(
        error.to_string(),
        "reference symbolic work-node accounting overflowed"
    );
    assert_eq!(overflow.charged_nodes, usize::MAX);
}
