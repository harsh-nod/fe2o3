//! Constructed independent relation tests, not raw capture or source evidence.
use super::*;
use crate::canonical_kir_transition_v1::commutative_bitwise_cse::{
    CanonicalKirCommutativeBitwiseCseErrorV1 as CseError,
    check_canonical_kir_commutative_bitwise_cse_v1 as check,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKirControlFlowScopeErrorV1 as CfgError,
    MemoryAccess,
};

fn expression(id: u32, ty: Type, kind: BinaryOp, a: u32, b: u32) -> Operation {
    value(
        id,
        ty,
        OperationKind::Binary {
            op: kind,
            lhs: ValueId(a),
            rhs: ValueId(b),
        },
    )
}
fn store(value: u32, width: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(99),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Global, width / 8),
        },
    )
}
fn pair(ty: ScalarType, kind: BinaryOp, swapped: bool, cross: bool) -> (Module, Module, Plan) {
    let scalar = Type::Scalar(ty);
    let parameters = vec![
        Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
        scalar.clone(),
        scalar.clone(),
    ];
    let anchor = expression(1, scalar.clone(), kind, 8, 9);
    let duplicate = expression(
        2,
        scalar.clone(),
        kind,
        if swapped { 9 } else { 8 },
        if swapped { 8 } else { 9 },
    );
    let width = u32::from(ty.bit_width().unwrap());
    let (input, output, origins, uses, chains, edges) = if cross {
        let mut entry = BasicBlock::new(BlockId(10));
        entry.operations.push(anchor);
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(20),
            arguments: vec![],
        });
        (
            vec![
                entry.clone(),
                returning(20, vec![duplicate, store(2, width)], &[2]),
            ],
            vec![entry, returning(20, vec![store(1, width)], &[1])],
            vec![Origin::Retained(op(0, 0)), Origin::Retained(op(1, 1))],
            vec![
                operand(0, 0, 0),
                operand(0, 0, 1),
                operand(1, 1, 0),
                operand(1, 1, 1),
                term(1, 0),
            ],
            vec![vec![(0, None)], vec![(1, None)]],
            vec![edge(0, 0)],
        )
    } else {
        (
            vec![returning(
                10,
                vec![anchor.clone(), duplicate, store(2, width)],
                &[2],
            )],
            vec![returning(10, vec![anchor, store(1, width)], &[1])],
            vec![Origin::Retained(op(0, 0)), Origin::Retained(op(0, 2))],
            vec![
                operand(0, 0, 0),
                operand(0, 0, 1),
                operand(0, 2, 0),
                operand(0, 2, 1),
                term(0, 0),
            ],
            vec![vec![(0, None)]],
            vec![],
        )
    };
    (
        module(
            parameters.clone(),
            vec![scalar.clone()],
            vec![99, 8, 9],
            input,
        ),
        module(parameters, vec![scalar], vec![99, 8, 9], output),
        Plan {
            chains,
            operations: origins,
            relations: vec![(99, 99, R), (8, 8, R), (9, 9, R), (1, 1, R), (2, 1, S)],
            uses,
            edges,
            ..Plan::default()
        },
    )
}
fn run(
    a: &Inventory<'_>,
    b: &Inventory<'_>,
    rows: &Rows,
    floor: usize,
) -> std::result::Result<(usize, usize, usize), CseError> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + LIMIT);
    budget.reserve_storage(floor).unwrap();
    let result = (|| {
        let (pairs, retained) = {
            let (view, storage) = check(a, b, rows.candidate(), &mut budget)?;
            assert!(std::ptr::eq(view.input(), a));
            assert!(std::ptr::eq(view.output(), b));
            assert!(!view.grants_authority());
            assert_eq!(view.rows().definitions.len(), a.definitions().len());
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(storage.retained_storage()).unwrap();
            (view.proved_pairs(), storage.retained_storage())
        };
        budget.release_storage(retained).unwrap();
        Ok((pairs, budget.work(), budget.peak_storage()))
    })();
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn all_fixed_types_three_ops_same_and_cross_block_keep_the_old_relation_positional() {
    for ty in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        for kind in [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor] {
            for cross in [false, true] {
                for swapped in [false, true] {
                    let (input, output, plan) = pair(ty, kind, swapped, cross);
                    inspect(
                        input,
                        output,
                        |a, b| plan.rows(a, b),
                        |a, b, rows, floor| {
                            assert_eq!(run(a, b, rows, floor).unwrap().0, 1);
                            if swapped {
                                assert!(matches!(rejected(a, b, rows, floor), Error::Rule(_)));
                            } else {
                                accepted(a, b, rows, floor);
                            }
                        },
                    );
                }
            }
        }
    }
}

