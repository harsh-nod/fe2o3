use fe2o3_mir_model::semantic_mir_v1::*;

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const FUNCTION_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const DATA_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn layout_identity(tag: u8) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(tag))
}

fn full_range(bits: u16) -> SemanticScalarValidityRangeV1 {
    SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1)
}

fn raw_pointee() -> SemanticAbiPointeeInfoV1 {
    SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()
}

fn model_types() -> Vec<SemanticTypeDeclV1> {
    let u32_backend = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        full_range(32),
    );
    let data_pointer_backend = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        full_range(64),
    );
    let function_pointer_backend = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(1)),
            layout_identity(1),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(u32_backend),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(2)),
            layout_identity(2),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(function_pointer_backend),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::FunctionPointer {
                safety: SemanticFunctionSafetyV1::Safe,
                extern_abi: SemanticExternAbiV1::Rust,
                c_variadic: false,
                arguments: SemanticAggregateTypeV1::new(vec![]).unwrap(),
                return_type: U32,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(3)),
            layout_identity(3),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(data_pointer_backend),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    U32,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_scalar_pointee_info(Some(raw_pointee()), None),
        ),
    ]
}

fn function_abi(tag: u8) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![],
        SemanticAbiValueV1::new(
            U32,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        ),
    )
    .unwrap()
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn function(
    tag: u8,
    role: SemanticFunctionRoleV1,
    temporary_types: &[SemanticTypeIdV1],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(1)),
        U32,
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )];
    locals.extend(temporary_types.iter().enumerate().map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes((index + 2) as u8)),
            *ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        )
    }));
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        function_abi(tag),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn request(
    allocations: Vec<SemanticAllocationDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> InertSemanticMirRequestV1 {
    let type_count = functions
        .iter()
        .flat_map(SemanticFunctionDeclV1::locals)
        .map(|local| local.ty().index() as usize + 1)
        .max()
        .unwrap_or(1);
    let mut types = model_types();
    types.truncate(type_count);
    request_with_types(types, allocations, functions)
}

fn request_with_types(
    types: Vec<SemanticTypeDeclV1>,
    allocations: Vec<SemanticAllocationDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
) -> InertSemanticMirRequestV1 {
    request_with_types_and_roots(
        types,
        allocations,
        functions,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
}

fn request_with_types_and_roots(
    types: Vec<SemanticTypeDeclV1>,
    allocations: Vec<SemanticAllocationDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    roots: Vec<SemanticFunctionIdV1>,
) -> InertSemanticMirRequestV1 {
    request_with_complete_graph(types, allocations, vec![], functions, roots)
}

fn request_with_complete_graph(
    types: Vec<SemanticTypeDeclV1>,
    allocations: Vec<SemanticAllocationDeclV1>,
    statics: Vec<SemanticStaticDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    roots: Vec<SemanticFunctionIdV1>,
) -> InertSemanticMirRequestV1 {
    request_with_vtables(types, allocations, statics, vec![], functions, roots)
}

fn request_with_vtables(
    types: Vec<SemanticTypeDeclV1>,
    allocations: Vec<SemanticAllocationDeclV1>,
    statics: Vec<SemanticStaticDeclV1>,
    vtables: Vec<SemanticVTableDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    roots: Vec<SemanticFunctionIdV1>,
) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        types,
        allocations,
        statics,
        vtables,
        functions,
        roots,
    )
    .unwrap()
}

fn static_pointer_statement(static_id: u32) -> SemanticStatementV1 {
    static_pointer_statement_with_type(static_id, DATA_POINTER)
}

fn static_pointer_statement_with_type(
    static_id: u32,
    pointer_type: SemanticTypeIdV1,
) -> SemanticStatementV1 {
    pointer_statement(
        pointer_type,
        SemanticPointerValueV1::new(
            0,
            SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(static_id)),
        ),
    )
}

fn pointer_statement(
    pointer_type: SemanticTypeIdV1,
    pointer: SemanticPointerValueV1,
) -> SemanticStatementV1 {
    constant_assignment(1, pointer_type, SemanticConstantValueV1::Pointer(pointer))
}

