use super::*;
use std::collections::VecDeque;

const CAP: usize = 262_144;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}
fn used(budget: &Budget) -> usize {
    budget.limit - budget.remaining
}
fn assert_work_error(error: ProductionSemanticSsaErrorV1, limit: usize) {
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        error,
        phase_work_units,
        remaining_work_units,
        requested_work_units,
        ..
    } = error
    else {
        panic!("missing original flow diagnostic");
    };
    assert!(requested_work_units > remaining_work_units);
    assert_eq!(
        phase_work_units.iter().sum::<usize>(),
        limit - remaining_work_units
    );
    assert_eq!(
        *error,
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: limit + 1,
            limit,
        }
    );
}

fn oracle(edges: &[Vec<u32>], start: u32, end: u32) -> bool {
    let mut seen = vec![false; edges.len()];
    let mut pending = VecDeque::from([start]);
    seen[start as usize] = true;
    while let Some(node) = pending.pop_front() {
        for &next in &edges[node as usize] {
            if next == end {
                return true;
            }
            if !seen[next as usize] {
                seen[next as usize] = true;
                pending.push_back(next);
            }
        }
    }
    false
}

#[test]
fn every_four_node_graph_preserves_all_nonempty_path_answers() {
    let mut shortcuts = [0usize; 3];
    for mask in 0..(1u32 << 16) {
        let edges = (0..4)
            .map(|source| {
                (0..4)
                    .filter(|target| mask & (1 << (source * 4 + target)) != 0)
                    .collect::<Vec<u32>>()
            })
            .collect::<Vec<_>>();
        let mut graph = SccGraph::new(edges.clone());
        let mut fallback_budget = budget(CAP);
        let mut budget = budget(CAP);
        let mut fallback = PathScratch::new(edges.len(), &mut fallback_budget).unwrap();
        for start in 0..4 {
            for end in 0..4 {
                let expected = oracle(&edges, start, end);
                match graph.shortcut(start, end, &mut budget).unwrap() {
                    Some(answer) => {
                        assert_eq!(answer, expected, "mask={mask} {start}->{end}");
                        shortcuts[usize::from(answer)] += 1;
                    }
                    None => {
                        assert_eq!(
                            fallback
                                .path(&edges, start, end, &mut fallback_budget)
                                .unwrap(),
                            expected
                        );
                        shortcuts[2] += 1;
                    }
                }
            }
        }
        let index = graph.index.as_ref().unwrap();
        for (source, targets) in edges.iter().enumerate() {
            for &target in targets {
                assert!(index.component[source] >= index.component[target as usize]);
            }
        }
        assert_eq!(graph.edges(), edges);
    }
    assert!(shortcuts.iter().all(|&count| count > 0));
    println!(
        "all 65536 four-node graphs: false={}, true={}, DFS fallback={}",
        shortcuts[0], shortcuts[1], shortcuts[2]
    );
}

#[test]
fn singleton_cycles_parallel_edges_and_disconnected_rows_remain_exact() {
    let edges = vec![vec![], vec![1, 1], vec![3, 3], vec![2], vec![5], vec![]];
    let mut graph = SccGraph::new(edges.clone());
    let mut budget = budget(CAP);
    assert_eq!(graph.shortcut(0, 0, &mut budget).unwrap(), Some(false));
    assert_eq!(graph.shortcut(1, 1, &mut budget).unwrap(), Some(true));
    assert_eq!(graph.shortcut(2, 2, &mut budget).unwrap(), Some(true));
    assert_eq!(graph.shortcut(2, 3, &mut budget).unwrap(), Some(true));
    assert_eq!(graph.shortcut(3, 2, &mut budget).unwrap(), Some(true));
    for start in 0..6 {
        for end in 0..6 {
            let answer = graph
                .shortcut(start, end, &mut budget)
                .unwrap()
                .unwrap_or_else(|| oracle(graph.edges(), start, end));
            assert_eq!(answer, oracle(&edges, start, end));
        }
    }
    assert_eq!(graph.edges()[1], [1, 1]);
    assert_eq!(graph.edges()[2], [3, 3]);
}

