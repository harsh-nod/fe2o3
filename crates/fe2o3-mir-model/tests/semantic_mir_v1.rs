use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::semantic_option_producers_v1;
use sha2::{Digest, Sha256};

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn type_identity(tag: u8) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1::from_sha256(bytes(tag))
}

fn layout_identity(tag: u8) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(tag))
}

fn function_identity(tag: u8) -> SemanticFunctionIdentityV1 {
    SemanticFunctionIdentityV1::from_sha256(bytes(tag))
}

fn item_identity(tag: u8) -> SemanticItemDefinitionIdentityV1 {
    SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag))
}

fn monomorphization_identity(tag: u8) -> SemanticMonomorphizationIdentityV1 {
    SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag))
}

fn generic_types_identity(tag: u8) -> SemanticGenericTypeArgumentsIdentityV1 {
    SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag))
}

fn const_generics_identity(tag: u8) -> SemanticConstGenericArgumentsIdentityV1 {
    SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag))
}

fn local_identity(tag: u8) -> SemanticLocalIdentityV1 {
    SemanticLocalIdentityV1::from_sha256(bytes(tag))
}

fn block_identity(tag: u8) -> SemanticBlockIdentityV1 {
    SemanticBlockIdentityV1::from_sha256(bytes(tag))
}

fn scalar_layout(
    size_bytes: u64,
    alignment_bytes: u64,
    primitive: SemanticBackendPrimitiveV1,
    valid_range: SemanticScalarValidityRangeV1,
) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::new_with_backend_repr(
        Some(size_bytes),
        alignment_bytes,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(primitive, valid_range)),
        false,
    )
    .unwrap()
}

fn u32_type(identity: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            4,
            4,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    )
}

fn i32_type(identity: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            4,
            4,
            SemanticBackendPrimitiveV1::integer(true, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 32,
        }),
    )
}

fn bool_type(identity: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            1,
            1,
            SemanticBackendPrimitiveV1::integer(false, 8, 1),
            SemanticScalarValidityRangeV1::new(0, 1),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    )
}

fn pointer_type(identity: u8, pointee: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::pointer(1, 8, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                pointee,
                SemanticMutabilityV1::Mutable,
                1,
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

fn pointer_kind_type(
    identity: u8,
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    address_space: u32,
) -> SemanticTypeDeclV1 {
    let validity = match kind {
        SemanticPointerKindV1::Raw => SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        SemanticPointerKindV1::Reference => SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    };
    let pointee_info = match (kind, mutability) {
        (SemanticPointerKindV1::Raw, _) => {
            SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()
        }
        (SemanticPointerKindV1::Reference, SemanticMutabilityV1::Immutable) => {
            SemanticAbiPointeeInfoV1::new(
                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                4,
                4,
            )
            .unwrap()
        }
        (SemanticPointerKindV1::Reference, SemanticMutabilityV1::Mutable) => {
            SemanticAbiPointeeInfoV1::new(
                SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                4,
                4,
            )
            .unwrap()
        }
    };
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::pointer(address_space, 8, 8),
            validity,
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                mutability,
                address_space,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_scalar_pointee_info(Some(pointee_info), None),
    )
}

fn abi(
    identity: u8,
    arguments: Vec<SemanticTypeIdV1>,
    return_type: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let direct = SemanticAbiPassModeV1::Direct(noundef_attributes(SemanticAbiExtensionV1::None));
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(identity)),
        layout_identity(identity),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments
            .into_iter()
            .map(|ty| SemanticAbiValueV1::new(ty, direct.clone()))
            .collect(),
        SemanticAbiValueV1::new(return_type, direct),
    )
    .unwrap()
}

fn local(identity: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        local_identity(identity),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn block(
    identity: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        block_identity(identity),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn function(
    identity: u8,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    function_with_identity_axes(
        identity,
        identity,
        identity,
        identity,
        identity,
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        blocks,
    )
}

#[allow(clippy::too_many_arguments)]
fn function_with_identity_axes(
    identity: u8,
    item: u8,
    monomorphization: u8,
    generic_types: u8,
    const_generics: u8,
    source: SemanticSourceProvenanceV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function_identity(identity),
        SemanticFunctionRoleV1::InternalHelper,
        item_identity(item),
        monomorphization_identity(monomorphization),
        generic_types_identity(generic_types),
        const_generics_identity(const_generics),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn request(
    types: Vec<SemanticTypeDeclV1>,
    allocations: Vec<SemanticAllocationDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> InertSemanticMirRequestV1 {
    let functions = functions
        .into_iter()
        .enumerate()
        .map(|(index, function)| {
            let role = if index == 0 {
                SemanticFunctionRoleV1::KernelRoot
            } else if matches!(function.role(), SemanticFunctionRoleV1::DropGlue(_)) {
                function.role()
            } else {
                SemanticFunctionRoleV1::InternalHelper
            };
            function.with_role(role)
        })
        .collect();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        types,
        allocations,
        vec![],
        vec![],
        functions,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn simple_request() -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![u32_id], u32_id),
            vec![
                local(1, u32_id, SemanticLocalRoleV1::Return),
                local(2, u32_id, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        )],
    )
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

fn validity_u8_type(identity: u8) -> SemanticTypeDeclV1 {
    validity_u8_type_with_ranges(
        identity,
        vec![SemanticScalarValidityRangeV1::new(1, u8::MAX.into())],
    )
}

fn validity_u8_type_with_ranges(
    identity: u8,
    valid_ranges: Vec<SemanticScalarValidityRangeV1>,
) -> SemanticTypeDeclV1 {
    let backend_range = valid_ranges[0];
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            1,
            1,
            SemanticBackendPrimitiveV1::integer(false, 8, 1),
            backend_range,
        ),
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 8,
                },
                valid_ranges,
            )
            .unwrap(),
        ),
    )
}

fn request_with_structured_abi(
    types: Vec<SemanticTypeDeclV1>,
    arguments: Vec<SemanticAbiValueV1>,
    return_value: SemanticAbiValueV1,
) -> InertSemanticMirRequestV1 {
    let mut locals = vec![local(1, return_value.ty(), SemanticLocalRoleV1::Return)];
    locals.extend(arguments.iter().enumerate().map(|(index, argument)| {
        local(
            u8::try_from(index + 2).unwrap(),
            argument.ty(),
            SemanticLocalRoleV1::Argument(u32::try_from(index).unwrap()),
        )
    }));
    locals.extend(types.iter().enumerate().map(|(index, _)| {
        local(
            u8::try_from(index + 128).unwrap(),
            SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()),
            SemanticLocalRoleV1::Temporary,
        )
    }));
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments,
        return_value,
    )
    .unwrap();
    request(
        types,
        vec![],
        vec![function(
            1,
            abi,
            locals,
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        )],
    )
}

fn direct_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(noundef_attributes(SemanticAbiExtensionV1::None)),
    )
}

fn noundef_attributes(extension: SemanticAbiExtensionV1) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        extension,
        0,
        None,
    )
    .unwrap()
}

fn extended_direct_value(
    ty: SemanticTypeIdV1,
    extension: SemanticAbiExtensionV1,
) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(noundef_attributes(extension)),
    )
}

#[test]
fn deterministic_encoding_and_sha256_cover_the_admitted_bytes() {
    let left = simple_request()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let right = simple_request()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(left.canonical_encoding(), right.canonical_encoding());
    assert_eq!(left.semantic_sha256(), right.semantic_sha256());
    assert_eq!(
        left.semantic_sha256().as_bytes(),
        &<[u8; 32]>::from(Sha256::digest(left.canonical_encoding()))
    );
    assert!(
        left.canonical_encoding()
            .starts_with(b"fe2o3.inert-semantic-mir")
    );
}

fn identity_axis_request(
    item: u8,
    monomorphization: u8,
    generic_types: u8,
    const_generics: u8,
    source: SemanticSourceProvenanceV1,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    request(
        vec![u32_type(1)],
        vec![],
        vec![function_with_identity_axes(
            1,
            item,
            monomorphization,
            generic_types,
            const_generics,
            source,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        )],
    )
}

#[test]
fn each_function_identity_axis_independently_changes_semantic_identity() {
    let unavailable = SemanticSourceProvenanceV1::unavailable();
    let inputs = [
        (1, 1, 1, 1),
        (2, 1, 1, 1),
        (1, 2, 1, 1),
        (1, 1, 2, 1),
        (1, 1, 1, 2),
    ];
    let identities = inputs.map(|(item, mono, types, consts)| {
        identity_axis_request(item, mono, types, consts, unavailable)
            .admit(SemanticMirLimitsV1::default())
            .unwrap()
            .semantic_sha256()
    });
    assert_eq!(
        identities
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5
    );

    let admitted = identity_axis_request(11, 12, 13, 14, unavailable)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let function = &admitted.functions()[0];
    assert_eq!(function.item_definition_identity(), item_identity(11));
    assert_eq!(
        function.monomorphization_identity(),
        monomorphization_identity(12)
    );
    assert_eq!(
        function.generic_type_arguments_identity(),
        generic_types_identity(13)
    );
    assert_eq!(
        function.const_generic_arguments_identity(),
        const_generics_identity(14)
    );
}

fn origin(file: u8, line: u32) -> SemanticSourceOriginV1 {
    SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256(bytes(file)),
        u64::from(line) * 10,
        u64::from(line) * 10 + 4,
        line,
        1,
        line,
        5,
    )
    .unwrap()
}

#[test]
fn expansion_and_callsite_origins_are_independent_without_source_text() {
    let expansion = origin(1, 10);
    let call_site = origin(2, 20);
    let expansion_changed = identity_axis_request(
        1,
        1,
        1,
        1,
        SemanticSourceProvenanceV1::new(Some(origin(3, 30)), Some(call_site)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let callsite_changed = identity_axis_request(
        1,
        1,
        1,
        1,
        SemanticSourceProvenanceV1::new(Some(expansion), Some(origin(4, 40))),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let baseline = identity_axis_request(
        1,
        1,
        1,
        1,
        SemanticSourceProvenanceV1::new(Some(expansion), Some(call_site)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(
        baseline.semantic_sha256(),
        expansion_changed.semantic_sha256()
    );
    assert_ne!(
        baseline.semantic_sha256(),
        callsite_changed.semantic_sha256()
    );
    assert_eq!(
        baseline.functions()[0].source().expansion(),
        Some(expansion)
    );
    assert_eq!(
        baseline.functions()[0].source().call_site(),
        Some(call_site)
    );
}

#[test]
fn provenance_is_retained_on_locals_statements_and_terminators() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let provenance = SemanticSourceProvenanceV1::new(Some(origin(1, 10)), Some(origin(2, 20)));
    let local = SemanticLocalDeclV1::new(
        local_identity(1),
        u32_id,
        SemanticLocalRoleV1::Return,
        provenance,
    );
    let statement = SemanticStatementV1::new(provenance, SemanticStatementKindV1::Nop);
    let terminator = SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return);
    let block =
        SemanticBasicBlockV1::new(block_identity(1), provenance, vec![statement], terminator)
            .unwrap();
    let model = request(
        vec![u32_type(1)],
        vec![],
        vec![function_with_identity_axes(
            1,
            1,
            1,
            1,
            1,
            provenance,
            abi(1, vec![], u32_id),
            vec![local],
            vec![block],
        )],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let function = &model.functions()[0];
    assert_eq!(function.locals()[0].source(), provenance);
    assert_eq!(function.blocks()[0].source(), provenance);
    assert_eq!(function.blocks()[0].statements()[0].source(), provenance);
    assert_eq!(function.blocks()[0].terminator().source(), provenance);
}

#[test]
fn invalid_type_local_and_block_references_fail_closed() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let bad_type = request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(
                1,
                SemanticTypeIdV1::from_index(99),
                SemanticLocalRoleV1::Return,
            )],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        )],
    );
    assert!(matches!(
        bad_type.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Type,
            index: 99,
            ..
        })
    ));

    let bad_local_place =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(99), vec![], u32_id).unwrap();
    let bad_local = request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(
                1,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Deinitialize(bad_local_place),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    );
    assert!(matches!(
        bad_local.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Local,
            index: 99,
            ..
        })
    ));

    let bad_block = request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(
                1,
                vec![],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(99),
                )),
            )],
        )],
    );
    assert!(matches!(
        bad_block.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Block,
            index: 99,
            ..
        })
    ));
}

fn projection_request(reverse: bool) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let pointer_id = SemanticTypeIdV1::from_index(1);
    let pointer_pointer_id = SemanticTypeIdV1::from_index(2);
    let dereference_outer =
        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, pointer_id).unwrap();
    let dereference_inner =
        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, u32_id).unwrap();
    let projections = if reverse {
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, pointer_pointer_id)
                .unwrap(),
            dereference_outer,
            dereference_inner,
        ]
    } else {
        vec![
            dereference_outer,
            SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, pointer_id).unwrap(),
            dereference_inner,
        ]
    };
    let place =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, u32_id).unwrap();
    request(
        vec![
            u32_type(1),
            pointer_type(2, u32_id),
            pointer_type(3, pointer_id),
        ],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![
                local(1, u32_id, SemanticLocalRoleV1::Return),
                local(2, pointer_pointer_id, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(
                1,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Deinitialize(place),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    )
}

#[test]
fn projection_order_is_semantic_and_is_never_sorted() {
    let forward = projection_request(false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let reverse = projection_request(true)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_ne!(forward.canonical_encoding(), reverse.canonical_encoding());
    assert_ne!(forward.semantic_sha256(), reverse.semantic_sha256());
}

#[test]
fn edge_roles_cannot_be_substituted() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let model = request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(
                1,
                vec![],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(0),
                )),
            )],
        )],
    );
    assert!(matches!(
        model.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidEdgeRole {
            expected: SemanticEdgeRoleV1::Goto,
            actual: SemanticEdgeRoleV1::CallReturn,
            ..
        })
    ));
}

