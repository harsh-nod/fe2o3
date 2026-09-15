fn acyclic_scratch_header() -> usize {
    2 * std::mem::size_of::<Vec<usize>>() / std::mem::size_of::<usize>()
}

// Independent fixed-point elimination, not the production degree/queue walk.
// Count ALL outgoing edges, including duplicates and edges outside the mask.
fn acyclic_scratch_fixture_scan(edges: &[Vec<u32>], region: &[bool]) -> (bool, usize) {
    let mut removed = vec![false; edges.len()];
    loop {
        let previous = removed.clone();
        for target in 0..edges.len() {
            if region[target]
                && edges.iter().enumerate().all(|(source, successors)| {
                    !region[source] || previous[source] || !successors.contains(&(target as u32))
                })
            {
                removed[target] = true;
            }
        }
        if removed == previous {
            break;
        }
    }
    let included = region.iter().filter(|&&inside| inside).count();
    let processed = removed.iter().filter(|&&done| done).count();
    let outgoing = |mask: &[bool]| {
        edges
            .iter()
            .enumerate()
            .filter(|(i, _)| mask[*i])
            .map(|(_, successors)| successors.len())
            .sum::<usize>()
    };
    (
        processed == included,
        2 * edges.len() + outgoing(region) + processed + outgoing(&removed),
    )
}

fn acyclic_scratch_fixture_cost(edges: &[Vec<u32>], region: &[bool], cold: bool) -> usize {
    let (_, scan) = acyclic_scratch_fixture_scan(edges, region);
    4 + edges.len() + scan + usize::from(cold) * (acyclic_scratch_header() + 2 * edges.len())
}

fn acyclic_scratch_assert_no_authority(graph: &CapabilitySsaGraphV1<'_>) {
    assert!(graph.reuse.uses.is_empty());
    assert!(graph.reuse.definitions.is_empty());
    assert!(graph.reuse.reachability.is_empty());
    assert!(graph.reuse.loans.is_empty());
    assert!(graph.reuse.loan_regions.is_empty());
    assert!(graph.reuse.owner_invalidations.is_empty());
}

#[test]
fn capability_acyclic_scratch_all_seeded_masks_match_frozen_cold_and_work_oracle() {
    let mut seed = 0x103a_6c91_u32;
    for _ in 0..48 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..6)
            .map(|_| (0..next() % 5).map(|_| next() % 6).collect())
            .collect();
        let body = graph_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        for (call, bits) in (0..64).chain((0..64).rev()).enumerate() {
            let region: Vec<bool> = (0..6).map(|i| bits & (1 << i) != 0).collect();
            let (expected, scan) = acyclic_scratch_fixture_scan(&edges, &region);
            let before = graph.remaining;
            let old_before = old.remaining;
            assert_eq!(
                graph.region_is_acyclic_reusing_scratch(&region).unwrap(),
                expected
            );
            assert_eq!(
                old.region_is_acyclic_cold_reference(&region).unwrap(),
                expected
            );
            assert_eq!(
                before - graph.remaining,
                acyclic_scratch_fixture_cost(&edges, &region, call == 0)
            );
            assert_eq!(
                old_before - old.remaining,
                acyclic_scratch_header() + 3 * edges.len() + scan
            );
            let scratch = graph.reuse.region_acyclic_scratch.as_ref().unwrap();
            assert_eq!(scratch.degree.len(), edges.len());
            assert_eq!(scratch.degree.capacity(), edges.len());
            assert_eq!(scratch.ready.capacity(), edges.len());
            assert!(scratch.ready.len() <= edges.len());
            assert!(old.reuse.region_acyclic_scratch.is_none());
            acyclic_scratch_assert_no_authority(&graph);
        }
    }
}

