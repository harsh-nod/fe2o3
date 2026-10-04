use super::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::*;

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 79;
const NAME: &str = "nominal_source";
const BINDING: [u8; 32] = [31; 32];
const KINDS: [SourceType; 5] = [
    SourceType::Usize,
    SourceType::Isize,
    SourceType::Scalar(Scalar::U64),
    SourceType::Scalar(Scalar::I64),
    SourceType::SharedSlice(Scalar::U64),
];

fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}

// Constructed owners keep target and FnAbi layout identities in separate domains.
// They are not ordinary-rustc capture or signed execution evidence.
fn semantic(erased: bool) -> ProductionSemanticMirOwnerV1 {
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let layout = SemanticLayoutIdentityV1::from_sha256([71; 32]);
    let unit = SemanticTypeIdV1::from_index(0);
    let u64_type = SemanticTypeIdV1::from_index(3);
    let mut types = vec![SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            Repr::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        Shape::Unit,
    )];
    for (i, kind) in [Kind::Usize, Kind::Isize, Kind::Ordinary, Kind::Ordinary]
        .into_iter()
        .enumerate()
    {
        let signed = i % 2 == 1;
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([10 + i as u8; 32]),
                SemanticLayoutIdentityV1::from_sha256([20 + i as u8; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    Repr::scalar(BackendScalar::initialized(
                        Primitive::integer(signed, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                Shape::Scalar(SourceScalar::Integer { signed, bits: 64 }),
            )
            .with_rust_type_kind(kind)
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true),
            ),
        );
    }
    let slice = SemanticTypeIdV1::from_index(5);
    let shared = SemanticTypeIdV1::from_index(6);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([15; 32]),
        SemanticLayoutIdentityV1::from_sha256([25; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            8,
            SemanticFieldsShapeV1::Array {
                stride_bytes: 8,
                count: 0,
            },
            SemanticRustcVariantsV1::Single { index: 0 },
            Repr::memory(false),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        Shape::Slice { element: u64_type },
    ));
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([16; 32]),
            SemanticLayoutIdentityV1::from_sha256([26; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                Repr::scalar_pair(
                    BackendScalar::initialized(
                        Primitive::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                    BackendScalar::initialized(
                        Primitive::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    ),
                ),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    slice,
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
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        Extension::None,
        0,
        None,
    )
    .unwrap();
    let shared_attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        Extension::None,
        0,
        Some(8),
    )
    .unwrap();
    let mut args = (1..=4)
        .map(|i| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(i),
                Mode::Direct(attrs),
            ))
        })
        .collect::<Vec<_>>();
    args.push(SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        shared,
        Mode::Pair {
            first: shared_attrs,
            second: attrs,
        },
    )));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([32; 32]),
        SemanticLayoutIdentityV1::from_sha256([72; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        5,
        args,
        SemanticAbiValueV1::new(unit, Mode::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SourceOwnership::ByValue,
        SourceOwnership::ByValue,
        SourceOwnership::ByValue,
        SourceOwnership::ByValue,
        SourceOwnership::SharedBorrow,
    ])
    .unwrap();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            provenance,
        )
    };
    let mut locals = vec![local(40, unit, SemanticLocalRoleV1::Return)];
    for i in 0..4 {
        locals.push(local(
            41 + i,
            SemanticTypeIdV1::from_index(u32::from(i) + 1),
            SemanticLocalRoleV1::Argument(u32::from(i)),
        ));
    }
    locals.push(local(45, shared, SemanticLocalRoleV1::Argument(4)));
    let block = |tag, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            provenance,
            vec![],
            SemanticTerminatorV1::new(provenance, terminator),
        )
        .unwrap()
    };
    let blocks = if erased {
        locals.push(local(46, unit, SemanticLocalRoleV1::Temporary));
        vec![
            block(
                50,
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(1),
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(6), vec![], unit)
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
            block(51, SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![block(50, SemanticTerminatorKindV1::Return)]
    };
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
        provenance,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(NAME.as_bytes().to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(BINDING),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let mut functions = vec![root];
    if erased {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([61; 32]),
            SemanticLayoutIdentityV1::from_sha256([73; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, Mode::Ignore),
        )
        .unwrap();
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([60; 32]),
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([60; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([60; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([60; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([60; 32]),
                provenance,
                abi,
                vec![
                    local(62, unit, SemanticLocalRoleV1::Return),
                    local(63, u64_type, SemanticLocalRoleV1::Temporary),
                ],
                SemanticBlockIdV1::from_index(0),
                vec![
                    SemanticBasicBlockV1::new(
                        SemanticBlockIdentityV1::from_sha256([63; 32]),
                        provenance,
                        vec![SemanticStatementV1::new(
                            provenance,
                            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(1),
                                    vec![],
                                    u64_type,
                                )
                                .unwrap(),
                                SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    u64_type,
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(0, 8).unwrap(),
                                    ),
                                )),
                                SemanticVolatilityV1::NonVolatile,
                                None,
                            )),
                        )],
                        SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
    }
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v35(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(admitted.types()[1].rust_type_kind(), Kind::Usize);
    assert_eq!(admitted.types()[2].rust_type_kind(), Kind::Isize);
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

enum Fixture {
    Direct(ProductionSemanticKirOwnerV1),
    Erased(ProductionUnitLocalErasedSourceOwnerV1),
}
impl Fixture {
    fn anchor(&self) -> Anchor<'_> {
        match self {
            Self::Direct(s) => Anchor::Direct(s),
            Self::Erased(s) => Anchor::Erased(s),
        }
    }
}

fn fixture(erased: bool, budget: &mut Budget<'_>) -> Fixture {
    let source = semantic(erased);
    let launch = ProductionSourceLaunchRosterV1::try_new(
        source.semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            NAME,
            BINDING,
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        )],
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let capture = ssa.occurrence_storage().map_or(0, |s| s.retained_storage());
    budget.reserve_storage(capture).unwrap();
    let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap();
    let types = materialized.semantic_ssa().source_semantic().types();
    assert_eq!(types[1].rust_type_kind(), Kind::Usize);
    assert_eq!(types[2].rust_type_kind(), Kind::Isize);
    let actual = &materialized.executable().module().functions[0]
        .signature
        .parameters;
    assert_eq!(
        &actual[..4],
        &[
            Type::Scalar(KirScalar::U64),
            Type::Scalar(KirScalar::I64),
            Type::Scalar(KirScalar::U64),
            Type::Scalar(KirScalar::I64)
        ]
    );
    assert!(
        matches!(&actual[4],Type::Slice(s) if s.address_space==AddressSpace::Global && s.access==KirAccess::ReadOnly)
    );
    let root = materialized.source_launch().roots()[0];
    let layout = root.layout();
    let kernel = ProductionRankedKernelV1::new(
        NAME,
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: layout.grid_identity(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                subgroup_size: layout.subgroup_size(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            }],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("nominal_source_fixture", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    let roots = vec![ProductionRankedSemanticProjectionRootV1::new(
        root.selected_root(),
        1,
        lowering,
        "actual source with empty external footprint".to_owned(),
        vec![],
        vec![],
    )];
    if erased {
        assert_eq!(
            materialized.helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::UnitLocal
        );
        let floor = ProductionUnitLocalErasedSourceOwnerV1::input_storage_floor_v1(
            &materialized,
            &roots,
            budget,
        )
        .unwrap();
        budget.reserve_storage(floor - capture).unwrap();
        let (owner, added) =
            ProductionUnitLocalErasedSourceOwnerV1::try_produce_v1(materialized, roots, budget)
                .unwrap();
        budget.reserve_storage(added.retained_storage()).unwrap();
        assert_eq!(
            (owner.deleted_call_count(), owner.deleted_function_count()),
            (1, 1)
        );
        Fixture::Erased(owner)
    } else {
        budget
            .reserve_storage(materialized.retained_analysis_storage_v1())
            .unwrap();
        let receipt = ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(materialized, roots).unwrap();
        let owner =
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(
                receipt, budget,
            )
            .unwrap();
        assert!(owner.pre_ranked_executable().is_some());
        Fixture::Direct(owner)
    }
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    FixedForNominal,
    NominalForFixed,
    SwapSigns,
    Binding,
    Export,
    Grid,
    Workgroup,
    Offset,
    Missing,
    Segment,
    DisjointForShared,
}

fn wire(mutation: Mutation, target: &str) -> Vec<u8> {
    let layout = |k| match k {
        SourceType::SharedSlice(s) => Layout::shared_slice(s),
        SourceType::DisjointSlice(s) => Layout::disjoint_slice(s),
        _ => Layout::scalar(k.physical_scalar()),
    };
    let mut kinds = KINDS.to_vec();
    match mutation {
        Mutation::FixedForNominal => kinds[0] = SourceType::Scalar(Scalar::U64),
        Mutation::NominalForFixed => kinds[2] = SourceType::Usize,
        Mutation::SwapSigns => kinds.swap(0, 1),
        Mutation::DisjointForShared => kinds[4] = SourceType::DisjointSlice(Scalar::U64),
        Mutation::Missing => {
            kinds.pop();
        }
        _ => {}
    }
    let mut sources = kinds
        .iter()
        .map(|k| SourceTypeRecordV3::new(*k, &mut free).unwrap())
        .collect::<Vec<_>>();
    sources.sort_by_key(|r| r.identity());
    sources.dedup();
    let mut layouts = kinds
        .iter()
        .map(|k| device_layout_record_v3(layout(*k), &mut free).unwrap())
        .collect::<Vec<_>>();
    layouts.sort_by_key(|r| r.identity());
    layouts.dedup();
    let components = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let offset = 8 * i as u32
                + if matches!(mutation, Mutation::Offset) {
                    8
                } else {
                    0
                };
            if matches!(k, SourceType::SharedSlice(_) | SourceType::DisjointSlice(_)) {
                vec![
                    PhysicalComponentV3 {
                        kind: Component::GlobalPointer,
                        offset,
                        size: 8,
                        alignment: 8,
                        access: if matches!(k, SourceType::SharedSlice(_)) {
                            Access::ReadOnly
                        } else {
                            Access::ReadWrite
                        },
                        alias: if matches!(k, SourceType::SharedSlice(_)) {
                            Alias::SharedReadOnly
                        } else {
                            Alias::Exclusive
                        },
                    },
                    PhysicalComponentV3 {
                        kind: Component::SliceLengthU64,
                        offset: offset + 8,
                        size: 8,
                        alignment: 8,
                        access: Access::ByValue,
                        alias: Alias::Value,
                    },
                ]
            } else {
                vec![PhysicalComponentV3 {
                    kind: Component::ScalarByValue(k.physical_scalar()),
                    offset,
                    size: 8,
                    alignment: 8,
                    access: Access::ByValue,
                    alias: Alias::Value,
                }]
            }
        })
        .collect::<Vec<_>>();
    let names = ["usize_arg", "isize_arg", "u64_arg", "i64_arg", "shared_arg"];
    let arguments = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| LogicalArgumentInputV3 {
            source_index: i as u16,
            name: names[i],
            source_type: SourceTypeRecordV3::new(*k, &mut free).unwrap().identity(),
            device_layout: device_layout_record_v3(layout(*k), &mut free)
                .unwrap()
                .identity(),
            ownership: match k {
                SourceType::SharedSlice(_) => Ownership::SharedBorrow,
                SourceType::DisjointSlice(_) => Ownership::UniqueBorrow,
                _ => Ownership::ByValue,
            },
            access: match k {
                SourceType::SharedSlice(_) => Access::ReadOnly,
                SourceType::DisjointSlice(_) => Access::ReadWrite,
                _ => Access::ByValue,
            },
            alias: match k {
                SourceType::SharedSlice(_) => Alias::SharedReadOnly,
                SourceType::DisjointSlice(_) => Alias::Exclusive,
                _ => Alias::Value,
            },
            components: &components[i],
        })
        .collect::<Vec<_>>();
    let block = if matches!(mutation, Mutation::Workgroup) {
        32
    } else {
        64
    };
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(block, 1, 1).unwrap()),
        DimensionsV1::new(
            if matches!(mutation, Mutation::Grid) {
                4
            } else {
                3
            },
            1,
            1,
        )
        .unwrap(),
        block,
        0,
        0,
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([81; 32]),
        EvidenceDigest::from_sha256_bytes([82; 32]),
    );
    let id = KernelId::from_bytes(if matches!(mutation, Mutation::Binding) {
        [32; 32]
    } else {
        BINDING
    });
    let size = kinds
        .iter()
        .map(|k| u32::from(layout(*k).size_bytes()))
        .sum::<u32>()
        + if matches!(mutation, Mutation::Offset) {
            8
        } else {
            0
        };
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "independent_label",
        entry_name: if matches!(mutation, Mutation::Export) {
            "other"
        } else {
            NAME
        },
        descriptor_symbol: if matches!(mutation, Mutation::Export) {
            "other.kd"
        } else {
            "nominal_source.kd"
        },
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(
            size,
            size + if matches!(mutation, Mutation::Segment) {
                264
            } else {
                256
            },
            8,
        )
        .unwrap(),
        launch: &launch,
        arguments: &arguments,
    }];
    let compiler = CompilerIdentityV1::new(
        Text::new("fixture-rustc").unwrap(),
        Text::new("test").unwrap(),
        [7; 20],
    );
    let producer =
        ProducerIdentityV1::new(Text::new("fixture").unwrap(), Text::new("test").unwrap());
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::empty(),
        AtomicRequirementsV2::empty(),
    )];
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(target).unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let len = encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap();
    let mut output = vec![0; len];
    encode_device_descriptor_table_v3(&input, &mut output, &mut free).unwrap();
    output
}