fn direct_call_request(tail: bool) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let arguments = vec![SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], u32_id).unwrap(),
    )];
    let first_terminator = if tail {
        SemanticTerminatorKindV1::TailCall(
            SemanticDirectTailCallV1::new(
                SemanticFunctionIdV1::from_index(1),
                arguments,
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    } else {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(1),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], u32_id).unwrap(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let first_blocks = if tail {
        vec![block(1, vec![], first_terminator)]
    } else {
        vec![
            block(1, vec![], first_terminator),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    request(
        vec![u32_type(1)],
        vec![],
        vec![
            function(
                1,
                abi(1, vec![u32_id], u32_id),
                vec![
                    local(1, u32_id, SemanticLocalRoleV1::Return),
                    local(2, u32_id, SemanticLocalRoleV1::Argument(0)),
                ],
                first_blocks,
            ),
            function(
                2,
                abi(2, vec![u32_id], u32_id),
                vec![
                    local(1, u32_id, SemanticLocalRoleV1::Return),
                    local(2, u32_id, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
            ),
        ],
    )
}

fn variadic_call_request(
    second_extra_arguments: usize,
    second_variadic_abis: usize,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let operand = || {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], u32_id).unwrap(),
        )
    };
    let call = |target, extra_arguments: usize, variadic_abis: usize| {
        let mut arguments = vec![operand()];
        arguments.extend((0..extra_arguments).map(|_| operand()));
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_with_variadic_argument_abis(
                SemanticFunctionIdV1::from_index(1),
                arguments,
                (0..variadic_abis).map(|_| direct_value(u32_id)).collect(),
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], u32_id).unwrap(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let variadic_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticCanonAbiV1::C,
        SemanticExternAbiV1::C { unwind: false },
        false,
        true,
        1,
        vec![SemanticAbiArgumentV1::source(direct_value(u32_id))],
        direct_value(u32_id),
    )
    .unwrap();
    request(
        vec![u32_type(1)],
        vec![],
        vec![
            function(
                1,
                abi(1, vec![u32_id], u32_id),
                vec![
                    local(1, u32_id, SemanticLocalRoleV1::Return),
                    local(2, u32_id, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![
                    block(1, vec![], call(1, 1, 1)),
                    block(
                        2,
                        vec![],
                        call(2, second_extra_arguments, second_variadic_abis),
                    ),
                    block(3, vec![], SemanticTerminatorKindV1::Return),
                ],
            ),
            function(
                2,
                variadic_abi,
                vec![
                    local(1, u32_id, SemanticLocalRoleV1::Return),
                    local(2, u32_id, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
            ),
        ],
    )
}

#[test]
fn variadic_abi_tails_are_per_call_canonical_and_exact() {
    let one_then_two = variadic_call_request(2, 2)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let one_then_one = variadic_call_request(1, 1)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let SemanticTerminatorKindV1::Call(first) =
        one_then_two.functions()[0].blocks()[0].terminator().kind()
    else {
        panic!("expected first direct call");
    };
    let SemanticTerminatorKindV1::Call(second) =
        one_then_two.functions()[0].blocks()[1].terminator().kind()
    else {
        panic!("expected second direct call");
    };
    assert_eq!(first.variadic_argument_abis().len(), 1);
    assert_eq!(second.variadic_argument_abis().len(), 2);
    assert_ne!(
        one_then_two.semantic_sha256(),
        one_then_one.semantic_sha256()
    );

    assert!(matches!(
        variadic_call_request(2, 1).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { tail: false, .. })
    ));
}

fn direct_call_to_extern_abi(extern_abi: SemanticExternAbiV1) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let canon_abi = match extern_abi {
        SemanticExternAbiV1::Custom => SemanticCanonAbiV1::Custom,
        SemanticExternAbiV1::GpuKernel => SemanticCanonAbiV1::GpuKernel,
        _ => panic!("test only constructs non-callable entry ABIs"),
    };
    let callee_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        canon_abi,
        extern_abi,
        false,
        false,
        0,
        vec![],
        direct_value(u32_id),
    )
    .unwrap();
    request(
        vec![u32_type(1)],
        vec![],
        vec![
            function(
                1,
                abi(1, vec![], u32_id),
                vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
                vec![
                    block(
                        1,
                        vec![],
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new(
                                SemanticFunctionIdV1::from_index(1),
                                vec![],
                                Some(SemanticCallDestinationV1::new(
                                    SemanticPlaceV1::new(
                                        SemanticLocalIdV1::from_index(0),
                                        vec![],
                                        u32_id,
                                    )
                                    .unwrap(),
                                    SemanticControlFlowEdgeV1::new(
                                        SemanticEdgeRoleV1::CallReturn,
                                        SemanticBlockIdV1::from_index(1),
                                    ),
                                )),
                                SemanticUnwindActionV1::Unreachable,
                            )
                            .unwrap(),
                        ),
                    ),
                    block(2, vec![], SemanticTerminatorKindV1::Return),
                ],
            ),
            function(
                2,
                callee_abi,
                vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
                vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
            ),
        ],
    )
}

#[test]
fn custom_and_gpu_entry_abis_are_not_ordinary_call_targets() {
    for extern_abi in [SemanticExternAbiV1::Custom, SemanticExternAbiV1::GpuKernel] {
        assert!(matches!(
            direct_call_to_extern_abi(extern_abi).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidCallShape { tail: false, .. })
        ));
    }
}

fn device_import_callable_request(
    effects: SemanticDeviceFfiEffectsV1,
    called: bool,
    tail: bool,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let pointer_id = SemanticTypeIdV1::from_index(1);
    let callable_pointer_statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], pointer_id).unwrap(),
            SemanticRvalueV1::new(
                pointer_id,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    pointer_id,
                    SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
                        0,
                        SemanticPointerProvenanceV1::Allocation(
                            SemanticAllocationIdV1::from_index(0),
                        ),
                    )),
                ))),
            ),
        )),
    );
    let root = function(
        1,
        abi(1, vec![], u32_id),
        vec![
            local(1, u32_id, SemanticLocalRoleV1::Return),
            local(2, pointer_id, SemanticLocalRoleV1::Temporary),
        ],
        if !called {
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)]
        } else if tail {
            vec![block(
                1,
                vec![],
                SemanticTerminatorKindV1::TailCall(
                    SemanticDirectTailCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![],
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            )]
        } else {
            vec![
                block(
                    1,
                    vec![callable_pointer_statement],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(1),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(0),
                                    vec![],
                                    u32_id,
                                )
                                .unwrap(),
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::CallReturn,
                                    SemanticBlockIdV1::from_index(1),
                                ),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(2, vec![], SemanticTerminatorKindV1::Return),
            ]
        },
    )
    .with_role(SemanticFunctionRoleV1::KernelRoot);
    let import_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticCanonAbiV1::C,
        SemanticExternAbiV1::C { unwind: false },
        false,
        false,
        0,
        vec![],
        direct_value(u32_id),
    )
    .unwrap();
    let binding = SemanticNonBodyCallableBindingV1::new(
        function_identity(2),
        item_identity(2),
        monomorphization_identity(2),
        generic_types_identity(2),
        const_generics_identity(2),
        SemanticSourceProvenanceV1::unavailable(),
        import_abi,
    );
    let contract = SemanticDeviceFfiImportContractV1::new(
        SemanticDeviceFfiContractIdentityV1::from_sha256(bytes(10)),
        SemanticLinkSymbolV1::new(b"device_import".to_vec()).unwrap(),
        SemanticDeviceFfiTargetV1::AmdGpuGfx942XnackMinus,
        SemanticCodeObjectVersionV1::V6,
        SemanticDeviceFfiPhysicalAbiIdentityV1::from_sha256(bytes(11)),
        effects,
        SemanticDeviceFfiSemanticIdentityV1::from_sha256(bytes(12)),
    );
    let allocations = if called && !tail {
        vec![
            SemanticAllocationDeclV1::new(
                SemanticAllocationIdentityV1::from_sha256(bytes(1)),
                vec![0; 8],
                vec![u8::MAX],
                8,
                false,
                vec![
                    SemanticRelocationV1::new(
                        0,
                        8,
                        0,
                        SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(1)),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        ]
    } else {
        vec![]
    };
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        vec![
            u32_type(1),
            pointer_kind_type(
                2,
                u32_id,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
            ),
        ],
        allocations,
        vec![],
        vec![],
        vec![root],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::DeviceFfiImport { binding, contract },
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn non_body_callables_are_canonical_reachable_terminals() {
    let none = device_import_callable_request(SemanticDeviceFfiEffectsV1::none(), true, false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let reads = device_import_callable_request(
        SemanticDeviceFfiEffectsV1::new(SemanticDeviceFfiEffectsV1::READ_GLOBAL).unwrap(),
        true,
        false,
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(none.callables().len(), 2);
    assert_ne!(none.semantic_sha256(), reads.semantic_sha256());
    assert!(matches!(
        device_import_callable_request(SemanticDeviceFfiEffectsV1::none(), false, false)
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::CallableOutsideRootClosure { .. })
    ));
    assert!(matches!(
        device_import_callable_request(SemanticDeviceFfiEffectsV1::none(), true, true)
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { tail: true, .. })
    ));
}

fn compiler_intrinsic_callable_request(output: SemanticTypeIdV1) -> InertSemanticMirRequestV1 {
    compiler_intrinsic_callable_request_with_relocation(output, false)
}

fn compiler_intrinsic_callable_request_with_relocation(
    output: SemanticTypeIdV1,
    address_taken: bool,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let pointer_id = SemanticTypeIdV1::from_index(2);
    let statements = if address_taken {
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], pointer_id).unwrap(),
                SemanticRvalueV1::new(
                    pointer_id,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            pointer_id,
                            SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
                                0,
                                SemanticPointerProvenanceV1::Allocation(
                                    SemanticAllocationIdV1::from_index(0),
                                ),
                            )),
                        ),
                    )),
                ),
            )),
        )]
    } else {
        vec![]
    };
    let root = function(
        1,
        abi(1, vec![], u32_id),
        vec![
            local(1, u32_id, SemanticLocalRoleV1::Return),
            local(
                2,
                SemanticTypeIdV1::from_index(1),
                SemanticLocalRoleV1::Temporary,
            ),
            local(3, pointer_id, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                1,
                statements,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], u32_id)
                                .unwrap(),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_role(SemanticFunctionRoleV1::KernelRoot);
    let intrinsic_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        direct_value(output),
    )
    .unwrap();
    let binding = SemanticNonBodyCallableBindingV1::new(
        function_identity(2),
        item_identity(2),
        monomorphization_identity(2),
        generic_types_identity(2),
        const_generics_identity(2),
        SemanticSourceProvenanceV1::unavailable(),
        intrinsic_abi,
    );
    let allocations = if address_taken {
        vec![
            SemanticAllocationDeclV1::new(
                SemanticAllocationIdentityV1::from_sha256(bytes(1)),
                vec![0; 8],
                vec![u8::MAX],
                8,
                false,
                vec![
                    SemanticRelocationV1::new(
                        0,
                        8,
                        0,
                        SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(1)),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        ]
    } else {
        vec![]
    };
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        vec![
            u32_type(1),
            unit_type(2),
            pointer_kind_type(
                3,
                u32_id,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
            ),
        ],
        allocations,
        vec![],
        vec![],
        vec![root],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X),
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(20)),
            },
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn compiler_intrinsics_require_exact_signatures_and_hostile_types_are_total() {
    compiler_intrinsic_callable_request(SemanticTypeIdV1::from_index(0))
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert!(matches!(
        compiler_intrinsic_callable_request(SemanticTypeIdV1::from_index(1))
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    let hostile = std::panic::catch_unwind(|| {
        compiler_intrinsic_callable_request(SemanticTypeIdV1::from_index(99))
            .admit(SemanticMirLimitsV1::default())
    });
    assert!(hostile.is_ok_and(|admission| admission.is_err()));
    assert!(matches!(
        compiler_intrinsic_callable_request_with_relocation(SemanticTypeIdV1::from_index(0), true,)
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

const READ_VIEW_ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const READ_VIEW_USIZE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const READ_VIEW_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const READ_VIEW_SLICE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const READ_VIEW_INVARIANT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const READ_VIEW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const READ_VIEW_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn read_view_usize_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(41),
        layout_identity(41),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    )
}

fn read_view_slice_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(42),
        layout_identity(42),
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
            element: READ_VIEW_ELEMENT,
        },
    )
}

fn read_view_shared_slice_reference_type() -> SemanticTypeDeclV1 {
    let data = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    SemanticTypeDeclV1::new(
        type_identity(43),
        layout_identity(43),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(data, length),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                READ_VIEW_SLICE,
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
    )
}

fn read_view_type(field_count: usize) -> SemanticTypeDeclV1 {
    let mut fields = vec![
        READ_VIEW_SLICE_REF,
        READ_VIEW_USIZE,
        READ_VIEW_USIZE,
        READ_VIEW_USIZE,
        READ_VIEW_USIZE,
    ];
    fields.extend(std::iter::repeat_n(
        READ_VIEW_INVARIANT,
        field_count.saturating_sub(fields.len()),
    ));
    fields.truncate(field_count);
    let mut offsets = vec![0, 16, 24, 32, 40];
    offsets.extend(std::iter::repeat_n(
        48,
        field_count.saturating_sub(offsets.len()),
    ));
    offsets.truncate(field_count);
    SemanticTypeDeclV1::new(
        type_identity(45),
        layout_identity(45),
        SemanticTypeLayoutV1::aggregate(
            Some(48),
            8,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn read_view_reference_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(46),
        layout_identity(46),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                READ_VIEW,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    48,
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn read_view_load_abi(identity: u8) -> SemanticFunctionAbiV1 {
    let shared_reference = SemanticAbiValueV1::new(
        READ_VIEW_REF,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                48,
                Some(8),
            )
            .unwrap(),
        ),
    );
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(identity)),
        layout_identity(identity),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![
            shared_reference,
            direct_value(READ_VIEW_USIZE),
            direct_value(READ_VIEW_USIZE),
            direct_value(READ_VIEW_ELEMENT),
        ],
        direct_value(READ_VIEW_ELEMENT),
    )
    .unwrap()
}

