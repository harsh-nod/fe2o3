fn array_unsize_types(length: u64, zero_sized: bool, mutable: bool) -> Vec<SemanticTypeDeclV1> {
    let id = SemanticTypeIdV1::from_index;
    let mutability = if mutable {
        SemanticMutabilityV1::Mutable
    } else {
        SemanticMutabilityV1::Immutable
    };
    let element_size = if zero_sized { 0 } else { 4 };
    let pointer = |pointee, metadata, size| {
        test_type(
            74 + pointee as u8,
            SemanticTypeLayoutV1::new(Some(size), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    id(pointee),
                    SemanticPointerKindV1::Reference,
                    mutability,
                    0,
                    64,
                    metadata,
                )
                .unwrap(),
            ),
        )
    };
    vec![
        test_type(
            70,
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        if zero_sized {
            test_type(
                71,
                SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
                SemanticTypeShapeV1::Unit,
            )
        } else {
            full_range_scalar_type(
                71,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            )
        },
        test_type(
            72,
            SemanticTypeLayoutV1::new(Some(length * element_size), 4).unwrap(),
            SemanticTypeShapeV1::Array {
                element: id(1),
                length,
            },
        ),
        test_type(
            73,
            SemanticTypeLayoutV1::new(None, 4).unwrap(),
            SemanticTypeShapeV1::Slice { element: id(1) },
        ),
        pointer(2, SemanticPointerMetadataV1::None, 8),
        pointer(3, SemanticPointerMetadataV1::SliceLength, 16),
    ]
}

#[test]
fn array_reference_unsize_preserves_exact_element_count_including_zero_sized_arrays() {
    for mutable in [false, true] {
        for zero_sized in [false, true] {
            for length in [0, 1, 4, 16] {
                let types = array_unsize_types(length, zero_sized, mutable);
                assert_eq!(
                    array_reference_unsize_length_v1(
                        &types,
                        SemanticTypeIdV1(4),
                        SemanticTypeIdV1(5)
                    ),
                    Some(length)
                );
            }
        }
    }
}

#[test]
fn array_reference_unsize_rejects_changed_pointer_custody_and_metadata() {
    for index in [4, 5] {
        for mutation in 0..6 {
            let mut types = array_unsize_types(4, false, false);
            let SemanticTypeShapeV1::Pointer(pointer) = &mut types[index].shape else {
                unreachable!()
            };
            match mutation {
                0 => pointer.kind = SemanticPointerKindV1::Raw,
                1 => pointer.mutability = SemanticMutabilityV1::Mutable,
                2 => pointer.address_space = 3,
                3 => pointer.pointer_width_bits = 32,
                4 => pointer.metadata = SemanticPointerMetadataV1::VTable,
                5 => {
                    pointer.metadata = if index == 4 {
                        SemanticPointerMetadataV1::SliceLength
                    } else {
                        SemanticPointerMetadataV1::None
                    }
                }
                _ => unreachable!(),
            }
            assert_eq!(
                array_reference_unsize_length_v1(&types, SemanticTypeIdV1(4), SemanticTypeIdV1(5)),
                None,
                "pointer {index}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn array_reference_unsize_rejects_substituted_pointees_and_unknown_types() {
    let mut types = array_unsize_types(4, false, false);
    types[3].shape = SemanticTypeShapeV1::Slice {
        element: SemanticTypeIdV1(0),
    };
    assert_eq!(
        array_reference_unsize_length_v1(&types, SemanticTypeIdV1(4), SemanticTypeIdV1(5)),
        None
    );
    types[3].shape = SemanticTypeShapeV1::Array {
        element: SemanticTypeIdV1(1),
        length: 4,
    };
    assert_eq!(
        array_reference_unsize_length_v1(&types, SemanticTypeIdV1(4), SemanticTypeIdV1(5)),
        None
    );
    for (input, output) in [(0, 5), (4, 0), (u32::MAX, 5), (4, u32::MAX)] {
        assert_eq!(
            array_reference_unsize_length_v1(
                &types,
                SemanticTypeIdV1(input),
                SemanticTypeIdV1(output)
            ),
            None
        );
    }
}

#[test]
fn array_reference_unsize_checks_the_cast_and_fragment_wire_version() {
    let types = array_unsize_types(4, false, false);
    let mut request = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([70; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let mut function = direct_selection_root(90, SemanticTypeIdV1(0));
    let source = SemanticSourceProvenanceV1::unavailable();
    function.locals = vec![
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([91; 32]),
            SemanticTypeIdV1(0),
            SemanticLocalRoleV1::Return,
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([92; 32]),
            SemanticTypeIdV1(4),
            SemanticLocalRoleV1::Temporary,
            source,
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([93; 32]),
            SemanticTypeIdV1(5),
            SemanticLocalRoleV1::Temporary,
            source,
        ),
    ]
    .into_boxed_slice();
    let cast = SemanticRvalueV1::new(
        SemanticTypeIdV1(5),
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::ArrayReferenceToSlice,
            operand: SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1(1), vec![], SemanticTypeIdV1(4)).unwrap(),
            ),
        },
    );
    let mut context = ValidationContextV1 {
        request: &request,
        limits: SemanticMirLimitsV1::default(),
        totals: ValidationTotalsV1::default(),
        work: 0,
    };
    assert_eq!(
        validate_rvalue(
            &mut context,
            &function,
            SemanticMirLocationV1::Module,
            &cast
        ),
        Ok(())
    );
    let mut invalid = cast.clone();
    invalid.result_type = SemanticTypeIdV1(4);
    assert!(matches!(
        validate_rvalue(
            &mut context,
            &function,
            SemanticMirLocationV1::Module,
            &invalid
        ),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Cast,
            ..
        })
    ));
    function.blocks[0].statements = vec![SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1(2), vec![], SemanticTypeIdV1(5)).unwrap(),
            cast,
        )),
    )]
    .into_boxed_slice();
    // Component encoding/type checks do not initialize the temporary or grant a borrow.
    for version in 1..23 {
        let Some(version) = SemanticMirWireVersionV1::from_u16(version) else {
            continue;
        };
        assert!(matches!(
            canonical_semantic_function_fragment_sha256_v1(&function, version, 4096),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                required: SemanticMirWireVersionV1::V23,
                ..
            })
        ));
    }
    assert!(
        canonical_semantic_function_fragment_sha256_v1(
            &function,
            SemanticMirWireVersionV1::V23,
            4096
        )
        .is_ok()
    );
    request.functions = vec![function].into_boxed_slice();
    assert_eq!(
        minimum_wire_version(&request),
        SemanticMirWireVersionV1::V23
    );
}
