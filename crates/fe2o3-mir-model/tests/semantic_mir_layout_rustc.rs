use fe2o3_mir_model::semantic_mir_v1::*;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn layout_identity(tag: u8) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(tag))
}

fn full_range(bits: u16) -> SemanticScalarValidityRangeV1 {
    let end = if bits == 128 {
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

fn opaque_type(tag: u8, layout: SemanticTypeLayoutV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        layout,
        SemanticTypeShapeV1::Opaque,
    )
}

fn unit_type(tag: u8) -> SemanticTypeDeclV1 {
    opaque_type(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
    )
}

fn request(
    types: Vec<SemanticTypeDeclV1>,
    return_type: SemanticTypeIdV1,
    mode: SemanticAbiPassModeV1,
) -> InertSemanticMirRequestV1 {
    request_for_target(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        types,
        return_type,
        mode,
    )
}

fn request_for_target(
    target: SemanticTargetDataLayoutV1,
    types: Vec<SemanticTypeDeclV1>,
    return_type: SemanticTypeIdV1,
    mode: SemanticAbiPassModeV1,
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
        target,
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn direct() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(initialized_attrs(false))
}

fn vector_direct() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
}

fn pair() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Pair {
        first: initialized_attrs(false),
        second: initialized_attrs(false),
    }
}

fn pair_with_first_nonnull() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Pair {
        first: initialized_attrs(true),
        second: initialized_attrs(false),
    }
}

fn initialized_attrs(non_null: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, non_null, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}

fn u64_scalar_type(tag: u8, alignment_bytes: u64) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 64, alignment_bytes);
    let backend = SemanticBackendReprV1::scalar(initialized(primitive, full_range(64)));
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticTypeLayoutV1::new_with_backend_repr(Some(8), alignment_bytes, backend, false)
            .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    )
}