fn strided_read_load_request(field_count: usize) -> InertSemanticMirRequestV1 {
    let argument_types = vec![
        READ_VIEW_REF,
        READ_VIEW_USIZE,
        READ_VIEW_USIZE,
        READ_VIEW_ELEMENT,
    ];
    let root = function(
        48,
        read_view_load_abi(48),
        vec![
            local(48, READ_VIEW_ELEMENT, SemanticLocalRoleV1::Return),
            local(49, READ_VIEW_REF, SemanticLocalRoleV1::Argument(0)),
            local(50, READ_VIEW_USIZE, SemanticLocalRoleV1::Argument(1)),
            local(51, READ_VIEW_USIZE, SemanticLocalRoleV1::Argument(2)),
            local(52, READ_VIEW_ELEMENT, SemanticLocalRoleV1::Argument(3)),
        ],
        vec![
            block(
                48,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        (1..=4)
                            .map(|local| {
                                SemanticOperandV1::Copy(
                                    SemanticPlaceV1::new(
                                        SemanticLocalIdV1::from_index(local),
                                        vec![],
                                        argument_types[local as usize - 1],
                                    )
                                    .unwrap(),
                                )
                            })
                            .collect(),
                        Some(SemanticCallDestinationV1::new(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(0),
                                vec![],
                                READ_VIEW_ELEMENT,
                            )
                            .unwrap(),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(49, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_role(SemanticFunctionRoleV1::KernelRoot);
    let binding = SemanticNonBodyCallableBindingV1::new(
        function_identity(49),
        item_identity(49),
        monomorphization_identity(49),
        generic_types_identity(49),
        const_generics_identity(49),
        SemanticSourceProvenanceV1::unavailable(),
        read_view_load_abi(49),
    );
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        vec![
            u32_type(40),
            read_view_usize_type(),
            read_view_slice_type(),
            read_view_shared_slice_reference_type(),
            unit_type(44),
            read_view_type(field_count),
            read_view_reference_type(),
        ],
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr {
                    view: READ_VIEW,
                    element: READ_VIEW_ELEMENT,
                },
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(49)),
            },
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn strided_read_view_v5_is_closed_canonical_and_requires_exact_six_field_layout() {
    let admitted = strided_read_load_request(6)
        .admit_exact_v5(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V5);
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v5_canonical(
        admitted.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    assert!(matches!(
        strided_read_load_request(6).admit_exact_v4(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V4,
            required: SemanticMirWireVersionV1::V5,
        })
    ));
    for field_count in [5, 7] {
        assert!(matches!(
            strided_read_load_request(field_count).admit_exact_v5(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }

    for end in 0..admitted.canonical_encoding().len() {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v5_canonical(
                &admitted.canonical_encoding()[..end],
                SemanticMirLimitsV1::default(),
            )
            .is_err(),
            "V5 decoder accepted truncation at byte {end}"
        );
    }
    let mut invalid_tag = admitted.canonical_encoding().to_vec();
    let operation_prefix = [38, 5, 0, 0, 0, 0, 0, 0, 0];
    let operation_tag = invalid_tag
        .windows(operation_prefix.len())
        .position(|window| window == operation_prefix)
        .expect("exact strided read intrinsic operation record");
    invalid_tag[operation_tag] = u8::MAX;
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v5_canonical(
            &invalid_tag,
            SemanticMirLimitsV1::default(),
        )
        .is_err()
    );
}

const FILL_ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const FILL_RAW_INDEX: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const FILL_UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const FILL_INDEX_MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const FILL_ELEMENT_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const FILL_INDEX_WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const FILL_INDEX_WITNESS_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const FILL_DISJOINT_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const FILL_DISJOINT_SLICE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const FILL_ELEMENT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const FILL_ACCESS_RESULT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const FILL_DISCRIMINANT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);

fn fill_usize_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    )
}

fn fill_discriminant_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(12),
        layout_identity(12),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::integer(true, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: true,
            bits: 64,
        }),
    )
}

fn fill_index_marker_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(4),
        layout_identity(4),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}

fn fill_raw_element_pointer_type() -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    SemanticTypeDeclV1::new(
        type_identity(5),
        layout_identity(5),
        scalar_layout(
            8,
            8,
            pointer,
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                FILL_ELEMENT,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn fill_index_witness_type() -> SemanticTypeDeclV1 {
    let raw_index = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    SemanticTypeDeclV1::new(
        type_identity(6),
        layout_identity(6),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(raw_index),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![FILL_RAW_INDEX, FILL_INDEX_MARKER]).unwrap(),
        ),
    )
}

fn fill_reference_type(
    identity: u8,
    pointee: SemanticTypeIdV1,
    mutability: SemanticMutabilityV1,
) -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let (kind, pointee_size, pointee_alignment) = match (pointee, mutability) {
        (FILL_INDEX_WITNESS, SemanticMutabilityV1::Immutable) => (
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            8,
            8,
        ),
        (FILL_DISJOINT_SLICE, SemanticMutabilityV1::Mutable) => (
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
            16,
            8,
        ),
        (FILL_ELEMENT, SemanticMutabilityV1::Mutable) => (
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
            4,
            4,
        ),
        _ => panic!("unexpected fill reference relationship"),
    };
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            8,
            8,
            pointer,
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(kind, pointee_size, pointee_alignment).unwrap()),
            None,
        ),
    )
}

fn fill_disjoint_slice_type() -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let raw_index = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    SemanticTypeDeclV1::new(
        type_identity(8),
        layout_identity(8),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::ScalarPair {
                first: pointer,
                second: raw_index,
            },
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                FILL_ELEMENT_POINTER,
                FILL_RAW_INDEX,
                FILL_INDEX_MARKER,
            ])
            .unwrap(),
        ),
    )
}

fn fill_access_result_type() -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let source_niche = SemanticLayoutNicheV1::new(
        0,
        pointer,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();
    let empty = SemanticEnumVariantLayoutV1::from_rustc(
        0,
        8,
        8,
        SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
        SemanticBackendReprV1::memory(true),
        None,
        false,
        None,
        8,
        100,
        SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
    )
    .unwrap();
    let occupied = SemanticEnumVariantLayoutV1::from_rustc(
        1,
        8,
        8,
        SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            pointer,
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )),
        Some(source_niche),
        false,
        None,
        8,
        101,
        SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
    )
    .unwrap();
    let niche = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0).unwrap(),
        source_niche,
        SemanticBackendScalarV1::initialized(pointer, SemanticScalarValidityRangeV1::new(1, 0)),
        1,
        0,
        0,
        0,
    )
    .unwrap();
    let enum_layout =
        SemanticEnumLayoutV1::new(vec![empty, occupied], SemanticEnumEncodingV1::Niche(niche))
            .unwrap();
    SemanticTypeDeclV1::new(
        type_identity(11),
        layout_identity(11),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                pointer,
                SemanticScalarValidityRangeV1::new(1, 0),
            )),
            false,
            enum_layout,
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            FILL_DISCRIMINANT,
            vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![FILL_ELEMENT_REF]).unwrap(),
                ),
            ],
        )
        .unwrap(),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn fill_intrinsic_argument(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let (regular, pointee_size, pointee_alignment) = match ty {
        FILL_INDEX_WITNESS_REF => (
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                true,
                false,
                true,
            ),
            8,
            Some(8),
        ),
        FILL_DISJOINT_SLICE_REF => (
            SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
            16,
            Some(8),
        ),
        _ => return direct_value(ty),
    };
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                regular,
                SemanticAbiExtensionV1::None,
                pointee_size,
                pointee_alignment,
            )
            .unwrap(),
        ),
    )
}

fn fill_intrinsic_abi(
    identity: u8,
    inputs: Vec<SemanticTypeIdV1>,
    output: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let return_value = if output == FILL_ACCESS_RESULT {
        SemanticAbiValueV1::new(
            output,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    Some(4),
                )
                .unwrap(),
            ),
        )
    } else {
        direct_value(output)
    };
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(identity)),
        layout_identity(identity),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        inputs.into_iter().map(fill_intrinsic_argument).collect(),
        return_value,
    )
    .unwrap()
}

fn fill_intrinsic_binding(
    identity: u8,
    abi: SemanticFunctionAbiV1,
) -> SemanticNonBodyCallableBindingV1 {
    SemanticNonBodyCallableBindingV1::new(
        function_identity(identity),
        item_identity(identity),
        monomorphization_identity(identity),
        generic_types_identity(identity),
        const_generics_identity(identity),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
    )
}

fn fill_intrinsic_operations() -> [SemanticCompilerIntrinsicOperationV1; 3] {
    [
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness: FILL_INDEX_WITNESS,
            raw_index: FILL_RAW_INDEX,
        },
        SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness: FILL_INDEX_WITNESS,
            raw_index: FILL_RAW_INDEX,
        },
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice: FILL_DISJOINT_SLICE,
            index_witness: FILL_INDEX_WITNESS,
            element: FILL_ELEMENT,
            raw_index: FILL_RAW_INDEX,
        },
    ]
}

fn fill_intrinsic_request(
    operations: [SemanticCompilerIntrinsicOperationV1; 3],
) -> InertSemanticMirRequestV1 {
    let root_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![
            fill_intrinsic_argument(FILL_INDEX_WITNESS_REF),
            fill_intrinsic_argument(FILL_DISJOINT_SLICE_REF),
        ],
        SemanticAbiValueV1::new(FILL_UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let call_destination = |local_index, ty, target_index| {
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local_index), vec![], ty).unwrap(),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(target_index),
            ),
        ))
    };
    let root = function(
        1,
        root_abi,
        vec![
            local(1, FILL_UNIT, SemanticLocalRoleV1::Return),
            local(2, FILL_INDEX_WITNESS, SemanticLocalRoleV1::Temporary),
            local(3, FILL_INDEX_WITNESS_REF, SemanticLocalRoleV1::Argument(0)),
            local(4, FILL_DISJOINT_SLICE_REF, SemanticLocalRoleV1::Argument(1)),
            local(5, FILL_RAW_INDEX, SemanticLocalRoleV1::Temporary),
            local(6, FILL_ACCESS_RESULT, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                1,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![],
                        call_destination(1, FILL_INDEX_WITNESS, 1),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(
                2,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(2),
                        vec![SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(2),
                                vec![],
                                FILL_INDEX_WITNESS_REF,
                            )
                            .unwrap(),
                        )],
                        call_destination(4, FILL_RAW_INDEX, 2),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(
                3,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(3),
                        vec![
                            SemanticOperandV1::Copy(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(3),
                                    vec![],
                                    FILL_DISJOINT_SLICE_REF,
                                )
                                .unwrap(),
                            ),
                            SemanticOperandV1::Move(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(1),
                                    vec![],
                                    FILL_INDEX_WITNESS,
                                )
                                .unwrap(),
                            ),
                        ],
                        call_destination(5, FILL_ACCESS_RESULT, 3),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(4, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_role(SemanticFunctionRoleV1::KernelRoot);
    let bindings = [
        fill_intrinsic_binding(20, fill_intrinsic_abi(20, vec![], FILL_INDEX_WITNESS)),
        fill_intrinsic_binding(
            21,
            fill_intrinsic_abi(21, vec![FILL_INDEX_WITNESS_REF], FILL_RAW_INDEX),
        ),
        fill_intrinsic_binding(
            22,
            fill_intrinsic_abi(
                22,
                vec![FILL_DISJOINT_SLICE_REF, FILL_INDEX_WITNESS],
                FILL_ACCESS_RESULT,
            ),
        ),
    ];
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        vec![
            u32_type(1),
            fill_usize_type(),
            unit_type(3),
            fill_index_marker_type(),
            fill_raw_element_pointer_type(),
            fill_index_witness_type(),
            fill_reference_type(7, FILL_INDEX_WITNESS, SemanticMutabilityV1::Immutable),
            fill_disjoint_slice_type(),
            fill_reference_type(9, FILL_DISJOINT_SLICE, SemanticMutabilityV1::Mutable),
            fill_reference_type(10, FILL_ELEMENT, SemanticMutabilityV1::Mutable),
            fill_access_result_type(),
            fill_discriminant_type(),
        ],
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: bindings[0].clone(),
                operation: operations[0],
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(30)),
            },
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: bindings[1].clone(),
                operation: operations[1],
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(31)),
            },
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: bindings[2].clone(),
                operation: operations[2],
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(32)),
            },
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

