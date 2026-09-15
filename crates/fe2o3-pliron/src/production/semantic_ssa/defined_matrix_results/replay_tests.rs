#[test]
fn execution_replay_rejects_omitted_defined_matrix_result_relation() {
    let mut owner = defined_matrix_results::tests::owner();
    assert!(
        !owner.execution.plans[0]
            .1
            .defined_matrix_results()
            .is_empty()
    );
    owner.execution.plans[0].1.defined_matrix_results =
        defined_matrix_results::DefinedMatrixResultsV1::default();
    assert_eq!(
        owner.verify_replay(),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
}
