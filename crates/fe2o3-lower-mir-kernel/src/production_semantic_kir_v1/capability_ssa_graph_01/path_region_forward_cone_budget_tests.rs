fn cone_assert_work_error<T>(result: Result<T, ProductionSemanticKirErrorV1>, limit: usize) {
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit: cap,
        }) if actual == limit + 1 && cap == limit
    ));
}

#[test]
fn capability_path_forward_cone_every_cold_hit_rebuild_growth_wrap_prefix_and_retry() {
    for (edges, warm, from, to, wrap) in [
        (vec![vec![1; 17], vec![1, 2], vec![]], None, 0, 2, false),
        (
            vec![vec![1, 4], vec![2], vec![1, 3], vec![], vec![4]],
            Some((0, 3)),
            0,
            4,
            false,
        ),
        (
            vec![vec![1], vec![2], vec![], vec![2]],
            Some((0, 1)),
            0,
            3,
            false,
        ),
        (vec![vec![1], vec![2], vec![], vec![2]], None, 0, 3, false),
        (vec![vec![1], vec![2], vec![]], Some((0, 1)), 1, 2, false),
        (
            vec![vec![1, 2], vec![3; 9], vec![3], vec![]],
            Some((2, 3)),
            0,
            3,
            false,
        ),
        (vec![vec![1], vec![2], vec![]], Some((0, 2)), 1, 2, true),
    ] {
        let body = graph_body(&edges);
        let plan = plan(&body);
        let mut before_oracle = ConeWorkOracle::new(edges.len());
        if let Some((a, b)) = warm {
            before_oracle.query(&edges, a, b, false);
        }
        let mut after_oracle = before_oracle.clone();
        let (required, expected, hit) = after_oracle.query(&edges, from, to, wrap);
        assert!(!wrap || !hit);
        for available in 0..=required {
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            if let Some((a, b)) = warm {
                graph.path_region(a as u32, b as u32).unwrap();
            }
            if wrap {
                let scratch = graph.reuse.path_region_scratch.as_mut().unwrap();
                scratch.epoch = usize::MAX;
                scratch.seen.fill(1);
            }
            let retained = graph.reuse.path_region_scratch.as_deref().map(|s| {
                (
                    s as *const _,
                    s.epoch,
                    s.completed_from,
                    s.seen.clone(),
                    s.predecessors.clone(),
                )
            });
            graph.remaining = available;
            let result = graph.path_region(from as u32, to as u32);
            if available < required {
                cone_assert_work_error(result, 100_000);
                assert!(graph.remaining <= available);
                let kept = warm.is_some() && available < edges.len() + 2;
                assert_eq!(graph.reuse.path_region_scratch.is_some(), kept);
                if kept {
                    let scratch = graph.reuse.path_region_scratch.as_deref().unwrap();
                    let saved = retained.as_ref().unwrap();
                    assert_eq!(
                        (scratch as *const _, scratch.epoch, scratch.completed_from),
                        (saved.0, saved.1, saved.2)
                    );
                    assert_eq!(scratch.seen, saved.3);
                    assert_eq!(scratch.predecessors, saved.4);
                }
                let mut retry_oracle = if kept {
                    before_oracle.clone()
                } else {
                    ConeWorkOracle::new(edges.len())
                };
                let (retry, retry_region, _) = retry_oracle.query(&edges, from, to, wrap && kept);
                graph.remaining = retry;
                assert_eq!(
                    graph.path_region(from as u32, to as u32).unwrap(),
                    retry_region
                );
                assert_eq!(graph.remaining, 0);
                cone_assert_retained(&graph, &edges, &retry_oracle);
            } else {
                assert_eq!(result.unwrap(), expected);
                assert_eq!(graph.remaining, 0);
                cone_assert_retained(&graph, &edges, &after_oracle);
                if wrap {
                    assert_eq!(graph.reuse.path_region_scratch.as_ref().unwrap().epoch, 1);
                }
            }
            assert!(graph.reuse.loans.is_empty());
            assert!(graph.reuse.loan_regions.is_empty());
        }
    }
}

