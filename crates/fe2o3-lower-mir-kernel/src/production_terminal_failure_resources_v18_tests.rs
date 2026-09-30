fn terminal_scope_fixture() -> (
    [(ValueId, fe2o3_kernel_ir::ExecutionRoleV15); 7],
    [LifecycleLiveV18; 7],
) {
    use LifecycleLiveV18 as Live;
    use fe2o3_kernel_ir::ExecutionRoleV15 as Role;
    (
        [
            (ValueId(1), Role::Context),
            (ValueId(3), Role::Workgroup),
            (
                ValueId(5),
                Role::MaskedTileU32 {
                    lanes: 32,
                    elements: 1,
                },
            ),
            (
                ValueId(7),
                Role::LaneFragmentU32 {
                    lanes: 32,
                    elements: 1,
                },
            ),
            (ValueId(9), Role::Context),
            (ValueId(11), Role::Workgroup),
            (
                ValueId(13),
                Role::MaskedTileU32 {
                    lanes: 32,
                    elements: 1,
                },
            ),
        ],
        [
            Live::Borrowed(1),
            Live::Workgroup(0),
            Live::Descendant(1),
            Live::Descendant(1),
            Live::Borrowed(5),
            Live::Workgroup(4),
            Live::Descendant(5),
        ],
    )
}

fn terminal_empty_row() -> PreparedTerminalFailureV18 {
    PreparedTerminalFailureV18 {
        witness: TerminalFailureClosureV18 {
            origin: 0,
            block: BlockId(7),
            original_gap: 0,
            first: 0,
            scope_ends: 0,
            diagnostic: 0,
            generated: true,
        },
        operations: Vec::new(),
        visited: false,
        source_index: 0,
    }
}

#[test]
fn terminal_descendant_closure_has_independent_exact_and_one_short_resources() {
    // Three width scans; two closures each scan twice, allocate one discard
    // vector (3 work), and visit D+1 operands. One operation allocation costs3.
    const WORK: usize = 3 * 7 + 3 + 2 * (2 * 7 + 3 + 1) + 3;
    const FLOOR: usize = 73;
    let bytes = 2 * size_of::<Operation>() + 3 * size_of::<ValueId>();
    assert_eq!(WORK, 63);
    for (work_limit, storage_limit, success) in [
        (WORK, FLOOR + bytes, true),
        (WORK - 1, FLOOR + bytes, false),
        (WORK, FLOOR + bytes - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = scoped_slot_attempt_v29(&mut budget, |budget| {
            let (slots, mut state) = terminal_scope_fixture();
            let mut row = terminal_empty_row();
            prepare_terminal_scope_ends_v18(&mut row, &slots, &mut state, budget)?;
            assert!(row.visited);
            assert_eq!(row.witness.scope_ends, 2);
            assert_eq!(row.operations.capacity(), 2);
            use fe2o3_kernel_ir::ExecutionOperationV15 as Op;
            assert_eq!(
                row.operations[0].kind,
                OperationKind::Execution(Op::ScopeEnd {
                    workgroup: ValueId(3),
                    discarded: vec![ValueId(5), ValueId(7)],
                })
            );
            assert_eq!(
                row.operations[1].kind,
                OperationKind::Execution(Op::ScopeEnd {
                    workgroup: ValueId(11),
                    discarded: vec![ValueId(13)],
                })
            );
            for operation in &row.operations {
                let OperationKind::Execution(Op::ScopeEnd { discarded, .. }) = &operation.kind
                else {
                    unreachable!()
                };
                assert_eq!(discarded.capacity(), discarded.len());
            }
            assert_eq!(
                state,
                [
                    LifecycleLiveV18::Context,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Context,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Absent
                ]
            );
            Ok(row)
        });
        assert_eq!(result.is_ok(), success);
        if let Ok(row) = result {
            assert_eq!(budget.work(), WORK);
            assert_eq!(budget.storage(), FLOOR + bytes);
            drop(row);
            budget.release_storage(bytes).unwrap();
        }
        assert_eq!(budget.storage(), FLOOR);
        if storage_limit < FLOOR + bytes {
            assert_eq!(budget.failed_storage(), Some(FLOOR + bytes));
            // The second discard-vector reserve fails after its three-work prepayment.
            assert_eq!(budget.work(), 53);
        } else if !success {
            assert_eq!(budget.work(), WORK - 7);
            assert_eq!(budget.failed_storage(), None);
        }
        drop(budget);
        assert_eq!(
            work.failed_work(),
            if work_limit < WORK { Some(WORK) } else { None }
        );
    }
}

#[test]
fn terminal_closure_preserves_prior_history_and_floor_on_error_and_panic() {
    const FLOOR: usize = 127;
    for exit in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.charge_work(10_001).is_err());
        assert!(budget.reserve_storage(10_001 - FLOOR).is_err());
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_slot_attempt_v29(&mut budget, |budget| {
                let (slots, mut state) = terminal_scope_fixture();
                let mut row = terminal_empty_row();
                prepare_terminal_scope_ends_v18(&mut row, &slots, &mut state, budget)?;
                if exit == 1 {
                    return Err(terminal_failure_error_v18());
                }
                if exit == 2 {
                    std::panic::panic_any(1206usize);
                }
                Ok(row)
            })
        }));
        match outcome {
            Ok(Ok(row)) => {
                assert_eq!(exit, 0);
                let bytes = 2 * size_of::<Operation>() + 3 * size_of::<ValueId>();
                assert_eq!(budget.storage(), FLOOR + bytes);
                drop(row);
                budget.release_storage(bytes).unwrap();
            }
            Ok(Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })) => {
                assert_eq!(exit, 1);
                assert_eq!(
                    detail,
                    "terminal failure differs from its original source occurrence"
                );
            }
            Err(payload) => {
                assert_eq!(exit, 2);
                assert_eq!(*payload.downcast::<usize>().unwrap(), 1206);
            }
            _ => panic!("unexpected terminal cleanup outcome"),
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 63);
        assert_eq!(budget.failed_storage(), Some(10_001));
        drop(budget);
        assert_eq!(work.failed_work(), Some(10_001));
    }
}

