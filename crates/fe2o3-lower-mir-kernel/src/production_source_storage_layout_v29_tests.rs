use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

pub(super) fn original_cycle_types_v29(
    count: usize,
    descriptor: bool,
) -> (Vec<SemanticTypeDeclV1>, SemanticTypeIdV1) {
    assert!(count > 0);
    let mut declarations = types();
    let base = declarations.len();
    let width = if descriptor { 3 } else { 2 };
    let id = |index: usize| SemanticTypeIdV1::from_index(u32::try_from(index).unwrap());
    for index in 0..count {
        let own = base + index * width;
        let next = base + ((index + 1) % count) * width;
        let size = if descriptor { 16 } else { 8 };
        declarations.push(aggregate(1, size, 8, vec![id(own + 1)], vec![0], vec![]));
        declarations.push(pointer(
            1,
            id(if descriptor { own + 2 } else { next }),
            false,
            descriptor,
        ));
        if descriptor {
            declarations.push(array(1, id(next), 0, size, 8, false));
        }
    }
    for (ordinal, row) in declarations.iter_mut().enumerate().skip(base) {
        let mut identity = [239; 32];
        identity[24..].copy_from_slice(&(ordinal as u64).to_be_bytes());
        *row = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity),
            SemanticLayoutIdentityV1::from_sha256(identity),
            row.layout().clone(),
            row.shape().clone(),
        );
    }
    (declarations, id(base))
}

#[test]
fn original_extension_closes_source_rows_without_rebinding_original_geometry() {
    for ty in [
        RAW,
        PAIR,
        ARRAY,
        HUGE,
        TWO_DESCRIPTORS,
        RECURSIVE,
        DIRECT,
        NICHE,
    ] {
        let owner = owner();
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(251).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[WORD], &mut budget).unwrap();
        let keys = layouts.keys.clone();
        let original_word = layouts.row_for(&owner, WORD, &mut budget).unwrap();
        assert_eq!(
            layouts.original_schema(&owner, ty, &mut budget).unwrap(),
            None
        );
        let schema = layouts
            .select_original_closure(&owner, ty, &mut budget)
            .unwrap();
        layouts
            .check_selected_schema(&owner, ty, schema, &mut budget)
            .unwrap();
        assert_eq!(
            layouts.original_schema(&owner, ty, &mut budget).unwrap(),
            None
        );
        assert!(layouts.row_for(&owner, ty, &mut budget).is_err());
        assert_eq!(layouts.keys, keys);
        assert_eq!(
            layouts.row_for(&owner, WORD, &mut budget).unwrap(),
            original_word
        );
        let rows = layouts.rows(&owner, &mut budget).unwrap();
        let declaration = &owner.source_semantic().types()[ty.index() as usize];
        assert_eq!(
            rows[schema.0 as usize].size,
            declaration.layout().size_bytes().unwrap()
        );
        assert_eq!(
            u64::from(rows[schema.0 as usize].alignment),
            declaration.layout().alignment_bytes()
        );
        fe2o3_kernel_ir::check_storage_layouts_v1(&rows, layouts.limits, &mut budget).unwrap();
        let count = rows.len();
        drop(rows);
        let credit = layouts.lease.persistent.get();
        assert_eq!(
            layouts
                .select_original_closure(&owner, ty, &mut budget)
                .unwrap(),
            schema
        );
        assert_eq!(layouts.physical.borrow().rows.len(), count);
        assert_eq!(layouts.lease.persistent.get(), credit);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 251);
    }
}

