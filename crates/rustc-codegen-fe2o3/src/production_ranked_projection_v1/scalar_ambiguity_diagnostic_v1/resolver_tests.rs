use super::*;
use super::super::GpuSemanticExpressionResolverV2;

#[test]
fn scalar_diagnostic_hook_keeps_original_multiple_definition_error_and_work() {
    let (types, function) = super::tests::fixture(2);
    let operand = super::tests::final_operand(&function);
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function);
    let error = resolver.resolve_operand_v2(operand, 0).unwrap_err();
    assert_eq!(
        error,
        "GPU semantic scalar local has multiple definitions; select/phi normalization is incomplete"
    );
    let ambiguity = resolver
        .ambiguous_use
        .expect("failure hook retains exact failed occurrence");
    assert_eq!(ambiguity.local, 2);
    assert!(exact_operand(operand, ambiguity.place));
    let work_before = resolver.work;
    let plan = fe2o3_pliron::plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &[],
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let context = Context {
        function: &function,
        plan: plan.plan(),
        view: None,
    };
    let report = context.describe(12, 0, Some((0, Some(2))), ambiguity);
    assert!(report.contains("exact-use=bb0:s2"));
    assert_eq!(resolver.work, work_before);
    assert_eq!(resolver.resolve_operand_v2(operand, 0).unwrap_err(), error);
}
