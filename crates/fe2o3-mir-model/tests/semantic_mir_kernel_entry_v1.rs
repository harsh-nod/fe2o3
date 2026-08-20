use fe2o3_mir_model::semantic_mir_v1::*;

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn u32_type() -> SemanticTypeDeclV1 {
    let primitive = SemanticBackendPrimitiveV1::integer(false, 32, 4);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(1)),
        SemanticLayoutIdentityV1::from_sha256(bytes(1)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
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

fn unit_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(2)),
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

fn empty_tuple_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(3)),
        SemanticLayoutIdentityV1::from_sha256(bytes(3)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}

fn function(tag: u8, role: SemanticFunctionRoleV1) -> SemanticFunctionDeclV1 {
    let ty = if role == SemanticFunctionRoleV1::KernelRoot {
        SemanticTypeIdV1::from_index(1)
    } else {
        SemanticTypeIdV1::from_index(0)
    };
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let (canon_abi, extern_abi, return_mode) = match role {
        SemanticFunctionRoleV1::KernelRoot => (
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            SemanticAbiPassModeV1::Ignore,
        ),
        SemanticFunctionRoleV1::DeviceFfiExport => (
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::C { unwind: false },
            SemanticAbiPassModeV1::Direct(attributes),
        ),
        SemanticFunctionRoleV1::InternalHelper | SemanticFunctionRoleV1::DropGlue(_) => (
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            SemanticAbiPassModeV1::Direct(attributes),
        ),
    };
    function_with_abi(tag, role, ty, canon_abi, extern_abi, false, return_mode)
}

fn function_with_abi(
    tag: u8,
    role: SemanticFunctionRoleV1,
    return_type: SemanticTypeIdV1,
    canon_abi: SemanticCanonAbiV1,
    extern_abi: SemanticExternAbiV1,
    c_variadic: bool,
    return_mode: SemanticAbiPassModeV1,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        canon_abi,
        extern_abi,
        false,
        c_variadic,
        0,
        vec![],
        SemanticAbiValueV1::new(return_type, return_mode),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(1)),
                return_type,
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(2)),
                SemanticTypeIdV1::from_index(0),
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::unavailable(),
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(bytes(1)),
                SemanticSourceProvenanceV1::unavailable(),
                vec![],
                SemanticTerminatorV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticTerminatorKindV1::Return,
                ),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn request(
    functions: Vec<SemanticFunctionDeclV1>,
    roots: Vec<SemanticFunctionIdV1>,
) -> InertSemanticMirRequestV1 {
    request_with_components(vec![u32_type(), unit_type()], vec![], functions, roots)
}

fn request_with_components(
    types: Vec<SemanticTypeDeclV1>,
    statics: Vec<SemanticStaticDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    roots: Vec<SemanticFunctionIdV1>,
) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        vec![],
        statics,
        vec![],
        functions,
        roots,
    )
    .unwrap()
}

fn launch() -> SemanticKernelLaunchBoundsV1 {
    let dimensions = SemanticWorkgroupDimensionsV1::new([256, 1, 1]).unwrap();
    SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), Some(2)).unwrap()
}

fn source_contract() -> SemanticKernelSourceContractV1 {
    SemanticKernelSourceContractV1::new(Some(launch()), None, None).unwrap()
}

fn unsafe_source_contract() -> SemanticKernelSourceContractV1 {
    let assembly = SemanticUnsafeAssemblyDeclarationV1::new(
        SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
        0x0001,
        SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
        0,
    )
    .unwrap();
    let reachable = SemanticReachableAssemblyV1::new(
        2,
        0x0001,
        SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
        0,
    )
    .unwrap();
    SemanticKernelSourceContractV1::new(Some(launch()), Some(assembly), Some(reachable)).unwrap()
}

fn entry(symbol: &[u8], binding_tag: u8) -> SemanticKernelEntryV1 {
    SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(symbol.to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(binding_tag)),
        source_contract(),
    )
}

fn kernel_digest(source_contract: SemanticKernelSourceContractV1) -> InertSemanticMirSha256V1 {
    let entry = SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"kernel".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(7)),
        source_contract,
    );
    request(
        vec![function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry)],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
    .semantic_sha256()
}

#[test]
fn production_readiness_requires_structured_kernel_entry_metadata() {
    let inert = request(
        vec![function(1, SemanticFunctionRoleV1::KernelRoot)],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(matches!(
        inert.require_complete_kernel_entries(),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));

    let complete = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"kernel", 7)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    complete.require_complete_kernel_entries().unwrap();
    let retained = complete.functions()[0].kernel_entry().unwrap();
    assert_eq!(retained.export_symbol().as_bytes(), b"kernel");
    assert_eq!(
        retained.kernel_binding_identity(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(7))
    );
    assert_eq!(
        retained.source_contract().launch().unwrap().required(),
        Some(SemanticWorkgroupDimensionsV1::new([256, 1, 1]).unwrap())
    );

    let missing_device_export = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"kernel", 7)),
            function(2, SemanticFunctionRoleV1::DeviceFfiExport),
        ],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(matches!(
        missing_device_export.require_complete_external_entries(),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    let complete_device_export = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"kernel", 7)),
            function(2, SemanticFunctionRoleV1::DeviceFfiExport).with_device_ffi_export_symbol(
                SemanticLinkSymbolV1::new(b"device_add".to_vec()).unwrap(),
            ),
        ],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    complete_device_export
        .require_complete_external_entries()
        .unwrap();
}

