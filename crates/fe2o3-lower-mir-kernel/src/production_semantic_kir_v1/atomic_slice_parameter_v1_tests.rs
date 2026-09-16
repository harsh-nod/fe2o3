use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

// Component fixtures only: genuine source and complete ABI admission remain
// upstream prerequisites, not authority supplied by these constructed types.
fn atomic_parameter_types() -> Vec<SemanticTypeDeclV1> {
    let declare = |index: u8, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([index + 1; 32]),
            SemanticLayoutIdentityV1::from_sha256([index + 1; 32]),
            layout,
            shape,
        )
    };
    let mut types = vec![declare(
        0,
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )];
    for index in 1..=3 {
        let declaration = declare(
            index,
            SemanticTypeLayoutV1::aggregate(
                Some(4),
                4,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(u32::from(
                    index - 1,
                ))])
                .unwrap(),
            ),
        );
        types.push(if index == 3 {
            declaration.with_rust_type_kind(SemanticRustTypeKindV1::CoreAtomicU32)
        } else {
            declaration
        });
    }
    types.push(declare(
        4,
        SemanticTypeLayoutV1::new(None, 4).unwrap(),
        SemanticTypeShapeV1::Slice {
            element: SemanticTypeIdV1::from_index(3),
        },
    ));
    types.push(declare(
        5,
        SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(4),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    ));
    types
}

#[test]
fn atomic_slice_parameter_keeps_nominal_readwrite_slice_not_readonly() {
    let types = atomic_parameter_types();
    let ty = SemanticTypeIdV1::from_index(5);
    let expected = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    assert_eq!(
        lower_shared_atomic_slice_parameter_v1(&types, ty),
        Some(expected.clone())
    );
    assert_eq!(lower_parameter_type(&types, &[], ty).unwrap(), expected);
    let mut ordinary = types.clone();
    ordinary[3] = ordinary[3]
        .clone()
        .with_rust_type_kind(SemanticRustTypeKindV1::Ordinary);
    assert!(lower_shared_atomic_slice_parameter_v1(&ordinary, ty).is_none());
    assert!(lower_parameter_type(&ordinary, &[], ty).is_err());
}

#[test]
fn atomic_slice_parameter_rejects_wrong_pointer_and_storage_contracts() {
    let types = atomic_parameter_types();
    let ty = SemanticTypeIdV1::from_index(5);
    for (kind, mutability, space, width, metadata) in [
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::SliceLength,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            0,
            64,
            SemanticPointerMetadataV1::SliceLength,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            3,
            64,
            SemanticPointerMetadataV1::SliceLength,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            32,
            SemanticPointerMetadataV1::SliceLength,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
        ),
    ] {
        let mut changed = types.clone();
        changed[5] = SemanticTypeDeclV1::new(
            types[5].identity(),
            types[5].layout_identity(),
            types[5].layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SemanticTypeIdV1::from_index(4),
                    kind,
                    mutability,
                    space,
                    width,
                    metadata,
                )
                .unwrap(),
            ),
        );
        assert!(lower_shared_atomic_slice_parameter_v1(&changed, ty).is_none());
    }
    for (size, alignment, offset) in [(8, 4, 0), (8, 8, 0), (4, 4, 1)] {
        let mut changed = types.clone();
        changed[3] = SemanticTypeDeclV1::new(
            types[3].identity(),
            types[3].layout_identity(),
            SemanticTypeLayoutV1::aggregate(
                Some(size),
                alignment,
                SemanticAggregateLayoutV1::new(vec![offset], vec![]).unwrap(),
            )
            .unwrap(),
            types[3].shape().clone(),
        )
        .with_rust_type_kind(SemanticRustTypeKindV1::CoreAtomicU32);
        assert!(lower_shared_atomic_slice_parameter_v1(&changed, ty).is_none());
    }
    let mut changed = types.clone();
    changed[0] = SemanticTypeDeclV1::new(
        types[0].identity(),
        types[0].layout_identity(),
        types[0].layout().clone(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        }),
    );
    assert!(lower_shared_atomic_slice_parameter_v1(&changed, ty).is_none());
}
