use super::*;
use crate::{SemanticLogicalArgumentErrorV1, SemanticSourceArgumentBindingV1};

const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

fn unit_layout() -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        0,
        1,
        SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::memory(true),
        None,
        false,
        None,
        1,
        0,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap()
}

fn request(fields: &[SemanticTypeIdV1], expanded: bool) -> InertSemanticMirRequestV1 {
    let mut request = minimal_request();
    let direct = request.functions[0].abi.return_value.clone();
    let mut size = 0;
    let offsets = fields
        .iter()
        .map(|field| {
            let offset = size;
            if *field == SCALAR {
                size += 4;
            }
            offset
        })
        .collect();
    let (layout, shape) = if fields.is_empty() {
        (unit_layout(), SemanticTypeShapeV1::Unit)
    } else {
        (
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                if size == 0 { 1 } else { 4 },
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields.to_vec()).unwrap()),
        )
    };
    request.types = vec![
        request.types[0].clone(),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(20)),
            SemanticLayoutIdentityV1(identity(20)),
            layout,
            shape,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(21)),
            SemanticLayoutIdentityV1(identity(21)),
            unit_layout(),
            SemanticTypeShapeV1::Unit,
        ),
    ]
    .into_boxed_slice();
    let mut arguments = vec![SemanticAbiArgumentV1::source(direct.clone())];
    arguments.extend(fields.iter().enumerate().map(|(index, ty)| {
        SemanticAbiArgumentV1::rust_call_tuple_field(
            index as u32,
            if *ty == UNIT {
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore)
            } else {
                direct.clone()
            },
        )
    }));
    request.functions[0].abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1(identity(3)),
        SemanticLayoutIdentityV1(identity(4)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::RustCall,
        false,
        false,
        1,
        vec![SCALAR, TUPLE],
        SCALAR,
        arguments,
        direct,
    )
    .unwrap();
    let mut locals = request.functions[0].locals.to_vec();
    if expanded {
        locals.extend(fields.iter().enumerate().map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(7 + index as u8)),
                *ty,
                SemanticLocalRoleV1::RustCallTupleField {
                    argument: 1,
                    field: index as u32,
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
        }));
    } else {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(7)),
            TUPLE,
            SemanticLocalRoleV1::Argument(1),
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    if !fields.contains(&UNIT) {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1(identity(99)),
            UNIT,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    request.functions[0].locals = locals.into_boxed_slice();
    request
}

#[test]
fn packed_and_expanded_rust_call_locals_round_trip_with_empty_and_ignored_fields() {
    let limits = SemanticMirLimitsV1::default();
    for fields in [
        vec![],
        vec![SCALAR],
        vec![SCALAR, SCALAR],
        vec![UNIT],
        vec![SCALAR, UNIT, SCALAR],
    ] {
        for expanded in [false, true] {
            let request = request(&fields, expanded);
            let admitted = request
                .clone()
                .admit_current_production(limits)
                .unwrap_or_else(|error| {
                    panic!("fields {fields:?}, expanded {expanded}: {error:?}")
                });
            assert_eq!(
                AdmittedInertSemanticMirV1::decode_current_production_canonical(
                    admitted.canonical_encoding(),
                    limits,
                )
                .unwrap()
                .canonical_encoding(),
                admitted.canonical_encoding()
            );
            if expanded || fields.is_empty() {
                assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V28);
                assert!(matches!(
                    request.admit_exact_v15(limits),
                    Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                        required: SemanticMirWireVersionV1::V28,
                        ..
                    })
                ));
                let mut downgraded = admitted.canonical_encoding().to_vec();
                downgraded[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&15_u16.to_le_bytes());
                assert!(
                    AdmittedInertSemanticMirV1::decode_current_production_canonical(
                        &downgraded,
                        limits
                    )
                    .is_err()
                );
            } else {
                let v15 = request.clone().admit_exact_v15(limits).unwrap();
                let v28 = request.admit_exact_v28(limits).unwrap();
                // The V28 envelope is new; every byte of the old model is unchanged.
                assert_eq!(
                    &v15.canonical_encoding()[MAGIC.len() + 2..],
                    &v28.canonical_encoding()[MAGIC.len() + 2..]
                );
            }
        }
    }
}

