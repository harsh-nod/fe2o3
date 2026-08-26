use dialect_kernel::{
    DIALECT_NAME, RequireEquivalentOp, ReturnOp, SemanticBinaryKindAttr, SemanticBinaryOp,
    SemanticExceptionalValueAttr, SemanticExpressionCommitmentOp, SemanticIeeeRoundingAttr,
    SemanticNumericalContractV1, SemanticNumericalPolicyAttr, SemanticOverflowAttr,
    SemanticScalarKindAttr, SemanticScalarType, SemanticSymbolOp, SemanticTypedBinaryKindAttr,
    SemanticTypedBinaryOp, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedExpressionV1, SemanticTypedScalarV1, SemanticTypedSymbolOp, register_dialect,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, ProofIdAttr,
    PropertyAttr, RequireRefinementOp,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, PlironSemanticRefinementFindingV1,
    require_pliron_semantic_refinement_before_lowering_v1, run_pliron_semantic_refinement_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    r#type::TypeHandle,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn proof_id(seed: u64) -> ProofIdAttr {
    ProofIdAttr::new([seed, seed + 1, seed + 2, seed + 3])
}

fn append_reference_contract(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    actual: pliron::value::Value,
    expected: pliron::value::Value,
    property: PropertyAttr,
    status: EvidenceStatusAttr,
    boundary: CoveredBoundaryAttr,
) {
    let obligation = ObligationOp::new(context, proof_id(1), proof_id(10), proof_id(20), property);
    let evidence = EvidenceRefOp::new(
        context,
        proof_id(30),
        proof_id(1),
        property,
        status,
        boundary,
    );
    let refinement = RequireRefinementOp::new(context, proof_id(1), actual, expected);
    append(context, block, &obligation);
    append(context, block, &evidence);
    append(context, block, &refinement);
}

fn function(context: &mut Context, name: &str, arguments: usize) -> FuncOp {
    let scalar: TypeHandle = SemanticScalarType::get(context).into();
    FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![scalar; arguments], vec![]),
    )
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn digest_words(digest: [u8; 32]) -> [u64; 4] {
    std::array::from_fn(|index| {
        u64::from_le_bytes(digest[index * 8..(index + 1) * 8].try_into().unwrap())
    })
}

fn exact_bitvector_contract() -> SemanticNumericalContractV1 {
    SemanticNumericalContractV1 {
        policy: SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        rounding: SemanticIeeeRoundingAttr::NearestTiesToEven,
        exceptional_values: SemanticExceptionalValueAttr::PreserveExactBits,
    }
}

fn typed_add_expression() -> SemanticTypedExpressionV1 {
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 32).unwrap();
    SemanticTypedExpressionV1::Binary {
        operation: SemanticTypedBinaryKindAttr::Add,
        scalar,
        overflow: SemanticOverflowAttr::Wrapping,
        lhs: Box::new(SemanticTypedExpressionV1::Symbol { symbol: 7, scalar }),
        rhs: Box::new(SemanticTypedExpressionV1::Constant { scalar, bits: 4 }),
    }
}

fn append_typed_binary_root(
    context: &mut Context,
    function: &FuncOp,
    binary_kind: SemanticTypedBinaryKindAttr,
    rhs_scalar: SemanticTypedScalarV1,
    contract: SemanticNumericalContractV1,
    commitment: [u64; 4],
) -> pliron::value::Value {
    let entry = function.get_entry_block(context);
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 32).unwrap();
    let symbol = SemanticTypedSymbolOp::new(context, 7, scalar);
    let constant = SemanticTypedConstantOp::new(context, 4, rhs_scalar);
    let binary = SemanticTypedBinaryOp::new(
        context,
        binary_kind,
        SemanticOverflowAttr::Wrapping,
        scalar,
        symbol.result(context),
        constant.result(context),
    );
    let root = SemanticTypedExpressionRootOp::new(
        context,
        binary.result(context),
        contract.policy,
        contract.rounding,
        contract.exceptional_values,
        commitment,
    );
    for operation in [
        symbol.get_operation(),
        constant.get_operation(),
        binary.get_operation(),
        root.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    root.result(context)
}

