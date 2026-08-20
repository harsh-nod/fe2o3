use fe2o3_mir_model::semantic_mir_v1::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const NEVER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const U8: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const NONZERO_U8: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const OPAQUE_DST: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const RAW_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const SLICE_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const VTABLE_REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const AGGREGATE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
const UNION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
const DIRECT_ENUM: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
const NICHE_ENUM: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
const FUNCTION_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(17);
const CHAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(18);
const I32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(19);
const F64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(20);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(21);
const TYPE_COUNT: usize = 22;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn type_identity(index: usize) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1::from_sha256(bytes(u8::try_from(index + 1).unwrap()))
}

fn layout_identity(tag: u8) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(tag))
}

fn full_range(bits: u16) -> SemanticScalarValidityRangeV1 {
    SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1)
}

fn u8_primitive() -> SemanticBackendPrimitiveV1 {
    SemanticBackendPrimitiveV1::integer(false, 8, 1)
}

fn u32_primitive() -> SemanticBackendPrimitiveV1 {
    SemanticBackendPrimitiveV1::integer(false, 32, 4)
}

fn i32_primitive() -> SemanticBackendPrimitiveV1 {
    SemanticBackendPrimitiveV1::integer(true, 32, 4)
}

fn u64_primitive() -> SemanticBackendPrimitiveV1 {
    SemanticBackendPrimitiveV1::integer(false, 64, 8)
}

fn f64_primitive() -> SemanticBackendPrimitiveV1 {
    SemanticBackendPrimitiveV1::float(64, 8)
}

fn pointer_primitive() -> SemanticBackendPrimitiveV1 {
    SemanticBackendPrimitiveV1::pointer(0, 8, 8)
}

fn initialized(
    primitive: SemanticBackendPrimitiveV1,
    valid_range: SemanticScalarValidityRangeV1,
) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(primitive, valid_range)
}

fn scalar_layout(
    size: u64,
    align: u64,
    primitive: SemanticBackendPrimitiveV1,
    valid_range: SemanticScalarValidityRangeV1,
) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::new_with_backend_repr(
        Some(size),
        align,
        SemanticBackendReprV1::scalar(initialized(primitive, valid_range)),
        false,
    )
    .unwrap()
}

fn declaration(
    id: SemanticTypeIdV1,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(id.index() as usize),
        layout_identity(u8::try_from(id.index() + 1).unwrap()),
        layout,
        shape,
    )
}

fn unit_type() -> SemanticTypeDeclV1 {
    declaration(
        UNIT,
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
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    )
}

fn never_type() -> SemanticTypeDeclV1 {
    declaration(
        NEVER,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Empty,
            SemanticBackendReprV1::memory(true),
            None,
            true,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Never,
    )
}

fn scalar_type(
    id: SemanticTypeIdV1,
    size: u64,
    align: u64,
    primitive: SemanticBackendPrimitiveV1,
    scalar: SemanticScalarTypeV1,
    valid_range: SemanticScalarValidityRangeV1,
) -> SemanticTypeDeclV1 {
    declaration(
        id,
        scalar_layout(size, align, primitive, valid_range),
        SemanticTypeShapeV1::Scalar(scalar),
    )
}

fn validity_scalar_type() -> SemanticTypeDeclV1 {
    let range = SemanticScalarValidityRangeV1::new(1, u8::MAX.into());
    declaration(
        NONZERO_U8,
        scalar_layout(1, 1, u8_primitive(), range),
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 8,
                },
                vec![range],
            )
            .unwrap(),
        ),
    )
}

