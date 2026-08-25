use dialect_kernel::{
    DIALECT_NAME as KERNEL_DIALECT_NAME, IndexConstantOp, RankedViewOp, RankedViewType,
    SemanticSymbolOp,
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ObligationOp, ObligationRefType,
    ProofIdAttr, ProofOverlayOpInterface, PropertyAttr, RegistrationError, RegistrationOutcome,
    RequireEffectRefinementOp, evidence_ref_op_attr_names, register_dialect,
};
use pliron::{
    attribute::{AttrObj, verify_attr},
    builtin::{
        attributes::{BytesAttr, UnitAttr},
        types::UnitType,
    },
    combine::{Parser, eof},
    context::Context,
    dialect::DialectName,
    identifier::Identifier,
    op::{Op, op_cast, verify_op},
    operation::{Operation, OperationParserConfig},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::TypeHandle,
};

fn id(seed: u64) -> ProofIdAttr {
    ProofIdAttr::new([seed, seed + 1, seed + 2, seed + 3])
}

#[test]
fn registration_is_real_duplicate_safe_and_round_trips_entities() {
    let mut context = Context::new();
    assert_eq!(
        register_dialect(&mut context),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(&mut context),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );

    let attribute: AttrObj = Box::new(id(10));
    let attribute_text = attribute.disp(&context).to_string();
    let parsed_attribute = parse_from_str(
        AttrObj::parser(()).skip(eof()),
        &mut context,
        &attribute_text,
    )
    .expect("registered proof attribute must parse");
    assert_eq!(
        parsed_attribute.downcast_ref::<ProofIdAttr>(),
        Some(&id(10))
    );

    let ty: TypeHandle = ObligationRefType::get(&context).into();
    let type_text = ty.disp(&context).to_string();
    let parsed_type = parse_from_str(TypeHandle::parser(()).skip(eof()), &mut context, &type_text)
        .expect("registered proof type must parse");
    assert!(parsed_type.deref(&context).is::<ObligationRefType>());

    let obligation = ObligationOp::new(
        &mut context,
        id(20),
        id(30),
        id(40),
        PropertyAttr::RaceFreedom,
    );
    let operation_text = obligation.disp(&context).to_string();
    let parsed_operation = parse_from_str(
        Operation::parser(OperationParserConfig {
            look_for_outlined_attrs: false,
        })
        .skip(eof()),
        &mut context,
        &operation_text,
    )
    .expect("registered proof operation must parse");
    let parsed = Operation::get_op_dyn(parsed_operation, &context);
    assert!(parsed.is::<ObligationOp>());
    verify_op(&*parsed, &context).expect("parsed obligation must verify");
}

#[test]
fn hostile_registration_marker_is_rejected() {
    let mut context = Context::new();
    let key = Identifier::try_from("fe2o3_dialect_proof_explicit_registration")
        .expect("valid marker key");
    let hostile = context.aux_data.insert(Box::new(false));
    context.aux_data_map.insert(key.clone(), hostile);
    assert_eq!(
        register_dialect(&mut context),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(&mut context),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn fixed_width_ids_have_bounded_parsing_and_reject_zero() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("proof registration");

    let valid = "0123456789abcdef".repeat(4);
    let parsed = parse_from_str(ProofIdAttr::parser(()).skip(eof()), &mut context, &valid)
        .expect("exactly 256 bits of hexadecimal identity");
    assert!(!parsed.is_zero());

    for hostile in ["a".repeat(63), "a".repeat(65), "a".repeat(4_096)] {
        assert!(
            parse_from_str(ProofIdAttr::parser(()).skip(eof()), &mut context, &hostile).is_err(),
            "hostile proof identity length {} parsed",
            hostile.len()
        );
    }
    assert!(verify_attr(&ProofIdAttr::new([0; 4]), &context).is_err());
}

#[test]
fn property_statuses_remain_independent_and_non_authoritative() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("proof registration");

    let obligation_id = id(100);
    let bounds = EvidenceRefOp::new(
        &mut context,
        id(110),
        obligation_id.clone(),
        PropertyAttr::Bounds,
        EvidenceStatusAttr::Proved,
        CoveredBoundaryAttr::TargetNeutralGpu,
    );
    let deadlock = EvidenceRefOp::new(
        &mut context,
        id(120),
        obligation_id,
        PropertyAttr::DeadlockFreedom,
        EvidenceStatusAttr::Unsupported,
        CoveredBoundaryAttr::TargetNeutralGpu,
    );

    verify_op(&bounds, &context).expect("bounded property evidence");
    verify_op(&deadlock, &context).expect("unsupported property evidence");
    assert_eq!(bounds.status(&context), Some(EvidenceStatusAttr::Proved));
    assert_eq!(
        deadlock.status(&context),
        Some(EvidenceStatusAttr::Unsupported)
    );
    assert!(!EvidenceStatusAttr::Proved.grants_authority());

    for op in [&bounds as &dyn Op, &deadlock] {
        let interface = op_cast::<dyn ProofOverlayOpInterface>(op)
            .expect("proof overlay interface must be registered");
        assert!(!interface.is_executable());
        assert!(!interface.grants_authority());
    }
}