fn chain(count: u32) -> (Module, Module, Plan) {
    let mut first = BasicBlock::new(BlockId(10));
    let mut second = BasicBlock::new(BlockId(20));
    let mut plan = Plan {
        chains: vec![vec![(0, None)], vec![(1, None)]],
        relations: vec![(8, 8, R), (9, 9, R)],
        edges: vec![edge(0, 0)],
        ..Plan::default()
    };
    let (mut a, mut b) = (8, 8);
    for index in 0..count {
        let (anchor, removed) = (100 + index, 10_000 + index);
        first
            .operations
            .push(expression(anchor, U32, BinaryOp::BitXor, a, 9));
        second
            .operations
            .push(expression(removed, U32, BinaryOp::BitXor, 9, b));
        plan.operations.push(Origin::Retained(op(0, index)));
        plan.relations
            .extend([(anchor, anchor, R), (removed, anchor, S)]);
        plan.uses
            .extend([operand(0, index, 0), operand(0, index, 1)]);
        a = anchor;
        b = removed;
    }
    first.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![],
    });
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(b)],
    });
    plan.uses.push(term(1, 0));
    (
        module(
            vec![U32, U32],
            vec![U32],
            vec![8, 9],
            vec![first.clone(), second],
        ),
        module(
            vec![U32, U32],
            vec![U32],
            vec![8, 9],
            vec![first, returning(20, vec![], &[a])],
        ),
        plan,
    )
}

#[test]
fn deep_chains_have_one_grounded_pair_each_and_linear_new_work() {
    for count in [1, 8, 64, 256] {
        let (input, output, plan) = chain(count);
        inspect(
            input,
            output,
            |a, b| plan.rows(a, b),
            |a, b, rows, floor| {
                let (pairs, work, peak) = run(a, b, rows, floor).unwrap();
                assert_eq!(pairs, count as usize);
                assert!(work <= 10_000 + 2_000 * count as usize, "{count}: {work}");
                assert!(peak - floor <= 10_000 + 512 * count as usize);
                let identity = Rows::identity(b);
                assert_eq!(
                    run(b, b, &identity, floor + identity.storage()).unwrap().0,
                    0
                );
            },
        );
    }
}

fn reversed_block_chain(count: u32) -> (Module, Module, Plan) {
    assert!(count >= 2);
    let mut entry = BasicBlock::new(BlockId(10));
    let mut plan = Plan {
        chains: (0..=count).map(|block| vec![(block, None)]).collect(),
        relations: vec![(8, 8, R), (9, 9, R)],
        edges: vec![edge(0, 0)],
        ..Plan::default()
    };
    for index in 0..count {
        entry.operations.push(expression(
            100 + index,
            U32,
            BinaryOp::BitXor,
            if index == 0 { 8 } else { 100 + index - 1 },
            9,
        ));
        plan.operations.push(Origin::Retained(op(0, index)));
        plan.relations.extend([
            (100 + index, 100 + index, R),
            (10_000 + index, 100 + index, S),
        ]);
        plan.uses
            .extend([operand(0, index, 0), operand(0, index, 1)]);
    }
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![],
    });
    let mut before = vec![entry.clone()];
    let mut after = vec![entry];
    // Execution visits 20, 21, ...; physical storage visits the reverse order.
    // The first omitted definition therefore depends on every later one.
    for index in (0..count).rev() {
        let mut source = BasicBlock::new(BlockId(20 + index));
        source.operations.push(expression(
            10_000 + index,
            U32,
            BinaryOp::BitXor,
            9,
            if index == 0 { 8 } else { 10_000 + index - 1 },
        ));
        let mut target = BasicBlock::new(source.id);
        if index + 1 == count {
            source.terminator = Some(Terminator::Return {
                values: vec![ValueId(10_000 + index)],
            });
            target.terminator = Some(Terminator::Return {
                values: vec![ValueId(100 + index)],
            });
            plan.uses.push(term(1, 0));
        } else {
            source.terminator = Some(Terminator::Branch {
                target: BlockId(20 + index + 1),
                arguments: vec![],
            });
            target.terminator = source.terminator.clone();
            plan.edges.push(edge(count - index, 0));
        }
        before.push(source);
        after.push(target);
    }
    (
        module(vec![U32, U32], vec![U32], vec![8, 9], before),
        module(vec![U32, U32], vec![U32], vec![8, 9], after),
        plan,
    )
}