fn pointer_layout(
    kind: SemanticPointerKindV1,
    metadata: SemanticPointerMetadataV1,
    vtable_uses_noncanonical_address_space: bool,
) -> SemanticTypeLayoutV1 {
    let data_range = match kind {
        SemanticPointerKindV1::Raw => full_range(64),
        SemanticPointerKindV1::Reference => SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    };
    let data = initialized(pointer_primitive(), data_range);
    let backend = match metadata {
        SemanticPointerMetadataV1::None => SemanticBackendReprV1::scalar(data),
        SemanticPointerMetadataV1::SliceLength => {
            SemanticBackendReprV1::scalar_pair(data, initialized(u64_primitive(), full_range(64)))
        }
        SemanticPointerMetadataV1::VTable => {
            let metadata = initialized(
                if vtable_uses_noncanonical_address_space {
                    SemanticBackendPrimitiveV1::pointer(1, 8, 8)
                } else {
                    pointer_primitive()
                },
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            );
            SemanticBackendReprV1::scalar_pair(data, metadata)
        }
    };
    SemanticTypeLayoutV1::new_with_backend_repr(
        Some(if matches!(metadata, SemanticPointerMetadataV1::None) {
            8
        } else {
            16
        }),
        8,
        backend,
        false,
    )
    .unwrap()
}

fn pointer_type(
    id: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    metadata: SemanticPointerMetadataV1,
    vtable_uses_noncanonical_address_space: bool,
) -> SemanticTypeDeclV1 {
    let ty = declaration(
        id,
        pointer_layout(kind, metadata, vtable_uses_noncanonical_address_space),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(pointee, kind, mutability, 0, 64, metadata)
                .unwrap(),
        ),
    );
    if !matches!(metadata, SemanticPointerMetadataV1::VTable) {
        return ty;
    }
    let first = SemanticAbiPointeeInfoV1::new(
        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
        0,
        1,
    )
    .unwrap();
    let second = SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap();
    ty.with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_scalar_pointee_info(Some(first), Some(second)),
    )
}

fn variant_layout(
    index: u32,
    size: u64,
    align: u64,
    offsets: Vec<u64>,
    padding: Vec<SemanticPaddingV1>,
    largest_niche: Option<SemanticLayoutNicheV1>,
) -> SemanticEnumVariantLayoutV1 {
    let mut memory_order = (0..u32::try_from(offsets.len()).unwrap()).collect::<Vec<_>>();
    memory_order.sort_by_key(|source| (offsets[*source as usize], *source));
    SemanticEnumVariantLayoutV1::from_rustc(
        index,
        size,
        align,
        SemanticFieldsShapeV1::arbitrary(offsets.clone(), memory_order).unwrap(),
        SemanticBackendReprV1::memory(true),
        largest_niche,
        false,
        None,
        align,
        100 + u64::from(index),
        SemanticAggregateLayoutV1::new(offsets, padding).unwrap(),
    )
    .unwrap()
}

fn direct_enum_type(payload_offset: u64) -> SemanticTypeDeclV1 {
    let payload_padding = if payload_offset == 4 {
        vec![SemanticPaddingV1::new(1, 3).unwrap()]
    } else {
        vec![]
    };
    let variants = vec![
        SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
    ];
    let layouts = vec![
        variant_layout(
            0,
            8,
            4,
            vec![],
            vec![SemanticPaddingV1::new(1, 7).unwrap()],
            None,
        ),
        variant_layout(1, 8, 4, vec![payload_offset], payload_padding, None),
    ];
    let tag = initialized(u8_primitive(), SemanticScalarValidityRangeV1::new(0, 1));
    declaration(
        DIRECT_ENUM,
        SemanticTypeLayoutV1::enum_layout(
            8,
            4,
            SemanticEnumLayoutV1::new(
                layouts,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(U8, variants).unwrap(),
    )
}

fn niche_enum_type(expected_offset: u64) -> SemanticTypeDeclV1 {
    let valid_range = SemanticScalarValidityRangeV1::new(1, u8::MAX.into());
    let source_niche = SemanticLayoutNicheV1::new(0, u8_primitive(), valid_range).unwrap();
    let variants = vec![
        SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![NONZERO_U8]).unwrap()),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ];
    let layouts = vec![
        variant_layout(0, 1, 1, vec![0], vec![], Some(source_niche)),
        variant_layout(1, 1, 1, vec![], vec![], None),
    ];
    let niche = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(
            vec![SemanticNichePathComponentV1::Field(0)],
            expected_offset,
        )
        .unwrap(),
        source_niche,
        initialized(u8_primitive(), SemanticScalarValidityRangeV1::new(1, 0)),
        0,
        1,
        1,
        0,
    )
    .unwrap();
    declaration(
        NICHE_ENUM,
        SemanticTypeLayoutV1::enum_layout(
            1,
            1,
            SemanticEnumLayoutV1::new(layouts, SemanticEnumEncodingV1::Niche(niche)).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(U8, variants).unwrap(),
    )
}

