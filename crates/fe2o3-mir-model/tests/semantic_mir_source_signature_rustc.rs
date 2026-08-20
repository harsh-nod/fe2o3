use fe2o3_mir_model::semantic_mir_v1::*;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn layout_identity(tag: u8) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(bytes(tag))
}

fn integer_type(tag: u8, bits: u16, size_bytes: u64) -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, bits, size_bytes);
    let valid_end = (1_u128 << bits) - 1;
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size_bytes),
            size_bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, valid_end),
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

fn tuple_type(tag: u8, fields: Vec<SemanticTypeIdV1>) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn unit_type(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    )
}

fn empty_tuple_type(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        layout_identity(tag),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}

fn fixture_types() -> Vec<SemanticTypeDeclV1> {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let u64_id = SemanticTypeIdV1::from_index(1);
    vec![
        integer_type(1, 32, 4),
        integer_type(2, 64, 8),
        tuple_type(3, vec![u32_id, u64_id]),
        unit_type(4),
        empty_tuple_type(5),
    ]
}

fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Direct(attributes))
}

fn ignored(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore)
}

#[allow(clippy::too_many_arguments)]
fn source_signature_abi(
    extern_abi: SemanticExternAbiV1,
    canon_abi: SemanticCanonAbiV1,
    can_unwind: bool,
    c_variadic: bool,
    fixed_count: u32,
    source_inputs: Vec<SemanticTypeIdV1>,
    source_output: SemanticTypeIdV1,
    arguments: Vec<SemanticAbiArgumentV1>,
    return_value: SemanticAbiValueV1,
) -> Result<SemanticFunctionAbiV1, SemanticMirErrorV1> {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256(bytes(200)),
        layout_identity(240),
        canon_abi,
        extern_abi,
        can_unwind,
        c_variadic,
        fixed_count,
        source_inputs,
        source_output,
        arguments,
        return_value,
    )
}

fn request(abi: SemanticFunctionAbiV1) -> InertSemanticMirRequestV1 {
    let types = fixture_types();
    let mut locals = vec![SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(1)),
        abi.source_output_type(),
        SemanticLocalRoleV1::Return,
        SemanticSourceProvenanceV1::unavailable(),
    )];
    for (index, ty) in abi.source_input_types().iter().copied().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(u8::try_from(index + 2).unwrap())),
            ty,
            SemanticLocalRoleV1::Argument(u32::try_from(index).unwrap()),
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    for (index, _) in types.iter().enumerate() {
        let identity = u8::try_from(locals.len() + 1).unwrap();
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(identity)),
            SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()),
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }

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
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout_identity(240)),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn admit(abi: SemanticFunctionAbiV1) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    request(abi).admit(SemanticMirLimitsV1::default())
}

fn rust_call_abi(
    tuple_arguments: Vec<SemanticAbiArgumentV1>,
) -> Result<SemanticFunctionAbiV1, SemanticMirErrorV1> {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let tuple_id = SemanticTypeIdV1::from_index(2);
    let unit_id = SemanticTypeIdV1::from_index(3);
    let mut arguments = vec![SemanticAbiArgumentV1::source(direct(u32_id))];
    arguments.extend(tuple_arguments);
    source_signature_abi(
        SemanticExternAbiV1::RustCall,
        SemanticCanonAbiV1::Rust,
        false,
        false,
        1,
        vec![u32_id, tuple_id],
        unit_id,
        arguments,
        ignored(unit_id),
    )
}

#[test]
fn nonempty_rust_call_tuple_expansion_admits() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let u64_id = SemanticTypeIdV1::from_index(1);
    let abi = rust_call_abi(vec![
        SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(u32_id)),
        SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(u64_id)),
    ])
    .unwrap();

    assert_eq!(abi.fixed_count(), 1);
    assert_eq!(abi.source_input_types().len(), 2);
    assert_eq!(abi.adjusted_arguments().len(), 3);
    let admitted = admit(abi).unwrap();
    assert_eq!(admitted.functions()[0].abi().adjusted_arguments().len(), 3);
}