#[test]
fn production_fill_intrinsics_preserve_typed_safety_relationships_and_pinned_encoding() {
    let admitted = fill_intrinsic_request(fill_intrinsic_operations())
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_canonical(
        admitted.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(admitted.callables().len(), 4);
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    assert_eq!(decoded.functions(), admitted.functions());
    assert_eq!(decoded.callables(), admitted.callables());
    assert_eq!(decoded.roots(), admitted.roots());
    let option_producers =
        semantic_option_producers_v1(&admitted.functions()[0], admitted.callables()).unwrap();
    assert_eq!(option_producers.len(), 1);
    assert_eq!(option_producers[0].option_local().index(), 5);
    assert_eq!(option_producers[0].continuation().index(), 3);
    assert_eq!(
        admitted.semantic_sha256().as_bytes(),
        &[
            0x30, 0x9f, 0x96, 0xaa, 0x6d, 0xdf, 0x3e, 0x50, 0xe1, 0x61, 0xa9, 0x1a, 0x72, 0x0c,
            0xcc, 0x14, 0xc0, 0xfe, 0x4d, 0x6a, 0x77, 0xc5, 0x50, 0x68, 0xcf, 0x0c, 0x26, 0x71,
            0xb4, 0xb5, 0xb6, 0xb8,
        ]
    );
}

#[test]
fn production_fill_intrinsics_fail_closed_on_forged_type_relationships() {
    let mut operations = fill_intrinsic_operations();
    operations[0] = SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
        index_witness: FILL_INDEX_WITNESS,
        raw_index: FILL_ELEMENT,
    };
    assert!(matches!(
        fill_intrinsic_request(operations).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let mut operations = fill_intrinsic_operations();
    operations[1] = SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
        index_witness: FILL_DISJOINT_SLICE,
        raw_index: FILL_RAW_INDEX,
    };
    assert!(matches!(
        fill_intrinsic_request(operations).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let mut operations = fill_intrinsic_operations();
    operations[2] = SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
        disjoint_slice: FILL_DISJOINT_SLICE,
        index_witness: FILL_INDEX_WITNESS,
        element: FILL_RAW_INDEX,
        raw_index: FILL_RAW_INDEX,
    };
    assert!(matches!(
        fill_intrinsic_request(operations).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let hostile = std::panic::catch_unwind(|| {
        let mut operations = fill_intrinsic_operations();
        operations[2] = SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice: SemanticTypeIdV1::from_index(99),
            index_witness: SemanticTypeIdV1::from_index(98),
            element: SemanticTypeIdV1::from_index(97),
            raw_index: SemanticTypeIdV1::from_index(96),
        };
        fill_intrinsic_request(operations).admit(SemanticMirLimitsV1::default())
    });
    assert!(
        hostile.is_ok_and(|result| matches!(result, Err(SemanticMirErrorV1::InvalidFunctionAbi)))
    );
}

#[test]
fn ordinary_and_tail_calls_are_distinct_typed_records() {
    let ordinary = direct_call_request(false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let tail = direct_call_request(true)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert!(matches!(
        ordinary.functions()[0].blocks()[0].terminator().kind(),
        SemanticTerminatorKindV1::Call(_)
    ));
    assert!(matches!(
        tail.functions()[0].blocks()[0].terminator().kind(),
        SemanticTerminatorKindV1::TailCall(_)
    ));
    assert_ne!(ordinary.semantic_sha256(), tail.semantic_sha256());
}

#[test]
fn invalid_function_reference_and_tail_abi_fail_closed() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let bad_target = request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(
                1,
                vec![],
                SemanticTerminatorKindV1::TailCall(
                    SemanticDirectTailCallV1::new(
                        SemanticFunctionIdV1::from_index(99),
                        vec![],
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            )],
        )],
    );
    assert!(matches!(
        bad_target.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Callable,
            index: 99,
            ..
        })
    ));

    let bool_id = SemanticTypeIdV1::from_index(0);
    let u32_id = SemanticTypeIdV1::from_index(1);
    let incompatible = request(
        vec![bool_type(1), u32_type(2)],
        vec![],
        vec![
            function(
                1,
                abi(1, vec![], u32_id),
                vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
                vec![block(
                    1,
                    vec![],
                    SemanticTerminatorKindV1::TailCall(
                        SemanticDirectTailCallV1::new(
                            SemanticFunctionIdV1::from_index(1),
                            vec![],
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                )],
            ),
            function(
                2,
                SemanticFunctionAbiV1::new(
                    SemanticAbiIdentityV1::from_sha256(bytes(2)),
                    layout_identity(2),
                    SemanticCanonAbiV1::Rust,
                    true,
                    false,
                    vec![],
                    SemanticAbiValueV1::new(
                        bool_id,
                        SemanticAbiPassModeV1::Direct(noundef_attributes(
                            SemanticAbiExtensionV1::ZeroExtend,
                        )),
                    ),
                )
                .unwrap(),
                vec![local(1, bool_id, SemanticLocalRoleV1::Return)],
                vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
            ),
        ],
    );
    assert!(matches!(
        incompatible.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { tail: true, .. })
    ));
}

fn exact_source_abi(
    tag: u8,
    extern_abi: SemanticExternAbiV1,
    c_variadic: bool,
    argument_type: SemanticTypeIdV1,
    hidden_caller_location: bool,
) -> SemanticFunctionAbiV1 {
    let canon_abi = match extern_abi {
        SemanticExternAbiV1::C { .. } | SemanticExternAbiV1::System { .. } => SemanticCanonAbiV1::C,
        SemanticExternAbiV1::Rust => SemanticCanonAbiV1::Rust,
        _ => panic!("unsupported test ABI"),
    };
    let mut arguments = vec![SemanticAbiArgumentV1::source(direct_value(argument_type))];
    if hidden_caller_location {
        arguments.push(SemanticAbiArgumentV1::hidden(
            SemanticAbiHiddenArgumentRoleV1::CallerLocation,
            SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(
                            true,
                            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                            true,
                            true,
                            false,
                            true,
                        ),
                        SemanticAbiExtensionV1::None,
                        4,
                        Some(4),
                    )
                    .unwrap(),
                ),
            ),
        ));
    }
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        canon_abi,
        extern_abi,
        false,
        c_variadic,
        1,
        arguments,
        direct_value(argument_type),
    )
    .unwrap()
}

fn caller_location_pointer_type(identity: u8, pointee: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
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
    )
}

fn hostile_tail_request(
    types: Vec<SemanticTypeDeclV1>,
    caller_abi: SemanticFunctionAbiV1,
    caller_type: SemanticTypeIdV1,
    callee_abi: SemanticFunctionAbiV1,
    callee_type: SemanticTypeIdV1,
) -> InertSemanticMirRequestV1 {
    request(
        types,
        vec![],
        vec![
            function(
                1,
                caller_abi,
                vec![
                    local(1, caller_type, SemanticLocalRoleV1::Return),
                    local(2, caller_type, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![block(
                    1,
                    vec![],
                    SemanticTerminatorKindV1::TailCall(
                        SemanticDirectTailCallV1::new(
                            SemanticFunctionIdV1::from_index(1),
                            vec![SemanticOperandV1::Copy(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(1),
                                    vec![],
                                    caller_type,
                                )
                                .unwrap(),
                            )],
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                )],
            ),
            function(
                2,
                callee_abi,
                vec![
                    local(1, callee_type, SemanticLocalRoleV1::Return),
                    local(2, callee_type, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
            ),
        ],
    )
}

#[test]
fn tail_calls_require_exact_non_variadic_source_abis_and_type_ids() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let c = SemanticExternAbiV1::C { unwind: false };
    let system = SemanticExternAbiV1::System { unwind: false };
    let variadic = hostile_tail_request(
        vec![u32_type(1)],
        exact_source_abi(1, c, true, u32_id, false),
        u32_id,
        exact_source_abi(2, c, true, u32_id, false),
        u32_id,
    );
    assert!(matches!(
        variadic.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { tail: true, .. })
    ));

    let hidden = hostile_tail_request(
        vec![u32_type(1), caller_location_pointer_type(2, u32_id)],
        exact_source_abi(1, SemanticExternAbiV1::Rust, false, u32_id, true),
        u32_id,
        exact_source_abi(2, SemanticExternAbiV1::Rust, false, u32_id, false),
        u32_id,
    );
    assert!(matches!(
        hidden.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { tail: true, .. })
    ));

    let source_abi_mismatch = hostile_tail_request(
        vec![u32_type(1)],
        exact_source_abi(1, c, false, u32_id, false),
        u32_id,
        exact_source_abi(2, system, false, u32_id, false),
        u32_id,
    );
    assert!(matches!(
        source_abi_mismatch.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidCallShape { tail: true, .. })
    ));

    let second_u32 = SemanticTypeIdV1::from_index(1);
    let type_mismatch = hostile_tail_request(
        vec![u32_type(1), u32_type(2)],
        exact_source_abi(1, c, false, u32_id, false),
        u32_id,
        exact_source_abi(2, c, false, second_u32, false),
        second_u32,
    );
    assert!(matches!(
        type_mismatch.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::TypeMismatch { .. })
    ));
}

#[test]
fn request_and_aggregate_budgets_fail_before_admission() {
    assert!(matches!(
        SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Types, HARD_MAX_TYPES_V1 + 1,),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Types,
            ..
        })
    ));

    let no_work = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, 0)
        .unwrap();
    assert!(matches!(
        simple_request().admit(no_work),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));

    let no_encoding = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, 1)
        .unwrap();
    assert!(matches!(
        simple_request().admit(no_encoding),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));

    let no_statements = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::Statements, 0)
        .unwrap();
    let u32_id = SemanticTypeIdV1::from_index(0);
    let one_statement = request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(
                1,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Nop,
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    );
    assert!(matches!(
        one_statement.admit(no_statements),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Statements,
            actual: 1,
            max: 0,
        })
    ));
}

#[test]
fn allocation_references_and_relocations_are_validated() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let relocation = SemanticRelocationV1::new(
        0,
        4,
        0,
        SemanticRelocationTargetV1::Allocation(SemanticAllocationIdV1::from_index(99)),
    )
    .unwrap();
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        vec![0; 4],
        vec![0x0f],
        4,
        false,
        vec![relocation],
    )
    .unwrap();
    let model = request(
        vec![u32_type(1)],
        vec![allocation],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        )],
    );
    assert!(matches!(
        model.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Allocation,
            index: 99,
            ..
        })
    ));
}

#[test]
fn volatility_and_atomic_ordering_are_independent_and_checked() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let place = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], u32_id).unwrap();
    let load = SemanticMemoryLoadV1::new(
        place.clone(),
        SemanticVolatilityV1::NonVolatile,
        Some(SemanticAtomicAccessV1::new(
            SemanticAtomicOrderingV1::Release,
            SemanticAtomicScopeV1::System,
        )),
    );
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place,
            SemanticRvalueV1::new(u32_id, SemanticRvalueKindV1::Load(load)),
        )),
    );
    let model = request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(1, vec![statement], SemanticTerminatorKindV1::Return)],
        )],
    );
    assert!(matches!(
        model.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidAtomicOrdering {
            operation: SemanticAtomicOperationV1::Load,
            ordering: SemanticAtomicOrderingV1::Release,
            ..
        })
    ));
}

#[test]
fn assertions_drops_and_unwind_edges_remain_role_distinct() {
    let bool_id = SemanticTypeIdV1::from_index(0);
    let u32_id = SemanticTypeIdV1::from_index(1);
    let unit_id = SemanticTypeIdV1::from_index(2);
    let pointer_id = SemanticTypeIdV1::from_index(3);
    let condition = SemanticOperandV1::Constant(SemanticConstantV1::new(
        bool_id,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
    ));
    let drop_place =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], u32_id).unwrap();
    let drop_argument = SemanticAbiValueV1::new(
        pointer_id,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                SemanticAbiExtensionV1::None,
                4,
                Some(4),
            )
            .unwrap(),
        ),
    )
    .with_pointee_override(
        SemanticAbiPointeeInfoV1::new(
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
            4,
            4,
        )
        .unwrap(),
    );
    let drop_pointer_type = SemanticTypeDeclV1::new(
        type_identity(4),
        layout_identity(4),
        scalar_layout(
            8,
            8,
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                u32_id,
                SemanticMutabilityV1::Mutable,
                0,
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
    );
    let model = request(
        vec![bool_type(1), u32_type(2), unit_type(3), drop_pointer_type],
        vec![],
        vec![
            function(
                1,
                abi(1, vec![], u32_id),
                vec![
                    local(1, u32_id, SemanticLocalRoleV1::Return),
                    local(2, u32_id, SemanticLocalRoleV1::Temporary),
                ],
                vec![
                    block(
                        1,
                        vec![],
                        SemanticTerminatorKindV1::Assert {
                            condition,
                            expected: true,
                            message: SemanticAssertMessageV1::NullPointerDereference,
                            target: SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::AssertSuccess,
                                SemanticBlockIdV1::from_index(1),
                            ),
                            unwind: SemanticUnwindActionV1::Cleanup(
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::AssertUnwind,
                                    SemanticBlockIdV1::from_index(2),
                                ),
                            ),
                        },
                    ),
                    block(
                        2,
                        vec![],
                        SemanticTerminatorKindV1::Drop {
                            place: drop_place,
                            drop_glue: SemanticFunctionIdV1::from_index(1),
                            target: SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::DropReturn,
                                SemanticBlockIdV1::from_index(3),
                            ),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                    block(3, vec![], SemanticTerminatorKindV1::UnwindResume),
                    block(4, vec![], SemanticTerminatorKindV1::Return),
                ],
            ),
            function(
                2,
                SemanticFunctionAbiV1::new(
                    SemanticAbiIdentityV1::from_sha256(bytes(2)),
                    layout_identity(2),
                    SemanticCanonAbiV1::Rust,
                    false,
                    false,
                    vec![drop_argument],
                    SemanticAbiValueV1::new(unit_id, SemanticAbiPassModeV1::Ignore),
                )
                .unwrap(),
                vec![
                    local(1, unit_id, SemanticLocalRoleV1::Return),
                    local(2, pointer_id, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
            )
            .with_role(SemanticFunctionRoleV1::DropGlue(u32_id)),
        ],
    );
    model.admit(SemanticMirLimitsV1::default()).unwrap();
}

#[test]
fn deterministic_order_rejects_identity_and_root_reordering() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let functions = vec![
        function(
            2,
            abi(2, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        ),
        function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        ),
    ];
    let model = request(vec![u32_type(1)], vec![], functions);
    assert!(matches!(
        model.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::NonDeterministicOrder {
            entity: SemanticMirEntityV1::Function,
        })
    ));

    assert!(matches!(
        InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
            vec![u32_type(1)],
            vec![],
            vec![],
            vec![],
            vec![function(
                1,
                abi(1, vec![], u32_id),
                vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
                vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
            )],
            vec![
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
            ],
        ),
        Err(SemanticMirErrorV1::NonDeterministicOrder {
            entity: SemanticMirEntityV1::Root,
        })
    ));
}

fn aggregate_type_with_zero_sized_offset(offset: u64) -> SemanticTypeDeclV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let unit_id = SemanticTypeIdV1::from_index(1);
    SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(vec![offset, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![unit_id, u32_id]).unwrap(),
        ),
    )
}

