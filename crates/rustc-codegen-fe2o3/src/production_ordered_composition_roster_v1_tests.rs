//! Synthetic state-machine/transport controls only; not live rustc authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantV1, SemanticControlFlowEdgeV1, SemanticFunctionIdentityV1, SemanticPlaceV1,
    SemanticScalarValueV1,
};

fn place(local: u32, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(ty),
    )
    .unwrap()
}
fn operand(value: Operand) -> SemanticOperandV1 {
    match value {
        Operand::Copy { local, ty } => SemanticOperandV1::Copy(place(local.index(), ty.index())),
        Operand::Move { local, ty } => SemanticOperandV1::Move(place(local.index(), ty.index())),
        Operand::Constant { bits, bytes, ty } => {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::Scalar(
                    SemanticScalarValueV1::new(u128::from(bits), bytes).unwrap(),
                ),
            ))
        }
    }
}
fn row() -> Row<u8> {
    // Semantic IDs intentionally differ from raw block/local coordinates.
    let ty = SemanticTypeIdV1::from_index(4);
    let u8_ty = SemanticTypeIdV1::from_index(2);
    let mut arguments = [None; 8];
    arguments[0] = Some(Operand::Copy {
        local: SemanticLocalIdV1::from_index(9),
        ty,
    });
    arguments[1] = Some(Operand::Move {
        local: SemanticLocalIdV1::from_index(2),
        ty,
    });
    arguments[2] = Some(Operand::Constant {
        bits: 17,
        bytes: 4,
        ty,
    });
    for (index, value) in [32, 33, 34, 35, 36].into_iter().enumerate() {
        arguments[index + 3] = Some(Operand::Constant {
            bits: value,
            bytes: 1,
            ty: u8_ty,
        });
    }
    Row {
        site: Site {
            function: SemanticFunctionIdV1::from_index(1),
            raw_block: 7,
            block: SemanticBlockIdV1::from_index(3),
            block_identity: SemanticBlockIdentityV1::from_sha256([1; 32]),
        },
        binding: Binding {
            caller: 11,
            callee: 12,
            callable: SemanticCallableIdV1::from_index(3),
            transport: Transport {
                arguments,
                count: 8,
                destination: SemanticLocalIdV1::from_index(6),
                result_type: ty,
                target: SemanticBlockIdV1::from_index(1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        },
        kind: Kind::Marker {
            actual: Marker {
                program: SemanticGfx942U32ProgramV32::from_packed(1, [8, 0, 0, 0]).unwrap(),
                registers: SemanticGfx942OrderedProgramRegistersV32::new(32, 33, [34, 35, 36])
                    .unwrap(),
            },
            source: SemanticOrderedProgramSourceV32::new(
                [1; 32],
                SemanticFunctionIdentityV1::from_sha256([2; 32]),
                [3; 32],
                [4; 32],
            )
            .unwrap(),
        },
        consumed: false,
    }
}
fn args(row: Row<u8>) -> Vec<SemanticOperandV1> {
    row.binding.transport.arguments[..usize::from(row.binding.transport.count)]
        .iter()
        .map(|value| operand(value.unwrap()))
        .collect()
}
fn destination(row: Row<u8>) -> SemanticCallDestinationV1 {
    let t = row.binding.transport;
    SemanticCallDestinationV1::new(
        place(t.destination.index(), t.result_type.index()),
        SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, t.target),
    )
}
fn observation<'a>(
    row: Row<u8>,
    arguments: &'a [SemanticOperandV1],
    destination: &'a SemanticCallDestinationV1,
) -> Observation<'a, u8> {
    Observation {
        site: row.site,
        caller: row.binding.caller,
        callee: row.binding.callee,
        callable: row.binding.callable,
        arguments,
        destination: Some(destination),
        unwind: row.binding.transport.unwind,
        marker: match row.kind {
            Kind::Marker { actual, .. } => Some(actual),
            Kind::Defined => None,
        },
    }
}