#[derive(Clone, Copy)]
struct CatalogOptions {
    raw_pointee: SemanticTypeIdV1,
    array_element: SemanticTypeIdV1,
    array_layout_count: u64,
    aggregate_padding_offset: Option<u64>,
    direct_payload_offset: u64,
    niche_expected_offset: u64,
    function_return: SemanticTypeIdV1,
    vtable_uses_noncanonical_address_space: bool,
    slice_element: SemanticTypeIdV1,
    slice_layout_stride: u64,
    slice_layout_count: u64,
    slice_rustc_size: u64,
    slice_alignment: u64,
    slice_backend_sized: bool,
}

impl Default for CatalogOptions {
    fn default() -> Self {
        Self {
            raw_pointee: U32,
            array_element: U8,
            array_layout_count: 3,
            aggregate_padding_offset: Some(1),
            direct_payload_offset: 4,
            niche_expected_offset: 0,
            function_return: U8,
            vtable_uses_noncanonical_address_space: false,
            slice_element: U8,
            slice_layout_stride: 1,
            slice_layout_count: 0,
            slice_rustc_size: 0,
            slice_alignment: 1,
            slice_backend_sized: false,
        }
    }
}

fn catalog_types(options: CatalogOptions) -> Vec<SemanticTypeDeclV1> {
    let aggregate_padding = options
        .aggregate_padding_offset
        .map(|offset| SemanticPaddingV1::new(offset, 3).unwrap())
        .into_iter()
        .collect();
    vec![
        unit_type(),
        never_type(),
        scalar_type(
            U8,
            1,
            1,
            u8_primitive(),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 8,
            },
            full_range(8),
        ),
        scalar_type(
            U32,
            4,
            4,
            u32_primitive(),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
            full_range(32),
        ),
        validity_scalar_type(),
        declaration(
            OPAQUE_DST,
            SemanticTypeLayoutV1::new(None, 1).unwrap(),
            SemanticTypeShapeV1::Opaque,
        ),
        pointer_type(
            RAW_POINTER,
            options.raw_pointee,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            SemanticPointerMetadataV1::None,
            false,
        ),
        pointer_type(
            REFERENCE,
            U32,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::None,
            false,
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        4,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
        pointer_type(
            SLICE_POINTER,
            OPAQUE_DST,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::SliceLength,
            false,
        ),
        pointer_type(
            VTABLE_REFERENCE,
            OPAQUE_DST,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            SemanticPointerMetadataV1::VTable,
            options.vtable_uses_noncanonical_address_space,
        ),
        declaration(
            ARRAY,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                3,
                1,
                SemanticFieldsShapeV1::array(1, options.array_layout_count),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: options.array_element,
                length: 3,
            },
        ),
        declaration(
            TUPLE,
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(
                    vec![4, 0],
                    vec![SemanticPaddingV1::new(5, 3).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U8, U32]).unwrap()),
        ),
        declaration(
            AGGREGATE,
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(vec![0, 4], aggregate_padding).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U8, U32]).unwrap()),
        ),
        declaration(
            UNION,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                4,
                4,
                SemanticFieldsShapeV1::union(2).unwrap(),
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
            SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![U8, U32]).unwrap()),
        ),
        direct_enum_type(options.direct_payload_offset),
        niche_enum_type(options.niche_expected_offset),
        declaration(
            FUNCTION_POINTER,
            scalar_layout(
                8,
                8,
                pointer_primitive(),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            ),
            SemanticTypeShapeV1::FunctionPointer {
                extern_abi: SemanticExternAbiV1::C { unwind: false },
                c_variadic: true,
                arguments: SemanticAggregateTypeV1::new(vec![U32]).unwrap(),
                return_type: options.function_return,
            },
        ),
        scalar_type(
            BOOL,
            1,
            1,
            u8_primitive(),
            SemanticScalarTypeV1::Bool,
            SemanticScalarValidityRangeV1::new(0, 1),
        ),
        scalar_type(
            CHAR,
            4,
            4,
            u32_primitive(),
            SemanticScalarTypeV1::Char,
            SemanticScalarValidityRangeV1::new(0, 0x10_ffff),
        ),
        scalar_type(
            I32,
            4,
            4,
            i32_primitive(),
            SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            },
            full_range(32),
        ),
        scalar_type(
            F64,
            8,
            8,
            f64_primitive(),
            SemanticScalarTypeV1::Float { bits: 64 },
            full_range(64),
        ),
        declaration(
            SLICE,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                options.slice_rustc_size,
                options.slice_alignment,
                SemanticFieldsShapeV1::array(
                    options.slice_layout_stride,
                    options.slice_layout_count,
                ),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(options.slice_backend_sized),
                None,
                false,
                None,
                options.slice_alignment,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Slice {
                element: options.slice_element,
            },
        ),
    ]
}