fn enum_variant_layout(
    variant_index: u32,
    size_bytes: u64,
    alignment_bytes: u64,
    aggregate: SemanticAggregateLayoutV1,
    uninhabited: bool,
) -> SemanticEnumVariantLayoutV1 {
    enum_variant_layout_with_seed(
        variant_index,
        size_bytes,
        alignment_bytes,
        aggregate,
        uninhabited,
        u64::from(variant_index) + 100,
    )
}

fn enum_variant_layout_with_seed(
    variant_index: u32,
    size_bytes: u64,
    alignment_bytes: u64,
    aggregate: SemanticAggregateLayoutV1,
    uninhabited: bool,
    randomization_seed: u64,
) -> SemanticEnumVariantLayoutV1 {
    enum_variant_layout_with_niche_and_seed(
        variant_index,
        size_bytes,
        alignment_bytes,
        aggregate,
        None,
        uninhabited,
        randomization_seed,
    )
}

fn enum_variant_layout_with_niche_and_seed(
    variant_index: u32,
    size_bytes: u64,
    alignment_bytes: u64,
    aggregate: SemanticAggregateLayoutV1,
    largest_niche: Option<SemanticLayoutNicheV1>,
    uninhabited: bool,
    randomization_seed: u64,
) -> SemanticEnumVariantLayoutV1 {
    let offsets = aggregate.field_offsets().to_vec();
    let mut memory_order = (0..u32::try_from(offsets.len()).unwrap()).collect::<Vec<_>>();
    memory_order.sort_by_key(|index| offsets[*index as usize]);
    SemanticEnumVariantLayoutV1::from_rustc(
        variant_index,
        size_bytes,
        alignment_bytes,
        SemanticFieldsShapeV1::arbitrary(offsets, memory_order).unwrap(),
        SemanticBackendReprV1::memory(true),
        largest_niche,
        uninhabited,
        None,
        alignment_bytes,
        randomization_seed,
        aggregate,
    )
    .unwrap()
}

fn aggregate_layout_request(offset: u64) -> InertSemanticMirRequestV1 {
    request_with_structured_abi(
        vec![
            u32_type(1),
            unit_type(2),
            aggregate_type_with_zero_sized_offset(offset),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
}

#[test]
fn aggregate_offsets_padding_and_sized_state_are_structured_and_canonical() {
    let first = aggregate_layout_request(0)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let moved_zst = aggregate_layout_request(3)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_ne!(first.semantic_sha256(), moved_zst.semantic_sha256());

    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = first.types()[2].layout().details() else {
        panic!("aggregate layout details were not retained");
    };
    assert_eq!(layout.field_offsets(), &[0, 0]);
    assert!(layout.padding().is_empty());

    let sized_opaque = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let unsized_opaque = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::new(None, 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let sized = request_with_structured_abi(
        vec![u32_type(1), sized_opaque],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let dynamically_sized = request_with_structured_abi(
        vec![u32_type(1), unsized_opaque],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(sized.semantic_sha256(), dynamically_sized.semantic_sha256());
    assert_eq!(dynamically_sized.types()[1].layout().size_bytes(), None);
}

#[test]
fn str_layout_is_unsized_lossless_and_canonical() {
    let str_type = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::array(1, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
    .with_rust_type_kind(SemanticRustTypeKindV1::Str);
    let admitted = request_with_structured_abi(
        vec![u32_type(1), str_type],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_canonical(
        admitted.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();

    assert!(matches!(
        decoded.types()[1].shape(),
        SemanticTypeShapeV1::Opaque
    ));
    assert_eq!(
        decoded.types()[1].rust_type_kind(),
        SemanticRustTypeKindV1::Str
    );
    assert_eq!(decoded.types()[1].layout().rustc_size_bytes(), 0);
    assert_eq!(decoded.types()[1].layout().size_bytes(), None);
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
}

#[test]
fn explicit_padding_is_checked_without_inventing_padding_from_field_gaps() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let aggregate = |padding| {
        SemanticTypeDeclV1::new(
            type_identity(2),
            layout_identity(2),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(vec![0], padding).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![u32_id]).unwrap()),
        )
    };
    let implicit_gap = request_with_structured_abi(
        vec![u32_type(1), aggregate(vec![])],
        vec![],
        direct_value(u32_id),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let explicit_padding = request_with_structured_abi(
        vec![
            u32_type(1),
            aggregate(vec![SemanticPaddingV1::new(4, 4).unwrap()]),
        ],
        vec![],
        direct_value(u32_id),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(
        implicit_gap.semantic_sha256(),
        explicit_padding.semantic_sha256()
    );

    let overlaps_field = request_with_structured_abi(
        vec![
            u32_type(1),
            aggregate(vec![SemanticPaddingV1::new(0, 4).unwrap()]),
        ],
        vec![],
        direct_value(u32_id),
    );
    assert!(matches!(
        overlaps_field.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

#[test]
fn empty_and_nonzero_single_rustc_variants_are_exact() {
    let empty_fields = || SemanticAggregateTypeV1::new(vec![]).unwrap();
    let single = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 1 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            7,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            SemanticTypeIdV1::from_index(0),
            vec![
                SemanticEnumVariantV1::new_with_inhabitedness(0, empty_fields(), true),
                SemanticEnumVariantV1::new(1, empty_fields()),
            ],
        )
        .unwrap(),
    );
    let admitted_single = request_with_structured_abi(
        vec![u32_type(1), single],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(matches!(
        admitted_single.types()[1].layout().variants(),
        SemanticRustcVariantsV1::Single { index: 1 }
    ));

    let empty = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
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
        SemanticTypeShapeV1::enum_type(
            SemanticTypeIdV1::from_index(0),
            vec![
                SemanticEnumVariantV1::new_with_inhabitedness(0, empty_fields(), true),
                SemanticEnumVariantV1::new_with_inhabitedness(1, empty_fields(), true),
            ],
        )
        .unwrap(),
    );
    let admitted_empty = request_with_structured_abi(
        vec![u32_type(1), empty],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(matches!(
        admitted_empty.types()[1].layout().variants(),
        SemanticRustcVariantsV1::Empty
    ));
    assert_ne!(
        admitted_single.semantic_sha256(),
        admitted_empty.semantic_sha256()
    );
}

fn direct_enum_type(tag_offset: u64, second_discriminant: u128) -> SemanticTypeDeclV1 {
    direct_enum_type_with_seed(tag_offset, second_discriminant, 100)
}

fn scalar_result_like_niche_types() -> Vec<SemanticTypeDeclV1> {
    let discriminant = i32_type(1);
    let unit = unit_type(2);
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    let error_niche =
        SemanticLayoutNicheV1::new(0, primitive, SemanticScalarValidityRangeV1::new(1, 3)).unwrap();
    let error = SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(1, 3),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::ValidityScalar(
            SemanticValidityScalarTypeV1::new(
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
                vec![SemanticScalarValidityRangeV1::new(1, 3)],
            )
            .unwrap(),
        ),
    );
    let variants = vec![
        SemanticEnumVariantV1::new(
            0,
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(1)]).unwrap(),
        ),
        SemanticEnumVariantV1::new(
            1,
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(2)]).unwrap(),
        ),
    ];
    let layouts = vec![
        enum_variant_layout(
            0,
            4,
            4,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            false,
        ),
        enum_variant_layout_with_niche_and_seed(
            1,
            4,
            4,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            Some(error_niche),
            false,
            101,
        ),
    ];
    let encoding = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0).unwrap(),
        error_niche,
        SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(1, 0)),
        1,
        0,
        0,
        0,
    )
    .unwrap();
    let result = SemanticTypeDeclV1::new(
        type_identity(4),
        layout_identity(4),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            4,
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, 3),
            )),
            false,
            SemanticEnumLayoutV1::new(layouts, SemanticEnumEncodingV1::Niche(encoding)).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), variants).unwrap(),
    );
    vec![discriminant, unit, error, result]
}

#[test]
fn shared_scalar_enum_decoder_handles_result_like_niches() {
    let types = scalar_result_like_niche_types();
    let result = SemanticTypeIdV1::from_index(3);

    assert_eq!(
        semantic_scalar_enum_variant_v1(&types, result, SemanticScalarValueV1::new(0, 4).unwrap(),),
        Some(0),
    );
    for error in 1..=3 {
        assert_eq!(
            semantic_scalar_enum_variant_v1(
                &types,
                result,
                SemanticScalarValueV1::new(error, 4).unwrap(),
            ),
            Some(1),
        );
    }
    assert_eq!(
        semantic_scalar_enum_variant_v1(&types, result, SemanticScalarValueV1::new(4, 4).unwrap(),),
        None,
    );
}

#[test]
fn shared_scalar_enum_decoder_handles_pointer_backed_niches() {
    let discriminant = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    let pointer = SemanticTypeIdV1::from_index(2);
    let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let pointer_niche = SemanticLayoutNicheV1::new(
        0,
        primitive,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();
    let variants = vec![
        SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![pointer]).unwrap()),
    ];
    let layouts = vec![
        enum_variant_layout(
            0,
            8,
            8,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
        enum_variant_layout_with_niche_and_seed(
            1,
            8,
            8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            Some(pointer_niche),
            false,
            102,
        ),
    ];
    let encoding = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0).unwrap(),
        pointer_niche,
        SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(1, 0)),
        1,
        0,
        0,
        0,
    )
    .unwrap();
    let option = SemanticTypeDeclV1::new(
        type_identity(4),
        layout_identity(4),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            8,
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
            SemanticEnumLayoutV1::new(layouts, SemanticEnumEncodingV1::Niche(encoding)).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(discriminant, variants).unwrap(),
    );
    let types = vec![
        u32_type(1),
        unit_type(2),
        pointer_kind_type(
            3,
            unit,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
        ),
        option,
    ];
    let option = SemanticTypeIdV1::from_index(3);

    assert_eq!(
        semantic_scalar_enum_variant_v1(&types, option, SemanticScalarValueV1::new(0, 8).unwrap()),
        Some(0),
    );
    assert_eq!(
        semantic_scalar_enum_variant_v1(&types, option, SemanticScalarValueV1::new(1, 8).unwrap()),
        Some(1),
    );
}

fn direct_result_like_types() -> Vec<SemanticTypeDeclV1> {
    let discriminant = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    let variants = vec![
        SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![unit]).unwrap()),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![discriminant]).unwrap()),
    ];
    let layouts = vec![
        enum_variant_layout(
            0,
            8,
            4,
            SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
            false,
        ),
        enum_variant_layout(
            1,
            8,
            4,
            SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
            false,
        ),
    ];
    let result = SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
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
        SemanticTypeShapeV1::enum_type(discriminant, variants).unwrap(),
    );
    vec![u32_type(1), unit_type(2), result]
}

#[test]
fn shared_direct_enum_decoder_retains_payload_variants() {
    let types = direct_result_like_types();
    let result = SemanticTypeIdV1::from_index(2);

    assert_eq!(
        semantic_direct_enum_variant_v1(&types, result, SemanticScalarValueV1::new(0, 4).unwrap(),),
        Some(0),
    );
    assert_eq!(
        semantic_direct_enum_variant_v1(&types, result, SemanticScalarValueV1::new(1, 4).unwrap(),),
        Some(1),
    );
    assert_eq!(
        semantic_direct_enum_variant_v1(&types, result, SemanticScalarValueV1::new(2, 4).unwrap(),),
        None,
    );
}

fn transparent_result_wrapper_request(
    swap_arguments: bool,
    wrapper_computes: bool,
    reachable_helper: bool,
) -> InertSemanticMirRequestV1 {
    let discriminant = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    let result = SemanticTypeIdV1::from_index(2);
    let direct = || SemanticAbiPassModeV1::Direct(noundef_attributes(SemanticAbiExtensionV1::None));
    let result_mode = || {
        cast_mode(
            None,
            vec![],
            None,
            SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
            8,
            false,
            SemanticAbiValueAttributesV1::plain(),
        )
    };
    let function_abi = |identity: u8, output, output_mode| {
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(identity)),
            layout_identity(identity),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![
                SemanticAbiValueV1::new(discriminant, direct()),
                SemanticAbiValueV1::new(discriminant, direct()),
            ],
            SemanticAbiValueV1::new(output, output_mode),
        )
        .unwrap()
    };
    let argument = |local_index| {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local_index),
                vec![],
                discriminant,
            )
            .unwrap(),
        )
    };
    let mut wrapper_statements = vec![];
    if wrapper_computes {
        wrapper_statements.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], discriminant)
                    .unwrap(),
                SemanticRvalueV1::new(
                    discriminant,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            discriminant,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(7, 4).unwrap(),
                            ),
                        ),
                    )),
                ),
            )),
        ));
    }
    let mut call_arguments = vec![argument(1), argument(2)];
    if swap_arguments {
        call_arguments.swap(0, 1);
    }
    let wrapper = function(
        1,
        function_abi(1, unit, SemanticAbiPassModeV1::Ignore),
        vec![
            local(1, unit, SemanticLocalRoleV1::Return),
            local(2, discriminant, SemanticLocalRoleV1::Argument(0)),
            local(3, discriminant, SemanticLocalRoleV1::Argument(1)),
            local(4, result, SemanticLocalRoleV1::Temporary),
            local(5, discriminant, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                1,
                wrapper_statements,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(1),
                        call_arguments,
                        Some(SemanticCallDestinationV1::new(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], result)
                                .unwrap(),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let helper_terminator = if reachable_helper {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(2),
                vec![argument(1), argument(2)],
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], discriminant)
                        .unwrap(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    } else {
        SemanticTerminatorKindV1::Return
    };
    let mut helper_locals = vec![
        local(6, result, SemanticLocalRoleV1::Return),
        local(7, discriminant, SemanticLocalRoleV1::Argument(0)),
        local(8, discriminant, SemanticLocalRoleV1::Argument(1)),
    ];
    if reachable_helper {
        helper_locals.push(local(9, discriminant, SemanticLocalRoleV1::Temporary));
    }
    let mut helper_blocks = vec![block(
        3,
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], result).unwrap(),
                SemanticRvalueV1::new(
                    result,
                    SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::EnumVariant(0),
                        vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                            unit,
                            SemanticConstantValueV1::ZeroSized,
                        ))],
                    )
                    .unwrap(),
                ),
            )),
        )],
        helper_terminator,
    )];
    if reachable_helper {
        helper_blocks.push(block(4, vec![], SemanticTerminatorKindV1::Return));
    }
    let helper = function(
        2,
        function_abi(2, result, result_mode()),
        helper_locals,
        helper_blocks,
    );
    let mut functions = vec![wrapper, helper];
    if reachable_helper {
        functions.push(function(
            3,
            function_abi(3, discriminant, direct()),
            vec![
                local(10, discriminant, SemanticLocalRoleV1::Return),
                local(11, discriminant, SemanticLocalRoleV1::Argument(0)),
                local(12, discriminant, SemanticLocalRoleV1::Argument(1)),
            ],
            vec![block(
                5,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(0),
                            vec![],
                            discriminant,
                        )
                        .unwrap(),
                        SemanticRvalueV1::new(
                            discriminant,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                SemanticConstantV1::new(
                                    discriminant,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(0, 4).unwrap(),
                                    ),
                                ),
                            )),
                        ),
                    )),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        ));
    }
    request(direct_result_like_types(), vec![], functions)
}