#[test]
fn typed_ssa_payload_is_reconstructed_and_hashed_by_the_mandatory_pass() {
    let context = &mut setup();
    let function = function(context, "typed_ssa_payload", 0);
    let expression = typed_add_expression();
    let contract = exact_bitvector_contract();
    let commitment = digest_words(expression.canonical_transcript_sha256(contract));
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 32).unwrap();
    let actual = append_typed_binary_root(
        context,
        &function,
        SemanticTypedBinaryKindAttr::Add,
        scalar,
        contract,
        commitment,
    );
    let expected = append_typed_binary_root(
        context,
        &function,
        SemanticTypedBinaryKindAttr::Add,
        scalar,
        contract,
        commitment,
    );
    let entry = function.get_entry_block(context);
    let requirement = RequireEquivalentOp::new(context, actual, expected);
    let ret = ReturnOp::new(context);
    append(context, entry, &requirement);
    append(context, entry, &ret);
    let report = run_pliron_semantic_refinement_check_v1(context, &function);
    assert!(report.is_clean());
    assert_eq!(report.typed_root_commitments(), &[commitment, commitment]);
}

#[test]
fn typed_ssa_payload_rejects_type_operator_policy_and_commitment_substitution() {
    let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 32).unwrap();
    let wrong_scalar =
        SemanticTypedScalarV1::new(SemanticScalarKindAttr::UnsignedInteger, 64).unwrap();
    let expression = typed_add_expression();
    let exact = exact_bitvector_contract();
    let commitment = digest_words(expression.canonical_transcript_sha256(exact));
    let ieee = SemanticNumericalContractV1 {
        policy: SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits,
        ..exact
    };
    let cases = [
        (
            SemanticTypedBinaryKindAttr::Add,
            wrong_scalar,
            exact,
            commitment,
        ),
        (
            SemanticTypedBinaryKindAttr::Multiply,
            scalar,
            exact,
            commitment,
        ),
        (
            SemanticTypedBinaryKindAttr::Add,
            scalar,
            ieee,
            digest_words(expression.canonical_transcript_sha256(ieee)),
        ),
        (
            SemanticTypedBinaryKindAttr::Add,
            scalar,
            exact,
            [
                commitment[0] ^ 1,
                commitment[1],
                commitment[2],
                commitment[3],
            ],
        ),
    ];
    for (index, (operation, rhs_scalar, contract, commitment)) in cases.into_iter().enumerate() {
        let context = &mut setup();
        let function = function(context, &format!("typed_substitution_{index}"), 0);
        append_typed_binary_root(
            context, &function, operation, rhs_scalar, contract, commitment,
        );
        let entry = function.get_entry_block(context);
        let ret = ReturnOp::new(context);
        append(context, entry, &ret);
        let report = run_pliron_semantic_refinement_check_v1(context, &function);
        assert!(matches!(
            report.findings(),
            [PlironSemanticRefinementFindingV1::TypedExpressionRejected { .. }]
        ));
    }
}

#[test]
fn identical_declared_expressions_are_accepted() {
    let context = &mut setup();
    let function = function(context, "identical", 0);
    let entry = function.get_entry_block(context);
    let alpha = SemanticSymbolOp::new(context, 0);
    let accumulator = SemanticSymbolOp::new(context, 1);
    let product = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Multiply,
        alpha.result(context),
        accumulator.result(context),
    );
    let requirement =
        RequireEquivalentOp::new(context, product.result(context), product.result(context));
    let ret = ReturnOp::new(context);
    append(context, entry, &alpha);
    append(context, entry, &accumulator);
    append(context, entry, &product);
    append(context, entry, &requirement);
    append(context, entry, &ret);
    let report = run_pliron_semantic_refinement_check_v1(context, &function);
    assert_eq!(report.pass(), KernelCheckPassKindV1::SemanticRefinement);
    assert!(report.is_clean());
}

