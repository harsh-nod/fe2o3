#[test]
fn capability_region_acyclic_matches_edge_reachability() {
    let mut seed = 0x6752_ab4d_u32;
    for _ in 0..160 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 5).map(|_| next() % 8).collect())
            .collect();
        let body = reuse_body(&edges);
        let plan = plan(&body);
        for from in 0..8 {
            for to in 0..8 {
                let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1_000_000).unwrap();
                let region = graph.path_region(from, to).unwrap();
                let actual = graph.region_is_acyclic_reusing_scratch(&region).unwrap();
                let mut expected = true;
                for (block, successors) in edges.iter().enumerate() {
                    if region[block] {
                        for &successor in successors {
                            expected &= !graph.reaches_uncached(successor, block as u32).unwrap();
                        }
                    }
                }
                assert_eq!(actual, expected, "{edges:?}, {from}->{to}");
            }
        }
    }
}

#[test]
fn capability_region_acyclic_preserves_duplicate_edges_and_dead_cycles() {
    for (edges, expected) in [
        (vec![vec![1, 1], vec![2], vec![]], true),
        (vec![vec![1, 3], vec![2], vec![], vec![3]], true),
        (vec![vec![1], vec![1, 2], vec![]], false),
        (vec![vec![1], vec![2], vec![0]], false),
    ] {
        let body = reuse_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let region = graph.path_region(0, 2).unwrap();
        assert_eq!(
            graph.region_is_acyclic_reusing_scratch(&region).unwrap(),
            expected
        );
    }
}

#[test]
fn capability_region_acyclic_budget_failures_keep_prefix_and_no_cache() {
    let body = reuse_body(&[vec![1, 1], vec![2], vec![]]);
    let plan = plan(&body);
    let region = [true; 3];
    let mut measured = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let before = measured.remaining;
    assert!(measured.region_is_acyclic_reusing_scratch(&region).unwrap());
    let cost = before - measured.remaining;
    for available in 0..cost {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        graph.remaining = available;
        assert!(matches!(
            graph.region_is_acyclic_reusing_scratch(&region),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 100_001,
                limit: 100_000,
            })
        ));
        assert!(graph.remaining <= available);
        assert!(graph.reuse.reachability.is_empty());
        assert!(graph.reuse.loans.is_empty());
    }
    // Reuse removes only the two paid vector headers and their V-slot payloads.
    // Reset, bookkeeping, both mask scans, vertices, and edges are still charged.
    let header = 2 * std::mem::size_of::<Vec<usize>>() / std::mem::size_of::<usize>();
    let reuse_saving = header + 2 * region.len();
    let warm_cost = cost.checked_sub(reuse_saving).unwrap();
    assert!(warm_cost > 0);
    measured.remaining = cost;
    assert!(measured.region_is_acyclic_reusing_scratch(&region).unwrap());
    assert_eq!(measured.remaining, reuse_saving);
    measured.remaining = warm_cost;
    assert!(measured.region_is_acyclic_reusing_scratch(&region).unwrap());
    assert_eq!(measured.remaining, 0);
    for available in 0..warm_cost {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        assert!(graph.region_is_acyclic_reusing_scratch(&region).unwrap());
        graph.remaining = available;
        assert!(matches!(
            graph.region_is_acyclic_reusing_scratch(&region),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: 100_001,
                limit: 100_000,
            })
        ));
        assert!(graph.remaining <= available);
        assert!(graph.reuse.reachability.is_empty());
        assert!(graph.reuse.loans.is_empty());
        assert!(graph.reuse.loan_regions.is_empty());
        assert_eq!(graph.reuse.region_acyclic_scratch.is_some(), available < 2);
    }
    assert!(matches!(
        measured.region_is_acyclic_reusing_scratch(&[]),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[test]
fn capability_region_acyclic_loan_avoids_per_edge_graph_searches() {
    let edges: Vec<Vec<u32>> = (0..128)
        .map(|block| if block < 127 { vec![block + 1] } else { vec![] })
        .collect();
    let body = reuse_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1_000_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 1_000_000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    let consumer = reuse_consumer(127, None);
    graph.loan_live(loan(owner, 1), consumer).unwrap();
    reference
        .loan_live_before_region_precheck(loan(owner, 1), consumer)
        .unwrap();
    assert!(graph.reuse.reachability.len() <= 1);
    assert!(1_000_000 - graph.remaining < (1_000_000 - reference.remaining) / 4);
}

#[test]
fn capability_region_acyclic_matches_prechange_loan_errors() {
    let mut seed = 0x4e67_d238_u32;
    for _ in 0..96 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 4).map(|_| next() % 8).collect())
            .collect();
        let body = reuse_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let owner = graph.use_value(0, 1).unwrap();
        for block in 0..8 {
            for statement in [Some(0), None] {
                let consumer = reuse_consumer(block, statement);
                reuse_assert_same(
                    graph.loan_live(loan(owner, 1), consumer),
                    reference.loan_live_before_region_precheck(loan(owner, 1), consumer),
                );
            }
        }
    }
}