fn assert_reverse_dependencies(input: &Inventory<'_>, count: usize) {
    assert_eq!(input.blocks().len(), count + 1);
    assert_eq!(input.operations().len(), 2 * count);
    assert_eq!(input.definitions().len(), 2 + 2 * count);
    for offset in 0..count {
        let definition = 2 + count + offset;
        let operation = &input.operations()[count + offset];
        assert_eq!(operation.coordinate, op((offset + 1) as u32, 0));
        assert_eq!(
            input.definitions()[definition].coordinate,
            result((offset + 1) as u32, 0, 0)
        );
        assert_eq!(
            input.definitions()[definition].value,
            Some(ValueId(10_000 + (count - offset - 1) as u32))
        );
        let child = input.uses()[operation.operands.start + 1].definition;
        assert_eq!(
            child,
            if offset + 1 == count {
                0
            } else {
                definition + 1
            }
        );
    }
}

#[test]
fn reversed_physical_blocks_force_nested_pairs_with_exact_and_short_resources() {
    for count in [2, 8, 32] {
        let (input, output, plan) = reversed_block_chain(count);
        inspect(
            input,
            output,
            |a, b| plan.rows(a, b),
            |a, b, rows, floor| {
                assert_reverse_dependencies(a, count as usize);
                assert!(a.owner().canonical().identity() != b.owner().canonical().identity());
                assert_eq!(b.operations().len(), count as usize);
                assert_eq!(
                    rows.definition_outputs
                        .iter()
                        .filter(|row| row.kind == S)
                        .count(),
                    count as usize
                );
                let (pairs, work_bound, peak) = run(a, b, rows, floor).unwrap();
                assert_eq!(pairs, count as usize);
                // The first unresolved definition's dependency chain has the
                // asserted depth above; none was visited earlier by prove().
                for (work_limit, storage_limit, succeeds) in [
                    (work_bound, peak, true),
                    (work_bound - 1, peak, false),
                    (work_bound, peak - 1, false),
                ] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                    let mut budget = Budget::new(&mut work, storage_limit);
                    budget.reserve_storage(floor).unwrap();
                    let ledger = budget.work_ledger_identity_v1();
                    let checked = check(a, b, rows.candidate(), &mut budget);
                    assert_eq!(checked.is_ok(), succeeds);
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    match checked {
                        Ok((view, _)) => {
                            assert_eq!(view.proved_pairs(), count as usize);
                            assert_eq!(budget.work(), work_bound);
                            assert_eq!(budget.peak_storage(), peak);
                        }
                        Err(error) => assert!(matches!(
                            error,
                            CseError::Resource(_)
                                | CseError::ControlFlow(CfgError::Resource(_))
                                | CseError::Transition(Error::Resource(_))
                        )),
                    }
                }
                let identity = Rows::identity(b);
                assert_eq!(
                    run(b, b, &identity, floor + identity.storage()).unwrap().0,
                    0
                );
            },
        );
    }
}

#[test]
fn deepest_reverse_dependency_failure_restores_the_shared_floor() {
    let count = 32;
    let (mut input, output, plan) = reversed_block_chain(count);
    // Change only the bottom omitted expression, reached after the other
    // unresolved definitions have been pushed. No cyclic candidate is needed.
    input.functions[0].body.as_mut().unwrap().blocks[count as usize].operations[0] =
        expression(10_000, U32, BinaryOp::BitXor, 9, 9);
    inspect(
        input,
        output,
        |a, b| plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                run(a, b, rows, floor).unwrap_err(),
                CseError::Rule("unproved exact-or-swapped operand pair")
            );
        },
    );
}

