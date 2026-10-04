use dialect_tile::{
    DIALECT_NAME, DISTRIBUTION_ATTR_KEY, DISTRIBUTION_ORDER_ATTR_KEY, DistributionAttr,
    DistributionOrderAttr, MAX_DISTRIBUTED_LANES, MAX_ELEMENTS_PER_LANE, MAX_TILE_ELEMENTS,
    MaterializeOp, TileError, register_dialect,
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
    op::{Op, verify_op},
    operation::{Operation, verify_operation},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
};

fn context() -> Context {
    let mut context = Context::new();
    register_dialect(
        &mut context,
        &DialectName::try_new(DIALECT_NAME).expect("valid dialect"),
    )
    .expect("tile registration");
    context
}

fn set_attribute(tile: &MaterializeOp, context: &Context, key: &str, value: Box<dyn Attribute>) {
    tile.get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(key.try_into().expect("valid key"), value);
}

fn assert_rejected(tile: &MaterializeOp, context: &Context) {
    assert!(verify_op(tile, context).is_err());
    assert!(tile.distribution_order(context).is_err());
    assert!(tile.logical_index(context, 0, 0).is_err());
    assert!(tile.fragment_coordinate(context, 0).is_err());
}

#[test]
fn explicit_three_lane_tables_distinguish_both_orders() {
    let distribution = DistributionAttr::new(1, 3, 2).unwrap();
    for (order, expected) in [
        (DistributionOrderAttr::Blocked, [0, 1, 2, 3, 4, 5]),
        (DistributionOrderAttr::Striped, [0, 3, 1, 4, 2, 5]),
    ] {
        let mut position = 0;
        for lane in 0..3 {
            for component in 0..2 {
                let logical = expected[position];
                assert_eq!(
                    distribution.logical_index(order, lane, component),
                    Ok(logical)
                );
                assert_eq!(
                    distribution.fragment_coordinate(order, logical),
                    Ok((lane, component))
                );
                position += 1;
            }
        }
    }
}

#[test]
fn small_domains_match_independent_enumeration_and_are_bijective() {
    for lanes in 1..=7 {
        for elements in 1..=5 {
            let distribution = DistributionAttr::new(1, lanes, elements).unwrap();
            for order in [
                DistributionOrderAttr::Blocked,
                DistributionOrderAttr::Striped,
            ] {
                // Enumerate ownership order, independently of the arithmetic implementation.
                let mut expected = Vec::new();
                match order {
                    DistributionOrderAttr::Blocked => {
                        for lane in 0..lanes {
                            for component in 0..elements {
                                expected.push((lane, component));
                            }
                        }
                    }
                    DistributionOrderAttr::Striped => {
                        for component in 0..elements {
                            for lane in 0..lanes {
                                expected.push((lane, component));
                            }
                        }
                    }
                }
                let mut visited = vec![false; expected.len()];
                for (logical, &(lane, component)) in expected.iter().enumerate() {
                    let index = u32::try_from(logical).unwrap();
                    assert_eq!(
                        distribution.fragment_coordinate(order, index),
                        Ok((lane, component))
                    );
                    let actual = distribution.logical_index(order, lane, component).unwrap();
                    assert_eq!(actual, index);
                    let slot = &mut visited[usize::try_from(actual).unwrap()];
                    assert!(!*slot);
                    *slot = true;
                }
                assert!(visited.into_iter().all(|seen| seen));
            }
        }
    }
}