#[test]
fn verifier_rejects_identity_confusion_and_zero_references() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("proof registration");

    let same = id(200);
    let confused = EvidenceRefOp::new(
        &mut context,
        same.clone(),
        same,
        PropertyAttr::FunctionalRefinement,
        EvidenceStatusAttr::Validated,
        CoveredBoundaryAttr::StructuredKernel,
    );
    assert!(verify_op(&confused, &context).is_err());

    let zero = ObligationOp::new(
        &mut context,
        ProofIdAttr::new([0; 4]),
        id(210),
        id(220),
        PropertyAttr::Initialization,
    );
    assert!(verify_op(&zero, &context).is_err());
}

#[test]
fn verifier_rejects_missing_wrong_extra_and_structural_payloads() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("proof registration");

    let wrong_status = EvidenceRefOp::new(
        &mut context,
        id(300),
        id(310),
        PropertyAttr::Convergence,
        EvidenceStatusAttr::Checked,
        CoveredBoundaryAttr::Schedule,
    );
    wrong_status
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(
            evidence_ref_op_attr_names::ATTR_KEY_PROOF_EVIDENCE_REF_STATUS.clone(),
            UnitAttr,
        );
    assert!(verify_op(&wrong_status, &context).is_err());

    let extra_obligation = ObligationOp::new(
        &mut context,
        id(320),
        id(330),
        id(340),
        PropertyAttr::Provenance,
    );
    let extra_evidence = EvidenceRefOp::new(
        &mut context,
        id(321),
        id(331),
        PropertyAttr::Bounds,
        EvidenceStatusAttr::Validated,
        CoveredBoundaryAttr::Mir,
    );
    for op in [&extra_obligation as &dyn Op, &extra_evidence] {
        op.get_operation().deref_mut(&context).attributes.set(
            Identifier::try_from("proof_hostile_extra").expect("valid key"),
            BytesAttr::new(vec![0xde, 0xad, 0xbe, 0xef]),
        );
        assert!(
            verify_op(op, &context).is_err(),
            "{} accepted an undeclared byte payload",
            op.get_opid()
        );
    }

    let missing = EvidenceRefOp::new(
        &mut context,
        id(350),
        id(360),
        PropertyAttr::NumericalBound,
        EvidenceStatusAttr::Contracted,
        CoveredBoundaryAttr::Source,
    );
    missing
        .get_operation()
        .deref_mut(&context)
        .attributes
        .0
        .remove(&*evidence_ref_op_attr_names::ATTR_KEY_PROOF_EVIDENCE_REF_STATUS);
    assert!(verify_op(&missing, &context).is_err());

    let result_type = UnitType::get(&context).into();
    let operation = Operation::new(
        &mut context,
        ObligationOp::get_concrete_op_info(),
        vec![result_type],
        vec![],
        vec![],
        0,
    );
    let malformed = ObligationOp::from_operation(operation);
    malformed.set_attr_proof_obligation_obligation_id(&context, id(370));
    malformed.set_attr_proof_obligation_subject_id(&context, id(380));
    malformed.set_attr_proof_obligation_model_id(&context, id(390));
    malformed.set_attr_proof_obligation_property(&context, PropertyAttr::Determinism);
    assert!(verify_op(&malformed, &context).is_err());
}

#[test]
fn effect_refinement_locally_requires_ranked_indices_and_six_semantic_expressions() {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(KERNEL_DIALECT_NAME).unwrap(),
    )
    .unwrap();
    register_dialect(&mut context).unwrap();
    let view_type = RankedViewType::new(&mut context, 32, true, vec![4]).unwrap();
    let view = RankedViewOp::new(&mut context, view_type, vec![]).unwrap();
    let index = IndexConstantOp::new(&mut context, 0);
    let scalar = SemanticSymbolOp::new(&mut context, 7);
    let view_value = view.result(&context);
    let index_value = index.result(&context);
    let scalar_value = scalar.result(&context);
    let valid = RequireEffectRefinementOp::new(
        &mut context,
        id(400),
        view_value,
        vec![index_value],
        vec![scalar_value],
        vec![scalar_value],
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
    );
    verify_op(&valid, &context).expect("closed typed effect contract");

    let missing_index = RequireEffectRefinementOp::new(
        &mut context,
        id(410),
        view_value,
        vec![],
        vec![scalar_value],
        vec![scalar_value],
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
    );
    assert!(verify_op(&missing_index, &context).is_err());

    let wrong_semantic = RequireEffectRefinementOp::new(
        &mut context,
        id(420),
        view_value,
        vec![index_value],
        vec![index_value],
        vec![scalar_value],
        index_value,
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
        scalar_value,
    );
    assert!(verify_op(&wrong_semantic, &context).is_err());
}
