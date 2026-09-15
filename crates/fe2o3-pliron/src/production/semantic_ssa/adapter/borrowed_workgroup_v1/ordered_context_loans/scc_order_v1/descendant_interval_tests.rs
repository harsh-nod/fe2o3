use super::*;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
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
fn later_dfs_subtree_proves_descendants_but_not_earlier_unrelated_components() {
    let edges = vec![vec![], vec![2], vec![3], vec![], vec![]];
    let mut graph = SccGraph::new(edges.clone());
    let mut work = budget(MAX_FLOW_WORK);
    assert_eq!(graph.shortcut(0, 0, &mut work).unwrap(), Some(false));
    let index = graph.index.as_ref().unwrap();
    assert_eq!(index.flags[index.component[1] as usize] & FIRST_ROOT, 0);
    assert_eq!(graph.shortcut(1, 3, &mut work).unwrap(), Some(true));
    assert_eq!(graph.shortcut(2, 3, &mut work).unwrap(), Some(true));
    assert_eq!(graph.shortcut(1, 1, &mut work).unwrap(), Some(false));
    assert_eq!(graph.shortcut(3, 1, &mut work).unwrap(), Some(false));
    // Component order alone cannot prove either disconnected query.
    assert_eq!(graph.shortcut(1, 0, &mut work).unwrap(), None);
    assert_eq!(graph.shortcut(4, 3, &mut work).unwrap(), None);
    let mut scratch = PathScratch::new(edges.len(), &mut work).unwrap();
    assert!(!scratch.path(&edges, 1, 0, &mut work).unwrap());
    assert!(!scratch.path(&edges, 4, 3, &mut work).unwrap());
}

#[test]
fn completed_lowlinks_are_reused_only_after_parent_propagation_and_scc_membership() {
    let edges = vec![vec![], vec![2, 4], vec![3], vec![1], vec![5], vec![]];
    let mut graph = SccGraph::new(edges);
    let mut work = budget(MAX_FLOW_WORK);
    assert_eq!(graph.shortcut(0, 0, &mut work).unwrap(), Some(false));
    let index = graph.index.as_ref().unwrap();
    for member in [1, 2, 3] {
        assert_eq!(index.component[member], index.component[1]);
        assert_eq!(index.first_descendant[member], index.first_descendant[1]);
        assert!(index.first_descendant[member] <= index.component[5]);
    }
    for member in [1, 2, 3] {
        assert_eq!(
            graph.shortcut(member, member, &mut work).unwrap(),
            Some(true)
        );
        assert_eq!(graph.shortcut(member, 5, &mut work).unwrap(), Some(true));
    }
    assert_eq!(graph.shortcut(4, 1, &mut work).unwrap(), Some(false));
    assert_eq!(graph.shortcut(5, 5, &mut work).unwrap(), Some(false));
}

#[test]
fn interval_query_keeps_spent_prefix_and_never_publishes_before_last_charge() {
    let edges = vec![vec![], vec![2], vec![3], vec![]];
    let prefix = 37;
    let mut full = order(edges.clone());
    let mut work = budget(MAX_FLOW_WORK);
    work.charge(prefix).unwrap();
    assert!(!full.path(0, 0, &mut work).unwrap());
    assert!(full.path(1, 3, &mut work).unwrap());
    let required = MAX_FLOW_WORK - work.remaining;
    assert!(full.scratch.is_none());
    for limit in prefix..required {
        let mut query = order(edges.clone());
        let mut work = budget(limit);
        work.charge(prefix).unwrap();
        let error = query
            .path(0, 0, &mut work)
            .and_then(|_| query.path(1, 3, &mut work))
            .unwrap_err();
        let ProductionSemanticSsaErrorV1::BorrowFlowWork {
            error,
            remaining_work_units,
            requested_work_units,
            phase_work_units,
            ..
        } = error
        else {
            panic!("original budget failure required")
        };
        assert!(requested_work_units > remaining_work_units);
        assert_eq!(remaining_work_units, work.remaining);
        assert_eq!(
            phase_work_units.iter().sum::<usize>(),
            limit - work.remaining
        );
        assert_eq!(
            *error,
            ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                required: limit + 1,
                limit,
            }
        );
        assert!(!query.cache.contains_key(&(1, 3)));
        assert_eq!(query.graph.edges(), edges);
    }
    let mut exact = order(edges);
    let mut work = budget(required);
    work.charge(prefix).unwrap();
    assert!(!exact.path(0, 0, &mut work).unwrap());
    assert!(exact.path(1, 3, &mut work).unwrap());
    assert_eq!(work.remaining, 0);
}

#[test]
fn distinct_forward_queries_share_one_index_and_the_unchanged_work_ceiling() {
    // Synthetic CFG, not held's source body. No pair repeats, so the existing
    // identical-pair cache cannot hide repeated graph traversals in this test.
    let count = 64u32;
    let edges = (0..count)
        .map(|i| if i + 1 < count { vec![i + 1] } else { vec![] })
        .collect();
    let mut query = order(edges);
    let prefix = 180_000;
    let mut work = budget(MAX_FLOW_WORK);
    work.charge(prefix).unwrap();
    assert!(!query.path(0, 0, &mut work).unwrap());
    for start in 0..count {
        for end in start + 1..count {
            assert!(query.path(start, end, &mut work).unwrap());
        }
    }
    assert!(
        query.scratch.is_none(),
        "all these positive paths have proved intervals"
    );
    assert_eq!(query.cache.len(), 2017);
    assert!(work.remaining > 0);
    eprintln!(
        "synthetic distinct forward pairs: prefix={prefix} remaining={} path-work={}",
        work.remaining,
        MAX_FLOW_WORK - work.remaining - prefix
    );
}