#[test]
fn topology_order_is_only_a_negative_test_between_components() {
    let mut graph = SccGraph::new(vec![vec![], vec![]]);
    let mut budget = budget(CAP);
    assert_eq!(graph.shortcut(0, 1, &mut budget).unwrap(), Some(false));
    // Higher component ID is NOT proof of a forward path.
    assert_eq!(graph.shortcut(1, 0, &mut budget).unwrap(), None);
    assert!(!oracle(graph.edges(), 1, 0));
}

fn order(edges: Vec<Vec<u32>>) -> Order {
    Order {
        graph: SccGraph::new(edges),
        entry: 0,
        cache: BTreeMap::new(),
        scratch: None,
    }
}

#[test]
fn integrated_pair_cache_and_unknown_component_order_use_exact_dfs() {
    // The topological order allows both 1->0 and 2->0, but only the first
    // exists. Both must be answered by retained DFS, not by an SCC guess.
    let mut order = order(vec![vec![], vec![0], vec![]]);
    let mut work = budget(CAP);
    // Seed the isolated root so both later sources still require exact DFS.
    assert!(!order.path(0, 0, &mut work).unwrap());
    assert!(order.path(1, 0, &mut work).unwrap());
    assert!(order.scratch.is_some());
    assert!(!order.path(2, 0, &mut work).unwrap());
    let before = used(&work);
    assert!(order.path(1, 0, &mut work).unwrap());
    assert!(!order.path(2, 0, &mut work).unwrap());
    assert_eq!(used(&work) - before, 2, "same pair-cache-hit charges");
    assert!(!order.path(0, 1, &mut work).unwrap());
    assert!(!order.path(0, 0, &mut work).unwrap());
    assert_eq!(order.cache.len(), 4);
}

#[test]
fn integrated_order_preserves_all_three_node_paths_and_statement_order() {
    for mask in 0..512 {
        let edges = (0..3)
            .map(|a| (0..3).filter(|b| mask & (1 << (a * 3 + b)) != 0).collect())
            .collect::<Vec<Vec<u32>>>();
        for entry in 0..3 {
            let mut order = order(edges.clone());
            order.entry = entry;
            let mut work = budget(CAP);
            for a in 0..3 {
                for b in 0..3 {
                    assert_eq!(order.path(a, b, &mut work).unwrap(), oracle(&edges, a, b));
                }
            }
            for a in 0..6 {
                for b in 0..6 {
                    let a = Site {
                        block: a / 2,
                        statement: a % 2,
                    };
                    let b = Site {
                        block: b / 2,
                        statement: b % 2,
                    };
                    let reachable = |p: Site| p.block == entry || oracle(&edges, entry, p.block);
                    let follows = |x: Site, y: Site| {
                        (x.block == y.block && x.statement <= y.statement)
                            || oracle(&edges, x.block, y.block)
                    };
                    assert_eq!(
                        order.before(a, b, &mut work).unwrap(),
                        a != b && reachable(a) && reachable(b) && follows(a, b) && !follows(b, a)
                    );
                }
            }
        }
    }
}

#[test]
fn integrated_query_never_publishes_a_cache_entry_before_its_last_charge() {
    for (edges, start, end, expected) in [
        (vec![vec![0]], 0, 0, true),
        (vec![vec![], vec![0]], 1, 0, true),
        (vec![vec![], vec![]], 1, 0, false),
    ] {
        let mut full = order(edges.clone());
        let mut work = budget(CAP);
        work.charge(37).unwrap();
        assert_eq!(full.path(start, end, &mut work).unwrap(), expected);
        let required = used(&work);
        for limit in 37..required {
            let mut query = order(edges.clone());
            let mut work = budget(limit);
            work.charge(37).unwrap();
            assert_work_error(query.path(start, end, &mut work).unwrap_err(), limit);
            assert!(query.cache.is_empty());
            assert_eq!(query.graph.edges(), edges);
        }
        assert_work_error(full.path(start, end, &mut budget(0)).unwrap_err(), 0);
        assert_eq!(full.cache.get(&(start, end)), Some(&expected));
    }
}

