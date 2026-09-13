fn entry_dom_block_id_v1(position: usize) -> BlockId {
    BlockId(u32::MAX - position as u32 * 65_537)
}

fn entry_dom_function_v1(successors: &[Vec<usize>]) -> Function {
    metered_cfg_function_v1(
        successors
            .iter()
            .enumerate()
            .map(|(position, targets)| {
                let mut block = BasicBlock::new(entry_dom_block_id_v1(position));
                block.terminator = Some(match targets.as_slice() {
                    [] => Terminator::Return { values: vec![] },
                    [target] => Terminator::Branch {
                        target: entry_dom_block_id_v1(*target),
                        arguments: vec![],
                    },
                    [cases @ .., default] => Terminator::Switch {
                        selector: ValueId(0),
                        cases: cases
                            .iter()
                            .enumerate()
                            .map(|(ordinal, target)| SwitchCase {
                                value: ordinal as u64,
                                target: entry_dom_block_id_v1(*target),
                                arguments: vec![],
                            })
                            .collect(),
                        default_target: entry_dom_block_id_v1(*default),
                        default_arguments: vec![],
                    },
                });
                block
            })
            .collect(),
    )
}

fn entry_dom_oracle_v1(successors: &[Vec<usize>]) -> (Vec<bool>, Vec<Vec<bool>>) {
    let blocks = successors.len();
    let mut reachable = vec![false; blocks];
    let mut pending = vec![0];
    reachable[0] = true;
    while let Some(block) = pending.pop() {
        for successor in successors[block].iter().copied() {
            if !reachable[successor] {
                reachable[successor] = true;
                pending.push(successor);
            }
        }
    }

    // Independent set fixed point: no RPO, immediate dominators or tree climbs.
    let mut dominators = vec![vec![false; blocks]; blocks];
    dominators[0][0] = true;
    for block in 1..blocks {
        if reachable[block] {
            dominators[block] = reachable.clone();
        }
    }
    loop {
        let mut changed = false;
        for block in 1..blocks {
            if !reachable[block] {
                continue;
            }
            let mut next = reachable.clone();
            for predecessor in 0..blocks {
                if reachable[predecessor] && successors[predecessor].contains(&block) {
                    for definition in 0..blocks {
                        next[definition] &= dominators[predecessor][definition];
                    }
                }
            }
            next[block] = true;
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            return (reachable, dominators);
        }
    }
}

fn assert_entry_dom_oracle_v1(successors: &[Vec<usize>]) -> IndexedControlFlow {
    let function = entry_dom_function_v1(successors);
    let (reachable, dominators) = entry_dom_oracle_v1(successors);
    let legacy = analyze_control_flow(&function).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_048_576);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100_000);
    let analysis = analyze_control_flow_with_verification_budget_v1(
        &function,
        ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .unwrap();
    assert_eq!(analysis.flow, legacy);
    for (block, dominator_row) in dominators.iter().enumerate() {
        assert_eq!(
            analysis.flow.is_reachable(entry_dom_block_id_v1(block)),
            reachable[block]
        );
        for (definition, &is_dominator) in dominator_row.iter().enumerate() {
            let expected = if reachable[block] {
                is_dominator
            } else {
                definition == block
            };
            assert_eq!(
                analysis
                    .dominates(
                        entry_dom_block_id_v1(definition),
                        entry_dom_block_id_v1(block),
                        &mut budget,
                    )
                    .unwrap(),
                expected,
                "definition={definition}, block={block}, successors={successors:?}",
            );
        }
    }
    analysis.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    legacy
}

fn entry_dom_fan_in_v1(chain: usize, anchor: usize) -> Vec<Vec<usize>> {
    let exit = chain + 1;
    let mut successors = vec![vec![]; exit + 1];
    for (block, targets) in successors.iter_mut().enumerate().take(exit) {
        targets.push(block + 1);
        if block >= anchor && block + 1 != exit {
            targets.push(exit);
        }
    }
    successors
}

#[test]
fn entry_dom_high_fan_in_and_non_entry_common_dominators_match_oracle() {
    for chain in [1_usize, 2, 8, 64] {
        let flow = assert_entry_dom_oracle_v1(&entry_dom_fan_in_v1(chain, 0));
        assert_eq!(flow.work().dominator_climbs, 0);
        assert_eq!(
            flow.work().dominator_predecessor_visits,
            2 * (chain as u64 + 1)
        );
        assert!(flow.dominates(entry_dom_block_id_v1(0), entry_dom_block_id_v1(chain + 1)));
        assert!(!flow.dominates(entry_dom_block_id_v1(1), entry_dom_block_id_v1(chain + 1)));

        let flow = assert_entry_dom_oracle_v1(&entry_dom_fan_in_v1(chain, 1));
        assert!(flow.dominates(entry_dom_block_id_v1(1), entry_dom_block_id_v1(chain + 1)));
        if chain > 1 {
            assert!(flow.work().dominator_climbs > 0);
            assert!(!flow.dominates(entry_dom_block_id_v1(2), entry_dom_block_id_v1(chain + 1)));
        }
    }
}

