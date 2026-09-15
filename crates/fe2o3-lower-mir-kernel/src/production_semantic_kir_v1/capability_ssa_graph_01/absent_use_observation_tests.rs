#[test]
fn absent_use_observation_reports_exact_query_coordinates_without_a_statement() {
    let mut bytes = Vec::new();
    absent_use_observation::write_observation(
        &mut bytes,
        &[0xab; 32],
        25,
        61,
        7,
        1234,
        "scoped_matrix_custody_01/resolve.rs",
        136,
        32,
    )
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with(&format!(
        "capability-ssa-absent-use body={} ",
        "ab".repeat(32)
    )));
    assert!(text.contains(
        "block=25 local=61 statement=unavailable(block-wide-query) events=7 remaining=1234"
    ));
    assert!(text.ends_with(
        "query_caller=scoped_matrix_custody_01/resolve.rs:136:32 caller_truncated=false\n"
    ));
    assert_eq!(text.lines().count(), 1);
    assert!(text.len() < 512);
}

#[test]
fn absent_use_observation_bounds_output_and_propagates_only_writer_failure() {
    let mut bytes = Vec::new();
    let file = "a".repeat(10_000);
    absent_use_observation::write_observation(
        &mut bytes,
        &[0xff; 32],
        u32::MAX,
        u32::MAX,
        usize::MAX,
        usize::MAX,
        &file,
        u32::MAX,
        u32::MAX,
    )
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.len() < 512);
    assert!(text.ends_with("caller_truncated=true\n"));
    assert!(!text.contains(&"a".repeat(161)));
    let mut short = &mut [0u8; 8][..];
    let error = absent_use_observation::write_observation(
        &mut short,
        &[0; 32],
        0,
        0,
        0,
        0,
        "caller.rs",
        1,
        1,
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::WriteZero);
}

#[test]
fn absent_use_rejection_preserves_error_cache_and_exact_shared_work() {
    let body = reuse_body(&[vec![]]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    // Local 2 is initialized but has no SSA Use in this block. Never substitute
    // that definition for the missing query, even on a repeated rejection.
    for _ in 0..2 {
        let before = graph.remaining;
        let expected_charge = lookup_work(graph.reuse.uses.len())
            + graph
                .ssa
                .resolved_events(SsaBlockIdV1::new(0))
                .unwrap()
                .len();
        assert!(matches!(
            graph.use_value(0, 2),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "capability reference has no exact SSA use",
            })
        ));
        assert_eq!(graph.remaining, before - expected_charge);
        assert!(graph.reuse.uses.is_empty());
        assert!(graph.reuse.definitions.is_empty());
    }
    graph.remaining = 0;
    assert!(matches!(
        graph.use_value(0, 2),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            limit: 100_000,
            ..
        })
    ));
    assert!(graph.reuse.uses.is_empty());
}
