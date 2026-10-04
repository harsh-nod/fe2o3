use fe2o3_mir_model::semantic_mir_v1::*;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn layout_identity(tag: u8) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(tag))
}

fn full_range(bits: u16) -> SemanticScalarValidityRangeV1 {
    let end = if bits >= 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    };
    SemanticScalarValidityRangeV1::new(0, end)
}

fn initialized(
    primitive: SemanticBackendPrimitiveV1,
    valid_range: SemanticScalarValidityRangeV1,
) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(primitive, valid_range)
}

fn direct() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
}

fn pair() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Pair {
        first: initialized_attributes(),
        second: initialized_attributes(),
    }
}

fn initialized_direct() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(initialized_attributes())
}

fn initialized_attributes() -> SemanticAbiValueAttributesV1 {
    initialized_attributes_with_nonnull(false)
}

fn initialized_attributes_with_nonnull(non_null: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, non_null, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}

fn opaque_type(tag: u8, layout: SemanticTypeLayoutV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        layout,
        SemanticTypeShapeV1::Opaque,
    )
}

fn u32_type(tag: u8) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(initialized(primitive, full_range(32))),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )
}

fn request(
    types: Vec<SemanticTypeDeclV1>,
    return_type: SemanticTypeIdV1,
    mode: SemanticAbiPassModeV1,
    statics: Vec<SemanticStaticDeclV1>,
) -> InertSemanticMirRequestV1 {
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(240),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(return_type, mode),
    )
    .unwrap();
    let local = SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(1)),
        return_type,
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    );
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(1)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(1)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(1)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![local],
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();

    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        types,
        vec![],
        statics,
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn admit_type(ty: SemanticTypeDeclV1, mode: SemanticAbiPassModeV1) {
    request(vec![ty], SemanticTypeIdV1::from_index(0), mode, vec![])
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
}

fn assert_invalid_layout(result: Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1>) {
    assert!(matches!(result, Err(SemanticMirErrorV1::InvalidTypeLayout)));
}

fn backend_pointer_type(
    tag: u8,
    address_space: u32,
    size_bytes: u64,
    alignment_bytes: u64,
) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::pointer(address_space, size_bytes, alignment_bytes);
    opaque_type(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size_bytes),
            alignment_bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::union(primitive)),
            false,
        )
        .unwrap(),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    )
}

#[test]
fn gfx942_descriptor_pointer_profiles_are_exact() {
    for (address_space, size_bytes, alignment_bytes) in [(7, 20, 32), (8, 16, 16), (9, 24, 32)] {
        admit_type(
            backend_pointer_type(1, address_space, size_bytes, alignment_bytes),
            direct(),
        );
    }

    for (address_space, size_bytes, alignment_bytes) in [
        (7, 24, 32),
        (7, 20, 16),
        (8, 8, 16),
        (8, 16, 8),
        (9, 32, 32),
        (9, 24, 16),
    ] {
        assert_invalid_layout(
            request(
                vec![backend_pointer_type(
                    1,
                    address_space,
                    size_bytes,
                    alignment_bytes,
                )],
                SemanticTypeIdV1::from_index(0),
                direct(),
                vec![],
            )
            .admit(SemanticMirLimitsV1::default()),
        );
    }
}

#[test]
fn descriptor_address_spaces_reject_rust_statics() {
    for address_space in [7, 8, 9] {
        let static_decl = SemanticStaticDeclV1::new(
            SemanticStaticIdentityV1::from_sha256(bytes(1)),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTypeIdV1::from_index(0),
            false,
            address_space,
            SemanticStaticDefinitionV1::ExternalRequired {
                symbol: SemanticLinkSymbolV1::new(
                    format!("descriptor_as{address_space}").into_bytes(),
                )
                .unwrap(),
            },
        );
        assert!(matches!(
            request(
                vec![u32_type(1)],
                SemanticTypeIdV1::from_index(0),
                initialized_direct(),
                vec![static_decl],
            )
            .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidStatic)
        ));
    }
}