#[test]
fn malformed_rust_call_tuple_index_type_and_count_are_rejected() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let u64_id = SemanticTypeIdV1::from_index(1);

    assert!(matches!(
        rust_call_abi(vec![
            SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(u32_id)),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(u64_id)),
        ]),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let wrong_type = rust_call_abi(vec![
        SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(u32_id)),
        SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(u32_id)),
    ])
    .unwrap();
    assert!(matches!(
        admit(wrong_type),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let wrong_count = rust_call_abi(vec![SemanticAbiArgumentV1::rust_call_tuple_field(
        0,
        direct(u32_id),
    )])
    .unwrap();
    assert!(matches!(
        admit(wrong_count),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn rust_call_empty_tuple_admits_and_hostile_source_shapes_fail_closed() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let u64_id = SemanticTypeIdV1::from_index(1);
    let tuple_id = SemanticTypeIdV1::from_index(2);
    let unit_id = SemanticTypeIdV1::from_index(3);
    let empty_tuple_id = SemanticTypeIdV1::from_index(4);

    let empty = source_signature_abi(
        SemanticExternAbiV1::RustCall,
        SemanticCanonAbiV1::Rust,
        false,
        false,
        1,
        vec![u32_id, empty_tuple_id],
        unit_id,
        vec![SemanticAbiArgumentV1::source(direct(u32_id))],
        ignored(unit_id),
    )
    .unwrap();
    admit(empty).unwrap();

    let non_tuple_tail = source_signature_abi(
        SemanticExternAbiV1::RustCall,
        SemanticCanonAbiV1::Rust,
        false,
        false,
        1,
        vec![u32_id, u64_id],
        unit_id,
        vec![
            SemanticAbiArgumentV1::source(direct(u32_id)),
            SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(u64_id)),
        ],
        ignored(unit_id),
    )
    .unwrap();
    assert!(matches!(
        admit(non_tuple_tail),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    assert!(matches!(
        source_signature_abi(
            SemanticExternAbiV1::RustCall,
            SemanticCanonAbiV1::Rust,
            false,
            false,
            0,
            vec![u32_id, tuple_id],
            unit_id,
            vec![
                SemanticAbiArgumentV1::source(direct(u32_id)),
                SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(u32_id)),
                SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(u64_id)),
            ],
            ignored(unit_id),
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    assert!(matches!(
        source_signature_abi(
            SemanticExternAbiV1::RustCall,
            SemanticCanonAbiV1::Rust,
            false,
            false,
            1,
            vec![u32_id, tuple_id],
            unit_id,
            vec![
                SemanticAbiArgumentV1::source(direct(u32_id)),
                SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(u32_id)),
                SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(u64_id)),
                SemanticAbiArgumentV1::hidden(
                    SemanticAbiHiddenArgumentRoleV1::CallerLocation,
                    direct(u32_id),
                ),
            ],
            ignored(unit_id),
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn cdecl_canonicalizes_to_c_and_is_a_legal_variadic_abi() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let abi = source_signature_abi(
        SemanticExternAbiV1::Cdecl { unwind: false },
        SemanticCanonAbiV1::C,
        false,
        true,
        1,
        vec![u32_id],
        u32_id,
        vec![SemanticAbiArgumentV1::source(direct(u32_id))],
        direct(u32_id),
    )
    .unwrap();
    assert_eq!(abi.canon_abi(), SemanticCanonAbiV1::C);
    assert_eq!(
        abi.extern_abi(),
        SemanticExternAbiV1::Cdecl { unwind: false }
    );
    assert!(abi.c_variadic());
    admit(abi).unwrap();

    assert!(matches!(
        source_signature_abi(
            SemanticExternAbiV1::Cdecl { unwind: false },
            SemanticCanonAbiV1::Rust,
            false,
            true,
            1,
            vec![u32_id],
            u32_id,
            vec![SemanticAbiArgumentV1::source(direct(u32_id))],
            direct(u32_id),
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    assert!(matches!(
        source_signature_abi(
            SemanticExternAbiV1::Rust,
            SemanticCanonAbiV1::Rust,
            false,
            true,
            0,
            vec![],
            u32_id,
            vec![],
            direct(u32_id),
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}

#[test]
fn source_unwind_permission_and_effective_unwind_are_distinct() {
    let unit_id = SemanticTypeIdV1::from_index(3);
    for extern_abi in [
        SemanticExternAbiV1::C { unwind: true },
        SemanticExternAbiV1::Cdecl { unwind: true },
    ] {
        let abi = source_signature_abi(
            extern_abi,
            SemanticCanonAbiV1::C,
            false,
            false,
            0,
            vec![],
            unit_id,
            vec![],
            ignored(unit_id),
        )
        .unwrap();
        admit(abi).unwrap();
    }

    for extern_abi in [
        SemanticExternAbiV1::C { unwind: false },
        SemanticExternAbiV1::Cdecl { unwind: false },
    ] {
        let abi = source_signature_abi(
            extern_abi,
            SemanticCanonAbiV1::C,
            true,
            false,
            0,
            vec![],
            unit_id,
            vec![],
            ignored(unit_id),
        )
        .unwrap();
        assert!(matches!(
            admit(abi),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }

    for (extern_abi, canon_abi) in [
        (SemanticExternAbiV1::Rust, SemanticCanonAbiV1::Rust),
        (
            SemanticExternAbiV1::C { unwind: true },
            SemanticCanonAbiV1::C,
        ),
    ] {
        let abi = source_signature_abi(
            extern_abi,
            canon_abi,
            true,
            false,
            0,
            vec![],
            unit_id,
            vec![],
            ignored(unit_id),
        )
        .unwrap();
        assert!(matches!(
            admit(abi),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn gpu_abi_has_no_offload_dyn_ptr_and_special_signatures_fail_closed() {
    let u32_id = SemanticTypeIdV1::from_index(0);
    let unit_id = SemanticTypeIdV1::from_index(3);
    let gpu = source_signature_abi(
        SemanticExternAbiV1::GpuKernel,
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        unit_id,
        vec![],
        ignored(unit_id),
    )
    .unwrap();

    // The semantic ABI has only explicit source/adjusted arguments; GPU offload
    // state is not represented by a synthetic dyn_ptr argument or side channel.
    assert!(gpu.source_input_types().is_empty());
    assert!(gpu.arguments().is_empty());
    assert!(gpu.hidden_arguments().is_empty());
    assert!(!format!("{gpu:?}").contains("dyn_ptr"));
    admit(gpu).unwrap();

    let non_unit_gpu = source_signature_abi(
        SemanticExternAbiV1::GpuKernel,
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        u32_id,
        vec![],
        direct(u32_id),
    )
    .unwrap();
    assert!(matches!(
        admit(non_unit_gpu),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));

    let nonempty_custom = source_signature_abi(
        SemanticExternAbiV1::Custom,
        SemanticCanonAbiV1::Custom,
        false,
        false,
        1,
        vec![u32_id],
        unit_id,
        vec![SemanticAbiArgumentV1::source(direct(u32_id))],
        ignored(unit_id),
    )
    .unwrap();
    assert!(matches!(
        admit(nonempty_custom),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}