#[test]
fn entry_dom_late_entry_intersection_unreachable_and_irreducible_match_oracle() {
    // The join's first two predecessors meet at entry; later chain predecessors
    // still exist in the validated CFG even though the fold can now stop.
    let late_entry = vec![vec![1, 2], vec![3, 5], vec![5], vec![4, 5], vec![5], vec![]];
    let flow = assert_entry_dom_oracle_v1(&late_entry);
    assert_eq!(flow.predecessors[5], [1, 2, 3, 4]);
    assert!(!flow.dominates(entry_dom_block_id_v1(1), entry_dom_block_id_v1(5)));

    // An unreachable predecessor occurs before the reachable join predecessors.
    let unreachable = vec![vec![2], vec![5], vec![3, 4], vec![5], vec![5], vec![]];
    let flow = assert_entry_dom_oracle_v1(&unreachable);
    assert_eq!(flow.predecessors[5], [1, 3, 4]);
    assert!(flow.dominates(entry_dom_block_id_v1(2), entry_dom_block_id_v1(5)));

    let irreducible = vec![vec![1, 2], vec![2], vec![1], vec![3, 1]];
    let flow = assert_entry_dom_oracle_v1(&irreducible);
    assert_eq!(
        flow.irreducible_blocks(),
        [entry_dom_block_id_v1(2), entry_dom_block_id_v1(1)]
    );
}

#[test]
fn entry_dom_all_three_block_graphs_match_oracle() {
    for edges in 0_u16..512 {
        let successors = (0..3)
            .map(|source| {
                (0..3)
                    .filter(|&target| edges & (1 << (source * 3 + target)) != 0)
                    .collect()
            })
            .collect::<Vec<Vec<usize>>>();
        assert_entry_dom_oracle_v1(&successors);
    }
}

#[test]
fn entry_dom_high_fan_in_has_literal_work_storage_and_failure_prefixes() {
    const FLOOR: usize = 7;
    for chain in [0_usize, 1, 8, 512] {
        let blocks = chain + 2;
        let mut predecessors = vec![vec![]; blocks];
        for (block, incoming) in predecessors.iter_mut().enumerate().take(chain + 1).skip(1) {
            incoming.push(block - 1);
        }
        predecessors[chain + 1] = (0..=chain).collect();
        let reachable = vec![true; blocks];
        let reverse_postorder = (0..blocks).collect::<Vec<_>>();

        // Initialization 3B+3, two sweeps of B+6N+7, final sentinel clear 1.
        // Every reachable non-entry block folds one predecessor, without climbs.
        let expected_work = 17 * chain + 28;
        let peak = FLOOR + 3 * blocks;
        let before_entry_check = 4 * blocks + 7;
        for (limit, accepted, actual) in [
            (expected_work, expected_work, None),
            (expected_work - 1, expected_work - 1, Some(expected_work)),
            (
                before_entry_check,
                before_entry_check,
                Some(before_entry_check + 1),
            ),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, peak);
            budget.reserve_storage(FLOOR).unwrap();
            let mut meter = WorkMeter::new(ControlFlowLimits::DEFAULT.analysis_work);
            let result = compute_immediate_dominators(
                &predecessors,
                &reachable,
                &reverse_postorder,
                &mut meter,
                &mut ControlFlowResourcesV1 {
                    budget: Some(&mut budget),
                },
            );
            assert_eq!(budget.work(), accepted, "chain={chain}, limit={limit}");
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(meter.work.dominator_climbs, 0);
            if let Some(actual) = actual {
                assert!(matches!(result, Err(MeteredControlFlowErrorV1::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Work(error)
                )) if error.actual() == actual && error.limit() == limit));
                assert_eq!(budget.storage(), peak);
                // The enclosing analyzer owns rollback after a private phase fails.
                budget.rollback_storage(FLOOR).unwrap();
            } else {
                let dominators = result.unwrap();
                assert_eq!(dominators[0], None);
                assert_eq!(
                    &dominators[1..chain + 1],
                    &(0..chain).map(Some).collect::<Vec<_>>()
                );
                assert_eq!(dominators[chain + 1], Some(0));
                assert_eq!(
                    meter.work.dominator_predecessor_visits,
                    2 * (chain as u64 + 1)
                );
                assert_eq!(budget.storage(), FLOOR + 2 * blocks);
                ControlFlowResourcesV1 {
                    budget: Some(&mut budget),
                }
                .free(dominators, blocks, 2)
                .unwrap();
            }
            assert_eq!(budget.storage(), FLOOR);
        }

        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, peak - 1);
        budget.reserve_storage(FLOOR).unwrap();
        let error = compute_immediate_dominators(
            &predecessors,
            &reachable,
            &reverse_postorder,
            &mut WorkMeter::new(ControlFlowLimits::DEFAULT.analysis_work),
            &mut ControlFlowResourcesV1 {
                budget: Some(&mut budget),
            },
        )
        .unwrap_err();
        assert!(matches!(error, MeteredControlFlowErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        ) if error.actual() == peak && error.limit() == peak - 1));
        assert_eq!(budget.work(), 2 * blocks + 2);
        assert_eq!(budget.storage(), FLOOR + blocks);
        assert_eq!(budget.peak_storage(), FLOOR + blocks);
        budget.rollback_storage(FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn entry_dom_absorbing_join_does_not_skip_unknown_reference_validation() {
    let mut function = entry_dom_function_v1(&entry_dom_fan_in_v1(8, 0));
    function.body.as_mut().unwrap().blocks[7].terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![SwitchCase {
            value: 0,
            target: entry_dom_block_id_v1(9),
            arguments: vec![],
        }],
        default_target: BlockId(0),
        default_arguments: vec![],
    });
    let expected = ControlFlowError::UnknownSuccessor {
        source: entry_dom_block_id_v1(7),
        target: BlockId(0),
    };
    assert_eq!(analyze_control_flow(&function), Err(expected.clone()));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 10_000);
    budget.reserve_storage(7).unwrap();
    assert!(matches!(
        analyze_control_flow_with_verification_budget_v1(&function, ControlFlowLimits::DEFAULT, &mut budget),
        Err(MeteredControlFlowErrorV1::ControlFlow(error)) if error == expected
    ));
    assert_eq!(budget.storage(), 7);
}
