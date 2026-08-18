use dialect_schedule::{
    DIALECT_NAME, MAX_PIPELINE_STAGES, MAX_SCHEDULE_RANK, MAX_TILE_EXTENT, NonExecutableScheduleOp,
    PARAMETERS_ATTR_KEY, ParametersAttr, PlanOp, PlanType, RegistrationError, RegistrationOutcome,
    ScheduleError, SemanticOwner, register_dialect,
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

fn schedule_name() -> DialectName {
    DialectName::try_new(DIALECT_NAME).expect("valid dialect")
}

#[test]
fn registration_is_real_idempotent_and_collision_safe() {
    let context = &mut Context::new();
    assert_eq!(
        register_dialect(context, &schedule_name()),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(context, &schedule_name()),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );
    assert_eq!(
        register_dialect(
            context,
            &DialectName::try_new("kernel").expect("valid dialect")
        ),
        Err(RegistrationError::WrongDialect)
    );

    let parsed = parse_from_str(TypeHandle::parser(()), context, "schedule.plan_type <2, 3>")
        .expect("registered type parses");
    assert!(parsed.verify(context).is_ok());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "schedule.parameters <2, 64, 3>",
    )
    .expect("registered attribute parses");
    assert!(parsed.verify(context).is_ok());

    let operation = PlanOp::new(context, 2, 64, 3).expect("bounded plan");
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
    let key: Identifier = "fe2o3_dialect_schedule_registration_v1"
        .try_into()
        .expect("valid key");
    let hostile = context.aux_data.insert(Box::new(false));
    context.aux_data_map.insert(key, hostile);
    assert_eq!(
        register_dialect(context, &schedule_name()),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(context, &schedule_name()),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn constructors_and_parsed_values_enforce_all_bounds() {
    let context = &mut Context::new();
    assert_eq!(
        PlanType::new(context, 0, 1).unwrap_err(),
        ScheduleError::RankOutOfBounds(0)
    );
    assert_eq!(
        ParametersAttr::new(1, MAX_TILE_EXTENT + 1, 1).unwrap_err(),
        ScheduleError::TileExtentOutOfBounds(MAX_TILE_EXTENT + 1)
    );
    assert_eq!(
        ParametersAttr::new(1, 1, MAX_PIPELINE_STAGES + 1).unwrap_err(),
        ScheduleError::PipelineStagesOutOfBounds(MAX_PIPELINE_STAGES + 1)
    );

    let parsed = parse_from_str(
        TypeHandle::parser(()),
        context,
        &format!("schedule.plan_type <{}, 1>", MAX_SCHEDULE_RANK + 1),
    )
    .expect("syntax is valid before semantic verification");
    assert!(parsed.verify(context).is_err());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "schedule.parameters <1, 0, 1>",
    )
    .expect("bounded scalar syntax parses");
    assert!(parsed.verify(context).is_err());
}

#[test]
fn interface_marks_plans_target_neutral_and_non_executable() {
    let context = &mut Context::new();
    let plan = PlanOp::new(context, 2, 64, 3).expect("bounded plan");
    verify_op(&plan, context).expect("valid plan");

    let interface = op_cast::<dyn NonExecutableScheduleOp>(&plan).expect("interface present");
    assert_eq!(interface.semantic_owner(), SemanticOwner::Schedule);
    assert!(interface.is_target_neutral());
    assert!(!interface.is_executable());
}

#[test]
fn hostile_operation_shapes_and_metadata_fail_verification() {
    let context = &mut Context::new();

    let raw = Operation::new(
        context,
        PlanOp::get_concrete_op_info(),
        vec![UnitType::get(context).into()],
        vec![],
        vec![],
        0,
    );
    let wrong_type = PlanOp::from_operation(raw);
    wrong_type.set_parameters(
        context,
        ParametersAttr::new(2, 64, 2).expect("bounded parameters"),
    );
    assert!(verify_op(&wrong_type, context).is_err());

    let plan_type = PlanType::new(context, 2, 2).expect("bounded type").into();
    let missing = Operation::new(
        context,
        PlanOp::get_concrete_op_info(),
        vec![plan_type],
        vec![],
        vec![],
        0,
    );
    assert!(verify_op(&PlanOp::from_operation(missing), context).is_err());

    let mismatched = PlanOp::new(context, 2, 64, 2).expect("bounded plan");
    mismatched
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            PARAMETERS_ATTR_KEY.try_into().expect("valid key"),
            Box::new(ParametersAttr::new(3, 64, 2).expect("bounded parameters")),
        );
    assert!(verify_op(&mismatched, context).is_err());

    let extra = PlanOp::new(context, 2, 64, 2).expect("bounded plan");
    extra
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            "schedule_hostile_extra".try_into().expect("valid key"),
            Box::new(BytesAttr::new(vec![0xde, 0xad, 0xbe, 0xef])),
        );
    assert!(verify_op(&extra, context).is_err());
}
