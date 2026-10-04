use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_mir_model::semantic_mir_v1::*;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn type_identity(tag: u8) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1::from_sha256(bytes(tag))
}

fn layout_identity(tag: u8) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(tag))
}

fn scalar_type(
    identity: u8,
    layout: SemanticLayoutIdentityV1,
    bits: u16,
    size_bytes: u64,
    alignment_bytes: u64,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size_bytes),
            alignment_bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, bits, alignment_bytes),
                SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }),
    )
}

fn u32_type(identity: u8) -> SemanticTypeDeclV1 {
    scalar_type(identity, layout_identity(identity), 32, 4, 4)
}

fn unit_type(identity: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
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

fn raw_pointer_type(
    identity: u8,
    pointee: SemanticTypeIdV1,
    address_space: u32,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(address_space, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                pointee,
                SemanticMutabilityV1::Immutable,
                address_space,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    )
}

fn dyn_type(identity: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        SemanticTypeLayoutV1::new(None, 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}

fn vtable_pointer_type(identity: u8, pointee: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    vtable_pointer_type_with_metadata(
        identity,
        pointee,
        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
    )
}

fn vtable_pointer_type_with_metadata(
    identity: u8,
    pointee: SemanticTypeIdV1,
    metadata_pointee: SemanticAbiPointeeInfoV1,
) -> SemanticTypeDeclV1 {
    let nonnull_pointer = || {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )
    };
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(nonnull_pointer(), nonnull_pointer()),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::VTable,
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
            Some(metadata_pointee),
        ),
    )
}

fn local(identity: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(identity)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn kernel_function(
    unit: SemanticTypeIdV1,
    temporary: SemanticTypeIdV1,
    statements: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(200),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(1)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(1)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(1)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            local(1, unit, SemanticLocalRoleV1::Return),
            local(2, temporary, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
}

fn request(
    types: Vec<SemanticTypeDeclV1>,
    allocations: Vec<SemanticAllocationDeclV1>,
    vtables: Vec<SemanticVTableDeclV1>,
    function: SemanticFunctionDeclV1,
) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        types,
        allocations,
        vec![],
        vtables,
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn constant_pointer_assignment(
    ty: SemanticTypeIdV1,
    pointer: SemanticPointerValueV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ty).unwrap(),
            SemanticRvalueV1::new(
                ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty,
                    SemanticConstantValueV1::Pointer(pointer),
                ))),
            ),
        )),
    )
}

#[test]
fn out_of_range_pointer_pointee_returns_error_without_panicking() {
    let unit = SemanticTypeIdV1::from_index(0);
    let pointer = SemanticTypeIdV1::from_index(1);
    let model = request(
        vec![
            unit_type(1),
            raw_pointer_type(2, SemanticTypeIdV1::from_index(99), 0),
        ],
        vec![],
        vec![],
        kernel_function(unit, pointer, vec![]),
    );

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        model.admit(SemanticMirLimitsV1::default())
    }));
    let admission = outcome.expect("hostile pointee IDs must not unwind admission");
    assert!(admission.is_err(), "out-of-range pointee was admitted");
}

#[test]
fn raw_pointer_abi_evidence_is_conservative() {
    SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1)
        .expect("the conservative raw-pointer fact must be representable");
    for (size, alignment) in [(1, 1), (0, 2), (8, 8)] {
        assert!(matches!(
            SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, size, alignment,),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

fn incompatible_layout_cast_request(
    projection_kind: SemanticProjectionKindV1,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let u64_id = SemanticTypeIdV1::from_index(1);
    let unit_id = SemanticTypeIdV1::from_index(2);
    let shared_layout_identity = layout_identity(77);
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(projection_kind, u64_id).unwrap()],
        u64_id,
    )
    .unwrap();
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Deinitialize(place),
    );
    request(
        vec![
            scalar_type(1, shared_layout_identity, 32, 4, 4),
            scalar_type(2, shared_layout_identity, 64, 8, 8),
            unit_type(3),
        ],
        vec![],
        vec![],
        kernel_function(unit_id, u32_id, vec![statement]),
    )
}

#[test]
fn incompatible_layouts_cannot_authorize_opaque_cast_or_subtype() {
    for projection in [
        SemanticProjectionKindV1::OpaqueCast,
        SemanticProjectionKindV1::Subtype,
    ] {
        let result =
            incompatible_layout_cast_request(projection).admit(SemanticMirLimitsV1::default());
        assert!(
            result.is_err(),
            "{projection:?} trusted one layout identity for incompatible layouts"
        );
    }
}