fn pointee_type() -> SemanticTypeDeclV1 {
    opaque_type(
        1,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
    )
}

fn fat_pointer_type(
    metadata: SemanticPointerMetadataV1,
    metadata_scalar: SemanticBackendScalarV1,
) -> SemanticTypeDeclV1 {
    let data = initialized(SemanticBackendPrimitiveV1::pointer(0, 8, 8), full_range(64));
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(data, metadata_scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                metadata,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            (metadata == SemanticPointerMetadataV1::VTable
                && matches!(
                    metadata_scalar.primitive(),
                    SemanticBackendPrimitiveV1::Pointer { .. }
                ))
            .then(|| SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        ),
    )
}

fn admit_fat_pointer(
    metadata: SemanticPointerMetadataV1,
    metadata_scalar: SemanticBackendScalarV1,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    request(
        vec![pointee_type(), fat_pointer_type(metadata, metadata_scalar)],
        SemanticTypeIdV1::from_index(1),
        if metadata == SemanticPointerMetadataV1::VTable {
            SemanticAbiPassModeV1::Pair {
                first: initialized_attributes(),
                second: initialized_attributes_with_nonnull(true),
            }
        } else {
            pair()
        },
        vec![],
    )
    .admit(SemanticMirLimitsV1::default())
}

#[test]
fn slice_metadata_requires_initialized_unsigned_gfx942_usize() {
    let exact = initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        full_range(64),
    );
    admit_fat_pointer(SemanticPointerMetadataV1::SliceLength, exact).unwrap();

    let malformed = [
        initialized(
            SemanticBackendPrimitiveV1::integer(true, 64, 8),
            full_range(64),
        ),
        SemanticBackendScalarV1::union(SemanticBackendPrimitiveV1::integer(false, 64, 8)),
        initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, 1024),
        ),
        initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            full_range(32),
        ),
        initialized(SemanticBackendPrimitiveV1::pointer(0, 8, 8), full_range(64)),
    ];
    for metadata in malformed {
        assert_invalid_layout(admit_fat_pointer(
            SemanticPointerMetadataV1::SliceLength,
            metadata,
        ));
    }
}