#[test]
fn capability_acyclic_scratch_resets_after_false_partial_walk_empty_and_poisoned_storage() {
    let edges = [vec![1, 1], vec![2], vec![1], vec![4], vec![]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    assert!(!graph.region_is_acyclic_reusing_scratch(&[true; 5]).unwrap());
    let scratch = graph.reuse.region_acyclic_scratch.as_ref().unwrap();
    assert_eq!(scratch.ready, [0, 3, 4]);
    assert_eq!(scratch.degree, [0, 1, 1, 0, 0]);
    let pointers = (scratch.degree.as_ptr(), scratch.ready.as_ptr());
    for region in [
        [false; 5],
        [true, true, false, false, false],
        [false, true, true, false, false],
        [false, false, true, true, true],
        [true; 5],
    ] {
        // Dirty every degree and the queue, including after a false result.
        let scratch = graph.reuse.region_acyclic_scratch.as_mut().unwrap();
        scratch.degree.fill(usize::MAX);
        scratch.ready.clear();
        scratch.ready.extend([usize::MAX; 5]);
        graph.remaining = acyclic_scratch_fixture_cost(&edges, &region, false);
        assert_eq!(
            graph.region_is_acyclic_reusing_scratch(&region).unwrap(),
            acyclic_scratch_fixture_scan(&edges, &region).0
        );
        assert_eq!(graph.remaining, 0);
        let scratch = graph.reuse.region_acyclic_scratch.as_ref().unwrap();
        assert_eq!((scratch.degree.as_ptr(), scratch.ready.as_ptr()), pointers);
        acyclic_scratch_assert_no_authority(&graph);
    }
}

#[test]
fn capability_acyclic_scratch_cold_warm_duplicate_edge_prefixes_fail_closed_and_retry() {
    for duplicates in [1, 2, 8, 257] {
        for cyclic in [false, true] {
            let edges = [
                vec![1; duplicates],
                if cyclic { vec![1, 2] } else { vec![2] },
                vec![],
            ];
            let body = graph_body(&edges);
            let plan = plan(&body);
            let region = [true; 3];
            for warm in [false, true] {
                let cost = acyclic_scratch_fixture_cost(&edges, &region, !warm);
                for available in 0..=cost {
                    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
                    if warm {
                        assert!(
                            graph
                                .region_is_acyclic_reusing_scratch(&[false; 3])
                                .unwrap()
                        );
                    }
                    graph.remaining = available;
                    let result = graph.region_is_acyclic_reusing_scratch(&region);
                    if available < cost {
                        assert!(matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                                actual: 100_001,
                                limit: 100_000,
                            })
                        ));
                        assert!(graph.remaining <= available);
                        assert_eq!(
                            graph.reuse.region_acyclic_scratch.is_some(),
                            warm && available < 2
                        );
                        let cold = graph.reuse.region_acyclic_scratch.is_none();
                        graph.remaining = acyclic_scratch_fixture_cost(&edges, &region, cold);
                        assert_eq!(
                            graph.region_is_acyclic_reusing_scratch(&region).unwrap(),
                            !cyclic
                        );
                        assert_eq!(graph.remaining, 0);
                    } else {
                        assert_eq!(result.unwrap(), !cyclic);
                        assert_eq!(graph.remaining, 0);
                    }
                    acyclic_scratch_assert_no_authority(&graph);
                }
            }
        }
    }
}