#[test]
fn absent_original_pointer_and_descriptor_cycles_are_finite_source_closed_groups() {
    for descriptor in [false, true] {
        for count in [1, 2, 97] {
            let (declarations, root) = original_cycle_types_v29(count, descriptor);
            let owner = owner_with(declarations);
            let mut work = work();
            let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
            budget.reserve_storage(257).unwrap();
            let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
            let schema = layouts
                .select_original_closure(&owner, root, &mut budget)
                .unwrap();
            assert!(layouts.keys.is_empty());
            assert_eq!(
                layouts.physical.borrow().rows.len(),
                count * if descriptor { 4 } else { 2 }
            );
            let rows = layouts.rows(&owner, &mut budget).unwrap();
            fe2o3_kernel_ir::check_storage_layouts_v1(&rows, layouts.limits, &mut budget).unwrap();
            let mut current = schema;
            for _ in 0..count {
                let StorageLayoutKindV1::Record(fields) = &rows[current.0 as usize].kind else {
                    panic!("source record");
                };
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].offset, 0);
                let pointer = if descriptor {
                    let StorageLayoutKindV1::Slice {
                        element,
                        data,
                        length,
                        value_space,
                        access,
                    } = rows[fields[0].layout.0 as usize].kind
                    else {
                        panic!("source descriptor");
                    };
                    assert_eq!(value_space, AddressSpace::Generic);
                    assert_eq!(access, AccessMode::ReadOnly);
                    assert_eq!(length.offset, 8);
                    assert_eq!(
                        rows[length.layout.0 as usize].kind,
                        StorageLayoutKindV1::Scalar(ScalarType::Index)
                    );
                    let StorageLayoutKindV1::Pointer(pointer) = rows[data.layout.0 as usize].kind
                    else {
                        panic!("descriptor data");
                    };
                    assert_eq!(pointer.pointee, element);
                    pointer
                } else {
                    let StorageLayoutKindV1::Pointer(pointer) =
                        rows[fields[0].layout.0 as usize].kind
                    else {
                        panic!("raw pointer");
                    };
                    pointer
                };
                assert_eq!(pointer.value_space, AddressSpace::Generic);
                assert_eq!(pointer.encoded_space, AddressSpace::Generic);
                assert_eq!(pointer.access, AccessMode::ReadOnly);
                assert_eq!(pointer.stored_bits, 64);
                current = pointer.pointee;
            }
            assert_eq!(current, schema);
            drop(rows);
            layouts.release(&mut budget).unwrap();
            assert_eq!(budget.storage(), 257);
        }
    }
}

#[test]
fn original_extension_reuses_selected_original_leaves_under_exact_final_row_policy() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let mut limits = ProductionSemanticKirLimitsV1::default().storage_layout_limits();
    limits.rows = 2;
    let layouts =
        SourceStorageLayoutsV29::new_with_limits(&owner, &[], limits, &mut budget).unwrap();
    let word = layouts
        .select_original_leaf_schema(&owner, WORD, &mut budget)
        .unwrap();
    let pointer = layouts
        .select_original_closure(&owner, RAW, &mut budget)
        .unwrap();
    assert_eq!(layouts.physical.borrow().rows.len(), 2);
    assert!(
        matches!(layouts.physical.borrow().rows[pointer.0 as usize].kind,
        StorageLayoutKindV1::Pointer(row) if row.pointee == word)
    );
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn original_table_dag_is_bounded_by_type_rows_not_repeated_object_bytes() {
    const DEPTH: usize = 48;
    let mut source = types();
    let mut element = WORD;
    let mut size = 8;
    for index in 0..DEPTH {
        let ty = SemanticTypeIdV1::from_index(source.len() as u32);
        source.push(array(201 + index as u8, element, 2, size, 8, true));
        element = ty;
        size *= 2;
    }
    let source_count = source.len();
    let owner = owner_with(source);
    let mut work =
        CanonicalKernelIrWorkBudgetV1::new(100 * (DEPTH + 1) * (DEPTH + 1) + source_count);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts = SourceStorageLayoutsV29::new(&owner, &[element], &mut budget).unwrap();
    assert_eq!(layouts.keys.len(), DEPTH + 1);
    let last = layouts.row_for(&owner, element, &mut budget).unwrap();
    assert_eq!(
        layouts.rows(&owner, &mut budget).unwrap()[last.0 as usize].size,
        size
    );
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

pub(super) const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
pub(super) const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
pub(super) const BYTE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
pub(super) const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
pub(super) const RAW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
pub(super) const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
pub(super) const PACKED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
pub(super) const REORDERED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
pub(super) const UNION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
pub(super) const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
pub(super) const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
pub(super) const DESCRIPTOR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
pub(super) const RECURSIVE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
pub(super) const LINK: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
pub(super) const TWO_DESCRIPTORS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
pub(super) const TWO_ZST: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
pub(super) const HUGE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);
pub(super) const DIRECT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(17);
pub(super) const NICHE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(18);
pub(super) const SIGNED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(19);
pub(super) const BYTE_OCTET: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(20);
pub(super) const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

pub(super) fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn integer(bits: u16) -> SemanticTypeLayoutV1 {
    let bytes = u64::from(bits / 8);
    SemanticTypeLayoutV1::new_with_backend_repr(
        Some(bytes),
        bytes,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, bits, bytes),
            SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
        )),
        false,
    )
    .unwrap()
}