#[test]
fn maximum_and_degenerate_capacities_need_only_boundary_queries() {
    let lanes = MAX_DISTRIBUTED_LANES;
    let elements = MAX_ELEMENTS_PER_LANE;
    let distribution = DistributionAttr::new(1, lanes, elements).unwrap();
    assert_eq!(distribution.total_elements(), Ok(MAX_TILE_ELEMENTS));
    for (order, probes) in [
        (
            DistributionOrderAttr::Blocked,
            [
                (0, 0, 0),
                (0, elements - 1, elements - 1),
                (lanes - 1, 0, MAX_TILE_ELEMENTS - elements),
                (lanes - 1, elements - 1, MAX_TILE_ELEMENTS - 1),
            ],
        ),
        (
            DistributionOrderAttr::Striped,
            [
                (0, 0, 0),
                (lanes - 1, 0, lanes - 1),
                (0, elements - 1, MAX_TILE_ELEMENTS - lanes),
                (lanes - 1, elements - 1, MAX_TILE_ELEMENTS - 1),
            ],
        ),
    ] {
        for (lane, component, logical) in probes {
            assert_eq!(
                distribution.logical_index(order, lane, component),
                Ok(logical)
            );
            assert_eq!(
                distribution.fragment_coordinate(order, logical),
                Ok((lane, component))
            );
        }
    }
    for order in [
        DistributionOrderAttr::Blocked,
        DistributionOrderAttr::Striped,
    ] {
        for (lanes, elements, lane, component, logical) in [
            (1, 1, 0, 0, 0),
            (
                1,
                MAX_ELEMENTS_PER_LANE,
                0,
                MAX_ELEMENTS_PER_LANE - 1,
                MAX_ELEMENTS_PER_LANE - 1,
            ),
            (
                MAX_DISTRIBUTED_LANES,
                1,
                MAX_DISTRIBUTED_LANES - 1,
                0,
                MAX_DISTRIBUTED_LANES - 1,
            ),
        ] {
            let distribution = DistributionAttr::new(1, lanes, elements).unwrap();
            assert_eq!(
                distribution.logical_index(order, lane, component),
                Ok(logical)
            );
            assert_eq!(
                distribution.fragment_coordinate(order, logical),
                Ok((lane, component))
            );
        }
    }
}

#[test]
fn out_of_bounds_coordinates_fail_without_wrapping() {
    let distribution = DistributionAttr::new(1, 3, 2).unwrap();
    for order in [
        DistributionOrderAttr::Blocked,
        DistributionOrderAttr::Striped,
    ] {
        for lane in [3, u32::MAX] {
            assert_eq!(
                distribution.logical_index(order, lane, 0),
                Err(TileError::LaneOutOfBounds { lane, lanes: 3 })
            );
        }
        for component in [2, u32::MAX] {
            assert_eq!(
                distribution.logical_index(order, 0, component),
                Err(TileError::ComponentOutOfBounds {
                    component,
                    elements_per_lane: 2,
                })
            );
        }
        for logical_index in [6, u32::MAX] {
            assert_eq!(
                distribution.fragment_coordinate(order, logical_index),
                Err(TileError::LogicalIndexOutOfBounds {
                    logical_index,
                    total_elements: 6,
                })
            );
        }
    }
}

#[test]
fn parsed_invalid_geometry_is_rejected_by_both_mapping_queries() {
    let context = &mut context();
    for (text, error) in [
        ("tile.distribution <0, 3, 2>", TileError::RankOutOfBounds(0)),
        ("tile.distribution <9, 3, 2>", TileError::RankOutOfBounds(9)),
        (
            "tile.distribution <1, 0, 2>",
            TileError::LanesOutOfBounds(0),
        ),
        (
            "tile.distribution <1, 1025, 2>",
            TileError::LanesOutOfBounds(1025),
        ),
        (
            "tile.distribution <1, 3, 0>",
            TileError::ElementsPerLaneOutOfBounds(0),
        ),
        (
            "tile.distribution <1, 3, 1025>",
            TileError::ElementsPerLaneOutOfBounds(1025),
        ),
        (
            "tile.distribution <1, 4294967295, 4294967295>",
            TileError::LanesOutOfBounds(u32::MAX),
        ),
    ] {
        let parsed = parse_from_str(<Box<dyn Attribute>>::parser(()), context, text).unwrap();
        assert!(parsed.verify(context).is_err());
        let distribution = parsed.downcast_ref::<DistributionAttr>().unwrap();
        for order in [
            DistributionOrderAttr::Blocked,
            DistributionOrderAttr::Striped,
        ] {
            assert_eq!(distribution.logical_index(order, 0, 0), Err(error));
            assert_eq!(distribution.fragment_coordinate(order, 0), Err(error));
        }
    }
    let rank_two = DistributionAttr::new(2, 3, 2).unwrap();
    for order in [
        DistributionOrderAttr::Blocked,
        DistributionOrderAttr::Striped,
    ] {
        assert_eq!(
            rank_two.logical_index(order, 0, 0),
            Err(TileError::MappingRequiresRankOne(2))
        );
        assert_eq!(
            rank_two.fragment_coordinate(order, 0),
            Err(TileError::MappingRequiresRankOne(2))
        );
    }
}

#[test]
fn materialization_queries_consume_explicit_order() {
    let context = &mut context();
    for (order, logical) in [
        (DistributionOrderAttr::Blocked, 2),
        (DistributionOrderAttr::Striped, 1),
    ] {
        let tile = MaterializeOp::new_rank_one(context, 3, 2, order).unwrap();
        verify_op(&tile, context).unwrap();
        assert_eq!(tile.distribution_order(context).unwrap(), Some(order));
        assert_eq!(tile.logical_index(context, 1, 0).unwrap(), logical);
        assert_eq!(tile.fragment_coordinate(context, logical).unwrap(), (1, 0));
        assert!(tile.logical_index(context, 3, 0).is_err());
        assert!(tile.logical_index(context, 0, 2).is_err());
        assert!(tile.fragment_coordinate(context, 6).is_err());
    }
}

