use super::*;
use crate::{BasicBlock, Signature, ValueDef, analyze_control_flow};
use meter::LiveGuardMeter;

fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}

fn conditional(left: u32, right: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(left),
        then_arguments: vec![],
        else_target: BlockId(right),
        else_arguments: vec![],
    }
}

fn function(edges: &[Terminator]) -> Function {
    let mut blocks: Vec<_> = edges
        .iter()
        .enumerate()
        .map(|(id, edge)| {
            let mut block = BasicBlock::new(BlockId(id as u32));
            block.terminator = Some(edge.clone());
            block
        })
        .collect();
    blocks[1].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    Function::internal_helper(
        "guard-entry",
        Signature::new(
            vec![
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        blocks,
    )
}

fn done() -> Terminator {
    Terminator::Return { values: vec![] }
}

fn control_row(function: &Function, block: u32) -> ControlRow {
    let flow = analyze_control_flow(function).unwrap();
    let control = GuardedControlV1::collect(function, &flow).unwrap().unwrap();
    *control
        .rows
        .iter()
        .find(|row| row.block == BlockId(block))
        .unwrap()
}

#[test]
fn unique_entering_edge_keeps_only_exact_reachable_external_occurrences() {
    let cases = [
        // Self, multi-block and nested loops all have one initial entrance.
        (vec![conditional(1, 2), conditional(1, 2), done()], true),
        (
            vec![conditional(1, 2), branch(3), done(), conditional(1, 2)],
            true,
        ),
        (
            vec![
                conditional(1, 2),
                branch(3),
                done(),
                conditional(4, 1),
                conditional(3, 1),
            ],
            true,
        ),
        // Both branch occurrences count even when their target/source match.
        (vec![conditional(1, 1), conditional(1, 2), done()], false),
        // A side entry is not a backedge: header 1 does not dominate block 3.
        (
            vec![conditional(1, 3), conditional(3, 2), done(), branch(1)],
            false,
        ),
        // An unreachable incoming block does not supply a first entrance.
        (
            vec![conditional(1, 2), conditional(1, 2), done(), branch(1)],
            true,
        ),
    ];
    for (edges, expected) in cases {
        let function = function(&edges);
        let row = control_row(&function, 1);
        assert!(row.interval.is_some());
        assert!(
            row.incoming
                == expected.then_some(Edge {
                    source: BlockId(0),
                    ordinal: 0,
                    target: BlockId(1)
                })
        );
        assert!(control_row(&function, 0).incoming.is_none());
    }
    // Original entry cycles cannot acquire an incoming predicate seed.
    let function = function(&[conditional(0, 1), done()]);
    assert!(control_row(&function, 0).incoming.is_none());
}

#[test]
fn real_control_collection_has_independent_exact_work_and_storage_boundaries() {
    let function = function(&[conditional(1, 2), conditional(1, 2), done()]);
    let flow = analyze_control_flow(&function).unwrap();
    const FLOOR: usize = 37;
    const PRIOR: usize = 5;
    const BLOCKS: usize = 3;
    const EDGES: usize = 4;
    const LOOKUP: usize = 2 + 4; // ceil_log2(3) plus the existing query envelope.
    const SORT: usize = 4 * BLOCKS * 2;
    // Start, block/op selection (one Load), empty-vector reserve, block queries,
    // all incoming-edge queries, NEW reachable-source interval query, and sort.
    const WORK: usize = 32
        + 2 * BLOCKS
        + 2
        + 2
        + BLOCKS * (2 * LOOKUP + 8)
        + EDGES * (LOOKUP + 8)
        + EDGES * (LOOKUP + 4)
        + SORT;
    assert_eq!(WORK, 222);
    #[allow(dead_code)]
    struct IndependentEdge {
        source: BlockId,
        ordinal: usize,
        target: BlockId,
    }
    #[allow(dead_code)]
    struct IndependentRow {
        block: BlockId,
        interval: Option<(u32, u32)>,
        incoming: Option<IndependentEdge>,
    }
    assert_eq!(size_of::<IndependentRow>(), size_of::<ControlRow>());
    let headers =
        size_of::<GuardedAnalysisV1<'_, LiveGuardMeter<'_, '_>>>() + 5 * size_of::<Vec<()>>();
    let rows = BLOCKS * size_of::<IndependentRow>();
    let storage = headers + rows;
    for prior_denial in [false, true] {
        for cut in 0..4 {
            // Cut 3 stops BEFORE the first new interval query. Entry and header
            // queries plus the header's first old edge debit have cost 96.
            let limit = match cut {
                1 => WORK - 1,
                3 => 96 + LOOKUP + 4 - 1,
                _ => WORK,
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(PRIOR + limit);
            work.charge_work(PRIOR).unwrap();
            let earlier_work = if prior_denial {
                assert!(work.charge_work(WORK + 123).is_err());
                Some(PRIOR + WORK + 123)
            } else {
                None
            };
            let mut budget = Budget::new(&mut work, FLOOR + storage - usize::from(cut == 2));
            budget.reserve_storage(FLOOR).unwrap();
            let earlier_storage = if prior_denial {
                assert!(budget.reserve_storage(storage + 123).is_err());
                Some(FLOOR + storage + 123)
            } else {
                None
            };
            let result = GuardedControlV1::collect_with_ledger(
                &function,
                &flow,
                LiveGuardMeter::new(&mut budget, usize::MAX, usize::MAX, usize::MAX),
            );
            match cut {
                0 => {
                    let control = result.unwrap().unwrap();
                    assert_eq!(control.rows.len(), BLOCKS);
                    assert_eq!(control.rows.capacity(), BLOCKS);
                    assert!(
                        control.rows[1].incoming
                            == Some(Edge {
                                source: BlockId(0),
                                ordinal: 0,
                                target: BlockId(1)
                            })
                    );
                    drop(control);
                    assert_eq!(budget.work(), PRIOR + WORK);
                    assert_eq!(budget.storage(), FLOOR + storage);
                }
                1 | 3 => {
                    assert!(matches!(result, Err(ResourceError::Work(_))));
                    drop(result);
                    assert_eq!(
                        budget.work(),
                        PRIOR + if cut == 1 { WORK - SORT } else { 96 }
                    );
                    assert_eq!(budget.storage(), FLOOR + storage);
                }
                2 => {
                    assert!(
                        matches!(result, Err(ResourceError::Storage { actual, limit })
                        if actual == FLOOR + storage && limit + 1 == actual)
                    );
                    drop(result);
                    assert_eq!(budget.work(), PRIOR + 32 + 2 * BLOCKS + 2 + 2);
                    assert_eq!(budget.storage(), FLOOR + headers);
                }
                _ => unreachable!(),
            }
            let accepted = budget.work();
            let peak = budget.peak_storage();
            let expected_failure = earlier_storage.or((cut == 2).then_some(FLOOR + storage));
            assert_eq!(budget.failed_storage(), expected_failure);
            // Every owned row/result has dropped before the scope settles.
            budget.rollback_storage(FLOOR).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.work(), accepted);
            assert_eq!(budget.failed_storage(), expected_failure);
            drop(budget);
            assert_eq!(
                work.failed_work(),
                earlier_work.or(match cut {
                    1 => Some(PRIOR + WORK),
                    3 => Some(PRIOR + 96 + LOOKUP + 4),
                    _ => None,
                })
            );
        }
    }
}
