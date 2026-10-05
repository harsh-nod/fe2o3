use super::*;
use dialect_kernel::{AnalysisSplitOp, IndexType, ReturnOp};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, types::FunctionType},
    r#type::TypeHandle,
    value::Value,
};
use std::cell::Cell;

thread_local! { static QUERIES: Cell<Option<usize>> = const { Cell::new(None) }; }

pub(super) fn record() {
    QUERIES.with(|counter| {
        if let Some(value) = counter.get() {
            counter.set(Some(value.checked_add(1).unwrap()));
        }
    });
}

fn counted<T>(action: impl FnOnce() -> T) -> (T, usize) {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            QUERIES.with(|counter| counter.set(self.0));
        }
    }
    let _restore = Restore(QUERIES.with(|counter| counter.replace(Some(0))));
    let result = action();
    (result, QUERIES.with(|counter| counter.get().unwrap()))
}

fn append(context: &Context, block: Ptr<BasicBlock>, operation: &impl Op) {
    operation.get_operation().insert_at_back(block, context);
}

fn block(context: &mut Context, function: &FuncOp, count: usize) -> (Ptr<BasicBlock>, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let block = BasicBlock::new(context, None, vec![index; count]);
    block.insert_at_back(function.get_region(context), context);
    let arguments = (0..count)
        .map(|i| block.deref(context).get_argument(i))
        .collect();
    (block, arguments)
}

// Two genuine outside predecessors; optional nested recurrence causes the
// canonical engine to query entries before returning Incomplete to fallback.
fn fixture(nested: bool, duplicate: bool, unequal: bool) -> (Context, FuncOp) {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    let ty = FunctionType::get(&context, vec![], vec![]);
    let function = FuncOp::new(&mut context, "multi_entry".try_into().unwrap(), ty);
    let entry = function.get_entry_block(&context);
    let (left, _) = block(&mut context, &function, 0);
    let (right, _) = block(&mut context, &function, 0);
    let (header, induction) = block(&mut context, &function, 1);
    let (body, body_induction) = block(&mut context, &function, 1);
    let inner = nested.then(|| block(&mut context, &function, 2));
    let inner_body = nested.then(|| block(&mut context, &function, 2));
    let (latch, latch_induction) = block(&mut context, &function, 1);
    let (exit, _) = block(&mut context, &function, 0);
    let zero = IndexConstantOp::new(&mut context, 0);
    let one = IndexConstantOp::new(&mut context, 1);
    let bound = IndexConstantOp::new(&mut context, 3);
    for operation in [&zero, &one, &bound] {
        append(&context, entry, operation);
    }
    let z = zero.result(&context);
    let o = one.result(&context);
    let limit = bound.result(&context);
    let split = AnalysisSplitOp::new_with_arguments(&mut context, vec![], vec![], left, right);
    append(&context, entry, &split);
    let enter = BranchArgsOp::new(&mut context, vec![z], header);
    append(&context, left, &enter);
    let seed = if unequal { o } else { z };
    if duplicate {
        let enter =
            AnalysisSplitOp::new_with_arguments(&mut context, vec![z], vec![seed], header, header);
        append(&context, right, &enter);
    } else {
        let enter = BranchArgsOp::new(&mut context, vec![seed], header);
        append(&context, right, &enter);
    }
    let guard = IndexLessThanBranchArgsOp::new(
        &mut context,
        induction[0],
        limit,
        induction,
        vec![],
        body,
        exit,
    );
    append(&context, header, &guard);
    if let (Some((inner, inner_args)), Some((inner_body, inner_body_args))) = (inner, inner_body) {
        let enter = BranchArgsOp::new(&mut context, vec![z, body_induction[0]], inner);
        append(&context, body, &enter);
        let guard = IndexLessThanBranchArgsOp::new(
            &mut context,
            inner_args[0],
            limit,
            inner_args.clone(),
            vec![inner_args[1]],
            inner_body,
            latch,
        );
        append(&context, inner, &guard);
        let next = IndexBinaryOp::new(
            &mut context,
            IndexBinaryKindAttr::Add,
            inner_body_args[0],
            o,
        );
        append(&context, inner_body, &next);
        let value = next.result(&context);
        let repeat = BranchArgsOp::new(&mut context, vec![value, inner_body_args[1]], inner);
        append(&context, inner_body, &repeat);
    } else {
        let forward = BranchArgsOp::new(&mut context, body_induction, latch);
        append(&context, body, &forward);
    }
    let next = IndexBinaryOp::new(
        &mut context,
        IndexBinaryKindAttr::Add,
        latch_induction[0],
        o,
    );
    append(&context, latch, &next);
    let value = next.result(&context);
    let repeat = BranchArgsOp::new(&mut context, vec![value], header);
    append(&context, latch, &repeat);
    let ret = ReturnOp::new(&mut context);
    append(&context, exit, &ret);
    verify_operation(function.get_operation(), &context).unwrap();
    (context, function)
}