#[test]
fn exact_permuted_semantic_coordinates_are_consumed_once() {
    let row = row();
    let a = args(row);
    let d = destination(row);
    let mut roster = Roster::new(2).unwrap();
    roster.insert(row).unwrap();
    assert!(roster.require_drained().is_err());
    let source = roster.take(observation(row, &a, &d)).unwrap();
    assert!(source.is_some());
    roster.require_drained().unwrap();
    assert!(roster.take(observation(row, &a, &d)).is_err());
}

#[test]
fn every_source_call_join_refuses_substitution_without_consuming() {
    for changed in 0..12 {
        let row = row();
        let a = args(row);
        let d = destination(row);
        let mut observed = observation(row, &a, &d);
        match changed {
            0 => observed.site.function = SemanticFunctionIdV1::from_index(0),
            1 => observed.site.raw_block += 1,
            2 => observed.site.block = SemanticBlockIdV1::from_index(row.site.raw_block),
            3 => observed.site.block_identity = SemanticBlockIdentityV1::from_sha256([2; 32]),
            4 => observed.caller += 1,
            5 => observed.callee += 1,
            6 => observed.callable = SemanticCallableIdV1::from_index(4),
            7 => observed.marker = None,
            8 => {
                observed.marker.as_mut().unwrap().program =
                    SemanticGfx942U32ProgramV32::from_packed(1, [24, 0, 0, 0]).unwrap()
            }
            9 => {
                observed.marker.as_mut().unwrap().registers =
                    SemanticGfx942OrderedProgramRegistersV32::new(31, 33, [34, 35, 36]).unwrap()
            }
            10 => observed.unwind = SemanticUnwindActionV1::Continue,
            11 => observed.destination = None,
            _ => unreachable!(),
        }
        let mut roster = Roster::new(2).unwrap();
        roster.insert(row).unwrap();
        assert!(roster.take(observed).is_err(), "join {changed}");
        assert!(roster.require_drained().is_err());
        assert!(roster.take(observation(row, &a, &d)).unwrap().is_some());
    }
}

#[test]
fn operand_and_destination_transport_refuses_exactly_not_by_value_only() {
    for changed in 0..12 {
        let row = row();
        let mut a = args(row);
        let mut d = destination(row);
        match changed {
            0 => a[0] = SemanticOperandV1::Move(place(9, 4)),
            1 => a[0] = SemanticOperandV1::Copy(place(1, 4)), // raw-like ID, not semantic 9
            2 => a[0] = SemanticOperandV1::Copy(place(9, 3)),
            3 => a[1] = SemanticOperandV1::Copy(place(2, 4)),
            4 => {
                a[2] = operand(Operand::Constant {
                    bits: 18,
                    bytes: 4,
                    ty: SemanticTypeIdV1::from_index(4),
                })
            }
            5 => {
                a[3] = operand(Operand::Constant {
                    bits: 32,
                    bytes: 4,
                    ty: SemanticTypeIdV1::from_index(2),
                })
            }
            6 => {
                a.pop();
            }
            7 => a.push(a[0].clone()),
            8 => d = SemanticCallDestinationV1::new(place(0, 4), d.edge()),
            9 => d = SemanticCallDestinationV1::new(place(6, 3), d.edge()),
            10 => {
                d = SemanticCallDestinationV1::new(
                    d.place().clone(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(8),
                    ),
                )
            }
            11 => {
                d = SemanticCallDestinationV1::new(
                    d.place().clone(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallUnwind,
                        d.edge().target(),
                    ),
                )
            }
            _ => unreachable!(),
        }
        let mut roster = Roster::new(2).unwrap();
        roster.insert(row).unwrap();
        assert!(
            roster.take(observation(row, &a, &d)).is_err(),
            "transport {changed}"
        );
        assert!(!roster.rows[0].consumed);
    }
}

