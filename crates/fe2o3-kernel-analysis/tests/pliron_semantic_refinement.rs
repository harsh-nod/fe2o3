use dialect_kernel::{
    DIALECT_NAME, RequireEquivalentOp, ReturnOp, SemanticBinaryKindAttr, SemanticBinaryOp,
    SemanticScalarType, SemanticSymbolOp, register_dialect,
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
fn exact_proved_mir_reference_is_joined_to_semantic_equality() {
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
        EvidenceStatusAttr::Proved,
        CoveredBoundaryAttr::Mir,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);

    let report = run_pliron_semantic_refinement_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
    assert_eq!(report.reference_obligation_count(), 1);
    assert_eq!(report.proved_reference_obligation_count(), 1);
    assert!(report.all_reference_obligations_are_proved());
    assert!(!report.grants_compiler_refinement_authority());
}

#[test]
fn proved_reference_rejects_a_semantic_mismatch() {
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
        EvidenceStatusAttr::Proved,
        CoveredBoundaryAttr::Mir,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);

    let error =
        require_pliron_semantic_refinement_before_lowering_v1(context, &function).unwrap_err();
    assert!(error.to_string().contains("error[FE2O3-SEMANTIC-001]"));
    assert_eq!(error.report().reference_obligation_count(), 1);
    assert_eq!(error.report().proved_reference_obligation_count(), 0);
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
fn non_proved_or_wrong_boundary_evidence_is_incomplete() {
    for (name, status, boundary, expected) in [
        (
            "checked_only",
            EvidenceStatusAttr::Checked,
            CoveredBoundaryAttr::Mir,
            "requires exact Proved evidence",
        ),
        (
            "wrong_boundary",
            EvidenceStatusAttr::Proved,
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
        EvidenceStatusAttr::Proved,
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
        EvidenceStatusAttr::Proved,
        CoveredBoundaryAttr::Mir,
    );
    let duplicate = EvidenceRefOp::new(
        context,
        proof_id(40),
        proof_id(1),
        PropertyAttr::FunctionalRefinement,
        EvidenceStatusAttr::Proved,
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
