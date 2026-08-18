use dialect_tile::{
    DIALECT_NAME, DISTRIBUTION_ATTR_KEY, DistributedTileOp, DistributedTileType, DistributionAttr,
    MAX_DISTRIBUTED_LANES, MAX_ELEMENTS_PER_LANE, MAX_TILE_ELEMENTS, MaterializeOp,
    RegistrationError, RegistrationOutcome, SemanticOwner, TileError, register_dialect,
};
use pliron::{
    attribute::Attribute,
    builtin::{op_interfaces::SingleBlockRegionInterface, ops::ModuleOp, types::UnitType},
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

fn tile_name() -> DialectName {
    DialectName::try_new(DIALECT_NAME).expect("valid dialect")
}

#[test]
fn registration_is_real_idempotent_and_collision_safe() {
    let context = &mut Context::new();
    assert_eq!(
        register_dialect(context, &tile_name()),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(context, &tile_name()),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );
    assert_eq!(
        register_dialect(
            context,
            &DialectName::try_new("kernel").expect("valid dialect")
        ),
        Err(RegistrationError::WrongDialect)
    );

    let parsed = parse_from_str(TypeHandle::parser(()), context, "tile.distributed <2, 128>")
        .expect("registered type parses");
    assert!(parsed.verify(context).is_ok());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "tile.distribution <2, 32, 4>",
    )
    .expect("registered attribute parses");
    assert!(parsed.verify(context).is_ok());

    let operation = MaterializeOp::new(context, 2, 32, 4).expect("bounded tile");
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
    let key: Identifier = "fe2o3_dialect_tile_registration_v1"
        .try_into()
        .expect("valid key");
    let hostile = context.aux_data.insert(Box::new("foreign"));
    context.aux_data_map.insert(key, hostile);
    assert_eq!(
        register_dialect(context, &tile_name()),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(context, &tile_name()),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn constructors_and_parsed_values_enforce_distribution_bounds() {
    let context = &mut Context::new();
    assert_eq!(
        DistributedTileType::new(context, 1, MAX_TILE_ELEMENTS + 1).unwrap_err(),
        TileError::TotalElementsOutOfBounds(MAX_TILE_ELEMENTS + 1)
    );
    assert_eq!(
        DistributionAttr::new(1, MAX_DISTRIBUTED_LANES + 1, 1).unwrap_err(),
        TileError::LanesOutOfBounds(MAX_DISTRIBUTED_LANES + 1)
    );
    assert_eq!(
        DistributionAttr::new(1, 1, MAX_ELEMENTS_PER_LANE + 1).unwrap_err(),
        TileError::ElementsPerLaneOutOfBounds(MAX_ELEMENTS_PER_LANE + 1)
    );

    let parsed = parse_from_str(TypeHandle::parser(()), context, "tile.distributed <0, 1>")
        .expect("syntax is valid before semantic verification");
    assert!(parsed.verify(context).is_err());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "tile.distribution <1, 0, 1>",
    )
    .expect("bounded scalar syntax parses");
    assert!(parsed.verify(context).is_err());
}

#[test]
fn interface_marks_materialization_target_neutral() {
    let context = &mut Context::new();
    let tile = MaterializeOp::new(context, 2, 32, 4).expect("bounded tile");
    verify_op(&tile, context).expect("valid tile");

    let interface = op_cast::<dyn DistributedTileOp>(&tile).expect("interface present");
    assert_eq!(interface.semantic_owner(), SemanticOwner::Tile);
    assert!(interface.is_target_neutral());
}

#[test]
fn hostile_operation_shapes_and_metadata_fail_verification() {
    let context = &mut Context::new();

    let unit_type = UnitType::get(context).into();
    let raw = Operation::new(
        context,
        MaterializeOp::get_concrete_op_info(),
        vec![unit_type],
        vec![],
        vec![],
        0,
    );
    let wrong_type = MaterializeOp::from_operation(raw);
    wrong_type.set_distribution(
        context,
        DistributionAttr::new(2, 32, 4).expect("bounded distribution"),
    );
    assert!(verify_op(&wrong_type, context).is_err());

    let tile_type = DistributedTileType::new(context, 2, 128)
        .expect("bounded type")
        .into();
    let missing = Operation::new(
        context,
        MaterializeOp::get_concrete_op_info(),
        vec![tile_type],
        vec![],
        vec![],
        0,
    );
    assert!(verify_op(&MaterializeOp::from_operation(missing), context).is_err());

    let mismatched = MaterializeOp::new(context, 2, 32, 4).expect("bounded tile");
    mismatched
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            DISTRIBUTION_ATTR_KEY.try_into().expect("valid key"),
            Box::new(DistributionAttr::new(3, 32, 4).expect("bounded distribution")),
        );
    assert!(verify_op(&mismatched, context).is_err());
}