fn vtable_request(
    dyn_decl: SemanticTypeDeclV1,
    predicates: Vec<SemanticDynPredicateIdentityV1>,
) -> Result<InertSemanticMirRequestV1, SemanticMirErrorV1> {
    let dyn_id = SemanticTypeIdV1::from_index(1);
    let pointer_id = SemanticTypeIdV1::from_index(2);
    let unit_id = SemanticTypeIdV1::from_index(3);
    let mut header = vec![0; 24];
    header[8..16].copy_from_slice(&4_u64.to_le_bytes());
    header[16..24].copy_from_slice(&4_u64.to_le_bytes());
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        header,
        vec![u8::MAX; 3],
        8,
        false,
        vec![],
    )
    .unwrap();
    let vtable = SemanticVTableDeclV1::new(
        SemanticVTableIdentityV1::from_sha256(bytes(1)),
        SemanticTypeIdV1::from_index(0),
        dyn_id,
        predicates,
        SemanticVTableHeaderV1::new(None, 4, 4).unwrap(),
        vec![],
        SemanticAllocationIdV1::from_index(0),
    )?;
    let statement = constant_pointer_assignment(
        pointer_id,
        SemanticPointerValueV1::new_with_metadata(
            0,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
            SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(0)),
        ),
    );
    Ok(request(
        vec![
            u32_type(1),
            dyn_decl,
            vtable_pointer_type(3, dyn_id),
            unit_type(4),
        ],
        vec![allocation],
        vec![vtable],
        kernel_function(unit_id, pointer_id, vec![statement]),
    ))
}

fn admit_vtable(
    dyn_decl: SemanticTypeDeclV1,
    predicates: Vec<SemanticDynPredicateIdentityV1>,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    vtable_request(dyn_decl, predicates)?.admit(SemanticMirLimitsV1::default())
}

fn predicate(tag: u8) -> SemanticDynPredicateIdentityV1 {
    SemanticDynPredicateIdentityV1::from_sha256(bytes(tag))
}

#[test]
fn vtable_dyn_type_must_be_an_unsized_dyn_shape() {
    admit_vtable(dyn_type(2), vec![predicate(1)]).expect("control vtable must be admissible");

    let scalar_dyn = scalar_type(2, layout_identity(2), 32, 4, 4);
    assert!(
        admit_vtable(scalar_dyn, vec![predicate(1)]).is_err(),
        "a sized scalar was accepted as a vtable dyn type"
    );
}

#[test]
fn vtable_predicates_must_be_nonempty_strictly_ordered_and_unique() {
    admit_vtable(dyn_type(2), vec![predicate(1), predicate(2)])
        .expect("control vtable must be admissible");

    for (case, predicates) in [
        ("empty", vec![]),
        ("out of order", vec![predicate(2), predicate(1)]),
        ("duplicate", vec![predicate(1), predicate(1)]),
    ] {
        assert!(
            admit_vtable(dyn_type(2), predicates).is_err(),
            "{case} vtable predicates were admitted"
        );
    }
}