#[test]
fn expanded_rust_call_local_roles_require_exact_unique_complete_source_fields() {
    let baseline = request(&[SCALAR, SCALAR], true);
    let mut mutations = Vec::new();
    for role in [
        SemanticLocalRoleV1::RustCallTupleField {
            argument: 0,
            field: 1,
        },
        SemanticLocalRoleV1::RustCallTupleField {
            argument: 1,
            field: 0,
        },
        SemanticLocalRoleV1::RustCallTupleField {
            argument: 1,
            field: 2,
        },
        SemanticLocalRoleV1::Temporary,
    ] {
        let mut changed = baseline.clone();
        changed.functions[0].locals[3].role = role;
        mutations.push(changed);
    }
    let mut wrong_type = baseline.clone();
    wrong_type.functions[0].locals[3].ty = UNIT;
    mutations.push(wrong_type);
    let mut mixed = baseline.clone();
    let mut locals = mixed.functions[0].locals.to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(100)),
        TUPLE,
        SemanticLocalRoleV1::Argument(1),
        SemanticSourceProvenanceV1::unavailable(),
    ));
    mixed.functions[0].locals = locals.into_boxed_slice();
    mutations.push(mixed);
    let mut not_rust_call = minimal_request();
    not_rust_call.functions[0].locals[1].role = SemanticLocalRoleV1::RustCallTupleField {
        argument: 0,
        field: 0,
    };
    mutations.push(not_rust_call);
    let mut ignored_missing = request(&[SCALAR, UNIT], true);
    ignored_missing.functions[0].locals[3].role = SemanticLocalRoleV1::Temporary;
    mutations.push(ignored_missing);
    for (index, changed) in mutations.into_iter().enumerate() {
        let result = changed.admit_current_production(SemanticMirLimitsV1::default());
        assert!(
            matches!(result, Err(SemanticMirErrorV1::InvalidLocalRoles { .. })),
            "mutation {index}: {result:?}"
        );
    }
    let mut reordered = baseline;
    reordered.functions[0].locals[2].role = SemanticLocalRoleV1::RustCallTupleField {
        argument: 1,
        field: 1,
    };
    reordered.functions[0].locals[3].role = SemanticLocalRoleV1::RustCallTupleField {
        argument: 1,
        field: 0,
    };
    reordered
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
}

#[test]
fn rust_call_fields_are_entry_arguments_not_whole_arguments() {
    let field = SemanticLocalRoleV1::RustCallTupleField {
        argument: 1,
        field: 0,
    };
    assert!(field.is_entry_argument());
    assert_ne!(field, SemanticLocalRoleV1::Argument(1));
    assert!(!SemanticLocalRoleV1::Temporary.is_entry_argument());
    assert!(!SemanticLocalRoleV1::Return.is_entry_argument());
}

#[test]
fn empty_rust_call_preserves_old_tuple_admission_without_widening_it_to_unit() {
    let limits = SemanticMirLimitsV1::default();
    for expanded in [false, true] {
        let unit = request(&[], expanded);
        assert_eq!(
            unit.clone()
                .admit_current_production(limits)
                .unwrap()
                .wire_version(),
            SemanticMirWireVersionV1::V28
        );
        for version in 2..=15 {
            assert!(matches!(
                unit.clone().admit_for_wire_version(
                    SemanticMirWireVersionV1::from_u16(version).unwrap(),
                    limits
                ),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    required: SemanticMirWireVersionV1::V28,
                    ..
                })
            ));
        }
        let mut tuple = unit;
        tuple.types[TUPLE.index() as usize].shape =
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![]).unwrap());
        tuple.types[TUPLE.index() as usize].layout = SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap();
        let admitted = tuple.admit_current_production(limits).unwrap();
        assert_eq!(
            admitted.wire_version(),
            if expanded {
                SemanticMirWireVersionV1::V28
            } else {
                SemanticMirWireVersionV1::V5
            }
        );
    }
}