fn with_table<T>(
    bytes: &[u8],
    budget: &mut Budget<'_>,
    run: impl FnOnce(&Table<'_>, &mut Budget<'_>) -> T,
) -> T {
    let floor = budget.storage();
    let reader = DESCRIPTOR_READER_SCRATCH_STORAGE_V3;
    budget
        .reserve_storage(reader + DESCRIPTOR_TABLE_VIEW_STORAGE_V3)
        .unwrap();
    let table = decode_device_descriptor_table_v3(bytes, &mut |w| budget.charge_work(w)).unwrap();
    budget.release_storage(reader).unwrap();
    let result = run(&table, budget);
    drop(table);
    budget
        .release_storage(DESCRIPTOR_TABLE_VIEW_STORAGE_V3)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn nominal_abi_direct_and_erased_bind_actual_nominal_and_fixed_source_arguments() {
    for erased in [false, true] {
        for target in ["gfx942", "gfx950"] {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FLOOR).unwrap();
            let source = fixture(erased, &mut budget);
            let wire = wire(Mutation::None, target);
            budget
                .reserve_storage(size_of_val(&wire) + wire.capacity())
                .unwrap();
            with_table(&wire, &mut budget, |table, budget| {
                let floor = budget.storage();
                let (agreement, storage) =
                    check_nominal_source_abi_v3(source.anchor(), table, budget).unwrap();
                assert_eq!(storage.retained_storage(), HEADER);
                assert_eq!(agreement.retained_storage(), HEADER);
                assert!(std::ptr::eq(agreement.table, table));
                match (agreement.source, source.anchor()) {
                    (Anchor::Direct(a), Anchor::Direct(b)) => assert!(std::ptr::eq(a, b)),
                    (Anchor::Erased(a), Anchor::Erased(b)) => assert!(std::ptr::eq(a, b)),
                    _ => panic!("same retained source variant"),
                }
                assert_eq!(budget.storage(), floor);
                assert!(!agreement.grants_artifact_or_launch_authority());
                budget.reserve_storage(storage.retained_storage()).unwrap();
                agreement.verify_equivalence(budget).unwrap();
                assert_eq!(budget.storage(), floor + HEADER);
                drop(agreement);
                budget.release_storage(storage.retained_storage()).unwrap();
            });
            drop(wire);
            drop(source);
            budget.release_storage(budget.storage() - FLOOR).unwrap();
            assert_eq!(budget.storage(), FLOOR);
        }
    }
}

