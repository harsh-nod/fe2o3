use super::*;
use std::collections::VecDeque;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
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
            if !std::mem::replace(&mut seen[next as usize], true) {
                pending.push_back(next);
            }
        }
    }
    false
}

#[test]
fn first_root_closure_is_exact_for_every_four_node_graph_and_seed() {
    for mask in 0u32..65536 {
        let edges = (0..4)
            .map(|a| {
                (0..4)
                    .filter(|b| mask & (1 << (a * 4 + b)) != 0)
                    .collect::<Vec<u32>>()
            })
            .collect::<Vec<_>>();
        for seed in 0..4 {
            let mut graph = SccGraph::new(edges.clone());
            let mut work = budget(MAX_FLOW_WORK);
            assert_eq!(
                graph.shortcut(seed, seed, &mut work).unwrap(),
                Some(oracle(&edges, seed, seed))
            );
            let index = graph.index.as_ref().unwrap();
            let seeded = index.component[seed as usize];
            assert_eq!(
                index
                    .flags
                    .iter()
                    .filter(|flags| **flags & FIRST_ROOT != 0)
                    .count(),
                1
            );
            for target in 0..4 {
                assert_eq!(
                    index.component[target] <= seeded,
                    target == seed as usize || oracle(&edges, seed, target as u32)
                );
            }
            for start in 0..4 {
                let component = index.component[start];
                let first = index.first_descendant[start];
                assert!(first <= component);
                for end in 0..4 {
                    let target = index.component[end];
                    if component == target {
                        assert_eq!(first, index.first_descendant[end]);
                    } else if first <= target && target < component {
                        assert!(oracle(&edges, start as u32, end as u32),
                            "unproved interval mask={mask} seed={seed} {start}->{end}");
                    }
                }
            }
            for start in 0..4 {
                for end in 0..4 {
                    let answer = graph.shortcut(start, end, &mut work).unwrap();
                    if start == seed {
                        assert!(answer.is_some());
                    }
                    if let Some(answer) = answer {
                        assert_eq!(
                            answer,
                            oracle(&edges, start, end),
                            "mask={mask} seed={seed} {start}->{end}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn first_root_flag_never_turns_an_acyclic_self_query_positive() {
    let mut graph = SccGraph::new(vec![vec![1], vec![], vec![]]);
    let mut work = budget(MAX_FLOW_WORK);
    assert_eq!(graph.shortcut(2, 2, &mut work).unwrap(), Some(false));
    let index = graph.index.as_ref().unwrap();
    assert_eq!(index.flags[index.component[2] as usize], FIRST_ROOT);
    // This later DFS subtree proves its own descendant, not the seed's flag.
    assert_eq!(graph.shortcut(0, 1, &mut work).unwrap(), Some(true));
    assert_eq!(graph.shortcut(1, 2, &mut work).unwrap(), None);
    assert!(oracle(graph.edges(), 0, 1));
    assert!(!oracle(graph.edges(), 1, 2));
}

#[test]
fn every_member_of_the_first_root_scc_shares_only_that_exact_closure() {
    let mut graph = SccGraph::new(vec![vec![1], vec![2], vec![1, 3], vec![], vec![0]]);
    let mut work = budget(MAX_FLOW_WORK);
    assert_eq!(graph.shortcut(2, 3, &mut work).unwrap(), Some(true));
    assert_eq!(graph.shortcut(1, 3, &mut work).unwrap(), Some(true));
    assert_eq!(graph.shortcut(1, 1, &mut work).unwrap(), Some(true));
    assert_eq!(graph.shortcut(2, 2, &mut work).unwrap(), Some(true));
    assert_eq!(graph.shortcut(2, 0, &mut work).unwrap(), Some(false));
    assert_eq!(graph.shortcut(2, 4, &mut work).unwrap(), Some(false));
    assert_eq!(graph.shortcut(0, 3, &mut work).unwrap(), None);
}

#[test]
fn repeated_seed_queries_retain_exact_constant_debits_without_dfs_storage() {
    let count = 512;
    let seed = 251;
    let edges = (0..count)
        .map(|i| if i + 1 < count { vec![i + 1] } else { vec![] })
        .collect();
    let mut order = Order {
        graph: SccGraph::new(edges),
        entry: seed,
        cache: BTreeMap::new(),
        scratch: None,
    };
    let mut work = budget(MAX_FLOW_WORK);
    assert!(!order.path(seed, seed, &mut work).unwrap());
    let index = order.graph.index.as_ref().unwrap();
    let pointers = (index.component.as_ptr(), index.flags.as_ptr());
    for end in 0..count {
        let before = work.remaining;
        assert_eq!(order.path(seed, end, &mut work).unwrap(), end > seed);
        let expected = if end == seed {
            1
        } else if end > seed {
            4
        } else {
            3
        };
        assert_eq!(before - work.remaining, expected);
    }
    assert!(order.scratch.is_none());
    let index = order.graph.index.as_ref().unwrap();
    assert_eq!(pointers, (index.component.as_ptr(), index.flags.as_ptr()));
    assert_eq!(index.component.capacity(), count as usize);
    assert_eq!(index.flags.capacity(), count as usize);
    assert_eq!(size_of::<Index>(), size_of::<(Vec<u32>, Vec<u8>, Vec<u32>)>());
}

#[test]
fn failed_build_or_final_publication_never_caches_a_partial_closure() {
    let edges = vec![vec![1], vec![2], vec![], vec![3]];
    let mut full = Order {
        graph: SccGraph::new(edges.clone()),
        entry: 1,
        cache: BTreeMap::new(),
        scratch: None,
    };
    let mut work = budget(MAX_FLOW_WORK);
    work.charge(31).unwrap();
    assert!(full.path(1, 2, &mut work).unwrap());
    let required = MAX_FLOW_WORK - work.remaining;
    for limit in 31..required {
        let mut order = Order {
            graph: SccGraph::new(edges.clone()),
            entry: 1,
            cache: BTreeMap::new(),
            scratch: None,
        };
        let mut work = budget(limit);
        work.charge(31).unwrap();
        let error = order.path(1, 2, &mut work).unwrap_err();
        let ProductionSemanticSsaErrorV1::BorrowFlowWork {
            error,
            remaining_work_units,
            requested_work_units,
            phase_work_units,
            ..
        } = error
        else {
            panic!("lost flow error")
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
                limit
            }
        );
        assert!(order.cache.is_empty());
        assert_eq!(order.graph.edges(), edges);
    }
    let mut bad = SccGraph::new(vec![vec![], vec![99]]);
    assert!(matches!(
        bad.shortcut(0, 0, &mut budget(MAX_FLOW_WORK)),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert!(
        bad.index.is_none(),
        "unreachable malformed edges still reject"
    );
}
