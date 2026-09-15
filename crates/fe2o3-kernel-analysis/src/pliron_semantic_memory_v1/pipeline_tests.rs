//! Common analysis consumers, not compiler authentication or launch authority.
use super::*;
use crate::pliron_semantic_refinement::{
    SemanticExpressionBuildErrorV1, SemanticExpressionTableV1,
};

fn reference(context: &mut Context, f: &Fixture) -> SemanticTypedBinaryOp {
    // The sequential component specification is input[i] + 1. Its operator
    // and constant are independent of the GPU arithmetic being checked.
    let one = SemanticTypedConstantOp::new(context, u64::from(1f32.to_bits()), scalar());
    one.get_operation().insert_before(context, f.write.get_operation());
    let result = SemanticTypedBinaryOp::new(
        context,
        SemanticTypedBinaryKindAttr::Add,
        SemanticOverflowAttr::Wrapping,
        scalar(),
        f.read.result(context),
        one.result(context),
    );
    result.get_operation().insert_before(context, f.write.get_operation());
    result
}

#[test]
fn source_memory_flows_into_common_expression_equality_and_operator_mutation() {
    for changed_operator in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context);
        let expected = reference(&mut context, &f);
        if changed_operator {
            f.sum.set_attr_kernel_semantic_typed_binary_kind(
                &mut context, SemanticTypedBinaryKindAttr::Multiply,
            );
        }
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &f.function).unwrap();
        let expressions = SemanticExpressionTableV1::from_inventory(
            &context, &f.function, &inventory,
        ).unwrap_or_else(|error| panic!("live read expression build: {error:?}"));
        assert_ne!(f.sum.result(&context), expected.result(&context));
        assert_eq!(expressions.equivalent(f.sum.result(&context), expected.result(&context)),
            Some(!changed_operator));
        expressions.revalidate_live_reads().unwrap();
    }
}

#[test]
fn common_expression_queries_cannot_outlive_the_actual_read_graph() {
    let mut context = setup();
    let f = fixture(&mut context);
    let expected = reference(&mut context, &f);
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &f.function).unwrap();
    let expressions = SemanticExpressionTableV1::from_inventory(
        &context, &f.function, &inventory,
    ).unwrap_or_else(|error| panic!("live read expression build: {error:?}"));
    assert_eq!(expressions.equivalent(f.sum.result(&context), expected.result(&context)), Some(true));
    f.access.get_operation().unlink(&context);
    assert_eq!(expressions.equivalent(f.sum.result(&context), expected.result(&context)), None);
    assert!(matches!(expressions.revalidate_live_reads(),
        Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(_))));
}

#[test]
fn common_expression_build_rejects_read_after_write_and_free_load_symbols() {
    for free_symbol in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context);
        if free_symbol {
            let forged = SemanticTypedSymbolOp::new(
                &mut context, SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, scalar(),
            );
            forged.get_operation().insert_before(&context, f.ret.get_operation());
        } else {
            f.write.get_operation().unlink(&context);
            f.write.get_operation().insert_before(&context, f.access.get_operation());
        }
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &f.function).unwrap();
        assert!(matches!(
            SemanticExpressionTableV1::from_inventory(&context, &f.function, &inventory),
            Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(_)),
        ));
    }
}

#[test]
fn producer_pair_passes_bounds_but_never_supplies_a_missing_write_contract() {
    let mut context = setup();
    let f = fixture(&mut context);
    let bounds = crate::run_pliron_ranked_bounds_check_v1(&context, &f.function);
    assert!(bounds.is_clean(), "{bounds:?}");
    let effects = crate::run_pliron_effect_refinement_check_v1(&context, &f.function);
    assert!(effects.findings().iter().any(|finding| matches!(finding,
        crate::PlironEffectRefinementFindingV1::UnmodeledWriteSite { .. })));
    assert!(!crate::run_pliron_semantic_refinement_check_v1(&context, &f.function).is_clean());

    f.access.get_operation().unlink(&context);
    let bounds = crate::run_pliron_ranked_bounds_check_v1(&context, &f.function);
    assert!(bounds.findings().iter().any(|finding| matches!(finding,
        crate::RankedBoundsFindingV1::UnsupportedOperation { operation: 2, .. })));
}
