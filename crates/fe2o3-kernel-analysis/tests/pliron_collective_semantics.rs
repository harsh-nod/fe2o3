use dialect_kernel::{
    AccessKindAttr, DIALECT_NAME, IndexConstantOp, MemorySpaceAttr, OwnershipContractOp,
    OwnershipCoverageAttr, OwnershipPartitionAttr, RankedAccessOp, RankedViewOp, RankedViewType,
    RequireFiniteFoldOp, ReturnOp, SemanticCoverageBindingAttr, SemanticEvaluationOrderAttr,
    SemanticExceptionalValueAttr, SemanticExpressionCommitmentAttr, SemanticIeeeRoundingAttr,
    SemanticNumericalContractV1, SemanticNumericalPolicyAttr, SemanticScalarKindAttr,
    SemanticTypedConstantOp, SemanticTypedExpressionRootOp, SemanticTypedExpressionV1,
    SemanticTypedScalarV1, register_dialect,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, ProofIdAttr,
    PropertyAttr, RequireRefinementOp,
};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, PlironSemanticRefinementFindingV1, run_pliron_ranked_bounds_check_v1,
    run_pliron_semantic_refinement_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn proof_id(seed: u64) -> ProofIdAttr {
    ProofIdAttr::new([seed, seed + 1, seed + 2, seed + 3])
}

fn digest_words(digest: [u8; 32]) -> [u64; 4] {
    std::array::from_fn(|index| {
        u64::from_le_bytes(digest[index * 8..(index + 1) * 8].try_into().unwrap())
    })
}

fn append_u32_root(context: &mut Context, block: Ptr<BasicBlock>) -> SemanticTypedExpressionRootOp {
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 32).unwrap();
    let expression = SemanticTypedExpressionV1::Constant { scalar, bits: 0 };
    let contract = SemanticNumericalContractV1 {
        policy: SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        rounding: SemanticIeeeRoundingAttr::NearestTiesToEven,
        exceptional_values: SemanticExceptionalValueAttr::PreserveExactBits,
    };
    let constant = SemanticTypedConstantOp::new(context, 0, scalar);
    let root = SemanticTypedExpressionRootOp::new(
        context,
        constant.result(context),
        contract.policy,
        contract.rounding,
        contract.exceptional_values,
        digest_words(expression.canonical_transcript_sha256(contract)),
    );
    append(context, block, &constant);
    append(context, block, &root);
    root
}

fn fold_report(
    requested: SemanticCoverageBindingAttr,
    provided: OwnershipCoverageAttr,
    with_proof: bool,
) -> fe2o3_kernel_analysis::PlironSemanticRefinementReportV1 {
    let context = &mut setup();
    let function = FuncOp::new(
        context,
        "finite_fold".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let entry = function.get_entry_block(context);
    let view_type = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        context,
        view_type,
        vec![],
        MemorySpaceAttr::Global,
        17,
        17,
    )
    .unwrap();
    let actual = append_u32_root(context, entry);
    let expected = append_u32_root(context, entry);
    let identity = append_u32_root(context, entry);
    let operator = append_u32_root(context, entry);
    let zero = IndexConstantOp::new(context, 0);
    let write = RankedAccessOp::new(
        context,
        AccessKindAttr::Write,
        view.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ownership = OwnershipContractOp::new(
        context,
        view.result(context),
        provided,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    let fold = RequireFiniteFoldOp::new(
        context,
        view.result(context),
        actual.result(context),
        expected.result(context),
        identity.result(context),
        operator.result(context),
        SemanticExpressionCommitmentAttr::new([21, 22, 23, 24]),
        SemanticExpressionCommitmentAttr::new([31, 32, 33, 34]),
        64,
        64,
        SemanticEvaluationOrderAttr::Ascending,
        SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        requested,
    );
    for operation in [
        view.get_operation(),
        zero.get_operation(),
        write.get_operation(),
        ownership.get_operation(),
        fold.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    if with_proof {
        let obligation = ObligationOp::new(
            context,
            proof_id(41),
            proof_id(51),
            proof_id(61),
            PropertyAttr::FunctionalRefinement,
        );
        let evidence = EvidenceRefOp::new(
            context,
            proof_id(71),
            proof_id(41),
            PropertyAttr::FunctionalRefinement,
            EvidenceStatusAttr::Proved,
            CoveredBoundaryAttr::Mir,
        );
        let refinement = RequireRefinementOp::new(
            context,
            proof_id(41),
            actual.result(context),
            expected.result(context),
        );
        append(context, entry, &obligation);
        append(context, entry, &evidence);
        append(context, entry, &refinement);
    }
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    let bounds = run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(bounds.is_clean(), "{bounds:?}");
    run_pliron_semantic_refinement_check_v1(context, &function)
}

#[test]
fn finite_fold_needs_both_coverage_and_an_independent_value_proof() {
    let report = fold_report(
        SemanticCoverageBindingAttr::TotalView,
        OwnershipCoverageAttr::TotalView,
        true,
    );
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.collective_contract_count(), 1);
    assert_eq!(report.proved_collective_contract_count(), 1);
    assert!(report.all_collective_contracts_are_proved());
}

#[test]
fn exactly_once_contributions_never_infer_the_fold_value() {
    let report = fold_report(
        SemanticCoverageBindingAttr::CollectiveContributions,
        OwnershipCoverageAttr::CollectiveContributions,
        false,
    );
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert_eq!(report.proved_collective_contract_count(), 0);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironSemanticRefinementFindingV1::CollectiveContractIncomplete { reason, .. }
            if reason.contains("coverage never proves a final value")
    )));
}

#[test]
fn a_different_coverage_theorem_is_rejected() {
    let report = fold_report(
        SemanticCoverageBindingAttr::TotalView,
        OwnershipCoverageAttr::CollectiveContributions,
        true,
    );
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironSemanticRefinementFindingV1::CollectiveContractRejected { .. }
    )));
}
