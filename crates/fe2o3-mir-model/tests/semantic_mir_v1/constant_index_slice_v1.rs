use super::*;

fn constant_slice_request(
    offset: u64,
    minimum_length: u64,
    from_end: bool,
    result_type: SemanticTypeIdV1,
) -> InertSemanticMirRequestV1 {
    let source = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, READ_VIEW_SLICE)
                .unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                },
                result_type,
            )
            .unwrap(),
        ],
        result_type,
    )
    .unwrap();
    request(
        vec![
            u32_type(40),
            read_view_usize_type(),
            read_view_slice_type(),
            read_view_shared_slice_reference_type(),
        ],
        vec![],
        vec![function(
            1,
            abi(1, vec![], READ_VIEW_ELEMENT),
            vec![
                local(1, READ_VIEW_ELEMENT, SemanticLocalRoleV1::Return),
                local(2, READ_VIEW_SLICE_REF, SemanticLocalRoleV1::Temporary),
                local(3, READ_VIEW_USIZE, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(
                1,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Deinitialize(source),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    )
}

#[test]
fn constant_slice_index_type_admission_preserves_literal_and_minimum_length() {
    let first = constant_slice_request(0, 1, false, READ_VIEW_ELEMENT)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let later = constant_slice_request(24, 25, false, READ_VIEW_ELEMENT)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let larger_minimum = constant_slice_request(24, 64, false, READ_VIEW_ELEMENT)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_ne!(first.semantic_sha256(), later.semantic_sha256());
    assert_ne!(later.semantic_sha256(), larger_minimum.semantic_sha256());
    // This is type/shape admission only; no runtime bounds authority is produced.
}

#[test]
fn constant_slice_index_rejects_from_end_and_mismatched_element_type() {
    for request in [
        constant_slice_request(1, 2, true, READ_VIEW_ELEMENT),
        constant_slice_request(24, 25, false, READ_VIEW_USIZE),
    ] {
        assert!(request.admit(SemanticMirLimitsV1::default()).is_err());
    }
}

#[test]
fn constant_slice_index_still_requires_well_formed_projection_shape() {
    for (offset, minimum_length) in [(0, 0), (24, 24), (25, 24), (u64::MAX, u64::MAX)] {
        assert!(matches!(
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end: false
                },
                READ_VIEW_ELEMENT,
            ),
            Err(SemanticMirErrorV1::InvalidProjectionShape)
        ));
    }
}