#[test]
fn terminal_closure_keeps_absent_context_and_descendant_roles_separate() {
    let (slots, original) = terminal_scope_fixture();
    for case in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
        budget.reserve_storage(31).unwrap();
        let result = scoped_slot_attempt_v29(&mut budget, |budget| {
            let mut row = terminal_empty_row();
            let mut state = match case {
                0 => [LifecycleLiveV18::Absent; 7],
                1 => [
                    LifecycleLiveV18::Context,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Context,
                    LifecycleLiveV18::Absent,
                    LifecycleLiveV18::Absent,
                ],
                _ => original,
            };
            if case == 2 {
                state[0] = LifecycleLiveV18::Context;
            }
            if case == 3 {
                state[1] = LifecycleLiveV18::Absent;
            }
            prepare_terminal_scope_ends_v18(&mut row, &slots, &mut state, budget)?;
            assert!(row.operations.is_empty());
            assert_eq!(row.witness.scope_ends, 0);
            Ok(row)
        });
        assert_eq!(result.is_ok(), case < 2);
        drop(result);
        assert_eq!(budget.storage(), 31);
    }
}

#[test]
fn shared_terminal_target_is_split_before_context_and_absent_states_join() {
    // This isolates the production ownership walker, not source admission.
    // The source-derived module tests above authenticate the actual redirects.
    with_terminal_module(FailureFixture::Shared, |owner, budget| {
        let (witness, issued) = owner
            .pending
            .roots
            .iter()
            .find_map(|root| {
                let body = owner.pending.graph.module().functions[root.function_ordinal]
                    .body
                    .as_ref()?;
                root.insertions.iter().find_map(|witness| {
                    let block = body
                        .blocks
                        .iter()
                        .find(|block| block.id == witness.after.block)?;
                    let operation = block.operations.get(witness.after.first as usize)?;
                    matches!(
                        operation.kind,
                        OperationKind::Execution(
                            fe2o3_kernel_ir::ExecutionOperationV15::ContextIssue
                        )
                    )
                    .then(|| (*witness, operation.clone()))
                })
            })
            .unwrap();
        for split in [false, true] {
            let mut blocks: Vec<_> = (0..4).map(|id| BasicBlock::new(BlockId(id))).collect();
            blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(999),
                then_target: BlockId(1),
                then_arguments: Vec::new(),
                else_target: BlockId(2),
                else_arguments: Vec::new(),
            });
            for block in &mut blocks[1..3] {
                block.terminator = Some(Terminator::Branch {
                    target: BlockId(3),
                    arguments: Vec::new(),
                });
            }
            blocks[3].terminator = Some(Terminator::Unreachable);
            let body = fe2o3_kernel_ir::FunctionBody {
                parameters: Vec::new(),
                blocks,
            };
            let mut witness = witness;
            witness.before.block = BlockId(1);
            witness.before.first = 0;
            let mut events = [PreparedLifecycleEventV29 {
                block: 1,
                witness,
                operation: Some(issued.clone()),
            }];
            let mut rows = vec![terminal_empty_row(), terminal_empty_row()];
            for (index, row) in rows.iter_mut().enumerate() {
                row.witness.origin = index;
                row.witness.block = BlockId(4 + index as u32);
            }
            let mut failures = PreparedTerminalFailuresV18 {
                rows,
                by_block: vec![(BlockId(4), 0), (BlockId(5), 1)],
                redirects: vec![((BlockId(1), 0), 0), ((BlockId(2), 0), 1)],
                generated: vec![0, 1],
                source_blocks: Vec::new(),
            };
            let floor = budget.storage();
            let result = scoped_slot_attempt_v29(budget, |budget| {
                prepare_lifecycle_ownership_v18(
                    &body,
                    &mut events,
                    split.then_some(&mut failures),
                    budget,
                )
            });
            assert_eq!(result.is_ok(), split);
            assert_eq!(budget.storage(), floor);
            if split {
                assert!(failures.rows.iter().all(|row| row.visited));
                assert!(failures.rows.iter().all(|row| row.operations.is_empty()));
                assert!(failures.rows.iter().all(|row| row.witness.scope_ends == 0));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "execution lifecycle differs from its retained source instance",
                        ..
                    })
                ));
                assert!(failures.rows.iter().all(|row| !row.visited));
            }
        }
    });
}

