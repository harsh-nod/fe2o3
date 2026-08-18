use dialect_kernel::{
    AlgorithmOp, AlgorithmType, DIALECT_NAME, ITERATION_DOMAIN_ATTR_KEY, IterationDomainAttr,
    KernelError, MAX_ITERATION_RANK, RegistrationError, RegistrationOutcome, SemanticOwner,
    StructuredAlgorithmOp, register_dialect,
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

fn kernel_name() -> DialectName {
    DialectName::try_new(DIALECT_NAME).expect("valid dialect")
}

#[test]
fn registration_is_real_idempotent_and_collision_safe() {
    let context = &mut Context::new();
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );
    assert_eq!(
        register_dialect(
            context,
            &DialectName::try_new("schedule").expect("valid dialect")
        ),
        Err(RegistrationError::WrongDialect)
    );

    let parsed = parse_from_str(TypeHandle::parser(()), context, "kernel.algorithm <2>")
        .expect("registered type parses");
    assert!(parsed.verify(context).is_ok());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "kernel.iteration_domain <2>",
    )
    .expect("registered attribute parses");
    assert!(parsed.verify(context).is_ok());

    let operation = AlgorithmOp::new(context, 2).expect("bounded algorithm");
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
    let key: Identifier = "fe2o3_dialect_kernel_registration_v1"
        .try_into()
        .expect("valid key");
    let hostile = context.aux_data.insert(Box::new(17_u32));
    context.aux_data_map.insert(key, hostile);
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn constructors_and_parsed_values_enforce_rank_bounds() {
    let context = &mut Context::new();
    assert_eq!(
        AlgorithmType::new(context, 0).unwrap_err(),
        KernelError::IterationRankOutOfBounds(0)
    );
    assert_eq!(
        IterationDomainAttr::new(MAX_ITERATION_RANK + 1).unwrap_err(),
        KernelError::IterationRankOutOfBounds(MAX_ITERATION_RANK + 1)
    );

    let parsed = parse_from_str(TypeHandle::parser(()), context, "kernel.algorithm <0>")
        .expect("syntax is valid before semantic verification");
    assert!(parsed.verify(context).is_err());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "kernel.iteration_domain <4294967295>",
    )
    .expect("bounded scalar syntax parses");
    assert!(parsed.verify(context).is_err());
}

#[test]
fn op_interface_reports_only_target_neutral_kernel_ownership() {
    let context = &mut Context::new();
    let algorithm = AlgorithmOp::new(context, 3).expect("bounded algorithm");
    verify_op(&algorithm, context).expect("valid algorithm");

    let interface = op_cast::<dyn StructuredAlgorithmOp>(&algorithm).expect("interface present");
    assert_eq!(interface.semantic_owner(), SemanticOwner::Kernel);
    assert!(interface.is_target_neutral());
}

#[test]
fn hostile_operation_shapes_and_metadata_fail_verification() {
    let context = &mut Context::new();

    let wrong_type = UnitType::get(context);
    let raw = Operation::new(
        context,
        AlgorithmOp::get_concrete_op_info(),
        vec![wrong_type.into()],
        vec![],
        vec![],
        0,
    );
    let wrong_type_op = AlgorithmOp::from_operation(raw);
    wrong_type_op.set_iteration_domain(
        context,
        IterationDomainAttr::new(2).expect("bounded domain"),
    );
    assert!(verify_op(&wrong_type_op, context).is_err());

    let bounded_type = AlgorithmType::new(context, 2).expect("bounded type").into();
    let missing = Operation::new(
        context,
        AlgorithmOp::get_concrete_op_info(),
        vec![bounded_type],
        vec![],
        vec![],
        0,
    );
    assert!(verify_op(&AlgorithmOp::from_operation(missing), context).is_err());

    let mismatched = AlgorithmOp::new(context, 2).expect("bounded algorithm");
    mismatched
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            ITERATION_DOMAIN_ATTR_KEY.try_into().expect("valid key"),
            Box::new(IterationDomainAttr::new(3).expect("bounded domain")),
        );
    assert!(verify_op(&mismatched, context).is_err());

    let extra = AlgorithmOp::new(context, 2).expect("bounded algorithm");
    extra
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            "kernel_hostile_extra".try_into().expect("valid key"),
            Box::new(BytesAttr::new(vec![0xde, 0xad, 0xbe, 0xef])),
        );
    assert!(verify_op(&extra, context).is_err());
}
