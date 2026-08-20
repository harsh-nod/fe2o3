use fe2o3_mir_model::semantic_mir_v1::*;

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const AS2_OBJECT_SIZE_BOUND: u64 = 1 << 31;

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
    range: SemanticScalarValidityRangeV1,
) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(primitive, range)
}

fn u32_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                full_range(32),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )
}

fn function_abi() -> SemanticFunctionAbiV1 {
    let initialized_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(U32, SemanticAbiPassModeV1::Direct(initialized_attributes)),
    )
    .unwrap()
}

fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn block(tag: u8, statements: Vec<SemanticStatementV1>) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap()
}

fn request(
    types: Vec<SemanticTypeDeclV1>,
    statics: Vec<SemanticStaticDeclV1>,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> InertSemanticMirRequestV1 {
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(1)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(1)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(1)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        function_abi(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
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

fn assign_static_pointer(pointer_type: SemanticTypeIdV1) -> SemanticStatementV1 {
    let constant = SemanticConstantV1::new(
        pointer_type,
        SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
            0,
            SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(0)),
        )),
    );
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], pointer_type).unwrap(),
            SemanticRvalueV1::new(
                pointer_type,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)),
            ),
        )),
    )
}

fn as2_static_request(size_bytes: u64) -> InertSemanticMirRequestV1 {
    let object_type = SemanticTypeIdV1::from_index(1);
    let pointer_type = SemanticTypeIdV1::from_index(2);
    let object = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size_bytes),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let pointer = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(3)),
        layout_identity(3),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(initialized(
                SemanticBackendPrimitiveV1::pointer(2, 4, 4),
                full_range(32),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                object_type,
                SemanticMutabilityV1::Immutable,
                2,
                32,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    let external = SemanticStaticDeclV1::new(
        SemanticStaticIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        object_type,
        false,
        2,
        SemanticStaticDefinitionV1::ExternalRequired {
            symbol: SemanticLinkSymbolV1::new(b"as2_boundary".to_vec()).unwrap(),
        },
    );

    request(
        vec![u32_type(), object, pointer],
        vec![external],
        vec![
            local(1, U32, SemanticLocalRoleV1::Return),
            local(2, pointer_type, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(1, vec![assign_static_pointer(pointer_type)])],
    )
}

#[test]
fn as2_external_static_object_size_bound_is_exclusive() {
    as2_static_request(AS2_OBJECT_SIZE_BOUND - 1)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert!(matches!(
        as2_static_request(AS2_OBJECT_SIZE_BOUND).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidStatic)
    ));
}

fn fat_reference_niche_request(substitute_variant_primitive: bool) -> InertSemanticMirRequestV1 {
    let pointee_type = SemanticTypeIdV1::from_index(1);
    let fat_reference_type = SemanticTypeIdV1::from_index(2);
    let enum_type = SemanticTypeIdV1::from_index(3);
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let metadata = initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        full_range(64),
    );
    let data = initialized(
        pointer,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let source_niche = SemanticLayoutNicheV1::new(
        0,
        pointer,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();

    let pointee = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::new_with_backend_repr(
            None,
            1,
            SemanticBackendReprV1::memory(false),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let fat_reference = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(3)),
        layout_identity(3),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(data, metadata),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee_type,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    );

    let variant_data = if substitute_variant_primitive {
        initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )
    } else {
        data
    };
    let inhabited_layout = SemanticEnumVariantLayoutV1::from_rustc(
        0,
        16,
        8,
        SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
        SemanticBackendReprV1::scalar_pair(variant_data, metadata),
        Some(source_niche),
        false,
        None,
        8,
        16,
        SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
    )
    .unwrap();
    let empty_layout = SemanticEnumVariantLayoutV1::from_rustc(
        1,
        16,
        8,
        SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
        SemanticBackendReprV1::memory(true),
        None,
        false,
        None,
        8,
        0,
        SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
    )
    .unwrap();
    let tag = initialized(pointer, SemanticScalarValidityRangeV1::new(1, 0));
    let encoding = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0).unwrap(),
        source_niche,
        tag,
        0,
        1,
        1,
        0,
    )
    .unwrap();
    let variants = vec![
        SemanticEnumVariantV1::new(
            0,
            SemanticAggregateTypeV1::new(vec![fat_reference_type]).unwrap(),
        ),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ];
    let enum_decl = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4)),
        layout_identity(4),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            16,
            8,
            SemanticBackendReprV1::scalar_pair(tag, metadata),
            false,
            SemanticEnumLayoutV1::new(
                vec![inhabited_layout, empty_layout],
                SemanticEnumEncodingV1::Niche(encoding),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(U32, variants).unwrap(),
    );

    request(
        vec![u32_type(), pointee, fat_reference, enum_decl],
        vec![],
        vec![
            local(1, U32, SemanticLocalRoleV1::Return),
            local(2, enum_type, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(1, vec![])],
    )
}

#[test]
fn fat_reference_niche_resolves_scalar_pair_first_component() {
    fat_reference_niche_request(false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
}

#[test]
fn enum_variant_rejects_same_size_backend_primitive_substitution() {
    assert!(matches!(
        fat_reference_niche_request(true).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

fn budget_request() -> InertSemanticMirRequestV1 {
    request(
        vec![u32_type()],
        vec![],
        vec![
            local(1, U32, SemanticLocalRoleV1::Return),
            local(2, U32, SemanticLocalRoleV1::Temporary),
            local(3, U32, SemanticLocalRoleV1::Temporary),
            local(4, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(1, vec![]), block(2, vec![]), block(3, vec![])],
    )
}

#[test]
fn local_and_block_identity_scans_have_an_exact_validation_work_boundary() {
    const EXACT_VALIDATION_WORK: u64 = 49;

    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, EXACT_VALIDATION_WORK)
        .unwrap();
    budget_request().admit(exact).unwrap();

    let one_short = SemanticMirLimitsV1::default()
        .with_limit(
            SemanticMirResourceV1::ValidationWork,
            EXACT_VALIDATION_WORK - 1,
        )
        .unwrap();
    assert!(matches!(
        budget_request().admit(one_short),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: EXACT_VALIDATION_WORK,
            max,
        }) if max == EXACT_VALIDATION_WORK - 1
    ));
}

#[test]
fn canonical_bytes_budget_accepts_exact_length_and_rejects_one_short() {
    let encoded_len = u64::try_from(
        budget_request()
            .admit(SemanticMirLimitsV1::default())
            .unwrap()
            .canonical_encoding()
            .len(),
    )
    .unwrap();

    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, encoded_len)
        .unwrap();
    assert_eq!(
        u64::try_from(
            budget_request()
                .admit(exact)
                .unwrap()
                .canonical_encoding()
                .len()
        )
        .unwrap(),
        encoded_len
    );

    let one_short = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, encoded_len - 1)
        .unwrap();
    assert!(matches!(
        budget_request().admit(one_short),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            max,
            ..
        }) if max == encoded_len - 1
    ));
}