fn direct_abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn request_with_types(types: Vec<SemanticTypeDeclV1>) -> InertSemanticMirRequestV1 {
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(1)),
        U8,
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )];
    locals.extend((0..types.len()).map(|index| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(u8::try_from(index + 2).unwrap())),
            SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()),
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        )
    }));
    let function_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(200),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        direct_abi_value(U8),
    )
    .unwrap();
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
        function_abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn catalog_request(options: CatalogOptions) -> InertSemanticMirRequestV1 {
    request_with_types(catalog_types(options))
}

#[test]
fn canonical_catalog_covers_every_semantic_type_and_exact_layout_form() {
    let admitted = catalog_request(CatalogOptions::default())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let types = admitted.types();
    assert_eq!(types.len(), TYPE_COUNT);

    assert!(matches!(
        types[UNIT.index() as usize].shape(),
        SemanticTypeShapeV1::Unit
    ));
    assert!(matches!(
        types[NEVER.index() as usize].shape(),
        SemanticTypeShapeV1::Never
    ));
    assert!(matches!(
        types[U8.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 8
        })
    ));
    assert!(matches!(
        types[BOOL.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    ));
    assert!(matches!(
        types[CHAR.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char)
    ));
    assert!(matches!(
        types[I32.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32
        })
    ));
    assert!(matches!(
        types[F64.index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 64 })
    ));
    let SemanticTypeShapeV1::ValidityScalar(validity) = types[NONZERO_U8.index() as usize].shape()
    else {
        panic!("validity scalar shape was not retained");
    };
    assert_eq!(
        validity.valid_ranges(),
        &[SemanticScalarValidityRangeV1::new(1, u8::MAX.into())]
    );
    assert!(matches!(
        types[OPAQUE_DST.index() as usize].shape(),
        SemanticTypeShapeV1::Opaque
    ));
    assert_eq!(
        types[OPAQUE_DST.index() as usize].layout().size_bytes(),
        None
    );

    let pointer = |id: SemanticTypeIdV1| {
        let SemanticTypeShapeV1::Pointer(pointer) = types[id.index() as usize].shape() else {
            panic!("pointer shape was not retained for type {}", id.index());
        };
        pointer
    };
    assert_eq!(pointer(RAW_POINTER).kind(), SemanticPointerKindV1::Raw);
    assert_eq!(pointer(REFERENCE).kind(), SemanticPointerKindV1::Reference);
    assert_eq!(
        pointer(SLICE_POINTER).metadata(),
        SemanticPointerMetadataV1::SliceLength
    );
    assert_eq!(
        pointer(VTABLE_REFERENCE).metadata(),
        SemanticPointerMetadataV1::VTable
    );
    assert_eq!(
        types[RAW_POINTER.index() as usize].layout().size_bytes(),
        Some(8)
    );
    assert_eq!(
        types[SLICE_POINTER.index() as usize].layout().size_bytes(),
        Some(16)
    );

    assert!(matches!(
        types[ARRAY.index() as usize].layout().fields(),
        SemanticFieldsShapeV1::Array {
            stride_bytes: 1,
            count: 3
        }
    ));
    assert!(matches!(
        types[SLICE.index() as usize].shape(),
        SemanticTypeShapeV1::Slice { element } if *element == U8
    ));
    assert_eq!(types[SLICE.index() as usize].layout().size_bytes(), None);
    assert!(matches!(
        types[SLICE.index() as usize].layout().fields(),
        SemanticFieldsShapeV1::Array {
            stride_bytes: 1,
            count: 0
        }
    ));
    assert!(matches!(
        types[TUPLE.index() as usize].layout().fields(),
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } if source_order_offsets_bytes.as_ref() == [4, 0]
            && memory_order_source_indices.as_ref() == [1, 0]
    ));
    let SemanticTypeLayoutDetailsV1::Aggregate(aggregate) =
        types[AGGREGATE.index() as usize].layout().details()
    else {
        panic!("aggregate layout details were not retained");
    };
    assert_eq!(aggregate.field_offsets(), &[0, 4]);
    assert_eq!(
        aggregate.padding(),
        &[SemanticPaddingV1::new(1, 3).unwrap()]
    );
    assert!(matches!(
        types[UNION.index() as usize].layout().fields(),
        SemanticFieldsShapeV1::Union { field_count: 2 }
    ));

    let SemanticRustcVariantsV1::Multiple(direct_layout) =
        types[DIRECT_ENUM.index() as usize].layout().variants()
    else {
        panic!("direct enum layout was not retained");
    };
    assert!(matches!(
        direct_layout.encoding(),
        SemanticEnumEncodingV1::Direct(encoding)
            if encoding.tag_field() == 0 && encoding.tag_offset_bytes() == 0
    ));
    assert_eq!(
        direct_layout.variants()[1].aggregate().field_offsets(),
        &[4]
    );

    let SemanticRustcVariantsV1::Multiple(niche_layout) =
        types[NICHE_ENUM.index() as usize].layout().variants()
    else {
        panic!("niche enum layout was not retained");
    };
    let SemanticEnumEncodingV1::Niche(niche) = niche_layout.encoding() else {
        panic!("niche enum encoding was not retained");
    };
    assert_eq!(
        niche.source().path(),
        &[SemanticNichePathComponentV1::Field(0)]
    );
    assert_eq!(niche.niche_variant_range(), (1, 1));
    assert_eq!(niche.niche_start(), 0);

    assert!(matches!(
        types[FUNCTION_POINTER.index() as usize].shape(),
        SemanticTypeShapeV1::FunctionPointer {
            extern_abi: SemanticExternAbiV1::C { unwind: false },
            c_variadic: true,
            arguments,
            return_type,
        } if arguments.fields() == [U32] && *return_type == U8
    ));
}

