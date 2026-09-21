use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as Function, CheckedBinaryOperator, Constant,
    Function as KirFunction, Module, Operation as KirOperation, Signature, ValueDef,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
const LIMIT: usize = 100_000_000;
const FLOOR: usize = 43;
fn coordinate(block: u32) -> Block {
    Block {
        function: Function(0),
        block,
    }
}
fn branch(target: u32, arguments: Vec<ValueId>) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments,
    }
}
fn basic(
    id: u32,
    parameters: Vec<ValueDef>,
    operations: Vec<KirOperation>,
    terminator: Terminator,
) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.parameters = parameters;
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn constant(scalar: ScalarType, value: u64) -> Constant {
    match scalar {
        ScalarType::U8 => Constant::U8(value.try_into().unwrap()),
        ScalarType::U16 => Constant::U16(value.try_into().unwrap()),
        ScalarType::U32 => Constant::U32(value.try_into().unwrap()),
        ScalarType::U64 => Constant::U64(value),
        ScalarType::I32 => Constant::I32(value.try_into().unwrap()),
        _ => panic!("fixture width"),
    }
}
fn literal_op(id: u32, scalar: ScalarType, value: u64) -> KirOperation {
    KirOperation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(scalar)),
        OperationKind::Constant(constant(scalar, value)),
    )
}
fn compare(id: u32, lhs: u32, rhs: u32) -> KirOperation {
    KirOperation::effect_free(
        ValueDef::new(ValueId(id), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn condition(value: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(value),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn add(id: u32, scalar: ScalarType, lhs: u32, rhs: u32) -> KirOperation {
    KirOperation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(scalar)),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn fixture(scalar: ScalarType, literals: Option<(u64, u64)>, step: u64, checked: bool) -> Module {
    let ty = Type::Scalar(scalar);
    let mut entry = vec![literal_op(2, scalar, step)];
    let (initial, bound) = if let Some((a, b)) = literals {
        entry.extend([literal_op(9, scalar, a), literal_op(10, scalar, b)]);
        (9, 10)
    } else {
        (0, 1)
    };
    let update = if checked {
        KirOperation::checked_binary(
            ValueDef::new(ValueId(5), ty.clone()),
            ValueDef::new(ValueId(6), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(3),
            ValueId(2),
        )
    } else {
        add(5, scalar, 3, 2)
    };
    let mut module = Module::new("induction-facts");
    module.functions.push(KirFunction::internal_helper(
        "f",
        Signature::new(vec![ty.clone(), ty.clone()], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            basic(90, vec![], entry, branch(11, vec![ValueId(initial)])),
            basic(
                11,
                vec![ValueDef::new(ValueId(3), ty)],
                vec![compare(4, 3, bound)],
                condition(4, 70, 100),
            ),
            basic(70, vec![], vec![update], branch(11, vec![ValueId(5)])),
            basic(100, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
fn with_loops(module: Module, run: impl FnOnce(&Loops<'_, '_>, &mut Budget<'_>)) {
    let sibling = [0x83u8; FLOOR];
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (owner, a) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget.reserve_storage(a.retained_storage()).unwrap();
    let (inventory, b) = Inventory::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(b.retained_storage()).unwrap();
    let (loops, c) = Loops::derive(&inventory, Limits::default(), &mut budget).unwrap();
    budget.reserve_storage(c.retained_storage()).unwrap();
    let floor = budget.storage();
    run(&loops, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(loops);
    budget.release_storage(c.retained_storage()).unwrap();
    drop(inventory);
    budget.release_storage(b.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(a.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(sibling, [0x83; FLOOR]);
}
fn with_facts(
    module: Module,
    run: impl FnOnce(&mut CanonicalKirInductionFactsV1<'_, '_, '_>, &mut Budget<'_>),
) {
    with_loops(module, |loops, budget| {
        let floor = budget.storage();
        let (mut facts, receipt) =
            CanonicalKirInductionFactsV1::derive(loops, Limits::default(), budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            receipt.retained_storage(),
            size_of_val(&facts) + facts.rows.capacity() * size_of::<Row>()
        );
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        facts.replay(loops, Limits::default(), budget).unwrap();
        assert!(std::ptr::eq(facts.loops(), loops));
        assert!(!facts.grants_authority());
        run(&mut facts, budget);
        drop(facts);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}
use std::mem::size_of_val;
fn guarded(facts: &CanonicalKirInductionFactsV1<'_, '_, '_>) -> Fact {
    assert_eq!(facts.rows().len(), 1);
    let Outcome::Guarded(fact) = facts.rows()[0].outcome() else {
        panic!("real supported induction")
    };
    fact
}

#[test]
fn unit_stride_unsigned_widths_checked_and_plain_have_symbolic_not_executed_counts() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for checked in [false, true] {
            with_facts(fixture(scalar, None, 1, checked), |facts, _| {
                let fact = guarded(facts);
                assert_eq!(fact.guarded_update(), Update::NonWrapping);
                assert_eq!(
                    fact.guard_distance(),
                    Distance::UnitStride {
                        initial: Definition::FunctionArgument {
                            function: Function(0),
                            argument: 0
                        },
                        bound: Definition::FunctionArgument {
                            function: Function(0),
                            argument: 1
                        },
                    }
                );
                assert_eq!(fact.iteration_scope(), Iterations::NormalHeaderCompletion);
                assert_eq!(
                    fact.guard(),
                    Operation {
                        block: coordinate(1),
                        operation: 0
                    }
                );
                assert_eq!(
                    fact.body_edge(),
                    Edge {
                        source: coordinate(1),
                        successor: 0
                    }
                );
                assert_eq!(
                    fact.exit_edge(),
                    Edge {
                        source: coordinate(1),
                        successor: 1
                    }
                );
                assert_eq!(facts.rows()[0].recurrence().overflow().is_some(), checked);
            });
        }
    }
}
#[test]
fn literal_zero_one_many_nondivisible_and_maximum_bounds_have_exact_distances() {
    for (a, b, step, n) in [
        (7, 3, 1, 0),
        (3, 3, 8, 0),
        (3, 4, 1, 1),
        (2, 11, 3, 3),
        (2, 12, 3, 4),
        (254, 255, 1, 1),
        (0, 255, 1, 255),
    ] {
        with_facts(
            fixture(ScalarType::U8, Some((a, b)), step, false),
            |facts, _| {
                let fact = guarded(facts);
                assert_eq!(fact.guard_distance(), Distance::Literal(n));
                assert_eq!(
                    fact.guarded_update(),
                    if n == 0 {
                        Update::NoUpdate
                    } else {
                        Update::NonWrapping
                    }
                );
                assert_eq!(fact.iteration_scope(), Iterations::NormalHeaderCompletion);
                // N counts updates. The terminating header condition is the N+1th.
                let mut value = a;
                let mut updates = 0;
                let mut checks = 0;
                loop {
                    checks += 1;
                    if value >= b {
                        break;
                    }
                    value += step;
                    updates += 1;
                }
                assert_eq!((updates, checks), (n, n + 1));
            },
        );
    }
    with_facts(
        fixture(ScalarType::U64, Some((u64::MAX - 1, u64::MAX)), 1, true),
        |facts, _| {
            assert_eq!(guarded(facts).guard_distance(), Distance::Literal(1));
        },
    );
}
#[test]
fn unsupported_signed_dynamic_stride_and_literal_wrap_are_explicit_outcomes() {
    for (module, reason) in [
        (
            fixture(ScalarType::I32, None, 1, false),
            Unavailable::Scalar,
        ),
        (
            fixture(ScalarType::U64, None, 2, false),
            Unavailable::Arithmetic,
        ),
        (
            fixture(ScalarType::U8, Some((254, 255)), 2, false),
            Unavailable::Arithmetic,
        ),
    ] {
        with_facts(module, |facts, _| {
            assert_eq!(facts.rows().len(), 1);
            assert_eq!(facts.rows()[0].outcome(), Outcome::Unavailable(reason));
        });
    }
}
#[test]
fn a_guard_block_update_is_not_misrepresented_as_after_the_taken_edge() {
    let mut module = fixture(ScalarType::U32, None, 1, false);
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    let update = blocks[2].operations.remove(0);
    blocks[1].operations.push(update);
    with_facts(module, |facts, _| {
        assert_eq!(
            facts.rows()[0].outcome(),
            Outcome::Unavailable(Unavailable::Control)
        );
    });
}
#[test]
fn normal_early_exit_and_internal_cycle_keep_guarded_no_wrap_but_not_completion() {
    for cycle in [false, true] {
        let mut module = fixture(ScalarType::U32, None, 1, false);
        module.functions[0].signature.parameters.push(Type::BOOL);
        let body = module.functions[0].body.as_mut().unwrap();
        body.parameters.push(ValueId(20));
        body.blocks[2].terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(20),
            then_target: BlockId(77),
            then_arguments: vec![],
            else_target: BlockId(if cycle { 70 } else { 100 }),
            else_arguments: vec![],
        });
        body.blocks
            .push(basic(77, vec![], vec![], branch(11, vec![ValueId(5)])));
        with_facts(module, |facts, _| {
            let fact = guarded(facts);
            assert_eq!(fact.guarded_update(), Update::NonWrapping);
            assert_eq!(fact.iteration_scope(), Iterations::Unavailable);
        });
    }
}
#[test]
fn mutated_guard_bound_polarity_distance_scope_and_complete_row_order_are_rejected() {
    let mut module = fixture(ScalarType::U32, None, 1, false);
    let mut second = module.functions[0].clone();
    second.id = fe2o3_kernel_ir::FunctionId::new("second");
    module.functions.push(second);
    with_facts(module, |facts, budget| {
        assert_eq!(facts.rows.len(), 2);
        let original = facts.rows.clone();
        for mode in 0..7 {
            facts.rows.clone_from(&original);
            if mode == 0 {
                facts.rows.swap(0, 1);
            } else if mode == 1 {
                facts.rows.pop();
            } else {
                let initial = facts.rows[0].recurrence.initial;
                let Outcome::Guarded(ref mut fact) = facts.rows[0].outcome else {
                    panic!("guarded")
                };
                match mode {
                    2 => fact.bound = initial,
                    3 => fact.body.successor = 1,
                    4 => fact.distance = Distance::Literal(0),
                    5 => fact.iterations = Iterations::Unavailable,
                    _ => fact.guard.operation += 1,
                }
            }
            assert!(matches!(
                facts.replay(facts.loops, Limits::default(), budget),
                Err(Error::ReplayMismatch)
            ));
        }
        facts.rows = original;
        facts
            .replay(facts.loops, Limits::default(), budget)
            .unwrap();
    });
}
#[test]
fn actual_guard_polarity_and_forwarded_recurrence_never_become_empty_positive_proofs() {
    let mut module = fixture(ScalarType::U32, None, 1, false);
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    blocks[1].terminator = Some(condition(4, 100, 70));
    with_facts(module, |facts, _| {
        assert_eq!(
            facts.rows()[0].outcome(),
            Outcome::Unavailable(Unavailable::Guard)
        );
    });
    let mut module = fixture(ScalarType::U32, None, 1, false);
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    blocks[2]
        .parameters
        .push(ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32)));
    if let Some(Terminator::ConditionalBranch { then_arguments, .. }) = &mut blocks[1].terminator {
        then_arguments.push(ValueId(3));
    }
    if let OperationKind::Binary { lhs, .. } = &mut blocks[2].operations[0].kind {
        *lhs = ValueId(21);
    }
    with_facts(module, |facts, _| {
        assert_eq!(facts.loops.loop_count(), 1);
        assert!(facts.rows().is_empty());
    });
}
#[test]
fn a_forwarded_header_bound_is_not_claimed_to_be_an_outside_invariant() {
    let mut module = fixture(ScalarType::U32, None, 1, false);
    let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
    blocks[1]
        .parameters
        .push(ValueDef::new(ValueId(23), Type::Scalar(ScalarType::U32)));
    blocks[0].terminator = Some(branch(11, vec![ValueId(0), ValueId(1)]));
    blocks[2].terminator = Some(branch(11, vec![ValueId(5), ValueId(23)]));
    if let OperationKind::Compare { rhs, .. } = &mut blocks[1].operations[0].kind {
        *rhs = ValueId(23);
    }
    with_facts(module, |facts, _| {
        assert_eq!(facts.rows().len(), 1);
        assert_eq!(
            facts.rows()[0].outcome(),
            Outcome::Unavailable(Unavailable::Bound)
        );
    });
}
#[test]
fn borrowed_report_rejects_equal_foreign_loop_owner_and_deterministically_repeats() {
    with_facts(fixture(ScalarType::U32, None, 1, false), |facts, budget| {
        with_loops(fixture(ScalarType::U32, None, 1, false), |foreign, _| {
            assert!(matches!(
                facts.replay(foreign, Limits::default(), budget),
                Err(Error::ForeignLoops)
            ));
        });
        let (other, receipt) =
            CanonicalKirInductionFactsV1::derive(facts.loops, Limits::default(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(facts.rows(), other.rows());
        other
            .replay(facts.loops, Limits::default(), budget)
            .unwrap();
        drop(other);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn duplicate_internal_edge_occurrences_keep_exact_acyclic_completion() {
    let mut module = fixture(ScalarType::U32, None, 1, false);
    module.functions[0].signature.parameters.push(Type::BOOL);
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[2].terminator = Some(condition(20, 77, 77));
    body.blocks
        .push(basic(77, vec![], vec![], branch(11, vec![ValueId(5)])));
    with_facts(module, |facts, _| {
        assert_eq!(
            guarded(facts).iteration_scope(),
            Iterations::NormalHeaderCompletion
        );
        assert_eq!(
            facts.rows()[0].recurrence().backedge().source,
            coordinate(4)
        );
    });
}

#[test]
fn nested_loop_has_inner_completion_without_promising_outer_body_acyclicity() {
    let scalar = ScalarType::U64;
    let ty = Type::Scalar(scalar);
    let mut module = Module::new("nested-induction");
    module.functions.push(KirFunction::internal_helper(
        "nested",
        Signature::new(vec![ty.clone(); 3], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![
            basic(
                90,
                vec![],
                vec![literal_op(10, scalar, 0), literal_op(11, scalar, 1)],
                branch(11, vec![ValueId(0)]),
            ),
            basic(
                11,
                vec![ValueDef::new(ValueId(3), ty.clone())],
                vec![compare(5, 3, 1)],
                condition(5, 30, 100),
            ),
            basic(30, vec![], vec![], branch(40, vec![ValueId(10)])),
            basic(
                40,
                vec![ValueDef::new(ValueId(4), ty)],
                vec![compare(6, 4, 2)],
                condition(6, 50, 60),
            ),
            basic(
                50,
                vec![],
                vec![add(8, scalar, 4, 11)],
                branch(40, vec![ValueId(8)]),
            ),
            basic(
                60,
                vec![],
                vec![add(7, scalar, 3, 11)],
                branch(11, vec![ValueId(7)]),
            ),
            basic(100, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    with_facts(module, |facts, _| {
        assert_eq!(facts.rows().len(), 2);
        let scopes: Vec<_> = facts
            .rows()
            .iter()
            .map(|row| {
                let Outcome::Guarded(fact) = row.outcome() else {
                    panic!("nested guarded recurrence")
                };
                assert_eq!(fact.guarded_update(), Update::NonWrapping);
                (row.loop_ordinal(), fact.iteration_scope())
            })
            .collect();
        assert_eq!(
            scopes,
            [
                (0, Iterations::Unavailable),
                (1, Iterations::NormalHeaderCompletion)
            ]
        );
    });
}

#[path = "canonical_kir_loop_induction_resources_v1_tests.rs"]
mod resources_tests;