#[test]
fn hostile_rows_cannot_supply_equivalence_or_change_origin_order() {
    for hostile in 0..10 {
        let (input, output, plan) = pair(ScalarType::U32, BinaryOp::BitAnd, true, true);
        inspect(
            input,
            output,
            |a, b| plan.rows(a, b),
            |a, b, rows, floor| {
                run(a, b, rows, floor).unwrap();
                match hostile {
                    0 => {
                        rows.definition_outputs.pop();
                    }
                    1 => {
                        rows.definitions[0].outputs = rows.definitions[1].outputs;
                    }
                    2 => {
                        rows.definition_outputs[0].kind = S;
                    }
                    3 => {
                        rows.operations[0].origin = Origin::ConstantFrom(result(0, 0, 0));
                    }
                    4 => {
                        rows.operations[1].origin = rows.operations[0].origin;
                    }
                    5 => {
                        rows.uses[0].input = operand(1, 0, 0);
                    }
                    6 => {
                        rows.edges[0].input.source.function = FunctionCoordinate(1);
                    }
                    7 => {
                        rows.segments[0].connector = Some(edge(0, 0));
                    }
                    8 => {
                        rows.definition_outputs.last_mut().unwrap().output =
                            Definition::FunctionArgument {
                                function: F,
                                argument: 1,
                            };
                    }
                    9 => {
                        rows.definition_outputs[3].kind = S;
                        rows.definition_outputs[4].kind = R;
                    }
                    _ => unreachable!(),
                }
                assert!(run(a, b, rows, floor).is_err(), "hostile {hostile}");
            },
        );
    }
}

#[test]
fn actual_payload_cfg_effect_and_final_use_changes_refuse() {
    for hostile in 0..4 {
        let (input, mut output, mut plan) = pair(ScalarType::U32, BinaryOp::BitAnd, true, true);
        let body = output.functions[0].body.as_mut().unwrap();
        match hostile {
            0 => {
                body.blocks[0].operations[0].kind = OperationKind::Binary {
                    op: BinaryOp::BitOr,
                    lhs: ValueId(8),
                    rhs: ValueId(9),
                };
            }
            1 => {
                body.blocks[1].operations[0] = store(8, 32);
            }
            2 => {
                body.blocks[1].operations.clear();
                plan.operations.pop();
                plan.uses.drain(2..4);
            }
            3 => {
                body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(50),
                    then_target: BlockId(20),
                    then_arguments: vec![],
                    else_target: BlockId(20),
                    else_arguments: vec![],
                });
                body.blocks[0]
                    .operations
                    .push(constant(50, Constant::Bool(true)));
            }
            _ => unreachable!(),
        }
        if hostile == 3 {
            // A CFG change is tested directly with mismatched complete rosters;
            // no fabricated candidate is needed to excuse the new operations.
            inspect(
                input,
                output,
                |a, _| Rows::identity(a),
                |a, b, rows, floor| assert!(run(a, b, rows, floor).is_err()),
            );
        } else {
            inspect(
                input,
                output,
                |a, b| plan.rows(a, b),
                |a, b, rows, floor| assert!(run(a, b, rows, floor).is_err()),
            );
        }
    }
}

#[test]
fn an_unused_sibling_deletion_still_requires_independent_dominance() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(1));
    left.operations
        .push(expression(1, U32, BinaryOp::BitAnd, 8, 9));
    left.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut right = BasicBlock::new(BlockId(2));
    right
        .operations
        .push(expression(2, U32, BinaryOp::BitAnd, 9, 8));
    right.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let join = returning(3, vec![], &[8]);
    let input = module(
        vec![Type::BOOL, U32, U32],
        vec![U32],
        vec![7, 8, 9],
        vec![entry.clone(), left.clone(), right.clone(), join.clone()],
    );
    right.operations.clear();
    let output = module(
        vec![Type::BOOL, U32, U32],
        vec![U32],
        vec![7, 8, 9],
        vec![entry, left, right, join],
    );
    let plan = Plan {
        chains: (0..4).map(|b| vec![(b, None)]).collect(),
        operations: vec![Origin::Retained(op(1, 0))],
        relations: vec![(7, 7, R), (8, 8, R), (9, 9, R), (1, 1, R), (2, 1, S)],
        uses: vec![term(0, 0), operand(1, 0, 0), operand(1, 0, 1), term(3, 0)],
        edges: vec![edge(0, 0), edge(0, 1), edge(1, 0), edge(2, 0)],
        ..Plan::default()
    };
    inspect(
        input,
        output,
        |a, b| plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                run(a, b, rows, floor).unwrap_err(),
                CseError::Rule("retained expression must precede and dominate deletion")
            );
        },
    );
}

