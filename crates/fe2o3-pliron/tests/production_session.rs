use fe2o3_pliron::{
    DialectRegistration, HARD_MAX_NAME_BYTES, HARD_MAX_PRODUCTION_CONSTRUCTIONS,
    ProductionConstructionV1, ProductionPlironSessionV1, ProductionSessionErrorV1,
    ProductionSessionLimitErrorV1, ProductionSessionLimitsV1, ShellLimits,
};

fn limits(max_constructions: usize) -> ProductionSessionLimitsV1 {
    ProductionSessionLimitsV1::new(ShellLimits::default(), max_constructions)
        .expect("valid production limits")
}

fn session(max_constructions: usize) -> ProductionPlironSessionV1 {
    ProductionPlironSessionV1::new(
        limits(max_constructions),
        std::iter::empty::<DialectRegistration>(),
    )
    .expect("fresh production session")
}

fn recipe(name: &str) -> ProductionConstructionV1 {
    ProductionConstructionV1::builtin_module(name).expect("valid recipe")
}

#[test]
fn closed_session_constructs_and_inspects_a_verified_builtin_root() {
    let mut session = session(2);
    let registered = session
        .register_construction(recipe("root"))
        .expect("registered construction");
    let (stage, root) = session
        .construct_registered(registered)
        .expect("constructed graph");

    let shape = session.root_shape(&stage, &root).expect("root shape");
    assert_eq!(shape.operand_count(), 0);
    assert_eq!(shape.result_count(), 0);
    assert_eq!(shape.region_count(), 1);
    assert_eq!(shape.block_count(), 1);
    assert_eq!(shape.child_operation_count(), 0);
    assert!(!session.is_poisoned());
}

#[test]
fn construction_registration_is_monotonically_bounded() {
    let mut session = session(1);
    let first = session
        .register_construction(recipe("first"))
        .expect("first registration");
    assert!(matches!(
        session.register_construction(recipe("second")),
        Err(ProductionSessionErrorV1::ConstructionLimitExceeded)
    ));
    assert!(!session.is_poisoned());

    let (stage, root) = session
        .construct_registered(first)
        .expect("existing registration remains usable");
    assert!(session.root_shape(&stage, &root).is_ok());
    assert!(matches!(
        session.register_construction(recipe("third")),
        Err(ProductionSessionErrorV1::ConstructionLimitExceeded)
    ));
}

#[test]
fn duplicate_registration_is_rejected_without_consuming_budget() {
    let mut session = session(2);
    let first = session
        .register_construction(recipe("same"))
        .expect("first registration");
    assert!(matches!(
        session.register_construction(recipe("same")),
        Err(ProductionSessionErrorV1::DuplicateConstructionName(
            name
        ))
            if name == "same"
    ));
    let second = session
        .register_construction(recipe("different"))
        .expect("duplicate did not consume the budget");

    let (first_stage, first_root) = session.construct_registered(first).expect("first graph");
    let (second_stage, second_root) = session.construct_registered(second).expect("second graph");
    assert!(session.root_shape(&first_stage, &first_root).is_ok());
    assert!(session.root_shape(&second_stage, &second_root).is_ok());
}

#[test]
fn foreign_registered_stage_fails_before_construction() {
    let mut owner = session(1);
    let mut foreign = session(1);
    let owner_stage = owner
        .register_construction(recipe("owner"))
        .expect("owner registration");
    let foreign_stage = foreign
        .register_construction(recipe("foreign"))
        .expect("foreign registration");

    assert!(matches!(
        foreign.construct_registered(owner_stage),
        Err(ProductionSessionErrorV1::ForeignSession)
    ));
    assert!(!foreign.is_poisoned());
    let (stage, root) = foreign
        .construct_registered(foreign_stage)
        .expect("foreign session remains usable");
    assert!(foreign.root_shape(&stage, &root).is_ok());
}

#[test]
fn foreign_stage_and_root_handles_fail_before_pointer_access() {
    let mut first = session(1);
    let mut second = session(1);
    let first_registered = first
        .register_construction(recipe("first"))
        .expect("first registration");
    let second_registered = second
        .register_construction(recipe("second"))
        .expect("second registration");
    let (first_stage, first_root) = first
        .construct_registered(first_registered)
        .expect("first graph");
    let (second_stage, second_root) = second
        .construct_registered(second_registered)
        .expect("second graph");

    assert!(matches!(
        second.root_shape(&first_stage, &first_root),
        Err(ProductionSessionErrorV1::ForeignSession)
    ));
    assert!(matches!(
        second.root_shape(&second_stage, &first_root),
        Err(ProductionSessionErrorV1::ForeignSession)
    ));
    assert!(!second.is_poisoned());
    assert!(second.root_shape(&second_stage, &second_root).is_ok());
}

#[test]
fn same_session_root_stage_substitution_fails_closed() {
    let mut session = session(2);
    let first = session
        .register_construction(recipe("first"))
        .expect("first registration");
    let second = session
        .register_construction(recipe("second"))
        .expect("second registration");
    let (first_stage, first_root) = session.construct_registered(first).expect("first graph");
    let (second_stage, second_root) = session.construct_registered(second).expect("second graph");

    assert_eq!(
        session.root_shape(&first_stage, &second_root),
        Err(ProductionSessionErrorV1::StageRootMismatch)
    );
    assert_eq!(
        session.root_shape(&second_stage, &first_root),
        Err(ProductionSessionErrorV1::StageRootMismatch)
    );
    assert!(!session.is_poisoned());
    assert!(session.root_shape(&first_stage, &first_root).is_ok());
    assert!(session.root_shape(&second_stage, &second_root).is_ok());
}

#[test]
fn handles_do_not_render_owner_or_registry_identities() {
    let mut session = session(1);
    let registered = session
        .register_construction(recipe("root"))
        .expect("registration");
    assert_eq!(format!("{registered:?}"), "ProductionStageHandleV1 { .. }");
    let (stage, root) = session
        .construct_registered(registered)
        .expect("constructed graph");
    assert_eq!(format!("{stage:?}"), "ProductionStageHandleV1 { .. }");
    assert_eq!(format!("{root:?}"), "ProductionRootHandleV1 { .. }");
}

#[test]
fn construction_limits_reject_zero_and_values_above_the_hard_cap() {
    assert_eq!(
        ProductionSessionLimitsV1::new(ShellLimits::default(), 0),
        Err(ProductionSessionLimitErrorV1::ZeroConstructions)
    );
    assert_eq!(
        ProductionSessionLimitsV1::new(
            ShellLimits::default(),
            HARD_MAX_PRODUCTION_CONSTRUCTIONS + 1
        ),
        Err(ProductionSessionLimitErrorV1::TooManyConstructions)
    );
}

#[test]
fn construction_recipe_names_are_validated_before_registration() {
    assert!(ProductionConstructionV1::builtin_module("").is_err());
    assert!(ProductionConstructionV1::builtin_module("Uppercase").is_err());
    assert!(
        ProductionConstructionV1::builtin_module(&"m".repeat(HARD_MAX_NAME_BYTES + 1)).is_err()
    );

    let mut session = session(1);
    let registered = session
        .register_construction(recipe("valid_root"))
        .expect("invalid recipes did not consume session budget");
    assert!(session.construct_registered(registered).is_ok());
}