#[test]
fn canonical_encoding_is_pinned_deterministic_and_semantically_complete() {
    let left = catalog_request(CatalogOptions::default())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let right = catalog_request(CatalogOptions::default())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(left.canonical_encoding(), right.canonical_encoding());
    assert_eq!(left.semantic_sha256(), right.semantic_sha256());
    assert!(
        left.canonical_encoding()
            .starts_with(b"fe2o3.inert-semantic-mir\x01\x00")
    );
    assert_eq!(
        left.semantic_sha256().as_bytes(),
        &[
            0x75, 0x42, 0x1a, 0x55, 0x19, 0xa9, 0x5e, 0x4e, 0xf2, 0xb2, 0xdb, 0x74, 0x1f, 0x06,
            0x03, 0xf9, 0xde, 0x7c, 0xfe, 0x35, 0x43, 0xd0, 0x27, 0x8e, 0xc2, 0x6b, 0x6f, 0x66,
            0x24, 0x40, 0x78, 0x6b,
        ]
    );

    let without_explicit_padding = catalog_request(CatalogOptions {
        aggregate_padding_offset: None,
        ..CatalogOptions::default()
    })
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(
        left.canonical_encoding(),
        without_explicit_padding.canonical_encoding()
    );

    let different_slice_element = catalog_request(CatalogOptions {
        slice_element: NONZERO_U8,
        ..CatalogOptions::default()
    })
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(
        left.canonical_encoding(),
        different_slice_element.canonical_encoding()
    );

    let mut out_of_order = catalog_types(CatalogOptions::default());
    out_of_order.swap(0, 1);
    assert!(matches!(
        request_with_types(out_of_order).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::NonDeterministicOrder {
            entity: SemanticMirEntityV1::Type
        })
    ));
}