#[test]
fn scalar_facts_use_target_alignment_and_reject_inconsistent_pass_modes() {
    let ty = u64_scalar_type(1, 8);
    let admitted = request(vec![ty.clone()], SemanticTypeIdV1::from_index(0), direct())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();

    assert_eq!(admitted.types()[0].layout().alignment_bytes(), 8);
    assert!(!admitted.types()[0].layout().is_uninhabited());
    let SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::Initialized {
        primitive,
        valid_range,
    }) = admitted.types()[0].layout().backend_repr()
    else {
        panic!("scalar backend facts were not retained");
    };
    assert_eq!(primitive.size_bytes(), Some(8));
    assert_eq!(primitive.alignment_bytes(), 8);
    assert_eq!(*valid_range, full_range(64));

    assert!(matches!(
        request(vec![ty], SemanticTypeIdV1::from_index(0), pair())
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        request(
            vec![u64_scalar_type(1, 4)],
            SemanticTypeIdV1::from_index(0),
            direct(),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

#[test]
fn scalar_pair_and_non_two_word_pointer_layout_are_exact() {
    let data = initialized(
        SemanticBackendPrimitiveV1::pointer(2, 4, 4),
        SemanticScalarValidityRangeV1::new(1, u32::MAX.into()),
    );
    let metadata = initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        full_range(64),
    );
    let backend = SemanticBackendReprV1::scalar_pair(data, metadata);
    let pointer = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![0, 8], vec![0, 1]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            backend,
            Some(
                SemanticLayoutNicheV1::new(
                    0,
                    SemanticBackendPrimitiveV1::pointer(2, 4, 4),
                    SemanticScalarValidityRangeV1::new(1, u32::MAX.into()),
                )
                .unwrap(),
            ),
            false,
            None,
            8,
            12,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                2,
                32,
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
                    1,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let admitted = request(
        vec![unit_type(1), pointer.clone()],
        SemanticTypeIdV1::from_index(1),
        pair_with_first_nonnull(),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    let SemanticBackendReprV1::ScalarPair { .. } = admitted.types()[1].layout().backend_repr()
    else {
        panic!("scalar-pair backend facts were not retained");
    };
    assert!(matches!(
        admitted.types()[1].layout().fields(),
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } if source_order_offsets_bytes.as_ref() == [0, 8]
            && memory_order_source_indices.as_ref() == [0, 1]
    ));
    assert_eq!(admitted.types()[1].layout().size_bytes(), Some(16));

    assert!(matches!(
        request(
            vec![unit_type(1), pointer],
            SemanticTypeIdV1::from_index(1),
            direct(),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn fixed_and_scalable_simd_are_distinct_direct_backend_forms() {
    let element = SemanticBackendScalarV1::union(SemanticBackendPrimitiveV1::float(32, 4));
    let fixed = opaque_type(
        1,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            16,
            SemanticBackendReprV1::simd_vector(element, 4),
            false,
        )
        .unwrap(),
    );
    let scalable = opaque_type(
        1,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            16,
            SemanticBackendReprV1::simd_scalable_vector(element, 4),
            false,
        )
        .unwrap(),
    );

    let fixed = request(
        vec![fixed],
        SemanticTypeIdV1::from_index(0),
        vector_direct(),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let scalable = request(
        vec![scalable],
        SemanticTypeIdV1::from_index(0),
        vector_direct(),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(fixed.semantic_sha256(), scalable.semantic_sha256());
    assert!(!scalable.types()[0].layout().is_uninhabited());
}

fn opaque_scalar_request(
    scalar: SemanticBackendScalarV1,
    uninhabited: bool,
) -> AdmittedInertSemanticMirV1 {
    let ty = opaque_type(
        1,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(scalar),
            uninhabited,
        )
        .unwrap(),
    );
    let extension = match scalar {
        SemanticBackendScalarV1::Initialized {
            primitive:
                SemanticBackendPrimitiveV1::Integer {
                    signed: false,
                    bits: 8,
                    ..
                },
            valid_range,
        } if valid_range == SemanticScalarValidityRangeV1::new(0, 1) => {
            SemanticAbiExtensionV1::ZeroExtend
        }
        _ => SemanticAbiExtensionV1::None,
    };
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            false,
            None,
            false,
            false,
            false,
            matches!(scalar, SemanticBackendScalarV1::Initialized { .. }),
        ),
        extension,
        0,
        None,
    )
    .unwrap();
    request(
        vec![ty],
        SemanticTypeIdV1::from_index(0),
        SemanticAbiPassModeV1::Direct(attributes),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn backend_facts_are_canonical_and_collision_separated() {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 8, 1);
    let initialized_full = opaque_scalar_request(
        initialized(
            primitive,
            SemanticScalarValidityRangeV1::new(0, u8::MAX.into()),
        ),
        false,
    );
    let initialized_wrapping = opaque_scalar_request(
        initialized(primitive, SemanticScalarValidityRangeV1::new(200, 100)),
        false,
    );
    let union = opaque_scalar_request(SemanticBackendScalarV1::union(primitive), false);
    let identities = [
        initialized_full.semantic_sha256(),
        initialized_wrapping.semantic_sha256(),
        union.semantic_sha256(),
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(identities.len(), 3);

    let impossible_direct_uninhabited = opaque_type(
        1,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, u8::MAX.into()),
            )),
            true,
        )
        .unwrap(),
    );
    assert!(matches!(
        request(
            vec![impossible_direct_uninhabited],
            SemanticTypeIdV1::from_index(0),
            direct(),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));

    let encoding = initialized_full.canonical_encoding();
    let magic = b"fe2o3.inert-semantic-mir";
    assert_eq!(
        initialized_full.wire_version(),
        SemanticMirWireVersionV1::V2
    );
    assert_eq!(&encoding[..magic.len()], magic);
    assert_eq!(
        u16::from_le_bytes([encoding[magic.len()], encoding[magic.len() + 1]]),
        INERT_SEMANTIC_MIR_VERSION_V2
    );
    assert_eq!(
        initialized_full.semantic_sha256().as_bytes(),
        &[
            226, 57, 77, 14, 16, 183, 42, 234, 36, 11, 68, 211, 228, 130, 10, 31, 155, 129, 124,
            78, 156, 53, 230, 74, 2, 132, 173, 102, 59, 240, 185, 114,
        ],
    );
}

#[test]
fn hostile_backend_records_are_rejected_before_admission() {
    let u8_primitive = SemanticBackendPrimitiveV1::integer(false, 8, 1);
    let out_of_range = initialized(
        u8_primitive,
        SemanticScalarValidityRangeV1::new(0, u16::MAX.into()),
    );
    assert!(matches!(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(out_of_range),
            false,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));

    let first = initialized(u8_primitive, full_range(8));
    let second = initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        full_range(32),
    );
    assert!(matches!(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar_pair(first, second),
            false,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(131_076),
            4,
            SemanticBackendReprV1::simd_vector(first, 32_769),
            false,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            4,
            SemanticBackendReprV1::simd_scalable_vector(first, 4),
            false,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::simd_vector(first, 0),
            false,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            4,
            SemanticBackendReprV1::memory(false),
            false,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));

    let memory = opaque_type(
        1,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
    );
    assert!(matches!(
        request(vec![memory], SemanticTypeIdV1::from_index(0), direct())
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

fn exact_u64_layout(
    fields: SemanticFieldsShapeV1,
    largest_niche: Option<SemanticLayoutNicheV1>,
    max_repr_alignment_bytes: Option<u64>,
    unadjusted_abi_alignment_bytes: u64,
) -> SemanticTypeLayoutV1 {
    exact_u64_layout_with_seed(
        fields,
        largest_niche,
        max_repr_alignment_bytes,
        unadjusted_abi_alignment_bytes,
        8_589_934_599,
    )
}

fn exact_u64_layout_with_seed(
    fields: SemanticFieldsShapeV1,
    largest_niche: Option<SemanticLayoutNicheV1>,
    max_repr_alignment_bytes: Option<u64>,
    unadjusted_abi_alignment_bytes: u64,
    randomization_seed: u64,
) -> SemanticTypeLayoutV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 64, 8);
    SemanticTypeLayoutV1::with_exact_rustc_layout(
        8,
        8,
        fields,
        SemanticRustcVariantsV1::Single { index: 0 },
        SemanticBackendReprV1::scalar(initialized(primitive, full_range(64))),
        largest_niche,
        false,
        max_repr_alignment_bytes,
        unadjusted_abi_alignment_bytes,
        randomization_seed,
        SemanticTypeLayoutDetailsV1::None,
    )
    .unwrap()
}

fn exact_layout_identity(layout: SemanticTypeLayoutV1) -> InertSemanticMirSha256V1 {
    request(
        vec![opaque_type(1, layout)],
        SemanticTypeIdV1::from_index(0),
        direct(),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
    .semantic_sha256()
}

#[test]
fn fields_niche_and_alignment_axes_are_exact_and_canonical() {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 64, 8);
    let niche = SemanticLayoutNicheV1::new(
        0,
        primitive,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();
    let baseline = exact_u64_layout(SemanticFieldsShapeV1::Primitive, None, None, 8);
    let baseline_identity = exact_layout_identity(baseline);
    let forwarded =
        exact_u64_layout_with_seed(SemanticFieldsShapeV1::union(1).unwrap(), None, None, 8, 17);
    assert_ne!(baseline_identity, exact_layout_identity(forwarded));

    let malformed = [
        exact_u64_layout(SemanticFieldsShapeV1::Primitive, Some(niche), None, 8),
        exact_u64_layout(SemanticFieldsShapeV1::Primitive, None, Some(8), 8),
        exact_u64_layout(SemanticFieldsShapeV1::Primitive, None, None, 4),
        exact_u64_layout(SemanticFieldsShapeV1::Primitive, None, Some(16), 8),
        exact_u64_layout(SemanticFieldsShapeV1::Primitive, None, None, 16),
        exact_u64_layout_with_seed(SemanticFieldsShapeV1::Primitive, None, None, 8, 18),
    ];
    for layout in malformed {
        assert!(matches!(
            request(
                vec![opaque_type(1, layout)],
                SemanticTypeIdV1::from_index(0),
                direct(),
            )
            .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeLayout)
        ));
    }
}

#[test]
fn hostile_rustc_layout_facts_fail_before_admission() {
    let target = SemanticTargetDataLayoutV1::gfx942(layout_identity(250));
    assert_eq!(target.object_size_bound_bytes(), 1 << 61);
    assert_eq!(
        target.architecture(),
        SemanticTargetArchitectureV1::AmdGpuGfx942
    );
    assert!(matches!(
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(initialized(
                SemanticBackendPrimitiveV1::float(8, 1),
                SemanticScalarValidityRangeV1::new(0, u8::MAX.into()),
            )),
            false,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticFieldsShapeV1::union(0),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticFieldsShapeV1::arbitrary(vec![0, 4], vec![0, 0]),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticFieldsShapeV1::arbitrary(vec![4, 0], vec![0, 1]),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));

    let primitive = SemanticBackendPrimitiveV1::integer(false, 64, 8);
    assert!(matches!(
        SemanticLayoutNicheV1::new(0, primitive, full_range(64)),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    let out_of_bounds_niche = SemanticLayoutNicheV1::new(
        8,
        primitive,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();
    assert!(matches!(
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            8,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::scalar(initialized(primitive, full_range(64))),
            Some(out_of_bounds_niche),
            false,
            None,
            8,
            17,
            SemanticTypeLayoutDetailsV1::None,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            8,
            SemanticFieldsShapeV1::array(8, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::scalar(initialized(primitive, full_range(64))),
            None,
            false,
            None,
            8,
            17,
            SemanticTypeLayoutDetailsV1::None,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert!(matches!(
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            8,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::scalar(initialized(primitive, full_range(64))),
            None,
            false,
            Some(1_u64 << 30),
            8,
            17,
            SemanticTypeLayoutDetailsV1::None,
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

fn pointer_kind_identity(kind: SemanticPointerKindV1) -> InertSemanticMirSha256V1 {
    let primitive = SemanticBackendPrimitiveV1::pointer(3, 4, 4);
    let valid_range = match kind {
        SemanticPointerKindV1::Raw => full_range(32),
        SemanticPointerKindV1::Reference => SemanticScalarValidityRangeV1::new(1, u32::MAX.into()),
    };
    let pointer = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(initialized(primitive, valid_range)),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
                kind,
                SemanticMutabilityV1::Immutable,
                3,
                32,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    match kind {
                        SemanticPointerKindV1::Raw => SemanticAbiPointeeKindV1::Raw,
                        SemanticPointerKindV1::Reference => {
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        }
                    },
                    0,
                    1,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    request(
        vec![unit_type(1), pointer],
        SemanticTypeIdV1::from_index(1),
        SemanticAbiPassModeV1::Direct(initialized_attrs(matches!(
            kind,
            SemanticPointerKindV1::Reference
        ))),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
    .semantic_sha256()
}

#[test]
fn raw_and_reference_pointer_semantics_are_distinct() {
    assert_ne!(
        pointer_kind_identity(SemanticPointerKindV1::Raw),
        pointer_kind_identity(SemanticPointerKindV1::Reference)
    );
}

fn union_type(field_count: u64) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(3)),
        layout_identity(3),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            8,
            SemanticFieldsShapeV1::union(field_count).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            17,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(
            SemanticAggregateTypeV1::new(vec![
                SemanticTypeIdV1::from_index(0),
                SemanticTypeIdV1::from_index(1),
            ])
            .unwrap(),
        ),
    )
}

fn exact_rust_cast(size: u64) -> SemanticAbiPassModeV1 {
    let register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, size).unwrap();
    SemanticAbiPassModeV1::cast(
        false,
        SemanticAbiCastV1::new(
            [None; 8],
            None,
            SemanticAbiUniformV1::new(register, size).unwrap(),
            SemanticAbiValueAttributesV1::plain(),
        ),
    )
}

#[test]
fn union_layout_is_distinct_from_struct_layout_and_validates_field_count() {
    let fields = vec![u64_scalar_type(1, 8), u64_scalar_type(2, 8)];
    let mut types = fields.clone();
    types.push(union_type(2));
    let admitted = request(types, SemanticTypeIdV1::from_index(2), exact_rust_cast(8))
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert!(matches!(
        admitted.types()[2].layout().fields(),
        SemanticFieldsShapeV1::Union { field_count: 2 }
    ));

    let mut wrong_count = fields;
    wrong_count.push(union_type(1));
    assert!(matches!(
        request(
            wrong_count,
            SemanticTypeIdV1::from_index(2),
            exact_rust_cast(8),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}