#[test]
fn later_same_block_anchor_and_unreachable_self_dominance_refuse() {
    for unreachable in [false, true] {
        let body = returning(
            10,
            vec![
                expression(1, U32, BinaryOp::BitAnd, 8, 9),
                expression(2, U32, BinaryOp::BitAnd, 9, 8),
            ],
            &[8],
        );
        let mut after = body.clone();
        let (deleted, retained) = if unreachable { (2, 1) } else { (1, 2) };
        after.operations.remove(if unreachable { 1 } else { 0 });
        let mut input = vec![body];
        let mut output = vec![after];
        if unreachable {
            input.insert(0, returning(0, vec![], &[8]));
            output.insert(0, returning(0, vec![], &[8]));
        }
        let block = u32::from(unreachable);
        let mut uses = if unreachable {
            vec![term(0, 0)]
        } else {
            vec![]
        };
        let retained_ordinal = u32::from(!unreachable);
        uses.extend([
            operand(block, retained_ordinal, 0),
            operand(block, retained_ordinal, 1),
            term(block, 0),
        ]);
        let plan = Plan {
            chains: (0..input.len() as u32).map(|b| vec![(b, None)]).collect(),
            operations: vec![Origin::Retained(op(block, retained_ordinal))],
            relations: vec![
                (8, 8, R),
                (9, 9, R),
                (retained, retained, R),
                (deleted, retained, S),
            ],
            uses,
            ..Plan::default()
        };
        inspect(
            module(vec![U32, U32], vec![U32], vec![8, 9], input),
            module(vec![U32, U32], vec![U32], vec![8, 9], output),
            |a, b| plan.rows(a, b),
            |a, b, rows, floor| {
                assert_eq!(
                    run(a, b, rows, floor).unwrap_err(),
                    CseError::Rule("retained expression must precede and dominate deletion")
                )
            },
        );
    }
}

#[test]
fn block_arguments_are_distinct_atoms_even_when_a_phi_solver_could_alias_them() {
    let mut first = BasicBlock::new(BlockId(10));
    first
        .operations
        .push(expression(1, U32, BinaryOp::BitAnd, 8, 9));
    first.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![ValueId(8)],
    });
    let mut second = returning(20, vec![expression(2, U32, BinaryOp::BitAnd, 3, 9)], &[2]);
    second.parameters.push(ValueDef::new(ValueId(3), U32));
    let input = module(
        vec![U32, U32],
        vec![U32],
        vec![8, 9],
        vec![first.clone(), second.clone()],
    );
    second.operations.clear();
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let output = module(vec![U32, U32], vec![U32], vec![8, 9], vec![first, second]);
    let plan = Plan {
        chains: vec![vec![(0, None)], vec![(1, None)]],
        operations: vec![Origin::Retained(op(0, 0))],
        relations: vec![(8, 8, R), (9, 9, R), (1, 1, R), (3, 3, R), (2, 1, S)],
        uses: vec![operand(0, 0, 0), operand(0, 0, 1), term(0, 0), term(1, 0)],
        edges: vec![edge(0, 0)],
        edge_arguments: vec![edge_arg(0, 0, 0)],
    };
    inspect(
        input,
        output,
        |a, b| plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                run(a, b, rows, floor).unwrap_err(),
                CseError::Rule("unproved exact-or-swapped operand pair")
            );
        },
    );
}

#[test]
fn boolean_index_and_different_operand_pairs_are_outside_the_closed_rule() {
    for ty in [Type::BOOL, Type::Scalar(ScalarType::Index)] {
        let input = module(
            vec![ty.clone(), ty.clone()],
            vec![ty.clone()],
            vec![8, 9],
            vec![returning(
                0,
                vec![
                    expression(1, ty.clone(), BinaryOp::BitAnd, 8, 9),
                    expression(2, ty.clone(), BinaryOp::BitAnd, 9, 8),
                ],
                &[2],
            )],
        );
        let output = module(
            vec![ty.clone(), ty.clone()],
            vec![ty.clone()],
            vec![8, 9],
            vec![returning(
                0,
                vec![expression(1, ty, BinaryOp::BitAnd, 8, 9)],
                &[1],
            )],
        );
        let plan = Plan {
            chains: vec![vec![(0, None)]],
            operations: vec![Origin::Retained(op(0, 0))],
            relations: vec![(8, 8, R), (9, 9, R), (1, 1, R), (2, 1, S)],
            uses: vec![operand(0, 0, 0), operand(0, 0, 1), term(0, 0)],
            ..Plan::default()
        };
        inspect(
            input,
            output,
            |a, b| plan.rows(a, b),
            |a, b, rows, floor| {
                assert_eq!(
                    run(a, b, rows, floor).unwrap_err(),
                    CseError::Rule("closed fixed integer bitwise eligibility")
                )
            },
        );
    }
    let (mut input, output, plan) = pair(ScalarType::U32, BinaryOp::BitAnd, true, true);
    input.functions[0].body.as_mut().unwrap().blocks[1].operations[0] =
        expression(2, U32, BinaryOp::BitAnd, 8, 8);
    inspect(
        input,
        output,
        |a, b| plan.rows(a, b),
        |a, b, rows, floor| {
            assert_eq!(
                run(a, b, rows, floor).unwrap_err(),
                CseError::Rule("unproved exact-or-swapped operand pair")
            )
        },
    );
}