fn pointer(tag: u8, pointee: SemanticTypeIdV1, reference: bool, slice: bool) -> SemanticTypeDeclV1 {
    let data = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(u128::from(reference), u128::from(u64::MAX)),
    );
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
    );
    let declaration = declaration(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(if slice { 16 } else { 8 }),
            8,
            if slice {
                SemanticBackendReprV1::scalar_pair(data, length)
            } else {
                SemanticBackendReprV1::scalar(data)
            },
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                if reference {
                    SemanticPointerKindV1::Reference
                } else {
                    SemanticPointerKindV1::Raw
                },
                SemanticMutabilityV1::Immutable,
                0,
                64,
                if slice {
                    SemanticPointerMetadataV1::SliceLength
                } else {
                    SemanticPointerMetadataV1::None
                },
            )
            .unwrap(),
        ),
    );
    if reference {
        declaration.with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        if slice { 0 } else { 8 },
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        )
    } else {
        declaration
    }
}

fn aggregate(
    tag: u8,
    size: u64,
    alignment: u64,
    fields: Vec<SemanticTypeIdV1>,
    offsets: Vec<u64>,
    padding: Vec<SemanticPaddingV1>,
) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            alignment,
            SemanticAggregateLayoutV1::new(offsets, padding).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn array(
    tag: u8,
    element: SemanticTypeIdV1,
    count: u64,
    stride: u64,
    alignment: u64,
    sized: bool,
) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            stride.checked_mul(count).unwrap(),
            alignment,
            SemanticFieldsShapeV1::array(stride, count),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(sized),
            None,
            false,
            None,
            alignment,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        if sized {
            SemanticTypeShapeV1::Array {
                element,
                length: count,
            }
        } else {
            SemanticTypeShapeV1::Slice { element }
        },
    )
}

pub(super) fn types() -> Vec<SemanticTypeDeclV1> {
    vec![
        declaration(
            1,
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
        ),
        declaration(
            2,
            integer(64),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
        declaration(
            3,
            integer(8),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 8,
            }),
        ),
        pointer(4, WORD, true, false),
        pointer(5, WORD, false, false),
        aggregate(
            6,
            16,
            8,
            vec![WORD, BYTE],
            vec![0, 8],
            vec![SemanticPaddingV1::new(9, 7).unwrap()],
        ),
        // Byte-packed geometry, not an unaligned u64 or repr(packed) admission.
        aggregate(7, 9, 1, vec![BYTE, BYTE_OCTET], vec![0, 1], vec![]),
        aggregate(
            8,
            16,
            8,
            vec![BYTE, WORD],
            vec![8, 0],
            vec![SemanticPaddingV1::new(9, 7).unwrap()],
        ),
        declaration(
            9,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                8,
                8,
                SemanticFieldsShapeV1::Union { field_count: 2 },
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![WORD, BYTE]).unwrap()),
        ),
        array(10, PAIR, 4, 16, 8, true),
        array(11, WORD, 0, 8, 8, false),
        pointer(12, SLICE, false, true),
        aggregate(13, 8, 8, vec![LINK], vec![0], vec![]),
        pointer(14, RECURSIVE, false, false),
        aggregate(15, 32, 8, vec![DESCRIPTOR, DESCRIPTOR], vec![0, 16], vec![]),
        aggregate(16, 0, 1, vec![UNIT, UNIT], vec![0, 0], vec![]),
        array(17, WORD, u64::from(u32::MAX), 8, 8, true),
        direct_enum(),
        niche_enum(),
        declaration(
            20,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(1),
                1,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(true, 8, 1),
                    SemanticScalarValidityRangeV1::new(0, 255),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 8,
            }),
        ),
        array(21, BYTE, 8, 1, 1, true),
    ]
}

fn direct_enum() -> SemanticTypeDeclV1 {
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(true, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let variants = (0..2)
        .map(|index| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(vec![8], vec![0]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                20 + u64::from(index),
                SemanticAggregateLayoutV1::new(
                    vec![8],
                    vec![SemanticPaddingV1::new(1, 7).unwrap()],
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    declaration(
        18,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            16,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            SIGNED,
            vec![
                SemanticEnumVariantV1::new(255, SemanticAggregateTypeV1::new(vec![WORD]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![WORD]).unwrap()),
            ],
        )
        .unwrap(),
    )
}

fn niche_enum() -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let nonnull = SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX));
    let niche = SemanticLayoutNicheV1::new(0, pointer, nonnull).unwrap();
    let variants = vec![
        SemanticEnumVariantLayoutV1::from_rustc(
            0,
            8,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            30,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticEnumVariantLayoutV1::from_rustc(
            1,
            8,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(pointer, nonnull)),
            Some(niche),
            false,
            None,
            8,
            31,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
    ];
    let tag =
        SemanticBackendScalarV1::initialized(pointer, SemanticScalarValidityRangeV1::new(1, 0));
    declaration(
        19,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            8,
            SemanticBackendReprV1::scalar(tag),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Niche(
                    SemanticNicheEnumEncodingV1::new(
                        0,
                        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0)
                            .unwrap(),
                        niche,
                        tag,
                        1,
                        0,
                        0,
                        0,
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            BYTE,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![REFERENCE]).unwrap(),
                ),
            ],
        )
        .unwrap(),
    )
}

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}

