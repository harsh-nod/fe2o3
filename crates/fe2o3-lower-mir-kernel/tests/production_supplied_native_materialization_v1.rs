use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1, CheckedKernelIrContractCatalogV1, check_kernel_ir_contract_catalog_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    KernelIrPipelineContractDefinitionV1 as Definition, OperationKind, Terminator,
    VerifiedCanonicalKernelIrModuleV12 as Native,
};
use fe2o3_lower_mir_kernel::{
    ProductionPreRankedKirOwnerV1 as Materialized, ProductionSemanticKirErrorV1,
    ProductionSemanticKirLimitsV1 as Limits, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1 as Launch,
    ReplayedSuppliedNativeMaterializationV1 as Relation,
    SuppliedNativeMaterializationErrorV1 as Error,
    check_supplied_native_materialization_consistency_v1 as check,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1 as Ssa,
};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const PREFIX: usize = 11;
const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const SCOPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const SCOPE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const PIPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const PIPE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn scalar(value: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 4).unwrap()),
    ))
}
fn assignment(local: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, U32),
            SemanticRvalueV1::new(U32, value),
        )),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}
fn types(pipeline: bool) -> Vec<SemanticTypeDeclV1> {
    let mut types = vec![SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([4; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
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
    )];
    for (tag, bits, size, maximum) in [(5, 32, 4, u32::MAX as u128), (6, 64, 8, u64::MAX as u128)] {
        if bits == 64 && !pipeline {
            continue;
        }
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, bits, size),
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        ));
    }
    if pipeline {
        for (tag, pointee) in [(150, SCOPE), (152, PIPE)] {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::aggregate_with_backend_repr(
                    Some(0),
                    1,
                    SemanticBackendReprV1::memory(true),
                    false,
                    SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            ));
            types.push(
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([tag + 1; 32]),
                    SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
                    SemanticTypeLayoutV1::new_with_backend_repr(
                        Some(8),
                        8,
                        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                        )),
                        false,
                    )
                    .unwrap(),
                    SemanticTypeShapeV1::Pointer(
                        SemanticPointerTypeV1::new_with_kind(
                            pointee,
                            SemanticPointerKindV1::Reference,
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
                        Some(
                            SemanticAbiPointeeInfoV1::new(
                                SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                                0,
                                1,
                            )
                            .unwrap(),
                        ),
                        None,
                    ),
                ),
            );
        }
    }
    types
}
fn abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let mode = if matches!(ty, UNIT | SCOPE | PIPE) {
        SemanticAbiPassModeV1::Ignore
    } else {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    false,
                    None,
                    matches!(ty, SCOPE_REF | PIPE_REF),
                    false,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        )
    };
    SemanticAbiValueV1::new(ty, mode)
}
fn intrinsic(
    tag: u8,
    layout: u8,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([layout; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                inputs.iter().copied().map(abi_value).collect(),
                abi_value(output),
            )
            .unwrap(),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

// Real source body order puts the shared helper before the first kernel. The
// two roots reuse local IDs and share a helper through different Call paths.
fn source(multi: bool, pipeline: bool, value: u32, layout: u8) -> Ssa {
    let mut functions = Vec::new();
    for (ordinal, tag, export, dimensions, binding, callee) in [
        (0, 60, None, [1, 1, 1], 0, None),
        (1, 70, Some("zeta_entry"), [64, 1, 1], 0xa1, Some(2)),
        (2, 75, None, [1, 1, 1], 0, Some(0)),
        (3, 80, Some("alpha_entry"), [4, 4, 4], 0x7a, Some(0)),
    ] {
        if !multi && ordinal == 3 {
            break;
        }
        let source = SemanticSourceProvenanceV1::unavailable();
        let root = export.is_some();
        let statements = if root {
            vec![
                assignment(1, SemanticRvalueKindV1::Use(scalar(value))),
                assignment(2, SemanticRvalueKindV1::Use(scalar(7))),
                assignment(
                    3,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(place(1, U32)),
                        right: SemanticOperandV1::Copy(place(2, U32)),
                    },
                ),
            ]
        } else {
            vec![]
        };
        let mut blocks = match callee {
            Some(callee) => vec![
                block(
                    tag + 10,
                    statements,
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new(
                            SemanticFunctionIdV1::from_index(callee),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                place(0, UNIT),
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
                block(tag + 11, vec![], SemanticTerminatorKindV1::Return),
            ],
            None => vec![block(
                tag + 10,
                statements,
                SemanticTerminatorKindV1::Return,
            )],
        };
        if pipeline && ordinal == 0 {
            // Exact source closure includes retained dormant syntax. These typed
            // references keep both catalog declarations reachable in that closure,
            // while the helper's entry Return leaves their blocks unexecuted.
            let create = if multi { 4 } else { 3 };
            let call = |callable, arguments, local, ty, target| {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(callable),
                        arguments,
                        Some(SemanticCallDestinationV1::new(
                            place(local, ty),
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
            let zero = || {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    U64,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
                ))
            };
            blocks.push(block(
                tag + 11,
                vec![],
                call(
                    create,
                    vec![SemanticOperandV1::Copy(place(1, SCOPE_REF))],
                    2,
                    PIPE,
                    2,
                ),
            ));
            blocks.push(block(
                tag + 12,
                vec![],
                call(
                    create + 1,
                    vec![
                        SemanticOperandV1::Copy(place(3, PIPE_REF)),
                        zero(),
                        zero(),
                        scalar(0),
                    ],
                    0,
                    UNIT,
                    0,
                ),
            ));
        }
        let local_types = if root {
            vec![UNIT, U32, U32, U32]
        } else if pipeline && ordinal == 0 {
            vec![UNIT, SCOPE_REF, PIPE, PIPE_REF]
        } else {
            vec![UNIT]
        };
        let locals = local_types
            .into_iter()
            .enumerate()
            .map(|(local, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 1 + local as u8; 32]),
                    ty,
                    if local == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect();
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([layout; 32]),
            if root {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if root {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            0,
            vec![],
            abi_value(UNIT),
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            if root {
                SemanticFunctionRoleV1::KernelRoot
            } else {
                SemanticFunctionRoleV1::InternalHelper
            },
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        let dimensions = SemanticWorkgroupDimensionsV1::new(dimensions).unwrap();
        functions.push(match export {
            Some(export) => function.with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(export.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([binding; 32]),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                            .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
            )),
            None => function,
        });
    }
    let mut callables: Vec<_> = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    if pipeline {
        // The helper's retained dormant Calls close this callable roster and
        // exercise nonempty definitions without emitting pipeline bindings.
        callables.push(intrinsic(
            160,
            layout,
            &[SCOPE_REF],
            PIPE,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                scope: SCOPE,
                pipeline: PIPE,
                buffers: 2,
                elements: 64,
                prefetch_distance: 1,
            },
        ));
        callables.push(intrinsic(
            161,
            layout,
            &[PIPE_REF, U64, U64, U32],
            UNIT,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
                pipeline: PIPE,
                element: U32,
            },
        ));
    }
    let roots = if multi {
        vec![
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(3),
        ]
    } else {
        vec![SemanticFunctionIdV1::from_index(1)]
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([layout; 32])),
        types(pipeline),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        roots,
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source = Ssa::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        source.source_semantic().types().len(),
        if pipeline { 7 } else { 2 }
    );
    if pipeline {
        let helper = source
            .plan_for_function(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        assert_eq!(helper.plan().reverse_postorder().len(), 1);
        for block in [1, 2] {
            assert!(
                !helper
                    .plan()
                    .is_reachable(fe2o3_mir_model::SsaBlockIdV1::new(block))
            );
        }
    }
    source
}
fn launch(source: &Ssa, grid: u32, name: &str) -> Launch {
    let mut inputs = vec![ProductionSourceLaunchRootInputV1::new(
        name,
        [0xa1; 32],
        ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [grid, 1, 1]),
    )];
    if source.source_semantic().roots().len() == 2 {
        inputs.push(ProductionSourceLaunchRootInputV1::new(
            "logical_alpha",
            [0x7a; 32],
            ProductionSourceLaunchInputV1::new(3, Some([4, 4, 4]), [2, 1, 1]),
        ));
    }
    Launch::try_new(source.source_semantic(), &inputs).unwrap()
}
fn catalog(source: &Ssa, pipeline: bool, budget: &mut Budget<'_>) -> Catalog {
    let definitions = if pipeline {
        vec![Definition {
            key: 0,
            semantic_pipeline_type: PIPE.index(),
            semantic_payload_type: U32.index(),
            buffers: 2,
            elements: 64,
            prefetch_distance: 1,
            packed_bits: 32,
            source_size_bytes: 4,
            source_alignment_bytes: 4,
        }]
    } else {
        vec![]
    };
    let (catalog, storage) = Catalog::from_rows_with_budget(
        *source.source_semantic().semantic_sha256().as_bytes(),
        &definitions,
        &[],
        budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    catalog
}
fn with_fixture(
    multi: bool,
    pipeline: bool,
    layout: u8,
    body: impl FnOnce(&Materialized, &Catalog, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(PREFIX).unwrap();
    {
        let source = source(multi, pipeline, 3, layout);
        let launch = launch(&source, 1, "logical_zeta");
        let owner = Materialized::try_materialize_with_budget(
            source,
            launch,
            Limits::default(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(owner.executable_storage().retained_storage())
            .unwrap();
        budget
            .reserve_storage(owner.assert_origin_storage().payload_storage())
            .unwrap();
        let catalog = catalog(owner.semantic_ssa(), pipeline, &mut budget);
        body(&owner, &catalog, &mut budget);
    }
    budget.release_storage(budget.storage() - PREFIX).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}
fn with_checked(
    native: &Native,
    catalog: &Catalog,
    budget: &mut Budget<'_>,
    body: impl FnOnce(&CheckedKernelIrContractCatalogV1<'_, '_>, &mut Budget<'_>),
) {
    let floor = budget.storage();
    {
        let (inventory, storage) = CanonicalKirInventoryV1::derive(native, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (checked, storage) =
            check_kernel_ir_contract_catalog_v1(&inventory, catalog, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        body(&checked, budget);
    }
    budget.release_storage(budget.storage() - floor).unwrap();
}
fn success(
    source: &Ssa,
    launch: &Launch,
    native: &Native,
    catalog: &CheckedKernelIrContractCatalogV1<'_, '_>,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let before = budget.work();
    let storage = {
        let (relation, storage) =
            check(source, launch, native, catalog, Limits::default(), budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(storage.retained_storage(), std::mem::size_of_val(&relation));
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(std::ptr::eq(relation.source(), source));
        assert!(std::ptr::eq(relation.launch(), launch));
        assert!(std::ptr::eq(relation.native(), native));
        assert!(std::ptr::eq(relation.catalog(), catalog.catalog()));
        assert!(!relation.grants_authority());
        assert!(budget.work() > before);
        storage
    };
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn genuine_one_and_two_root_owners_replay_nonempty_catalogs_and_preserve_borrows() {
    for multi in [false, true] {
        for pipeline in [false, true] {
            for layout in [249, 250] {
                with_fixture(multi, pipeline, layout, |owner, catalog, budget| {
                    assert_eq!(
                        owner.executable().module().kernels.len(),
                        if multi { 2 } else { 1 }
                    );
                    assert_eq!(catalog.definitions().len(), usize::from(pipeline));
                    assert!(catalog.bindings().is_empty());
                    with_checked(owner.executable(), catalog, budget, |checked, budget| {
                        success(
                            owner.semantic_ssa(),
                            owner.source_launch(),
                            owner.executable(),
                            checked,
                            budget,
                        );
                        success(
                            owner.semantic_ssa(),
                            owner.source_launch(),
                            owner.executable(),
                            checked,
                            budget,
                        );
                    });
                });
            }
        }
    }
}

#[test]
fn equal_content_independent_source_native_and_catalog_owners_are_not_custody() {
    with_fixture(true, true, 250, |owner, catalog, budget| {
        let floor = budget.storage();
        {
            let source = source(true, true, 3, 250);
            let launch = launch(&source, 1, "logical_zeta");
            let (native, storage) = Native::from_module_ref_with_verification_budget_v12(
                owner.executable().module(),
                budget,
            )
            .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (copy, storage) =
                Catalog::decode_with_budget(catalog.canonical_bytes(), budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert!(!std::ptr::eq(&native, owner.executable()));
            assert!(!std::ptr::eq(&source, owner.semantic_ssa()));
            with_checked(&native, &copy, budget, |checked, budget| {
                success(&source, &launch, &native, checked, budget)
            });
        }
        budget.release_storage(budget.storage() - floor).unwrap();
    });
}

#[test]
fn graph_bound_but_wrong_complete_catalog_is_rejected() {
    with_fixture(true, true, 250, |owner, catalog, budget| {
        for change in 0..3 {
            let floor = budget.storage();
            {
                let mut digest = *owner
                    .semantic_ssa()
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes();
                let mut definitions = catalog.definitions().to_vec();
                match change {
                    0 => digest[0] ^= 1,
                    1 => definitions[0].elements += 1,
                    _ => definitions.clear(),
                }
                let (wrong, storage) =
                    Catalog::from_rows_with_budget(digest, &definitions, &[], budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                with_checked(owner.executable(), &wrong, budget, |checked, budget| {
                    let floor = budget.storage();
                    assert!(matches!(
                        check(
                            owner.semantic_ssa(),
                            owner.source_launch(),
                            owner.executable(),
                            checked,
                            Limits::default(),
                            budget
                        ),
                        Err(Error::Invalid("complete source catalog bytes"))
                    ));
                    assert_eq!(budget.storage(), floor);
                });
            }
            budget.release_storage(budget.storage() - floor).unwrap();
        }
    });
}

#[test]
fn valid_graph_mutations_reach_complete_native_byte_comparison() {
    with_fixture(true, false, 250, |owner, catalog, budget| {
        for mutation in 0..6 {
            let floor = budget.storage();
            {
                let mut module = owner.executable().module().clone();
                match mutation {
                    0 => {
                        let constant = module
                            .functions
                            .iter_mut()
                            .filter_map(|f| f.body.as_mut())
                            .flat_map(|b| &mut b.blocks)
                            .flat_map(|b| &mut b.operations)
                            .find_map(|op| match &mut op.kind {
                                OperationKind::Constant(Constant::U32(n)) => Some(n),
                                _ => None,
                            })
                            .unwrap();
                        *constant += 1;
                    }
                    1 => {
                        let pair = module
                            .functions
                            .iter_mut()
                            .filter_map(|f| f.body.as_mut())
                            .flat_map(|b| &mut b.blocks)
                            .flat_map(|b| &mut b.operations)
                            .find_map(|op| match &mut op.kind {
                                OperationKind::Binary { lhs, rhs, .. } => Some((lhs, rhs)),
                                _ => None,
                            })
                            .unwrap();
                        assert_ne!(*pair.0, *pair.1);
                        *pair.1 = *pair.0;
                    }
                    2 => {
                        let helper = module
                            .functions
                            .iter()
                            .find(|f| {
                                f.body.as_ref().is_some_and(|b| {
                                    b.blocks.iter().all(|b| {
                                        b.operations.iter().all(|op| {
                                            !matches!(op.kind, OperationKind::Call { .. })
                                        })
                                    })
                                })
                            })
                            .unwrap()
                            .id
                            .clone();
                        let callee = module
                            .functions
                            .iter_mut()
                            .filter_map(|f| f.body.as_mut())
                            .flat_map(|b| &mut b.blocks)
                            .flat_map(|b| &mut b.operations)
                            .find_map(|op| match &mut op.kind {
                                OperationKind::Call { callee, .. } if *callee != helper => {
                                    Some(callee)
                                }
                                _ => None,
                            })
                            .unwrap();
                        *callee = helper;
                    }
                    3 => {
                        let terminator = module
                            .functions
                            .iter_mut()
                            .filter_map(|f| f.body.as_mut())
                            .flat_map(|b| &mut b.blocks)
                            .find(|b| matches!(b.terminator, Some(Terminator::Return { .. })))
                            .unwrap();
                        terminator.terminator = Some(Terminator::Unreachable);
                    }
                    4 => {
                        module.kernels[0].id = fe2o3_kernel_ir::KernelId::new(format!(
                            "{}_changed",
                            module.kernels[0].id.as_str()
                        ));
                    }
                    _ => {
                        let first = module.kernels[0].entry.clone();
                        assert_ne!(first, module.kernels[1].entry);
                        module.kernels[0].entry = module.kernels[1].entry.clone();
                        module.kernels[1].entry = first;
                    }
                }
                let (native, storage) =
                    Native::from_module_ref_with_verification_budget_v12(&module, budget).unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                with_checked(&native, catalog, budget, |checked, budget| {
                    let floor = budget.storage();
                    assert!(matches!(
                        check(
                            owner.semantic_ssa(),
                            owner.source_launch(),
                            &native,
                            checked,
                            Limits::default(),
                            budget
                        ),
                        Err(Error::Invalid("complete native bytes"))
                    ));
                    assert_eq!(budget.storage(), floor);
                });
            }
            budget.release_storage(budget.storage() - floor).unwrap();
        }
    });
}

#[test]
fn changed_source_and_complete_root_roster_do_not_match_supplied_native() {
    with_fixture(true, false, 250, |owner, catalog, budget| {
        with_checked(owner.executable(), catalog, budget, |checked, budget| {
            for (multi, value) in [(true, 5), (false, 3)] {
                let source = source(multi, false, value, 250);
                let launch = launch(&source, 1, "logical_zeta");
                let floor = budget.storage();
                assert!(matches!(
                    check(
                        &source,
                        &launch,
                        owner.executable(),
                        checked,
                        Limits::default(),
                        budget
                    ),
                    Err(Error::Invalid("complete native bytes"))
                ));
                assert_eq!(budget.storage(), floor);
            }
            let different_source = source(true, false, 5, 250);
            assert!(matches!(
                check(
                    &different_source,
                    owner.source_launch(),
                    owner.executable(),
                    checked,
                    Limits::default(),
                    budget
                ),
                Err(Error::Source(
                    ProductionSemanticKirErrorV1::CorrespondenceMismatch
                ))
            ));
        });
    });
}

#[test]
fn source_launch_configuration_is_compared_where_materialized_not_authenticated() {
    with_fixture(false, false, 250, |owner, catalog, budget| {
        with_checked(owner.executable(), catalog, budget, |checked, budget| {
            let changed = launch(owner.semantic_ssa(), 2, "logical_zeta");
            assert!(matches!(
                check(
                    owner.semantic_ssa(),
                    &changed,
                    owner.executable(),
                    checked,
                    Limits::default(),
                    budget
                ),
                Err(Error::Invalid("complete native bytes"))
            ));
            let renamed = launch(owner.semantic_ssa(), 1, "different_diagnostic_name");
            success(
                owner.semantic_ssa(),
                &renamed,
                owner.executable(),
                checked,
                budget,
            );
        });
    });
}

#[test]
fn old_source_lowering_limit_refusal_is_preserved_without_full_engine_caps() {
    with_fixture(false, false, 250, |owner, catalog, budget| {
        with_checked(owner.executable(), catalog, budget, |checked, budget| {
            let floor = budget.storage();
            let before = budget.work();
            assert!(matches!(
                check(
                    owner.semantic_ssa(),
                    owner.source_launch(),
                    owner.executable(),
                    checked,
                    Limits::new_with_max_operations(0, 0, 0, 0),
                    budget
                ),
                Err(Error::Source(
                    ProductionSemanticKirErrorV1::ResourceLimit { .. }
                ))
            ));
            assert_eq!(budget.work() - before, 4);
            assert_eq!(budget.storage(), floor);
            success(
                owner.semantic_ssa(),
                owner.source_launch(),
                owner.executable(),
                checked,
                budget,
            );
        });
    });
}

#[test]
fn exact_header_denial_cleans_up_and_does_not_poison_later_success() {
    with_fixture(false, false, 250, |owner, catalog, budget| {
        with_checked(owner.executable(), catalog, budget, |checked, budget| {
            let floor = budget.storage();
            let header = std::mem::size_of::<Relation<'_, '_, '_, '_>>();
            let unrelated = budget.storage_limit() - floor - (header - 1);
            budget.reserve_storage(unrelated).unwrap();
            let before = budget.work();
            assert!(matches!(
                check(
                    owner.semantic_ssa(),
                    owner.source_launch(),
                    owner.executable(),
                    checked,
                    Limits::default(),
                    budget
                ),
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.work() - before, 4);
            assert_eq!(budget.storage(), floor + unrelated);
            assert_eq!(budget.failed_storage(), Some(STORAGE + 1));
            budget.release_storage(unrelated).unwrap();
            let exact = budget.storage_limit() - floor - header;
            budget.reserve_storage(exact).unwrap();
            let before = budget.work();
            assert!(matches!(
                check(
                    owner.semantic_ssa(),
                    owner.source_launch(),
                    owner.executable(),
                    checked,
                    Limits::new_with_max_operations(0, 0, 0, 0),
                    budget
                ),
                Err(Error::Source(
                    ProductionSemanticKirErrorV1::ResourceLimit { .. }
                ))
            ));
            assert_eq!(budget.work() - before, 4);
            assert_eq!(budget.storage(), floor + exact);
            budget.release_storage(exact).unwrap();
            success(
                owner.semantic_ssa(),
                owner.source_launch(),
                owner.executable(),
                checked,
                budget,
            );
            assert_eq!(budget.failed_storage(), Some(STORAGE + 1));
        });
    });
}

#[test]
fn owner_refusal_and_work_three_four_boundaries_precede_source_replay() {
    for remaining in [3, 4] {
        with_fixture(false, false, 250, |owner, catalog, budget| {
            let floor = budget.storage();
            {
                let (other, storage) = Native::from_module_ref_with_verification_budget_v12(
                    owner.executable().module(),
                    budget,
                )
                .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                with_checked(&other, catalog, budget, |checked, budget| {
                    budget
                        .charge_work(WORK - budget.work() - remaining)
                        .unwrap();
                    let before = budget.work();
                    let floor = budget.storage();
                    let result = check(
                        owner.semantic_ssa(),
                        owner.source_launch(),
                        owner.executable(),
                        checked,
                        Limits::new_with_max_operations(0, 0, 0, 0),
                        budget,
                    );
                    if remaining == 3 {
                        assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                        assert_eq!(budget.work(), before);
                        budget.charge_work(1).unwrap(); // A denied 4 does not poison a smaller charge.
                    } else {
                        assert!(matches!(result, Err(Error::Invalid("catalog graph owner"))));
                        assert_eq!(budget.work() - before, 4);
                    }
                    assert_eq!(budget.storage(), floor);
                });
            }
            budget.release_storage(budget.storage() - floor).unwrap();
        });
    }
}

fn emitted_pipeline_source() -> Ssa {
    let base = source(false, true, 3, 250);
    let semantic = base.source_semantic();
    let original = &semantic.functions()[1];
    assert_eq!(semantic.functions().len(), 3);
    assert!(matches!(
        semantic.callables()[3],
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { .. },
            ..
        }
    ));
    let mut callables = semantic.callables().to_vec();
    assert_eq!(callables.len(), 5);
    callables.push(intrinsic(
        162,
        250,
        &[],
        SCOPE,
        SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent { scope: SCOPE },
    ));
    let call = |callable, arguments, local, ty, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callable),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(local, ty),
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
    let borrow = SemanticStatementV1::new(
        original.source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(2, SCOPE_REF),
            SemanticRvalueV1::new(
                SCOPE_REF,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(1, SCOPE),
                },
            ),
        )),
    );
    let blocks = vec![
        block(80, vec![], call(5, vec![], 1, SCOPE, 1)),
        block(
            81,
            vec![borrow],
            call(
                3,
                vec![SemanticOperandV1::Copy(place(2, SCOPE_REF))],
                3,
                PIPE,
                2,
            ),
        ),
        // Keep both genuine helpers in the exact source root closure after
        // replacing this root's arithmetic body with the emitted Create path.
        block(82, vec![], call(2, vec![], 0, UNIT, 3)),
        block(83, vec![], SemanticTerminatorKindV1::Return),
    ];
    let locals = [UNIT, SCOPE, SCOPE_REF, PIPE]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                original.locals()[index].identity(),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                original.source(),
            )
        })
        .collect();
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut functions = semantic.functions().to_vec();
    functions[1] = function;
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    Ssa::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn emitted_pipeline_storage_binding_replays_and_graph_valid_omission_refuses() {
    use fe2o3_kernel_ir::{KernelIrPipelineStorageBindingV1 as Binding, WorkgroupMemoryExtent};
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(PREFIX).unwrap();
    {
        let source = emitted_pipeline_source();
        let launch = launch(&source, 1, "logical_zeta");
        let owner = Materialized::try_materialize_with_budget(
            source,
            launch,
            Limits::default(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(owner.executable_storage().retained_storage())
            .unwrap();
        budget
            .reserve_storage(owner.assert_origin_storage().payload_storage())
            .unwrap();
        let mut bindings = Vec::new();
        for (function, body) in owner.executable().module().functions.iter().enumerate() {
            for (block, body) in body.body.as_ref().unwrap().blocks.iter().enumerate() {
                for (operation, op) in body.operations.iter().enumerate() {
                    if let OperationKind::WorkgroupMemory(memory) = &op.kind {
                        assert_eq!(memory.extent, WorkgroupMemoryExtent::Static(128));
                        assert_eq!(
                            memory.element,
                            fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)
                        );
                        assert_eq!(op.results.len(), 1);
                        bindings.push(Binding {
                            function: function as u32,
                            storage: op.results[0].id.0,
                            key: 0,
                            block: block as u32,
                            operation: operation as u32,
                        });
                    }
                }
            }
        }
        assert_eq!(
            bindings.len(),
            1,
            "actual source pipeline create must emit storage"
        );
        let definition = Definition {
            key: 0,
            semantic_pipeline_type: PIPE.index(),
            semantic_payload_type: U32.index(),
            buffers: 2,
            elements: 64,
            prefetch_distance: 1,
            packed_bits: 32,
            source_size_bytes: 4,
            source_alignment_bytes: 4,
        };
        let (catalog, storage) = Catalog::from_rows_with_budget(
            *owner
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            &[definition],
            &bindings,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(catalog.bindings().len(), 1);
        with_checked(
            owner.executable(),
            &catalog,
            &mut budget,
            |checked, budget| {
                success(
                    owner.semantic_ssa(),
                    owner.source_launch(),
                    owner.executable(),
                    checked,
                    budget,
                );
            },
        );
        // Allocation-only source construction has no event marker requiring this
        // row during graph binding. Complete source replay must still detect it.
        let (omitted, storage) = Catalog::from_rows_with_budget(
            *owner
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            &[definition],
            &[],
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        with_checked(
            owner.executable(),
            &omitted,
            &mut budget,
            |checked, budget| {
                assert_eq!(checked.marker_count(), 0);
                let floor = budget.storage();
                assert!(matches!(
                    check(
                        owner.semantic_ssa(),
                        owner.source_launch(),
                        owner.executable(),
                        checked,
                        Limits::default(),
                        budget
                    ),
                    Err(Error::Invalid("complete source catalog bytes"))
                ));
                assert_eq!(budget.storage(), floor);
            },
        );
    }
    budget.release_storage(budget.storage() - PREFIX).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}