#[test]
fn defined_and_external_statics_close_under_the_same_root_graph() {
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        vec![0; 4],
        vec![0x0f],
        4,
        false,
        vec![],
    )
    .unwrap();
    let statics = vec![
        SemanticStaticDeclV1::new(
            SemanticStaticIdentityV1::from_sha256(bytes(1)),
            SemanticSourceProvenanceV1::unavailable(),
            U32,
            false,
            0,
            SemanticStaticDefinitionV1::Defined {
                initializer: SemanticAllocationIdV1::from_index(0),
            },
        ),
        SemanticStaticDeclV1::new(
            SemanticStaticIdentityV1::from_sha256(bytes(2)),
            SemanticSourceProvenanceV1::unavailable(),
            U32,
            true,
            0,
            SemanticStaticDefinitionV1::ExternalRequired {
                symbol: SemanticLinkSymbolV1::new(b"external_counter".to_vec()).unwrap(),
            },
        ),
    ];
    let function = function(
        1,
        SemanticFunctionRoleV1::KernelRoot,
        &[DATA_POINTER, FUNCTION_POINTER],
        vec![block(
            1,
            vec![
                static_pointer_statement(0),
                static_pointer_statement(1),
                constant_assignment(
                    2,
                    FUNCTION_POINTER,
                    SemanticConstantValueV1::Callable(SemanticCallableIdV1::from_index(0)),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = request_with_complete_graph(
        model_types(),
        vec![allocation],
        statics,
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(admitted.statics().len(), 2);
    assert!(matches!(
        admitted.statics()[0].definition(),
        SemanticStaticDefinitionV1::Defined { .. }
    ));
    assert_eq!(
        match admitted.statics()[1].definition() {
            SemanticStaticDefinitionV1::ExternalRequired { symbol } => symbol.as_bytes(),
            SemanticStaticDefinitionV1::Defined { .. } => panic!("expected external static"),
        },
        b"external_counter"
    );
}

#[test]
fn exported_defined_statics_are_typed_roots() {
    let allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        vec![0; 4],
        vec![0x0f],
        4,
        false,
        vec![],
    )
    .unwrap();
    let exported = SemanticStaticDeclV1::new(
        SemanticStaticIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        U32,
        false,
        0,
        SemanticStaticDefinitionV1::Defined {
            initializer: SemanticAllocationIdV1::from_index(0),
        },
    )
    .with_export_symbol(SemanticLinkSymbolV1::new(b"exported_counter".to_vec()).unwrap());
    let root = function(1, SemanticFunctionRoleV1::KernelRoot, &[], return_block());
    let admitted = request_with_complete_graph(
        model_types().into_iter().take(1).collect(),
        vec![allocation],
        vec![exported],
        vec![root],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(
        admitted.statics()[0].export_symbol().unwrap().as_bytes(),
        b"exported_counter"
    );

    let external_with_export = SemanticStaticDeclV1::new(
        SemanticStaticIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        U32,
        false,
        0,
        SemanticStaticDefinitionV1::ExternalRequired {
            symbol: SemanticLinkSymbolV1::new(b"external_counter".to_vec()).unwrap(),
        },
    )
    .with_export_symbol(SemanticLinkSymbolV1::new(b"forged_export".to_vec()).unwrap());
    assert!(matches!(
        request_with_complete_graph(
            model_types().into_iter().take(1).collect(),
            vec![],
            vec![external_with_export],
            vec![function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[],
                return_block(),
            )],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidStatic)
    ));
}

#[test]
fn static_references_and_definition_contracts_fail_closed() {
    let root = |statement| {
        function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[DATA_POINTER],
            vec![block(1, vec![statement], SemanticTerminatorKindV1::Return)],
        )
    };
    let missing = request_with_complete_graph(
        model_types(),
        vec![],
        vec![],
        vec![root(static_pointer_statement(0))],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    assert!(matches!(
        missing.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Static,
            ..
        })
    ));

    let external_static = || {
        SemanticStaticDeclV1::new(
            SemanticStaticIdentityV1::from_sha256(bytes(1)),
            SemanticSourceProvenanceV1::unavailable(),
            U32,
            false,
            0,
            SemanticStaticDefinitionV1::ExternalRequired {
                symbol: SemanticLinkSymbolV1::new(b"external_counter".to_vec()).unwrap(),
            },
        )
    };
    let pointer_type = |kind, mutability, address_space, metadata| {
        let primitive = SemanticBackendPrimitiveV1::pointer(address_space, 8, 8);
        let valid_range = match kind {
            SemanticPointerKindV1::Raw => full_range(64),
            SemanticPointerKindV1::Reference => {
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into())
            }
        };
        let data_pointer = SemanticBackendScalarV1::initialized(primitive, valid_range);
        let (size_bytes, backend_repr) = match metadata {
            SemanticPointerMetadataV1::None => (8, SemanticBackendReprV1::scalar(data_pointer)),
            SemanticPointerMetadataV1::SliceLength => (
                16,
                SemanticBackendReprV1::scalar_pair(
                    data_pointer,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        full_range(64),
                    ),
                ),
            ),
            SemanticPointerMetadataV1::VTable => (
                16,
                SemanticBackendReprV1::scalar_pair(
                    data_pointer,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                ),
            ),
        };
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(2)),
            layout_identity(2),
            SemanticTypeLayoutV1::new_with_backend_repr(Some(size_bytes), 8, backend_repr, false)
                .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    U32,
                    kind,
                    mutability,
                    address_space,
                    64,
                    metadata,
                )
                .unwrap(),
            ),
        )
    };
    let pointer_request = |pointer: SemanticTypeDeclV1| {
        request_with_complete_graph(
            vec![model_types().remove(0), pointer],
            vec![],
            vec![external_static()],
            vec![function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[SemanticTypeIdV1::from_index(1)],
                vec![block(
                    1,
                    vec![static_pointer_statement_with_type(
                        0,
                        SemanticTypeIdV1::from_index(1),
                    )],
                    SemanticTerminatorKindV1::Return,
                )],
            )],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
    };
    for invalid_pointer in [
        pointer_type(
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            1,
            SemanticPointerMetadataV1::None,
        ),
        pointer_type(
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            0,
            SemanticPointerMetadataV1::None,
        ),
    ] {
        assert!(matches!(
            pointer_request(invalid_pointer).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidStatic)
        ));
    }
    pointer_request(pointer_type(
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Mutable,
        0,
        SemanticPointerMetadataV1::None,
    ))
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    pointer_request(pointer_type(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        SemanticPointerMetadataV1::None,
    ))
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    let slice_pointer = pointer_type(
        SemanticPointerKindV1::Raw,
        SemanticMutabilityV1::Immutable,
        0,
        SemanticPointerMetadataV1::SliceLength,
    );
    let slice_request = |metadata| {
        request_with_complete_graph(
            vec![model_types().remove(0), slice_pointer.clone()],
            vec![],
            vec![external_static()],
            vec![function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[SemanticTypeIdV1::from_index(1)],
                vec![block(
                    1,
                    vec![pointer_statement(
                        SemanticTypeIdV1::from_index(1),
                        SemanticPointerValueV1::new_with_metadata(
                            0,
                            SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(0)),
                            metadata,
                        ),
                    )],
                    SemanticTerminatorKindV1::Return,
                )],
            )],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
    };
    let slice = slice_request(SemanticPointerValueMetadataV1::SliceLength(4))
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(
        match &slice.functions()[0].blocks()[0].statements()[0].kind() {
            SemanticStatementKindV1::Assign(assignment) => match assignment.value().kind() {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) => {
                    match constant.value() {
                        SemanticConstantValueV1::Pointer(pointer) => pointer.metadata(),
                        _ => panic!("expected pointer constant"),
                    }
                }
                _ => panic!("expected constant assignment"),
            },
            _ => panic!("expected assignment"),
        },
        SemanticPointerValueMetadataV1::SliceLength(4)
    );
    assert!(matches!(
        slice_request(SemanticPointerValueMetadataV1::None).admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Constant,
            ..
        })
    ));

    let reference_slice_pointer = pointer_type(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        SemanticPointerMetadataV1::SliceLength,
    );
    let reference_slice_request = |length| {
        request_with_complete_graph(
            vec![model_types().remove(0), reference_slice_pointer.clone()],
            vec![],
            vec![external_static()],
            vec![function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[SemanticTypeIdV1::from_index(1)],
                vec![block(
                    1,
                    vec![pointer_statement(
                        SemanticTypeIdV1::from_index(1),
                        SemanticPointerValueV1::new_with_metadata(
                            0,
                            SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(0)),
                            SemanticPointerValueMetadataV1::SliceLength(length),
                        ),
                    )],
                    SemanticTerminatorKindV1::Return,
                )],
            )],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
    };
    reference_slice_request(1)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    for length in [2, u64::MAX] {
        let error = reference_slice_request(length)
            .admit(SemanticMirLimitsV1::default())
            .unwrap_err();
        assert!(
            matches!(error, SemanticMirErrorV1::InvalidStatic),
            "unexpected rejection for length {length}: {error:?}"
        );
    }

    let dyn_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::new(None, 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let data_pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let vtable_pointer = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(3)),
        layout_identity(3),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(
                data_pointer,
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                ),
            ),
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
            Some(raw_pointee()),
        ),
    );
    let mut vtable_bytes = vec![0; 24];
    vtable_bytes[8..16].copy_from_slice(&4_u64.to_le_bytes());
    vtable_bytes[16..24].copy_from_slice(&4_u64.to_le_bytes());
    let vtable_allocation = SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(1)),
        vtable_bytes,
        vec![u8::MAX; 3],
        8,
        false,
        vec![],
    )
    .unwrap();
    let vtable = SemanticVTableDeclV1::new(
        SemanticVTableIdentityV1::from_sha256(bytes(1)),
        U32,
        SemanticTypeIdV1::from_index(1),
        vec![SemanticDynPredicateIdentityV1::from_sha256(bytes(1))],
        SemanticVTableHeaderV1::new(None, 4, 4).unwrap(),
        vec![],
        SemanticAllocationIdV1::from_index(0),
    )
    .unwrap();
    let vtable_model = request_with_vtables(
        vec![model_types().remove(0), dyn_type, vtable_pointer],
        vec![vtable_allocation],
        vec![external_static()],
        vec![vtable],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[SemanticTypeIdV1::from_index(2)],
            vec![block(
                1,
                vec![pointer_statement(
                    SemanticTypeIdV1::from_index(2),
                    SemanticPointerValueV1::new_with_metadata(
                        0,
                        SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(0)),
                        SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(0)),
                    ),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(vtable_model.statics().len(), 1);
    assert_eq!(vtable_model.vtables().len(), 1);
    assert_eq!(vtable_model.vtables()[0].dyn_predicates().len(), 1);

    let unused = SemanticStaticDeclV1::new(
        SemanticStaticIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        U32,
        false,
        0,
        SemanticStaticDefinitionV1::ExternalRequired {
            symbol: SemanticLinkSymbolV1::new(b"unused".to_vec()).unwrap(),
        },
    );
    let unrooted = request_with_complete_graph(
        model_types().into_iter().take(1).collect(),
        vec![],
        vec![unused],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[],
            return_block(),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    assert!(matches!(
        unrooted.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::StaticOutsideRootClosure { static_id })
            if static_id == SemanticStaticIdV1::from_index(0)
    ));

    let mutable_static = SemanticStaticDeclV1::new(
        SemanticStaticIdentityV1::from_sha256(bytes(1)),
        SemanticSourceProvenanceV1::unavailable(),
        DATA_POINTER,
        true,
        0,
        SemanticStaticDefinitionV1::Defined {
            initializer: SemanticAllocationIdV1::from_index(0),
        },
    );
    let mismatched = request_with_complete_graph(
        model_types(),
        vec![allocation(1, vec![])],
        vec![mutable_static],
        vec![root(static_pointer_statement(0))],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    assert!(matches!(
        mismatched.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidStatic)
    ));
}

#[test]
fn vtable_headers_bind_typed_drop_glue_and_relocation_closure() {
    let u32_type = model_types().remove(0);
    let dyn_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::new(None, 1).unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let nonnull_pointer = || {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )
    };
    let vtable_pointer = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(3)),
        layout_identity(3),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(nonnull_pointer(), nonnull_pointer()),
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
            Some(raw_pointee()),
        ),
    );
    let drop_pointer = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4)),
        layout_identity(4),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                U32,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_scalar_pointee_info(Some(raw_pointee()), None),
    );
    let unit = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(5)),
        layout_identity(5),
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
    );
    let drop_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        4,
        Some(4),
    )
    .unwrap();
    let drop_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![
            SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(3),
                SemanticAbiPassModeV1::Direct(drop_attributes),
            )
            .with_pointee_override(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    4,
                    4,
                )
                .unwrap(),
            ),
        ],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(4),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap();
    let drop_glue = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2)),
        SemanticFunctionRoleV1::DropGlue(U32),
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticSourceProvenanceV1::unavailable(),
        drop_abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(1)),
                SemanticTypeIdV1::from_index(4),
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(2)),
                SemanticTypeIdV1::from_index(3),
                SemanticLocalRoleV1::Argument(0),
                SemanticSourceProvenanceV1::unavailable(),
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        return_block(),
    )
    .unwrap();
    let build_request = |drop_target: SemanticFunctionIdV1,
                         method_relocation: SemanticFunctionIdV1,
                         method_slot: SemanticFunctionIdV1| {
        let mut header = vec![0; 32];
        header[8..16].copy_from_slice(&4_u64.to_le_bytes());
        header[16..24].copy_from_slice(&4_u64.to_le_bytes());
        let allocation = SemanticAllocationDeclV1::new(
            SemanticAllocationIdentityV1::from_sha256(bytes(1)),
            header,
            vec![u8::MAX; 4],
            8,
            false,
            vec![
                SemanticRelocationV1::new(
                    0,
                    8,
                    0,
                    SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(
                        drop_target.index(),
                    )),
                )
                .unwrap(),
                SemanticRelocationV1::new(
                    24,
                    8,
                    0,
                    SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(
                        method_relocation.index(),
                    )),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let vtable_relocation = SemanticAllocationDeclV1::new(
            SemanticAllocationIdentityV1::from_sha256(bytes(2)),
            vec![0; 8],
            vec![u8::MAX],
            8,
            false,
            vec![
                SemanticRelocationV1::new(
                    0,
                    8,
                    0,
                    SemanticRelocationTargetV1::VTable(SemanticVTableIdV1::from_index(0)),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let vtable = SemanticVTableDeclV1::new(
            SemanticVTableIdentityV1::from_sha256(bytes(1)),
            U32,
            SemanticTypeIdV1::from_index(1),
            vec![SemanticDynPredicateIdentityV1::from_sha256(bytes(1))],
            SemanticVTableHeaderV1::new(Some(drop_target), 4, 4).unwrap(),
            vec![method_slot],
            SemanticAllocationIdV1::from_index(0),
        )
        .unwrap();
        let root = function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[
                SemanticTypeIdV1::from_index(2),
                SemanticTypeIdV1::from_index(3),
            ],
            vec![block(
                1,
                vec![
                    pointer_statement(
                        SemanticTypeIdV1::from_index(2),
                        SemanticPointerValueV1::new_with_metadata(
                            0,
                            SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(0)),
                            SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(
                                0,
                            )),
                        ),
                    ),
                    constant_assignment(
                        2,
                        SemanticTypeIdV1::from_index(3),
                        SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
                            0,
                            SemanticPointerProvenanceV1::Allocation(
                                SemanticAllocationIdV1::from_index(1),
                            ),
                        )),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        );
        request_with_vtables(
            vec![
                u32_type.clone(),
                dyn_type.clone(),
                vtable_pointer.clone(),
                drop_pointer.clone(),
                unit.clone(),
            ],
            vec![allocation, vtable_relocation],
            vec![SemanticStaticDeclV1::new(
                SemanticStaticIdentityV1::from_sha256(bytes(1)),
                SemanticSourceProvenanceV1::unavailable(),
                U32,
                false,
                0,
                SemanticStaticDefinitionV1::ExternalRequired {
                    symbol: SemanticLinkSymbolV1::new(b"drop_target".to_vec()).unwrap(),
                },
            )],
            vec![vtable],
            vec![
                root,
                drop_glue.clone(),
                function(
                    3,
                    SemanticFunctionRoleV1::InternalHelper,
                    &[],
                    return_block(),
                ),
            ],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
    };

    let admitted = build_request(
        SemanticFunctionIdV1::from_index(1),
        SemanticFunctionIdV1::from_index(2),
        SemanticFunctionIdV1::from_index(2),
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(
        admitted.vtables()[0].header().drop_glue(),
        Some(SemanticFunctionIdV1::from_index(1))
    );
    assert_eq!(
        admitted.vtables()[0].slots(),
        &[SemanticVTableSlotV1::Method(
            SemanticFunctionIdV1::from_index(2)
        )]
    );
    assert!(matches!(
        admitted.allocations()[0].relocations()[0].target(),
        SemanticRelocationTargetV1::Callable(function)
            if function == SemanticCallableIdV1::from_index(1)
    ));
    assert!(matches!(
        admitted.allocations()[1].relocations()[0].target(),
        SemanticRelocationTargetV1::VTable(vtable)
            if vtable == SemanticVTableIdV1::from_index(0)
    ));

    assert!(matches!(
        build_request(
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(2),
            SemanticFunctionIdV1::from_index(2),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidAllocation)
    ));
    assert!(matches!(
        build_request(
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(2),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidAllocation)
    ));
    assert!(matches!(
        build_request(
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(1),
        )
        .admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidAllocation)
    ));
}

fn return_block() -> Vec<SemanticBasicBlockV1> {
    vec![block(1, vec![], SemanticTerminatorKindV1::Return)]
}

fn direct_call_blocks(callee: u32) -> Vec<SemanticBasicBlockV1> {
    let destination = SemanticCallDestinationV1::new(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], U32).unwrap(),
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn,
            SemanticBlockIdV1::from_index(1),
        ),
    );
    vec![
        block(
            1,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new(
                    SemanticFunctionIdV1::from_index(callee),
                    vec![],
                    Some(destination),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        block(2, vec![], SemanticTerminatorKindV1::Return),
    ]
}

fn tail_call_block(callee: u32) -> Vec<SemanticBasicBlockV1> {
    vec![block(
        1,
        vec![],
        SemanticTerminatorKindV1::TailCall(
            SemanticDirectTailCallV1::new(
                SemanticFunctionIdV1::from_index(callee),
                vec![],
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    )]
}

fn constant_assignment(
    local: u32,
    ty: SemanticTypeIdV1,
    value: SemanticConstantValueV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap(),
            SemanticRvalueV1::new(
                ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty, value,
                ))),
            ),
        )),
    )
}

