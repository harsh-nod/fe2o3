fn staging_bounds() -> (ResolvedReferenceBoundsCheckV1, ReferenceGuardClauseV1) {
    let mut check = check();
    let point = check.index.clone();
    let batch = binary(ReferenceBinaryOpV1::Divide, point.clone(), constant(64));
    let lane = binary(ReferenceBinaryOpV1::Remainder, point, constant(64));
    check.index = binary(
        ReferenceBinaryOpV1::Add,
        binary(ReferenceBinaryOpV1::Multiply, batch.clone(), constant(16)),
        lane.clone(),
    );
    check.condition = binary(
        ReferenceBinaryOpV1::LessThan,
        check.index.clone(),
        check.length.clone(),
    );
    let guards = ReferenceGuardClauseV1 {
        atoms: vec![
            reference_boolean_guard_atom_v1(
                binary(
                    ReferenceBinaryOpV1::Equal,
                    check.length.clone(),
                    constant(256),
                ),
                true,
            ),
            reference_boolean_guard_atom_v1(
                binary(ReferenceBinaryOpV1::LessThan, batch, constant(16)),
                true,
            ),
            reference_boolean_guard_atom_v1(
                binary(ReferenceBinaryOpV1::LessThan, lane, constant(16)),
                true,
            ),
        ]
        .into_boxed_slice(),
    };
    (check, guards)
}

#[test]
fn output_slice_staging_arithmetic_reaches_the_real_path_bounds_consumer() {
    let (check, guards) = staging_bounds();
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    assert!(
        cpu_path_proves_bound_v2(
            &ReferencePathPredicateV1 {
                clauses: vec![guards.clone()].into_boxed_slice(),
            },
            &check,
            &mut work
        )
        .unwrap()
    );
    for omitted in 0..3 {
        let mut incomplete = guards.atoms.to_vec();
        incomplete.remove(omitted);
        let path = ReferencePathPredicateV1 {
            clauses: vec![
                guards.clone(),
                ReferenceGuardClauseV1 {
                    atoms: incomplete.into_boxed_slice(),
                },
            ]
            .into_boxed_slice(),
        };
        assert!(
            !cpu_path_proves_bound_v2(&path, &check, &mut work).unwrap(),
            "missing atom {omitted}"
        );
    }
}

#[test]
fn output_slice_staging_bounds_do_not_substitute_another_slice_or_the_assertion() {
    let (check, guards) = staging_bounds();
    let mut other = check.clone();
    other.length = ReferenceEffectExpressionV1::InputLength {
        reference_argument: 2,
    };
    other.condition = binary(
        ReferenceBinaryOpV1::LessThan,
        other.index.clone(),
        other.length.clone(),
    );
    let path = ReferencePathPredicateV1 {
        clauses: vec![guards].into_boxed_slice(),
    };
    assert!(
        !cpu_path_proves_bound_v2(&path, &other, &mut ReferenceSymbolicWorkBudgetV2::default())
            .unwrap()
    );
    assert!(
        !cpu_path_proves_bound_v2(
            &ReferencePathPredicateV1::unconditional_v1(),
            &check,
            &mut ReferenceSymbolicWorkBudgetV2::default()
        )
        .unwrap()
    );
}

#[test]
fn output_slice_staging_bounds_exhaustion_remains_fatal_after_a_success() {
    let (check, guards) = staging_bounds();
    let path = ReferencePathPredicateV1 {
        clauses: vec![guards].into_boxed_slice(),
    };
    let mut work = ReferenceSymbolicWorkBudgetV2::default();
    assert!(cpu_path_proves_bound_v2(&path, &check, &mut work).unwrap());
    assert!(
        work.charge_v2(crate::reference_effect_v1::MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
            .is_err()
    );
    assert!(cpu_path_proves_bound_v2(&path, &check, &mut work).is_err());
}
