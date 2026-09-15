fn path_scratch_allocation_work(blocks: usize) -> usize {
    let word = std::mem::size_of::<usize>();
    let rows = std::mem::size_of::<Vec<u32>>().div_ceil(word);
    // Expanded header and retained payloads, plus both bindings and key setup.
    std::mem::size_of::<CapabilityPathRegionScratchV1<'_>>().div_ceil(word)
        + blocks * (rows + 4)
        + 3
}

// Independent fixture oracle: reachability via repeated set expansion, then
// counts of vertices and edges. It does not read the production scratch.
fn path_scratch_fixture_work(
    edges: &[Vec<u32>],
    from: usize,
    to: usize,
    capacities: &mut [usize],
    completed_from: &mut Option<usize>,
) -> (usize, Vec<bool>) {
    let count = edges.len();
    let closure = |root: usize| {
        let mut reached = vec![false; count];
        reached[root] = true;
        loop {
            let old = reached.clone();
            for source in 0..count {
                if old[source] {
                    for &target in &edges[source] {
                        reached[target as usize] = true;
                    }
                }
            }
            if old == reached {
                break;
            }
        }
        reached
    };
    let live = closure(0);
    if !live[from] {
        return (count, vec![false; count]);
    }
    let cold = completed_from.is_none();
    let hit = *completed_from == Some(from);
    let forward = closure(from);
    let region: Vec<_> = (0..count)
        .map(|block| forward[block] && closure(block)[to])
        .collect();
    let mut incoming = vec![0; count];
    let mut traversal = 0;
    for source in 0..count {
        if forward[source] {
            if !hit {
                traversal += 1 + edges[source].len();
            }
            for &target in &edges[source] {
                incoming[target as usize] += 1;
            }
        }
        if region[source] {
            traversal += 1;
        }
    }
    for target in 0..count {
        if region[target] {
            traversal += incoming[target];
        }
    }
    let mut growth = 0;
    if !hit {
        for target in 0..count {
            while capacities[target] < incoming[target] {
                let old = capacities[target];
                capacities[target] = (2 * old).max(1);
                growth += capacities[target] + old;
            }
        }
        *completed_from = Some(from);
    }
    // Every live query pays two Box operations and three binding/key checks.
    // A rebuild also pays key invalidation, epoch advance, and key completion.
    (
        count
            + 5
            + 3 * usize::from(!hit)
            + usize::from(cold) * path_scratch_allocation_work(count)
            + traversal
            + growth,
        region,
    )
}

#[test]
fn capability_path_scratch_matches_old_regions_on_seeded_queries() {
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
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut capacities = vec![0; edges.len()];
        let mut completed_from = None;
        for from in (0..8).rev().chain(0..8) {
            for to in 0..8 {
                let previous_from = completed_from;
                let (cost, expected) = path_scratch_fixture_work(
                    &edges,
                    from,
                    to,
                    &mut capacities,
                    &mut completed_from,
                );
                hits += usize::from(previous_from == Some(from));
                rebuilds += usize::from(previous_from != completed_from);
                let before = graph.remaining;
                let actual = graph.path_region(from as u32, to as u32).unwrap();
                assert_eq!(before - graph.remaining, cost, "{edges:?}: {from}->{to}");
                assert_eq!(actual, expected);
                assert_eq!(
                    actual,
                    reference
                        .path_region_before_scratch(from as u32, to as u32)
                        .unwrap()
                );
                assert_eq!(
                    graph.reuse.path_region_scratch.is_none(),
                    completed_from.is_none()
                );
                if let Some(scratch) = &graph.reuse.path_region_scratch {
                    assert_eq!(
                        scratch.completed_from,
                        completed_from.map(|from| from as u32)
                    );
                    assert_eq!(scratch.seen.capacity(), edges.len());
                    assert_eq!(scratch.pending.capacity(), edges.len());
                    assert_eq!(scratch.predecessors.capacity(), edges.len());
                    for (row, &capacity) in scratch.predecessors.iter().zip(&capacities) {
                        assert_eq!(row.capacity(), capacity);
                    }
                }
            }
        }
        assert!(reference.reuse.path_region_scratch.is_none());
        assert!(graph.reuse.loans.is_empty());
    }
    assert!(hits > 0);
    assert!(rebuilds > 48);
}