fn allocation(tag: u8, relocations: Vec<SemanticRelocationV1>) -> SemanticAllocationDeclV1 {
    SemanticAllocationDeclV1::new(
        SemanticAllocationIdentityV1::from_sha256(bytes(tag)),
        vec![0; 8],
        vec![0xff],
        8,
        false,
        relocations,
    )
    .unwrap()
}

#[test]
fn roots_and_kernel_roles_are_exactly_the_same_set() {
    let wrong_root = request(
        vec![],
        vec![function(
            1,
            SemanticFunctionRoleV1::InternalHelper,
            &[],
            return_block(),
        )],
    );
    assert!(matches!(
        wrong_root.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionRole {
            function,
            role: SemanticFunctionRoleV1::InternalHelper,
            rooted: true,
        }) if function == SemanticFunctionIdV1::from_index(0)
    ));

    let extra_kernel = request(
        vec![],
        vec![
            function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[],
                direct_call_blocks(1),
            ),
            function(2, SemanticFunctionRoleV1::KernelRoot, &[], return_block()),
        ],
    );
    assert!(matches!(
        extra_kernel.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidFunctionRole {
            function,
            role: SemanticFunctionRoleV1::KernelRoot,
            rooted: false,
        }) if function == SemanticFunctionIdV1::from_index(1)
    ));
}