#[test]
fn typed_commitments_normalize_only_by_exact_identity() {
    let context = &mut setup();
    let function = function(context, "typed_commitments", 0);
    let entry = function.get_entry_block(context);
    let actual = SemanticExpressionCommitmentOp::new(context, [1, 2, 3, 4]);
    let equal = SemanticExpressionCommitmentOp::new(context, [1, 2, 3, 4]);
    let different = SemanticExpressionCommitmentOp::new(context, [1, 2, 3, 5]);
    let accepted = RequireEquivalentOp::new(context, actual.result(context), equal.result(context));
    let rejected =
        RequireEquivalentOp::new(context, actual.result(context), different.result(context));
    let ret = ReturnOp::new(context);
    for operation in [
        actual.get_operation(),
        equal.get_operation(),
        different.get_operation(),
        accepted.get_operation(),
        rejected.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let report = run_pliron_semantic_refinement_check_v1(context, &function);
    assert!(!report.is_clean());
    assert!(matches!(
        report.findings(),
        [PlironSemanticRefinementFindingV1::ExpressionMismatch { .. }]
    ));
}

#[test]
fn commutative_operand_order_is_normalized_without_gemm_knowledge() {
    let context = &mut setup();
    let function = function(context, "commutative", 0);
    let entry = function.get_entry_block(context);
    let lhs = SemanticSymbolOp::new(context, 0);
    let rhs = SemanticSymbolOp::new(context, 1);
    let actual = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        lhs.result(context),
        rhs.result(context),
    );
    let expected = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        rhs.result(context),
        lhs.result(context),
    );
    let requirement =
        RequireEquivalentOp::new(context, actual.result(context), expected.result(context));
    let ret = ReturnOp::new(context);
    for operation in [
        lhs.get_operation(),
        rhs.get_operation(),
        actual.get_operation(),
        expected.get_operation(),
        requirement.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    assert!(run_pliron_semantic_refinement_check_v1(context, &function).is_clean());
}

#[test]
fn missing_dynamic_coefficient_is_reported_as_expression_mismatch() {
    let context = &mut setup();
    let function = function(context, "incorrect_epilogue", 0);
    let entry = function.get_entry_block(context);
    let alpha = SemanticSymbolOp::new(context, 0);
    let accumulator = SemanticSymbolOp::new(context, 1);
    let beta = SemanticSymbolOp::new(context, 2);
    let initial = SemanticSymbolOp::new(context, 3);
    let alpha_acc = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Multiply,
        alpha.result(context),
        accumulator.result(context),
    );
    let beta_initial = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Multiply,
        beta.result(context),
        initial.result(context),
    );
    let actual = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        alpha_acc.result(context),
        initial.result(context),
    );
    let expected = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        alpha_acc.result(context),
        beta_initial.result(context),
    );
    let requirement =
        RequireEquivalentOp::new(context, actual.result(context), expected.result(context));
    let ret = ReturnOp::new(context);
    for operation in [
        alpha.get_operation(),
        accumulator.get_operation(),
        beta.get_operation(),
        initial.get_operation(),
        alpha_acc.get_operation(),
        beta_initial.get_operation(),
        actual.get_operation(),
        expected.get_operation(),
        requirement.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let error =
        require_pliron_semantic_refinement_before_lowering_v1(context, &function).unwrap_err();
    assert!(matches!(
        error.report().findings(),
        [PlironSemanticRefinementFindingV1::ExpressionMismatch { .. }]
    ));
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("error[FE2O3-SEMANTIC-001]"));
    assert!(diagnostic.contains("actual expression"));
    assert!(diagnostic.contains("required expression"));
    assert!(diagnostic.contains("s2"));
}