#[test]
fn capability_path_scratch_clears_stale_rows_and_keeps_dead_cycles() {
    let edges = [vec![1, 4], vec![2], vec![1, 3], vec![], vec![4], vec![3]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    for (from, to) in [
        (0, 3),
        (3, 0),
        (4, 4),
        (1, 3),
        (5, 3),
        (0, 4),
        (3, 3),
        (0, 3),
    ] {
        let actual = graph.path_region(from, to).unwrap();
        assert_eq!(
            actual,
            reference.path_region_before_scratch(from, to).unwrap()
        );
        assert_eq!(
            graph.region_is_acyclic_reusing_scratch(&actual).unwrap(),
            reference.region_is_acyclic_cold_reference(&actual).unwrap(),
        );
    }
    assert_eq!(
        graph.path_region(0, 3).unwrap(),
        [true, true, true, true, false, false]
    );
    assert_eq!(graph.path_region(5, 3).unwrap(), [false; 6]);
    assert!(graph.reuse.loan_regions.is_empty());
}

#[test]
fn capability_path_scratch_cold_and_warm_exact_prefixes_fail_closed() {
    for edges in [
        vec![vec![1, 1], vec![2], vec![]],
        vec![vec![1, 2], vec![3], vec![3], vec![]],
        vec![vec![1; 17], vec![1, 2], vec![]],
    ] {
        let body = graph_body(&edges);
        let plan = plan(&body);
        let last = edges.len() as u32 - 1;
        for warm in [false, true] {
            let mut capacities = vec![0; edges.len()];
            let mut completed_from = None;
            if warm {
                path_scratch_fixture_work(
                    &edges,
                    0,
                    last as usize,
                    &mut capacities,
                    &mut completed_from,
                );
            }
            let (expected, region) = path_scratch_fixture_work(
                &edges,
                0,
                last as usize,
                &mut capacities,
                &mut completed_from,
            );
            let prepare = || {
                let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
                if warm {
                    graph.path_region(0, last).unwrap();
                }
                graph
            };
            for available in 0..expected {
                let mut graph = prepare();
                graph.remaining = available;
                assert!(matches!(
                    graph.path_region(0, last),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual: 100_001,
                        limit: 100_000,
                    })
                ));
                assert!(graph.remaining <= available);
                // Failure before checkout leaves the old scratch alone;
                // every later failure discards the checked-out working data.
                assert_eq!(
                    graph.reuse.path_region_scratch.is_some(),
                    warm && available < edges.len() + 2
                );
                assert!(graph.reuse.loans.is_empty());
                assert!(graph.reuse.loan_regions.is_empty());
            }
            let mut exact = prepare();
            exact.remaining = expected;
            assert_eq!(exact.path_region(0, last).unwrap(), region);
            assert_eq!(exact.remaining, 0);
        }
    }
}

