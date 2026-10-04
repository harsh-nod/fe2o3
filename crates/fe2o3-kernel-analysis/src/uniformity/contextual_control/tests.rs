use super::*;
use fe2o3_kernel_ir::{
    BarrierSemantics, Convergence, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent,
    MemoryOrdering, Signature, SwitchCase, SynchronizationScope, ValueDef, WorkgroupBarrier,
};

fn constant(id: u32, value: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Constant(Constant::Index(value)),
    )
}

fn comparison(id: u32, predicate: ComparePredicate, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::BOOL),
        OperationKind::Compare {
            predicate,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn global(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
            Type::INDEX,
        )),
    )
}

fn branch(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}

fn jump(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}

fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}

fn returned(id: u32) -> BasicBlock {
    block(id, vec![], Terminator::Return { values: vec![] })
}

fn module(function: &Function, workgroup: u32) -> Module {
    let mut module = Module::new("contextual_control");
    let mut kernel = Kernel::new(
        "entry",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(workgroup, 1, 1));
    module.kernels.push(kernel);
    module.functions.push(function.clone());
    module
}

fn with_facts<T>(
    function: &Function,
    run: impl FnOnce(&Facts<'_>, &mut BTreeMap<BlockId, BTreeSet<BlockId>>) -> T,
) -> T {
    let body = function.body.as_ref().unwrap();
    let blocks = body.blocks.iter().map(|block| (block.id, block)).collect();
    let (types, malformed) = index_value_types(function, body);
    assert!(!malformed);
    let reachable = reachable_blocks(body, &blocks);
    let (incoming, malformed) = incoming_edges(body, &blocks, &reachable, &types);
    assert!(!malformed);
    let dominators = compute_dominators(body, &reachable, &incoming);
    let definitions = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .flat_map(|operation| {
            operation
                .results
                .iter()
                .map(move |result| (result.id, operation))
        })
        .collect();
    let facts = Facts {
        body,
        incoming: &incoming,
        dominators: &dominators,
        types: &types,
        definitions: &definitions,
    };
    let mut effective = effective_successors(body, &BTreeMap::new());
    run(&facts, &mut effective)
}

fn refined(function: &Function) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    with_facts(function, |facts, effective| {
        refine_with_limits(
            facts,
            effective,
            MAX_RELATIONAL_PROOF_WORK,
            MAX_RELATIONAL_PROOF_WORK,
        )
        .unwrap();
        effective.clone()
    })
}

fn guarded_fixture() -> Function {
    let barrier = Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::Uniform {
                scope: SynchronizationScope::Workgroup,
            },
        }),
    );
    Function::kernel_entry(
        "guarded",
        Signature::new(vec![Type::INDEX], vec![]),
        vec![ValueId(1)],
        vec![
            block(
                0,
                vec![
                    global(0),
                    constant(3, 8),
                    constant(8, 0),
                    comparison(4, ComparePredicate::GreaterThanOrEqual, 0, 3),
                ],
                branch(4, 90, 1),
            ),
            block(
                1,
                vec![],
                Terminator::Switch {
                    selector: ValueId(1),
                    cases: vec![SwitchCase {
                        value: 8,
                        target: BlockId(2),
                        arguments: vec![],
                    }],
                    default_target: BlockId(90),
                    default_arguments: vec![],
                },
            ),
            block(
                2,
                vec![comparison(5, ComparePredicate::LessThan, 0, 3)],
                branch(5, 3, 91),
            ),
            block(
                3,
                vec![comparison(6, ComparePredicate::NotEqual, 0, 8)],
                branch(6, 4, 5),
            ),
            block(
                4,
                vec![comparison(9, ComparePredicate::LessThan, 0, 1)],
                branch(9, 6, 91),
            ),
            block(5, vec![], jump(7)),
            block(6, vec![], jump(7)),
            block(7, vec![barrier], Terminator::Return { values: vec![] }),
            block(90, vec![], Terminator::Unreachable),
            block(91, vec![], Terminator::Unreachable),
        ],
    )
}

fn block_mut(function: &mut Function, id: u32) -> &mut BasicBlock {
    function
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == BlockId(id))
        .unwrap()
}