#[test]
fn unrooted_function_records_are_rejected() {
    let model = request(
        vec![],
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot, &[], return_block()),
            function(
                2,
                SemanticFunctionRoleV1::InternalHelper,
                &[],
                return_block(),
            ),
        ],
    );
    assert!(matches!(
        model.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::FunctionOutsideRootClosure { function })
            if function == SemanticFunctionIdV1::from_index(1)
    ));
}

#[test]
fn unrooted_type_and_allocation_records_are_rejected() {
    let extra_type = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(250)),
        model_types(),
        vec![],
        vec![],
        vec![],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[],
            return_block(),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap();
    assert!(matches!(
        extra_type.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::TypeOutsideRootClosure { ty })
            if ty == SemanticTypeIdV1::from_index(1)
    ));

    let extra_allocation = request(
        vec![allocation(1, vec![])],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[],
            return_block(),
        )],
    );
    assert!(matches!(
        extra_allocation.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::AllocationOutsideRootClosure { allocation })
            if allocation == SemanticAllocationIdV1::from_index(0)
    ));
}

#[test]
fn slice_elements_are_part_of_the_retained_type_closure() {
    let mut types = model_types();
    types[FUNCTION_POINTER.index() as usize] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        layout_identity(2),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            8,
            SemanticFieldsShapeV1::array(8, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice {
            element: DATA_POINTER,
        },
    );
    let admitted = request_with_types(
        types,
        vec![],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[FUNCTION_POINTER],
            return_block(),
        )],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();

    assert!(matches!(
        admitted.types()[FUNCTION_POINTER.index() as usize].shape(),
        SemanticTypeShapeV1::Slice { element } if *element == DATA_POINTER
    ));
}