#[test]
fn malformed_type_references_fail_closed_at_the_type_boundary() {
    for options in [
        CatalogOptions {
            raw_pointee: SemanticTypeIdV1::from_index(99),
            ..CatalogOptions::default()
        },
        CatalogOptions {
            array_element: SemanticTypeIdV1::from_index(99),
            ..CatalogOptions::default()
        },
        CatalogOptions {
            slice_element: SemanticTypeIdV1::from_index(99),
            ..CatalogOptions::default()
        },
        CatalogOptions {
            function_return: SemanticTypeIdV1::from_index(99),
            ..CatalogOptions::default()
        },
    ] {
        assert!(matches!(
            catalog_request(options).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidReference {
                reference: SemanticMirReferenceV1::Type,
                index: 99,
                bound,
                location: SemanticMirLocationV1::Type(_),
            }) if bound == TYPE_COUNT as u32
        ));
    }
}

#[test]
fn malformed_layout_relationships_and_metadata_fail_closed() {
    let malformed = [
        CatalogOptions {
            array_layout_count: 2,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            aggregate_padding_offset: Some(0),
            ..CatalogOptions::default()
        },
        CatalogOptions {
            direct_payload_offset: 0,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            niche_expected_offset: 1,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            vtable_uses_noncanonical_address_space: true,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            slice_layout_stride: 2,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            slice_layout_count: 1,
            slice_rustc_size: 1,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            slice_alignment: 2,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            slice_backend_sized: true,
            ..CatalogOptions::default()
        },
        CatalogOptions {
            slice_element: OPAQUE_DST,
            ..CatalogOptions::default()
        },
    ];
    for options in malformed {
        assert!(matches!(
            catalog_request(options).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeLayout)
        ));
    }

    let integer = SemanticScalarTypeV1::Integer {
        signed: false,
        bits: 8,
    };
    for ranges in [
        vec![],
        vec![SemanticScalarValidityRangeV1::new(5, 4)],
        vec![
            SemanticScalarValidityRangeV1::new(1, 4),
            SemanticScalarValidityRangeV1::new(4, 8),
        ],
        vec![SemanticScalarValidityRangeV1::new(0, 256)],
    ] {
        assert!(matches!(
            SemanticValidityScalarTypeV1::new(integer, ranges),
            Err(SemanticMirErrorV1::InvalidTypeLayout)
        ));
    }
}

#[test]
fn schema_limits_bound_counts_work_and_canonical_bytes() {
    let admitted = catalog_request(CatalogOptions::default())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let canonical_len = admitted.canonical_encoding().len() as u64;

    let type_limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Types, TYPE_COUNT as u64 - 1)
        .unwrap();
    assert!(matches!(
        catalog_request(CatalogOptions::default()).admit(type_limits),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Types,
            actual,
            max,
        }) if actual == TYPE_COUNT as u64 && max == TYPE_COUNT as u64 - 1
    ));

    let work_limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, 0)
        .unwrap();
    assert!(matches!(
        catalog_request(CatalogOptions::default()).admit(work_limits),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));

    let canonical_limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, canonical_len - 1)
        .unwrap();
    assert!(matches!(
        catalog_request(CatalogOptions::default()).admit(canonical_limits),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));

    assert!(matches!(
        SemanticMirLimitsV1::default().with_limit(
            SemanticMirResourceV1::Types,
            HARD_MAX_TYPES_V1 + 1,
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Types,
            actual,
            max: HARD_MAX_TYPES_V1,
        }) if actual == HARD_MAX_TYPES_V1 + 1
    ));
}