#[test]
fn undeclared_foreign_expression_fails_closed() {
    let context = &mut setup();
    let function = function(context, "unresolved", 1);
    let entry = function.get_entry_block(context);
    let argument = entry.deref(context).arguments().next().unwrap();
    let expected = SemanticSymbolOp::new(context, 0);
    let requirement = RequireEquivalentOp::new(context, argument, expected.result(context));
    let ret = ReturnOp::new(context);
    append(context, entry, &expected);
    append(context, entry, &requirement);
    append(context, entry, &ret);
    let report = run_pliron_semantic_refinement_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [PlironSemanticRefinementFindingV1::UnresolvedExpression { .. }]
    ));
}

#[test]
fn structurally_different_association_is_not_silently_reassociated() {
    let context = &mut setup();
    let function = function(context, "association", 0);
    let entry = function.get_entry_block(context);
    let a = SemanticSymbolOp::new(context, 0);
    let b = SemanticSymbolOp::new(context, 1);
    let c = SemanticSymbolOp::new(context, 2);
    let ab = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        a.result(context),
        b.result(context),
    );
    let left = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        ab.result(context),
        c.result(context),
    );
    let bc = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        b.result(context),
        c.result(context),
    );
    let right = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        a.result(context),
        bc.result(context),
    );
    let requirement =
        RequireEquivalentOp::new(context, left.result(context), right.result(context));
    let ret = ReturnOp::new(context);
    for operation in [
        a.get_operation(),
        b.get_operation(),
        c.get_operation(),
        ab.get_operation(),
        left.get_operation(),
        bc.get_operation(),
        right.get_operation(),
        requirement.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    assert!(!run_pliron_semantic_refinement_check_v1(context, &function).is_clean());
}

#[test]
fn exact_policy_checked_mir_reference_is_joined_to_semantic_equality() {
    let context = &mut setup();
    let function = function(context, "reference_ok", 0);
    let entry = function.get_entry_block(context);
    let lhs = SemanticSymbolOp::new(context, 0);
    let rhs = SemanticSymbolOp::new(context, 1);
    let actual = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        lhs.result(context),
        rhs.result(context),
    );
    let expected = SemanticBinaryOp::new(
        context,
        SemanticBinaryKindAttr::Add,
        rhs.result(context),
        lhs.result(context),
    );
    for operation in [
        lhs.get_operation(),
        rhs.get_operation(),
        actual.get_operation(),
        expected.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append_reference_contract(
        context,
        entry,
        actual.result(context),
        expected.result(context),
        PropertyAttr::FunctionalRefinement,
        EvidenceStatusAttr::Checked,
        CoveredBoundaryAttr::Mir,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);

    let report = run_pliron_semantic_refinement_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert_eq!(report.reference_obligation_count(), 1);
    assert_eq!(report.policy_checked_reference_obligation_count(), 1);
    assert!(report.all_reference_obligations_are_policy_checked());
    assert!(!report.grants_compiler_refinement_authority());
}

#[test]
fn policy_checked_reference_rejects_a_semantic_mismatch() {
    let context = &mut setup();
    let function = function(context, "reference_mismatch", 0);
    let entry = function.get_entry_block(context);
    let actual = SemanticSymbolOp::new(context, 0);
    let expected = SemanticSymbolOp::new(context, 1);
    append(context, entry, &actual);
    append(context, entry, &expected);
    append_reference_contract(
        context,
        entry,
        actual.result(context),
        expected.result(context),
        PropertyAttr::FunctionalRefinement,
        EvidenceStatusAttr::Checked,
        CoveredBoundaryAttr::Mir,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);

    let error =
        require_pliron_semantic_refinement_before_lowering_v1(context, &function).unwrap_err();
    assert!(error.to_string().contains("error[FE2O3-SEMANTIC-001]"));
    assert_eq!(error.report().reference_obligation_count(), 1);
    assert_eq!(
        error.report().policy_checked_reference_obligation_count(),
        0
    );
}

