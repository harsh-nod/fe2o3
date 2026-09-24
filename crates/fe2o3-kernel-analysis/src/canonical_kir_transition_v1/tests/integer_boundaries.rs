use super::*;
use fe2o3_kernel_ir::ComparePredicate as Predicate;

fn extrema() -> [(Constant, Constant); 8] {
    [
        (Constant::U8(0), Constant::U8(u8::MAX)),
        (Constant::U16(0), Constant::U16(u16::MAX)),
        (Constant::U32(0), Constant::U32(u32::MAX)),
        (Constant::U64(0), Constant::U64(u64::MAX)),
        (Constant::I8(i8::MIN), Constant::I8(i8::MAX)),
        (Constant::I16(i16::MIN), Constant::I16(i16::MAX)),
        (Constant::I32(i32::MIN), Constant::I32(i32::MAX)),
        (Constant::I64(i64::MIN), Constant::I64(i64::MAX)),
    ]
}

fn source(bound: Constant, predicate: Predicate, constant_left: bool) -> Module {
    module(
        vec![bound.ty()],
        vec![Type::BOOL],
        vec![1],
        vec![returning(
            11,
            vec![
                constant(2, bound),
                value(
                    3,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate,
                        lhs: ValueId(if constant_left { 2 } else { 1 }),
                        rhs: ValueId(if constant_left { 1 } else { 2 }),
                    },
                ),
            ],
            &[3],
        )],
    )
}

fn folded(
    input: Module,
    claimed: bool,
    check: impl FnOnce(&Inventory<'_>, &Inventory<'_>, &Rows, usize),
) {
    let output = module(
        input.functions[0].signature.parameters.clone(),
        vec![Type::BOOL],
        vec![1],
        vec![returning(
            11,
            vec![constant(30, Constant::Bool(claimed))],
            &[30],
        )],
    );
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, None)]],
                operations: vec![Origin::ConstantFrom(result(0, 1, 0))],
                relations: vec![(1, 1, R), (3, 30, S)],
                uses: vec![term(0, 0)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| check(a, b, rows, floor),
    );
}

#[test]
fn integer_boundary_comparisons_check_all_signed_unsigned_extrema_and_operand_orders() {
    for (minimum, maximum) in extrema() {
        for (bound, predicate, left, expected) in [
            (minimum.clone(), Predicate::LessThan, false, false),
            (minimum.clone(), Predicate::GreaterThanOrEqual, false, true),
            (maximum.clone(), Predicate::GreaterThan, false, false),
            (maximum.clone(), Predicate::LessThanOrEqual, false, true),
            (minimum.clone(), Predicate::GreaterThan, true, false),
            (minimum, Predicate::LessThanOrEqual, true, true),
            (maximum.clone(), Predicate::LessThan, true, false),
            (maximum, Predicate::GreaterThanOrEqual, true, true),
        ] {
            for claimed in [expected, !expected] {
                folded(
                    source(bound.clone(), predicate, left),
                    claimed,
                    |a, b, rows, floor| {
                        if claimed == expected {
                            accepted(a, b, rows, floor);
                        } else {
                            assert_eq!(
                                rejected(a, b, rows, floor),
                                Error::Rule("constant synthesis bits")
                            );
                        }
                    },
                );
            }
        }
    }
}

#[test]
fn integer_boundary_comparisons_reject_interior_signed_zero_float_and_index_substitutions() {
    for (bound, predicate) in [
        (Constant::U8(1), Predicate::LessThan),
        (Constant::U64(u64::MAX - 1), Predicate::GreaterThan),
        (Constant::I8(0), Predicate::LessThan),
        (Constant::I64(0), Predicate::GreaterThanOrEqual),
        (Constant::F32Bits(0), Predicate::LessThan),
        (Constant::F64Bits(0), Predicate::GreaterThanOrEqual),
        (Constant::Index(0), Predicate::LessThan),
        (Constant::U32(0), Predicate::Equal),
        (Constant::U32(u32::MAX), Predicate::NotEqual),
        (Constant::Bool(false), Predicate::Equal),
    ] {
        for claimed in [false, true] {
            folded(
                source(bound.clone(), predicate, false),
                claimed,
                |a, b, rows, floor| {
                    assert_eq!(
                        rejected(a, b, rows, floor),
                        Error::Rule("constant synthesis bits")
                    );
                },
            );
        }
    }
}