#[test]
fn nominal_abi_full_table_mutations_keep_typed_source_and_physical_refusals() {
    for erased in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let source = fixture(erased, &mut budget);
        for mutation in [
            Mutation::FixedForNominal,
            Mutation::NominalForFixed,
            Mutation::SwapSigns,
            Mutation::Binding,
            Mutation::Export,
            Mutation::Grid,
            Mutation::Workgroup,
            Mutation::Offset,
            Mutation::Missing,
            Mutation::Segment,
            Mutation::DisjointForShared,
        ] {
            let bytes = wire(mutation, "gfx942");
            let wire_storage = size_of_val(&bytes) + bytes.capacity();
            budget.reserve_storage(wire_storage).unwrap();
            with_table(&bytes, &mut budget, |table, budget| {
                let floor = budget.storage();
                let error = check_nominal_source_abi_v3(source.anchor(), table, budget)
                    .err()
                    .unwrap();
                match mutation {
                    Mutation::FixedForNominal | Mutation::SwapSigns => assert!(matches!(
                        error,
                        E::NominalKind {
                            root: 0,
                            argument: 0
                        }
                    )),
                    Mutation::NominalForFixed => assert!(matches!(
                        error,
                        E::NominalKind {
                            root: 0,
                            argument: 2
                        }
                    )),
                    Mutation::Binding => assert!(matches!(
                        error,
                        E::Descriptor(DescriptorWireErrorV3::Decode(DecodeError::Validation(
                            ValidationError::DanglingReference { field: "kernel" }
                        )))
                    )),
                    Mutation::Export => assert!(matches!(error, E::Root { ordinal: 0 })),
                    Mutation::Grid | Mutation::Workgroup => {
                        assert!(matches!(error, E::Launch { ordinal: 0 }))
                    }
                    Mutation::Offset => assert!(matches!(
                        error,
                        E::PhysicalLayout {
                            root: 0,
                            argument: 0
                        }
                    )),
                    Mutation::Missing => assert!(matches!(error, E::Signature { root: 0 })),
                    Mutation::Segment => assert!(matches!(error, E::KernargLayout { root: 0 })),
                    Mutation::DisjointForShared => assert!(matches!(
                        error,
                        E::Argument {
                            root: 0,
                            argument: 4
                        }
                    )),
                    Mutation::None => panic!("not a mutation"),
                }
                assert_eq!(budget.storage(), floor);
            });
            drop(bytes);
            budget.release_storage(wire_storage).unwrap();
        }
        drop(source);
        budget.release_storage(budget.storage() - FLOOR).unwrap();
    }
}

