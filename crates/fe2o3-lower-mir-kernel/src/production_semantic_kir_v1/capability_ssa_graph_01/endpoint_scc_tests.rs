fn endpoint_owner(edges: &[Vec<u32>]) -> fe2o3_pliron::ProductionSemanticSsaOwnerV1 {
    let owner = indexed_owner(graph_body(edges).blocks().to_vec());
    owner.verify_replay().unwrap();
    owner
}

fn endpoint_query(
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
) -> fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'_> {
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    owner.source_query_for_root(root, view.body()).unwrap()
}

fn endpoint_graph<'a>(
    source: &fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'a>,
) -> CapabilitySsaGraphV1<'a> {
    CapabilitySsaGraphV1::new(source.function(), source.plan().plan(), 100_000)
        .unwrap()
        .with_definition_source(source)
        .unwrap()
}

fn endpoint_assert_no_loan(graph: &CapabilitySsaGraphV1<'_>) {
    assert!(graph.reuse.loans.is_empty());
    assert!(graph.reuse.uses.is_empty());
    assert!(graph.reuse.definitions.is_empty());
    assert!(graph.reuse.reachability.is_empty());
    assert!(graph.reuse.owner_invalidations.is_empty());
}

fn endpoint_recursive_rpo(edges: &[Vec<u32>]) -> Vec<usize> {
    fn visit(node: usize, edges: &[Vec<u32>], seen: &mut [bool], out: &mut Vec<usize>) {
        if seen[node] {
            return;
        }
        seen[node] = true;
        for &next in &edges[node] {
            visit(next as usize, edges, seen, out);
        }
        out.push(node);
    }
    let mut out = Vec::new();
    visit(0, edges, &mut vec![false; edges.len()], &mut out);
    out.reverse();
    out
}

#[test]
fn capability_endpoint_scc_all_pairs_match_old_checker_and_independent_debits() {
    let mut fixtures = vec![
        vec![vec![1, 1], vec![2], vec![]],
        vec![vec![1, 3], vec![2], vec![], vec![3], vec![1, 4]],
        vec![vec![1], vec![1, 2], vec![]],
        vec![vec![1], vec![2], vec![0]],
        vec![vec![1, 2], vec![2, 3], vec![1, 3], vec![]],
        vec![vec![2, 1], vec![3], vec![3], vec![]],
        vec![vec![], vec![1]],
    ];
    let mut seed = 0x106c_5a91_u32;
    for _ in 0..32 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        fixtures.push(
            (0..6)
                .map(|_| (0..next() % 5).map(|_| next() % 6).collect())
                .collect(),
        );
    }
    for edges in fixtures {
        let owner = endpoint_owner(&edges);
        let source = endpoint_query(&owner);
        assert_eq!(
            source
                .plan()
                .plan()
                .reverse_postorder()
                .iter()
                .map(|b| b.get() as usize)
                .collect::<Vec<_>>(),
            endpoint_recursive_rpo(&edges)
        );
        let mut graph = endpoint_graph(&source);
        let mut old = endpoint_graph(&source);
        let mut oracle = EndpointCostOracle::new(edges.len());
        let mut work = (0, 0);
        for from in (0..edges.len()).chain((0..edges.len()).rev()) {
            for to in (0..edges.len()).rev() {
                let (cost, region, acyclic) = oracle.query(&edges, from, to);
                let before = (graph.remaining, old.remaining);
                let actual = graph.loan_region(from as u32, to as u32).unwrap();
                let expected = old
                    .loan_region_before_endpoint_scc(from as u32, to as u32)
                    .unwrap();
                assert_eq!(actual.blocks, region, "{edges:?}, {from}->{to}");
                assert_eq!(actual.acyclic, acyclic, "{edges:?}, {from}->{to}");
                assert_eq!(actual.blocks, expected.blocks);
                assert_eq!(actual.acyclic, expected.acyclic);
                assert_eq!(before.0 - graph.remaining, cost, "{edges:?}, {from}->{to}");
                assert_eq!(
                    graph
                        .reuse
                        .path_region_scratch
                        .as_ref()
                        .and_then(|scratch| scratch.completed_from),
                    oracle.completed_from.map(|from| from as u32),
                );
                work.0 += cost;
                work.1 += before.1 - old.remaining;
                assert!(graph.reuse.endpoint_scc.is_some());
                assert!(graph.reuse.region_acyclic_scratch.is_none());
                assert!(old.reuse.endpoint_scc.is_none());
                endpoint_assert_no_loan(&graph);
            }
        }
        println!(
            "endpoint SCC complete sequence: new={} old={}",
            work.0, work.1
        );
    }
}

