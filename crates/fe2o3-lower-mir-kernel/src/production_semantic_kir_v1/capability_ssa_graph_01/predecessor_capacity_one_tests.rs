#[test]
fn capability_path_scratch_capacity_one_retains_original_context_prefix_budget() {
    let count = 1536;
    let edges: Vec<Vec<u32>> = (0..count)
        .map(|block| {
            if block + 1 < count {
                vec![block as u32 + 1]
            } else {
                vec![]
            }
        })
        .collect();
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 20_000).unwrap();
    let header = std::mem::size_of::<CapabilityPathRegionScratchV1<'_>>()
        .div_ceil(std::mem::size_of::<usize>());
    let row_words = std::mem::size_of::<Vec<u32>>().div_ceil(std::mem::size_of::<usize>());
    let setup = query_reuse::header_words() + count + body.locals().len();
    // All N vertices and N-1 forward edges are still visited, although only
    // two vertices and one predecessor edge belong to the result region.
    let traversal = count + (count - 1) + 2 + 1;
    // Eight query operations plus three cold binding/key initializations.
    let expected = setup + count * (row_words + 5) + header + 8 + 3 + traversal + (count - 1);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(expected, 18_495);
    assert!(expected < 20_000);
    let region = graph.path_region(0, 1).unwrap();
    assert_eq!(&region[..2], &[true, true]);
    assert!(region[2..].iter().all(|inside| !inside));
    assert_eq!(graph.remaining, 20_000 - expected);
    let scratch = graph.reuse.path_region_scratch.as_ref().unwrap();
    assert_eq!(scratch.predecessors[0].capacity(), 0);
    assert!(
        scratch.predecessors[1..]
            .iter()
            .all(|row| row.capacity() == 1)
    );
    // Same start: only two backward vertices and one predecessor are visited.
    let warm = count + 5 + 2 + 1;
    graph.remaining = warm;
    assert_eq!(graph.path_region(0, 1).unwrap(), region);
    assert_eq!(graph.remaining, 0);
    assert!(graph.reuse.loans.is_empty());
    assert!(graph.reuse.loan_regions.is_empty());
}

#[test]
fn capability_path_scratch_capacity_one_collision_tradeoff_is_exact_and_precharged() {
    for degree in 1usize..=17 {
        let body = graph_body(&[vec![1; degree], vec![]]);
        let plan = plan(&body);
        // Independent geometric sums: growth pays new capacity plus moved
        // live IDs. Duplicate edges count separately, never as one predecessor.
        let capacity = degree.next_power_of_two();
        let growth = 3 * capacity - 2;
        let previous_growth = 3 * capacity.max(4) - 8;
        assert_eq!(
            growth as isize - previous_growth as isize,
            match degree {
                1 => -3,
                2 => 0,
                _ => 6,
            },
        );
        let header = std::mem::size_of::<CapabilityPathRegionScratchV1<'_>>()
            .div_ceil(std::mem::size_of::<usize>());
        let row_words = std::mem::size_of::<Vec<u32>>().div_ceil(std::mem::size_of::<usize>());
        let traversal = 4 + 2 * degree;
        let expected = 2 * (row_words + 5) + header + 8 + 3 + traversal + growth;
        for available in 0..=expected {
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            graph.remaining = available;
            let result = graph.path_region(0, 1);
            if available < expected {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual: 100_001,
                        limit: 100_000,
                    })
                ));
                assert!(graph.remaining <= available);
                assert!(graph.reuse.path_region_scratch.is_none());
            } else {
                assert_eq!(result.unwrap(), [true, true]);
                assert_eq!(graph.remaining, 0);
                let scratch = graph.reuse.path_region_scratch.as_ref().unwrap();
                assert_eq!(scratch.predecessors[1].capacity(), capacity);
                assert_eq!(scratch.predecessors[1], vec![0; degree]);
                graph.remaining = 2 + 5 + 2 + degree;
                assert_eq!(graph.path_region(0, 1).unwrap(), [true, true]);
                assert_eq!(graph.remaining, 0);
            }
            assert!(graph.reuse.loans.is_empty());
            assert!(graph.reuse.loan_regions.is_empty());
        }
    }
}
