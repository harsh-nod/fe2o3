#[test]
fn execution_replay_rejects_omitted_defined_math_result_relation() {
    let mut owner = defined_math_results::tests::owner();
    assert!(!owner.execution.plans[0].1.defined_math_results().is_empty());
    owner.execution.plans[0].1.defined_math_results =
        defined_math_results::DefinedMathResultsV1::default();
    assert_eq!(
        owner.verify_replay(),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
}
