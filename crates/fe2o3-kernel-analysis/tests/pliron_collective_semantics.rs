use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
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
    KernelCheckStatusV1, PlironEffectRefinementFindingV1, PlironSemanticRefinementFindingV1,
    PlironSemanticRefinementReportV1, ProductionPlironPreloweringErrorV2,
    require_production_pliron_checks_before_lowering_v2, run_pliron_ranked_bounds_check_v1,
    run_pliron_semantic_refinement_check_v1,
};
use fe2o3_pliron_owner_core::ensure_context_identity;
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    operation::{Operation, verify_operation},
    parsable::parse_from_str,
};

fn setup() -> Context {
    let mut context = Context::new();
    ensure_context_identity(&mut context).expect("collective test context identity");
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
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
) -> PlironSemanticRefinementReportV1 {
    let context = &mut setup();
    let function = FuncOp::new(
        context,
        "finite_fold".try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let entry = function.get_entry_block(context);
    // Satisfy execution prerequisites so production reaches effect refinement.
    let layout = ExecutionLayoutOp::new_with_domain(
        context,
        41,
        [1, 1, 1],
        [1, 1, 1],
        1,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    append(context, entry, &layout);
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
            EvidenceStatusAttr::Checked,
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
    assert_missing_effect_contract_blocks_prelowering(context, &function)
}

fn assert_missing_effect_contract_blocks_prelowering(
    context: &Context,
    function: &FuncOp,
) -> PlironSemanticRefinementReportV1 {
    let bounds = run_pliron_ranked_bounds_check_v1(context, function);
    assert!(bounds.is_clean(), "{bounds:?}");
    let report = run_pliron_semantic_refinement_check_v1(context, function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected, "{report:?}");
    let effects = report.effect_refinement();
    assert_eq!(effects.status(), KernelCheckStatusV1::Rejected);
    assert_eq!(effects.contract_count(), 0);
    assert_eq!(effects.proved_contract_count(), 0);
    assert!(
        matches!(
            effects.findings(),
            [PlironEffectRefinementFindingV1::UnmodeledWriteSite { .. }]
        ),
        "{effects:?}"
    );
    assert!(!effects.all_declared_effects_are_proved());
    assert!(!effects.grants_compiler_refinement_authority());
    assert!(!effects.grants_artifact_or_launch_authority());
    assert!(!report.all_collective_contracts_are_policy_checked());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());

    let error = require_production_pliron_checks_before_lowering_v2(context, function)
        .expect_err("collective staging cannot discharge the missing write-effect contract");
    let ProductionPlironPreloweringErrorV2::Semantic(error) = error else {
        panic!("production must reach semantic effect refinement, got {error:?}");
    };
    assert_eq!(error.report(), &report);
    report
}

fn assert_collective_component_policy_checked(report: &PlironSemanticRefinementReportV1) {
    assert!(report.findings().is_empty(), "{report:?}");
    assert!(report.progress().is_clean(), "{report:?}");
    assert_eq!(report.reference_obligation_count(), 1);
    assert_eq!(report.policy_checked_reference_obligation_count(), 1);
    assert_eq!(report.collective_contract_count(), 1);
    assert_eq!(report.policy_checked_collective_contract_count(), 1);
    // The aggregate predicate also requires the independent effect check to pass.
    assert!(!report.all_collective_contracts_are_policy_checked());
}

#[test]
fn finite_fold_needs_both_coverage_and_an_independent_value_proof() {
    let report = fold_report(
        SemanticCoverageBindingAttr::TotalView,
        OwnershipCoverageAttr::TotalView,
        true,
    );
    assert_collective_component_policy_checked(&report);
}

#[test]
fn exactly_once_contributions_never_infer_the_fold_value() {
    let report = fold_report(
        SemanticCoverageBindingAttr::CollectiveContributions,
        OwnershipCoverageAttr::CollectiveContributions,
        false,
    );
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert_eq!(report.collective_contract_count(), 1);
    assert_eq!(report.reference_obligation_count(), 0);
    assert_eq!(report.policy_checked_reference_obligation_count(), 0);
    assert_eq!(report.policy_checked_collective_contract_count(), 0);
    let [finding @ PlironSemanticRefinementFindingV1::CollectiveContractIncomplete { reason, .. }] =
        report.findings()
    else {
        panic!("missing fold value proof must remain incomplete, got {report:?}");
    };
    assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
    assert!(reason.contains("coverage never proves a final value"));
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

#[test]
fn policy_checked_collective_fixtures_reject_missing_effect_contracts() {
    for (name, source) in [
        (
            "fold",
            include_str!("lit/collective_fold_policy_checked.pliron"),
        ),
        (
            "recurrence",
            include_str!("lit/collective_recurrence_policy_checked.pliron"),
        ),
        (
            "permutation",
            include_str!("lit/collective_permutation_policy_checked.pliron"),
        ),
    ] {
        let context = &mut setup();
        let ir = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let operation = parse_from_str(Operation::top_level_parser(), context, &ir)
            .unwrap_or_else(|error| panic!("{name} fixture failed to parse: {error:?}"));
        verify_operation(operation, context)
            .unwrap_or_else(|error| panic!("{name} fixture failed local verification: {error:?}"));
        assert!(Operation::is_op::<FuncOp>(operation, context));
        let function = FuncOp::from_operation(operation);
        let report = assert_missing_effect_contract_blocks_prelowering(context, &function);
        assert_collective_component_policy_checked(&report);
    }
}