#[test]
fn constant_operand_types_are_part_of_the_retained_type_closure() {
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], FUNCTION_POINTER)
                .unwrap(),
            SemanticRvalueV1::new(
                FUNCTION_POINTER,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    FUNCTION_POINTER,
                    SemanticConstantValueV1::Callable(SemanticCallableIdV1::from_index(1)),
                ))),
            ),
        )),
    );
    request_with_types(
        model_types().into_iter().take(2).collect(),
        vec![],
        vec![
            function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[FUNCTION_POINTER],
                vec![block(1, vec![statement], SemanticTerminatorKindV1::Return)],
            ),
            function(
                2,
                SemanticFunctionRoleV1::InternalHelper,
                &[],
                return_block(),
            ),
        ],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
}

fn reference_u32_type(tag: u8, mutability: SemanticMutabilityV1) -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                pointer,
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                U32,
                SemanticPointerKindV1::Reference,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn promoted_reference_request(
    offset: u64,
    mutability: SemanticMutabilityV1,
) -> InertSemanticMirRequestV1 {
    let reference_type = SemanticTypeIdV1::from_index(1);
    let mut types = model_types().into_iter().take(1).collect::<Vec<_>>();
    types.push(reference_u32_type(2, mutability));
    request_with_types(
        types,
        vec![allocation(1, vec![])],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[reference_type],
            vec![block(
                1,
                vec![constant_assignment(
                    1,
                    reference_type,
                    SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
                        offset,
                        SemanticPointerProvenanceV1::Allocation(
                            SemanticAllocationIdV1::from_index(0),
                        ),
                    )),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    )
}

#[test]
fn promoted_reference_constants_are_bounded_aligned_and_mutability_checked() {
    promoted_reference_request(4, SemanticMutabilityV1::Immutable)
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
    for invalid in [
        promoted_reference_request(2, SemanticMutabilityV1::Immutable),
        promoted_reference_request(8, SemanticMutabilityV1::Immutable),
        promoted_reference_request(4, SemanticMutabilityV1::Mutable),
    ] {
        assert!(matches!(
            invalid.admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidAllocation)
        ));
    }
}

#[test]
fn unreachable_constants_retain_allocation_relocation_closure() {
    let allocations = vec![
        allocation(
            1,
            vec![
                SemanticRelocationV1::new(
                    0,
                    8,
                    0,
                    SemanticRelocationTargetV1::Allocation(SemanticAllocationIdV1::from_index(1)),
                )
                .unwrap(),
            ],
        ),
        allocation(
            2,
            vec![
                SemanticRelocationV1::new(
                    0,
                    8,
                    0,
                    SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(1)),
                )
                .unwrap(),
            ],
        ),
    ];
    let dead_statement = constant_assignment(
        2,
        DATA_POINTER,
        SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
            0,
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
        )),
    );
    request(
        allocations,
        vec![
            function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[FUNCTION_POINTER, DATA_POINTER],
                vec![
                    block(1, vec![], SemanticTerminatorKindV1::Return),
                    block(
                        2,
                        vec![dead_statement],
                        SemanticTerminatorKindV1::Unreachable,
                    ),
                ],
            ),
            function(
                2,
                SemanticFunctionRoleV1::InternalHelper,
                &[],
                return_block(),
            ),
        ],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
}