pub(super) fn request(types: Vec<SemanticTypeDeclV1>) -> InertSemanticMirRequestV1 {
    request_with_roots(types, false)
}

fn request_with_roots(
    types: Vec<SemanticTypeDeclV1>,
    multiple_roots: bool,
) -> InertSemanticMirRequestV1 {
    let target = SemanticLayoutIdentityV1::from_sha256([250; 32]);
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([40; 32]),
        target,
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            WORD,
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let mut locals = vec![
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([50; 32]),
            UNIT,
            SemanticLocalRoleV1::Return,
            source(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([51; 32]),
            WORD,
            SemanticLocalRoleV1::Argument(0),
            source(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([52; 32]),
            WORD,
            SemanticLocalRoleV1::Temporary,
            source(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([53; 32]),
            REFERENCE,
            SemanticLocalRoleV1::Temporary,
            source(),
        ),
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([54; 32]),
            REFERENCE,
            SemanticLocalRoleV1::Temporary,
            source(),
        ),
    ];
    for index in 0..types.len() {
        if index == SLICE.index() as usize {
            continue;
        }
        let mut identity = [60; 32];
        identity[24..].copy_from_slice(&u64::try_from(index).unwrap().to_be_bytes());
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity),
            SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()),
            SemanticLocalRoleV1::Temporary,
            source(),
        ));
    }
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([90; 32]),
        source(),
        vec![
            assign(
                2,
                WORD,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
            ),
            assign(
                3,
                REFERENCE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(1, WORD),
                },
            ),
            assign(
                4,
                REFERENCE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(2, WORD),
                },
            ),
            assign(
                0,
                UNIT,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ),
        ],
        SemanticTerminatorV1::new(source(), SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([100; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([101; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([102; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([103; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([104; 32]),
        source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"storage_domain_source_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([105; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let mut functions = vec![function];
    let mut callables = vec![SemanticCallableDeclV1::defined(ROOT)];
    let mut roots = vec![ROOT];
    if multiple_roots {
        let first = &functions[0];
        let second = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([110; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([111; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([112; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([113; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([114; 32]),
            source(),
            first.abi().clone(),
            first.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            first.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"storage_domain_second_root".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([115; 32]),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        ));
        functions.push(second);
        callables.push(SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(1),
        ));
        roots.push(SemanticFunctionIdV1::from_index(1));
    }
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(target),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        roots,
    )
    .unwrap()
}

pub(super) fn owner_with(types: Vec<SemanticTypeDeclV1>) -> ProductionSemanticSsaOwnerV1 {
    let admitted = request(types)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let mut owner =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    owner
}
pub(super) fn owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(types())
}
pub(super) fn local_for(ty: SemanticTypeIdV1) -> SemanticLocalIdV1 {
    assert_ne!(ty, SLICE);
    SemanticLocalIdV1::from_index(5 + ty.index() - u32::from(ty.index() > SLICE.index()))
}
pub(super) fn work() -> CanonicalKernelIrWorkBudgetV1 {
    CanonicalKernelIrWorkBudgetV1::new(20_000_000)
}

pub(super) struct SelectedNicheFixtureV29 {
    pub owner: ProductionSemanticSsaOwnerV1,
    pub mixed: SemanticTypeIdV1,
    pub pointer: SemanticTypeIdV1,
    pub nested: Option<(SemanticTypeIdV1, SemanticTypeIdV1)>,
    pub enumeration: SemanticTypeIdV1,
    pub slice: bool,
}

pub(super) fn selected_niche_fixture_v29(slice: bool, nested: bool) -> SelectedNicheFixtureV29 {
    let mut declarations = types();
    let mut append = |ty| {
        let id = SemanticTypeIdV1::from_index(declarations.len() as u32);
        declarations.push(ty);
        id
    };
    let marker = append(declaration(
        121,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    let context = append(
        declaration(
            122,
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![0; 5], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![marker; 5]).unwrap()),
        )
        .with_rust_type_kind(SemanticRustTypeKindV1::Execution(
            SemanticExecutionRoleV29::KernelContext,
        )),
    );
    let mixed = append(aggregate(
        123,
        8,
        8,
        vec![context, WORD],
        vec![0, 0],
        vec![],
    ));
    let thin = append(pointer(124, mixed, true, false));
    drop(append);
    if slice {
        declarations[SLICE.index() as usize] = array(11, mixed, 0, 8, 8, false);
        declarations[DESCRIPTOR.index() as usize] = pointer(12, SLICE, true, true);
    }
    let pointer = if slice { DESCRIPTOR } else { thin };
    let pointer_bytes = if slice { 16 } else { 8 };
    let mut path = vec![SemanticNichePathComponentV1::Field(1)];
    let (payload, nested, size, offset) = if nested {
        let tuple = SemanticTypeIdV1::from_index(declarations.len() as u32);
        declarations.push(aggregate(
            125,
            pointer_bytes,
            8,
            vec![pointer],
            vec![0],
            vec![],
        ));
        let array_id = SemanticTypeIdV1::from_index(declarations.len() as u32);
        declarations.push(array(126, tuple, 2, pointer_bytes, 8, true));
        path.extend([
            SemanticNichePathComponentV1::ArrayElement(1),
            SemanticNichePathComponentV1::Field(0),
        ]);
        (
            array_id,
            Some((tuple, array_id)),
            2 * pointer_bytes,
            pointer_bytes,
        )
    } else {
        (pointer, None, pointer_bytes, 0)
    };
    let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let nonnull = SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX));
    let source_niche = SemanticLayoutNicheV1::new(offset, primitive, nonnull).unwrap();
    let tag =
        SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(1, 0));
    let variants = vec![
        SemanticEnumVariantLayoutV1::from_rustc(
            0,
            size,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            127,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticEnumVariantLayoutV1::from_rustc(
            1,
            size,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![0, 0], vec![0, 1]).unwrap(),
            SemanticBackendReprV1::memory(true),
            Some(source_niche),
            false,
            None,
            8,
            128,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
    ];
    let enumeration = SemanticTypeIdV1::from_index(declarations.len() as u32);
    declarations.push(declaration(
        129,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            size,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Niche(
                    SemanticNicheEnumEncodingV1::new(
                        0,
                        SemanticNicheSourceV1::new(path, offset).unwrap(),
                        source_niche,
                        tag,
                        1,
                        0,
                        0,
                        0,
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            BYTE,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![context, payload]).unwrap(),
                ),
            ],
        )
        .unwrap(),
    ));
    let admitted = request(declarations)
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    SelectedNicheFixtureV29 {
        owner,
        mixed,
        pointer,
        nested,
        enumeration,
        slice,
    }
}

#[test]
fn selected_niche_fixture_retains_admitted_original_path_validity_and_mixed_nominal_identity() {
    for slice in [false, true] {
        for nested in [false, true] {
            let fixture = selected_niche_fixture_v29(slice, nested);
            let types = fixture.owner.source_semantic().types();
            let SemanticTypeShapeV1::Tuple(fields) = types[fixture.mixed.index() as usize].shape()
            else {
                panic!("mixed source record");
            };
            assert_eq!(
                execution_cfg_nominal_kind_v29(types, fields.fields()[0]).unwrap(),
                Some(false)
            );
            assert_eq!(fields.fields()[1], WORD);
            let SemanticRustcVariantsV1::Multiple(layout) = types
                [fixture.enumeration.index() as usize]
                .layout()
                .variants()
            else {
                panic!("original enum");
            };
            let SemanticEnumEncodingV1::Niche(niche) = layout.encoding() else {
                panic!("original niche");
            };
            assert_eq!(
                niche.source_niche().valid_range(),
                SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX))
            );
            assert_eq!(
                niche.source().path()[0],
                SemanticNichePathComponentV1::Field(1)
            );
            assert_eq!(niche.source().path().len(), if nested { 3 } else { 1 });
            assert_eq!(
                niche.source().expected_offset_bytes(),
                if nested {
                    if slice { 16 } else { 8 }
                } else {
                    0
                }
            );
        }
    }
}

#[test]
fn selected_schema_projection_preserves_original_field_identity_and_geometry() {
    let owner = owner();
    let foreign = owner_with(types());
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(109).unwrap();
    let layouts = SourceStorageLayoutsV29::new(&owner, &[TWO_DESCRIPTORS], &mut budget).unwrap();
    let element = layouts.row_for(&owner, WORD, &mut budget).unwrap();
    let generic = layouts.row_for(&owner, DESCRIPTOR, &mut budget).unwrap();
    let global = layouts
        .select_schema(
            &owner,
            DESCRIPTOR,
            SourceStorageSelectionV29::Slice {
                element,
                value_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
            },
            &mut budget,
        )
        .unwrap();
    let schema = layouts
        .select_schema(
            &owner,
            TWO_DESCRIPTORS,
            SourceStorageSelectionV29::Aggregate {
                variant: None,
                fields: &[(0, global), (1, generic)],
            },
            &mut budget,
        )
        .unwrap();
    for (field, expected) in [(0, global), (1, generic)] {
        let path = [
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), DESCRIPTOR).unwrap(),
        ];
        assert_eq!(
            layouts
                .project_selected_schema(&owner, TWO_DESCRIPTORS, schema, &path, &mut budget)
                .unwrap(),
            (DESCRIPTOR, expected)
        );
        let logical = layouts
            .projected_subobject(&owner, TWO_DESCRIPTORS, &path, &mut budget)
            .unwrap();
        assert_eq!(
            (logical.range.start, logical.range.length()),
            (16 * u64::from(field), 16)
        );
        drop(logical);
    }
    let path = [SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), DESCRIPTOR).unwrap()];
    assert!(
        layouts
            .project_selected_schema(&foreign, TWO_DESCRIPTORS, schema, &path, &mut budget)
            .is_err()
    );
    assert!(
        layouts
            .project_selected_schema(&owner, TWO_DESCRIPTORS, global, &path, &mut budget)
            .is_err()
    );
    for (kind, ty) in [
        (SemanticProjectionKindV1::Field(0), WORD),
        (SemanticProjectionKindV1::Field(2), DESCRIPTOR),
        (SemanticProjectionKindV1::Downcast(0), TWO_DESCRIPTORS),
    ] {
        let invalid = [SemanticProjectionV1::new(kind, ty).unwrap()];
        assert!(
            layouts
                .project_selected_schema(&owner, TWO_DESCRIPTORS, schema, &invalid, &mut budget)
                .is_err()
        );
    }
    assert_eq!(
        layouts
            .project_selected_schema(&owner, TWO_DESCRIPTORS, schema, &path, &mut budget)
            .unwrap(),
        (DESCRIPTOR, global)
    );
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 109);
}

