use super::*;

const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

fn index(offset: u64, minimum_length: u64, from_end: bool) -> SemanticProjectionKindV1 {
    SemanticProjectionKindV1::ConstantIndex {
        offset,
        minimum_length,
        from_end,
    }
}

fn array_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(44),
        layout_identity(44),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            4,
            SemanticFieldsShapeV1::array(4, 4),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: READ_VIEW_ELEMENT,
            length: 4,
        },
    )
}

fn fixture(
    base: SemanticTypeIdV1,
    result: SemanticTypeIdV1,
    projection: SemanticProjectionKindV1,
) -> InertSemanticMirRequestV1 {
    let mut projections = Vec::new();
    if base == READ_VIEW_SLICE_REF {
        projections.push(
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, READ_VIEW_SLICE)
                .unwrap(),
        );
    }
    projections.push(SemanticProjectionV1::new(projection, result).unwrap());
    request_with_statement(
        vec![
            u32_type(1),
            read_view_usize_type(),
            read_view_slice_type(),
            read_view_shared_slice_reference_type(),
            array_type(),
        ],
        abi(1, vec![], READ_VIEW_ELEMENT),
        vec![
            local(1, READ_VIEW_ELEMENT, SemanticLocalRoleV1::Return),
            SemanticLocalDeclV1::new(
                local_identity(2),
                base,
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::new(Some(origin(11, 7)), Some(origin(12, 9))),
            ),
            local(3, READ_VIEW_USIZE, SemanticLocalRoleV1::Temporary),
            local(4, READ_VIEW_SLICE_REF, SemanticLocalRoleV1::Temporary),
            local(5, ARRAY, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Deinitialize(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, result).unwrap(),
        ),
    )
}

#[test]
fn forward_and_suffix_constructor_bounds_preserve_full_u64_domain() {
    for (offset, minimum, suffix) in [
        (0, 1, false),
        (3, 4, false),
        (u64::MAX - 1, u64::MAX, false),
        (1, 1, true),
        (1, 4, true),
        (4, 4, true),
        (u64::MAX, u64::MAX, true),
    ] {
        let kind = index(offset, minimum, suffix);
        let projection = SemanticProjectionV1::new(kind, READ_VIEW_ELEMENT).unwrap();
        assert_eq!(projection.kind(), kind);
        assert_eq!(projection.result_type(), READ_VIEW_ELEMENT);
    }
    for (offset, minimum, suffix) in [
        (0, 0, false),
        (1, 0, false),
        (1, 1, false),
        (2, 1, false),
        (u64::MAX, u64::MAX, false),
        (0, 0, true),
        (0, 1, true),
        (0, u64::MAX, true),
        (1, 0, true),
        (2, 1, true),
        (u64::MAX, u64::MAX - 1, true),
    ] {
        assert_eq!(
            SemanticProjectionV1::new(index(offset, minimum, suffix), READ_VIEW_ELEMENT),
            Err(SemanticMirErrorV1::InvalidProjectionShape),
        );
    }
}

#[test]
fn slice_constant_indices_round_trip_without_granting_runtime_bounds() {
    for kind in [
        index(0, 1, false),
        index(1, 4, false),
        index(1, 1, true),
        index(2, 4, true),
        index(u64::MAX, u64::MAX, true),
    ] {
        let admitted = fixture(READ_VIEW_SLICE_REF, READ_VIEW_ELEMENT, kind)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
        assert_eq!(
            decoded.functions()[0].locals()[1].source(),
            SemanticSourceProvenanceV1::new(Some(origin(11, 7)), Some(origin(12, 9))),
        );
        let SemanticStatementKindV1::Deinitialize(place) =
            decoded.functions()[0].blocks()[0].statements()[0].kind()
        else {
            panic!("retained place statement changed");
        };
        assert_eq!(place.local(), SemanticLocalIdV1::from_index(1));
        assert_eq!(place.ty(), READ_VIEW_ELEMENT);
        assert_eq!(
            place.projections(),
            &[
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, READ_VIEW_SLICE,)
                    .unwrap(),
                SemanticProjectionV1::new(kind, READ_VIEW_ELEMENT).unwrap(),
            ],
        );
    }
}

#[test]
fn constant_index_type_validation_preserves_array_bounds_and_element_identity() {
    for kind in [index(0, 1, false), index(3, 4, false), index(4, 4, true)] {
        fixture(ARRAY, READ_VIEW_ELEMENT, kind)
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
    }
    for (base, kind) in [
        (ARRAY, index(0, 5, false)),
        (ARRAY, index(5, 5, true)),
        (READ_VIEW_ELEMENT, index(0, 1, false)),
    ] {
        assert!(matches!(
            fixture(base, READ_VIEW_ELEMENT, kind).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::Projection,
                ..
            })
        ));
    }
    for base in [ARRAY, READ_VIEW_SLICE_REF] {
        assert!(matches!(
            fixture(base, READ_VIEW_USIZE, index(0, 1, false))
                .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::TypeMismatch { expected, actual, .. })
                if expected == READ_VIEW_ELEMENT && actual == READ_VIEW_USIZE
        ));
    }
}
