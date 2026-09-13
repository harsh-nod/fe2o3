use crate::{BasicBlock, CanonicalKernelIrWorkBudgetV1, Signature, SwitchCase};

fn metered_cfg_function_v1(blocks: Vec<BasicBlock>) -> Function {
    Function::definition(
        "metered_cfg",
        Signature::new(vec![], vec![]),
        vec![],
        blocks,
    )
}

fn metered_cfg_chain_v1(count: usize) -> Function {
    let blocks = (0..count)
        .map(|position| {
            let mut block = BasicBlock::new(BlockId((count - position - 1) as u32));
            block.terminator = Some(if position + 1 == count {
                Terminator::Return { values: vec![] }
            } else {
                Terminator::Branch {
                    target: BlockId((count - position - 2) as u32),
                    arguments: vec![],
                }
            });
            block
        })
        .collect();
    metered_cfg_function_v1(blocks)
}

fn metered_cfg_chain_work_v1(blocks: usize) -> usize {
    // Index: body/allocation headers + physical block scans + two edge passes,
    // exact incoming/successor/predecessor construction and duplicate-edge dedup.
    let index = 36 * blocks - 7;
    let radix = if blocks < 2 {
        0
    } else {
        // Descending source IDs invert at the first pair, after three probe units.
        3 + 4 * (3 * blocks + 2 * 256) + blocks
    };
    let reachability = 6 * blocks + 1;
    let reverse_postorder = 13 * blocks + 1;
    let dominators = if blocks == 1 { 9 } else { 17 * blocks - 6 };
    let intervals = 18 * blocks + 6;
    let reducibility = 24 * blocks - 4;
    index + radix + reachability + reverse_postorder + dominators + intervals + reducibility
}

#[test]
fn metered_cfg_chain_has_independent_exact_work_storage_and_legacy_counters() {
    const FLOOR: usize = 7;
    const DOMINANCE_QUERY: usize = 2 * (1 + 1) + 4;
    for blocks in [1_usize, 2, 8, 64, 1_024] {
        let function = metered_cfg_chain_v1(blocks);
        let expected_work = metered_cfg_chain_work_v1(blocks);
        // Retained graph =8B+6E+U, plus reachability and two interval arrays.
        // The interval phase adds idoms, exact children and its bounded stack.
        let retained = 18 * blocks - 7;
        let peak = 24 * blocks - 8;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work + DOMINANCE_QUERY);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, FLOOR + peak);
        budget.reserve_storage(FLOOR).unwrap();
        let analysis = analyze_control_flow_with_verification_budget_v1(
            &function,
            ControlFlowLimits::DEFAULT,
            &mut budget,
        )
        .unwrap();
        assert_eq!(analysis.flow, analyze_control_flow(&function).unwrap());
        assert_eq!(analysis.flow.work().total, 11 * blocks as u64 - 7);
        assert_eq!(budget.work(), expected_work, "blocks={blocks}");
        assert_eq!(budget.storage(), FLOOR + retained, "blocks={blocks}");
        assert_eq!(budget.peak_storage(), FLOOR + peak, "blocks={blocks}");
        assert!(
            analysis
                .dominates(BlockId((blocks - 1) as u32), BlockId(0), &mut budget)
                .unwrap()
        );
        assert_eq!(budget.work(), expected_work + DOMINANCE_QUERY);
        analysis.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + peak);
    }
}

#[test]
fn metered_cfg_rejected_work_and_storage_restore_the_incoming_floor() {
    const FLOOR: usize = 7;
    for blocks in [1_usize, 2, 17] {
        let function = metered_cfg_chain_v1(blocks);
        let expected_work = metered_cfg_chain_work_v1(blocks);
        let expected_peak = FLOOR + 24 * blocks - 8;
        for (work_limit, storage_limit, work_failure) in [
            (expected_work - 1, expected_peak, true),
            (expected_work, expected_peak - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let error = match analyze_control_flow_with_verification_budget_v1(
                &function,
                ControlFlowLimits::DEFAULT,
                &mut budget,
            ) {
                Ok(_) => panic!("one-under admission unexpectedly succeeded"),
                Err(error) => error,
            };
            if work_failure {
                assert!(matches!(error, MeteredControlFlowErrorV1::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                ) if error.actual() == expected_work && error.limit() == work_limit));
                assert_eq!(budget.work(), expected_work - blocks);
            } else {
                assert!(matches!(error, MeteredControlFlowErrorV1::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
                ) if error.actual() == expected_peak && error.limit() == storage_limit));
                assert!(budget.work() <= work_limit);
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn metered_cfg_sparse_identity_queries_and_empty_lookup_are_bounded() {
    let mut first = BasicBlock::new(BlockId(u32::MAX));
    first.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![],
    });
    let mut last = BasicBlock::new(BlockId(17));
    last.terminator = Some(Terminator::Return { values: vec![] });
    let function = metered_cfg_function_v1(vec![first, last]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000);
    let analysis = analyze_control_flow_with_verification_budget_v1(
        &function,
        ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .unwrap();
    assert_eq!(analysis.flow, analyze_control_flow(&function).unwrap());
    assert_eq!(
        analysis.flow.block_positions,
        [(BlockId(17), 1), (BlockId(u32::MAX), 0)]
    );
    assert!(
        analysis
            .dominates(BlockId(u32::MAX), BlockId(17), &mut budget)
            .unwrap()
    );
    assert!(
        !analysis
            .dominates(BlockId(17), BlockId(u32::MAX), &mut budget)
            .unwrap()
    );
    assert!(
        !analysis
            .dominates(BlockId(0), BlockId(17), &mut budget)
            .unwrap()
    );
    analysis.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);

    for limit in [0_usize, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let mut resources = ControlFlowResourcesV1 {
            budget: Some(&mut budget),
        };
        let result = resources.block_position(&[], BlockId(u32::MAX));
        if limit == 1 {
            assert_eq!(result.unwrap(), None);
            assert_eq!(budget.work(), 1);
        } else {
            assert!(matches!(result, Err(MeteredControlFlowErrorV1::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Work(error)
            )) if error.actual() == 1));
            assert_eq!(budget.work(), 0);
        }
    }

    // Numerical ID 1 indexes the row for ID 7, so the exact equality guard
    // must reject the shortcut and search the sparse roster instead.
    for limit in [5_usize, 6] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let mut resources = ControlFlowResourcesV1 {
            budget: Some(&mut budget),
        };
        let result = resources.block_position(&[(BlockId(1), 0), (BlockId(7), 1)], BlockId(1));
        if limit == 6 {
            assert_eq!(result.unwrap(), Some(0));
            assert_eq!(budget.work(), 6);
        } else {
            assert!(matches!(result, Err(MeteredControlFlowErrorV1::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Work(error)
            )) if error.actual() == 6));
            assert_eq!(budget.work(), 4);
        }
    }
}