#[test]
fn cyclic_candidate_aliases_cannot_seed_equivalence() {
    let input = module(
        vec![U32, U32],
        vec![U32],
        vec![8, 9],
        vec![returning(
            0,
            vec![
                expression(1, U32, BinaryOp::BitAnd, 8, 9),
                expression(2, U32, BinaryOp::BitAnd, 9, 8),
            ],
            &[2],
        )],
    );
    inspect(
        input.clone(),
        input,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            run(a, b, rows, floor).unwrap();
            rows.definition_outputs[2] = Descendant {
                output: result(0, 1, 0),
                kind: S,
            };
            rows.definition_outputs[3] = Descendant {
                output: result(0, 0, 0),
                kind: S,
            };
            assert_eq!(
                run(a, b, rows, floor).unwrap_err(),
                CseError::Rule("substitution requires omitted/retained pair")
            );
        },
    );
}

#[test]
fn equal_constants_and_reassociation_do_not_supply_extra_congruences() {
    for constants in [true, false] {
        let (parameters, ids, operations, target) = if constants {
            (
                vec![U32],
                vec![8],
                vec![
                    constant(1, Constant::U32(7)),
                    constant(2, Constant::U32(7)),
                    expression(3, U32, BinaryOp::BitAnd, 8, 1),
                    expression(4, U32, BinaryOp::BitAnd, 2, 8),
                ],
                3,
            )
        } else {
            (
                vec![U32, U32, U32],
                vec![8, 9, 10],
                vec![
                    expression(1, U32, BinaryOp::BitAnd, 9, 10),
                    expression(2, U32, BinaryOp::BitAnd, 8, 1),
                    expression(3, U32, BinaryOp::BitAnd, 8, 9),
                    expression(4, U32, BinaryOp::BitAnd, 3, 10),
                ],
                2,
            )
        };
        let input = module(
            parameters.clone(),
            vec![U32],
            ids.clone(),
            vec![returning(0, operations.clone(), &[4])],
        );
        let output = module(
            parameters,
            vec![U32],
            ids.clone(),
            vec![returning(0, operations[..3].to_vec(), &[target])],
        );
        let mut relations = ids.into_iter().map(|id| (id, id, R)).collect::<Vec<_>>();
        relations.extend([(1, 1, R), (2, 2, R), (3, 3, R), (4, target, S)]);
        let mut uses = Vec::new();
        for ordinal in if constants { 2..3 } else { 0..3 } {
            uses.extend([operand(0, ordinal, 0), operand(0, ordinal, 1)]);
        }
        uses.push(term(0, 0));
        let plan = Plan {
            chains: vec![vec![(0, None)]],
            operations: (0..3)
                .map(|ordinal| Origin::Retained(op(0, ordinal)))
                .collect(),
            relations,
            uses,
            ..Plan::default()
        };
        inspect(
            input,
            output,
            |a, b| plan.rows(a, b),
            |a, b, rows, floor| {
                assert_eq!(
                    run(a, b, rows, floor).unwrap_err(),
                    CseError::Rule("unproved exact-or-swapped operand pair")
                )
            },
        );
    }
}

#[test]
fn mutation_exact_and_one_short_observed_work_and_storage_restore_the_floor() {
    let (input, output, plan) = pair(ScalarType::U32, BinaryOp::BitXor, true, true);
    inspect(
        input,
        output,
        |a, b| plan.rows(a, b),
        |a, b, rows, floor| {
            let (_, work_bound, peak) = run(a, b, rows, floor).unwrap();
            for (work_limit, storage_limit, succeeds) in [
                (work_bound, peak, true),
                (work_bound - 1, peak, false),
                (work_bound, peak - 1, false),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let result = check(a, b, rows.candidate(), &mut budget);
                assert_eq!(result.is_ok(), succeeds);
                assert_eq!(budget.storage(), floor);
                if !succeeds {
                    assert!(matches!(
                        result,
                        Err(CseError::Resource(_))
                            | Err(CseError::ControlFlow(CfgError::Resource(_)))
                            | Err(CseError::Transition(Error::Resource(_)))
                    ));
                }
            }
        },
    );
}
