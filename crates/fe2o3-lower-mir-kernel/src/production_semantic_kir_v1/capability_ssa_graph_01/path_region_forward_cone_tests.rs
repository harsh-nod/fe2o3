fn cone_allocation_work(count: usize) -> usize {
    let word = std::mem::size_of::<usize>();
    let row_words = std::mem::size_of::<Vec<u32>>().div_ceil(word);
    std::mem::size_of::<CapabilityPathRegionScratchV1<'_>>().div_ceil(word)
        + count * (row_words + 4)
        + 3
}

#[derive(Clone)]
struct ConeWorkOracle {
    capacities: Vec<usize>,
    completed_from: Option<usize>,
}

impl ConeWorkOracle {
    fn new(count: usize) -> Self {
        Self {
            capacities: vec![0; count],
            completed_from: None,
        }
    }

    // Independent transitive closure and edge counts, never production stamps
    // or traversal order. Capacities model the pinned allocator's exact reserve.
    fn query(
        &mut self,
        edges: &[Vec<u32>],
        from: usize,
        to: usize,
        wrap: bool,
    ) -> (usize, Vec<bool>, bool) {
        let count = edges.len();
        let paths = endpoint_closure(edges);
        if !paths[0][from] {
            return (count, vec![false; count], false);
        }
        let forward = &paths[from];
        let region: Vec<_> = (0..count).map(|v| forward[v] && paths[v][to]).collect();
        let hit = self.completed_from == Some(from);
        let mut cost = count + 5;
        let mut incoming = vec![0; count];
        for source in 0..count {
            if forward[source] {
                for &target in &edges[source] {
                    incoming[target as usize] += 1;
                }
                if !hit {
                    cost += 1 + edges[source].len();
                }
            }
        }
        for target in 0..count {
            if region[target] {
                cost += 1 + incoming[target];
            }
        }
        if !hit {
            cost += 3 + usize::from(wrap) * count;
            if self.completed_from.is_none() {
                cost += cone_allocation_work(count);
            }
            for (capacity, &needed) in self.capacities.iter_mut().zip(&incoming) {
                while *capacity < needed {
                    let old = *capacity;
                    *capacity = (2 * old).max(1);
                    cost += *capacity + old;
                }
            }
            self.completed_from = Some(from);
        }
        (cost, region, hit)
    }
}

fn cone_assert_retained(
    graph: &CapabilitySsaGraphV1<'_>,
    edges: &[Vec<u32>],
    oracle: &ConeWorkOracle,
) {
    let Some(from) = oracle.completed_from else {
        assert!(graph.reuse.path_region_scratch.is_none());
        return;
    };
    let scratch = graph.reuse.path_region_scratch.as_ref().unwrap();
    assert_eq!(scratch.completed_from, Some(from as u32));
    assert!(std::ptr::eq(scratch.body, graph.body));
    assert!(std::ptr::eq(scratch.ssa, graph.ssa));
    assert!(scratch.pending.is_empty());
    assert_eq!(scratch.seen.len(), edges.len());
    assert_eq!(scratch.seen.capacity(), edges.len());
    assert_eq!(scratch.pending.capacity(), edges.len());
    assert_eq!(scratch.predecessors.len(), edges.len());
    assert_eq!(scratch.predecessors.capacity(), edges.len());
    let paths = endpoint_closure(edges);
    for target in 0..edges.len() {
        let row = &scratch.predecessors[target];
        assert_eq!(row.capacity(), oracle.capacities[target]);
        assert_eq!(scratch.seen[target] == scratch.epoch, paths[from][target]);
        if paths[from][target] {
            let mut expected = Vec::new();
            for (source, successors) in edges.iter().enumerate() {
                if paths[from][source] {
                    expected.extend(
                        successors
                            .iter()
                            .filter(|&&v| v as usize == target)
                            .map(|_| source as u32),
                    );
                }
            }
            let mut actual = row.clone();
            actual.sort_unstable();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn capability_path_forward_cone_seeded_pairs_match_closure_and_frozen_walks() {
    let mut seed = 0x9903_18ab_u32;
    let mut hits = 0;
    let mut rebuilds = 0;
    for _ in 0..48 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 5).map(|_| next() % 8).collect())
            .collect();
        let body = graph_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut oracle = ConeWorkOracle::new(edges.len());
        for from in (0..8).rev().chain(0..8).chain([0, 1, 0, 1, 0]) {
            for to in 0..8 {
                let previous = graph.reuse.path_region_scratch.as_ref().map(|s| s.epoch);
                let previous_from = oracle.completed_from;
                let (cost, expected, hit) = oracle.query(&edges, from, to, false);
                let before = graph.remaining;
                let actual = graph.path_region(from as u32, to as u32).unwrap();
                assert_eq!(before - graph.remaining, cost, "{edges:?}: {from}->{to}");
                assert_eq!(actual, expected);
                assert_eq!(
                    actual,
                    old.path_region_before_scratch(from as u32, to as u32)
                        .unwrap()
                );
                if let Some(scratch) = &graph.reuse.path_region_scratch {
                    let rebuilt = !hit && oracle.completed_from != previous_from;
                    assert_eq!(scratch.epoch, previous.unwrap_or(0) + usize::from(rebuilt));
                    hits += usize::from(hit);
                    rebuilds += usize::from(rebuilt);
                }
                cone_assert_retained(&graph, &edges, &oracle);
                assert!(graph.reuse.loans.is_empty());
                assert!(graph.reuse.loan_regions.is_empty());
            }
        }
        assert!(old.reuse.path_region_scratch.is_none());
    }
    assert!(hits > 0);
    assert!(rebuilds > 48);
}

