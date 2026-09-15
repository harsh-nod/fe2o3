use super::*;

include!("guarded_index_consumer_tests.rs");

fn constant(bits: u128) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits,
    })
}

fn binary(
    operation: ReferenceBinaryOpV1,
    lhs: ReferenceEffectExpressionV1,
    rhs: ReferenceEffectExpressionV1,
) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Binary {
        operation,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        checked: false,
    }
}

fn check() -> ResolvedReferenceBoundsCheckV1 {
    let index = ReferenceEffectExpressionV1::PointCoordinate { axis: 0 };
    let length = ReferenceEffectExpressionV1::InputLength {
        reference_argument: 3,
    };
    ResolvedReferenceBoundsCheckV1 {
        block: 4,
        expected: true,
        condition: binary(ReferenceBinaryOpV1::LessThan, index.clone(), length.clone()),
        index,
        length,
    }
}

fn clause() -> ReferenceGuardClauseV1 {
    let check = check();
    ReferenceGuardClauseV1 {
        atoms: vec![
            reference_boolean_guard_atom_v1(
                binary(ReferenceBinaryOpV1::Equal, check.length, constant(1024)),
                true,
            ),
            reference_boolean_guard_atom_v1(
                binary(ReferenceBinaryOpV1::LessThan, check.index, constant(1024)),
                true,
            ),
        ]
        .into_boxed_slice(),
    }
}

#[test]
fn output_slice_bounds_accept_only_exact_preceding_equality() {
    let mut clause = clause();
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    assert!(equal_length_bound(&clause, &check(), &mut work).unwrap());
    let ReferenceGuardAtomV1::SwitchValueSet {
        discriminant: ReferenceEffectExpressionV1::Binary { lhs, rhs, .. },
        ..
    } = &mut clause.atoms[0]
    else {
        unreachable!()
    };
    std::mem::swap(lhs, rhs);
    assert!(equal_length_bound(&clause, &check(), &mut work).unwrap());
}

#[test]
fn output_slice_bounds_reject_wrong_argument_point_width_and_polarity() {
    for mutation in 0..7 {
        let mut clause = clause();
        let ReferenceGuardAtomV1::SwitchValueSet {
            discriminant:
                ReferenceEffectExpressionV1::Binary {
                    operation,
                    lhs,
                    rhs,
                    checked,
                },
            inside_set,
            ..
        } = &mut clause.atoms[0]
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                **lhs = ReferenceEffectExpressionV1::InputLength {
                    reference_argument: 2,
                }
            }
            1 => **rhs = constant(1023),
            2 => {
                **rhs = ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::U32,
                    bits: 1024,
                })
            }
            3 => {
                **rhs = ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::Bool,
                    bits: 1,
                })
            }
            4 => *inside_set = true,
            5 => *checked = true,
            6 => *operation = ReferenceBinaryOpV1::LessThan,
            _ => unreachable!(),
        }
        assert!(
            !equal_length_bound(
                &clause,
                &check(),
                &mut ReferenceSymbolicWorkBudgetV2::default()
            )
            .unwrap(),
            "mutation {mutation}"
        );
    }
    let mut other_point = check();
    other_point.index = ReferenceEffectExpressionV1::PointCoordinate { axis: 1 };
    assert!(
        !equal_length_bound(
            &clause(),
            &other_point,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        )
        .unwrap()
    );
}

#[test]
fn output_slice_bounds_require_every_predecessor_clause() {
    let good = clause();
    let incomplete = ReferenceGuardClauseV1 {
        atoms: vec![good.atoms[0].clone()].into_boxed_slice(),
    };
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    let path = ReferencePathPredicateV1 {
        clauses: vec![good.clone(), good].into_boxed_slice(),
    };
    assert!(cpu_path_proves_bound_v2(&path, &check(), &mut work).unwrap());
    let path = ReferencePathPredicateV1 {
        clauses: vec![clause(), incomplete].into_boxed_slice(),
    };
    assert!(!cpu_path_proves_bound_v2(&path, &check(), &mut work).unwrap());
    assert!(
        !cpu_path_proves_bound_v2(
            &ReferencePathPredicateV1::unconditional_v1(),
            &check(),
            &mut work
        )
        .unwrap()
    );
}

#[test]
fn output_slice_bounds_never_assume_the_assertion_or_reset_work() {
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    let no_guards = ReferenceGuardClauseV1 {
        atoms: Box::default(),
    };
    assert!(!equal_length_bound(&no_guards, &check(), &mut work).unwrap());
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    work.charge_v2(crate::reference_effect_v1::MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
        .unwrap();
    assert!(
        equal_length_bound(&clause(), &check(), &mut work)
            .unwrap_err()
            .detail()
            .contains("cumulative work budget")
    );
}