#[test]
fn rank_one_materialization_rejects_invalid_capacities() {
    let context = &mut context();
    for order in [
        DistributionOrderAttr::Blocked,
        DistributionOrderAttr::Striped,
    ] {
        for (lanes, elements) in [
            (0, 1),
            (1, 0),
            (MAX_DISTRIBUTED_LANES + 1, 1),
            (1, MAX_ELEMENTS_PER_LANE + 1),
            (u32::MAX, u32::MAX),
        ] {
            assert!(MaterializeOp::new_rank_one(context, lanes, elements, order).is_err());
        }
    }
}

#[test]
fn legacy_materialization_never_invents_an_order() {
    let context = &mut context();
    for rank in [1, 2] {
        let tile = MaterializeOp::new(context, rank, 3, 2).unwrap();
        verify_op(&tile, context).unwrap();
        assert_eq!(tile.distribution_order(context).unwrap(), None);
        assert!(tile.logical_index(context, 0, 0).is_err());
        assert!(tile.fragment_coordinate(context, 0).is_err());
    }
    let tile = MaterializeOp::new_rank_one(context, 3, 2, DistributionOrderAttr::Striped).unwrap();
    tile.get_operation()
        .deref_mut(context)
        .attributes
        .0
        .remove(&DISTRIBUTION_ORDER_ATTR_KEY.try_into().unwrap());
    verify_op(&tile, context).unwrap();
    assert_eq!(tile.distribution_order(context).unwrap(), None);
    assert!(tile.logical_index(context, 0, 0).is_err());
    assert!(tile.fragment_coordinate(context, 0).is_err());
}

#[test]
fn query_revalidates_missing_or_wrong_typed_distribution() {
    let context = &mut context();
    let missing =
        MaterializeOp::new_rank_one(context, 3, 2, DistributionOrderAttr::Blocked).unwrap();
    missing
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .remove(&DISTRIBUTION_ATTR_KEY.try_into().unwrap());
    assert_rejected(&missing, context);

    let wrong = MaterializeOp::new_rank_one(context, 3, 2, DistributionOrderAttr::Blocked).unwrap();
    set_attribute(
        &wrong,
        context,
        DISTRIBUTION_ATTR_KEY,
        Box::new(BytesAttr::new(vec![1])),
    );
    assert_rejected(&wrong, context);
}

#[test]
fn wrong_typed_order_and_foreign_attributes_cannot_authorize_queries() {
    let context = &mut context();
    let wrong = MaterializeOp::new_rank_one(context, 3, 2, DistributionOrderAttr::Blocked).unwrap();
    set_attribute(
        &wrong,
        context,
        DISTRIBUTION_ORDER_ATTR_KEY,
        Box::new(BytesAttr::new(vec![1])),
    );
    assert_rejected(&wrong, context);

    let foreign =
        MaterializeOp::new_rank_one(context, 3, 2, DistributionOrderAttr::Striped).unwrap();
    set_attribute(
        &foreign,
        context,
        "tile_hostile_order",
        Box::new(DistributionOrderAttr::Blocked),
    );
    assert_rejected(&foreign, context);
}

#[test]
fn rank_two_order_and_result_extent_mismatch_reject_every_query() {
    let context = &mut context();
    let rank_two = MaterializeOp::new(context, 2, 3, 2).unwrap();
    set_attribute(
        &rank_two,
        context,
        DISTRIBUTION_ORDER_ATTR_KEY,
        Box::new(DistributionOrderAttr::Blocked),
    );
    assert_rejected(&rank_two, context);

    let mismatch =
        MaterializeOp::new_rank_one(context, 3, 2, DistributionOrderAttr::Striped).unwrap();
    mismatch.set_distribution(context, DistributionAttr::new(1, 2, 2).unwrap());
    assert_rejected(&mismatch, context);
}