#[test]
fn interval_truth_matches_an_independent_exhaustive_integer_oracle() {
    for predicate in [
        ComparePredicate::Equal,
        ComparePredicate::NotEqual,
        ComparePredicate::LessThan,
        ComparePredicate::LessThanOrEqual,
        ComparePredicate::GreaterThan,
        ComparePredicate::GreaterThanOrEqual,
    ] {
        for a in 0..8 {
            for b in a..8 {
                for c in 0..8 {
                    for d in c..8 {
                        let actual = comparison_truth(
                            predicate,
                            UnsignedRange { min: a, max: b },
                            UnsignedRange { min: c, max: d },
                        );
                        let mut answers = BTreeSet::new();
                        for lhs in a..=b {
                            for rhs in c..=d {
                                answers.insert(match predicate {
                                    ComparePredicate::Equal => lhs == rhs,
                                    ComparePredicate::NotEqual => lhs != rhs,
                                    ComparePredicate::LessThan => lhs < rhs,
                                    ComparePredicate::LessThanOrEqual => lhs <= rhs,
                                    ComparePredicate::GreaterThan => lhs > rhs,
                                    ComparePredicate::GreaterThanOrEqual => lhs >= rhs,
                                });
                            }
                        }
                        let expected = (answers.len() == 1).then(|| *answers.first().unwrap());
                        assert_eq!(actual, expected, "{predicate:?} [{a},{b}] [{c},{d}]");
                    }
                }
            }
        }
    }
}

#[test]
fn aligned_cutoffs_preserve_cohorts_for_small_workgroups_and_swapped_operands() {
    for width in 1..=8 {
        for bound in 0..=25 {
            let operations = [global(0), constant(1, bound)];
            let definitions = operations
                .iter()
                .map(|operation| (operation.results[0].id, operation))
                .collect();
            for predicate in [
                ComparePredicate::Equal,
                ComparePredicate::NotEqual,
                ComparePredicate::LessThan,
                ComparePredicate::LessThanOrEqual,
                ComparePredicate::GreaterThan,
                ComparePredicate::GreaterThanOrEqual,
            ] {
                let cutoff = match predicate {
                    ComparePredicate::LessThan | ComparePredicate::GreaterThanOrEqual => {
                        Some(bound)
                    }
                    ComparePredicate::LessThanOrEqual | ComparePredicate::GreaterThan => {
                        bound.checked_add(1)
                    }
                    _ => None,
                };
                let expected = cutoff.is_some_and(|cutoff| cutoff % u64::from(width) == 0);
                for swapped in [false, true] {
                    let (predicate, lhs, rhs) = if swapped {
                        (swap_predicate(predicate), ValueId(1), ValueId(0))
                    } else {
                        (predicate, ValueId(0), ValueId(1))
                    };
                    assert_eq!(
                        aligned_global_comparison(Some(width), &definitions, predicate, lhs, rhs),
                        expected
                    );
                }
                if expected {
                    for group in 0..5 {
                        let answers = (0..width)
                            .map(|local| {
                                let index = u64::from(group * width + local);
                                match predicate {
                                    ComparePredicate::LessThan => index < bound,
                                    ComparePredicate::LessThanOrEqual => index <= bound,
                                    ComparePredicate::GreaterThan => index > bound,
                                    ComparePredicate::GreaterThanOrEqual => index >= bound,
                                    _ => unreachable!(),
                                }
                            })
                            .collect::<BTreeSet<_>>();
                        assert_eq!(answers.len(), 1);
                    }
                }
            }
        }
    }
}

