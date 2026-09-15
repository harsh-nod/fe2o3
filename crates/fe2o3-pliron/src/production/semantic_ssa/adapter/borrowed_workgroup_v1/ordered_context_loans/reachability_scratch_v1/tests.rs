use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}
fn order(edges: Vec<Vec<u32>>, entry: u32) -> Order {
    Order {
        graph: SccGraph::dfs_only(edges),
        entry,
        cache: BTreeMap::new(),
        scratch: None,
    }
}
fn site(block: u32, statement: u32) -> Site {
    Site { block, statement }
}

#[test]
fn all_three_block_graphs_match_nonempty_transitive_closure_and_site_order() {
    for mask in 0..512 {
        let edges = (0..3)
            .map(|from| {
                (0..3)
                    .filter(|to| mask & (1 << (from * 3 + to)) != 0)
                    .map(|to| to as u32)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut reachable = [[false; 3]; 3];
        for (from, targets) in edges.iter().enumerate() {
            for &to in targets {
                reachable[from][to as usize] = true;
            }
        }
        for via in 0..3 {
            for from in 0..3 {
                for to in 0..3 {
                    reachable[from][to] |= reachable[from][via] && reachable[via][to];
                }
            }
        }
        for entry in 0..3 {
            let mut order = order(edges.clone(), entry);
            let mut work = budget(MAX_FLOW_WORK);
            for from in 0..3 {
                for to in 0..3 {
                    assert_eq!(
                        order.path(from, to, &mut work).unwrap(),
                        reachable[from as usize][to as usize],
                        "mask {mask} {from}->{to}"
                    );
                }
            }
            for from in 0..6 {
                for to in 0..6 {
                    let a = site(from / 2, from % 2);
                    let b = site(to / 2, to % 2);
                    let live =
                        |p: Site| p.block == entry || reachable[entry as usize][p.block as usize];
                    let follows = |a: Site, b: Site| {
                        (a.block == b.block && a.statement <= b.statement)
                            || reachable[a.block as usize][b.block as usize]
                    };
                    assert_eq!(
                        order.before(a, b, &mut work).unwrap(),
                        a != b && live(a) && live(b) && follows(a, b) && !follows(b, a),
                        "mask {mask}, entry {entry}, sites {a:?} {b:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn early_positive_pending_entries_and_failed_searches_cannot_contaminate_next_query() {
    let mut order = order(vec![vec![1, 2], vec![3], vec![], vec![]], 0);
    let mut work = budget(MAX_FLOW_WORK);
    assert!(order.path(0, 2, &mut work).unwrap());
    assert!(!order.scratch.as_ref().unwrap().pending.is_empty());
    assert!(!order.path(2, 3, &mut work).unwrap());
    let previous = order.cache.len();
    let error = order.path(0, 3, &mut budget(3)).unwrap_err();
    assert!(matches!(
        flow_work_profile_v1::original_error_for_test(error),
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            required: 4,
            limit: 3,
            ..
        }
    ));
    assert_eq!(order.cache.len(), previous);
    assert!(!order.cache.contains_key(&(0, 3)));
    assert!(order.path(0, 3, &mut work).unwrap());
    assert!(!order.path(3, 0, &mut work).unwrap());
}

#[test]
fn byte_generation_wrap_preserves_both_signs_and_storage_capacity() {
    for cyclic in [false, true] {
        let edges = (0..18)
            .map(|i| {
                if i < 17 {
                    vec![i + 1]
                } else if cyclic {
                    vec![0]
                } else {
                    vec![]
                }
            })
            .collect();
        let mut order = order(edges, 0);
        let mut work = budget(MAX_FLOW_WORK);
        assert_eq!(order.path(0, 0, &mut work).unwrap(), cyclic);
        let scratch = order.scratch.as_ref().unwrap();
        let storage = (
            scratch.seen.as_ptr(),
            scratch.seen.capacity(),
            scratch.pending.as_ptr(),
            scratch.pending.capacity(),
        );
        for from in 0..18 {
            for to in 0..18 {
                assert_eq!(
                    order.path(from, to, &mut work).unwrap(),
                    cyclic || from < to
                );
            }
        }
        let scratch = order.scratch.as_ref().unwrap();
        assert_eq!(scratch.generation, 69);
        assert_eq!(
            storage,
            (
                scratch.seen.as_ptr(),
                scratch.seen.capacity(),
                scratch.pending.as_ptr(),
                scratch.pending.capacity()
            )
        );
        assert_eq!(order.cache.len(), 324);
        let mut hit = budget(1);
        assert_eq!(order.path(0, 0, &mut hit).unwrap(), cyclic);
        assert_eq!(hit.remaining, 0);
        assert_eq!(order.scratch.as_ref().unwrap().generation, 69);
    }
    assert_eq!(std::mem::size_of::<u8>(), std::mem::size_of::<bool>());
}

#[test]
fn wrap_reset_is_fully_charged_before_reusing_a_generation() {
    let mut order = order(
        (0..18)
            .map(|i| if i < 17 { vec![i + 1] } else { vec![] })
            .collect(),
        0,
    );
    let mut work = budget(MAX_FLOW_WORK);
    for i in 0..255 {
        order.path(i / 18, i % 18, &mut work).unwrap();
    }
    assert_eq!(order.scratch.as_ref().unwrap().generation, u8::MAX);
    let error = order.path(14, 3, &mut budget(19)).unwrap_err();
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        stage,
        remaining_work_units,
        requested_work_units,
        error,
        ..
    } = error
    else {
        panic!("missing original phase diagnostic");
    };
    assert_eq!(
        (stage, remaining_work_units, requested_work_units),
        ("paths", 17, 18)
    );
    assert!(matches!(
        *error,
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            required: 20,
            limit: 19,
            ..
        }
    ));
    assert_eq!(order.scratch.as_ref().unwrap().generation, u8::MAX);
    assert_eq!(order.cache.len(), 255);
    assert!(!order.path(14, 3, &mut work).unwrap());
    assert_eq!(order.scratch.as_ref().unwrap().generation, 1);
}

// Independent old DFS work oracle; no production proof or graph is duplicated.
fn old_query(edges: &[Vec<u32>], start: u32, end: u32) -> (bool, usize) {
    let mut used = 2 + edges.len() * 2;
    let mut seen = vec![false; edges.len()];
    let mut pending = vec![start];
    seen[start as usize] = true;
    while let Some(current) = pending.pop() {
        used += 1;
        for &next in &edges[current as usize] {
            used += 1;
            if next == end {
                return (true, used);
            }
            if !seen[next as usize] {
                seen[next as usize] = true;
                pending.push(next);
            }
        }
    }
    (false, used)
}

#[test]
fn only_repeated_buffer_initialization_changes_query_work() {
    let mut order = order(vec![vec![1, 2], vec![3], vec![3], vec![]], 0);
    let mut work = budget(MAX_FLOW_WORK);
    for (i, (from, to)) in [(0, 1), (0, 3), (3, 0), (0, 0), (2, 3)]
        .into_iter()
        .enumerate()
    {
        let (expected, old_work) = old_query(order.graph.edges(), from, to);
        let before = work.remaining;
        assert_eq!(order.path(from, to, &mut work).unwrap(), expected);
        let expected_work = old_work + 1 - if i == 0 { 0 } else { order.graph.edges().len() * 2 };
        assert_eq!(before - work.remaining, expected_work);
    }
    let mut fresh = self::order(vec![vec![]; 4], 0);
    assert!(fresh.path(0, 0, &mut budget(8)).is_err());
    assert!(fresh.scratch.is_none());
    assert!(fresh.cache.is_empty());
    assert!(matches!(
        fresh.path(4, 0, &mut budget(1)),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn function(terms: Vec<SemanticTerminatorKindV1>) -> SemanticFunctionDeclV1 {
    let ty = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([1; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([1; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([1; 32]),
        source,
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([1; 32]),
            ty,
            SemanticLocalRoleV1::Return,
            source,
        )],
        SemanticBlockIdV1::from_index(0),
        terms
            .into_iter()
            .enumerate()
            .map(|(i, term)| {
                let mut identity = [0; 32];
                identity[..4].copy_from_slice(&(i as u32).to_be_bytes());
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(identity),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, term),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn real_cfg_constructor_preserves_imaginary_normal_unwind_and_drop_edges() {
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(0),
        vec![],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap();
    let body = function(vec![
        SemanticTerminatorKindV1::FalseEdge {
            real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
            imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 2),
        },
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place.clone(),
                    edge(SemanticEdgeRoleV1::CallReturn, 3),
                )),
                SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind, 4)),
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::Drop {
            place,
            drop_glue: SemanticFunctionIdV1::from_index(0),
            target: edge(SemanticEdgeRoleV1::DropReturn, 4),
            unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::DropUnwind, 0)),
        },
        SemanticTerminatorKindV1::Return,
        SemanticTerminatorKindV1::UnwindResume,
    ]);
    let mut work = budget(MAX_FLOW_WORK);
    let mut order = Order::new(&body, &mut work).unwrap();
    assert_eq!(
        order.graph.edges(),
        [vec![1, 2], vec![3, 4], vec![4, 0], vec![], vec![]]
    );
    assert!(order.path(0, 0, &mut work).unwrap());
    assert!(order.path(2, 3, &mut work).unwrap());
    assert!(order.path(1, 4, &mut work).unwrap());
    assert!(!order.before(site(0, 0), site(2, 0), &mut work).unwrap());
    let invalid = function(vec![SemanticTerminatorKindV1::Goto(edge(
        SemanticEdgeRoleV1::Goto,
        7,
    ))]);
    assert!(matches!(
        Order::new(&invalid, &mut work),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}