#[test]
fn direct_and_tail_calls_close_over_helpers() {
    let functions = vec![
        function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[],
            direct_call_blocks(1),
        ),
        function(
            2,
            SemanticFunctionRoleV1::InternalHelper,
            &[],
            tail_call_block(2),
        ),
        function(
            3,
            SemanticFunctionRoleV1::DeviceFfiExport,
            &[],
            return_block(),
        ),
    ];
    let admitted = request_with_types_and_roots(
        model_types().into_iter().take(1).collect(),
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(2),
        ],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(
        admitted.functions()[2].role(),
        SemanticFunctionRoleV1::DeviceFfiExport
    );
}

#[test]
fn executable_constants_and_relocation_chains_close_over_helpers() {
    let statements = vec![
        constant_assignment(
            1,
            FUNCTION_POINTER,
            SemanticConstantValueV1::Callable(SemanticCallableIdV1::from_index(1)),
        ),
        constant_assignment(
            2,
            DATA_POINTER,
            SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
                0,
                SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
            )),
        ),
    ];
    let allocations = vec![
        SemanticAllocationDeclV1::new(
            SemanticAllocationIdentityV1::from_sha256(bytes(1)),
            vec![0; 8],
            vec![0xff],
            8,
            false,
            vec![
                SemanticRelocationV1::new(
                    0,
                    8,
                    0,
                    SemanticRelocationTargetV1::Allocation(SemanticAllocationIdV1::from_index(1)),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        SemanticAllocationDeclV1::new(
            SemanticAllocationIdentityV1::from_sha256(bytes(2)),
            vec![0; 8],
            vec![0xff],
            8,
            false,
            vec![
                SemanticRelocationV1::new(
                    0,
                    8,
                    0,
                    SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(2)),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    ];
    request(
        allocations,
        vec![
            function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[FUNCTION_POINTER, DATA_POINTER],
                vec![block(1, statements, SemanticTerminatorKindV1::Return)],
            ),
            function(
                2,
                SemanticFunctionRoleV1::InternalHelper,
                &[],
                return_block(),
            ),
            function(
                3,
                SemanticFunctionRoleV1::InternalHelper,
                &[],
                return_block(),
            ),
        ],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
}

#[test]
fn function_constants_require_exact_source_abi_and_variadic_shape() {
    let function_pointer_type = |extern_abi, c_variadic| {
        let pointer = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(2)),
            layout_identity(2),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    pointer,
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::FunctionPointer {
                safety: SemanticFunctionSafetyV1::Safe,
                extern_abi,
                c_variadic,
                arguments: SemanticAggregateTypeV1::new(vec![]).unwrap(),
                return_type: U32,
            },
        )
    };
    let statement = constant_assignment(
        1,
        FUNCTION_POINTER,
        SemanticConstantValueV1::Callable(SemanticCallableIdV1::from_index(1)),
    );
    let functions = || {
        vec![
            function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[FUNCTION_POINTER],
                vec![block(
                    1,
                    vec![statement.clone()],
                    SemanticTerminatorKindV1::Return,
                )],
            ),
            function(
                2,
                SemanticFunctionRoleV1::InternalHelper,
                &[],
                return_block(),
            ),
        ]
    };

    for (extern_abi, c_variadic) in [
        (SemanticExternAbiV1::C { unwind: false }, false),
        (SemanticExternAbiV1::C { unwind: false }, true),
    ] {
        let mut types = model_types();
        types[1] = function_pointer_type(extern_abi, c_variadic);
        assert!(matches!(
            request_with_types(types, vec![], functions()).admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidTypeOperation {
                operation: SemanticTypeOperationV1::Constant,
                ..
            })
        ));
    }
}

