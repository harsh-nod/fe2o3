use dialect_kernel::{
    DIALECT_NAME, MemorySpaceAttr, RankedViewOp, RankedViewType, RequireFiniteFoldOp,
    RequireFiniteRecurrenceOp, RequirePermutationGatherOp, SemanticConstantOp,
    SemanticCoverageBindingAttr, SemanticEvaluationOrderAttr, SemanticExceptionalValueAttr,
    SemanticExpressionCommitmentAttr, SemanticIeeeRoundingAttr, SemanticNumericalPolicyAttr,
    SemanticScalarKindAttr, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedScalarV1, register_dialect,
};
use pliron::{
    context::Context,
    dialect::DialectName,
    op::{Op, verify_op},
    operation::Operation,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn view(context: &mut Context) -> RankedViewOp {
    let ty = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    RankedViewOp::new_in_space_with_allocation_contract(
        context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        7,
        7,
    )
    .unwrap()
}

fn typed_root(context: &mut Context, seed: u64) -> SemanticTypedExpressionRootOp {
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 64).unwrap();
    let value = SemanticTypedConstantOp::new(context, seed, scalar);
    SemanticTypedExpressionRootOp::new(
        context,
        value.result(context),
        SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        SemanticIeeeRoundingAttr::NearestTiesToEven,
        SemanticExceptionalValueAttr::PreserveExactBits,
        [seed, seed + 1, seed + 2, seed + 3],
    )
}

#[test]
fn all_finite_contract_kinds_have_closed_verified_payloads() {
    let context = &mut setup();
    let view = view(context);
    let values = (1..=6)
        .map(|seed| typed_root(context, seed * 10))
        .collect::<Vec<_>>();
    let fold = RequireFiniteFoldOp::new(
        context,
        view.result(context),
        values[0].result(context),
        values[0].result(context),
        values[1].result(context),
        values[2].result(context),
        SemanticExpressionCommitmentAttr::new([101, 102, 103, 104]),
        SemanticExpressionCommitmentAttr::new([111, 112, 113, 114]),
        64,
        64,
        SemanticEvaluationOrderAttr::Ascending,
        SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        SemanticCoverageBindingAttr::TotalView,
    );
    let recurrence = RequireFiniteRecurrenceOp::new(
        context,
        view.result(context),
        values[0].result(context),
        values[0].result(context),
        values[1].result(context),
        values[2].result(context),
        SemanticExpressionCommitmentAttr::new([121, 122, 123, 124]),
        SemanticExpressionCommitmentAttr::new([131, 132, 133, 134]),
        32,
        17,
        SemanticEvaluationOrderAttr::Lexicographic,
        SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits,
        SemanticCoverageBindingAttr::TotalView,
    );
    let permutation = RequirePermutationGatherOp::new(
        context,
        view.result(context),
        values[3].result(context),
        values[3].result(context),
        values[4].result(context),
        values[5].result(context),
        SemanticExpressionCommitmentAttr::new([141, 142, 143, 144]),
        SemanticExpressionCommitmentAttr::new([151, 152, 153, 154]),
        SemanticExpressionCommitmentAttr::new([161, 162, 163, 164]),
        128,
        128,
        SemanticEvaluationOrderAttr::Explicit,
        SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        SemanticCoverageBindingAttr::CollectiveContributions,
    );

    verify_op(&fold, context).unwrap();
    verify_op(&recurrence, context).unwrap();
    verify_op(&permutation, context).unwrap();
}

#[test]
fn zero_excessive_and_nonterminating_bounds_are_rejected() {
    let context = &mut setup();
    let view = view(context);
    let actual = typed_root(context, 1);
    let identity = typed_root(context, 10);
    let operator = typed_root(context, 20);
    for (domain, steps) in [(0, 0), (1 << 24, (1 << 24) + 1), ((1 << 24) + 1, 1)] {
        let contract = RequireFiniteFoldOp::new(
            context,
            view.result(context),
            actual.result(context),
            actual.result(context),
            identity.result(context),
            operator.result(context),
            SemanticExpressionCommitmentAttr::new([31, 32, 33, 34]),
            SemanticExpressionCommitmentAttr::new([41, 42, 43, 44]),
            domain,
            steps,
            SemanticEvaluationOrderAttr::Ascending,
            SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
            SemanticCoverageBindingAttr::TotalView,
        );
        assert!(verify_op(&contract, context).is_err());
    }
}

#[test]
fn untyped_or_open_witness_payloads_are_rejected() {
    let context = &mut setup();
    let view = view(context);
    let actual = typed_root(context, 1);
    let identity = typed_root(context, 10);
    let untyped_operator = SemanticConstantOp::new(context, 0);
    let contract = RequireFiniteFoldOp::new(
        context,
        view.result(context),
        actual.result(context),
        actual.result(context),
        identity.result(context),
        untyped_operator.result(context),
        SemanticExpressionCommitmentAttr::new([31, 32, 33, 34]),
        SemanticExpressionCommitmentAttr::new([41, 42, 43, 44]),
        1,
        1,
        SemanticEvaluationOrderAttr::Ascending,
        SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        SemanticCoverageBindingAttr::TotalView,
    );
    assert!(verify_op(&contract, context).is_err());
    Operation::pop_operand(contract.get_operation(), context);
    assert!(verify_op(&contract, context).is_err());
}