#[test]
fn multi_entry_actual_query_count_covers_canonical_and_duplicate_occurrences() {
    for duplicate in [false, true] {
        let (context, function) = fixture(false, duplicate, false);
        let (report, queries) = counted(|| run_pliron_progress_check_v1(&context, &function));
        assert!(report.is_clean(), "{:?}", report.findings());
        assert_eq!(report.certificates().len(), 1);
        assert_eq!(
            queries, 2,
            "parallel occurrences must all be checked by one source query"
        );
    }
}

#[test]
fn multi_entry_actual_queries_cover_consuming_canonical_then_nested_fallback() {
    for duplicate in [false, true] {
        let (context, function) = fixture(true, duplicate, false);
        let inventory = bounded_structural_inventory(&context, &function).unwrap();
        let blocks = &inventory.root_blocks;
        let indices = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
        let graph = build_root_graph(&context, blocks, &indices).unwrap();
        let dominators = progress_dominators_v1(&graph.edges, &graph.predecessors);
        let mut component = strongly_connected_components(&graph.edges)
            .into_iter()
            .find(|component| is_cycle(component, &graph.edges))
            .unwrap();
        component.sort_unstable();
        let members = component.iter().copied().collect();
        let (canonical, queries) = counted(|| {
            canonical_positive_induction_loop(
                &context,
                blocks,
                &indices,
                &inventory.root_operation_blocks,
                &dominators,
                &graph.predecessors,
                &graph.edges,
                &graph.incoming,
                &component,
                &members,
            )
        });
        assert!(matches!(canonical, CanonicalLoopResultV1::Incomplete(_)));
        assert_eq!(queries, 2);
        let e = graph.edges.iter().map(Vec::len).sum::<usize>();
        for _ in 0..16 {
            let (report, actual) = counted(|| run_pliron_progress_check_v1(&context, &function));
            assert!(report.is_clean(), "{:?}", report.findings());
            assert_eq!(report.certificates().len(), 2);
            assert!(actual > queries);
            assert!(actual <= e * (2 + e), "actual={actual}, E={e}");
        }
    }
}

#[test]
fn multi_entry_ranked_checker_rejects_different_and_parallel_seed_values() {
    for nested in [false, true] {
        for duplicate in [false, true] {
            let (context, function) = fixture(nested, duplicate, true);
            let (report, actual) = counted(|| run_pliron_progress_check_v1(&context, &function));
            assert!(!report.is_clean());
            assert!(report.findings().iter().any(|finding| matches!(
                finding,
                PlironProgressFindingV1::ProgressIncomplete { .. }
            )));
            assert!(actual >= 2);
        }
    }
}

fn propagation_chain(
    count: usize,
    reverse_layout: bool,
    parallel: bool,
    internal_cycle: bool,
    conflicting_parallel: bool,
) -> (Context, FuncOp, Vec<Ptr<BasicBlock>>) {
    assert!(count >= 4);
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    let ty = FunctionType::get(&context, vec![], vec![]);
    let function = FuncOp::new(&mut context, "propagation_chain".try_into().unwrap(), ty);
    let entry = function.get_entry_block(&context);
    let mut chain = (0..count)
        .map(|_| block(&mut context, &function, 1).0)
        .collect::<Vec<_>>();
    if reverse_layout {
        chain.reverse();
    }
    let zero = IndexConstantOp::new(&mut context, 0);
    let seed = zero.result(&context);
    append(&context, entry, &zero);
    let enter = BranchArgsOp::new(&mut context, vec![seed], chain[0]);
    append(&context, entry, &enter);
    for (index, source) in chain.iter().copied().enumerate() {
        let value = source.deref(&context).get_argument(0);
        let target = chain[(index + 1) % count];
        if index + 1 == count && internal_cycle {
            let jump = AnalysisSplitOp::new_with_arguments(
                &mut context,
                vec![value],
                vec![value],
                target,
                chain[count / 2],
            );
            append(&context, source, &jump);
        } else if parallel {
            let jump = AnalysisSplitOp::new_with_arguments(
                &mut context,
                vec![value],
                vec![if conflicting_parallel { seed } else { value }],
                target,
                target,
            );
            append(&context, source, &jump);
        } else {
            let jump = BranchArgsOp::new(&mut context, vec![value], target);
            append(&context, source, &jump);
        }
    }
    verify_operation(function.get_operation(), &context).unwrap();
    (context, function, chain)
}

