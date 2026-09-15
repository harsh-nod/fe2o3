#[test]
fn work_failure_trace_preserves_actual_graph_error_remaining_and_all_caches() {
    let body = reuse_body(&[vec![1], vec![]]);
    let plan = plan(&body);
    let run = |enabled| {
        work_failure_observation::with_enabled(enabled, || {
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let value = graph.use_value(0, 1).unwrap();
            graph.definition(value).unwrap();
            graph.reaches(0, 1).unwrap();
            graph
                .loan_live(loan(value, 1), reuse_consumer(1, None))
                .unwrap();
            let snapshot = (
                graph.reuse.uses.clone(),
                graph.reuse.definitions.clone(),
                graph.reuse.reachability.clone(),
                graph.reuse.loans.clone(),
            );
            graph.remaining = 3;
            for requested in [4, usize::MAX] {
                assert!(matches!(
                    graph.charge(requested),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual: 100_001,
                        limit: 100_000,
                    })
                ));
                assert_eq!(graph.remaining, 3);
                assert_eq!(
                    snapshot,
                    (
                        graph.reuse.uses.clone(),
                        graph.reuse.definitions.clone(),
                        graph.reuse.reachability.clone(),
                        graph.reuse.loans.clone()
                    )
                );
            }
            graph.charge(3).unwrap();
            graph.charge(0).unwrap();
            assert_eq!(graph.remaining, 0);
            snapshot
        })
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn traced_query_failure_does_not_publish_a_definition_or_a_fresh_allowance() {
    let body = reuse_body(&[vec![]]);
    let plan = plan(&body);
    for enabled in [false, true] {
        work_failure_observation::with_enabled(enabled, || {
            let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
            let value = graph.use_value(0, 1).unwrap();
            let lookup = reuse_definition_lookup_work();
            graph.remaining = lookup;
            assert!(matches!(
                graph.definition(value),
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: 100_001,
                    limit: 100_000,
                })
            ));
            assert_eq!(graph.remaining, 0);
            assert!(graph.reuse.definitions.is_empty());
            assert!(graph.reuse.definitions.owner.is_none());
            assert!(graph.reuse.definitions.rows.is_empty());
            assert_eq!(graph.reuse.definitions.rows.capacity(), 0);
            assert_eq!(graph.reuse.uses.len(), 1);
            assert!(graph.reuse.reachability.is_empty());
            assert!(graph.reuse.loans.is_empty());
        });
    }
}
