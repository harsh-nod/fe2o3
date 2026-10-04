use super::*;

#[derive(Clone, Copy)]
enum Seed {
    SameDefinition,
    EqualDefinitions,
    Different,
    Dynamic,
    UnresolvedEntry,
}

fn transported(scalar: ScalarType, bound: u64, seed: Seed, checked: bool) -> Module {
    let mut module = fixture(scalar, Some((0, bound)), 1, checked);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::BOOL);
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(25));
    body.blocks[0].terminator = Some(condition(25, 201, 202));
    let alternative = match seed {
        Seed::SameDefinition => 9,
        Seed::EqualDefinitions | Seed::Different => {
            body.blocks[0].operations.push(literal_op(
                30,
                scalar,
                u64::from(matches!(seed, Seed::Different)),
            ));
            30
        }
        Seed::Dynamic => 0,
        Seed::UnresolvedEntry => {
            body.blocks[0]
                .parameters
                .push(ValueDef::new(ValueId(42), Type::Scalar(scalar)));
            42
        }
    };
    let first = if matches!(seed, Seed::UnresolvedEntry) {
        alternative
    } else {
        9
    };
    body.blocks[1].operations[0] = compare(4, 3, 23);
    body.blocks.extend([
        basic(
            201,
            vec![],
            vec![],
            branch(203, vec![ValueId(first), ValueId(10)]),
        ),
        basic(
            202,
            vec![],
            vec![],
            branch(203, vec![ValueId(alternative), ValueId(10)]),
        ),
        basic(
            203,
            vec![
                ValueDef::new(ValueId(22), Type::Scalar(scalar)),
                ValueDef::new(ValueId(23), Type::Scalar(scalar)),
            ],
            vec![],
            branch(11, vec![ValueId(22)]),
        ),
    ]);
    module
}

#[test]
fn induction_sparse_inputs_preserve_literal_counts_through_actual_preheader_phis() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for bound in [0, 1, 3, 8] {
            for seed in [Seed::SameDefinition, Seed::EqualDefinitions] {
                for checked in [false, true] {
                    with_facts(transported(scalar, bound, seed, checked), |facts, _| {
                        let fact = guarded(facts);
                        assert_eq!(fact.guard_distance(), Distance::Literal(bound));
                        assert_eq!(fact.iteration_scope(), Iterations::NormalHeaderCompletion);
                        assert_eq!(
                            fact.guarded_update(),
                            if bound == 0 {
                                Update::NoUpdate
                            } else {
                                Update::NonWrapping
                            }
                        );
                        assert_eq!(
                            facts.rows()[0].recurrence().initial(),
                            Definition::BlockArgument {
                                block: coordinate(6),
                                argument: 0
                            }
                        );
                        assert_eq!(
                            fact.bound(),
                            Definition::BlockArgument {
                                block: coordinate(6),
                                argument: 1
                            }
                        );
                        assert_eq!(facts.rows()[0].recurrence().overflow().is_some(), checked);
                    });
                }
            }
        }
    }
}

#[test]
fn induction_sparse_inputs_never_turn_conflicting_dynamic_or_unresolved_seeds_into_counts() {
    for seed in [Seed::Different, Seed::Dynamic, Seed::UnresolvedEntry] {
        with_facts(transported(ScalarType::U32, 3, seed, true), |facts, _| {
            assert_eq!(
                guarded(facts).guard_distance(),
                Distance::UnitStride {
                    initial: Definition::BlockArgument {
                        block: coordinate(6),
                        argument: 0
                    },
                    bound: Definition::BlockArgument {
                        block: coordinate(6),
                        argument: 1
                    },
                }
            );
            assert_eq!(guarded(facts).guarded_update(), Update::NonWrapping);
        });
    }
    let mut module = transported(ScalarType::U64, 3, Seed::SameDefinition, false);
    for block in &mut module.functions[0].body.as_mut().unwrap().blocks[4..6] {
        block.terminator = Some(branch(203, vec![ValueId(9), ValueId(1)]));
    }
    with_facts(module, |facts, _| {
        assert!(matches!(
            guarded(facts).guard_distance(),
            Distance::UnitStride { .. }
        ));
    });
}

#[test]
fn induction_sparse_inputs_use_exact_computed_constants_and_recheck_forged_rows() {
    let mut module = transported(ScalarType::U64, 8, Seed::SameDefinition, false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0]
        .operations
        .push(add(31, ScalarType::U64, 2, 2));
    for block in &mut body.blocks[4..6] {
        block.terminator = Some(branch(203, vec![ValueId(31), ValueId(10)]));
    }
    with_facts(module, |facts, budget| {
        assert_eq!(guarded(facts).guard_distance(), Distance::Literal(6));
        let original = facts.rows[0];
        for wrong in [
            Distance::Literal(8),
            Distance::Literal(0),
            Distance::UnitStride {
                initial: original.recurrence.initial(),
                bound: guarded(facts).bound(),
            },
        ] {
            let Outcome::Guarded(mut fact) = original.outcome else {
                unreachable!()
            };
            fact.distance = wrong;
            facts.rows[0].outcome = Outcome::Guarded(fact);
            let floor = budget.storage();
            assert_eq!(
                facts.replay(facts.loops(), Limits::default(), budget),
                Err(Error::ReplayMismatch)
            );
            assert_eq!(budget.storage(), floor);
            facts.rows[0] = original;
        }
        facts
            .replay(facts.loops(), Limits::default(), budget)
            .unwrap();
    });
}

#[test]
fn induction_sparse_inputs_do_not_upgrade_wrapping_larger_stride_or_abnormal_completion() {
    let mut module = transported(ScalarType::U8, 255, Seed::SameDefinition, false);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0] =
        literal_op(2, ScalarType::U8, 2);
    with_facts(module, |facts, _| {
        assert_eq!(
            facts.rows()[0].outcome(),
            Outcome::Unavailable(Unavailable::Arithmetic)
        );
    });
    let mut module = transported(ScalarType::U32, 3, Seed::SameDefinition, false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[2].terminator = Some(condition(25, 301, 100));
    body.blocks
        .push(basic(301, vec![], vec![], branch(11, vec![ValueId(5)])));
    with_facts(module, |facts, _| {
        assert_eq!(guarded(facts).guard_distance(), Distance::Literal(3));
        assert_eq!(guarded(facts).iteration_scope(), Iterations::Unavailable);
    });
}