#[test]
fn vtable_metadata_cannot_claim_safe_pointee_evidence() {
    let dyn_id = SemanticTypeIdV1::from_index(1);
    let pointer_id = SemanticTypeIdV1::from_index(2);
    let unit_id = SemanticTypeIdV1::from_index(3);
    let mut header = vec![0; 24];
    header[8..16].copy_from_slice(&4_u64.to_le_bytes());
    header[16..24].copy_from_slice(&4_u64.to_le_bytes());
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        header,
        vec![u8::MAX; 3],
        8,
        false,
        vec![],
    )
    .unwrap();
    let vtable = SemanticVTableDeclV1::new(
        SemanticVTableIdentityV1::from_sha256(bytes(1)),
        SemanticTypeIdV1::from_index(0),
        dyn_id,
        vec![predicate(1)],
        SemanticVTableHeaderV1::new(None, 4, 4).unwrap(),
        vec![],
        SemanticAllocationIdV1::from_index(0),
    )
    .unwrap();
    let statement = constant_pointer_assignment(
        pointer_id,
        SemanticPointerValueV1::new_with_metadata(
            0,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
            SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(0)),
        ),
    );
    let forged = request(
        vec![
            u32_type(1),
            dyn_type(2),
            vtable_pointer_type_with_metadata(
                3,
                dyn_id,
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    1,
                )
                .unwrap(),
            ),
            unit_type(4),
        ],
        vec![allocation],
        vec![vtable],
        kernel_function(unit_id, pointer_id, vec![statement]),
    );
    assert!(matches!(
        forged.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

fn relocation_request(
    width_bytes: u8,
    storage: Vec<u8>,
    initialized_mask: Vec<u8>,
) -> InertSemanticMirRequestV1 {
    relocation_request_with_addend(width_bytes, storage, initialized_mask, 0)
}

fn relocation_request_with_addend(
    width_bytes: u8,
    storage: Vec<u8>,
    initialized_mask: Vec<u8>,
    addend: i64,
) -> InertSemanticMirRequestV1 {
    let pointer_id = SemanticTypeIdV1::from_index(1);
    let unit_id = SemanticTypeIdV1::from_index(2);
    let relocation = SemanticRelocationV1::new(
        0,
        width_bytes,
        addend,
        SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(0)),
    )
    .unwrap();
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        storage,
        initialized_mask,
        8,
        false,
        vec![relocation],
    )
    .unwrap();
    let statement = constant_pointer_assignment(
        pointer_id,
        SemanticPointerValueV1::new(
            0,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
        ),
    );
    request(
        vec![
            u32_type(1),
            raw_pointer_type(2, SemanticTypeIdV1::from_index(0), 0),
            unit_type(3),
        ],
        vec![allocation],
        vec![],
        kernel_function(unit_id, pointer_id, vec![statement]),
    )
}

fn valid_relocation_request() -> InertSemanticMirRequestV1 {
    relocation_request(8, vec![0; 8], vec![u8::MAX])
}

fn fully_initialized_mask(byte_len: usize) -> Vec<u8> {
    let mut mask = vec![u8::MAX; byte_len.div_ceil(8)];
    if !byte_len.is_multiple_of(8) {
        *mask.last_mut().unwrap() = (1_u8 << (byte_len % 8)) - 1;
    }
    mask
}

#[test]
fn relocation_width_must_match_the_gfx942_pointer_width() {
    valid_relocation_request()
        .admit(SemanticMirLimitsV1::default())
        .expect("control relocation must be admissible");

    for width in [1, 4, 16] {
        let length = usize::from(width);
        let initialized_mask = fully_initialized_mask(length);
        assert!(
            matches!(
                relocation_request(width, vec![0; length], initialized_mask)
                    .admit(SemanticMirLimitsV1::default()),
                Err(SemanticMirErrorV1::InvalidRelocation)
            ),
            "{width}-byte function relocation was admitted"
        );
    }
}

#[test]
fn relocation_storage_must_be_fully_initialized() {
    valid_relocation_request()
        .admit(SemanticMirLimitsV1::default())
        .expect("control relocation must be admissible");
    assert!(
        matches!(
            relocation_request(8, vec![0; 8], vec![0]).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidRelocation)
        ),
        "relocation over uninitialized storage was admitted"
    );
}

#[test]
fn relocation_storage_must_be_canonical_zero() {
    valid_relocation_request()
        .admit(SemanticMirLimitsV1::default())
        .expect("control relocation must be admissible");
    let mut nonzero = vec![0; 8];
    nonzero[3] = 1;
    assert!(
        matches!(
            relocation_request(8, nonzero, vec![u8::MAX]).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidRelocation)
        ),
        "relocation over nonzero storage was admitted"
    );
}

#[test]
fn callable_relocations_require_a_zero_unsigned_addend() {
    for addend in [-1, 1, i64::MAX] {
        assert!(matches!(
            relocation_request_with_addend(8, vec![0; 8], vec![u8::MAX], addend)
                .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidRelocation)
        ));
    }
}

fn function_pointer_type(
    backend: SemanticBackendReprV1,
    size_bytes: u64,
    alignment_bytes: u64,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(1),
        layout_identity(1),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size_bytes),
            alignment_bytes,
            backend,
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::FunctionPointer {
            safety: SemanticFunctionSafetyV1::Safe,
            extern_abi: SemanticExternAbiV1::Rust,
            c_variadic: false,
            arguments: SemanticAggregateTypeV1::new(vec![]).unwrap(),
            return_type: SemanticTypeIdV1::from_index(1),
        },
    )
}