#[test]
fn capability_path_forward_cone_keeps_sccs_stale_rows_and_empty_destinations_exact() {
    for edges in [
        vec![vec![1, 4], vec![2], vec![1, 3], vec![], vec![4], vec![3]],
        vec![
            vec![1; 17],
            vec![0, 2],
            vec![1, 3],
            vec![3],
            vec![5],
            vec![],
        ],
    ] {
        let body = graph_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut oracle = ConeWorkOracle::new(edges.len());
        for (from, to) in [
            (0, 5),
            (0, 3),
            (0, 4),
            (0, 0),
            (1, 1),
            (1, 3),
            (5, 3),
            (3, 0),
            (3, 3),
            (0, 3),
            (0, 4),
        ] {
            let (cost, expected, _) = oracle.query(&edges, from, to, false);
            graph.remaining = cost;
            let actual = graph.path_region(from as u32, to as u32).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(graph.remaining, 0);
            assert_eq!(
                actual,
                old.path_region_before_scratch(from as u32, to as u32)
                    .unwrap()
            );
            cone_assert_retained(&graph, &edges, &oracle);
        }
    }
}

#[test]
fn capability_path_forward_cone_hits_preserve_epoch_rows_and_fresh_masks() {
    let edges = [vec![1, 3], vec![2], vec![1], vec![3], vec![2]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    assert_eq!(graph.path_region(0, 4).unwrap(), [false; 5]);
    let scratch = graph.reuse.path_region_scratch.as_mut().unwrap();
    // A completed cone at the final epoch must still hit without a wrap clear.
    scratch.epoch = usize::MAX;
    for stamp in &mut scratch.seen {
        if *stamp != 0 {
            *stamp = usize::MAX;
        }
    }
    let retained = (
        scratch.seen.clone(),
        scratch.predecessors.clone(),
        scratch
            .predecessors
            .iter()
            .map(Vec::as_ptr)
            .collect::<Vec<_>>(),
        scratch.seen.as_ptr(),
        scratch.pending.as_ptr(),
    );
    for (to, expected, backward) in [
        (2, [true, true, true, false, false], 6),
        (3, [true, false, false, true, false], 4),
        (4, [false; 5], 0),
    ] {
        graph.remaining = edges.len() + 5 + backward;
        let mut result = graph.path_region(0, to).unwrap();
        assert_eq!(result, expected);
        assert_eq!(graph.remaining, 0);
        result.fill(true);
        let scratch = graph.reuse.path_region_scratch.as_ref().unwrap();
        assert_eq!(scratch.epoch, usize::MAX);
        assert_eq!(scratch.seen, retained.0);
        assert_eq!(scratch.predecessors, retained.1);
        assert_eq!(
            scratch
                .predecessors
                .iter()
                .map(Vec::as_ptr)
                .collect::<Vec<_>>(),
            retained.2
        );
        assert_eq!(
            (scratch.seen.as_ptr(), scratch.pending.as_ptr()),
            (retained.3, retained.4)
        );
        assert!(scratch.pending.is_empty());
    }
}

include!("path_region_forward_cone_budget_tests.rs");
include!("path_region_forward_cone_loan_tests.rs");