#[test]
fn capability_path_scratch_growth_after_warmup_is_precharged() {
    let edges = [vec![1, 2], vec![3; 9], vec![3], vec![]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut capacities = vec![0; edges.len()];
    let mut completed_from = None;
    path_scratch_fixture_work(&edges, 2, 3, &mut capacities, &mut completed_from);
    assert_eq!(capacities, [0, 0, 0, 1]);
    let (expected, region) =
        path_scratch_fixture_work(&edges, 0, 3, &mut capacities, &mut completed_from);
    assert_eq!(capacities, [0, 1, 1, 16]);
    for available in 0..=expected {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        graph.path_region(2, 3).unwrap();
        graph.remaining = available;
        let result = graph.path_region(0, 3);
        if available < expected {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
            assert!(graph.reuse.loans.is_empty());
        } else {
            assert_eq!(result.unwrap(), region);
            assert_eq!(graph.remaining, 0);
        }
    }
}

#[test]
fn capability_path_scratch_epoch_wrap_is_precharged_and_reinitializes_seen() {
    let edges = [vec![1], vec![2], vec![]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut capacities = vec![0; edges.len()];
    let mut completed_from = None;
    path_scratch_fixture_work(&edges, 0, 2, &mut capacities, &mut completed_from);
    let (warm, expected) =
        path_scratch_fixture_work(&edges, 1, 2, &mut capacities, &mut completed_from);
    let required = warm + edges.len();
    for available in 0..=required {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        graph.path_region(0, 2).unwrap();
        let scratch = graph.reuse.path_region_scratch.as_mut().unwrap();
        scratch.epoch = usize::MAX;
        scratch.seen.fill(1);
        graph.remaining = available;
        let result = graph.path_region(1, 2);
        if available < required {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
        } else {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(graph.remaining, 0);
            assert_eq!(graph.reuse.path_region_scratch.as_ref().unwrap().epoch, 1);
        }
        assert!(graph.reuse.loans.is_empty());
    }
}

#[test]
fn capability_path_scratch_is_graph_local_and_preserves_invalid_endpoints() {
    let first = graph_body(&[vec![1], vec![], vec![]]);
    let second = graph_body(&[vec![2], vec![], vec![]]);
    assert_eq!(first.identity(), second.identity());
    let pa = plan(&first);
    let pb = plan(&second);
    let mut a = CapabilitySsaGraphV1::new(&first, pa.plan(), 100_000).unwrap();
    let mut b = CapabilitySsaGraphV1::new(&second, pb.plan(), 100_000).unwrap();
    assert_eq!(a.path_region(0, 1).unwrap(), [true, true, false]);
    assert!(b.reuse.path_region_scratch.is_none());
    assert_eq!(b.path_region(0, 1).unwrap(), [false; 3]);
    assert!(!std::ptr::eq(
        a.reuse.path_region_scratch.as_deref().unwrap(),
        b.reuse.path_region_scratch.as_deref().unwrap(),
    ));
    for (from, to) in [(3, 0), (0, 3), (u32::MAX, 0), (0, u32::MAX)] {
        let before = a.remaining;
        assert!(matches!(
            a.path_region(from, to),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(a.remaining, before);
    }
    let mut unreachable = CapabilitySsaGraphV1::new(&first, pa.plan(), 100_000).unwrap();
    unreachable.remaining = 3;
    assert_eq!(unreachable.path_region(2, 1).unwrap(), [false; 3]);
    assert_eq!(unreachable.remaining, 0);
    assert!(unreachable.reuse.path_region_scratch.is_none());
}

#[test]
fn capability_path_scratch_failed_edge_validation_discards_working_rows() {
    let scaffold = graph_body(&[vec![1], vec![], vec![]]);
    let plan = plan(&scaffold);
    let body = graph_body(&[vec![1], vec![99], vec![]]);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    reuse_assert_same(
        graph.path_region(0, 1),
        reference.path_region_before_scratch(0, 1),
    );
    assert!(graph.reuse.path_region_scratch.is_none());
    assert!(graph.reuse.loans.is_empty());
    assert!(graph.reuse.loan_regions.is_empty());
}

#[test]
fn capability_path_scratch_full_loans_match_exact_graph89_on_seeded_cfgs() {
    let mut seed = 0x9917_ac48_u32;
    for _ in 0..48 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 4).map(|_| next() % 8).collect())
            .collect();
        let original = reuse_body(&edges);
        let blocks = original
            .blocks()
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let mut statements = b.statements().to_vec();
                if i != 0 && next() % 3 == 0 {
                    statements.push(statement(SemanticStatementKindV1::StorageDead(
                        SemanticLocalIdV1::from_index(1),
                    )));
                }
                SemanticBasicBlockV1::new(
                    b.identity(),
                    b.source(),
                    statements,
                    b.terminator().clone(),
                )
                .unwrap()
            })
            .collect();
        let body = body(blocks);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let owner = graph.use_value(0, 1).unwrap();
        for target in 0..8 {
            for end in [Some(0), Some(1), Some(2), None] {
                for _ in 0..2 {
                    let consumer = reuse_consumer(target, end);
                    reuse_assert_same(
                        graph.loan_live(loan(owner, 1), consumer),
                        reference.loan_live_before_path_scratch(loan(owner, 1), consumer),
                    );
                }
            }
        }
        assert!(reference.reuse.path_region_scratch.is_none());
    }
}

#[test]
fn capability_path_scratch_aggregate_endpoints_and_debits_keep_graph89_checks() {
    for operands in [0usize, 1, 8, 257] {
        for moved in [None, operands.checked_sub(1)] {
            let body = aggregate_budget_body(operands, moved, 3);
            let plan = plan(&body);
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
            let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
            let owner = graph.use_value(0, 1).unwrap();
            for start in [None, Some(0), Some(1), Some(2), Some(4), Some(u32::MAX)] {
                for end in [
                    None,
                    Some(0),
                    Some(1),
                    Some(2),
                    Some(3),
                    Some(4),
                    Some(u32::MAX),
                ] {
                    let mut borrowed = loan(owner, 1);
                    borrowed.borrow.statement = start;
                    let consumer = reuse_consumer(0, end);
                    reuse_assert_same(
                        graph.loan_live(borrowed, consumer),
                        reference.loan_live_before_path_scratch(borrowed, consumer),
                    );
                }
            }
            assert!(reference.reuse.path_region_scratch.is_none());
        }
        let body = aggregate_budget_body(operands, None, 3);
        let plan = plan(&body);
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let owner = reference.use_value(0, 1).unwrap();
        let borrowed = loan(owner, 1);
        let consumer = reuse_consumer(0, Some(3));
        let before = reference.remaining;
        reference
            .loan_live_before_path_scratch(borrowed, consumer)
            .unwrap();
        // One block, no edges: original path cost is 4 + 1 + 1.
        // Cold acyclicity prepays two Box and two clear operations;
        // endpoint geometry also prepays one source/index dispatch.
        let expected =
            before - reference.remaining - 6 + 1 + 8 + path_scratch_allocation_work(1) + 2 + 4 + 1;
        for available in 0..=expected {
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            assert_eq!(graph.use_value(0, 1).unwrap(), owner);
            graph.remaining = available;
            let result = graph.loan_live(borrowed, consumer);
            if available < expected {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
                ));
                assert!(graph.reuse.loans.is_empty());
            } else {
                result.unwrap();
                assert_eq!(graph.remaining, 0);
                assert_eq!(graph.reuse.loans.len(), 1);
            }
        }
    }
}