#[test]
fn transparent_result_wrapper_requires_exact_forwarding_and_no_computation() {
    let admitted = transparent_result_wrapper_request(false, false, false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let selection = admitted.select_kernel_body_v1().unwrap();
    assert_eq!(selection.root().index(), 0);
    assert_eq!(selection.body().index(), 1);
    assert!(selection.has_transparent_result_wrapper());

    let with_reachable_helper = transparent_result_wrapper_request(false, false, true)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(
        with_reachable_helper.select_kernel_body_v1(),
        Some(selection)
    );

    let swapped = transparent_result_wrapper_request(true, false, false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(swapped.select_kernel_body_v1(), None);

    let computing = transparent_result_wrapper_request(false, true, false)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(computing.select_kernel_body_v1(), None);
}

fn direct_enum_type_with_seed(
    tag_offset: u64,
    second_discriminant: u128,
    first_variant_seed: u64,
) -> SemanticTypeDeclV1 {
    let padding = if tag_offset == 0 {
        vec![SemanticPaddingV1::new(1, 3).unwrap()]
    } else {
        vec![
            SemanticPaddingV1::new(0, tag_offset).unwrap(),
            SemanticPaddingV1::new(tag_offset + 1, 3 - tag_offset).unwrap(),
        ]
    };
    let variants = vec![
        SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
        SemanticEnumVariantV1::new(
            second_discriminant,
            SemanticAggregateTypeV1::new(vec![]).unwrap(),
        ),
    ];
    let variant_layouts = vec![
        enum_variant_layout_with_seed(
            0,
            4,
            1,
            SemanticAggregateLayoutV1::new(vec![], padding.clone()).unwrap(),
            false,
            first_variant_seed,
        ),
        enum_variant_layout(
            1,
            4,
            1,
            SemanticAggregateLayoutV1::new(vec![], padding).unwrap(),
            false,
        ),
    ];
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, second_discriminant),
    );
    SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::enum_layout(
            4,
            1,
            SemanticEnumLayoutV1::new(
                variant_layouts,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0, tag_offset, tag,
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), variants).unwrap(),
    )
}

#[test]
fn direct_enum_tag_is_zero_and_exact_variant_facts_change_identity() {
    let at_zero = request_with_structured_abi(
        vec![u32_type(1), direct_enum_type(0, 1)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let at_one = request_with_structured_abi(
        vec![u32_type(1), direct_enum_type(1, 1)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default());
    assert!(matches!(at_one, Err(SemanticMirErrorV1::InvalidTypeLayout)));
    let changed_discriminant = request_with_structured_abi(
        vec![u32_type(1), direct_enum_type(0, 2)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(
        at_zero.semantic_sha256(),
        changed_discriminant.semantic_sha256()
    );
    let changed_variant_seed = request_with_structured_abi(
        vec![u32_type(1), direct_enum_type_with_seed(0, 1, 999)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(
        at_zero.semantic_sha256(),
        changed_variant_seed.semantic_sha256()
    );

    let SemanticRustcVariantsV1::Multiple(layout) = at_zero.types()[1].layout().variants() else {
        panic!("multi-variant rustc layout was not retained");
    };
    let SemanticEnumEncodingV1::Direct(direct) = layout.encoding() else {
        panic!("direct enum encoding was not retained");
    };
    assert_eq!(direct.tag_offset_bytes(), 0);
    assert_eq!(
        at_zero.types()[1].layout().largest_niche(),
        Some(
            SemanticLayoutNicheV1::new(
                0,
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )
            .unwrap()
        )
    );
    assert_eq!(
        layout.variants()[0].aggregate().padding()[0].offset_bytes(),
        1
    );

    let signed_variants = vec![
        SemanticEnumVariantV1::new(
            u128::from(u32::MAX),
            SemanticAggregateTypeV1::new(vec![]).unwrap(),
        ),
        SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ];
    let signed_layouts = vec![
        enum_variant_layout(
            0,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
        enum_variant_layout(
            1,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
    ];
    let signed_enum = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::enum_layout(
            1,
            1,
            SemanticEnumLayoutV1::new(
                signed_layouts,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(true, 8, 1),
                        SemanticScalarValidityRangeV1::new(u8::MAX.into(), 0),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), signed_variants).unwrap(),
    );
    request_with_structured_abi(
        vec![i32_type(1), signed_enum],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
}

fn exact_tag_field_enum(tag_field: u32, tag_valid_end: u128) -> SemanticTypeDeclV1 {
    let variants = vec![
        SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ];
    let layouts = vec![
        enum_variant_layout(
            0,
            4,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
        enum_variant_layout(
            1,
            4,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
    ];
    let (tag_offset, outer_fields) = if tag_field == 0 {
        (
            0,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
        )
    } else {
        (
            1,
            SemanticFieldsShapeV1::arbitrary(vec![3, 1], vec![1, 0]).unwrap(),
        )
    };
    let multiple = SemanticEnumLayoutV1::new(
        layouts,
        SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
            tag_field,
            tag_offset,
            SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, tag_valid_end),
            ),
        )),
    )
    .unwrap();
    SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            4,
            1,
            outer_fields,
            SemanticRustcVariantsV1::Multiple(Box::new(multiple)),
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            55,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), variants).unwrap(),
    )
}

#[test]
fn nonzero_direct_tag_field_is_rejected_for_ordinary_enums() {
    let nonzero_tag_field = request_with_structured_abi(
        vec![u32_type(1), exact_tag_field_enum(1, 1)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default());
    assert!(matches!(
        nonzero_tag_field,
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));

    let excludes_second_discriminant = request_with_structured_abi(
        vec![u32_type(1), exact_tag_field_enum(0, 0)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        excludes_second_discriminant.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

fn niche_enum_type(
    niche_start: u128,
    claimed_validity: Vec<SemanticScalarValidityRangeV1>,
) -> SemanticTypeDeclV1 {
    let source_niche = SemanticLayoutNicheV1::new(
        0,
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        claimed_validity[0],
    )
    .unwrap();
    let variants = vec![
        SemanticEnumVariantV1::new(
            0,
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(1)]).unwrap(),
        ),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ];
    let layouts = vec![
        enum_variant_layout_with_niche_and_seed(
            0,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            Some(source_niche),
            false,
            100,
        ),
        enum_variant_layout(
            1,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
    ];
    let niche = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0).unwrap(),
        source_niche,
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 8, 1),
            SemanticScalarValidityRangeV1::new(claimed_validity[0].start(), 0),
        ),
        0,
        1,
        1,
        niche_start,
    )
    .unwrap();
    SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::enum_layout(
            1,
            1,
            SemanticEnumLayoutV1::new(layouts, SemanticEnumEncodingV1::Niche(niche)).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), variants).unwrap(),
    )
}

fn nonzero_niche_source_enum_type() -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 8, 1);
    let source_niche = SemanticLayoutNicheV1::new(
        1,
        primitive,
        SemanticScalarValidityRangeV1::new(1, u8::MAX.into()),
    )
    .unwrap();
    let variants = vec![
        SemanticEnumVariantV1::new(
            0,
            SemanticAggregateTypeV1::new(vec![
                SemanticTypeIdV1::from_index(1),
                SemanticTypeIdV1::from_index(1),
            ])
            .unwrap(),
        ),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ];
    let layouts = vec![
        enum_variant_layout_with_niche_and_seed(
            0,
            2,
            1,
            SemanticAggregateLayoutV1::new(vec![0, 1], vec![]).unwrap(),
            Some(source_niche),
            false,
            100,
        ),
        enum_variant_layout(
            1,
            2,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
    ];
    let niche = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(1)], 1).unwrap(),
        source_niche,
        SemanticBackendScalarV1::initialized(primitive, SemanticScalarValidityRangeV1::new(1, 0)),
        0,
        1,
        1,
        0,
    )
    .unwrap();
    SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::enum_layout(
            2,
            1,
            SemanticEnumLayoutV1::new(layouts, SemanticEnumEncodingV1::Niche(niche)).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), variants).unwrap(),
    )
}

#[test]
fn nonzero_niche_source_offset_is_retained_in_outer_layout() {
    let admitted = request_with_structured_abi(
        vec![
            u32_type(1),
            validity_u8_type(2),
            nonzero_niche_source_enum_type(),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(matches!(
        admitted.types()[2].layout().fields(),
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } if source_order_offsets_bytes.as_ref() == [1]
            && memory_order_source_indices.as_ref() == [0]
    ));
    assert_eq!(admitted.types()[2].layout().largest_niche(), None);
}

#[test]
fn niche_source_validity_and_encoding_are_retained_and_checked() {
    let ranges = vec![SemanticScalarValidityRangeV1::new(1, u8::MAX.into())];
    let admitted = request_with_structured_abi(
        vec![
            u32_type(1),
            validity_u8_type(2),
            niche_enum_type(0, ranges.clone()),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let SemanticRustcVariantsV1::Multiple(layout) = admitted.types()[2].layout().variants() else {
        panic!("multi-variant rustc layout was not retained");
    };
    let SemanticEnumEncodingV1::Niche(niche) = layout.encoding() else {
        panic!("niche encoding was not retained");
    };
    assert_eq!(
        niche.source().path(),
        &[SemanticNichePathComponentV1::Field(0)]
    );
    assert_eq!(niche.source_niche().valid_range(), ranges[0]);
    assert_eq!(niche.niche_variant_range(), (1, 1));

    let wider_niche = vec![SemanticScalarValidityRangeV1::new(2, u8::MAX.into())];
    let niche_zero = request_with_structured_abi(
        vec![
            u32_type(1),
            validity_u8_type_with_ranges(2, wider_niche.clone()),
            niche_enum_type(0, wider_niche.clone()),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let invalid_niche_one = request_with_structured_abi(
        vec![
            u32_type(1),
            validity_u8_type_with_ranges(2, wider_niche.clone()),
            niche_enum_type(1, wider_niche),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        invalid_niche_one.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
    assert_ne!(admitted.semantic_sha256(), niche_zero.semantic_sha256());

    let conflicting = request_with_structured_abi(
        vec![
            u32_type(1),
            validity_u8_type(2),
            niche_enum_type(
                0,
                vec![SemanticScalarValidityRangeV1::new(2, u8::MAX.into())],
            ),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        conflicting.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));

    let overlapping_niche = request_with_structured_abi(
        vec![u32_type(1), validity_u8_type(2), niche_enum_type(1, ranges)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        overlapping_niche.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

fn reference_u32_type(identity: u8) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            8,
            8,
            primitive,
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        ),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn reference_niche_enum_type() -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let source_niche = SemanticLayoutNicheV1::new(
        0,
        pointer,
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();
    let variants = vec![
        SemanticEnumVariantV1::new(
            0,
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(1)]).unwrap(),
        ),
        SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ];
    let layouts = vec![
        enum_variant_layout_with_niche_and_seed(
            0,
            8,
            8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            Some(source_niche),
            false,
            100,
        ),
        enum_variant_layout(
            1,
            8,
            8,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
    ];
    let niche = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0).unwrap(),
        source_niche,
        SemanticBackendScalarV1::initialized(pointer, SemanticScalarValidityRangeV1::new(1, 0)),
        0,
        1,
        1,
        0,
    )
    .unwrap();
    SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::enum_layout(
            8,
            8,
            SemanticEnumLayoutV1::new(layouts, SemanticEnumEncodingV1::Niche(niche)).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), variants).unwrap(),
    )
}

fn inclusive_dead_variant_niche_enum_type(outside_variant_uninhabited: bool) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 8, 1);
    let source_niche = SemanticLayoutNicheV1::new(
        0,
        primitive,
        SemanticScalarValidityRangeV1::new(3, u8::MAX.into()),
    )
    .unwrap();
    let variants = vec![
        SemanticEnumVariantV1::new_with_inhabitedness(
            0,
            SemanticAggregateTypeV1::new(vec![]).unwrap(),
            true,
        ),
        SemanticEnumVariantV1::new(
            1,
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(1)]).unwrap(),
        ),
        SemanticEnumVariantV1::new(2, SemanticAggregateTypeV1::new(vec![]).unwrap()),
        SemanticEnumVariantV1::new_with_inhabitedness(
            3,
            SemanticAggregateTypeV1::new(vec![]).unwrap(),
            outside_variant_uninhabited,
        ),
    ];
    let layouts = vec![
        enum_variant_layout(
            0,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            true,
        ),
        enum_variant_layout_with_niche_and_seed(
            1,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            Some(source_niche),
            false,
            101,
        ),
        enum_variant_layout(
            2,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            false,
        ),
        enum_variant_layout(
            3,
            1,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            outside_variant_uninhabited,
        ),
    ];
    let niche = SemanticNicheEnumEncodingV1::new(
        0,
        SemanticNicheSourceV1::new(vec![SemanticNichePathComponentV1::Field(0)], 0).unwrap(),
        source_niche,
        SemanticBackendScalarV1::initialized(
            primitive,
            SemanticScalarValidityRangeV1::new(0, u8::MAX.into()),
        ),
        1,
        0,
        2,
        0,
    )
    .unwrap();
    SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::enum_layout(
            1,
            1,
            SemanticEnumLayoutV1::new(layouts, SemanticEnumEncodingV1::Niche(niche)).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(SemanticTypeIdV1::from_index(0), variants).unwrap(),
    )
}