#[test]
fn nominal_abi_disconnected_source_and_work_entry_refuse_without_allocating() {
    let source = ProductionSemanticKirOwnerV1::try_lower(
        semantic(false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    assert!(source.pre_ranked_executable().is_none());
    let bytes = wire(Mutation::None, "gfx942");
    let table = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
    for limit in [3, 4] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor =
            FLOOR + bytes.capacity() + size_of_val(&bytes) + DESCRIPTOR_TABLE_VIEW_STORAGE_V3;
        budget.reserve_storage(floor).unwrap();
        let result = check_nominal_source_abi_v3(Anchor::Direct(&source), &table, &mut budget);
        if limit == 3 {
            let Err(E::Resource(Resource::Work(error))) = result else {
                panic!("entry Work denial")
            };
            assert_eq!((error.actual(), error.limit()), (4, 3));
        } else {
            assert!(matches!(result, Err(E::MissingConnectedSource)));
        }
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (if limit == 3 { 0 } else { 4 }, floor, floor, None)
        );
    }
}

#[test]
fn nominal_abi_initial_header_and_query_storage_short_are_exact() {
    for erased in [false, true] {
        let mut setup_work = Work::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let source = fixture(erased, &mut setup);
        let bytes = wire(Mutation::None, "gfx942");
        let table = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
        let floor = setup.storage()
            + FLOOR
            + size_of_val(&bytes)
            + bytes.capacity()
            + DESCRIPTOR_TABLE_VIEW_STORAGE_V3;
        for (extra, accepted, attempt) in [
            (HEADER - 1, 0, HEADER),
            (HEADER + SCRATCH - 1, HEADER, HEADER + SCRATCH),
        ] {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, floor + extra);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let Err(E::Resource(Resource::Storage(error))) =
                check_nominal_source_abi_v3(source.anchor(), &table, &mut budget)
            else {
                panic!("exact initial storage phase")
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (floor + attempt, floor + extra)
            );
            assert_eq!(
                (
                    budget.work(),
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (4, floor, floor + accepted, Some(floor + attempt))
            );
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
        drop(source);
    }
}

#[test]
fn nominal_abi_exact_factory_work_peak_and_final_work_short_are_replayed() {
    for erased in [false, true] {
        let mut setup_work = Work::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let source = fixture(erased, &mut setup);
        let bytes = wire(Mutation::None, "gfx942");
        let table = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
        let sibling = vec![0x35u8; 37];
        let floor = setup.storage()
            + FLOOR
            + size_of_val(&bytes)
            + bytes.capacity()
            + DESCRIPTOR_TABLE_VIEW_STORAGE_V3
            + size_of_val(&sibling)
            + sibling.capacity();
        let run = |wl, sl| {
            let mut work = Work::new(wl);
            let metrics = {
                let mut budget = Budget::new(&mut work, sl);
                budget.reserve_storage(floor).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = check_nominal_source_abi_v3(source.anchor(), &table, &mut budget).map(
                    |(value, storage)| {
                        assert_eq!(storage.retained_storage(), HEADER);
                        drop(value);
                    },
                );
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                (
                    result,
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                )
            };
            (metrics, work.failed_work())
        };
        let ((result, w, p, failed), fw) = run(WORK, STORAGE);
        result.unwrap();
        assert_eq!((failed, fw), (None, None));
        let ((result, w2, p2, failed), fw) = run(w, p);
        result.unwrap();
        assert_eq!((w2, p2, failed, fw), (w, p, None, None));
        let ((result, w2, p2, failed), fw) = run(w - 1, p);
        let Err(E::Resource(Resource::Work(error))) = result else {
            panic!("final Work unit")
        };
        assert_eq!((error.actual(), error.limit()), (w, w - 1));
        assert_eq!((w2, p2, failed, fw), (w - 1, p, None, Some(w)));
        assert_eq!(sibling, [0x35; 37]);
        drop(source);
    }
}

#[test]
fn nominal_abi_replay_requires_retained_agreement_floor() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let source = fixture(false, &mut budget);
    let bytes = wire(Mutation::None, "gfx942");
    budget
        .reserve_storage(size_of_val(&bytes) + bytes.capacity())
        .unwrap();
    with_table(&bytes, &mut budget, |table, budget| {
        let (value, storage) = check_nominal_source_abi_v3(source.anchor(), table, budget).unwrap();
        let before = budget.work();
        assert!(matches!(
            value.verify_equivalence(budget),
            Err(E::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.work(), before);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        value.verify_equivalence(budget).unwrap();
        drop(value);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
    drop(source);
}

#[test]
fn nominal_abi_original_source_and_borrowed_table_floor_is_required() {
    for erased in [false, true] {
        let mut setup_work = Work::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let source = fixture(erased, &mut setup);
        let bytes = wire(Mutation::None, "gfx942");
        let table = decode_device_descriptor_table_v3(&bytes, &mut free).unwrap();
        let required =
            source_floor(source.anchor()).unwrap() + bytes.len() + DESCRIPTOR_TABLE_VIEW_STORAGE_V3;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(required - 1).unwrap();
        assert!(matches!(
            check_nominal_source_abi_v3(source.anchor(), &table, &mut budget),
            Err(E::Resource(Resource::Accounting))
        ));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (4, required - 1, required - 1, None)
        );
        drop(source);
    }
}

#[test]
fn nominal_abi_unsupported_source_shapes_do_not_coerce_into_nominal_scalars() {
    let source = semantic(false);
    let actual = &source.semantic().types()[1];
    for kind in [Kind::Ordinary, Kind::Isize] {
        let altered = actual.clone().with_rust_type_kind(kind);
        assert!(matches!(
            nominal(&altered, SourceType::Usize, 2, 3),
            Err(E::NominalKind {
                root: 2,
                argument: 3
            })
        ));
    }
    for (layout, shape) in [
        (
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                Repr::scalar(BackendScalar::initialized(
                    Primitive::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            Shape::Scalar(SourceScalar::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        (
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            Shape::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ),
    ] {
        // Private predicate controls, not fabricated admitted source owners.
        let altered =
            SemanticTypeDeclV1::new(actual.identity(), actual.layout_identity(), layout, shape)
                .with_rust_type_kind(Kind::Usize);
        assert!(matches!(
            nominal(&altered, SourceType::Usize, 2, 3),
            Err(E::NominalKind {
                root: 2,
                argument: 3
            })
        ));
    }
}

#[test]
fn nominal_abi_scope_unwind_and_foreign_ledger_do_not_refund_siblings() {
    let sibling = vec![0x79u8; 23];
    let floor = FLOOR + size_of_val(&sibling) + sibling.capacity();
    let mut work = Work::new(WORK);
    let mut other_work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let mut other = Budget::new(&mut other_work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    other.reserve_storage(floor + 7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let foreign = other.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut budget, |b| {
        b.reserve_storage(SCRATCH)?;
        panic!("after paid scope");
    });
    assert!(matches!(result, Err(E::Panicked)));
    assert_eq!(budget.storage(), floor);
    let result = scoped(&mut budget, |b| {
        b.charge_work(4)?;
        std::mem::swap(b, &mut other);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert!(budget.work_ledger_identity_v1() == foreign);
    assert_eq!(budget.storage(), floor + 7);
    std::mem::swap(&mut budget, &mut other);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), floor);
    assert_eq!(sibling, [0x79; 23]);
}

#[test]
fn nominal_abi_panic_payload_drops_after_scope_refund() {
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            panic!("payload destructor");
        }
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: R<()> = scoped(&mut budget, |b| {
            b.reserve_storage(SCRATCH)?;
            std::panic::panic_any(Payload)
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn nominal_abi_artifact_and_descriptor_domains_are_distinct_exact_literals() {
    use sha2::{Digest, Sha256};
    for (kind, source, tag) in [
        (RustNominalScalarKindV3::Usize, SourceType::Usize, 5u8),
        (RustNominalScalarKindV3::Isize, SourceType::Isize, 6u8),
    ] {
        let evidence = RustNominalScalarEvidenceV3::new(kind, PointerWidth::Bits64).unwrap();
        assert_eq!(evidence.canonical_type_payload(), [3, 0, tag, 0]);
        assert_eq!(source.canonical_payload(), [tag, 0, 0, 0]);
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/RUST-TYPE/V3\0");
        hash.update(4u64.to_le_bytes());
        hash.update([tag, 0, 0, 0]);
        let descriptor: [u8; 32] = hash.finalize().into();
        assert_eq!(
            SourceTypeRecordV3::new(source, &mut free)
                .unwrap()
                .identity()
                .as_bytes(),
            &descriptor
        );
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/RUST-NOMINAL-TYPE/V3\0");
        hash.update(4u64.to_le_bytes());
        hash.update([3, 0, tag, 0]);
        let portable: [u8; 32] = hash.finalize().into();
        assert_ne!(portable, descriptor);
        let identity = evidence.type_identity();
        assert_eq!(identity.rust_type().bytes().as_bytes(), &portable);
        let mut payload = [0u8; 56];
        payload[..32].copy_from_slice(&portable);
        payload[32..34].copy_from_slice(&64u16.to_le_bytes());
        payload[34] = if tag == 5 { 8 } else { 7 };
        payload[35] = 1;
        payload[36..44].copy_from_slice(&8u64.to_le_bytes());
        payload[44..48].copy_from_slice(&8u32.to_le_bytes());
        assert_eq!(evidence.canonical_layout_payload(), payload);
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/RUST-NOMINAL-LAYOUT/V3\0");
        hash.update(payload);
        let layout: [u8; 32] = hash.finalize().into();
        assert_eq!(identity.layout().bytes().as_bytes(), &layout);
        assert_eq!(&evidence.canonical_layout_payload()[..32], &portable);
        assert_eq!(
            evidence.canonical_layout_payload()[32..34],
            64u16.to_le_bytes()
        );
        assert!(RustNominalScalarEvidenceV3::new(kind, PointerWidth::Bits32).is_err());
    }
}