#[test]
fn capability_path_scratch_terminal_checks_keep_exact_graph89_order() {
    let scaffold = reuse_body(&[vec![1], vec![2], vec![]]);
    let plan = plan(&scaffold);
    let edge = |role| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(2));
    let call = |operand, destination| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![operand],
                Some(SemanticCallDestinationV1::new(
                    place(destination),
                    edge(SemanticEdgeRoleV1::CallReturn),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    for (terminator, invalid) in [
        (call(SemanticOperandV1::Move(place(1)), 3), true),
        (call(SemanticOperandV1::Copy(place(1)), 1), true),
        (call(SemanticOperandV1::Copy(place(1)), 3), false),
        (
            SemanticTerminatorKindV1::Drop {
                place: place(1),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: edge(SemanticEdgeRoleV1::DropReturn),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            true,
        ),
    ] {
        let mut blocks = scaffold.blocks().to_vec();
        blocks[1] = SemanticBasicBlockV1::new(
            blocks[1].identity(),
            blocks[1].source(),
            blocks[1].statements().to_vec(),
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap();
        let body = body(blocks);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let owner = graph.use_value(0, 1).unwrap();
        for consumer in [
            reuse_consumer(1, Some(0)),
            reuse_consumer(1, None),
            reuse_consumer(2, None),
        ] {
            let actual = graph.loan_live(loan(owner, 1), consumer);
            assert_eq!(
                actual.is_err(),
                invalid && consumer != reuse_consumer(1, Some(0))
            );
            reuse_assert_same(
                actual,
                reference.loan_live_before_path_scratch(loan(owner, 1), consumer),
            );
        }
    }
}

#[test]
fn capability_path_scratch_saves_exact_repeated_work_but_can_reject_cold_earlier() {
    let count = 618;
    let edges: Vec<Vec<u32>> = (0..count)
        .map(|b| {
            if b + 1 < count {
                vec![b as u32 + 1]
            } else {
                vec![]
            }
        })
        .collect();
    let body = graph_body(&edges);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    graph.path_region(0, count as u32 - 1).unwrap();
    let mut completed_from = Some(0);
    for from in 0..24 {
        let to = count - 1 - from;
        // Independent closed form for this chain: F/F-1 forward vertices/edges,
        // R/R-1 backward vertices/edges. Warmup already sized every incoming row.
        let forward = count - from;
        let included = to - from + 1;
        let hit = completed_from == Some(from);
        let expected = count + 5 + (2 * included - 1) + usize::from(!hit) * (3 + 2 * forward - 1);
        let region: Vec<_> = (0..count)
            .map(|block| from <= block && block <= to)
            .collect();
        let before = graph.remaining;
        let old_before = reference.remaining;
        assert_eq!(graph.path_region(from as u32, to as u32).unwrap(), region);
        assert_eq!(
            reference
                .path_region_before_scratch(from as u32, to as u32)
                .unwrap(),
            region
        );
        let new_work = before - graph.remaining;
        let old_work = old_before - reference.remaining;
        assert_eq!(new_work, expected);
        assert_eq!(
            old_work - new_work,
            if hit {
                3 * count + (2 * forward - 1) - 5
            } else {
                3 * count - 8
            }
        );
        completed_from = Some(from);
    }
    let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    let before = old.remaining;
    old.path_region_before_scratch(0, 617).unwrap();
    let old_cost = before - old.remaining;
    let mut cold = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    cold.remaining = old_cost;
    assert!(matches!(
        cold.path_region(0, 617),
        Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
    ));
    assert!(cold.reuse.loans.is_empty());
    assert!(cold.reuse.path_region_scratch.is_none());
}

include!("predecessor_capacity_one_tests.rs");