#[test]
fn pointer_and_inclusive_dead_variant_niches_match_rustc() {
    request_with_structured_abi(
        vec![
            u32_type(1),
            reference_u32_type(2),
            reference_niche_enum_type(),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    let source_range = vec![SemanticScalarValidityRangeV1::new(3, u8::MAX.into())];
    request_with_structured_abi(
        vec![
            u32_type(1),
            validity_u8_type_with_ranges(2, source_range.clone()),
            inclusive_dead_variant_niche_enum_type(true),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    let inhabited_outside_range = request_with_structured_abi(
        vec![
            u32_type(1),
            validity_u8_type_with_ranges(2, source_range),
            inclusive_dead_variant_niche_enum_type(false),
        ],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        inhabited_outside_range.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

fn abi_identity_for(mode: SemanticAbiPassModeV1) -> InertSemanticMirSha256V1 {
    let argument = SemanticTypeIdV1::from_index(0);
    let (types, return_type) = match mode {
        SemanticAbiPassModeV1::Pair { .. } => {
            let first = SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 16, 2),
                SemanticScalarValidityRangeV1::new(0, u16::MAX.into()),
            );
            let pair = SemanticTypeDeclV1::new(
                type_identity(1),
                layout_identity(1),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    2,
                    SemanticBackendReprV1::scalar_pair(first, first),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Opaque,
            );
            (vec![pair, u32_type(2)], SemanticTypeIdV1::from_index(1))
        }
        SemanticAbiPassModeV1::Cast { .. } | SemanticAbiPassModeV1::Indirect { .. } => {
            let memory = SemanticTypeDeclV1::new(
                type_identity(1),
                layout_identity(1),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    4,
                    SemanticBackendReprV1::memory(true),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Opaque,
            );
            (vec![memory, u32_type(2)], SemanticTypeIdV1::from_index(1))
        }
        SemanticAbiPassModeV1::Direct(_) | SemanticAbiPassModeV1::Ignore => {
            (vec![u32_type(1)], argument)
        }
    };
    request_with_structured_abi(
        types,
        vec![SemanticAbiValueV1::new(argument, mode)],
        direct_value(return_type),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
    .semantic_sha256()
}

fn pointer_abi_identity_for(attributes: SemanticAbiValueAttributesV1) -> InertSemanticMirSha256V1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let pointer_id = SemanticTypeIdV1::from_index(1);
    request_with_structured_abi(
        vec![u32_type(1), pointer_type(2, u32_id)],
        vec![SemanticAbiValueV1::new(
            pointer_id,
            SemanticAbiPassModeV1::Direct(attributes),
        )],
        direct_value(u32_id),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
    .semantic_sha256()
}

fn small_integer_abi_identity_for(
    signed: bool,
    extension: SemanticAbiExtensionV1,
) -> InertSemanticMirSha256V1 {
    let argument = SemanticTypeIdV1::from_index(0);
    let primitive = SemanticBackendPrimitiveV1::integer(signed, 8, 1);
    let ty = SemanticTypeDeclV1::new(
        type_identity(1),
        layout_identity(1),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, u8::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    request_with_structured_abi(
        vec![ty, u32_type(2)],
        vec![extended_direct_value(argument, extension)],
        direct_value(SemanticTypeIdV1::from_index(1)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
    .semantic_sha256()
}

fn attributes(
    regular: SemanticAbiRegularAttributesV1,
    extension: SemanticAbiExtensionV1,
    pointee_size: u64,
    pointee_alignment: Option<u64>,
) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(regular, extension, pointee_size, pointee_alignment).unwrap()
}

fn indirect_attributes(pointee_size: u64, pointee_alignment: u64) -> SemanticAbiValueAttributesV1 {
    attributes(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            true,
            false,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        pointee_size,
        Some(pointee_alignment),
    )
}

#[test]
fn valid_abi_attributes_change_canonical_identity() {
    let integer_flags = [SemanticAbiRegularAttributesV1::new(
        false, None, false, false, false, true,
    )];
    let mut identities = std::collections::BTreeSet::new();
    for regular in integer_flags {
        identities.insert(abi_identity_for(SemanticAbiPassModeV1::Direct(attributes(
            regular,
            SemanticAbiExtensionV1::None,
            0,
            None,
        ))));
    }
    identities.insert(small_integer_abi_identity_for(
        false,
        SemanticAbiExtensionV1::None,
    ));
    identities.insert(small_integer_abi_identity_for(
        true,
        SemanticAbiExtensionV1::None,
    ));
    identities.insert(pointer_abi_identity_for(attributes(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )));
    assert_eq!(identities.len(), 4);

    let rejected_pointer_flags = [
        SemanticAbiRegularAttributesV1::new(true, None, false, false, false, true),
        SemanticAbiRegularAttributesV1::new(
            false,
            Some(SemanticAbiPointerCaptureV1::CapturesNone),
            false,
            false,
            false,
            true,
        ),
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiRegularAttributesV1::new(false, None, false, true, false, true),
    ];
    for regular in rejected_pointer_flags {
        let result = request_with_structured_abi(
            vec![
                u32_type(1),
                pointer_type(2, SemanticTypeIdV1::from_index(0)),
            ],
            vec![SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticAbiPassModeV1::Direct(attributes(
                    regular,
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )),
            )],
            direct_value(SemanticTypeIdV1::from_index(0)),
        )
        .admit(SemanticMirLimitsV1::default());
        assert!(matches!(
            result,
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
    for (pointee_size, pointee_alignment) in [(4, None), (0, Some(4))] {
        let result = request_with_structured_abi(
            vec![
                u32_type(1),
                pointer_type(2, SemanticTypeIdV1::from_index(0)),
            ],
            vec![SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticAbiPassModeV1::Direct(attributes(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    pointee_size,
                    pointee_alignment,
                )),
            )],
            direct_value(SemanticTypeIdV1::from_index(0)),
        )
        .admit(SemanticMirLimitsV1::default());
        assert!(matches!(
            result,
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

fn cast_mode(
    padding: Option<SemanticAbiRegisterV1>,
    prefix: Vec<Option<SemanticAbiRegisterV1>>,
    rest_offset: Option<u64>,
    rest: SemanticAbiRegisterV1,
    rest_total: u64,
    consecutive: bool,
    attributes: SemanticAbiValueAttributesV1,
) -> SemanticAbiPassModeV1 {
    let mut exact_prefix = [None; 8];
    for (slot, register) in prefix.into_iter().enumerate() {
        exact_prefix[slot] = register;
    }
    SemanticAbiPassModeV1::cast(
        padding.is_some(),
        SemanticAbiCastV1::new(
            exact_prefix,
            rest_offset,
            SemanticAbiUniformV1::from_rustc(rest, rest_total, consecutive).unwrap(),
            attributes,
        ),
    )
}

#[test]
fn pass_modes_registers_cast_structure_and_indirect_facts_are_canonical() {
    let plain = SemanticAbiValueAttributesV1::plain();
    let pair_attributes = attributes(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    );
    let direct_attributes = noundef_attributes(SemanticAbiExtensionV1::None);
    let i32_register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 4).unwrap();
    let f32_register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Float, 4).unwrap();
    let i16_register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 2).unwrap();
    let i8_register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 1).unwrap();
    let modes = vec![
        SemanticAbiPassModeV1::Direct(direct_attributes),
        SemanticAbiPassModeV1::Pair {
            first: pair_attributes,
            second: pair_attributes,
        },
        cast_mode(None, vec![], None, i32_register, 4, false, plain),
    ];
    let mut identities = modes
        .into_iter()
        .map(abi_identity_for)
        .collect::<std::collections::BTreeSet<_>>();
    let indirect = request_with_structured_abi(
        vec![
            SemanticTypeDeclV1::new(
                type_identity(1),
                layout_identity(1),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(16),
                    8,
                    SemanticBackendReprV1::memory(true),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Opaque,
            ),
            u32_type(2),
        ],
        vec![SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Indirect {
                attributes: indirect_attributes(16, 8),
                metadata_attributes: None,
                on_stack: false,
            },
        )],
        direct_value(SemanticTypeIdV1::from_index(1)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    identities.insert(indirect.semantic_sha256());
    assert_eq!(identities.len(), 4);

    let rejected_modes = vec![
        cast_mode(
            Some(i8_register),
            vec![],
            None,
            i32_register,
            4,
            false,
            plain,
        ),
        cast_mode(
            None,
            vec![None, Some(i8_register)],
            None,
            i32_register,
            4,
            false,
            plain,
        ),
        cast_mode(
            None,
            vec![Some(i8_register)],
            None,
            i32_register,
            4,
            false,
            plain,
        ),
        cast_mode(
            None,
            vec![Some(i8_register)],
            Some(4),
            i32_register,
            4,
            false,
            plain,
        ),
        cast_mode(None, vec![], None, f32_register, 4, false, plain),
        cast_mode(None, vec![], None, i16_register, 4, false, plain),
        cast_mode(None, vec![], None, i32_register, 8, false, plain),
        cast_mode(None, vec![], None, i32_register, 4, true, plain),
        SemanticAbiPassModeV1::Indirect {
            attributes: indirect_attributes(4, 4),
            metadata_attributes: None,
            on_stack: false,
        },
    ];
    for mode in rejected_modes {
        let result = request_with_structured_abi(
            vec![
                SemanticTypeDeclV1::new(
                    type_identity(1),
                    layout_identity(1),
                    SemanticTypeLayoutV1::new_with_backend_repr(
                        Some(4),
                        4,
                        SemanticBackendReprV1::memory(true),
                        false,
                    )
                    .unwrap(),
                    SemanticTypeShapeV1::Opaque,
                ),
                u32_type(2),
            ],
            vec![SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(0),
                mode,
            )],
            direct_value(SemanticTypeIdV1::from_index(1)),
        )
        .admit(SemanticMirLimitsV1::default());
        assert!(matches!(
            result,
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }

    let admitted = request_with_structured_abi(
        vec![
            SemanticTypeDeclV1::new(
                type_identity(1),
                layout_identity(1),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(4),
                    4,
                    SemanticBackendReprV1::memory(true),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Opaque,
            ),
            u32_type(2),
        ],
        vec![SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            cast_mode(None, vec![], None, i32_register, 4, false, plain),
        )],
        direct_value(SemanticTypeIdV1::from_index(1)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(admitted.functions()[0].abi().arguments()[0].ty().index(), 0);
    assert!(matches!(
        admitted.functions()[0].abi().arguments()[0].mode(),
        SemanticAbiPassModeV1::Cast { .. }
    ));
    assert_eq!(admitted.functions()[0].abi().return_value().ty().index(), 1);
}

fn request_with_statement(
    types: Vec<SemanticTypeDeclV1>,
    function_abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    statement: SemanticStatementKindV1,
) -> InertSemanticMirRequestV1 {
    request(
        types,
        vec![],
        vec![function(
            1,
            function_abi,
            locals,
            vec![block(
                1,
                vec![SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    statement,
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    )
}

#[test]
fn assume_is_boolean_typed_and_round_trips_canonically() {
    let bool_id = SemanticTypeIdV1::from_index(0);
    let unit_id = SemanticTypeIdV1::from_index(1);
    let assume = |condition_type, extension, types| {
        request_with_statement(
            types,
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256(bytes(1)),
                layout_identity(1),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                vec![SemanticAbiValueV1::new(
                    condition_type,
                    SemanticAbiPassModeV1::Direct(noundef_attributes(extension)),
                )],
                SemanticAbiValueV1::new(unit_id, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap(),
            vec![
                local(1, unit_id, SemanticLocalRoleV1::Return),
                local(2, condition_type, SemanticLocalRoleV1::Argument(0)),
            ],
            SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], condition_type)
                    .unwrap(),
            )),
        )
    };

    let admitted = assume(
        bool_id,
        SemanticAbiExtensionV1::ZeroExtend,
        vec![bool_type(1), unit_type(2)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let decoded = AdmittedInertSemanticMirV1::decode_canonical(
        admitted.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
    assert!(matches!(
        decoded.functions()[0].blocks()[0].statements()[0].kind(),
        SemanticStatementKindV1::Assume(_)
    ));

    let u32_id = SemanticTypeIdV1::from_index(0);
    assert!(matches!(
        assume(
            u32_id,
            SemanticAbiExtensionV1::None,
            vec![u32_type(1), unit_type(2)],
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Assume,
            ..
        })
    ));
}

#[test]
fn abi_arguments_are_charged_against_the_configured_total() {
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CallArguments, 0)
        .unwrap();
    assert!(matches!(
        simple_request().admit(limits),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CallArguments,
            actual: 1,
            max: 0,
        })
    ));
}

fn request_with_dead_terminator(
    dead_terminator: SemanticTerminatorKindV1,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    request(
        vec![u32_type(1)],
        vec![],
        vec![function(
            1,
            abi(1, vec![], u32_id),
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![
                block(1, vec![], SemanticTerminatorKindV1::Return),
                block(2, vec![], dead_terminator),
            ],
        )],
    )
}

#[test]
fn unreachable_blocks_are_retained_for_the_deterministic_middle_end() {
    let abort = request_with_dead_terminator(SemanticTerminatorKindV1::Abort)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let unreachable = request_with_dead_terminator(SemanticTerminatorKindV1::Unreachable)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(abort.functions()[0].blocks().len(), 2);
    assert_ne!(abort.canonical_encoding(), unreachable.canonical_encoding());
    assert_ne!(abort.semantic_sha256(), unreachable.semantic_sha256());
}

#[test]
fn ill_typed_projection_and_constant_records_fail_closed() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let invalid_dereference = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, u32_id).unwrap()],
        u32_id,
    )
    .unwrap();
    let projection = request_with_statement(
        vec![u32_type(1)],
        abi(1, vec![], u32_id),
        vec![
            local(1, u32_id, SemanticLocalRoleV1::Return),
            local(2, u32_id, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Deinitialize(invalid_dereference),
    );
    assert!(matches!(
        projection.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Projection,
            ..
        })
    ));

    let bool_id = SemanticTypeIdV1::from_index(0);
    let place = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], bool_id).unwrap();
    let invalid_bool = SemanticOperandV1::Constant(SemanticConstantV1::new(
        bool_id,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(2, 1).unwrap()),
    ));
    let constant = request_with_statement(
        vec![bool_type(1)],
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(1)),
            layout_identity(1),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            extended_direct_value(bool_id, SemanticAbiExtensionV1::ZeroExtend),
        )
        .unwrap(),
        vec![local(1, bool_id, SemanticLocalRoleV1::Return)],
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place,
            SemanticRvalueV1::new(bool_id, SemanticRvalueKindV1::Use(invalid_bool)),
        )),
    );
    assert!(matches!(
        constant.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Constant,
            ..
        })
    ));
}

