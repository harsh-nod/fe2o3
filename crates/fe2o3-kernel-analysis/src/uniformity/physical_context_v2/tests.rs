use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BarrierSemantics, Convergence, FormalIndexWidth, IntrinsicOperation, Kernel,
    LaunchDomain, LaunchExtent, MemoryOrdering, Signature, SynchronizationScope, ValueDef,
    WorkgroupBarrier,
};

fn value(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn constant(id: u32, n: u64) -> Operation {
    value(id, Type::INDEX, OperationKind::Constant(Constant::Index(n)))
}
fn compare(id: u32, predicate: ComparePredicate, lhs: u32, rhs: u32) -> Operation {
    value(
        id,
        Type::BOOL,
        OperationKind::Compare {
            predicate,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn checked(id: u32, operator: CheckedBinaryOperator, lhs: u32, rhs: u32) -> Operation {
    Operation::new(
        vec![
            ValueDef::new(ValueId(id), Type::INDEX),
            ValueDef::new(ValueId(id + 1), Type::BOOL),
        ],
        OperationKind::Binary {
            op: BinaryOp::Checked(operator),
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
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
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn fixture() -> Module {
    let slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let function = Function::kernel_entry(
        "entry",
        Signature::new(vec![slice.clone(), slice], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(
                0,
                vec![
                    constant(2, 16 * 2880),
                    value(
                        3,
                        Type::INDEX,
                        OperationKind::SliceLength { slice: ValueId(0) },
                    ),
                    compare(4, ComparePredicate::GreaterThanOrEqual, 3, 2),
                ],
                branch(4, 1, 99),
            ),
            block(
                1,
                vec![
                    value(
                        5,
                        Type::INDEX,
                        OperationKind::Intrinsic(IntrinsicOperation::new(
                            IntrinsicKind::InvocationIndex {
                                kind: IndexKind::Global,
                                axis: Axis::X,
                            },
                            Type::INDEX,
                        )),
                    ),
                    constant(6, 64),
                    constant(7, 2880),
                    value(
                        8,
                        Type::INDEX,
                        OperationKind::Binary {
                            op: BinaryOp::Divide,
                            lhs: ValueId(5),
                            rhs: ValueId(6),
                        },
                    ),
                    checked(9, CheckedBinaryOperator::Multiply, 8, 7),
                    checked(11, CheckedBinaryOperator::Add, 9, 7),
                    value(
                        13,
                        Type::INDEX,
                        OperationKind::SliceLength { slice: ValueId(0) },
                    ),
                    compare(14, ComparePredicate::LessThanOrEqual, 11, 13),
                    value(
                        15,
                        Type::BOOL,
                        OperationKind::Binary {
                            op: BinaryOp::BitOr,
                            lhs: ValueId(10),
                            rhs: ValueId(12),
                        },
                    ),
                    value(
                        16,
                        Type::BOOL,
                        OperationKind::Unary {
                            op: UnaryOp::Not,
                            operand: ValueId(15),
                        },
                    ),
                    value(
                        17,
                        Type::BOOL,
                        OperationKind::Binary {
                            op: BinaryOp::BitAnd,
                            lhs: ValueId(14),
                            rhs: ValueId(16),
                        },
                    ),
                ],
                branch(17, 2, 99),
            ),
            block(
                2,
                vec![Operation::new(
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
                )],
                Terminator::Return { values: vec![] },
            ),
            block(99, vec![], Terminator::Return { values: vec![] }),
        ],
    );
    let mut module = Module::new("physical_context");
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
    module.kernels.push(kernel);
    module.functions.push(function);
    module
}
fn context(module: &Module, grid: u32) -> UniformityPhysicalLaunchV2<'_> {
    UniformityPhysicalLaunchV2::new(
        module,
        &module.kernels[0],
        [grid, 1, 1],
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}
fn with_refinement<T>(
    module: &Module,
    grid: u32,
    run: impl FnOnce(
        &UniformityPhysicalLaunchV2<'_>,
        &FunctionBody,
        &BTreeMap<BlockId, Vec<Edge>>,
        &BTreeMap<BlockId, BTreeSet<BlockId>>,
        &BTreeMap<ValueId, Type>,
        &BTreeMap<ValueId, &Operation>,
        &mut BTreeMap<BlockId, BTreeSet<BlockId>>,
    ) -> T,
) -> T {
    let context = context(module, grid);
    let function = context.function();
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
    let mut effective = effective_successors(body, &BTreeMap::new());
    run(
        &context,
        body,
        &incoming,
        &dominators,
        &types,
        &definitions,
        &mut effective,
    )
}
fn refined(module: &Module, grid: u32) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    with_refinement(
        module,
        grid,
        |context, body, incoming, dominators, types, definitions, effective| {
            selector::refine(
                context,
                body,
                incoming,
                dominators,
                types,
                definitions,
                effective,
                MAX_RELATIONAL_PROOF_WORK,
                MAX_RELATIONAL_PROOF_WORK,
            )
            .unwrap();
            effective.clone()
        },
    )
}

#[test]
fn physical_launch_and_same_slice_guard_prove_checked_view_without_weakening_barrier() {
    let module = fixture();
    assert_eq!(
        refined(&module, 4)[&BlockId(1)],
        BTreeSet::from([BlockId(2)])
    );
    assert!(
        context(&module, 4)
            .analyze()
            .block_control(BlockId(2))
            .is_uniform_for(SynchronizationScope::Workgroup)
    );
    assert!(
        !analyze_kernel_entry(&module, &module.functions[0])
            .block_control(BlockId(2))
            .is_uniform_for(SynchronizationScope::Workgroup)
    );
}

#[test]
fn extra_physical_group_is_not_replaced_by_logical_static_extent() {
    for extent in [
        LaunchExtent::Dynamic,
        LaunchExtent::Static(1),
        LaunchExtent::Static(1024),
    ] {
        let mut module = fixture();
        module.kernels[0].domain = LaunchDomain::D1 { x: extent };
        assert_eq!(refined(&module, 4)[&BlockId(1)].len(), 1);
        assert_eq!(refined(&module, 5)[&BlockId(1)].len(), 2);
        assert!(
            !context(&module, 5)
                .analyze()
                .block_control(BlockId(2))
                .is_uniform_for(SynchronizationScope::Workgroup)
        );
    }
}

#[test]
fn physical_context_refuses_foreign_kernel_ambiguous_entry_rank_and_width() {
    let original = fixture();
    let copy = original.clone();
    assert!(matches!(
        UniformityPhysicalLaunchV2::new(
            &original,
            &copy.kernels[0],
            [4, 1, 1],
            FormalIndexWidth::Bits64
        ),
        Err(UniformityPhysicalLaunchErrorV2::ForeignKernel)
    ));
    for width in [FormalIndexWidth::Unknown, FormalIndexWidth::Bits32] {
        assert!(matches!(
            UniformityPhysicalLaunchV2::new(&original, &original.kernels[0], [4, 1, 1], width),
            Err(UniformityPhysicalLaunchErrorV2::IndexWidth)
        ));
    }
    for grid in [[0, 1, 1], [4, 2, 1], [4, 1, 2]] {
        assert!(matches!(
            UniformityPhysicalLaunchV2::new(
                &original,
                &original.kernels[0],
                grid,
                FormalIndexWidth::Bits64
            ),
            Err(UniformityPhysicalLaunchErrorV2::Geometry)
        ));
    }
    let mut module = fixture();
    module.functions.push(module.functions[0].clone());
    assert!(matches!(
        UniformityPhysicalLaunchV2::new(
            &module,
            &module.kernels[0],
            [4, 1, 1],
            FormalIndexWidth::Bits64
        ),
        Err(UniformityPhysicalLaunchErrorV2::EntryClosure)
    ));
    for mode in 0..5 {
        let mut module = fixture();
        match mode {
            0 => module.kernels.push(module.kernels[0].clone()),
            1 => module.functions[0].role = FunctionRole::InternalHelper,
            2 => module.functions[0].body = None,
            3 => {
                module.kernels[0].domain = LaunchDomain::D2 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                }
            }
            4 => module.kernels[0].workgroup_size = Some(WorkgroupSize::new(256, 2, 1)),
            _ => unreachable!(),
        }
        assert!(
            UniformityPhysicalLaunchV2::new(
                &module,
                &module.kernels[0],
                [4, 1, 1],
                FormalIndexWidth::Bits64,
            )
            .is_err()
        );
    }
}

#[test]
fn equal_type_foreign_slice_and_non_dominating_guard_do_not_refine() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[1].operations[6].kind =
        OperationKind::SliceLength { slice: ValueId(1) };
    assert_eq!(refined(&module, 4)[&BlockId(1)].len(), 2);
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(branch(4, 1, 1));
    assert_eq!(refined(&module, 4)[&BlockId(1)].len(), 2);
}

#[test]
fn checked_overflow_and_zero_divisor_do_not_supply_a_false_overflow_flag() {
    for (index, constant_value) in [(1, 0), (2, u64::MAX)] {
        let mut module = fixture();
        let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[1].operations[index];
        operation.kind = OperationKind::Constant(Constant::Index(constant_value));
        assert_eq!(refined(&module, 4)[&BlockId(1)].len(), 2);
    }
}

#[test]
fn physical_context_query_budget_is_cumulative_and_mutation_is_atomic() {
    let module = fixture();
    with_refinement(
        &module,
        4,
        |context, body, incoming, dominators, types, definitions, effective| {
            let original = effective.clone();
            let receipt = selector::refine(
                context,
                body,
                incoming,
                dominators,
                types,
                definitions,
                effective,
                MAX_RELATIONAL_PROOF_WORK,
                MAX_RELATIONAL_PROOF_WORK,
            )
            .unwrap();
            let expected = effective.clone();
            assert_ne!(expected, original);
            for (work, rows, success) in [
                (receipt.work, receipt.rows, true),
                (receipt.work - 1, receipt.rows, false),
                (receipt.work, receipt.rows - 1, false),
            ] {
                let mut trial = original.clone();
                assert_eq!(
                    selector::refine(
                        context,
                        body,
                        incoming,
                        dominators,
                        types,
                        definitions,
                        &mut trial,
                        work,
                        rows
                    )
                    .is_ok(),
                    success
                );
                assert_eq!(
                    trial,
                    if success {
                        expected.clone()
                    } else {
                        original.clone()
                    }
                );
            }
        },
    );
}

#[test]
fn full_u32_grid_product_is_checked_in_u64_without_narrowing_to_static() {
    let mut module = fixture();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(u32::MAX, 1, 1));
    let context = context(&module, u32::MAX);
    assert_eq!(
        context.invocation_range(IndexKind::Global, Axis::X),
        Some(UnsignedRange {
            min: 0,
            max: u128::from(u32::MAX) * u128::from(u32::MAX) - 1
        })
    );
}

#[test]
fn signed_selector_constants_preserve_checked_predicate_and_typed_switch() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[1].operations.extend([
        value(
            18,
            Type::Scalar(ScalarType::I64),
            OperationKind::Constant(Constant::I64(1)),
        ),
        value(
            19,
            Type::Scalar(ScalarType::I64),
            OperationKind::Constant(Constant::I64(0)),
        ),
        value(
            20,
            Type::Scalar(ScalarType::I64),
            OperationKind::Select {
                condition: ValueId(17),
                true_value: ValueId(18),
                false_value: ValueId(19),
            },
        ),
    ]);
    body.blocks[1].terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(20),
        cases: vec![fe2o3_kernel_ir::IntegerSwitchCase {
            value: Constant::I64(1),
            target: BlockId(2),
            arguments: vec![],
        }],
        default_target: BlockId(99),
        default_arguments: vec![],
    });
    assert_eq!(
        refined(&module, 4)[&BlockId(1)],
        BTreeSet::from([BlockId(2)])
    );
    assert_eq!(refined(&module, 5)[&BlockId(1)].len(), 2);
}

