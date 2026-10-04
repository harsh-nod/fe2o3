use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const WORK: usize = 10_000_000;
const STORAGE: usize = 16 * 1024 * 1024;

fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}
fn place(index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], SemanticTypeIdV1::from_index(1)).unwrap()
}
fn moved_local(index: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(index))
}
fn edge(index: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(index),
    )
}
fn go(index: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(index))
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn nop() -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Nop)
}
fn anchor() -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
        place(1),
        SemanticOperandV1::Copy(place(2)),
        SemanticVolatilityV1::NonVolatile,
        None,
    )))
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

// Grammar/solver components deliberately do not claim source-owner admission.
// The separate genuine constructed-source tests exercise that enclosing path.
fn function(blocks: Vec<SemanticBasicBlockV1>, entry: u32) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([11; 32]),
        SemanticLayoutIdentityV1::from_sha256([12; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![])
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([2; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([3; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([4; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([5; 32]),
        source,
        abi,
        vec![],
        SemanticBlockIdV1::from_index(entry),
        blocks,
    )
    .unwrap()
}

fn kills(function: &SemanticFunctionDeclV1) -> Vec<Kill> {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let mut rows = vec![];
    for (block, body) in function.blocks().iter().enumerate() {
        for (ordinal, value) in body.statements().iter().enumerate() {
            statement_kills(value.kind(), &mut budget, |local, hard, _| {
                rows.push(Kill {
                    function: 0,
                    block,
                    local,
                    statement: ordinal,
                    hard,
                });
                Ok(())
            })
            .unwrap();
        }
        terminator_kills(body.terminator().kind(), &mut budget, |local, hard, _| {
            rows.push(Kill {
                function: 0,
                block,
                local,
                statement: body.statements().len(),
                hard,
            });
            Ok(())
        })
        .unwrap();
    }
    rows
}

fn query(anchor: u32, first: u32, block: u32, last: u32) -> Query {
    Query {
        store: 3,
        function: SemanticFunctionIdV1::from_index(0),
        anchor: SemanticBlockIdV1::from_index(anchor),
        first,
        local: local(1),
        block: SemanticBlockIdV1::from_index(block),
        last,
    }
}

fn solve(function: &SemanticFunctionDeclV1, query: Query, accepted: bool) {
    let rows = kills(function);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let result = solve_function(function, &[query], &rows, &mut budget);
    if accepted {
        result.unwrap();
    } else {
        assert!(matches!(
            result,
            Err(E::Unsupported {
                phase: "private source",
                detail: "exact cross-block anchor survives every source path"
            })
        ));
    }
}

#[test]
fn source_solver_uses_declared_entry_and_exact_anchor_not_block_zero() {
    let source = function(
        vec![
            block(20, vec![nop()], SemanticTerminatorKindV1::Return),
            block(21, vec![anchor()], go(0)),
        ],
        1,
    );
    solve(&source, query(1, 0, 0, 0), true);
    let bypass = function(
        vec![
            block(20, vec![nop()], SemanticTerminatorKindV1::Return),
            block(21, vec![anchor()], go(0)),
        ],
        0,
    );
    solve(&bypass, query(1, 0, 0, 0), false);
    let entry_backedge = function(
        vec![
            block(20, vec![nop()], go(1)),
            block(21, vec![anchor()], go(0)),
        ],
        0,
    );
    solve(&entry_backedge, query(1, 0, 0, 0), false);
}

#[test]
fn source_lifetime_kills_remain_inclusive_and_unrelated_kills_do_not_interfere() {
    for kind in [
        SemanticStatementKindV1::StorageLive(local(1)),
        SemanticStatementKindV1::StorageDead(local(1)),
        SemanticStatementKindV1::Deinitialize(place(1)),
        SemanticStatementKindV1::Assume(moved_local(1)),
    ] {
        for at_read in [false, true] {
            let mut first = vec![anchor()];
            let mut second = vec![nop()];
            if at_read {
                second[0] = statement(kind.clone());
            } else {
                first.push(statement(kind.clone()));
            }
            let source = function(
                vec![
                    block(20, first, go(1)),
                    block(21, second, SemanticTerminatorKindV1::Return),
                ],
                0,
            );
            solve(&source, query(0, 0, 1, 0), false);
        }
    }
    let source = function(
        vec![
            block(
                20,
                vec![
                    anchor(),
                    statement(SemanticStatementKindV1::StorageDead(local(2))),
                ],
                go(1),
            ),
            block(21, vec![nop()], SemanticTerminatorKindV1::Return),
        ],
        0,
    );
    solve(&source, query(0, 0, 1, 0), true);
}

#[test]
fn source_anchor_reexecution_repairs_earlier_kill_but_not_endpoint_move() {
    let source = function(
        vec![
            block(
                20,
                vec![
                    statement(SemanticStatementKindV1::StorageDead(local(1))),
                    anchor(),
                ],
                go(1),
            ),
            block(
                21,
                vec![nop()],
                SemanticTerminatorKindV1::FalseEdge {
                    real_target: edge(0),
                    imaginary_target: edge(2),
                },
            ),
            block(22, vec![], SemanticTerminatorKindV1::Return),
        ],
        0,
    );
    solve(&source, query(0, 1, 1, 0), true);
    let moving_anchor = statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
        place(1),
        moved_local(1),
        SemanticVolatilityV1::NonVolatile,
        None,
    )));
    let source = function(
        vec![
            block(20, vec![moving_anchor], go(1)),
            block(21, vec![nop()], SemanticTerminatorKindV1::Return),
        ],
        0,
    );
    solve(&source, query(0, 0, 1, 0), false);
    let move_assignment = |destination| {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(destination),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticRvalueKindV1::Use(moved_local(1)),
            ),
        )))
    };
    for at_read in [false, true] {
        let source = function(
            vec![
                block(
                    20,
                    vec![if at_read {
                        anchor()
                    } else {
                        move_assignment(1)
                    }],
                    go(1),
                ),
                block(
                    21,
                    vec![if at_read { move_assignment(2) } else { nop() }],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            0,
        );
        solve(&source, query(0, 0, 1, 0), false);
    }
}

#[test]
fn source_backedge_kill_and_distinct_store_cannot_reuse_old_anchor() {
    for kill in [
        statement(SemanticStatementKindV1::StorageDead(local(1))),
        anchor(),
    ] {
        let source = function(
            vec![
                block(20, vec![anchor()], go(1)),
                block(21, vec![nop()], go(2)),
                block(22, vec![kill], go(1)),
            ],
            0,
        );
        solve(&source, query(0, 0, 1, 0), false);
    }
}

#[test]
fn source_false_and_duplicate_edges_keep_all_paths() {
    for target in [1, 2] {
        let source = function(
            vec![
                block(
                    20,
                    vec![anchor()],
                    SemanticTerminatorKindV1::FalseEdge {
                        real_target: edge(1),
                        imaginary_target: edge(target),
                    },
                ),
                block(21, vec![nop()], SemanticTerminatorKindV1::Return),
                block(
                    22,
                    vec![statement(SemanticStatementKindV1::StorageDead(local(1)))],
                    go(1),
                ),
            ],
            0,
        );
        solve(&source, query(0, 0, 1, 0), target == 1);
    }
}

fn statement_rows(kind: SemanticStatementKindV1) -> Vec<(u32, bool)> {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let mut rows = vec![];
    statement_kills(&kind, &mut budget, |local, hard, _| {
        rows.push((local.index(), hard));
        Ok(())
    })
    .unwrap();
    rows
}

fn terminator_rows(kind: SemanticTerminatorKindV1) -> Vec<(u32, bool)> {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let mut rows = vec![];
    terminator_kills(&kind, &mut budget, |local, hard, _| {
        rows.push((local.index(), hard));
        Ok(())
    })
    .unwrap();
    rows
}

#[test]
fn source_statement_moves_include_projected_places_and_destination_effects() {
    let projected = SemanticPlaceV1::new(
        local(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 0,
                    minimum_length: 2,
                    from_end: false,
                },
                SemanticTypeIdV1::from_index(1),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap();
    let assignment = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(2),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(projected)),
        ),
    ));
    assert_eq!(statement_rows(assignment), vec![(1, true), (2, false)]);
    assert_eq!(
        statement_rows(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(2),
            moved_local(1),
            SemanticVolatilityV1::NonVolatile,
            None
        ))),
        vec![(1, true), (2, false)]
    );
    assert_eq!(
        statement_rows(SemanticStatementKindV1::Assume(moved_local(1))),
        vec![(1, true)]
    );
    assert_eq!(
        statement_rows(SemanticStatementKindV1::SetDiscriminant {
            place: place(1),
            variant_index: 0
        }),
        vec![(1, true)]
    );
}