#[test]
fn direct_terminal_coordinate_mapping_has_exact_inclusive_work_bound() {
    with_terminal_module(FailureFixture::Direct, |owner, _| {
        let relation = owner
            .pending
            .roots
            .iter()
            .find(|root| root.coordinates.root.index() == 1)
            .unwrap()
            .terminal_failures
            .as_ref()
            .unwrap();
        let direct = relation.closures.iter().find(|row| !row.generated).unwrap();
        assert_eq!(direct.scope_ends, 1);
        let work_bound = relation.closures.len();
        assert_eq!(work_bound, 2);
        for (gap, original, expected) in [
            (true, direct.original_gap, direct.first),
            (false, direct.original_gap, direct.diagnostic),
            (true, direct.original_gap + 1, direct.diagnostic + 1),
        ] {
            for limit in [work_bound, work_bound - 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 53);
                budget.reserve_storage(53).unwrap();
                let result = terminal_failure_ordinal_v18(
                    Some(relation),
                    direct.block,
                    original,
                    gap,
                    &mut budget,
                );
                if limit == work_bound {
                    assert_eq!(result.unwrap(), expected);
                    assert_eq!(budget.work(), work_bound);
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                    assert_eq!(budget.work(), 0);
                }
                assert_eq!(budget.storage(), 53);
                assert_eq!(budget.failed_storage(), None);
                drop(budget);
                assert_eq!(
                    work.failed_work(),
                    (limit < work_bound).then_some(work_bound)
                );
            }
        }
    });
}

#[test]
fn raw_terminal_and_ordinary_coordinates_compose_from_the_same_original_position() {
    with_terminal_module(FailureFixture::Direct, |owner, _| {
        let root = owner
            .pending
            .roots
            .iter_mut()
            .find(|root| root.coordinates.root.index() == 1)
            .unwrap();
        let relation = root.terminal_failures.as_ref().unwrap();
        let direct = *relation.closures.iter().find(|row| !row.generated).unwrap();
        assert_eq!(direct.scope_ends, 1);
        assert_eq!(relation.closures.len(), 2);
        // These substituted locators test only coordinate arithmetic, not
        // source admission. Restore the genuine rows before the owner settles.
        let original = std::mem::take(&mut root.insertions);
        let mut before = original[0];
        before.before.block = direct.block;
        before.before.first = direct.original_gap;
        let mut later = before;
        later.before.first = direct.original_gap + 2;
        let mut elsewhere = before;
        elsewhere.before.block = BlockId(direct.block.0.checked_add(100).unwrap());
        root.insertions = vec![before, later, elsewhere];
        const WORK: usize = 1 + 3 * 3 + 2;
        for (offset, gap, delta) in [
            (0, true, 0),
            (0, false, 2),
            (1, true, 2),
            (1, false, 2),
            (2, true, 2),
            (2, false, 3),
        ] {
            for limit in [WORK, WORK - 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 53);
                budget.reserve_storage(53).unwrap();
                let p = direct.original_gap as usize + offset;
                let result = scoped_raw_admission_v29::test_immutable_memory_gap_v29(
                    root,
                    direct.block,
                    p,
                    gap,
                    &mut budget,
                );
                if limit == WORK {
                    assert_eq!(result.unwrap(), p + delta);
                    assert_eq!(budget.work(), WORK);
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Work(_)
                        ))
                    ));
                    assert_eq!(budget.work(), WORK - 2);
                }
                assert_eq!(budget.storage(), 53);
                assert_eq!(budget.failed_storage(), None);
                drop(budget);
                assert_eq!(work.failed_work(), (limit < WORK).then_some(WORK));
            }
        }
        let substituted = std::mem::replace(&mut root.insertions, original);
        drop(substituted);
    });
}