#[test]
fn independent_empty_query_equation_pays_complete_index_and_retained_rows() {
    let mut module = fixture();
    module.functions[0] = Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block(
            0,
            vec![constant(0, 0)],
            Terminator::Return { values: vec![] },
        )],
    );
    with_refinement(
        &module,
        4,
        |context, body, incoming, dominators, types, definitions, effective| {
            let receipt = selector::refine(
                context,
                body,
                incoming,
                dominators,
                types,
                definitions,
                effective,
                MAX_RELATIONAL_PROOF_WORK,
                MAX_RELATIONAL_PROOF_WORK,
            )
            .unwrap();
            // One key scan, one type install, one definition install, two guard
            // census block visits and one query-loop block visit. Allocation work
            // separately counts the actual retained index/proposal capacities.
            assert_eq!(receipt.work, 6 + receipt.rows);
            assert!(receipt.rows >= 2);
            let original = effective.clone();
            assert!(
                selector::refine(
                    context,
                    body,
                    incoming,
                    dominators,
                    types,
                    definitions,
                    effective,
                    receipt.work - 1,
                    receipt.rows
                )
                .is_err()
            );
            assert_eq!(*effective, original);
        },
    );
}

#[test]
fn paid_source_index_handles_large_unrelated_definition_census_without_repeated_map_scans() {
    let mut module = fixture();
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .extend((32..7163).map(|id| constant(id, 0)));
    assert_eq!(
        refined(&module, 4)[&BlockId(1)],
        BTreeSet::from([BlockId(2)])
    );
    let mut sparse = fixture();
    sparse.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(constant(u32::MAX, 0));
    with_refinement(
        &sparse,
        4,
        |context, body, incoming, dominators, types, definitions, effective| {
            let original = effective.clone();
            assert!(
                selector::refine(
                    context,
                    body,
                    incoming,
                    dominators,
                    types,
                    definitions,
                    effective,
                    MAX_RELATIONAL_PROOF_WORK,
                    MAX_RELATIONAL_PROOF_WORK
                )
                .is_err()
            );
            assert_eq!(*effective, original);
        },
    );
}

