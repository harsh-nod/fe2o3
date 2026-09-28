use super::*;
use crate::{
    BasicBlock, CanonicalKernelIrWorkBudgetV1 as Work, Signature, Terminator, Type, ValueId,
};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 37;

fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}
fn choose(left: u32, right: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(left),
        then_arguments: vec![],
        else_target: BlockId(right),
        else_arguments: vec![],
    }
}
fn graph(duplicate: bool, bypass: bool) -> Function {
    let block = |id, terminator| {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(terminator);
        block
    };
    Function::internal_helper(
        "raw-scoped-cfg",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![
            block(90, choose(10, if duplicate { 10 } else { 30 })),
            block(10, branch(20)),
            block(20, choose(10, 40)),
            block(
                30,
                if bypass {
                    branch(20)
                } else {
                    Terminator::Return { values: vec![] }
                },
            ),
            block(40, Terminator::Return { values: vec![] }),
            block(99, branch(99)),
        ],
    )
}
fn run(
    function: &Function,
    work_limit: usize,
    storage_limit: usize,
    query: bool,
) -> (Result<(), Error>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_function_control_flow_v1(function, Default::default(), &mut budget, |view| {
        assert!(std::ptr::eq(view.function(), function));
        if query {
            assert!(view.success_edge_dominates(BlockId(90), 0, BlockId(40))?);
            assert!(!view.is_reachable(BlockId(99))?);
            assert!(!view.dominates(BlockId(99), BlockId(99))?);
        }
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn original_nonordinal_ids_success_region_and_internal_backedge_are_checked() {
    assert_eq!(run(&graph(false, false), LIMIT, LIMIT, true).0, Ok(()));
}

#[test]
fn opposite_duplicate_edge_and_loop_bypass_cannot_supply_a_success_guard() {
    for function in [graph(true, false), graph(false, true)] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
            assert!(!view.success_edge_dominates(BlockId(90), 0, BlockId(40))?);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn absent_coordinates_are_sticky_errors_not_disconnected_false_facts() {
    let function = graph(false, false);
    for edge in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let expected = if edge {
            Error::InvalidEdge {
                source: BlockId(90),
                ordinal: 2,
            }
        } else {
            Error::InvalidBlock(BlockId(123))
        };
        let result =
            with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
                let first = if edge {
                    view.success_edge_dominates(BlockId(90), 2, BlockId(40))
                } else {
                    view.is_reachable(BlockId(123))
                };
                assert_eq!(first, Err(expected.clone()));
                let work = view.budget.work();
                assert_eq!(view.is_reachable(BlockId(90)), Err(expected.clone()));
                assert_eq!(view.budget.work(), work);
                Ok(())
            });
        assert_eq!(result, Err(expected));
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn malformed_graph_refuses_before_callback_and_preserves_floor() {
    let mut function = graph(false, false);
    function.body.as_mut().unwrap().blocks[0].terminator = Some(branch(404));
    let mut called = false;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_function_control_flow_v1(&function, Default::default(), &mut budget, |_| {
        called = true;
        Ok(())
    });
    assert!(matches!(result, Err(Error::ControlFlow(_))));
    assert!(!called);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn callback_error_and_original_panic_settle_backing_before_refund() {
    let function = graph(false, false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let error = Error::InvalidBlock(BlockId(808));
    assert_eq!(
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |_| Err(
            error.clone()
        )),
        Err(error)
    );
    assert_eq!(budget.storage(), FLOOR);
    let marker = std::sync::Arc::new(());
    let expected = marker.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |_| {
            std::panic::panic_any(marker)
        })
    }));
    let payload = result
        .unwrap_err()
        .downcast::<std::sync::Arc<()>>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&payload, &expected));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn independent_scope_header_and_query_costs_match_exact_and_one_short_limits() {
    let function = graph(false, false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let flow = analyze_control_flow_with_verification_budget_v1(
        &function,
        Default::default(),
        &mut budget,
    )
    .unwrap();
    let bare_work = budget.work();
    let bare_peak = budget.peak_storage();
    let header = size_of::<Option<MeteredIndexedControlFlowV1>>()
        + size_of::<Option<Error>>()
        + size_of::<Option<Vec<(ValueId, Option<ValueId>)>>>()
        + size_of::<FunctionControlFlowViewV1<'_, '_, '_>>()
        + size_of::<Result<(), Error>>()
        + size_of::<Result<Result<(), Error>, Box<dyn std::any::Any + Send>>>()
        + 3 * size_of::<usize>();
    drop(flow);
    let constructor = run(&function, LIMIT, LIMIT, false);
    assert_eq!(constructor.0, Ok(()));
    assert_eq!(constructor.1, bare_work + 4);
    assert_eq!(constructor.2, bare_peak + header);
    let query = run(&function, LIMIT, LIMIT, true);
    // Two block searches per edge/dominance query, one per reachability;
    // incoming target rows include the selected edge and one internal backedge.
    let lookup = 6usize.ilog2() as usize + 3;
    assert_eq!(query.1, constructor.1 + 5 * lookup + 3 + 5 + 2 * 6 + 1 + 4);
    assert_eq!(query.2, constructor.2);
    assert_eq!(run(&function, query.1, query.2, true).0, Ok(()));
    assert!(matches!(
        run(&function, query.1 - 1, query.2, true).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(&function, query.1, query.2 - 1, true).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert!(matches!(
        run(&function, constructor.1 - 1, constructor.2, false).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(&function, constructor.1, constructor.2 - 1, false).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
}

fn origin_graph(ambiguous: bool) -> Function {
    let mut entry = BasicBlock::new(BlockId(90));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(10),
        then_arguments: vec![ValueId(0)],
        else_target: BlockId(10),
        else_arguments: vec![ValueId(u32::from(ambiguous))],
    });
    let mut header = BasicBlock::new(BlockId(10));
    header.parameters = vec![crate::ValueDef {
        id: ValueId(20),
        ty: Type::INDEX,
    }];
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(10),
        then_arguments: vec![ValueId(20)],
        else_target: BlockId(30),
        else_arguments: vec![ValueId(20)],
    });
    let mut exit = BasicBlock::new(BlockId(30));
    exit.parameters = vec![crate::ValueDef {
        id: ValueId(30),
        ty: Type::INDEX,
    }];
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut island = BasicBlock::new(BlockId(40));
    island.parameters = vec![crate::ValueDef {
        id: ValueId(40),
        ty: Type::INDEX,
    }];
    island.terminator = Some(Terminator::Branch {
        target: BlockId(40),
        arguments: vec![ValueId(40)],
    });
    Function::internal_helper(
        "origin-scc",
        Signature::new(vec![Type::INDEX, Type::INDEX, Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, header, exit, island],
    )
}

#[test]
fn unique_origins_preserve_duplicate_edge_arguments_seeded_loops_and_ambiguity() {
    for ambiguous in [false, true] {
        let function = origin_graph(ambiguous);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut completed = false;
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
            let expected = (!ambiguous).then_some(ValueId(0));
            assert_eq!(view.unique_value_origin(ValueId(20))?, expected);
            assert_eq!(view.unique_value_origin(ValueId(30))?, expected);
            assert_eq!(view.unique_value_origin(ValueId(40))?, None);
            assert_eq!(view.unique_value_origin(ValueId(0))?, Some(ValueId(0)));
            let storage = view.budget.storage();
            let before = view.budget.work();
            assert_eq!(view.unique_value_origin(ValueId(30))?, expected);
            assert_eq!(view.budget.storage(), storage);
            assert_eq!(view.budget.work() - before, 1 + 3usize.ilog2() as usize + 3);
            completed = true;
            Ok(())
        })
        .unwrap();
        assert!(completed);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn unique_origins_reject_unseeded_dependencies_and_malformed_occurrences() {
    for fault in 0..3 {
        let mut function = origin_graph(false);
        let body = function.body.as_mut().unwrap();
        match fault {
            0 => {
                body.blocks[3].terminator = Some(Terminator::ConditionalBranch {
                    condition: ValueId(2),
                    then_target: BlockId(40),
                    then_arguments: vec![ValueId(40)],
                    else_target: BlockId(10),
                    else_arguments: vec![ValueId(40)],
                })
            }
            1 => {
                body.blocks[0].terminator = Some(Terminator::Branch {
                    target: BlockId(10),
                    arguments: vec![],
                })
            }
            _ => body.blocks[2].parameters[0].id = ValueId(20),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut completed = false;
        let result =
            with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
                if fault == 0 {
                    assert_eq!(view.unique_value_origin(ValueId(20))?, None);
                } else {
                    assert_eq!(
                        view.unique_value_origin(ValueId(20)),
                        Err(Error::Resource(Resource::Accounting))
                    );
                    let before = view.budget.work();
                    assert_eq!(
                        view.unique_value_origin(ValueId(0)),
                        Err(Error::Resource(Resource::Accounting))
                    );
                    assert_eq!(view.budget.work(), before);
                }
                completed = true;
                Ok(())
            });
        assert!(completed);
        assert_eq!(
            result,
            if fault == 0 {
                Ok(())
            } else {
                Err(Error::Resource(Resource::Accounting))
            }
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn unique_origin_query_resource_boundaries_and_panic_preserve_custody() {
    let function = origin_graph(false);
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
                assert_eq!(view.unique_value_origin(ValueId(30))?, Some(ValueId(0)));
                Ok(())
            });
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    };
    // Calibrated full-scope boundaries; the header equation above is independent.
    let (result, work, peak) = run(LIMIT, LIMIT);
    assert_eq!(result, Ok(()));
    assert_eq!(run(work, peak).0, Ok(()));
    assert!(matches!(
        run(work - 1, peak).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(work, peak - 1).0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let marker = std::sync::Arc::new(());
    let expected = marker.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
            assert_eq!(view.unique_value_origin(ValueId(30))?, Some(ValueId(0)));
            std::panic::panic_any(marker)
        })
    }));
    let payload = result
        .unwrap_err()
        .downcast::<std::sync::Arc<()>>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&payload, &expected));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn unique_origins_share_one_bounded_index_across_growing_transport_chains() {
    let mut previous = None;
    for count in [16usize, 64, 256, 1024] {
        let mut blocks = Vec::new();
        let mut entry = BasicBlock::new(BlockId(0));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(0)],
        });
        blocks.push(entry);
        for i in 1..=count {
            let mut block = BasicBlock::new(BlockId(i as u32));
            block.parameters = vec![crate::ValueDef {
                id: ValueId(i as u32),
                ty: Type::INDEX,
            }];
            block.terminator = Some(if i == count {
                Terminator::Return { values: vec![] }
            } else {
                Terminator::Branch {
                    target: BlockId(i as u32 + 1),
                    arguments: vec![ValueId(i as u32)],
                }
            });
            blocks.push(block);
        }
        let function = Function::internal_helper(
            "origin-chain",
            Signature::new(vec![Type::INDEX], vec![]),
            vec![ValueId(0)],
            blocks,
        );
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut query_work = 0;
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
            let before = view.budget.work();
            for i in 1..=count {
                assert_eq!(
                    view.unique_value_origin(ValueId(i as u32))?,
                    Some(ValueId(0))
                );
            }
            query_work = view.budget.work() - before;
            Ok(())
        })
        .unwrap();
        assert!(query_work > count);
        if let Some(previous) = previous {
            // Quadrupling a chain must not repeat a whole traversal per query.
            assert!(
                query_work < previous * 6,
                "{count}: {query_work} vs {previous}"
            );
        }
        previous = Some(query_work);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn structural_origin_work_failure_preserves_exact_scope_diagnostic_and_history() {
    let function = origin_graph(false);
    let mut entry_work = 0;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
        entry_work = view.budget.work();
        assert_eq!(view.unique_value_origin(ValueId(30))?, Some(ValueId(0)));
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let limit = entry_work + 1;
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut observed = None;
    let result =
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
            assert_eq!(view.budget.work(), entry_work);
            let error = view.unique_value_origin(ValueId(30)).unwrap_err();
            let Error::Resource(Resource::Work(failure)) = &error else {
                panic!("expected original work refusal");
            };
            // The outer query charge succeeds; the first SCC input block charge fails.
            assert_eq!(failure.actual(), limit + 1);
            assert_eq!(failure.limit(), limit);
            assert_eq!(view.budget.work(), limit);
            assert_eq!(view.budget.failed_work(), Some(limit + 1));
            assert_eq!(view.budget.failed_storage(), None);
            assert!(view.origins.is_none());
            let before = (
                view.budget.work(),
                view.budget.storage(),
                view.budget.peak_storage(),
            );
            assert_eq!(view.unique_value_origin(ValueId(0)), Err(error.clone()));
            assert_eq!(
                (
                    view.budget.work(),
                    view.budget.storage(),
                    view.budget.peak_storage()
                ),
                before
            );
            observed = Some(error);
            Ok(())
        });
    assert_eq!(result, Err(observed.expect("genuine origin query entered")));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), limit);
    assert_eq!(budget.failed_work(), Some(limit + 1));
}