fn admit_function_pointer(
    backend: SemanticBackendReprV1,
    size_bytes: u64,
    alignment_bytes: u64,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let function_pointer = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    request(
        vec![
            function_pointer_type(backend, size_bytes, alignment_bytes),
            unit_type(2),
        ],
        vec![],
        vec![],
        kernel_function(unit, function_pointer, vec![]),
    )
    .admit(SemanticMirLimitsV1::default())
}

#[test]
fn function_pointer_layout_is_exact_for_gfx942() {
    let as0_pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let exact = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        as0_pointer,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    ));
    admit_function_pointer(exact, 8, 8).expect("exact function pointer must be admissible");

    let hostile = [
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::union(as0_pointer)),
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            as0_pointer,
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        )),
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(1, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )),
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )),
    ];
    for backend in hostile {
        assert!(admit_function_pointer(backend, 8, 8).is_err());
    }
    let narrow = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 4, 4),
        SemanticScalarValidityRangeV1::new(1, u32::MAX.into()),
    ));
    assert!(admit_function_pointer(narrow, 4, 4).is_err());
}

fn static_relocation_request(width_bytes: u8, address_space: u32) -> InertSemanticMirRequestV1 {
    static_relocation_request_with_addend(width_bytes, address_space, 0)
}

fn static_relocation_request_with_addend(
    width_bytes: u8,
    address_space: u32,
    addend: i64,
) -> InertSemanticMirRequestV1 {
    let pointer = SemanticTypeIdV1::from_index(1);
    let unit = SemanticTypeIdV1::from_index(2);
    let relocation = SemanticRelocationV1::new_in_address_space(
        0,
        width_bytes,
        address_space,
        addend,
        SemanticRelocationTargetV1::Static(SemanticStaticIdV1::from_index(0)),
    )
    .unwrap();
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        vec![0; usize::from(width_bytes)],
        fully_initialized_mask(usize::from(width_bytes)),
        4,
        false,
        vec![relocation],
    )
    .unwrap();
    let static_decl = SemanticStaticDeclV1::new(
        SemanticStaticIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticTypeIdV1::from_index(0),
        false,
        3,
        SemanticStaticDefinitionV1::ExternalRequired {
            symbol: SemanticLinkSymbolV1::new(b"lds_static".to_vec()).unwrap(),
        },
    );
    let statement = constant_pointer_assignment(
        pointer,
        SemanticPointerValueV1::new(
            0,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
        ),
    );
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        vec![
            u32_type(1),
            raw_pointer_type(2, SemanticTypeIdV1::from_index(0), 0),
            unit_type(3),
        ],
        vec![allocation],
        vec![static_decl],
        vec![],
        vec![kernel_function(unit, pointer, vec![statement])],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn relocation_address_space_selects_the_exact_pointer_profile() {
    static_relocation_request(4, 3)
        .admit(SemanticMirLimitsV1::default())
        .expect("gfx942 LDS relocation must use its 32-bit pointer profile");
    for (width, address_space) in [(8, 3), (4, 0)] {
        assert!(matches!(
            static_relocation_request(width, address_space).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidRelocation)
        ));
    }
    static_relocation_request_with_addend(4, 3, 4)
        .admit(SemanticMirLimitsV1::default())
        .expect("one-past static relocation addends remain representable, not dereferenceable");
    for addend in [-1, 5] {
        assert!(matches!(
            static_relocation_request_with_addend(4, 3, addend)
                .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidRelocation)
        ));
    }
}

fn vtable_bytes(slot_count: usize) -> Vec<u8> {
    let mut bytes = vec![0; 24 + slot_count * 8];
    bytes[8..16].copy_from_slice(&4_u64.to_le_bytes());
    bytes[16..24].copy_from_slice(&4_u64.to_le_bytes());
    bytes
}

fn vacant_vtable_request(slot_initialized: bool) -> InertSemanticMirRequestV1 {
    let dyn_id = SemanticTypeIdV1::from_index(1);
    let pointer_id = SemanticTypeIdV1::from_index(2);
    let unit_id = SemanticTypeIdV1::from_index(3);
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        vtable_bytes(1),
        vec![
            u8::MAX,
            u8::MAX,
            u8::MAX,
            u8::from(slot_initialized) * u8::MAX,
        ],
        8,
        false,
        vec![],
    )
    .unwrap();
    let vtable = SemanticVTableDeclV1::new_with_slots(
        SemanticVTableIdentityV1::from_sha256(bytes(1)),
        SemanticTypeIdV1::from_index(0),
        dyn_id,
        vec![predicate(1)],
        SemanticVTableHeaderV1::new(None, 4, 4).unwrap(),
        vec![SemanticVTableSlotV1::Vacant],
        SemanticAllocationIdV1::from_index(0),
    )
    .unwrap();
    let statement = constant_pointer_assignment(
        pointer_id,
        SemanticPointerValueV1::new_with_metadata(
            0,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
            SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(0)),
        ),
    );
    request(
        vec![
            u32_type(1),
            dyn_type(2),
            vtable_pointer_type(3, dyn_id),
            unit_type(4),
        ],
        vec![allocation],
        vec![vtable],
        kernel_function(unit_id, pointer_id, vec![statement]),
    )
}