#[test]
fn capability_path_forward_cone_backward_failure_drops_completed_cone_before_retry() {
    let edges = [vec![1], vec![2], vec![]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    for warm in [false, true] {
        let mut oracle = ConeWorkOracle::new(edges.len());
        if warm {
            oracle.query(&edges, 0, 1, false);
        }
        let (required, _, hit) = oracle.query(&edges, 0, 2, false);
        assert_eq!(hit, warm);
        // Three vertices and two predecessor entries: stop only after the
        // forward cone has completed, at each possible backward failure.
        for backward_paid in 0..5 {
            let mut attempt = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            if warm {
                attempt.path_region(0, 1).unwrap();
            }
            attempt.remaining = required - 5 + backward_paid;
            cone_assert_work_error(attempt.path_region(0, 2), 100_000);
            assert!(attempt.reuse.path_region_scratch.is_none());
            assert!(attempt.reuse.loans.is_empty());
            assert!(attempt.reuse.loan_regions.is_empty());
            let mut retry_oracle = ConeWorkOracle::new(edges.len());
            let (cold, expected, hit) = retry_oracle.query(&edges, 0, 1, false);
            assert!(!hit);
            attempt.remaining = cold;
            assert_eq!(attempt.path_region(0, 1).unwrap(), expected);
            assert_eq!(attempt.remaining, 0);
            cone_assert_retained(&attempt, &edges, &retry_oracle);
        }
    }
}

#[test]
fn capability_path_forward_cone_invalid_bounds_and_dead_starts_preserve_prior_cone() {
    let edges = [vec![1], vec![], vec![1]];
    let body = graph_body(&edges);
    let plan = plan(&body);
    for warm in [false, true] {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        if warm {
            graph.path_region(0, 1).unwrap();
        }
        let retained = graph
            .reuse
            .path_region_scratch
            .as_deref()
            .map(|s| s as *const _);
        for (from, to) in [(3, 0), (0, 3), (u32::MAX, 0), (0, u32::MAX)] {
            graph.remaining = 0;
            assert!(matches!(
                graph.path_region(from, to),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(graph.remaining, 0);
            assert_eq!(
                graph
                    .reuse
                    .path_region_scratch
                    .as_deref()
                    .map(|s| s as *const _),
                retained
            );
        }
        for available in 0..=edges.len() {
            graph.remaining = available;
            let result = graph.path_region(2, 1);
            if available < edges.len() {
                cone_assert_work_error(result, 100_000);
                assert_eq!(graph.remaining, available);
            } else {
                assert_eq!(result.unwrap(), [false; 3]);
                assert_eq!(graph.remaining, 0);
            }
            assert_eq!(
                graph
                    .reuse
                    .path_region_scratch
                    .as_deref()
                    .map(|s| s as *const _),
                retained
            );
        }
    }
}

#[test]
fn capability_path_forward_cone_equal_hash_body_and_plan_substitutions_reject() {
    let first = endpoint_owner(&[vec![1], vec![2], vec![]]);
    let second = endpoint_owner(&[vec![1], vec![2], vec![]]);
    let a = endpoint_query(&first);
    let b = endpoint_query(&second);
    let body_copy = a.function().clone();
    let plan_copy = a.plan().plan().clone();
    assert_eq!(a.function().identity(), b.function().identity());
    assert_eq!(a.plan().plan().identity(), b.plan().plan().identity());
    assert_eq!(&body_copy, a.function());
    assert_eq!(&plan_copy, a.plan().plan());
    for substitution in 0..5 {
        for through_loan_region in [false, true] {
            // Unattached callers must enforce the cone's bindings themselves.
            let mut graph =
                CapabilitySsaGraphV1::new(a.function(), a.plan().plan(), 100_000).unwrap();
            graph.loan_region(0, 1).unwrap();
            let retained = Arc::clone(graph.reuse.loan_regions.get(&(0, 1)).unwrap());
            match substitution {
                0 => graph.body = b.function(),
                1 => graph.ssa = b.plan().plan(),
                2 => {
                    graph.body = b.function();
                    graph.ssa = b.plan().plan();
                }
                3 => graph.body = &body_copy,
                _ => graph.ssa = &plan_copy,
            }
            let before = graph.remaining;
            let result = if through_loan_region {
                graph.loan_region(0, 2).map(|_| ())
            } else {
                graph.path_region(0, 2).map(|_| ())
            };
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            let dispatch = if through_loan_region {
                endpoint_fixture_lookup(1) + 2
            } else {
                0
            };
            assert_eq!(before - graph.remaining, dispatch + 3 + 5);
            assert!(graph.reuse.path_region_scratch.is_none());
            assert_eq!(graph.reuse.loan_regions.len(), 1);
            assert!(Arc::ptr_eq(
                &retained,
                graph.reuse.loan_regions.get(&(0, 1)).unwrap()
            ));
            assert!(graph.reuse.loans.is_empty());
        }
    }
    let mut foreign = CapabilitySsaGraphV1::new(b.function(), b.plan().plan(), 100_000).unwrap();
    assert_eq!(foreign.path_region(0, 2).unwrap(), [true; 3]);
}

#[test]
fn capability_path_forward_cone_rebuild_validates_edges_beyond_requested_endpoint() {
    let scaffold = graph_body(&[vec![1, 2], vec![], vec![]]);
    let plan = plan(&scaffold);
    let body = graph_body(&[vec![1, 2], vec![], vec![99]]);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    assert_eq!(graph.path_region(1, 1).unwrap(), [false, true, false]);
    for to in [1, 0, 2] {
        let mut old = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let result = graph.path_region(0, to);
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        reuse_assert_same(result, old.path_region_before_scratch(0, to));
        assert!(graph.reuse.path_region_scratch.is_none());
        assert!(graph.reuse.loans.is_empty());
        assert!(graph.reuse.loan_regions.is_empty());
    }
}