#[test]
fn logical_argument_map_preserves_outer_tuple_identity_and_ignored_rows() {
    for fields in [
        vec![],
        vec![SCALAR],
        vec![SCALAR, SCALAR],
        vec![UNIT, UNIT],
        vec![UNIT, SCALAR, UNIT, SCALAR],
    ] {
        for expanded in [false, true] {
            let mut request = request(&fields, expanded);
            request.functions[0].abi = request.functions[0]
                .abi
                .clone()
                .with_source_argument_ownership(vec![
                    SemanticSourceArgumentOwnershipV1::Unspecified,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ])
                .unwrap();
            // Entry local order must not be mistaken for field/ABI order.
            request.functions[0].locals[2..].reverse();
            for (index, local) in request.functions[0].locals.iter_mut().enumerate() {
                local.identity = SemanticLocalIdentityV1(identity(5 + index as u8));
            }
            let admitted = request
                .admit_current_production(SemanticMirLimitsV1::default())
                .unwrap();
            let bytes = admitted.canonical_encoding().to_vec();
            let map = admitted
                .logical_arguments_v1(SemanticFunctionIdV1::from_index(0))
                .unwrap();
            let sources: Vec<_> = map.source_arguments().collect();
            assert_eq!(sources.len(), 2);
            assert_eq!(sources[0].ordinal(), 0);
            assert_eq!(sources[0].ty(), SCALAR);
            assert_eq!(sources[1].ordinal(), 1);
            assert_eq!(sources[1].ty(), TUPLE);
            assert_eq!(
                sources[1].source_ownership(),
                SemanticSourceArgumentOwnershipV1::ByValue
            );
            match sources[1].binding() {
                SemanticSourceArgumentBindingV1::Whole(local) => {
                    assert!(!expanded);
                    assert_eq!(
                        admitted.functions()[0].locals()[local.index() as usize].ty(),
                        TUPLE
                    );
                }
                SemanticSourceArgumentBindingV1::ExpandedTuple(locals) => {
                    assert!(expanded);
                    assert_eq!(locals.len(), fields.len());
                    for (field, local) in locals.iter().enumerate() {
                        assert_eq!(
                            admitted.functions()[0].locals()[local.index() as usize].role(),
                            SemanticLocalRoleV1::RustCallTupleField {
                                argument: 1,
                                field: field as u32
                            }
                        );
                    }
                }
            }
            let adjusted: Vec<_> = map.adjusted_arguments().collect();
            assert_eq!(adjusted.len(), fields.len() + 1);
            for (ordinal, mapped) in adjusted.iter().copied().enumerate() {
                assert_eq!(mapped.ordinal(), ordinal as u32);
                assert!(std::ptr::eq(
                    mapped.abi(),
                    &admitted.functions()[0].abi().adjusted_arguments()[ordinal]
                ));
                if ordinal == 0 {
                    assert_eq!(mapped.source_argument(), 0);
                    assert_eq!(mapped.tuple_field(), None);
                    assert_eq!(mapped.local_field(), None);
                    assert_eq!(
                        mapped.source_ownership(),
                        SemanticSourceArgumentOwnershipV1::Unspecified
                    );
                } else {
                    let field = ordinal as u32 - 1;
                    assert_eq!(mapped.source_argument(), 1);
                    assert_eq!(mapped.tuple_field(), Some(field));
                    assert_eq!(mapped.local_field(), (!expanded).then_some(field));
                    assert_eq!(mapped.abi().ty(), fields[field as usize]);
                    assert_eq!(
                        matches!(mapped.abi().mode(), SemanticAbiPassModeV1::Ignore),
                        fields[field as usize] == UNIT
                    );
                    assert_eq!(
                        mapped.source_ownership(),
                        SemanticSourceArgumentOwnershipV1::ByValue
                    );
                }
            }
            assert_eq!(admitted.canonical_encoding(), bytes);
        }
    }
}

#[test]
fn logical_argument_map_uses_roles_not_local_positions() {
    let mut request = minimal_request();
    request.functions[0].locals.swap(0, 1);
    for (index, local) in request.functions[0].locals.iter_mut().enumerate() {
        local.identity = SemanticLocalIdentityV1(identity(5 + index as u8));
    }
    let admitted = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let map = admitted
        .logical_arguments_v1(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let source = map.source_arguments().next().unwrap();
    assert_eq!(source.ordinal(), 0);
    assert_eq!(
        source.binding(),
        SemanticSourceArgumentBindingV1::Whole(SemanticLocalIdV1::from_index(0))
    );
    let adjusted = map.adjusted_arguments().next().unwrap();
    assert_eq!(adjusted.local(), SemanticLocalIdV1::from_index(0));
    assert_eq!(adjusted.source_argument(), 0);
    assert_eq!(adjusted.tuple_field(), None);
    assert!(matches!(
        admitted.logical_arguments_v1(SemanticFunctionIdV1::from_index(1)),
        Err(SemanticLogicalArgumentErrorV1::UnknownFunction)
    ));
}