#[test]
fn invalid_targets_in_unreachable_rows_cannot_publish_an_index() {
    for edges in [vec![vec![], vec![2]], vec![vec![1], vec![u32::MAX]]] {
        let mut graph = SccGraph::new(edges.clone());
        let mut budget = budget(CAP);
        assert_eq!(
            graph.shortcut(0, 0, &mut budget),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        );
        assert!(graph.index.is_none());
        assert_eq!(graph.edges(), edges);
    }
    let mut graph = SccGraph::new(vec![]);
    assert_eq!(
        graph.shortcut(0, 0, &mut budget(CAP)),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
}

#[test]
fn failed_work_never_publishes_partial_components_or_uses_a_fresh_quota() {
    let edges = vec![vec![1], vec![2, 3], vec![1], vec![3]];
    let mut full = SccGraph::new(edges.clone());
    let mut reference = budget(CAP);
    assert_eq!(full.shortcut(2, 1, &mut reference).unwrap(), Some(true));
    let required = used(&reference);
    for available in 0..required {
        let limit = available + 37;
        let mut budget = budget(limit);
        budget.charge(37).unwrap();
        let mut graph = SccGraph::new(edges.clone());
        assert_work_error(graph.shortcut(2, 1, &mut budget).unwrap_err(), limit);
        assert!(graph.index.is_none());
        assert!(used(&budget) >= 37);
        assert_eq!(graph.edges(), edges);
    }
    let mut zero = budget(0);
    assert_work_error(full.shortcut(2, 1, &mut zero).unwrap_err(), 0);
    assert!(full.index.is_some());
    println!("construction work boundaries checked: {required}");
}

#[test]
fn graph_ownership_and_repeated_queries_do_not_rebuild_or_cross_bind() {
    let mut external = vec![vec![1], vec![0]];
    let mut first = SccGraph::new(external.clone());
    external[1].clear();
    let mut second = SccGraph::new(external);
    let mut budget = budget(CAP);
    assert_eq!(first.shortcut(1, 0, &mut budget).unwrap(), Some(true));
    assert_eq!(second.shortcut(1, 0, &mut budget).unwrap(), Some(false));
    let index = first.index.as_ref().unwrap();
    let pointers = (index.component.as_ptr(), index.flags.as_ptr(), index.first_descendant.as_ptr());
    let bytes = (index.component.capacity() + index.first_descendant.capacity()) * size_of::<u32>()
        + index.flags.capacity();
    assert_eq!(bytes, 9 * first.edges().len());
    let before = used(&budget);
    for _ in 0..1000 {
        assert_eq!(first.shortcut(0, 0, &mut budget).unwrap(), Some(true));
    }
    assert_eq!(used(&budget) - before, 1000);
    let index = first.index.as_ref().unwrap();
    assert_eq!(pointers, (index.component.as_ptr(), index.flags.as_ptr(), index.first_descendant.as_ptr()));
}

#[test]
fn repeated_reverse_queries_amortize_real_traversal_at_unchanged_cap() {
    let edges = (0..64)
        .map(|i| if i + 1 < 64 { vec![i + 1] } else { vec![] })
        .collect::<Vec<_>>();
    let mut old_budget = budget(CAP);
    let mut new_budget = budget(CAP);
    let mut old = PathScratch::new(edges.len(), &mut old_budget).unwrap();
    let mut graph = SccGraph::new(edges.clone());
    for start in 1..64 {
        for end in 0..start {
            assert!(!old.path(&edges, start, end, &mut old_budget).unwrap());
            assert_eq!(
                graph.shortcut(start, end, &mut new_budget).unwrap(),
                Some(false)
            );
        }
    }
    assert!(used(&new_budget) < used(&old_budget));
    println!(
        "2016 unique reverse queries on synthetic 64-node chain: work {} -> {}",
        used(&old_budget),
        used(&new_budget)
    );
}

#[test]
fn deep_graphs_are_iterative_and_index_build_is_charged_even_without_savings() {
    let n = 5000;
    let edges = (0..n).map(|i| vec![(i + 1) % n]).collect::<Vec<_>>();
    let mut graph = SccGraph::new(edges);
    let mut budget = budget(CAP);
    assert_eq!(graph.shortcut(4999, 0, &mut budget).unwrap(), Some(true));
    assert!(used(&budget) >= 8 * n as usize);
    assert_eq!(graph.shortcut(0, 0, &mut budget).unwrap(), Some(true));
    println!(
        "synthetic 5000-node cycle: completed build + two queries work={}",
        used(&budget)
    );
}