#[test]
fn atomic_value_expected_replacement_and_destination_invalidations_are_complete() {
    let access = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::SingleThread,
    );
    let rmw = SemanticAtomicRmwV1::new(
        place(2),
        place(3),
        moved_local(1),
        SemanticAtomicRmwOpV1::Exchange,
        access,
    );
    assert_eq!(
        statement_rows(SemanticStatementKindV1::AtomicRmw(rmw)),
        vec![(2, true), (3, true), (1, true)]
    );
    let exchange = SemanticAtomicCompareExchangeV1::new(
        place(2),
        place(3),
        moved_local(1),
        moved_local(4),
        access,
        SemanticAtomicOrderingV1::Relaxed,
        false,
    );
    assert_eq!(
        statement_rows(SemanticStatementKindV1::AtomicCompareExchange(exchange)),
        vec![(2, true), (3, true), (1, true), (4, true)]
    );
}

#[test]
fn source_call_drop_switch_and_assert_operands_are_not_silent() {
    let call = SemanticDirectCallV1::new(
        SemanticFunctionIdV1::from_index(2),
        vec![moved_local(1)],
        Some(SemanticCallDestinationV1::new(place(2), edge(1))),
        SemanticUnwindActionV1::Cleanup(edge(2)),
    )
    .unwrap();
    assert_eq!(
        terminator_rows(SemanticTerminatorKindV1::Call(call)),
        vec![(1, true), (2, true)]
    );
    let tail = SemanticDirectTailCallV1::new(
        SemanticFunctionIdV1::from_index(2),
        vec![moved_local(1)],
        SemanticUnwindActionV1::Cleanup(edge(2)),
    )
    .unwrap();
    assert_eq!(
        terminator_rows(SemanticTerminatorKindV1::TailCall(tail)),
        vec![(1, true)]
    );
    assert_eq!(
        terminator_rows(SemanticTerminatorKindV1::Drop {
            place: place(1),
            drop_glue: SemanticFunctionIdV1::from_index(2),
            target: edge(1),
            unwind: SemanticUnwindActionV1::Cleanup(edge(2))
        }),
        vec![(1, true)]
    );
    let targets = SemanticSwitchTargetsV1::new(vec![], edge(1)).unwrap();
    assert_eq!(
        terminator_rows(SemanticTerminatorKindV1::SwitchInt {
            discriminant: moved_local(1),
            targets
        }),
        vec![(1, true)]
    );
    for message in [
        SemanticAssertMessageV1::BoundsCheck {
            length: moved_local(1),
            index: moved_local(2),
        },
        SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: moved_local(1),
            right: moved_local(2),
        },
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: moved_local(1),
            found_alignment: moved_local(2),
        },
    ] {
        assert_eq!(
            terminator_rows(SemanticTerminatorKindV1::Assert {
                condition: moved_local(3),
                expected: true,
                message,
                target: edge(1),
                unwind: SemanticUnwindActionV1::Cleanup(edge(2))
            }),
            vec![(3, true), (1, true), (2, true)]
        );
    }
    for message in [
        SemanticAssertMessageV1::DivisionByZero(moved_local(1)),
        SemanticAssertMessageV1::RemainderByZero(moved_local(1)),
    ] {
        assert_eq!(
            terminator_rows(SemanticTerminatorKindV1::Assert {
                condition: moved_local(3),
                expected: true,
                message,
                target: edge(1),
                unwind: SemanticUnwindActionV1::Unreachable
            }),
            vec![(3, true), (1, true)]
        );
    }
}

