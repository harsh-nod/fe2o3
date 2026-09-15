fn shared_region_slice_semantic_types() -> Vec<SemanticTypeDeclV1> {
    let slice = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([0x84; 32]),
        SemanticLayoutIdentityV1::from_sha256([0x85; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice {
            element: SemanticTypeIdV1::from_index(0),
        },
    );
    let data = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let reference = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([0x86; 32]),
        SemanticLayoutIdentityV1::from_sha256([0x87; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(data, length),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(1),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    vec![scalar_semantic_type(0x80, 32, 4), slice, reference]
}

fn shared_region_slice_physical() -> SemanticAbiArgumentV1 {
    let data = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let length = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        SemanticTypeIdV1::from_index(2),
        SemanticAbiPassModeV1::Pair {
            first: data,
            second: length,
        },
    ))
}