#[test]
fn physical_view_proof_does_not_discharge_residual_divergent_loop_control() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[2].operations.extend([
        constant(18, 257),
        compare(19, ComparePredicate::LessThan, 5, 18),
    ]);
    body.blocks[2].terminator = Some(branch(19, 2, 99));
    let successors = refined(&module, 4);
    assert_eq!(successors[&BlockId(1)], BTreeSet::from([BlockId(2)]));
    assert_eq!(successors[&BlockId(2)].len(), 2);
    assert!(
        !context(&module, 4)
            .analyze()
            .block_control(BlockId(2))
            .is_uniform_for(SynchronizationScope::Workgroup)
    );
}

#[test]
fn future_iteration_length_guard_is_not_an_initial_iteration_fact() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    // Enter the query before testing the length. A later backedge's guard must
    // not prove the first visit, even though it guards every repeated visit.
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    body.blocks[2].terminator = Some(branch(4, 1, 99));
    let successors = refined(&module, 4);
    assert_eq!(successors[&BlockId(1)].len(), 2);
    assert!(
        !context(&module, 4)
            .analyze()
            .block_control(BlockId(2))
            .is_uniform_for(SynchronizationScope::Workgroup)
    );
}

#[test]
fn physical_index_preserves_original_direct_comparison_guard_grammar() {
    for predicate in [
        ComparePredicate::Equal,
        ComparePredicate::GreaterThanOrEqual,
    ] {
        let mut module = fixture();
        module.functions[0] = Function::kernel_entry(
            "entry",
            Signature::new(vec![Type::INDEX], vec![]),
            vec![ValueId(0)],
            vec![
                block(
                    0,
                    vec![constant(1, 17), compare(2, predicate, 0, 1)],
                    branch(2, 1, 99),
                ),
                block(1, vec![compare(3, predicate, 0, 1)], branch(3, 2, 99)),
                block(2, vec![], Terminator::Return { values: vec![] }),
                block(99, vec![], Terminator::Return { values: vec![] }),
            ],
        );
        with_refinement(
            &module,
            4,
            |context, body, incoming, dominators, types, definitions, effective| {
                let mut legacy = effective.clone();
                contextual_control::refine_successors(
                    body,
                    incoming,
                    dominators,
                    types,
                    definitions,
                    &mut legacy,
                );
                selector::refine(
                    context,
                    body,
                    incoming,
                    dominators,
                    types,
                    definitions,
                    effective,
                    MAX_RELATIONAL_PROOF_WORK,
                    MAX_RELATIONAL_PROOF_WORK,
                )
                .unwrap();
                assert_eq!(legacy[&BlockId(1)], BTreeSet::from([BlockId(2)]));
                assert_eq!(*effective, legacy);
            },
        );
    }
}