#[test]
fn structural_origin_storage_failure_preserves_exact_scope_diagnostic_and_history() {
    #[allow(dead_code)]
    struct LiveOriginMeterMirror<'b, 'w> {
        budget: &'b mut Budget<'w>,
        work: Work,
        bytes: usize,
        storage_limit: usize,
        records: usize,
        record_limit: usize,
    }
    let header = size_of::<LiveOriginMeterMirror<'_, '_>>()
        + 4 * size_of::<Vec<()>>()
        + size_of::<Result<Vec<(ValueId, Option<ValueId>)>, Resource>>()
        + size_of::<Result<Vec<Option<ValueId>>, crate::FormalGuardedMemoryResourceErrorV1>>();
    let function = origin_graph(false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut observed = None;
    let mut accepted_work = 0;
    let result =
        with_function_control_flow_v1(&function, Default::default(), &mut budget, |view| {
            let entry_work = view.budget.work();
            let filler = LIMIT - view.budget.storage();
            view.budget.reserve_storage(filler)?;
            let error = view.unique_value_origin(ValueId(30)).unwrap_err();
            let Error::Resource(Resource::Storage(failure)) = &error else {
                panic!("expected original storage refusal");
            };
            assert_eq!(failure.actual(), LIMIT + header);
            assert_eq!(failure.limit(), LIMIT);
            assert_eq!(view.budget.work(), entry_work + 1);
            assert_eq!(view.budget.storage(), LIMIT);
            assert_eq!(view.budget.peak_storage(), LIMIT);
            assert_eq!(view.budget.failed_storage(), Some(LIMIT + header));
            assert_eq!(view.budget.failed_work(), None);
            assert!(view.origins.is_none());
            assert_eq!(view.unique_value_origin(ValueId(0)), Err(error.clone()));
            assert_eq!(view.budget.work(), entry_work + 1);
            assert_eq!(view.budget.storage(), LIMIT);
            assert_eq!(view.budget.failed_storage(), Some(LIMIT + header));
            view.budget.release_storage(filler)?;
            accepted_work = view.budget.work();
            observed = Some(error);
            Ok(())
        });
    assert_eq!(result, Err(observed.expect("genuine origin query entered")));
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.work(), accepted_work);
    assert_eq!(budget.peak_storage(), LIMIT);
    assert_eq!(budget.failed_storage(), Some(LIMIT + header));
}