fn cfg_terminal_block_v1(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

#[test]
fn metered_cfg_preserves_duplicate_missing_and_unknown_successor_precedence() {
    let missing = BasicBlock::new(BlockId(0));
    let mut unknown = cfg_terminal_block_v1(0);
    unknown.terminator = Some(Terminator::Branch {
        target: BlockId(99),
        arguments: vec![],
    });
    let cases = [
        (vec![], ControlFlowError::EmptyFunction),
        (
            vec![missing.clone(), cfg_terminal_block_v1(0)],
            ControlFlowError::MissingTerminator(BlockId(0)),
        ),
        (
            vec![cfg_terminal_block_v1(0), missing],
            ControlFlowError::DuplicateBlock(BlockId(0)),
        ),
        (
            vec![
                cfg_terminal_block_v1(0),
                BasicBlock::new(BlockId(1)),
                cfg_terminal_block_v1(0),
            ],
            ControlFlowError::MissingTerminator(BlockId(1)),
        ),
        (
            vec![unknown.clone(), cfg_terminal_block_v1(0)],
            ControlFlowError::DuplicateBlock(BlockId(0)),
        ),
        (
            vec![unknown],
            ControlFlowError::UnknownSuccessor {
                source: BlockId(0),
                target: BlockId(99),
            },
        ),
    ];
    for (blocks, expected) in cases {
        let function = metered_cfg_function_v1(blocks);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000);
        budget.reserve_storage(3).unwrap();
        let limits = ControlFlowLimits {
            edges: 0,
            ..ControlFlowLimits::DEFAULT
        };
        assert_eq!(
            analyze_control_flow_with_limits(&function, limits),
            Err(expected.clone())
        );
        let result =
            analyze_control_flow_with_verification_budget_v1(&function, limits, &mut budget);
        assert!(
            matches!(result, Err(MeteredControlFlowErrorV1::ControlFlow(error)) if error == expected)
        );
        assert_eq!(budget.storage(), 3);
    }
}

#[test]
fn metered_cfg_duplicate_edges_reducibility_and_unreachable_dominance_match_legacy() {
    let mut entry = cfg_terminal_block_v1(0);
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(1),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(1),
                arguments: vec![],
            },
        ],
        default_target: BlockId(2),
        default_arguments: vec![],
    });
    let mut left = cfg_terminal_block_v1(1);
    left.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut right = cfg_terminal_block_v1(2);
    right.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let unreachable = cfg_terminal_block_v1(u32::MAX);
    let function = metered_cfg_function_v1(vec![entry, left, right, unreachable]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 10_000);
    let analysis = analyze_control_flow_with_verification_budget_v1(
        &function,
        ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .unwrap();
    assert_eq!(analysis.flow, analyze_control_flow(&function).unwrap());
    assert_eq!(analysis.flow.edge_count(), 5);
    assert_eq!(analysis.flow.incoming_edges(BlockId(1)).unwrap().len(), 3);
    assert_eq!(
        analysis
            .flow
            .predecessor_blocks(BlockId(1))
            .unwrap()
            .collect::<Vec<_>>(),
        [BlockId(0), BlockId(2)]
    );
    assert_eq!(analysis.flow.irreducible_blocks(), [BlockId(1), BlockId(2)]);
    assert!(
        analysis
            .dominates(BlockId(u32::MAX), BlockId(u32::MAX), &mut budget)
            .unwrap()
    );
    assert!(
        !analysis
            .dominates(BlockId(0), BlockId(u32::MAX), &mut budget)
            .unwrap()
    );
    analysis.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn metered_cfg_source_spare_capacity_does_not_change_its_receipt() {
    let mut function = metered_cfg_chain_v1(8);
    let mut receipts = Vec::new();
    for spare in [0_usize, 1_024] {
        function.body.as_mut().unwrap().blocks.reserve_exact(spare);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000);
        let flow = analyze_control_flow_with_verification_budget_v1(
            &function,
            ControlFlowLimits::DEFAULT,
            &mut budget,
        )
        .unwrap();
        receipts.push((budget.work(), budget.storage(), budget.peak_storage()));
        flow.release(&mut budget).unwrap();
    }
    assert_eq!(receipts[0], receipts[1]);
}
