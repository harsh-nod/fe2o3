use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticLayoutIdentityV1, SemanticPointerTypeV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

fn scalar() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )
}

fn array(
    element: u32,
    length: u64,
    size: u64,
    alignment: u64,
    stride: u64,
    count: u64,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            size,
            alignment,
            SemanticFieldsShapeV1::array(stride, count),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            alignment,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(element),
            length,
        },
    )
}

#[test]
fn exact_array_layout_and_element_limit_are_independent_of_ssa_component_limits() {
    let ty = SemanticTypeIdV1::from_index(1);
    let types = [scalar(), array(0, 512, 2048, 4, 4, 512)];
    let slot = retained_array_slot_plan_v1(&types, ty, 512).unwrap();
    assert_eq!(slot.kernel_type, Type::Scalar(ScalarType::U32));
    assert_eq!(slot.alignment, 4);
    assert_eq!(
        slot.array,
        Some(SemanticRetainedArrayLayoutV1 {
            element: SemanticTypeIdV1::from_index(0),
            length: 512,
        })
    );
    assert!(matches!(
        retained_array_slot_plan_v1(&types, ty, 511),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations,
            actual: 512,
            limit: 511,
        })
    ));
}

#[test]
fn array_plan_rejects_mismatched_rustc_count_stride_size_and_alignment() {
    let ty = SemanticTypeIdV1::from_index(1);
    for invalid in [
        array(0, 8, 32, 4, 4, 7),
        array(0, 8, 64, 4, 8, 8),
        array(0, 8, 36, 4, 4, 8),
        array(0, 8, 32, 8, 4, 8),
        array(0, 0, 0, 4, 4, 0),
    ] {
        assert!(matches!(
            retained_array_slot_plan_v1(&[scalar(), invalid], ty, 512),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "retained array has no exact nonempty fixed element layout",
                ..
            })
        ));
    }
    assert!(matches!(
        retained_array_slot_plan_v1(&[scalar(), array(0, u64::MAX, 4, 4, 4, 1)], ty, usize::MAX,),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "retained array byte extent overflows",
            ..
        })
    ));
}

#[test]
fn arrays_admit_thin_pointer_elements_without_erasing_address_space_or_access() {
    let pointer = |metadata, size| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([6; 32]),
            SemanticTypeLayoutV1::new(Some(size), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(0),
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    1,
                    64,
                    metadata,
                )
                .unwrap(),
            ),
        )
    };
    let ty = SemanticTypeIdV1::from_index(2);
    let types = [
        scalar(),
        pointer(SemanticPointerMetadataV1::None, 8),
        array(1, 8, 64, 8, 8, 8),
    ];
    let slot = retained_array_slot_plan_v1(&types, ty, 8).unwrap();
    assert_eq!(
        slot.kernel_type,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )
    );
    assert!(matches!(
        retained_array_slot_plan_v1(
            &[
                scalar(),
                pointer(SemanticPointerMetadataV1::SliceLength, 16),
                array(1, 8, 128, 8, 16, 8)
            ],
            ty,
            8,
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "retained array element is not an exact storable scalar or metadata-free pointer",
            ..
        })
    ));
}

#[test]
fn nested_array_elements_are_not_reinterpreted_as_scalar_storage() {
    let types = [
        scalar(),
        array(0, 8, 32, 4, 4, 8),
        array(1, 2, 64, 4, 32, 2),
    ];
    assert!(matches!(
        retained_array_slot_plan_v1(&types, SemanticTypeIdV1::from_index(2), 512),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "retained array element is not an exact storable scalar or metadata-free pointer",
            ..
        })
    ));
}

#[test]
fn aggregate_expansion_checks_exact_prefixed_operation_limits_before_capacity() {
    // 8 * (Index + GEP + Load/Store), plus five operations already emitted.
    assert_eq!(retained_array_expansion_v1(8, 5, 4, 29).unwrap(), 8);
    assert!(matches!(
        retained_array_expansion_v1(8, 5, 4, 28),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations,
            actual: 29,
            limit: 28,
        })
    ));
    assert_eq!(
        retained_array_expansion_v1(8, 5, MAX_BLOCK_OPERATIONS_V1 - 24, 29).unwrap(),
        8,
    );
    assert!(matches!(
        retained_array_expansion_v1(8, 5, MAX_BLOCK_OPERATIONS_V1 - 23, 29),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Operations, actual, limit,
        }) if actual == MAX_BLOCK_OPERATIONS_V1 + 1 && limit == MAX_BLOCK_OPERATIONS_V1
    ));
    assert!(retained_array_expansion_v1(u64::MAX, 0, 0, usize::MAX).is_err());
    assert!(retained_array_expansion_v1(8, usize::MAX, 0, usize::MAX).is_err());
}