fn trait_vptr_request(wrong_trait_ref: bool, cycle: bool) -> InertSemanticMirRequestV1 {
    let dyn_source = SemanticTypeIdV1::from_index(1);
    let dyn_target = SemanticTypeIdV1::from_index(2);
    let pointer_id = SemanticTypeIdV1::from_index(3);
    let unit_id = SemanticTypeIdV1::from_index(4);
    let target_trait_ref =
        SemanticTraitRefIdentityV1::from_sha256(bytes(if wrong_trait_ref { 9 } else { 2 }));
    let source_allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        vtable_bytes(1),
        vec![u8::MAX; 4],
        8,
        false,
        vec![
            SemanticRelocationV1::new(
                24,
                8,
                0,
                SemanticRelocationTargetV1::VTable(SemanticVTableIdV1::from_index(1)),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let target_allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(2)),
        vtable_bytes(usize::from(cycle)),
        vec![u8::MAX; 3 + usize::from(cycle)],
        8,
        false,
        if cycle {
            vec![
                SemanticRelocationV1::new(
                    24,
                    8,
                    0,
                    SemanticRelocationTargetV1::VTable(SemanticVTableIdV1::from_index(0)),
                )
                .unwrap(),
            ]
        } else {
            vec![]
        },
    )
    .unwrap();
    let source_vtable = SemanticVTableDeclV1::new_with_trait_identity_and_slots(
        SemanticVTableIdentityV1::from_sha256(bytes(1)),
        SemanticTypeIdV1::from_index(0),
        dyn_source,
        SemanticVTableTraitIdentityV1::new(
            SemanticTraitRefIdentityV1::from_sha256(bytes(1)),
            vec![predicate(1)],
        )
        .unwrap(),
        SemanticVTableHeaderV1::new(None, 4, 4).unwrap(),
        vec![SemanticVTableSlotV1::TraitVPtr {
            trait_ref: target_trait_ref,
            target: SemanticVTableIdV1::from_index(1),
        }],
        SemanticAllocationIdV1::from_index(0),
    )
    .unwrap();
    let target_vtable = SemanticVTableDeclV1::new_with_trait_identity_and_slots(
        SemanticVTableIdentityV1::from_sha256(bytes(2)),
        SemanticTypeIdV1::from_index(0),
        dyn_target,
        SemanticVTableTraitIdentityV1::new(
            SemanticTraitRefIdentityV1::from_sha256(bytes(2)),
            vec![predicate(2)],
        )
        .unwrap(),
        SemanticVTableHeaderV1::new(None, 4, 4).unwrap(),
        if cycle {
            vec![SemanticVTableSlotV1::TraitVPtr {
                trait_ref: SemanticTraitRefIdentityV1::from_sha256(bytes(1)),
                target: SemanticVTableIdV1::from_index(0),
            }]
        } else {
            vec![]
        },
        SemanticAllocationIdV1::from_index(1),
    )
    .unwrap();
    let statement = constant_pointer_assignment(
        pointer_id,
        SemanticPointerValueV1::new_with_metadata(
            0,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
            SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(0)),
        ),
    );
    request(
        vec![
            u32_type(1),
            dyn_type(2),
            dyn_type(3),
            vtable_pointer_type(4, dyn_source),
            unit_type(5),
        ],
        vec![source_allocation, target_allocation],
        vec![source_vtable, target_vtable],
        kernel_function(unit_id, pointer_id, vec![statement]),
    )
}

#[test]
fn vacant_and_trait_vptr_slots_are_exact_and_acyclic() {
    vacant_vtable_request(false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    trait_vptr_request(false, false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    for hostile in [
        vacant_vtable_request(true),
        trait_vptr_request(true, false),
        trait_vptr_request(false, true),
    ] {
        assert!(matches!(
            hostile.admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidAllocation)
        ));
    }
}