#[test]
fn foreign_result_type_and_missing_results_reject_before_index_access() {
    let context = &mut context();
    let unit = UnitType::get(context).into();
    for results in [vec![unit], vec![], vec![unit, unit]] {
        let raw = Operation::new(
            context,
            MaterializeOp::get_concrete_op_info(),
            results,
            vec![],
            vec![],
            0,
        );
        let tile = MaterializeOp::from_operation(raw);
        tile.set_distribution(context, DistributionAttr::new(1, 3, 2).unwrap());
        set_attribute(
            &tile,
            context,
            DISTRIBUTION_ORDER_ATTR_KEY,
            Box::new(DistributionOrderAttr::Blocked),
        );
        assert_rejected(&tile, context);
    }
}

#[test]
fn parsed_malformed_result_geometry_rejects_order_queries() {
    let context = &mut context();
    let result = parse_from_str(
        pliron::r#type::TypeHandle::parser(()),
        context,
        "tile.distributed <0, 6>",
    )
    .unwrap();
    let raw = Operation::new(
        context,
        MaterializeOp::get_concrete_op_info(),
        vec![result],
        vec![],
        vec![],
        0,
    );
    let tile = MaterializeOp::from_operation(raw);
    tile.set_distribution(context, DistributionAttr::new(1, 3, 2).unwrap());
    set_attribute(
        &tile,
        context,
        DISTRIBUTION_ORDER_ATTR_KEY,
        Box::new(DistributionOrderAttr::Blocked),
    );
    assert_rejected(&tile, context);
}

#[test]
fn order_attributes_and_materializations_round_trip_in_fresh_contexts() {
    use dialect_tile::DistributedTileType;
    use pliron::linked_list::ContainsLinkedList;

    let context = &mut context();
    let module = ModuleOp::new(context, "ordered_tiles".try_into().unwrap());
    for order in [
        DistributionOrderAttr::Blocked,
        DistributionOrderAttr::Striped,
    ] {
        let attribute: Box<dyn Attribute> = Box::new(order);
        let text = attribute.disp(context).to_string();
        let fresh = &mut self::context();
        let parsed = parse_from_str(<Box<dyn Attribute>>::parser(()), fresh, &text).unwrap();
        parsed.verify(fresh).unwrap();
        assert_eq!(parsed.downcast_ref::<DistributionOrderAttr>(), Some(&order));

        let tile = MaterializeOp::new_rank_one(context, 3, 2, order).unwrap();
        module.append_operation(context, tile.get_operation(), 0);
    }
    let printed = module.get_operation().disp(context).to_string();
    let fresh = &mut self::context();
    let parsed = parse_from_str(Operation::top_level_parser(), fresh, &printed).unwrap();
    verify_operation(parsed, fresh).unwrap();
    let parsed_module = Operation::get_op::<ModuleOp>(parsed, fresh).unwrap();
    let children: Vec<_> = parsed_module
        .get_body(fresh, 0)
        .deref(fresh)
        .iter(fresh)
        .collect();
    let expected = [
        (
            DistributionOrderAttr::Blocked,
            [
                (0, 0, 0),
                (0, 1, 1),
                (1, 0, 2),
                (1, 1, 3),
                (2, 0, 4),
                (2, 1, 5),
            ],
        ),
        (
            DistributionOrderAttr::Striped,
            [
                (0, 0, 0),
                (0, 1, 3),
                (1, 0, 1),
                (1, 1, 4),
                (2, 0, 2),
                (2, 1, 5),
            ],
        ),
    ];
    assert_eq!(children.len(), expected.len());
    for (child, (order, coordinates)) in children.into_iter().zip(expected) {
        let tile = Operation::get_op::<MaterializeOp>(child, fresh).unwrap();
        verify_op(&tile, fresh).unwrap();
        assert_eq!(child.deref(fresh).get_num_results(), 1);
        let result_type = child.deref(fresh).get_type(0);
        let result_type = result_type.deref(fresh);
        let tile_type = result_type.downcast_ref::<DistributedTileType>().unwrap();
        assert_eq!(tile_type.rank(), 1);
        assert_eq!(tile_type.total_elements(), 6);
        let distribution = tile.distribution(fresh).unwrap();
        assert_eq!(distribution.rank(), 1);
        assert_eq!(distribution.lanes(), 3);
        assert_eq!(distribution.elements_per_lane(), 2);
        assert_eq!(distribution.total_elements().unwrap(), 6);
        assert_eq!(tile.distribution_order(fresh).unwrap(), Some(order));
        for (lane, component, logical_index) in coordinates {
            assert_eq!(
                tile.logical_index(fresh, lane, component).unwrap(),
                logical_index
            );
            assert_eq!(
                tile.fragment_coordinate(fresh, logical_index).unwrap(),
                (lane, component)
            );
        }
    }
    verify_operation(parsed, fresh).unwrap();
}