#[test]
fn source_call_destination_kills_normal_and_unwind_paths() {
    let call = SemanticDirectCallV1::new(
        SemanticFunctionIdV1::from_index(2),
        vec![],
        Some(SemanticCallDestinationV1::new(place(1), edge(1))),
        SemanticUnwindActionV1::Cleanup(edge(2)),
    )
    .unwrap();
    let source = function(
        vec![
            block(20, vec![anchor()], SemanticTerminatorKindV1::Call(call)),
            block(21, vec![nop()], SemanticTerminatorKindV1::Return),
            block(22, vec![nop()], SemanticTerminatorKindV1::Return),
        ],
        0,
    );
    solve(&source, query(0, 0, 1, 0), false);
    solve(&source, query(0, 0, 2, 0), false);
}

#[test]
fn source_solver_exact_and_one_short_work_use_the_same_sibling_floor() {
    let source = function(
        vec![
            block(20, vec![anchor()], go(1)),
            block(21, vec![nop()], SemanticTerminatorKindV1::Return),
        ],
        0,
    );
    let kills = kills(&source);
    let queries = [query(0, 0, 1, 0)];
    let sibling = vec![19_u8; 37];
    let floor = sibling.capacity() + std::mem::size_of_val(&sibling);
    let (work_limit, storage_limit) = {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        solve_function(&source, &queries, &kills, &mut budget).unwrap();
        (budget.work(), budget.storage())
    };
    let mut exact = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
    {
        let mut budget = AssertOriginBudgetV1::new(&mut exact, storage_limit);
        budget.reserve_storage(floor).unwrap();
        solve_function(&source, &queries, &kills, &mut budget).unwrap();
        assert_eq!(
            (budget.work(), budget.peak_storage()),
            (work_limit, storage_limit)
        );
        assert_eq!(budget.failed_storage(), None);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(exact.failed_work(), None);
    let mut short = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit - 1);
    {
        let mut budget = AssertOriginBudgetV1::new(&mut short, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let error = erased_general_scratch_v1(&mut budget, |budget| {
            solve_function(&source, &queries, &kills, budget)
        })
        .unwrap_err();
        let E::Resource(AssertOriginResourceV1::Work(error)) = error else {
            panic!("source solver Work refusal")
        };
        assert_eq!(
            (error.actual(), error.limit()),
            (work_limit, work_limit - 1)
        );
        assert_eq!(budget.work(), work_limit - 5);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(short.failed_work(), Some(work_limit));
    let mut storage_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(WORK);
    {
        let mut budget = AssertOriginBudgetV1::new(&mut storage_work, storage_limit - 1);
        budget.reserve_storage(floor).unwrap();
        let error = erased_general_scratch_v1(&mut budget, |budget| {
            solve_function(&source, &queries, &kills, budget)
        })
        .unwrap_err();
        let E::Resource(AssertOriginResourceV1::Storage(error)) = error else {
            panic!("source solver Storage refusal")
        };
        assert_eq!(
            (error.actual(), error.limit()),
            (storage_limit, storage_limit - 1)
        );
        assert_eq!(budget.failed_storage(), Some(storage_limit));
        // The two-block queue's final bit vector is refused before population.
        let queue_bits = std::mem::size_of::<Vec<bool>>() + 2 * std::mem::size_of::<bool>();
        assert_eq!(budget.work(), 32);
        assert_eq!(
            budget.peak_storage(),
            storage_limit.checked_sub(queue_bits).unwrap()
        );
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(storage_work.failed_work(), None);
    assert_eq!(sibling, vec![19; 37]);
}