#[test]
fn unreachable_blocks_and_their_references_are_retained_in_one_closed_graph() {
    let retained = request(
        vec![],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[],
            vec![
                block(1, vec![], SemanticTerminatorKindV1::Return),
                block(2, vec![], SemanticTerminatorKindV1::Unreachable),
            ],
        )],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(retained.functions()[0].blocks().len(), 2);

    let dead_call = request(
        vec![],
        vec![
            function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[],
                vec![
                    block(1, vec![], SemanticTerminatorKindV1::Return),
                    block(
                        2,
                        vec![],
                        SemanticTerminatorKindV1::TailCall(
                            SemanticDirectTailCallV1::new(
                                SemanticFunctionIdV1::from_index(1),
                                vec![],
                                SemanticUnwindActionV1::Unreachable,
                            )
                            .unwrap(),
                        ),
                    ),
                ],
            ),
            function(
                2,
                SemanticFunctionRoleV1::InternalHelper,
                &[],
                return_block(),
            ),
        ],
    );
    let admitted = dead_call.admit(SemanticMirLimitsV1::default()).unwrap();
    assert_eq!(admitted.functions().len(), 2);
}

#[test]
fn function_roles_are_canonical_and_missing_dead_references_still_fail() {
    let helper = |role| {
        let functions = vec![
            function(
                1,
                SemanticFunctionRoleV1::KernelRoot,
                &[],
                direct_call_blocks(1),
            ),
            function(2, role, &[], return_block()),
        ];
        let roots = if role == SemanticFunctionRoleV1::DeviceFfiExport {
            vec![
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(1),
            ]
        } else {
            vec![SemanticFunctionIdV1::from_index(0)]
        };
        request_with_types_and_roots(
            model_types().into_iter().take(1).collect(),
            vec![],
            functions,
            roots,
        )
        .admit(SemanticMirLimitsV1::default())
        .unwrap()
    };
    assert_ne!(
        helper(SemanticFunctionRoleV1::InternalHelper).semantic_sha256(),
        helper(SemanticFunctionRoleV1::DeviceFfiExport).semantic_sha256()
    );

    let missing = request(
        vec![],
        vec![function(
            1,
            SemanticFunctionRoleV1::KernelRoot,
            &[],
            vec![
                block(1, vec![], SemanticTerminatorKindV1::Return),
                block(
                    2,
                    vec![],
                    SemanticTerminatorKindV1::TailCall(
                        SemanticDirectTailCallV1::new(
                            SemanticFunctionIdV1::from_index(99),
                            vec![],
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
            ],
        )],
    );
    assert!(matches!(
        missing.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidReference {
            reference: SemanticMirReferenceV1::Callable,
            index: 99,
            ..
        })
    ));
}