#[test]
fn geometry_and_typed_identity_are_required_for_aligned_comparisons() {
    let function = guarded_fixture();
    let valid = module(&function, 8);
    assert_eq!(exact_d1_workgroup(&valid, &function), Some(8));
    for bad in 0..7 {
        let mut module = valid.clone();
        match bad {
            0 => module.kernels.clear(),
            1 => module.kernels[0].workgroup_size = None,
            2 => module.kernels[0].workgroup_size = Some(WorkgroupSize::new(0, 1, 1)),
            3 => {
                module.kernels[0].domain = LaunchDomain::D2 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                }
            }
            4 => {
                module.kernels[0].domain = LaunchDomain::D3 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                    z: LaunchExtent::Dynamic,
                }
            }
            5 => {
                let mut conflicting = module.kernels[0].clone();
                conflicting.workgroup_size = Some(WorkgroupSize::new(16, 1, 1));
                module.kernels.push(conflicting);
            }
            _ => module.kernels[0].workgroup_size = Some(WorkgroupSize::new(8, 2, 1)),
        }
        assert_eq!(exact_d1_workgroup(&module, &function), None);
    }
    for bad in 0..6 {
        let mut operations = [global(0), constant(1, 8)];
        let extent = match bad {
            0 => None,
            1 => Some(0),
            _ => Some(8),
        };
        match bad {
            2 => operations[0] = constant(0, 0),
            3 => operations[1] = constant(1, 7),
            4 => {
                operations[1] = Operation::effect_free(
                    ValueDef::new(ValueId(1), Type::Scalar(ScalarType::I64)),
                    OperationKind::Constant(Constant::I64(8)),
                )
            }
            5 => {
                operations[0] = Operation::effect_free(
                    ValueDef::new(ValueId(0), Type::INDEX),
                    OperationKind::Cast {
                        kind: CastKind::Truncate,
                        value: ValueId(2),
                        to: Type::INDEX,
                    },
                )
            }
            _ => {}
        }
        let definitions = operations
            .iter()
            .map(|operation| (operation.results[0].id, operation))
            .collect();
        assert!(!aligned_global_comparison(
            extent,
            &definitions,
            ComparePredicate::LessThan,
            ValueId(0),
            ValueId(1)
        ));
    }
    let operations = [global(0), constant(1, u64::MAX)];
    let definitions = operations
        .iter()
        .map(|operation| (operation.results[0].id, operation))
        .collect();
    assert!(!aligned_global_comparison(
        Some(8),
        &definitions,
        ComparePredicate::LessThanOrEqual,
        ValueId(0),
        ValueId(1)
    ));
}

#[test]
fn exact_dominated_bounds_restore_masked_reconvergence_without_uniformizing_the_index() {
    let function = guarded_fixture();
    let effective = refined(&function);
    assert_eq!(effective[&BlockId(0)].len(), 2);
    assert_eq!(effective[&BlockId(2)], BTreeSet::from([BlockId(3)]));
    assert_eq!(effective[&BlockId(4)], BTreeSet::from([BlockId(6)]));
    let report = analyze_kernel_entry(&module(&function, 8), &function);
    assert_eq!(report.value(ValueId(0)), Variation::Varying);
    assert_eq!(
        report.block_control(BlockId(7)),
        Variation::WorkgroupUniform
    );
    assert_eq!(
        analyze_function(&function).block_control(BlockId(7)),
        Variation::Varying
    );
    assert_eq!(
        analyze_kernel_entry(&module(&function, 16), &function).block_control(BlockId(7)),
        Variation::Varying
    );
}

#[test]
fn bounds_do_not_authorize_sibling_joins_duplicate_edges_or_shared_switch_targets() {
    for mutation in 0..5 {
        let mut function = guarded_fixture();
        let query = match mutation {
            0 => {
                block_mut(&mut function, 90).terminator = Some(jump(1));
                2
            }
            1 => {
                block_mut(&mut function, 0).terminator = Some(branch(4, 1, 1));
                2
            }
            2 => {
                if let Some(Terminator::Switch { cases, .. }) =
                    &mut block_mut(&mut function, 1).terminator
                {
                    cases.push(SwitchCase {
                        value: 9,
                        target: BlockId(2),
                        arguments: vec![],
                    });
                }
                4
            }
            3 => {
                if let Some(Terminator::Switch { default_target, .. }) =
                    &mut block_mut(&mut function, 1).terminator
                {
                    *default_target = BlockId(2);
                }
                4
            }
            _ => {
                block_mut(&mut function, 0).terminator = Some(branch(4, 2, 1));
                4
            }
        };
        assert_eq!(
            refined(&function)[&BlockId(query)].len(),
            2,
            "mutation {mutation}"
        );
    }
}