#[test]
fn capability_endpoint_scc_generic_masks_still_use_induced_kahn() {
    let edges = [vec![1], vec![0, 2], vec![3], vec![], vec![4], vec![2]];
    let owner = endpoint_owner(&edges);
    let source = endpoint_query(&owner);
    let mut graph = endpoint_graph(&source);
    let mut old = endpoint_graph(&source);
    assert!(!graph.loan_region(0, 1).unwrap().acyclic);
    let index = &**graph.reuse.endpoint_scc.as_ref().unwrap() as *const _;
    for bits in 0..64 {
        let mask: Vec<_> = (0..6).map(|i| bits & (1 << i) != 0).collect();
        let before = graph.remaining;
        assert_eq!(
            graph.region_is_acyclic_reusing_scratch(&mask).unwrap(),
            old.region_is_acyclic_cold_reference(&mask).unwrap()
        );
        assert_eq!(
            before - graph.remaining,
            acyclic_scratch_fixture_cost(&edges, &mask, bits == 0)
        );
        assert_eq!(
            &**graph.reuse.endpoint_scc.as_ref().unwrap() as *const _,
            index
        );
    }
    // Vertex 0 is cyclic in the complete graph but this mask has no cycle.
    assert!(
        graph
            .region_is_acyclic_reusing_scratch(&[true, false, false, false, false, false])
            .unwrap()
    );
    assert_eq!(graph.reuse.loan_regions.len(), 1);
    endpoint_assert_no_loan(&graph);
}

#[test]
fn capability_endpoint_scc_cold_warm_all_short_prefixes_and_exact_zero() {
    for duplicate in [1, 2, 8, 17] {
        for cyclic in [false, true] {
            let edges = [
                vec![1; duplicate],
                if cyclic { vec![1, 2] } else { vec![2] },
                vec![],
            ];
            let owner = endpoint_owner(&edges);
            let source = endpoint_query(&owner);
            for warm in [false, true] {
                let mut oracle = EndpointCostOracle::new(3);
                if warm {
                    oracle.query(&edges, 0, 1);
                }
                let (cost, expected, acyclic) = oracle.query(&edges, 0, 2);
                for available in 0..=cost {
                    let mut graph = endpoint_graph(&source);
                    if warm {
                        graph.loan_region(0, 1).unwrap();
                    }
                    let old_index = graph.reuse.endpoint_scc.as_ref().map(|x| &**x as *const _);
                    graph.remaining = available;
                    let result = graph.loan_region(0, 2);
                    if available < cost {
                        assert!(
                            matches!(
                                result,
                                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                                    actual: 100_001,
                                    limit: 100_000,
                                })
                            ),
                            "duplicate={duplicate}, cyclic={cyclic}, warm={warm}, prefix={available}/{cost}"
                        );
                        assert!(graph.remaining <= available);
                        assert_eq!(graph.reuse.loan_regions.len(), usize::from(warm));
                        assert!(!graph.reuse.loan_regions.contains_key(&(0, 2)));
                        assert_eq!(
                            graph.reuse.endpoint_scc.as_ref().map(|x| &**x as *const _),
                            old_index
                        );
                    } else {
                        let actual = result.unwrap();
                        assert_eq!(actual.blocks, expected);
                        assert_eq!(actual.acyclic, acyclic);
                        assert_eq!(graph.remaining, 0);
                        assert_eq!(graph.reuse.loan_regions.len(), usize::from(warm) + 1);
                        assert!(graph.reuse.endpoint_scc.is_some());
                    }
                    assert!(graph.reuse.region_acyclic_scratch.is_none());
                    endpoint_assert_no_loan(&graph);
                }
            }
        }
    }
}