#[test]
fn vtable_metadata_requires_initialized_non_null_as0_pointer() {
    let exact = initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    admit_fat_pointer(SemanticPointerMetadataV1::VTable, exact).unwrap();

    let malformed = [
        initialized(SemanticBackendPrimitiveV1::pointer(0, 8, 8), full_range(64)),
        initialized(
            SemanticBackendPrimitiveV1::pointer(1, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        ),
        SemanticBackendScalarV1::union(SemanticBackendPrimitiveV1::pointer(0, 8, 8)),
        initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        ),
    ];
    for metadata in malformed {
        assert!(matches!(
            admit_fat_pointer(SemanticPointerMetadataV1::VTable, metadata),
            Err(SemanticMirErrorV1::InvalidTypeLayout | SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

fn exact_pair_layout(randomization_seed: u64) -> SemanticTypeLayoutV1 {
    let first = initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        full_range(32),
    );
    let second = initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        full_range(64),
    );
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        16,
        8,
        SemanticFieldsShapeV1::arbitrary(vec![0, 8], vec![0, 1]).unwrap(),
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::scalar_pair(first, second),
        None,
        false,
        None,
        8,
        randomization_seed,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap()
}

fn scalar_seed_for_union(primitive: SemanticBackendPrimitiveV1) -> u64 {
    let size = primitive.size_bytes().unwrap();
    let kind = match primitive {
        SemanticBackendPrimitiveV1::Integer { signed: true, .. } => 1,
        SemanticBackendPrimitiveV1::Integer { signed: false, .. } => 2,
        SemanticBackendPrimitiveV1::Float { .. } => 3,
        SemanticBackendPrimitiveV1::Pointer { .. } => 4,
    };
    let bits = u32::try_from(size * 8).unwrap();
    let end = if bits >= 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    size.wrapping_add(kind << 32)
        .wrapping_add(end.rotate_right(16))
}

fn vector_backend(scalable: bool) -> SemanticBackendReprV1 {
    let element = SemanticBackendScalarV1::union(SemanticBackendPrimitiveV1::float(32, 4));
    if scalable {
        SemanticBackendReprV1::simd_scalable_vector(element, 4)
    } else {
        SemanticBackendReprV1::simd_vector(element, 4)
    }
}

fn exact_vector_layout(scalable: bool, randomization_seed: u64) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        16,
        16,
        SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
        SemanticRustcVariantsV1::Single { index: 0 },
        vector_backend(scalable),
        None,
        false,
        None,
        4,
        randomization_seed,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap()
}

#[test]
fn direct_pair_and_vector_seed_mutations_are_rejected() {
    let pair_seed = 4 + 8;
    admit_type(opaque_type(1, exact_pair_layout(pair_seed)), pair());
    assert_invalid_layout(
        request(
            vec![opaque_type(1, exact_pair_layout(pair_seed + 1))],
            SemanticTypeIdV1::from_index(0),
            pair(),
            vec![],
        )
        .admit(SemanticMirLimitsV1::default()),
    );

    let vector_seed = scalar_seed_for_union(SemanticBackendPrimitiveV1::float(32, 4)) + 4;
    for scalable in [false, true] {
        admit_type(
            opaque_type(1, exact_vector_layout(scalable, vector_seed)),
            direct(),
        );
        assert_invalid_layout(
            request(
                vec![opaque_type(
                    1,
                    exact_vector_layout(scalable, vector_seed ^ 1),
                )],
                SemanticTypeIdV1::from_index(0),
                direct(),
                vec![],
            )
            .admit(SemanticMirLimitsV1::default()),
        );
    }
}

#[test]
fn aggregate_pair_retains_its_exact_rustc_seed_without_primitive_reinterpretation() {
    let scalar = initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        full_range(32),
    );
    let aggregate = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![0, 4], vec![0, 1]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::scalar_pair(scalar, scalar),
            None,
            false,
            None,
            4,
            0x5a17_9c03,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                SemanticTypeIdV1::from_index(0),
                SemanticTypeIdV1::from_index(0),
            ])
            .unwrap(),
        ),
    );
    request(
        vec![u32_type(1), aggregate],
        SemanticTypeIdV1::from_index(1),
        pair(),
        vec![],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
}

fn union_forwarded_type(
    backend_repr: SemanticBackendReprV1,
    size_bytes: u64,
    alignment_bytes: u64,
    unadjusted_alignment_bytes: u64,
    randomization_seed: u64,
) -> Vec<SemanticTypeDeclV1> {
    let field = opaque_type(
        1,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size_bytes),
            alignment_bytes,
            backend_repr,
            false,
        )
        .unwrap(),
    );
    let union = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            size_bytes,
            alignment_bytes,
            SemanticFieldsShapeV1::union(1).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            backend_repr,
            None,
            false,
            None,
            unadjusted_alignment_bytes,
            randomization_seed,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(0)]).unwrap(),
        ),
    );
    vec![field, union]
}

#[test]
fn union_forwarded_pair_and_vector_layouts_are_admitted() {
    let pair_backend = SemanticBackendReprV1::scalar_pair(
        initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            full_range(32),
        ),
        initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            full_range(64),
        ),
    );
    request(
        union_forwarded_type(pair_backend, 16, 8, 8, 12),
        SemanticTypeIdV1::from_index(1),
        pair(),
        vec![],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    let vector_seed = scalar_seed_for_union(SemanticBackendPrimitiveV1::float(32, 4)) + 4;
    for scalable in [false, true] {
        request(
            union_forwarded_type(vector_backend(scalable), 16, 16, 4, vector_seed),
            SemanticTypeIdV1::from_index(1),
            direct(),
            vec![],
        )
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    }
}