#[test]
fn capability_acyclic_scratch_length_and_target_errors_match_cold_without_publication() {
    let scaffold = graph_body(&[vec![1], vec![], vec![]]);
    let plan = plan(&scaffold);
    let body = graph_body(&[vec![1], vec![99], vec![]]);
    for warm in [false, true] {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        if warm {
            assert!(
                graph
                    .region_is_acyclic_reusing_scratch(&[false; 3])
                    .unwrap()
            );
        }
        for region in [vec![], vec![true; 2], vec![true; 4]] {
            let before = graph.remaining;
            assert!(matches!(
                graph.region_is_acyclic_reusing_scratch(&region),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert!(matches!(
                old.region_is_acyclic_cold_reference(&region),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(graph.remaining, before);
            assert_eq!(graph.reuse.region_acyclic_scratch.is_some(), warm);
        }
        assert!(matches!(
            graph.region_is_acyclic_reusing_scratch(&[true; 3]),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(matches!(
            old.region_is_acyclic_cold_reference(&[true; 3]),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(graph.reuse.region_acyclic_scratch.is_none());
        acyclic_scratch_assert_no_authority(&graph);
        // An excluded bad source is not scanned, exactly as in the old helper.
        assert!(
            graph
                .region_is_acyclic_reusing_scratch(&[true, false, false])
                .unwrap()
        );
        assert!(
            old.region_is_acyclic_cold_reference(&[true, false, false])
                .unwrap()
        );
    }
}

#[test]
fn capability_acyclic_scratch_is_graph_local_with_equal_body_identifiers() {
    let first = graph_body(&[vec![1], vec![2], vec![]]);
    let second = graph_body(&[vec![1], vec![1, 2], vec![]]);
    assert_eq!(first.identity(), second.identity());
    let first_plan = plan(&first);
    let second_plan = plan(&second);
    let mut a = CapabilitySsaGraphV1::new(&first, first_plan.plan(), 100_000).unwrap();
    let mut b = CapabilitySsaGraphV1::new(&second, second_plan.plan(), 100_000).unwrap();
    assert!(a.region_is_acyclic_reusing_scratch(&[true; 3]).unwrap());
    assert!(b.reuse.region_acyclic_scratch.is_none());
    assert!(!b.region_is_acyclic_reusing_scratch(&[true; 3]).unwrap());
    assert!(!std::ptr::eq(
        a.reuse.region_acyclic_scratch.as_deref().unwrap(),
        b.reuse.region_acyclic_scratch.as_deref().unwrap()
    ));
    assert!(a.region_is_acyclic_reusing_scratch(&[true; 3]).unwrap());
    acyclic_scratch_assert_no_authority(&a);
    acyclic_scratch_assert_no_authority(&b);
}

#[test]
fn capability_acyclic_scratch_repeated_618_scans_keep_walks_and_exact_storage_savings() {
    let count = 618;
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
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1_048_576).unwrap();
    let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 1_048_576).unwrap();
    for call in 0..24 {
        let end = count - 1 - call;
        let region: Vec<bool> = (0..count)
            .map(|block| call <= block && block <= end)
            .collect();
        let included = end - call + 1;
        // All outgoing edges are visited twice, even end -> end+1 outside R.
        let outgoing = included - usize::from(end == count - 1);
        let scan = 2 * count + included + 2 * outgoing;
        let old_cost = acyclic_scratch_header() + 3 * count + scan;
        let new_cost =
            4 + count + scan + usize::from(call == 0) * (acyclic_scratch_header() + 2 * count);
        let before = graph.remaining;
        let old_before = old.remaining;
        assert!(graph.region_is_acyclic_reusing_scratch(&region).unwrap());
        assert!(old.region_is_acyclic_cold_reference(&region).unwrap());
        assert_eq!(before - graph.remaining, new_cost);
        assert_eq!(old_before - old.remaining, old_cost);
        if call == 0 {
            assert_eq!(new_cost - old_cost, 4);
        } else {
            assert_eq!(
                old_cost - new_cost,
                acyclic_scratch_header() + 2 * count - 4
            );
            #[cfg(target_pointer_width = "64")]
            assert_eq!(old_cost - new_cost, 1_238);
        }
    }
    acyclic_scratch_assert_no_authority(&graph);
}

#[test]
fn capability_acyclic_scratch_cold_can_reject_earlier_without_changing_old_contract() {
    let edges = [vec![1, 1], vec![2], vec![]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    let region = [true; 3];
    let (_, scan) = acyclic_scratch_fixture_scan(&edges, &region);
    let old_cost = acyclic_scratch_header() + 3 * edges.len() + scan;
    let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    for _ in 0..2 {
        old.remaining = old_cost;
        assert!(old.region_is_acyclic_cold_reference(&region).unwrap());
        assert_eq!(old.remaining, 0);
        assert!(old.reuse.region_acyclic_scratch.is_none());
    }
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    graph.remaining = old_cost;
    assert!(matches!(
        graph.region_is_acyclic_reusing_scratch(&region),
        Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
    ));
    assert!(graph.reuse.region_acyclic_scratch.is_none());
    acyclic_scratch_assert_no_authority(&graph);
}

#[test]
fn capability_acyclic_scratch_geometry_publication_failure_keeps_storage_not_authority() {
    let edges = [vec![1, 1], vec![2], vec![]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut capacities = vec![0; 3];
    let mut completed_from = None;
    let (path_cost, region) =
        path_scratch_fixture_work(&edges, 0, 2, &mut capacities, &mut completed_from);
    let allocation =
        std::mem::size_of::<CapabilityLoanRegionV1>().div_ceil(std::mem::size_of::<usize>()) + 2;
    let publication = insertion_work::<(u32, u32), Arc<CapabilityLoanRegionV1>>(0) + allocation;
    let cost = lookup_work(0)
        + 2
        + path_cost
        + acyclic_scratch_fixture_cost(&edges, &region, true)
        + publication;
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    graph.remaining = cost - 1;
    assert!(matches!(
        graph.loan_region(0, 2),
        Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
    ));
    assert_eq!(graph.remaining, publication - 1);
    assert!(graph.reuse.region_acyclic_scratch.is_some());
    assert_eq!(
        graph
            .reuse
            .path_region_scratch
            .as_ref()
            .unwrap()
            .completed_from,
        Some(0)
    );
    acyclic_scratch_assert_no_authority(&graph);
    let (warm_path, _) =
        path_scratch_fixture_work(&edges, 0, 2, &mut capacities, &mut completed_from);
    graph.remaining = lookup_work(0)
        + 2
        + warm_path
        + acyclic_scratch_fixture_cost(&edges, &region, false)
        + publication;
    let actual = graph.loan_region(0, 2).unwrap();
    assert_eq!(actual.blocks, region);
    assert!(actual.acyclic);
    assert_eq!(graph.remaining, 0);
    assert_eq!(graph.reuse.loan_regions.len(), 1);
    assert!(graph.reuse.loans.is_empty());
}

#[test]
fn capability_acyclic_scratch_full_aggregate_loans_have_independent_cold_warm_prefixes() {
    for operands in [0, 1, 8, 257] {
        for warm in [false, true] {
            let body = aggregate_budget_body(operands, None, 3);
            let plan = plan(&body);
            let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let owner = old.use_value(0, 1).unwrap();
            let borrowed = loan(owner, 1);
            let consumer = reuse_consumer(0, Some(3));
            let before = old.remaining;
            old.loan_live_before_path_scratch(borrowed, consumer)
                .unwrap();
            // Frozen loan/geometry/cold checker. Only cold path work and the
            // independently calculated acyclicity cold/warm delta and one
            // endpoint source/index dispatch differ.
            let path_delta = 1 + 8 + path_scratch_allocation_work(1) + 2 - 6;
            let expected = before - old.remaining + path_delta + 4 + 1
                - usize::from(warm) * (acyclic_scratch_header() + 2);
            for available in 0..=expected {
                let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
                assert_eq!(graph.use_value(0, 1).unwrap(), owner);
                if warm {
                    assert!(graph.region_is_acyclic_reusing_scratch(&[false]).unwrap());
                }
                graph.remaining = available;
                let result = graph.loan_live(borrowed, consumer);
                if available < expected {
                    assert!(matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
                    ));
                    assert!(graph.remaining <= available);
                    assert!(graph.reuse.loans.is_empty());
                } else {
                    result.unwrap();
                    assert_eq!(graph.remaining, 0);
                    assert_eq!(graph.reuse.loans.len(), 1);
                }
            }
            assert!(old.reuse.region_acyclic_scratch.is_none());
        }
    }
}