#[test]
fn nested_induction_worklist_visits_each_occurrence_once_across_chain_sizes() {
    for reverse_layout in [false, true] {
        for count in [16, 32, 64, 128] {
            let (context, function, chain) =
                propagation_chain(count, reverse_layout, false, false, false);
            let inventory = bounded_structural_inventory(&context, &function).unwrap();
            let blocks = &inventory.root_blocks;
            let indices: HashMap<_, _> = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
            let graph = build_root_graph(&context, blocks, &indices).unwrap();
            let members = chain.iter().map(|block| indices[block]).collect();
            let header = indices[&chain[0]];
            let induction = chain[0].deref(&context).get_argument(0);
            let (facts, queries) = counted(|| {
                propagate_loop_induction_v1(
                    &context,
                    blocks,
                    &graph.edges,
                    &members,
                    header,
                    induction,
                )
            });
            let facts = facts.unwrap();
            assert_eq!(
                queries,
                count - 1,
                "count={count}, reverse={reverse_layout}"
            );
            assert_eq!(facts.len(), count);
            for block in chain {
                assert_eq!(
                    facts[&indices[&block]],
                    block.deref(&context).get_argument(0)
                );
            }
        }
    }
}

#[test]
fn nested_induction_worklist_checks_internal_cycles_and_parallel_occurrences() {
    for reverse_layout in [false, true] {
        for parallel in [false, true] {
            let count = 16;
            let (context, function, chain) =
                propagation_chain(count, reverse_layout, parallel, true, false);
            let inventory = bounded_structural_inventory(&context, &function).unwrap();
            let blocks = &inventory.root_blocks;
            let indices: HashMap<_, _> = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
            let graph = build_root_graph(&context, blocks, &indices).unwrap();
            let members = chain.iter().map(|block| indices[block]).collect();
            let (facts, queries) = counted(|| {
                propagate_loop_induction_v1(
                    &context,
                    blocks,
                    &graph.edges,
                    &members,
                    indices[&chain[0]],
                    chain[0].deref(&context).get_argument(0),
                )
            });
            assert_eq!(facts.unwrap().len(), count);
            // Header backedges are excluded; the nonheader cycle's last
            // occurrence must still check an already processed target.
            assert_eq!(queries, (count - 1) * (1 + usize::from(parallel)) + 1);
        }
    }
}

#[test]
fn nested_induction_worklist_refuses_conflicting_join_aliases() {
    for reverse_targets in [false, true] {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let ty = FunctionType::get(&context, vec![], vec![]);
        let diamond = FuncOp::new(&mut context, "conflicting_aliases".try_into().unwrap(), ty);
        let entry = diamond.get_entry_block(&context);
        let (header, header_args) = block(&mut context, &diamond, 1);
        let (left, left_args) = block(&mut context, &diamond, 1);
        let (right, right_args) = block(&mut context, &diamond, 1);
        let (join, join_args) = block(&mut context, &diamond, 2);
        let zero = IndexConstantOp::new(&mut context, 0);
        let z = zero.result(&context);
        append(&context, entry, &zero);
        let enter = BranchArgsOp::new(&mut context, vec![z], header);
        append(&context, entry, &enter);
        let (first, second) = if reverse_targets {
            (right, left)
        } else {
            (left, right)
        };
        let split = AnalysisSplitOp::new_with_arguments(
            &mut context,
            header_args.clone(),
            header_args.clone(),
            first,
            second,
        );
        append(&context, header, &split);
        let first = BranchArgsOp::new(&mut context, vec![left_args[0], z], join);
        append(&context, left, &first);
        let second = BranchArgsOp::new(&mut context, vec![z, right_args[0]], join);
        append(&context, right, &second);
        let repeat = BranchArgsOp::new(&mut context, vec![join_args[0]], header);
        append(&context, join, &repeat);
        verify_operation(diamond.get_operation(), &context).unwrap();
        let inventory = bounded_structural_inventory(&context, &diamond).unwrap();
        let blocks = &inventory.root_blocks;
        let indices: HashMap<_, _> = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
        let graph = build_root_graph(&context, blocks, &indices).unwrap();
        let members = [header, left, right, join]
            .iter()
            .map(|b| indices[b])
            .collect();
        let (result, queries) = counted(|| {
            propagate_loop_induction_v1(
                &context,
                blocks,
                &graph.edges,
                &members,
                indices[&header],
                header_args[0],
            )
        });
        assert!(result.is_err());
        assert_eq!(queries, 4);
    }
}

#[test]
fn nested_induction_worklist_refuses_unequal_parallel_payloads_before_assignment() {
    for reverse_layout in [false, true] {
        let (context, function, chain) = propagation_chain(8, reverse_layout, true, false, true);
        let inventory = bounded_structural_inventory(&context, &function).unwrap();
        let blocks = &inventory.root_blocks;
        let indices: HashMap<_, _> = blocks.iter().enumerate().map(|(i, b)| (*b, i)).collect();
        let graph = build_root_graph(&context, blocks, &indices).unwrap();
        let members = chain.iter().map(|block| indices[block]).collect();
        let (result, queries) = counted(|| {
            propagate_loop_induction_v1(
                &context,
                blocks,
                &graph.edges,
                &members,
                indices[&chain[0]],
                chain[0].deref(&context).get_argument(0),
            )
        });
        assert!(result.is_err());
        assert_eq!(queries, 1);
    }
}