#[test]
fn capability_endpoint_scc_cached_geometry_binds_owner_and_prepays_all_access() {
    let edges = [vec![1, 1], vec![2], vec![]];
    let owner = endpoint_owner(&edges);
    let source = endpoint_query(&owner);
    let mut graph = endpoint_graph(&source);
    let first = graph.loan_region(0, 2).unwrap();
    let cost = endpoint_fixture_lookup(1) + 2 + 5;
    for available in 0..cost {
        graph.remaining = available;
        assert!(matches!(
            graph.loan_region(0, 2),
            Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
        ));
        assert!(graph.remaining <= available);
        assert_eq!(graph.reuse.loan_regions.len(), 1);
    }
    graph.remaining = cost;
    assert!(Arc::ptr_eq(&first, &graph.loan_region(0, 2).unwrap()));
    assert_eq!(graph.remaining, 0);
    endpoint_assert_no_loan(&graph);
}

#[test]
fn capability_endpoint_scc_equal_hash_owner_substitution_rejects_before_geometry_hits() {
    let first = endpoint_owner(&[vec![1], vec![0, 2], vec![]]);
    let second = endpoint_owner(&[vec![1], vec![0, 2], vec![]]);
    let a = endpoint_query(&first);
    let b = endpoint_query(&second);
    assert_eq!(a.function().identity(), b.function().identity());
    assert_eq!(a.plan().plan().identity(), b.plan().plan().identity());
    assert!(!std::ptr::eq(a.function(), b.function()));
    assert!(!std::ptr::eq(a.plan().plan(), b.plan().plan()));
    for warm in [false, true] {
        for substitution in 0..5 {
            let mut graph = endpoint_graph(&a);
            if warm {
                graph.loan_region(0, 2).unwrap();
            }
            let retained = graph.reuse.endpoint_scc.as_ref().map(|x| &**x as *const _);
            match substitution {
                0 => graph.body = b.function(),
                1 => graph.ssa = b.plan().plan(),
                2 => graph.reuse.definition_source = Some(b),
                3 => {
                    graph.body = b.function();
                    graph.ssa = b.plan().plan();
                    graph.reuse.definition_source = Some(b);
                }
                _ => graph.reuse.definition_source = None,
            }
            // A wholly substituted, still-cold owner is valid and independent;
            // removal before any index exists selects the old Kahn fallback.
            if !warm && substitution >= 3 {
                graph.loan_region(0, 2).unwrap();
                assert_eq!(graph.reuse.endpoint_scc.is_some(), substitution == 3);
            } else {
                assert!(matches!(
                    graph.loan_region(0, 2),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                assert_eq!(graph.reuse.loan_regions.len(), usize::from(warm));
                assert_eq!(
                    graph.reuse.endpoint_scc.as_ref().map(|x| &**x as *const _),
                    retained
                );
            }
            endpoint_assert_no_loan(&graph);
        }
    }
    let mut separate = endpoint_graph(&b);
    assert!(separate.reuse.endpoint_scc.is_none());
    assert!(!separate.loan_region(0, 2).unwrap().acyclic);
}

#[test]
fn capability_endpoint_scc_dead_start_and_invalid_endpoints_do_not_forge_results() {
    let edges = [vec![1], vec![], vec![2, 1]];
    let owner = endpoint_owner(&edges);
    let source = endpoint_query(&owner);
    let mut graph = endpoint_graph(&source);
    let mut oracle = EndpointCostOracle::new(3);
    let (cost, expected, acyclic) = oracle.query(&edges, 2, 1);
    graph.remaining = cost;
    let result = graph.loan_region(2, 1).unwrap();
    assert_eq!(result.blocks, expected);
    assert_eq!(result.blocks, [false; 3]);
    assert_eq!(result.acyclic, acyclic);
    assert_eq!(graph.remaining, 0);
    assert!(graph.reuse.path_region_scratch.is_none());
    assert!(graph.reuse.endpoint_scc.is_some());
    for (from, to) in [(3, 0), (0, 3), (u32::MAX, 0)] {
        let mut graph = endpoint_graph(&source);
        assert!(matches!(
            graph.loan_region(from, to),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(graph.reuse.endpoint_scc.is_none());
        assert!(graph.reuse.loan_regions.is_empty());
        endpoint_assert_no_loan(&graph);
    }
}

#[test]
fn capability_endpoint_scc_dense_and_parallel_inventory_cost_is_fully_counted() {
    for vertices in [2, 4, 6] {
        for duplicate in [1, 2, 8, 17] {
            let edges: Vec<Vec<u32>> = (0..vertices)
                .map(|_| {
                    (0..vertices)
                        .flat_map(|v| std::iter::repeat_n(v as u32, duplicate))
                        .collect()
                })
                .collect();
            let owner = endpoint_owner(&edges);
            let source = endpoint_query(&owner);
            let mut graph = endpoint_graph(&source);
            let mut oracle = EndpointCostOracle::new(vertices);
            let (cost, expected, acyclic) = oracle.query(&edges, 0, vertices - 1);
            graph.remaining = cost;
            let actual = graph.loan_region(0, vertices as u32 - 1).unwrap();
            assert_eq!(actual.blocks, expected);
            assert_eq!(actual.acyclic, acyclic);
            assert!(!acyclic);
            assert_eq!(graph.remaining, 0);
            let (warm, expected, acyclic) = oracle.query(&edges, 1, 0);
            graph.remaining = warm;
            let actual = graph.loan_region(1, 0).unwrap();
            assert_eq!(actual.blocks, expected);
            assert_eq!(actual.acyclic, acyclic);
            assert_eq!(graph.remaining, 0);
            endpoint_assert_no_loan(&graph);
        }
    }
}

#[test]
fn capability_endpoint_scc_deep_chain_borrows_order_without_recursive_or_edge_index_storage() {
    let n = 1536usize;
    let edges: Vec<Vec<u32>> = (0..n)
        .map(|i| {
            if i + 1 < n {
                vec![i as u32 + 1]
            } else {
                vec![]
            }
        })
        .collect();
    // The small graph helper uses little-endian IDs, which are not sorted
    // beyond block 255. Full semantic admission requires canonical ordering.
    let original = graph_body(&edges);
    let blocks: Vec<_> = original
        .blocks()
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let mut identity = *block.identity().as_bytes();
            identity[..4].copy_from_slice(&u32::try_from(index).unwrap().to_be_bytes());
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(identity),
                block.source(),
                block.statements().to_vec(),
                block.terminator().clone(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(blocks.len(), n);
    assert!(
        blocks
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    for (before, after) in original.blocks().iter().zip(&blocks) {
        assert_eq!(before.source(), after.source());
        assert_eq!(before.statements(), after.statements());
        assert_eq!(before.terminator(), after.terminator());
    }
    let owner = indexed_owner(blocks);
    owner.verify_replay().unwrap();
    let source = endpoint_query(&owner);
    let mut graph = endpoint_graph(&source);
    // Closed form avoids the small-fixture oracle's cubic transitive closure.
    let vector_header = std::mem::size_of::<Vec<u32>>().div_ceil(std::mem::size_of::<usize>());
    let build = (2 + 2 * vector_header) + 4 * n + 6 * n + 3 * (n - 1) + 2 * n + 4;
    let path = n + 8 + path_scratch_allocation_work(n) + n + (n - 1) + 2 + 1 + (n - 1);
    let cold =
        endpoint_fixture_lookup(0) + 7 + path + 1 + n + 2 + build + endpoint_fixture_publication(0);
    graph.remaining = cold;
    let actual = graph.loan_region(0, 1).unwrap();
    assert_eq!(&actual.blocks[..2], &[true, true]);
    assert!(actual.blocks[2..].iter().all(|&x| !x));
    assert!(actual.acyclic);
    assert_eq!(graph.remaining, 0);
    // The changed start rebuilds the cone despite retaining all row capacities.
    let warm_path = n + 8 + (n - 1) + (n - 2) + 2 + 1;
    let warm =
        endpoint_fixture_lookup(1) + 7 + warm_path + 1 + n + 2 + endpoint_fixture_publication(1);
    graph.remaining = warm;
    let actual = graph.loan_region(1, 2).unwrap();
    assert_eq!(&actual.blocks[1..3], &[true, true]);
    assert!(actual.acyclic);
    assert_eq!(graph.remaining, 0);
    endpoint_assert_no_loan(&graph);
}

#[test]
fn capability_endpoint_scc_loans_keep_ordered_windows_and_failed_publication() {
    let owner = indexed_owner(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            statement(SemanticStatementKindV1::Nop),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(1),
            )),
        ],
        None,
    )]);
    owner.verify_replay().unwrap();
    let source = endpoint_query(&owner);
    for end in [Some(2), Some(3), None] {
        let mut graph = endpoint_graph(&source);
        let mut old = endpoint_graph(&source);
        let value = graph.use_value(0, 1).unwrap();
        assert_eq!(old.use_value(0, 1).unwrap(), value);
        let consumer = reuse_consumer(0, end);
        let actual = graph.loan_live(loan(value, 1), consumer);
        assert_eq!(actual.is_ok(), end.is_some());
        reuse_assert_same(
            actual,
            old.loan_live_before_path_scratch(loan(value, 1), consumer),
        );
        assert_eq!(graph.reuse.loans.len(), usize::from(end.is_some()));
    }
    // Independently metered old full-loan body plus the exact geometry delta.
    let mut old = endpoint_graph(&source);
    let value = old.use_value(0, 1).unwrap();
    let consumer = reuse_consumer(0, Some(3));
    let before = old.remaining;
    old.loan_live_before_path_scratch(loan(value, 1), consumer)
        .unwrap();
    let old_geometry = endpoint_fixture_lookup(0)
        + 1
        + 6
        + acyclic_scratch_header()
        + 3
        + 3
        + endpoint_fixture_publication(0);
    let mut oracle = EndpointCostOracle::new(1);
    let (geometry, _, _) = oracle.query(&[vec![]], 0, 0);
    let expected = (before - old.remaining) - old_geometry + geometry;
    for available in 0..=expected {
        let mut graph = endpoint_graph(&source);
        assert_eq!(graph.use_value(0, 1).unwrap(), value);
        graph.remaining = available;
        let result = graph.loan_live(loan(value, 1), consumer);
        if available < expected {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
            assert!(graph.reuse.loans.is_empty());
            assert!(graph.remaining <= available);
        } else {
            result.unwrap();
            assert_eq!(graph.remaining, 0);
            assert_eq!(graph.reuse.loans.len(), 1);
        }
    }
}

#[test]
fn capability_endpoint_scc_full_loans_keep_original_cycle_rejections() {
    for edges in [
        vec![vec![1, 1], vec![2], vec![]],
        vec![vec![1], vec![1, 2], vec![]],
        vec![vec![1, 2], vec![2, 3], vec![1, 3], vec![]],
        vec![vec![1], vec![], vec![2]],
    ] {
        let owner = indexed_owner(reuse_body(&edges).blocks().to_vec());
        owner.verify_replay().unwrap();
        let source = endpoint_query(&owner);
        let mut graph = endpoint_graph(&source);
        let mut old = endpoint_graph(&source);
        let value = graph.use_value(0, 1).unwrap();
        assert_eq!(old.use_value(0, 1).unwrap(), value);
        for block in 0..edges.len() {
            for end in [Some(0), None] {
                let consumer = reuse_consumer(block as u32, end);
                reuse_assert_same(
                    graph.loan_live(loan(value, 1), consumer),
                    old.loan_live_before_path_scratch(loan(value, 1), consumer),
                );
                assert_eq!(graph.reuse.loans, old.reuse.loans);
            }
        }
        assert!(old.reuse.endpoint_scc.is_none());
    }
}