#[test]
fn reference_without_evidence_fails_closed() {
    let context = &mut setup();
    let function = function(context, "reference_missing_evidence", 0);
    let entry = function.get_entry_block(context);
    let value = SemanticSymbolOp::new(context, 0);
    let obligation = ObligationOp::new(
        context,
        proof_id(1),
        proof_id(10),
        proof_id(20),
        PropertyAttr::FunctionalRefinement,
    );
    let refinement = RequireRefinementOp::new(
        context,
        proof_id(1),
        value.result(context),
        value.result(context),
    );
    append(context, entry, &value);
    append(context, entry, &obligation);
    append(context, entry, &refinement);
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);

    let error =
        require_pliron_semantic_refinement_before_lowering_v1(context, &function).unwrap_err();
    assert!(error.to_string().contains("error[FE2O3-SEMANTIC-003]"));
    assert!(error.to_string().contains("evidence_ref record is missing"));
}

#[test]
fn forged_proved_or_wrong_boundary_evidence_is_incomplete() {
    for (name, status, boundary, expected) in [
        (
            "forged_proved",
            EvidenceStatusAttr::Proved,
            CoveredBoundaryAttr::Mir,
            "requires exact Checked evidence",
        ),
        (
            "wrong_boundary",
            EvidenceStatusAttr::Checked,
            CoveredBoundaryAttr::Source,
            "must cover the exact MIR boundary",
        ),
    ] {
        let context = &mut setup();
        let function = function(context, name, 0);
        let entry = function.get_entry_block(context);
        let value = SemanticSymbolOp::new(context, 0);
        append(context, entry, &value);
        append_reference_contract(
            context,
            entry,
            value.result(context),
            value.result(context),
            PropertyAttr::FunctionalRefinement,
            status,
            boundary,
        );
        let ret = ReturnOp::new(context);
        append(context, entry, &ret);
        let error =
            require_pliron_semantic_refinement_before_lowering_v1(context, &function).unwrap_err();
        assert!(error.to_string().contains(expected));
    }
}

#[test]
fn wrong_property_and_duplicate_evidence_are_rejected() {
    let context = &mut setup();
    let wrong_function = function(context, "wrong_property", 0);
    let entry = wrong_function.get_entry_block(context);
    let value = SemanticSymbolOp::new(context, 0);
    append(context, entry, &value);
    append_reference_contract(
        context,
        entry,
        value.result(context),
        value.result(context),
        PropertyAttr::Bounds,
        EvidenceStatusAttr::Checked,
        CoveredBoundaryAttr::Mir,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    let error = require_pliron_semantic_refinement_before_lowering_v1(context, &wrong_function)
        .unwrap_err();
    assert!(error.to_string().contains("error[FE2O3-SEMANTIC-004]"));
    assert!(error.to_string().contains("not FunctionalRefinement"));

    let context = &mut setup();
    let function = function(context, "duplicate_evidence", 0);
    let entry = function.get_entry_block(context);
    let value = SemanticSymbolOp::new(context, 0);
    append(context, entry, &value);
    append_reference_contract(
        context,
        entry,
        value.result(context),
        value.result(context),
        PropertyAttr::FunctionalRefinement,
        EvidenceStatusAttr::Checked,
        CoveredBoundaryAttr::Mir,
    );
    let duplicate = EvidenceRefOp::new(
        context,
        proof_id(40),
        proof_id(1),
        PropertyAttr::FunctionalRefinement,
        EvidenceStatusAttr::Checked,
        CoveredBoundaryAttr::Mir,
    );
    append(context, entry, &duplicate);
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    let error =
        require_pliron_semantic_refinement_before_lowering_v1(context, &function).unwrap_err();
    assert!(error.to_string().contains("more than one evidence record"));
}

#[test]
fn orphan_functional_obligation_is_incomplete() {
    let context = &mut setup();
    let function = function(context, "orphan", 0);
    let entry = function.get_entry_block(context);
    let obligation = ObligationOp::new(
        context,
        proof_id(1),
        proof_id(10),
        proof_id(20),
        PropertyAttr::FunctionalRefinement,
    );
    append(context, entry, &obligation);
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    let error =
        require_pliron_semantic_refinement_before_lowering_v1(context, &function).unwrap_err();
    assert!(error.to_string().contains("has no semantic equality"));
}