#[test]
fn rvalues_retain_and_validate_operation_specific_types() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let bool_id = SemanticTypeIdV1::from_index(1);
    let destination =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], bool_id).unwrap();
    let operand = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], u32_id).unwrap(),
    );
    let invalid_add = request_with_statement(
        vec![u32_type(1), bool_type(2)],
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(1)),
            layout_identity(1),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            extended_direct_value(bool_id, SemanticAbiExtensionV1::ZeroExtend),
        )
        .unwrap(),
        vec![
            local(1, bool_id, SemanticLocalRoleV1::Return),
            local(2, u32_id, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(
                bool_id,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: operand.clone(),
                    right: operand,
                },
            ),
        )),
    );
    assert!(matches!(
        invalid_add.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Binary,
            ..
        })
    ));
}

fn pointer_cast_request(
    input_kind: SemanticPointerKindV1,
    input_mutability: SemanticMutabilityV1,
    input_address_space: u32,
    output_kind: SemanticPointerKindV1,
    output_mutability: SemanticMutabilityV1,
    output_address_space: u32,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let input_id = SemanticTypeIdV1::from_index(1);
    let output_id = SemanticTypeIdV1::from_index(2);
    request_with_statement(
        vec![
            u32_type(1),
            pointer_kind_type(2, u32_id, input_kind, input_mutability, input_address_space),
            pointer_kind_type(
                3,
                u32_id,
                output_kind,
                output_mutability,
                output_address_space,
            ),
        ],
        abi(1, vec![], u32_id),
        vec![
            local(1, u32_id, SemanticLocalRoleV1::Return),
            local(2, input_id, SemanticLocalRoleV1::Temporary),
            local(3, output_id, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], output_id).unwrap(),
            SemanticRvalueV1::new(
                output_id,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], input_id)
                            .unwrap(),
                    ),
                },
            ),
        )),
    )
}

fn borrow_request(
    borrow_kind: SemanticBorrowKindV1,
    pointer_kind: SemanticPointerKindV1,
    pointer_mutability: SemanticMutabilityV1,
    address_of: bool,
) -> InertSemanticMirRequestV1 {
    borrow_request_in_address_space(borrow_kind, pointer_kind, pointer_mutability, address_of, 0)
}

fn borrow_request_in_address_space(
    borrow_kind: SemanticBorrowKindV1,
    pointer_kind: SemanticPointerKindV1,
    pointer_mutability: SemanticMutabilityV1,
    address_of: bool,
    address_space: u32,
) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let pointer_id = SemanticTypeIdV1::from_index(1);
    let place = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], u32_id).unwrap();
    let rvalue = if address_of {
        SemanticRvalueKindV1::AddressOf {
            mutability: pointer_mutability,
            place,
        }
    } else {
        SemanticRvalueKindV1::Borrow {
            kind: borrow_kind,
            place,
        }
    };
    request_with_statement(
        vec![
            u32_type(1),
            pointer_kind_type(2, u32_id, pointer_kind, pointer_mutability, address_space),
        ],
        abi(1, vec![], u32_id),
        vec![
            local(1, u32_id, SemanticLocalRoleV1::Return),
            local(2, u32_id, SemanticLocalRoleV1::Temporary),
            local(3, pointer_id, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], pointer_id).unwrap(),
            SemanticRvalueV1::new(pointer_id, rvalue),
        )),
    )
}

#[test]
fn pointer_operations_cannot_forge_reference_or_address_space_evidence() {
    pointer_cast_request(
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        1,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Immutable,
        1,
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    borrow_request(
        SemanticBorrowKindV1::Shared,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        false,
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    borrow_request(
        SemanticBorrowKindV1::Shared,
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Immutable,
        true,
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    for hostile in [
        pointer_cast_request(
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
        ),
        pointer_cast_request(
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            1,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            0,
        ),
        pointer_cast_request(
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            0,
        ),
        borrow_request(
            SemanticBorrowKindV1::Fake,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            false,
        ),
        borrow_request(
            SemanticBorrowKindV1::Shared,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            false,
        ),
        borrow_request(
            SemanticBorrowKindV1::Shared,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            true,
        ),
        borrow_request_in_address_space(
            SemanticBorrowKindV1::Shared,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            false,
            1,
        ),
        borrow_request_in_address_space(
            SemanticBorrowKindV1::Shared,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            true,
            1,
        ),
    ] {
        assert!(matches!(
            hostile.admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeOperation { .. })
        ));
    }
}

fn tuple_u32_pair_type(identity: u8, u32_id: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![u32_id, u32_id]).unwrap()),
    )
}

fn tuple_aggregate_request(kind: SemanticAggregateKindV1) -> InertSemanticMirRequestV1 {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let tuple_id = SemanticTypeIdV1::from_index(1);
    let u32_place = |local_index| {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local_index), vec![], u32_id)
                .unwrap(),
        )
    };
    request_with_statement(
        vec![u32_type(1), tuple_u32_pair_type(2, u32_id)],
        abi(1, vec![], u32_id),
        vec![
            local(1, u32_id, SemanticLocalRoleV1::Return),
            local(2, tuple_id, SemanticLocalRoleV1::Temporary),
            local(3, u32_id, SemanticLocalRoleV1::Temporary),
            local(4, u32_id, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], tuple_id).unwrap(),
            SemanticRvalueV1::new(
                tuple_id,
                SemanticRvalueKindV1::aggregate(kind, vec![u32_place(2), u32_place(3)]).unwrap(),
            ),
        )),
    )
}

#[test]
fn aggregate_rvalues_retain_their_exact_constructor_kind() {
    let admitted = tuple_aggregate_request(SemanticAggregateKindV1::Tuple)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    let SemanticStatementKindV1::Assign(assignment) =
        admitted.functions()[0].blocks()[0].statements()[0].kind()
    else {
        panic!("expected assignment")
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        panic!("expected aggregate rvalue")
    };
    assert_eq!(aggregate.kind(), &SemanticAggregateKindV1::Tuple);

    assert!(matches!(
        tuple_aggregate_request(SemanticAggregateKindV1::Aggregate)
            .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Aggregate,
            ..
        })
    ));
}

fn f32_type(identity: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(identity),
        layout_identity(identity),
        scalar_layout(
            4,
            4,
            SemanticBackendPrimitiveV1::float(32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    )
}

#[test]
fn atomic_rmw_types_are_operation_specific() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let f32_id = SemanticTypeIdV1::from_index(1);
    let float_place = |local_index| {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local_index), vec![], f32_id).unwrap()
    };
    let model = request_with_statement(
        vec![u32_type(1), f32_type(2)],
        abi(1, vec![], u32_id),
        vec![
            local(1, u32_id, SemanticLocalRoleV1::Return),
            local(2, f32_id, SemanticLocalRoleV1::Temporary),
            local(3, f32_id, SemanticLocalRoleV1::Temporary),
        ],
        SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
            float_place(1),
            float_place(2),
            SemanticOperandV1::Copy(float_place(1)),
            SemanticAtomicRmwOpV1::Add,
            SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Relaxed,
                SemanticAtomicScopeV1::Agent,
            ),
        )),
    );
    assert!(matches!(
        model.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Atomic,
            ..
        })
    ));
}

#[test]
fn ignore_metadata_indirect_and_malformed_abi_are_checked() {
    let unit_id = SemanticTypeIdV1::from_index(1);
    let ignored = request_with_structured_abi(
        vec![u32_type(1), unit_type(2)],
        vec![SemanticAbiValueV1::new(
            unit_id,
            SemanticAbiPassModeV1::Ignore,
        )],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(matches!(
        ignored.functions()[0].abi().arguments()[0].mode(),
        SemanticAbiPassModeV1::Ignore
    ));

    let dynamic = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::new(None, 4).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let metadata_plain = SemanticAbiValueAttributesV1::plain();
    request_with_structured_abi(
        vec![u32_type(1), dynamic.clone()],
        vec![SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticAbiPassModeV1::Indirect {
                attributes: indirect_attributes(0, 4),
                metadata_attributes: Some(metadata_plain),
                on_stack: false,
            },
        )],
        direct_value(SemanticTypeIdV1::from_index(0)),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    let metadata_non_null = attributes(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, false),
        SemanticAbiExtensionV1::None,
        0,
        None,
    );
    assert!(matches!(
        request_with_structured_abi(
            vec![u32_type(1), dynamic.clone()],
            vec![SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticAbiPassModeV1::Indirect {
                    attributes: indirect_attributes(0, 4),
                    metadata_attributes: Some(metadata_non_null),
                    on_stack: false,
                },
            )],
            direct_value(SemanticTypeIdV1::from_index(0)),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let bad_ignore = request_with_structured_abi(
        vec![u32_type(1)],
        vec![SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        )],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        bad_ignore.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let preserved_indirect = request_with_structured_abi(
        vec![u32_type(1)],
        vec![SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Indirect {
                attributes: indirect_attributes(4, 4),
                metadata_attributes: None,
                on_stack: false,
            },
        )],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        preserved_indirect.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let unsized_without_metadata = request_with_structured_abi(
        vec![u32_type(1), dynamic],
        vec![SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticAbiPassModeV1::Indirect {
                attributes: indirect_attributes(0, 4),
                metadata_attributes: None,
                on_stack: false,
            },
        )],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        unsized_without_metadata.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    assert!(matches!(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::default(),
            SemanticAbiExtensionV1::None,
            0,
            Some(3),
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn malformed_aggregate_layouts_are_rejected() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let malformed = [
        SemanticAggregateLayoutV1::new(vec![], vec![SemanticPaddingV1::new(0, 4).unwrap()])
            .unwrap(),
        SemanticAggregateLayoutV1::new(vec![1], vec![]).unwrap(),
        SemanticAggregateLayoutV1::new(vec![0], vec![SemanticPaddingV1::new(0, 4).unwrap()])
            .unwrap(),
    ];
    for layout in malformed {
        let aggregate = SemanticTypeDeclV1::new(
            type_identity(2),
            layout_identity(2),
            SemanticTypeLayoutV1::aggregate(Some(4), 4, layout).unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![u32_id]).unwrap()),
        );
        let model =
            request_with_structured_abi(vec![u32_type(1), aggregate], vec![], direct_value(u32_id));
        assert!(matches!(
            model.admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeLayout)
        ));
    }

    let dynamic = SemanticTypeDeclV1::new(
        type_identity(2),
        layout_identity(2),
        SemanticTypeLayoutV1::new(None, 4).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let falsely_sized = SemanticTypeDeclV1::new(
        type_identity(3),
        layout_identity(3),
        SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![u32_id, SemanticTypeIdV1::from_index(1)]).unwrap(),
        ),
    );
    let model = request_with_structured_abi(
        vec![u32_type(1), dynamic, falsely_sized],
        vec![],
        direct_value(u32_id),
    );
    assert!(matches!(
        model.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

#[test]
fn malformed_enum_layouts_are_rejected() {
    let layouts = vec![
        enum_variant_layout(
            0,
            4,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![SemanticPaddingV1::new(0, 4).unwrap()])
                .unwrap(),
            false,
        ),
        enum_variant_layout(
            1,
            4,
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![SemanticPaddingV1::new(0, 4).unwrap()])
                .unwrap(),
            false,
        ),
    ];
    assert!(matches!(
        SemanticTypeLayoutV1::enum_layout(
            4,
            1,
            SemanticEnumLayoutV1::new(
                layouts,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    4,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 8, 1),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        ),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));

    let duplicate_discriminant = request_with_structured_abi(
        vec![u32_type(1), direct_enum_type(0, 0)],
        vec![],
        direct_value(SemanticTypeIdV1::from_index(0)),
    );
    assert!(matches!(
        duplicate_discriminant.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeLayout)
    ));
}

#[test]
fn fn_abi_argument_cardinality_and_types_are_checked() {
    let bool_id = SemanticTypeIdV1::from_index(0);
    let u32_id = SemanticTypeIdV1::from_index(1);
    let function_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![direct_value(u32_id)],
        direct_value(u32_id),
    )
    .unwrap();
    let wrong_type = request(
        vec![bool_type(1), u32_type(2)],
        vec![],
        vec![function(
            1,
            function_abi.clone(),
            vec![
                local(1, u32_id, SemanticLocalRoleV1::Return),
                local(2, bool_id, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        )],
    );
    assert!(matches!(
        wrong_type.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidLocalRoles { .. })
    ));

    let missing_argument = request(
        vec![bool_type(1), u32_type(2)],
        vec![],
        vec![function(
            1,
            function_abi,
            vec![local(1, u32_id, SemanticLocalRoleV1::Return)],
            vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
        )],
    );
    assert!(matches!(
        missing_argument.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidLocalRoles { .. })
    ));
}