#[test]
fn integer_boundary_false_guard_can_merge_only_its_exact_exit_occurrence() {
    let mut input = source(Constant::U32(0), Predicate::LessThan, false);
    input.functions[0].signature.results.clear();
    let body = input.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    body.blocks.push(returning(20, vec![], &[]));
    body.blocks.push(returning(30, vec![], &[]));
    let output = module(vec![U32], vec![], vec![1], vec![returning(11, vec![], &[])]);
    inspect(
        input,
        output,
        |a, b| {
            Plan {
                chains: vec![vec![(0, Some(edge(0, 1))), (2, None)]],
                relations: vec![(1, 1, R)],
                ..Plan::default()
            }
            .rows(a, b)
        },
        |a, b, rows, floor| {
            accepted(a, b, rows, floor);
            rows.segments[0].connector = Some(edge(0, 0));
            assert_eq!(rejected(a, b, rows, floor), Error::Rule("merge connector"));
        },
    );
}

#[test]
fn integer_boundary_comparison_replay_has_exact_work_and_preserves_storage_floor() {
    folded(
        source(Constant::U64(0), Predicate::LessThan, false),
        false,
        |a, b, rows, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, floor + LIMIT);
            budget.reserve_storage(floor).unwrap();
            {
                let (_checked, _) =
                    check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget).unwrap();
            }
            let required = budget.work();
            let peak = budget.peak_storage();
            assert_eq!(budget.storage(), floor);
            for allowance in [required, required - 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(allowance);
                let mut budget = Budget::new(&mut work, peak);
                budget.reserve_storage(floor).unwrap();
                let checked =
                    check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget);
                if allowance == required {
                    assert!(checked.is_ok());
                } else {
                    assert!(matches!(checked, Err(Error::Resource(Resource::Work(_)))));
                }
                assert_eq!(budget.work(), allowance);
                assert_eq!(budget.peak_storage(), peak);
                assert_eq!(budget.storage(), floor);
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, peak - 1);
            budget.reserve_storage(floor).unwrap();
            assert!(
                matches!(check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget),
            Err(Error::Resource(Resource::Storage(error))) if error.actual() == peak && error.limit() == peak - 1)
            );
            assert_eq!(budget.failed_storage(), Some(peak));
            assert!(budget.peak_storage() < peak);
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn integer_boundary_comparisons_cover_128_bit_extrema_without_width_sized_shifts() {
    use super::super::values::integer_boundary_comparison as compare;
    for (ty, minimum, maximum) in [
        (ScalarType::U128, 0, u128::MAX),
        (ScalarType::I128, 1u128 << 127, (1u128 << 127) - 1),
    ] {
        for (predicate, bound, left, expected) in [
            (Predicate::LessThan, minimum, false, false),
            (Predicate::GreaterThanOrEqual, minimum, false, true),
            (Predicate::GreaterThan, maximum, false, false),
            (Predicate::LessThanOrEqual, maximum, false, true),
            (Predicate::GreaterThan, minimum, true, false),
            (Predicate::LessThanOrEqual, minimum, true, true),
            (Predicate::LessThan, maximum, true, false),
            (Predicate::GreaterThanOrEqual, maximum, true, true),
        ] {
            let literal = Some(Literal { ty, bits: bound });
            assert_eq!(
                compare(
                    ty,
                    predicate,
                    if left { literal } else { None },
                    if left { None } else { literal }
                ),
                Some(expected)
            );
        }
        for predicate in [Predicate::Equal, Predicate::NotEqual] {
            assert_eq!(
                compare(ty, predicate, None, Some(Literal { ty, bits: minimum })),
                None
            );
        }
    }
    assert_eq!(
        compare(
            ScalarType::U8,
            Predicate::LessThan,
            None,
            Some(Literal {
                ty: ScalarType::U64,
                bits: 0
            })
        ),
        None
    );
    assert_eq!(
        compare(
            ScalarType::U8,
            Predicate::GreaterThan,
            None,
            Some(Literal {
                ty: ScalarType::U8,
                bits: 511
            })
        ),
        None
    );
}