#[test]
fn kernel_entry_fields_are_canonical_and_role_bound() {
    let admitted = |symbol: &[u8], binding_tag| {
        request(
            vec![
                function(1, SemanticFunctionRoleV1::KernelRoot)
                    .with_kernel_entry(entry(symbol, binding_tag)),
            ],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .admit(SemanticMirLimitsV1::default())
        .unwrap()
        .semantic_sha256()
    };
    assert_ne!(admitted(b"alpha", 7), admitted(b"beta", 7));
    assert_ne!(admitted(b"alpha", 7), admitted(b"alpha", 8));

    let misplaced = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot),
            function(2, SemanticFunctionRoleV1::InternalHelper)
                .with_kernel_entry(entry(b"helper", 9)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    assert!(matches!(
        misplaced.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    let wrong_export_kind = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot).with_device_ffi_export_symbol(
                SemanticLinkSymbolV1::new(b"wrong_kind".to_vec()).unwrap(),
            ),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    assert!(matches!(
        wrong_export_kind.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));

    let duplicate = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"same", 1)),
            function(2, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"same", 2)),
        ],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    );
    assert!(matches!(
        duplicate.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));

    let duplicate_binding = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"first", 1)),
            function(2, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"second", 1)),
        ],
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    );
    assert!(matches!(
        duplicate_binding.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));

    let external_symbol = SemanticLinkSymbolV1::new(b"same_symbol".to_vec()).unwrap();
    let cross_namespace_collision = request_with_components(
        vec![u32_type(), unit_type()],
        vec![SemanticStaticDeclV1::new(
            SemanticStaticIdentityV1::from_sha256(bytes(1)),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTypeIdV1::from_index(0),
            false,
            0,
            SemanticStaticDefinitionV1::ExternalRequired {
                symbol: external_symbol,
            },
        )],
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot)
                .with_kernel_entry(entry(b"same_symbol", 1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    assert!(matches!(
        cross_namespace_collision.admit(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));

    let symbol_budget = request(
        vec![
            function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(entry(b"kernel", 7)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    assert!(matches!(
        symbol_budget.admit(
            SemanticMirLimitsV1::default()
                .with_limit(SemanticMirResourceV1::LinkSymbolBytes, 5)
                .unwrap()
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::LinkSymbolBytes,
            actual: 6,
            max: 5,
        })
    ));
}

#[test]
fn external_entry_abis_are_exact() {
    let out_of_range_return = function_with_abi(
        1,
        SemanticFunctionRoleV1::KernelRoot,
        SemanticTypeIdV1::from_index(99),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        SemanticAbiPassModeV1::Ignore,
    )
    .with_kernel_entry(entry(b"invalid_kernel", 1));
    assert!(
        request(
            vec![out_of_range_return],
            vec![SemanticFunctionIdV1::from_index(0)]
        )
        .admit(SemanticMirLimitsV1::default())
        .is_err()
    );

    let zst_kernel = function_with_abi(
        1,
        SemanticFunctionRoleV1::KernelRoot,
        SemanticTypeIdV1::from_index(2),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        SemanticAbiPassModeV1::Ignore,
    )
    .with_kernel_entry(entry(b"zst_kernel", 1));
    let non_unit_return = request_with_components(
        vec![u32_type(), unit_type(), empty_tuple_type()],
        vec![],
        vec![zst_kernel],
        vec![SemanticFunctionIdV1::from_index(0)],
    );
    let non_unit_return = non_unit_return.admit(SemanticMirLimitsV1::default());
    assert!(
        matches!(non_unit_return, Err(SemanticMirErrorV1::InvalidKernelEntry)),
        "unexpected non-unit kernel result: {non_unit_return:?}"
    );

    let direct_return = || {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        )
    };
    let cases = [
        (
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::Cdecl { unwind: false },
            false,
        ),
        (
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::System { unwind: false },
            false,
        ),
        (SemanticCanonAbiV1::Rust, SemanticExternAbiV1::Rust, false),
        (
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::C { unwind: false },
            true,
        ),
    ];
    for (canon_abi, extern_abi, c_variadic) in cases {
        let export = function_with_abi(
            1,
            SemanticFunctionRoleV1::DeviceFfiExport,
            SemanticTypeIdV1::from_index(0),
            canon_abi,
            extern_abi,
            c_variadic,
            direct_return(),
        )
        .with_device_ffi_export_symbol(SemanticLinkSymbolV1::new(b"device".to_vec()).unwrap());
        assert!(matches!(
            request(vec![export], vec![SemanticFunctionIdV1::from_index(0)])
                .admit(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidKernelEntry)
        ));
    }
}

#[test]
fn every_source_contract_field_is_canonical() {
    let baseline = kernel_digest(SemanticKernelSourceContractV1::new(None, None, None).unwrap());
    let required = SemanticWorkgroupDimensionsV1::new([256, 1, 1]).unwrap();
    let maximum = SemanticWorkgroupDimensionsV1::new([512, 1, 1]).unwrap();
    let launch_variants = [
        SemanticKernelLaunchBoundsV1::new(Some(required), None, None).unwrap(),
        SemanticKernelLaunchBoundsV1::new(
            Some(SemanticWorkgroupDimensionsV1::new([128, 1, 1]).unwrap()),
            None,
            None,
        )
        .unwrap(),
        SemanticKernelLaunchBoundsV1::new(Some(required), Some(maximum), None).unwrap(),
        SemanticKernelLaunchBoundsV1::new(Some(required), Some(maximum), Some(3)).unwrap(),
    ];
    for launch in launch_variants {
        assert_ne!(
            baseline,
            kernel_digest(SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap())
        );
    }

    let assembly_contract = |blocks, operand_bits, option_bits, effect_bits| {
        SemanticKernelSourceContractV1::new(
            None,
            Some(
                SemanticUnsafeAssemblyDeclarationV1::new(
                    SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
                    operand_bits,
                    option_bits,
                    effect_bits,
                )
                .unwrap(),
            ),
            Some(
                SemanticReachableAssemblyV1::new(blocks, operand_bits, option_bits, effect_bits)
                    .unwrap(),
            ),
        )
        .unwrap()
    };
    let assembly_baseline = kernel_digest(assembly_contract(
        1,
        0x0001,
        SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
        0,
    ));
    for contract in [
        assembly_contract(
            2,
            0x0001,
            SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
            0,
        ),
        assembly_contract(
            1,
            0x0002,
            SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
            0,
        ),
        assembly_contract(
            1,
            0x0001,
            SemanticUnsafeAssemblyDeclarationV1::OPTION_READONLY,
            0x0001,
        ),
        assembly_contract(
            1,
            0x0001,
            SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
            SemanticUnsafeAssemblyDeclarationV1::EFFECT_CONTROL_FLOW,
        ),
    ] {
        assert_ne!(assembly_baseline, kernel_digest(contract));
    }
    assert_ne!(baseline, assembly_baseline);
}

#[test]
fn launch_and_unsafe_assembly_contracts_fail_closed() {
    let unsafe_entry = SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"unsafe_kernel".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(8)),
        unsafe_source_contract(),
    );
    let admitted = request(
        vec![function(1, SemanticFunctionRoleV1::KernelRoot).with_kernel_entry(unsafe_entry)],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    assert!(matches!(
        admitted.require_complete_external_entries(),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    SemanticKernelSourceContractV1::new(None, None, None).unwrap();
    assert!(matches!(
        SemanticWorkgroupDimensionsV1::new([0, 1, 1]),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    assert!(matches!(
        SemanticWorkgroupDimensionsV1::new([1024, 2, 1]),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    let required = SemanticWorkgroupDimensionsV1::new([256, 1, 1]).unwrap();
    let maximum = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    assert!(matches!(
        SemanticKernelLaunchBoundsV1::new(Some(required), Some(maximum), None),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    assert!(matches!(
        SemanticKernelLaunchBoundsV1::new(None, None, Some(1)),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));

    let declaration = SemanticUnsafeAssemblyDeclarationV1::new(
        SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
        0x0001,
        SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
        0,
    )
    .unwrap();
    let mismatched = SemanticReachableAssemblyV1::new(
        1,
        0x0002,
        SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
        0,
    )
    .unwrap();
    assert!(matches!(
        SemanticKernelSourceContractV1::new(None, Some(declaration), Some(mismatched)),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    assert!(matches!(
        SemanticKernelSourceContractV1::new(None, Some(declaration), None),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
    assert!(matches!(
        SemanticUnsafeAssemblyDeclarationV1::new(
            SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
            0x0001,
            SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
            SemanticUnsafeAssemblyDeclarationV1::EFFECT_WRITE_GLOBAL,
        ),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));

    let declaration = SemanticUnsafeAssemblyDeclarationV1::new(
        SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
        0x0001,
        0,
        SemanticUnsafeAssemblyDeclarationV1::EFFECT_WRITE_GLOBAL,
    )
    .unwrap();
    let mismatched_effects = SemanticReachableAssemblyV1::new(
        1,
        0x0001,
        0,
        SemanticUnsafeAssemblyDeclarationV1::EFFECT_READ_GLOBAL,
    )
    .unwrap();
    assert!(matches!(
        SemanticKernelSourceContractV1::new(None, Some(declaration), Some(mismatched_effects),),
        Err(SemanticMirErrorV1::InvalidKernelEntry)
    ));
}