#[test]
fn helper_definition_and_two_actual_calls_drain_three_rows_not_two_markers() {
    let marker = row();
    let mut first = marker;
    first.site.function = SemanticFunctionIdV1::from_index(0);
    first.binding.callable = SemanticCallableIdV1::from_index(1);
    first.binding.caller = 10;
    first.binding.callee = 11;
    first.binding.transport.count = 3;
    first.binding.transport.arguments[3..].fill(None);
    first.kind = Kind::Defined;
    let mut second = first;
    second.site.raw_block += 1;
    second.site.block = SemanticBlockIdV1::from_index(5);
    second.site.block_identity = SemanticBlockIdentityV1::from_sha256([7; 32]);
    let mut roster = Roster::new(2).unwrap();
    for row in [marker, first, second] {
        roster.insert(row).unwrap();
    }
    for row in [second, marker, first] {
        let a = args(row);
        let d = destination(row);
        assert_eq!(
            roster.take(observation(row, &a, &d)).unwrap().is_some(),
            matches!(row.kind, Kind::Marker { .. })
        );
    }
    assert_eq!((roster.markers, roster.calls), (1, 2));
    roster.require_drained().unwrap();
}

#[test]
fn only_absent_nonmarker_terminal_is_inert() {
    let row = row();
    let a = args(row);
    let d = destination(row);
    let mut roster = Roster::new(2).unwrap();
    assert!(roster.take(observation(row, &a, &d)).is_err());
    let mut absent = observation(row, &a, &d);
    absent.marker = None;
    absent.callable = SemanticCallableIdV1::from_index(1);
    assert!(roster.take(absent).is_err());
    let mut ordinary = observation(row, &a, &d);
    ordinary.marker = None;
    assert_eq!(roster.take(ordinary), Ok(None));
    assert!(roster.require_drained().is_err());
}

#[test]
fn roster_capacity_duplicate_and_preconsumed_controls() {
    assert!(Roster::<u8>::new(0).is_err());
    assert!(Roster::<u8>::new(4).is_err());
    let mut roster = Roster::new(2).unwrap();
    let original = row();
    roster.insert(original).unwrap();
    let mut duplicate = original;
    duplicate.site.block = SemanticBlockIdV1::from_index(99);
    assert!(roster.insert(duplicate).is_err());
    let mut used = original;
    used.site.raw_block += 1;
    used.consumed = true;
    assert!(roster.insert(used).is_err());
    for raw in 8..15 {
        let mut next = original;
        next.site.raw_block = raw;
        roster.insert(next).unwrap();
    }
    let mut ninth = original;
    ninth.site.raw_block = 15;
    assert!(roster.insert(ninth).is_err());
    assert_eq!(roster.rows.len(), MAX_MARKERS);
    assert!(roster.rows.capacity() <= MAX_ROWS);
}

#[test]
fn census_distinguishes_two_monos_and_expanded_occurrences() {
    assert!(validate_census(1, 3, &[1, 1, 1], &[2, 3, 4], &[(1, 0), (1, 2)]).is_ok());
    assert!(validate_census(0, 2, &[1, 1, 0], &[16, 16, 0], &[(0, 1); 7]).is_ok());
    assert!(validate_census(0, 2, &[1, 1, 0], &[16, 16, 0], &[(0, 1); 8]).is_err());
    // A scalar-only helper must remain reachable/retained even with no marker.
    assert!(validate_census(0, 2, &[1, 0, 0], &[1, 0, 0], &[(0, 1)]).is_ok());
}

#[test]
fn census_rejects_nested_recursive_unreachable_foreign_and_overflow() {
    for calls in [vec![(1, 2)], vec![(0, 0)], vec![(0, 3)], vec![]] {
        assert!(validate_census(0, 3, &[1, 1, 1], &[1, 1, 1], &calls).is_err());
    }
    assert!(validate_census(0, 2, &[0, 1, 1], &[0, 1, 1], &[(0, 1)]).is_err());
    assert!(validate_census(0, 2, &[0, 0, 0], &[0, 0, 0], &[(0, 1)]).is_err());
    assert!(validate_census(0, 1, &[9, 0, 0], &[9, 0, 0], &[]).is_err());
    assert!(validate_census(0, 1, &[1, 0, 0], &[129, 0, 0], &[]).is_err());
    assert!(validate_census(0, 2, &[1, 1, 0], &[usize::MAX, 1, 0], &[(0, 1)]).is_err());
}
