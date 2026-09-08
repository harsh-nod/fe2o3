fn runtime_v7_semantic_fixture() -> (Vec<u8>, u16, [u8; 32], [u8; 32]) {
    let target_layout = [0x71; 32];
    let abi_identity = [0x72; 32];
    let mut types = owned_region_slice_semantic_types(
        [
            SemanticTypeIdV1::from_index(1),
            SemanticTypeIdV1::from_index(2),
            SemanticTypeIdV1::from_index(3),
        ],
        Some(u64::MAX),
    );
    types[3] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([0x84; 32]),
        SemanticLayoutIdentityV1::from_sha256([0x85; 32]),
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
    let unit = SemanticTypeIdV1::from_index(5);
    let unit_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([0x88; 32]),
        SemanticLayoutIdentityV1::from_sha256([0x89; 32]),
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
        1,
        vec![owned_region_slice_physical()],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
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
    let launch =
        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
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
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([0x7b; 32]),
                unit,
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([0x7d; 32]),
                SemanticTypeIdV1::from_index(4),
                SemanticLocalRoleV1::Argument(0),
                SemanticSourceProvenanceV1::unavailable(),
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"bundle_v7_entry".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([0x7c; 32]),
        contract,
    ));
    types.push(unit_type);
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(target_layout)),
        types,
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

fn runtime_v7_bundle_fixture() -> (VerifiedSimulationBundleV7, [u8; 32]) {
    let scalar = Type::Scalar(ScalarType::U32);
    let physical = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let context = KernelContextTypeV1::new("bundle_v7_entry", [0x81; 32], [0x82; 32], [0x83; 32]);
    let capability = GlobalCapabilityTypeV1::read_only(scalar.clone(), context.clone());
    let global_read = capability.physical_pointer_type();
    let read_write = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let read_only = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(8),
            context,
            KernelContextSourceIdentityV1::new([0x84; 32], [0x85; 32], [0x86; 32], [0x87; 32]),
        ),
        Operation::global_capability_bind(ValueId(9), capability, ValueId(8), ValueId(7)),
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::global_capability_index(ValueId(11), ValueId(9), ValueId(10), None),
        Operation::effect_free(
            ValueDef::new(ValueId(12), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(9) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(11),
                rhs: ValueId(12),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), Type::INDEX),
            OperationKind::Select {
                condition: ValueId(13),
                true_value: ValueId(11),
                false_value: ValueId(10),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(15), global_read.clone()),
            OperationKind::SliceData { slice: ValueId(9) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(16), global_read),
            OperationKind::GetElementPointer {
                base: ValueId(15),
                offset: ValueId(14),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(17), scalar.clone()),
            OperationKind::Constant(Constant::U32(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(18), scalar.clone()),
            OperationKind::GuardedLoad {
                pointer: ValueId(16),
                predicate: ValueId(13),
                fallback: ValueId(17),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
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
        "bundle_v7_entry",
        Signature::new(vec![physical], vec![]),
        vec![ValueId(7)],
        vec![block],
    );
    let mut module = Module::new("sim-runtime-bundle-v7-test");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "bundle_v7_kernel",
        "bundle_v7_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let canonical = VerifiedCanonicalKernelIrV12::from_module(module).unwrap();
    let production_digest = *canonical.identity().digest();
    let production_length = canonical.identity().canonical_length();
    let prepared = PreparedSimulationBundleV7::new(
        SimulationSourceLineageV1::new([0x7d; 32], 201, [0x7e; 32], 202).unwrap(),
        SimulationProductionKirIdentityV7::new(12, production_digest, production_length).unwrap(),
        37,
        "gfx942:xnack-",
        canonical,
    )
    .unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([0x7f; 32], 16, "runtime-bundle-v7.rs".into()).unwrap()],
        vec![],
        vec![DebugSourceMapSpanV1::new([0x7f; 32], 1, 2, 1, 2).unwrap()],
        vec![],
        vec![],
    )
    .unwrap();
    let (semantic, semantic_version, semantic_digest, abi_identity) = runtime_v7_semantic_fixture();
    let storage = SemanticStorageMapV7::new(
        *prepared.subject_identity(),
        semantic_version,
        semantic_digest,
        semantic.len() as u64,
        [0x71; 32],
        *prepared.canonical_kir_v12_digest(),
        prepared.canonical_kir_v12_length(),
        vec![SemanticKernelStorageV1::new(
            0,
            0,
            0,
            vec![SemanticArgumentStorageV1::new(
                0,
                1,
                4,
                SemanticArgumentOwnershipV1::SharedBorrow,
                SemanticStorageBindingV1::ExactKirParameter {
                    kir_parameter_ordinal: 0,
                    kir_value_ordinal: 7,
                    representation: SemanticKirStorageRepresentationV1::RegionSlice,
                },
            )],
        )],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV7::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v12_digest(),
        prepared.canonical_kir_v12_length(),
        vec![SemanticKernelStorageV2::new(
            0,
            0,
            0,
            16,
            8,
            vec![SemanticArgumentStorageV2::new(
                0,
                1,
                4,
                SemanticArgumentOwnershipV1::SharedBorrow,
                SemanticComponentStorageBindingV2::exact(vec![owned_region_slice_component(
                    Vec::new(),
                    SemanticKernargSlotV2::new(0, 8, 8),
                    Some(SemanticKernargSlotV2::new(8, 8, 8)),
                )]),
            )],
        )],
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
fn bundle_v7_executes_exact_v12_context_and_global_capabilities_without_downgrade() {
    let (bundle, abi_identity) = runtime_v7_bundle_fixture();
    assert_eq!(bundle.production_kir_identity().version(), 12);
    assert_eq!(bundle.final_graph_epoch(), 37);
    let parsed = parse_bundle(
        bundle.canonical_bytes(),
        VirtualTargetProfileV1::Gfx942XnackMinus,
    )
    .unwrap();
    assert_eq!(parsed.admitted.identity().wire_version(), 12);
    assert_eq!(
        parsed.admitted.identity().digest(),
        bundle.canonical_kir_v12_digest()
    );
    assert_eq!(parsed.final_graph_epoch, Some(37));

    let mut backend = SimRuntimeBackendV1::gfx942([0x70; 32]).unwrap();
    let stream = backend.create_stream_v1(DEVICE_HANDLE).unwrap();
    let allocation = backend
        .allocate_v1(DEVICE_HANDLE, RuntimeMemoryKindV1::HostVisible, 4, 4)
        .unwrap();
    backend
        .write_allocation_v1(allocation, 0, &7_u32.to_le_bytes())
        .unwrap();
    let module = backend
        .load_module_v1(DEVICE_HANDLE, bundle.canonical_bytes())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "bundle_v7_kernel", abi_identity)
        .unwrap();
    let mut kernarg = [0_u8; 16];
    kernarg[8..].copy_from_slice(&1_u64.to_le_bytes());
    let binding = BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 4,
        },
        kernarg_byte_offset: 0,
    };
    let submission = backend
        .submit_v1(BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &kernarg,
            bindings: &[binding],
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
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
}
