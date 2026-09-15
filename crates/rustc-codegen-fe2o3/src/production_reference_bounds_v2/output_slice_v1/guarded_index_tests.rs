use super::*;

fn c(bits: u128) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Usize,
        bits,
    })
}

fn point() -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
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

fn clause(bounds: &[(ReferenceEffectExpressionV1, u64)]) -> ReferenceGuardClauseV1 {
    ReferenceGuardClauseV1 {
        atoms: bounds
            .iter()
            .map(|(index, bound)| {
                reference_boolean_guard_atom_v1(
                    binary(
                        ReferenceBinaryOpV1::LessThan,
                        index.clone(),
                        c((*bound).into()),
                    ),
                    true,
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}

fn bound(index: &ReferenceEffectExpressionV1, guards: &ReferenceGuardClauseV1) -> Option<u64> {
    upper_bound(
        index,
        guards,
        &mut ReferenceSymbolicWorkBudgetV2::default(),
        0,
        0,
    )
    .unwrap()
}

fn tiled(
    divisor: u64,
    stride: u64,
) -> (
    ReferenceEffectExpressionV1,
    ReferenceEffectExpressionV1,
    ReferenceEffectExpressionV1,
) {
    let batch = binary(ReferenceBinaryOpV1::Divide, point(), c(divisor.into()));
    let lane = binary(ReferenceBinaryOpV1::Remainder, point(), c(divisor.into()));
    let index = binary(
        ReferenceBinaryOpV1::Add,
        binary(
            ReferenceBinaryOpV1::Multiply,
            batch.clone(),
            c(stride.into()),
        ),
        lane.clone(),
    );
    (batch, lane, index)
}

#[test]
fn guarded_index_staging_maximum_comes_from_both_preceding_guards() {
    let (batch, lane, index) = tiled(64, 16);
    assert_eq!(
        bound(&index, &clause(&[(batch.clone(), 16), (lane.clone(), 16)])),
        Some(255)
    );
    assert_eq!(bound(&index, &clause(&[(batch.clone(), 16)])), Some(303));
    assert!(bound(&index, &clause(&[(lane, 16)])).unwrap() >= 256);
    assert_eq!(bound(&index, &clause(&[(batch, 17)])), Some(319));
}

#[test]
fn guarded_index_does_not_treat_another_coordinate_or_false_comparison_as_a_bound() {
    let other = ReferenceEffectExpressionV1::PointCoordinate { axis: 1 };
    assert_eq!(bound(&point(), &clause(&[(other, 16)])), Some(u64::MAX));
    let mut guards = clause(&[(point(), 16)]);
    let ReferenceGuardAtomV1::SwitchValueSet { inside_set, .. } = &mut guards.atoms[0] else {
        unreachable!()
    };
    *inside_set = true;
    assert_eq!(bound(&point(), &guards), Some(u64::MAX));
    let ReferenceGuardAtomV1::SwitchValueSet {
        inside_set, values, ..
    } = &mut guards.atoms[0]
    else {
        unreachable!()
    };
    *inside_set = false;
    *values = vec![1].into_boxed_slice();
    assert_eq!(bound(&point(), &guards), Some(u64::MAX));
}

#[test]
fn guarded_index_rejects_wrapping_overflow_zero_divisor_and_unknown_types() {
    let empty = clause(&[]);
    for operation in [ReferenceBinaryOpV1::Add, ReferenceBinaryOpV1::Multiply] {
        let index = binary(operation, point(), c(2));
        assert_eq!(bound(&index, &empty), None);
        assert_eq!(bound(&index, &clause(&[(index.clone(), 16)])), None);
    }
    for operation in [ReferenceBinaryOpV1::Divide, ReferenceBinaryOpV1::Remainder] {
        assert_eq!(bound(&binary(operation, point(), c(0)), &empty), None);
        assert_eq!(
            bound(
                &binary(operation, point(), point()),
                &clause(&[(point(), 16)])
            ),
            None
        );
    }
    for scalar in [
        ReferenceScalarTypeV1::U32,
        ReferenceScalarTypeV1::I64,
        ReferenceScalarTypeV1::F64,
        ReferenceScalarTypeV1::Bool,
    ] {
        let value =
            ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar { scalar, bits: 1 });
        assert_eq!(bound(&value, &clause(&[(value.clone(), 16)])), None);
    }
    assert_eq!(bound(&c(u128::from(u64::MAX) + 1), &empty), None);
    let unknown = ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 };
    assert_eq!(bound(&unknown, &clause(&[(unknown.clone(), 16)])), None);
}

#[test]
fn guarded_index_checked_arithmetic_and_unsupported_operators_stay_closed() {
    let mut index = binary(ReferenceBinaryOpV1::Add, c(1), c(2));
    let ReferenceEffectExpressionV1::Binary { checked, .. } = &mut index else {
        unreachable!()
    };
    *checked = true;
    assert_eq!(bound(&index, &clause(&[])), None);
    for operation in [
        ReferenceBinaryOpV1::Subtract,
        ReferenceBinaryOpV1::ShiftLeft,
        ReferenceBinaryOpV1::BitAnd,
        ReferenceBinaryOpV1::LessThan,
    ] {
        assert_eq!(bound(&binary(operation, c(1), c(2)), &clause(&[])), None);
    }
}

#[test]
fn guarded_index_zero_upper_guard_does_not_invent_an_unreachable_path() {
    assert_eq!(bound(&point(), &clause(&[(point(), 0)])), Some(u64::MAX));
    assert_eq!(
        bound(&point(), &clause(&[(point(), u64::MAX)])),
        Some(u64::MAX - 1)
    );
}

#[test]
fn guarded_index_uses_the_callers_work_and_depth_limits() {
    let guards = clause(&[(point(), 16)]);
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    upper_bound(&point(), &guards, &mut work, 7, 0).unwrap();
    // Exhaust the existing owner, without replacing it after a successful query.
    assert!(
        work.charge_v2(crate::reference_effect_v1::MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
            .is_err()
    );
    assert!(upper_bound(&point(), &guards, &mut work, 7, 0).is_err());
    assert!(
        upper_bound(
            &point(),
            &guards,
            &mut ReferenceSymbolicWorkBudgetV2::default(),
            7,
            MAX_BOUND_DEPTH_V2
        )
        .is_err()
    );
}

#[test]
fn guarded_index_tile_bounds_cover_every_active_small_domain_point() {
    for divisor in [4, 8, 16, 64] {
        for stride in [1, 2, 4, 8, 16] {
            for batches in [1, 2, 3, 16] {
                let active_lanes = stride.min(divisor);
                let (batch, lane, index) = tiled(divisor, stride);
                let maximum =
                    bound(&index, &clause(&[(batch, batches), (lane, active_lanes)])).unwrap();
                let mut observed = 0;
                for p in 0..(batches * divisor + divisor) {
                    let batch = p / divisor;
                    let lane = p % divisor;
                    if batch < batches && lane < active_lanes {
                        let value = batch * stride + lane;
                        assert!(
                            value <= maximum,
                            "{p} / {divisor}, stride={stride}, batches={batches}"
                        );
                        observed = observed.max(value);
                    }
                }
                assert_eq!(maximum, observed);
            }
        }
    }
}
