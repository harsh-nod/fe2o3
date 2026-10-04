#[test]
fn ranked_custody_recipe_mismatch_keeps_error_and_nonpoisoned_state() {
    let mut session = ranked_session();
    let (stage, mut root) = construct_ranked(&mut session, "custody_mismatch");
    let hostile = ProductionExactGraphIdentityV1([0xa5; 32]);
    session
        .constructed_roots
        .get_mut(&stage.identity)
        .expect("live constructed record")
        .exact_graph_identity = Some(hostile);
    root.exact_graph_identity = Some(hostile);

    let result = session.verify_production_ranked_kernel_pipeline(stage, root);
    assert!(matches!(
        result,
        Err(ProductionSessionErrorV1::RankedGraphChanged)
    ));
    assert!(!session.is_poisoned());
}