#[test]
fn reused_selectors_keep_their_earlier_unknown_edges() {
    let mut function = guarded_fixture();
    block_mut(&mut function, 0).operations[3] = comparison(4, ComparePredicate::LessThan, 0, 3);
    block_mut(&mut function, 0).terminator = Some(branch(4, 1, 90));
    block_mut(&mut function, 2).terminator = Some(branch(4, 3, 91));
    let effective = refined(&function);
    assert_eq!(effective[&BlockId(0)].len(), 2);
    assert_eq!(effective[&BlockId(2)], BTreeSet::from([BlockId(3)]));
}

#[test]
fn loop_backedges_do_not_authorize_entry_or_changing_phi_facts() {
    let function = Function::kernel_entry(
        "entry_backedge",
        Signature::new(vec![], vec![]),
        vec![],
        vec![
            block(0, vec![global(0), constant(1, 8)], jump(1)),
            block(
                1,
                vec![comparison(2, ComparePredicate::LessThan, 0, 1)],
                branch(2, 2, 3),
            ),
            block(
                2,
                vec![comparison(3, ComparePredicate::LessThan, 0, 1)],
                branch(3, 0, 3),
            ),
            returned(3),
        ],
    );
    assert_eq!(refined(&function)[&BlockId(1)].len(), 2);
    let mut header = block(
        1,
        vec![comparison(4, ComparePredicate::LessThan, 3, 1)],
        branch(4, 2, 3),
    );
    header
        .parameters
        .push(ValueDef::new(ValueId(3), Type::INDEX));
    let function = Function::kernel_entry(
        "changing_phi",
        Signature::new(vec![], vec![]),
        vec![],
        vec![
            block(
                0,
                vec![global(0), constant(1, 8), constant(2, 1)],
                Terminator::Branch {
                    target: BlockId(1),
                    arguments: vec![ValueId(0)],
                },
            ),
            header,
            block(
                2,
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(5), Type::INDEX),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(3),
                        rhs: ValueId(2),
                    },
                )],
                Terminator::Branch {
                    target: BlockId(1),
                    arguments: vec![ValueId(5)],
                },
            ),
            returned(3),
        ],
    );
    assert_eq!(refined(&function)[&BlockId(1)].len(), 2);
}

fn constant_fixture(two_queries: bool) -> Function {
    let mut blocks = vec![
        block(
            0,
            vec![
                constant(0, 0),
                constant(1, 1),
                comparison(2, ComparePredicate::LessThan, 0, 1),
            ],
            branch(2, 1, 2),
        ),
        returned(1),
        returned(2),
    ];
    if two_queries {
        blocks[1].terminator = Some(branch(2, 3, 2));
        blocks.push(returned(3));
    }
    Function::kernel_entry("constant", Signature::new(vec![], vec![]), vec![], blocks)
}

#[test]
fn contextual_proof_has_literal_work_and_storage_boundaries_and_atomic_exhaustion() {
    let function = constant_fixture(false);
    with_facts(&function, |facts, effective| {
        let original = effective.clone();
        assert_eq!(refine_with_limits(facts, effective, 43, 1), Err(()));
        assert_eq!(*effective, original);
        assert_eq!(refine_with_limits(facts, effective, 44, 0), Err(()));
        assert_eq!(*effective, original);
        assert_eq!(
            refine_with_limits(facts, effective, 44, 1),
            Ok(Receipt { work: 44, rows: 1 })
        );
        assert_eq!(effective[&BlockId(0)], BTreeSet::from([BlockId(1)]));
    });
    with_facts(&constant_fixture(true), |facts, effective| {
        let original = effective.clone();
        assert_eq!(
            refine_with_limits(facts, effective, MAX_RELATIONAL_PROOF_WORK, 1),
            Err(())
        );
        assert_eq!(*effective, original);
        assert_eq!(
            refine_with_limits(facts, effective, 90, 2),
            Ok(Receipt { work: 90, rows: 2 }),
        );
        *effective = original.clone();
        assert_eq!(refine_with_limits(facts, effective, 89, 2), Err(()));
        assert_eq!(*effective, original);
    });
}

