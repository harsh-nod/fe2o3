fn runtime_v6_semantic_fixture() -> (Vec<u8>, u16, [u8; 32], [u8; 32]) {
    let target_layout = [0x71; 32];
    let abi_identity = [0x72; 32];
    let unit = SemanticTypeIdV1::from_index(0);
    let unit_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([0x73; 32]),
        SemanticLayoutIdentityV1::from_sha256([0x74; 32]),
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
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(abi_identity),
        SemanticLayoutIdentityV1::from_sha256(target_layout),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([0x75; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    let launch = SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let contract = SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([0x76; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([0x77; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([0x78; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([0x79; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([0x7a; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([0x7b; 32]),
            unit,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"bundle_v6_entry".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([0x7c; 32]),
        contract,
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(target_layout)),
        vec![unit_type],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    (
        admitted.canonical_encoding().to_vec(),
        admitted.wire_version().as_u16(),
        *admitted.semantic_sha256().as_bytes(),
        abi_identity,
    )
}

fn runtime_v6_bundle_fixture() -> (VerifiedSimulationBundleV6, [u8; 32]) {
    let scalar = Type::Scalar(ScalarType::U32);
    let read_write = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let read_only = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(0), scalar.clone()),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), read_write),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), read_only.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(1),
                to: read_only,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar),
            OperationKind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "bundle_v6_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let mut module = Module::new("sim-runtime-bundle-v6-test");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "bundle_v6_kernel",
        "bundle_v6_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let canonical = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
    let production_digest = *canonical.identity().digest();
    let production_length = canonical.identity().canonical_length();
    let prepared = PreparedSimulationBundleV6::new(
        SimulationSourceLineageV1::new([0x7d; 32], 201, [0x7e; 32], 202).unwrap(),
        SimulationProductionKirIdentityV6::new(11, production_digest, production_length).unwrap(),
        "gfx942:xnack-",
        canonical,
    )
    .unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new(
            [0x7f; 32],
            16,
            "runtime-bundle-v6.rs".into(),
        )
        .unwrap()],
        vec![],
        vec![DebugSourceMapSpanV1::new([0x7f; 32], 1, 2, 1, 2).unwrap()],
        vec![],
        vec![],
    )
    .unwrap();
    let (semantic, semantic_version, semantic_digest, abi_identity) =
        runtime_v6_semantic_fixture();
    let storage = SemanticStorageMapV6::new(
        *prepared.subject_identity(),
        semantic_version,
        semantic_digest,
        semantic.len() as u64,
        [0x71; 32],
        *prepared.canonical_kir_v11_digest(),
        prepared.canonical_kir_v11_length(),
        vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV6::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v11_digest(),
        prepared.canonical_kir_v11_length(),
        vec![SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![])],
    )
    .unwrap();
    (
        prepared
            .finalize(source_map, semantic, storage, aggregate)
            .unwrap(),
        abi_identity,
    )
}

#[test]
fn bundle_v6_executes_through_the_normal_runtime_lifecycle() {
    let (bundle, abi_identity) = runtime_v6_bundle_fixture();
    assert_eq!(bundle.production_kir_identity().version(), 11);
    let mut backend = SimRuntimeBackendV1::gfx942([0x70; 32]).unwrap();
    let stream = backend.create_stream_v1(DEVICE_HANDLE).unwrap();
    let module = backend
        .load_module_v1(DEVICE_HANDLE, bundle.canonical_bytes())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "bundle_v6_kernel", abi_identity)
        .unwrap();
    let submission = backend
        .submit_v1(BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &[],
            bindings: &[],
            dependencies: &[],
            geometry: fe2o3_runtime::RuntimeLaunchGeometryV1 {
                grid: [2, 1, 1],
                workgroup: [1, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: fe2o3_runtime::BackendSemanticLaunchV1::Ordinary,
        })
        .unwrap();
    assert_eq!(
        backend
            .wait_v1(submission, Instant::now() + Duration::from_secs(5))
            .unwrap(),
        BackendPollV1::Succeeded
    );
    backend.release_submission_v1(submission).unwrap();
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
}
