use dialect_autotune::{
    AutotuneError, BUDGET_ATTR_KEY, CandidateBudgetAttr, CandidateSetOp, CandidateSetType,
    DIALECT_NAME, InertAutotuneOp, MAX_CANDIDATES, MAX_OBSERVATIONS_PER_CANDIDATE,
    RegistrationError, RegistrationOutcome, SemanticOwner, register_dialect,
};
use pliron::{
    attribute::Attribute,
    builtin::{
        attributes::BytesAttr, op_interfaces::SingleBlockRegionInterface, ops::ModuleOp,
        types::UnitType,
    },
    common_traits::Verify,
    context::Context,
    dialect::DialectName,
    identifier::Identifier,
    op::{Op, op_cast, verify_op},
    operation::{Operation, verify_operation},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::TypeHandle,
};

fn autotune_name() -> DialectName {
    DialectName::try_new(DIALECT_NAME).expect("valid dialect")
}

#[test]
fn registration_is_real_idempotent_and_collision_safe() {
    let context = &mut Context::new();
    assert_eq!(
        register_dialect(context, &autotune_name()),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(context, &autotune_name()),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );
    assert_eq!(
        register_dialect(
            context,
            &DialectName::try_new("kernel").expect("valid dialect")
        ),
        Err(RegistrationError::WrongDialect)
    );

    let parsed = parse_from_str(
        TypeHandle::parser(()),
        context,
        "autotune.candidate_set <4>",
    )
    .expect("registered type parses");
    assert!(parsed.verify(context).is_ok());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "autotune.candidate_budget <4, 8>",
    )
    .expect("registered attribute parses");
    assert!(parsed.verify(context).is_ok());

    let operation = CandidateSetOp::new(context, 4, 8).expect("bounded candidate set");
    let module = ModuleOp::new(context, "registration".try_into().expect("valid name"));
    module.append_operation(context, operation.get_operation(), 0);
    let printed = module.get_operation().disp(context).to_string();
    let parsed = parse_from_str(Operation::top_level_parser(), context, &printed)
        .expect("registered operation parses");
    verify_operation(parsed, context).expect("parsed operation verifies");
}

#[test]
fn hostile_registration_marker_is_rejected() {
    let context = &mut Context::new();
    let key: Identifier = "fe2o3_dialect_autotune_registration_v1"
        .try_into()
        .expect("valid key");
    let hostile = context.aux_data.insert(Box::new(9_u8));
    context.aux_data_map.insert(key, hostile);
    assert_eq!(
        register_dialect(context, &autotune_name()),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(context, &autotune_name()),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn constructors_and_parsed_values_enforce_candidate_bounds() {
    let context = &mut Context::new();
    assert_eq!(
        CandidateSetType::new(context, MAX_CANDIDATES + 1).unwrap_err(),
        AutotuneError::CandidatesOutOfBounds(MAX_CANDIDATES + 1)
    );
    assert_eq!(
        CandidateBudgetAttr::new(1, MAX_OBSERVATIONS_PER_CANDIDATE + 1).unwrap_err(),
        AutotuneError::ObservationsOutOfBounds(MAX_OBSERVATIONS_PER_CANDIDATE + 1)
    );

    let parsed = parse_from_str(
        TypeHandle::parser(()),
        context,
        "autotune.candidate_set <0>",
    )
    .expect("syntax is valid before semantic verification");
    assert!(parsed.verify(context).is_err());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "autotune.candidate_budget <1, 0>",
    )
    .expect("bounded scalar syntax parses");
    assert!(parsed.verify(context).is_err());
}

#[test]
fn interface_is_target_neutral_and_inert() {
    let context = &mut Context::new();
    let candidates = CandidateSetOp::new(context, 4, 8).expect("bounded candidate set");
    verify_op(&candidates, context).expect("valid candidate set");

    let interface = op_cast::<dyn InertAutotuneOp>(&candidates).expect("interface present");
    assert_eq!(interface.semantic_owner(), SemanticOwner::Autotune);
    assert!(interface.is_target_neutral());
    assert!(!interface.is_executable());
}

#[test]
fn hostile_operation_shapes_and_metadata_fail_verification() {
    let context = &mut Context::new();

    let unit_type = UnitType::get(context).into();
    let raw = Operation::new(
        context,
        CandidateSetOp::get_concrete_op_info(),
        vec![unit_type],
        vec![],
        vec![],
        0,
    );
    let wrong_type = CandidateSetOp::from_operation(raw);
    wrong_type.set_budget(
        context,
        CandidateBudgetAttr::new(2, 4).expect("bounded budget"),
    );
    assert!(verify_op(&wrong_type, context).is_err());

    let candidate_type = CandidateSetType::new(context, 2)
        .expect("bounded type")
        .into();
    let missing = Operation::new(
        context,
        CandidateSetOp::get_concrete_op_info(),
        vec![candidate_type],
        vec![],
        vec![],
        0,
    );
    assert!(verify_op(&CandidateSetOp::from_operation(missing), context).is_err());

    let mismatched = CandidateSetOp::new(context, 2, 4).expect("bounded candidate set");
    mismatched
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            BUDGET_ATTR_KEY.try_into().expect("valid key"),
            Box::new(CandidateBudgetAttr::new(3, 4).expect("bounded budget")),
        );
    assert!(verify_op(&mismatched, context).is_err());

    let extra = CandidateSetOp::new(context, 2, 4).expect("bounded candidate set");
    extra
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            "autotune_hostile_extra".try_into().expect("valid key"),
            Box::new(BytesAttr::new(vec![0xde, 0xad, 0xbe, 0xef])),
        );
    assert!(verify_op(&extra, context).is_err());
}