#[test]
fn malformed_checked_result_ordinal_cannot_be_used_as_an_overflow_proof() {
    for fault in [0, 1] {
        let mut module = fixture();
        let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[1].operations[4];
        if fault == 0 {
            operation.results.swap(0, 1);
        } else {
            operation.results[1].ty = Type::INDEX;
        }
        assert_eq!(refined(&module, 4)[&BlockId(1)].len(), 2);
    }
}

#[test]
fn independent_single_proposal_equation_pays_query_and_commit_before_mutation() {
    let mut module = fixture();
    module.functions[0] = Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![
            block(
                0,
                vec![value(
                    0,
                    Type::BOOL,
                    OperationKind::Constant(Constant::Bool(true)),
                )],
                branch(0, 1, 2),
            ),
            block(1, vec![], Terminator::Return { values: vec![] }),
            block(2, vec![], Terminator::Return { values: vec![] }),
        ],
    );
    with_refinement(
        &module,
        4,
        |context, body, incoming, dominators, types, definitions, effective| {
            let original = effective.clone();
            let receipt = selector::refine(
                context,
                body,
                incoming,
                dominators,
                types,
                definitions,
                effective,
                MAX_RELATIONAL_PROOF_WORK,
                MAX_RELATIONAL_PROOF_WORK,
            )
            .unwrap();
            // Nonallocation work: index 3; guard census/recognition 8; query
            // loop 3; explicit-stack constant evaluation 16; proposal lookup
            // and insertion 6; full commit preflight 10. All actual vector
            // capacities contribute separately, once, to both rows and work.
            let fixed = 3 + 8 + 3 + 16 + 6 + 10;
            assert_eq!(receipt.work, fixed + receipt.rows);
            assert_eq!(effective[&BlockId(0)], BTreeSet::from([BlockId(1)]));
            let expected = effective.clone();
            for (work, rows, succeeds) in [
                (fixed + receipt.rows, receipt.rows, true),
                (fixed + receipt.rows - 1, receipt.rows, false),
                (fixed + receipt.rows, receipt.rows - 1, false),
            ] {
                let mut trial = original.clone();
                assert_eq!(
                    selector::refine(
                        context,
                        body,
                        incoming,
                        dominators,
                        types,
                        definitions,
                        &mut trial,
                        work,
                        rows,
                    )
                    .is_ok(),
                    succeeds
                );
                assert_eq!(
                    trial,
                    if succeeds {
                        expected.clone()
                    } else {
                        original.clone()
                    }
                );
            }
        },
    );
}