#[test]
fn complete_source_fixture_roster_admits_with_byte_packed_geometry() {
    for multiple_roots in [false, true] {
        request_with_roots(types(), multiple_roots)
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
    }
}

#[test]
fn original_misaligned_word_tuple_is_rejected_at_source_admission() {
    let mut declarations = types();
    declarations[PACKED.index() as usize] =
        aggregate(7, 9, 1, vec![BYTE, WORD], vec![0, 1], vec![]);
    assert!(matches!(
        request(declarations).admit_current_production(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

#[test]
fn original_layout_geometry_and_recursive_pointer_closure_are_preserved() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let floor = budget.storage();
    let layouts = SourceStorageLayoutsV29::new(
        &owner,
        &[PACKED, REORDERED, UNION, ARRAY, RECURSIVE, DESCRIPTOR, HUGE],
        &mut budget,
    )
    .unwrap();
    let rows = layouts.rows(&owner, &mut budget).unwrap();
    for (ty, size, alignment, offsets) in [
        (PACKED, 9, 1, vec![0, 1]),
        (REORDERED, 16, 8, vec![8, 0]),
        (UNION, 8, 8, vec![0, 0]),
    ] {
        let id = layouts.row_for(&owner, ty, &mut budget).unwrap();
        let row = &rows[id.0 as usize];
        assert_eq!((row.size, row.alignment), (size, alignment));
        let fields = match &row.kind {
            StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => fields,
            _ => panic!("wrong aggregate row"),
        };
        assert_eq!(fields.iter().map(|f| f.offset).collect::<Vec<_>>(), offsets);
    }
    let link = layouts.row_for(&owner, LINK, &mut budget).unwrap();
    let StorageLayoutKindV1::Pointer(pointer) = &rows[link.0 as usize].kind else {
        panic!("wrong pointer row")
    };
    assert_eq!(pointer.value_space, AddressSpace::Generic);
    assert_eq!(pointer.encoded_space, AddressSpace::Generic);
    assert_eq!(
        pointer.pointee,
        layouts.row_for(&owner, RECURSIVE, &mut budget).unwrap()
    );
    let descriptor = layouts.row_for(&owner, DESCRIPTOR, &mut budget).unwrap();
    let StorageLayoutKindV1::Slice { data, length, .. } = &rows[descriptor.0 as usize].kind else {
        panic!("wrong descriptor row")
    };
    assert_eq!((data.offset, length.offset), (0, 8));
    assert!(matches!(
        rows[length.layout.0 as usize].kind,
        StorageLayoutKindV1::Scalar(ScalarType::Index)
    ));
    assert!(!layouts.keys.contains(&RowKey::ty(SLICE)));
    let huge = layouts.row_for(&owner, HUGE, &mut budget).unwrap();
    assert!(
        matches!(rows[huge.0 as usize].kind, StorageLayoutKindV1::Array { length, stride: 8, .. } if length == u64::from(u32::MAX))
    );
    drop(rows);
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn demand_order_is_canonical_and_foreign_equal_source_owners_are_rejected() {
    let owner = owner();
    let foreign = owner_with(types());
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let first =
        SourceStorageLayoutsV29::new(&owner, &[DESCRIPTOR, PACKED, ARRAY], &mut budget).unwrap();
    let second =
        SourceStorageLayoutsV29::new(&owner, &[ARRAY, PACKED, DESCRIPTOR, PACKED], &mut budget)
            .unwrap();
    assert_eq!(first.keys, second.keys);
    assert_eq!(
        *first.rows(&owner, &mut budget).unwrap(),
        *second.rows(&owner, &mut budget).unwrap()
    );
    assert!(first.row_for(&foreign, WORD, &mut budget).is_err());
    second.release(&mut budget).unwrap();
    first.release(&mut budget).unwrap();
}

#[test]
fn unsized_storage_and_out_of_roster_types_are_not_fabricated() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let floor = budget.storage();
    assert!(SourceStorageLayoutsV29::new(&owner, &[SLICE], &mut budget).is_err());
    assert_eq!(budget.storage(), floor);
    assert!(
        SourceStorageLayoutsV29::new(
            &owner,
            &[SemanticTypeIdV1::from_index(u32::MAX)],
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn direct_signed_tag_bits_and_pointer_niche_source_layouts_are_retained() {
    let owner = owner();
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts = SourceStorageLayoutsV29::new(&owner, &[DIRECT, NICHE], &mut budget).unwrap();
    let rows = layouts.rows(&owner, &mut budget).unwrap();
    let direct = layouts.row_for(&owner, DIRECT, &mut budget).unwrap();
    let StorageLayoutKindV1::Variants {
        encoding: StorageVariantEncodingV1::Direct { tag },
        variants,
    } = &rows[direct.0 as usize].kind
    else {
        panic!("direct source enum lost its tag")
    };
    assert_eq!(tag.offset, 0);
    assert_eq!(
        (variants[0].discriminant, variants[0].direct_tag_bits),
        (255, Some(255))
    );
    assert_eq!(
        (variants[1].discriminant, variants[1].direct_tag_bits),
        (1, Some(1))
    );
    let StorageLayoutKindV1::Record(payload) = &rows[variants[0].layout.0 as usize].kind else {
        panic!("missing payload")
    };
    assert_eq!(payload[0].offset, 8);
    let niche = layouts.row_for(&owner, NICHE, &mut budget).unwrap();
    let StorageLayoutKindV1::Variants {
        encoding:
            StorageVariantEncodingV1::Niche {
                tag,
                untagged_variant,
                first_niche_variant,
                last_niche_variant,
                niche_start,
            },
        variants,
    } = &rows[niche.0 as usize].kind
    else {
        panic!("niche source enum lost its tag")
    };
    assert_eq!(
        (
            *untagged_variant,
            *first_niche_variant,
            *last_niche_variant,
            *niche_start
        ),
        (1, 0, 0, 0)
    );
    assert_eq!(tag.offset, 0);
    assert!(matches!(
        rows[tag.layout.0 as usize].kind,
        StorageLayoutKindV1::Pointer(_)
    ));
    assert!(
        variants
            .iter()
            .all(|variant| variant.direct_tag_bits.is_none())
    );
    drop(rows);
    layouts.release(&mut budget).unwrap();
}

#[test]
fn changed_physical_layout_facts_fail_semantic_admission_before_row_construction() {
    let mut wrong = types();
    wrong[PAIR.index() as usize] = aggregate(6, 16, 8, vec![WORD, BYTE], vec![0, 7], vec![]);
    assert!(
        request(wrong)
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
    let mut wrong = types();
    wrong[ARRAY.index() as usize] = array(10, PAIR, 4, 8, 8, true);
    assert!(
        request(wrong)
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
}

#[test]
fn original_by_value_cycles_fail_both_storage_admission_paths_before_publishing_rows() {
    let mut cycle = types();
    cycle[RECURSIVE.index() as usize] = aggregate(13, 8, 8, vec![RECURSIVE], vec![0], vec![]);
    // V1 semantic admission checks local geometry; storage admission checks
    // the complete by-value closure before publishing physical rows.
    let owner = owner_with(cycle);
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(263).unwrap();
    assert!(matches!(
        SourceStorageLayoutsV29::new(&owner, &[RECURSIVE], &mut budget),
        Err(Error::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "source storage original containment is cyclic or missing",
        })
    ));
    assert_eq!(budget.storage(), 263);

    let layouts = SourceStorageLayoutsV29::new(&owner, &[], &mut budget).unwrap();
    let floor = budget.storage();
    let credit = layouts.lease.persistent.get();
    assert!(matches!(
        layouts.select_original_closure(&owner, RECURSIVE, &mut budget),
        Err(Error::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "original closure contains a by-value cycle",
        })
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(layouts.lease.persistent.get(), credit);
    assert!(layouts.keys.is_empty());
    assert!(layouts.physical.borrow().rows.is_empty());
    layouts.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 263);
}

#[test]
fn pointer_width_and_address_space_come_from_the_exact_source_target_contract() {
    fn local_pointer(space: u32, bits: u16) -> SemanticTypeDeclV1 {
        let bytes = u64::from(bits / 8);
        declaration(
            5,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(space, bytes, bytes),
                    SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    WORD,
                    SemanticMutabilityV1::Immutable,
                    space,
                    bits,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
    }
    for (space, bits, expected) in [
        (0, 64, AddressSpace::Generic),
        (1, 64, AddressSpace::Global),
        (3, 32, AddressSpace::Workgroup),
        (4, 64, AddressSpace::Constant),
        (5, 32, AddressSpace::Private),
    ] {
        let mut source = types();
        source[RAW.index() as usize] = local_pointer(space, bits);
        let owner = owner_with(source);
        let mut work = work();
        let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
        budget.reserve_storage(269).unwrap();
        let layouts = SourceStorageLayoutsV29::new(&owner, &[RAW], &mut budget).unwrap();
        let id = layouts.row_for(&owner, RAW, &mut budget).unwrap();
        let rows = layouts.rows(&owner, &mut budget).unwrap();
        let StorageLayoutKindV1::Pointer(pointer) = rows[id.0 as usize].kind else {
            panic!("missing pointer")
        };
        assert_eq!(
            (pointer.value_space, pointer.encoded_space, pointer.stored_bits),
            (expected, expected, bits)
        );
        for unsupported in [2, 6, u32::MAX] {
            assert!(matches!(
                layouts.pointer_space(unsupported),
                Err(Error::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "source storage pointer requires an additional admitted target address-space contract",
                })
            ));
        }
        drop(rows);
        layouts.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 269);
    }
    let mut wrong_width = types();
    wrong_width[RAW.index() as usize] = local_pointer(0, 32);
    assert!(
        request(wrong_width)
            .admit_current_production(SemanticMirLimitsV1::default())
            .is_err()
    );
}

#[test]
fn a_multiple_root_source_module_uses_one_demand_closure_and_shared_row_ids() {
    let admitted = request_with_roots(types(), true)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let owner =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    assert_eq!(owner.source_semantic().roots().len(), 2);
    let mut work = work();
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let layouts =
        SourceStorageLayoutsV29::new(&owner, &[PAIR, DESCRIPTOR, PAIR, DESCRIPTOR], &mut budget)
            .unwrap();
    let word = layouts.row_for(&owner, WORD, &mut budget).unwrap();
    assert_eq!(
        layouts
            .keys
            .iter()
            .filter(|key| **key == RowKey::ty(WORD))
            .count(),
        1
    );
    let pair = layouts.row_for(&owner, PAIR, &mut budget).unwrap();
    let rows = layouts.rows(&owner, &mut budget).unwrap();
    let StorageLayoutKindV1::Record(fields) = &rows[pair.0 as usize].kind else {
        panic!("missing pair")
    };
    assert_eq!(fields[0].layout, word);
    let descriptor = layouts.row_for(&owner, DESCRIPTOR, &mut budget).unwrap();
    let StorageLayoutKindV1::Slice { element, .. } = rows[descriptor.0 as usize].kind else {
        panic!("missing descriptor")
    };
    assert_eq!(element, word);
    drop(rows);
    layouts.release(&mut budget).unwrap();
}