#[test]
fn every_small_concrete_path_keeps_its_original_edge() {
    let function = guarded_fixture();
    let effective = refined(&function);
    let blocks = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    for index in 0..24 {
        for length in 0..12 {
            let mut values = BTreeMap::from([(ValueId(1), length)]);
            let mut current = BlockId(0);
            for step in 0..12 {
                assert!(step < 11, "fixture must terminate without a loop");
                let block = blocks[&current];
                for operation in &block.operations {
                    let value = match operation.kind {
                        OperationKind::Constant(Constant::Index(value)) => value,
                        OperationKind::Intrinsic(_) => index,
                        OperationKind::Compare {
                            predicate,
                            lhs,
                            rhs,
                        } => {
                            let lhs = values[&lhs];
                            let rhs = values[&rhs];
                            u64::from(match predicate {
                                ComparePredicate::Equal => lhs == rhs,
                                ComparePredicate::NotEqual => lhs != rhs,
                                ComparePredicate::LessThan => lhs < rhs,
                                ComparePredicate::LessThanOrEqual => lhs <= rhs,
                                ComparePredicate::GreaterThan => lhs > rhs,
                                ComparePredicate::GreaterThanOrEqual => lhs >= rhs,
                            })
                        }
                        OperationKind::WorkgroupBarrier(_) => continue,
                        _ => panic!("oracle fixture operation is outside its small language"),
                    };
                    values.insert(operation.results[0].id, value);
                }
                let next = match block.terminator.as_ref().unwrap() {
                    Terminator::ConditionalBranch {
                        condition,
                        then_target,
                        else_target,
                        ..
                    } => {
                        if values[condition] != 0 {
                            *then_target
                        } else {
                            *else_target
                        }
                    }
                    Terminator::Switch {
                        selector,
                        cases,
                        default_target,
                        ..
                    } => cases
                        .iter()
                        .find(|case| case.value == values[selector])
                        .map_or(*default_target, |case| case.target),
                    Terminator::Branch { target, .. } => *target,
                    Terminator::Return { .. } | Terminator::Unreachable => break,
                    _ => panic!("oracle fixture terminator is outside its small language"),
                };
                assert!(
                    effective[&current].contains(&next),
                    "index {index}, length {length}, edge {current:?}->{next:?}"
                );
                current = next;
            }
        }
    }
}

#[test]
fn integer_switch_facts_require_exact_unsigned_case_types() {
    for wrong_type in [false, true] {
        let mut function = guarded_fixture();
        block_mut(&mut function, 1).terminator = Some(Terminator::IntegerSwitch {
            selector: ValueId(1),
            cases: vec![fe2o3_kernel_ir::IntegerSwitchCase {
                value: if wrong_type {
                    Constant::U64(8)
                } else {
                    Constant::Index(8)
                },
                target: BlockId(2),
                arguments: vec![],
            }],
            default_target: BlockId(90),
            default_arguments: vec![],
        });
        assert_eq!(
            refined(&function)[&BlockId(4)].len(),
            if wrong_type { 2 } else { 1 }
        );
    }
}

#[test]
fn contradictory_path_bounds_remain_unknown() {
    let function = Function::kernel_entry(
        "contradiction",
        Signature::new(vec![], vec![]),
        vec![],
        vec![
            block(
                0,
                vec![
                    global(0),
                    constant(1, 8),
                    comparison(2, ComparePredicate::LessThan, 0, 1),
                ],
                branch(2, 1, 4),
            ),
            block(
                1,
                vec![comparison(3, ComparePredicate::GreaterThanOrEqual, 0, 1)],
                branch(3, 2, 4),
            ),
            block(
                2,
                vec![comparison(4, ComparePredicate::LessThan, 0, 1)],
                branch(4, 3, 4),
            ),
            returned(3),
            returned(4),
        ],
    );
    let effective = refined(&function);
    assert_eq!(effective[&BlockId(1)], BTreeSet::from([BlockId(4)]));
    assert_eq!(effective[&BlockId(2)].len(), 2);
}

#[test]
fn a_large_borrowed_index_exhausts_atomically_without_allocating_proof_indexes() {
    let mut function = constant_fixture(true);
    block_mut(&mut function, 0)
        .operations
        .extend((10..6000).map(|id| constant(id, u64::from(id))));
    with_facts(&function, |facts, effective| {
        let original = effective.clone();
        assert_eq!(
            refine_with_limits(
                facts,
                effective,
                MAX_RELATIONAL_PROOF_WORK,
                MAX_RELATIONAL_PROOF_WORK
            ),
            Err(())
        );
        assert_eq!(*effective, original);
    });
}
