use super::*;
use crate::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, CheckedSourcePipelineCatalogV1 as View,
    SourcePipelineCatalogCallbackErrorV1 as Error,
    with_checked_source_pipeline_catalog_v1 as check,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1 as Inventory, check_kernel_ir_contract_catalog_v1 as bind,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiPointeeInfoV1, SemanticAbiRegularAttributesV1, SemanticAbiValueAttributesV1,
    SemanticMemoryLoadV1, SemanticMemoryStoreV1, SemanticPointerTypeV1,
    SemanticTypeAbiPropertiesV1,
};
use std::{cell::Cell, mem::size_of};

const W: usize = 1_000_000_000;
const S: usize = 128 * 1024 * 1024;
const PRIOR: usize = 17;
const SIBLING: usize = 113;

fn materialize(source: ProductionSemanticMirOwnerV1) -> ProductionPreRankedKirOwnerV1 {
    let semantic = source.semantic();
    let entry = semantic.functions()[0].kernel_entry().unwrap();
    let name = std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        semantic,
        &[crate::ProductionSourceLaunchRootInputV1::new(
            name,
            *entry.kernel_binding_identity().as_bytes(),
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn ranked_root(
    original: &ProductionPreRankedKirOwnerV1,
) -> ProductionRankedSemanticProjectionRootV1 {
    let name = original.executable().module().kernels[0].id.as_str();
    let source = original.source_launch().roots()[0];
    let layout = source.layout();
    // The generic legacy helper uses grid identity 1. Materialized source
    // custody requires the identity derived from this actual kernel binding.
    let kernel = ProductionRankedKernelV1::new(
        name,
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
        ProductionConstructionV1::ranked_kernel(&format!("{name}_ranked"), kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    ProductionRankedSemanticProjectionRootV1::new(
        source.selected_root(),
        source.source_rank(),
        lowering,
        format!("func @{name} {{\n}}\n"),
        vec![],
        vec![],
    )
}

fn direct_from(source: ProductionSemanticMirOwnerV1) -> ProductionSemanticKirOwnerV1 {
    let original = materialize(source);
    let root = ranked_root(&original);
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            original,
            vec![root],
        )
        .unwrap();
    ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap()
}

fn direct() -> ProductionSemanticKirOwnerV1 {
    direct_from(noop_semantic_owner(&["catalog_root"]))
}

fn scalar_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            bits: 64,
            signed: false,
        }),
    )
}

fn erased() -> ProductionUnitLocalErasedSourceOwnerV1 {
    let seed = helper_closure_semantic_owner();
    let source = seed.semantic();
    let helper = &source.functions()[1];
    assert_eq!(helper.locals()[0].identity().as_bytes(), &[217; 32]);
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let scalar = SemanticTypeIdV1::from_index(1);
    let place = |i| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(i), vec![], scalar).unwrap();
    let local = |tag| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            scalar,
            SemanticLocalRoleV1::Temporary,
            provenance,
        )
    };
    let statements = vec![
        SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(1),
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    scalar,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(11, 8).unwrap()),
                )),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ),
        SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(2),
                SemanticRvalueV1::new(
                    scalar,
                    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                        place(1),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
            )),
        ),
    ];
    let fresh = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        provenance,
        helper.abi().clone(),
        vec![helper.locals()[0].clone(), local(218), local(219)],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([212; 32]),
                provenance,
                statements,
                SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new(
        source.target(),
        vec![source.types()[0].clone(), scalar_type()],
        vec![],
        vec![],
        vec![],
        vec![source.functions()[0].clone(), fresh],
        source.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let original = materialize(source);
    let roots = vec![ranked_root(&original)];
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let input = ProductionUnitLocalErasedSourceOwnerV1::input_storage_floor_v1(
        &original,
        &roots,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(input).unwrap();
    let (erased, _) =
        ProductionUnitLocalErasedSourceOwnerV1::try_produce_v1(original, roots, &mut budget)
            .unwrap();
    assert_eq!(
        (erased.deleted_function_count(), erased.deleted_call_count()),
        (1, 1)
    );
    assert_ne!(
        erased
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes(),
        erased.erased().canonical().canonical_bytes()
    );
    erased
}

fn floor(anchor: Anchor<'_>) -> usize {
    SIBLING
        + match anchor {
            Anchor::Direct(source) => source.pre_ranked_retained_analysis_storage_v1().unwrap(),
            Anchor::Erased(source) => source.retained_storage_floor_v1(),
        }
}

fn run<T, E>(
    anchor: Anchor<'_>,
    w: usize,
    s: usize,
    callback: impl for<'a> FnOnce(View<'a>, &mut Budget<'_>) -> Result<T, E>,
) -> (Result<T, Error<E>>, usize, usize) {
    let mut work = Work::new(w);
    let mut budget = Budget::new(&mut work, s);
    budget.charge_work(PRIOR).unwrap();
    let live = floor(anchor);
    budget.reserve_storage(live).unwrap();
    let result = check(anchor, &mut budget, callback);
    assert_eq!(budget.storage(), live);
    (result, budget.work(), budget.peak_storage())
}

fn inspect(view: View<'_>, budget: &mut Budget<'_>) -> Result<usize, Resource> {
    assert!(!view.grants_artifact_or_launch_authority());
    let catalog = view.catalog(budget)?;
    assert!(catalog.definitions().is_empty() && catalog.bindings().is_empty());
    assert!(
        !view
            .original_executable(budget)?
            .module()
            .kernels
            .is_empty()
    );
    Ok(catalog.canonical_bytes().len())
}

#[test]
fn source_catalog_callback_genuine_direct_and_unitlocal_erased_are_deterministic() {
    let direct = direct();
    let erased = erased();
    for anchor in [Anchor::Direct(&direct), Anchor::Erased(&erased)] {
        let (first, w, p) = run(anchor, W, S, inspect);
        let (second, again_w, again_p) = run(anchor, W, S, inspect);
        assert!(first.unwrap() > 0);
        assert!(second.unwrap() > 0);
        assert_eq!((w, p), (again_w, again_p));
    }
}

#[test]
fn source_catalog_callback_refuses_legacy_and_corrupted_source_without_callback() {
    let receipt = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        noop_semantic_owner(&["catalog_root"]), noop_ranked_roster(&["catalog_root"])).unwrap();
    let legacy = ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(SIBLING).unwrap();
    assert!(matches!(
        check(
            Anchor::Direct(&legacy),
            &mut budget,
            |_, _| Ok::<_, Resource>(())
        ),
        Err(Error::MissingConnectedSource)
    ));
    assert_eq!(budget.storage(), SIBLING);
    let mut source = direct();
    source.correspondence.semantic_sha256[0] ^= 1;
    let called = Cell::new(0);
    let (result, _, _) = run(Anchor::Direct(&source), W, S, |_, _| {
        called.set(called.get() + 1);
        Ok::<_, Resource>(())
    });
    assert!(matches!(result, Err(Error::Source(_))));
    assert_eq!(called.get(), 0);
}

#[test]
fn source_catalog_callback_rejects_a_valid_but_unrelated_erased_graph() {
    let mut source = erased();
    let mut candidate = source.erased().module().clone();
    candidate.kernels[0].id = fe2o3_kernel_ir::KernelId::new("unrelated_export");
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let (donor, receipt) =
        Graph::from_module_ref_with_verification_budget_v12(&candidate, &mut budget).unwrap();
    source.erased = donor;
    // Hostile test-only substitution gets its full extra graph receipt; a
    // stale original owner's receipt is not used to underpay the donor.
    let live = floor(Anchor::Erased(&source)) + receipt.retained_storage();
    budget.reserve_storage(live).unwrap();
    let called = Cell::new(false);
    let result = check(Anchor::Erased(&source), &mut budget, |_, _| {
        called.set(true);
        Ok::<_, Resource>(())
    });
    assert!(matches!(result, Err(Error::Source(_))));
    assert!(!called.get());
    assert_eq!(budget.storage(), live);
}

#[test]
fn source_catalog_callback_error_sources_keep_all_typed_causes() {
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryErrorV1, KernelIrContractCatalogBindingErrorV1,
    };
    use std::error::Error as _;
    type E = Error<Resource>;
    assert!(
        E::Resource(Resource::Accounting)
            .source()
            .unwrap()
            .is::<Resource>()
    );
    assert!(
        E::Source(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            .source()
            .unwrap()
            .is::<ProductionSemanticKirErrorV1>()
    );
    assert!(
        E::Inventory(CanonicalKirInventoryErrorV1::InconsistentOwner)
            .source()
            .unwrap()
            .is::<CanonicalKirInventoryErrorV1>()
    );
    assert!(
        E::Catalog(ProductionSourceOutputCatalogErrorV1::Invalid("fixture"))
            .source()
            .unwrap()
            .is::<ProductionSourceOutputCatalogErrorV1>()
    );
    assert!(
        E::Binding(KernelIrContractCatalogBindingErrorV1::Invalid("fixture"))
            .source()
            .unwrap()
            .is::<KernelIrContractCatalogBindingErrorV1>()
    );
    assert!(
        E::Callback(Resource::Arithmetic)
            .source()
            .unwrap()
            .is::<Resource>()
    );
    assert!(E::UnsupportedNonemptyCatalog.source().is_none());
    assert!(E::Panicked.source().is_none());
}

#[test]
fn source_catalog_callback_preserves_typed_callback_errors_and_panic_cleanup() {
    #[derive(Debug)]
    struct Marker;
    impl std::fmt::Display for Marker {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("marker")
        }
    }
    impl std::error::Error for Marker {}
    let source = direct();
    let (result, _, _) = run(Anchor::Direct(&source), W, S, |_, _| Err::<(), _>(Marker));
    let error = result.unwrap_err();
    assert!(std::error::Error::source(&error).unwrap().is::<Marker>());
    let (result, _, _) = run(
        Anchor::Direct(&source),
        W,
        S,
        |_, _| -> Result<(), Marker> { panic!("callback panic") },
    );
    assert!(matches!(result, Err(Error::Panicked)));
    assert!(run(Anchor::Direct(&source), W, S, inspect).0.is_ok());
    struct Payload(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("panic payload destructor");
        }
    }
    let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let live = floor(Anchor::Direct(&source));
    budget.reserve_storage(live).unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = check(
            Anchor::Direct(&source),
            &mut budget,
            |_, _| -> Result<(), Resource> { std::panic::panic_any(Payload(drops.clone())) },
        );
    }));
    assert!(unwind.is_err());
    assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), live);
    check(Anchor::Direct(&source), &mut budget, inspect).unwrap();
}

fn nonempty_source() -> ProductionSemanticKirOwnerV1 {
    // Retained, unreachable calls keep a genuine admitted pipeline contract.
    // SSA prunes their blocks, so N has no pipeline allocation or binding.
    let seed = noop_semantic_owner(&["catalog_root"]);
    let semantic = seed.semantic();
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let scope = SemanticTypeIdV1::from_index(2);
    let scope_ref = SemanticTypeIdV1::from_index(3);
    let pipeline = SemanticTypeIdV1::from_index(4);
    let pipeline_ref = SemanticTypeIdV1::from_index(5);
    let mut types = vec![semantic.types()[0].clone(), scalar_type()];
    // Type IDs remain positional. Keep their identities strictly ordered after
    // the original Unit (1) and appended U64 (201), without reindexing operands.
    for (tag, pointee) in [(220, scope), (222, pipeline)] {
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
    let value = |ty| {
        let mode = if ty == unit || ty == scope || ty == pipeline {
            SemanticAbiPassModeV1::Ignore
        } else {
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        false,
                        None,
                        ty == scope_ref || ty == pipeline_ref,
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
    };
    let intrinsic = |tag, inputs: Vec<SemanticTypeIdV1>, output, operation| {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            semantic.target_layout_identity(),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            inputs.into_iter().map(value).collect(),
            value(output),
        )
        .unwrap();
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
        }
    };
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        intrinsic(
            160,
            vec![scope_ref],
            pipeline,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                scope,
                pipeline,
                buffers: 2,
                elements: 64,
                prefetch_distance: 1,
            },
        ),
        intrinsic(
            161,
            vec![pipeline_ref, scalar, scalar],
            scalar,
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead {
                pipeline,
                element: scalar,
            },
        ),
        intrinsic(
            162,
            vec![],
            scope,
            SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent { scope },
        ),
    ];
    let root = &semantic.functions()[0];
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let local = |tag, ty| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            SemanticLocalRoleV1::Temporary,
            provenance,
        )
    };
    let borrow = |destination, reference, referent, ty| {
        SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination, reference),
                SemanticRvalueV1::new(
                    reference,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(referent, ty),
                    },
                ),
            )),
        )
    };
    let call = |callee, arguments, destination, ty, next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(next),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            provenance,
            statements,
            SemanticTerminatorV1::new(provenance, terminator),
        )
        .unwrap()
    };
    let zero = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
        ))
    };
    assert_eq!(root.locals()[0].identity().as_bytes(), &[26; 32]);
    assert_eq!(root.blocks()[0].identity().as_bytes(), &[60; 32]);
    let root = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        vec![
            root.locals()[0].clone(),
            local(27, scope),
            local(28, scope_ref),
            local(29, pipeline),
            local(30, pipeline_ref),
            local(31, scalar),
        ],
        root.entry(),
        vec![
            root.blocks()[0].clone(),
            block(61, vec![], call(3, vec![], 1, scope, 2)),
            block(
                62,
                vec![borrow(2, scope_ref, 1, scope)],
                call(
                    1,
                    vec![SemanticOperandV1::Move(place(2, scope_ref))],
                    3,
                    pipeline,
                    3,
                ),
            ),
            block(
                63,
                vec![borrow(4, pipeline_ref, 3, pipeline)],
                call(
                    2,
                    vec![
                        SemanticOperandV1::Move(place(4, pipeline_ref)),
                        zero(),
                        zero(),
                    ],
                    5,
                    scalar,
                    4,
                ),
            ),
            block(64, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source = direct_from(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
    );
    assert_eq!(source.semantic_ssa.summary().input_blocks(), 5);
    assert_eq!(source.semantic_ssa.summary().reachable_blocks(), 1);
    assert_eq!(source.semantic_ssa.summary().pruned_blocks(), 4);
    source
}

#[test]
fn source_catalog_callback_refuses_real_nonempty_source_contract_instead_of_dropping_it() {
    let source = nonempty_source();
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget
        .reserve_storage(floor(Anchor::Direct(&source)))
        .unwrap();
    let (inventory, receipt) =
        Inventory::derive(source.pre_ranked_executable().unwrap(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (catalog, receipt) = source_catalog_from_live_v1(
        source.semantic_ssa.source_semantic(),
        SourceCatalogCorrespondenceV1(&source.correspondence),
        &inventory,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(catalog.definitions().len(), 1);
    assert!(catalog.bindings().is_empty());
    let (binding, receipt) = bind(&inventory, &catalog, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let called = Cell::new(false);
    let (result, _, _) = run(Anchor::Direct(&source), W, S, |_, _| {
        called.set(true);
        Ok::<_, Resource>(())
    });
    assert!(matches!(result, Err(Error::UnsupportedNonemptyCatalog)));
    assert!(!called.get());
    drop(binding);
    drop(catalog);
    drop(inventory);
}

// Independent successful constituent composition, not the wrapper's counters
// learned from a failing run. No producer callback or wrapper helper is used.
fn oracle_direct(source: &ProductionSemanticKirOwnerV1) -> (usize, usize, usize, usize, usize) {
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.charge_work(PRIOR).unwrap();
    budget
        .reserve_storage(floor(Anchor::Direct(source)))
        .unwrap();
    let guard = 2 * size_of::<usize>()
        + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
        + size_of::<Result<usize, Error<Resource>>>();
    budget.reserve_storage(guard).unwrap();
    budget.charge_work(5).unwrap();
    budget
        .reserve_storage(size_of::<(
            &AdmittedInertSemanticMirV1,
            &SemanticKirCorrespondenceV1,
            &Graph,
            Option<&Graph>,
        )>())
        .unwrap();
    source
        .verify_equivalence_with_budget_v1(&mut budget)
        .unwrap();
    let (inventory, receipt) =
        Inventory::derive(source.pre_ranked_executable().unwrap(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let catalog_floor = budget.storage();
    let (catalog, receipt) = source_catalog_from_live_v1(
        source.semantic_ssa.source_semantic(),
        SourceCatalogCorrespondenceV1(&source.correspondence),
        &inventory,
        &mut budget,
    )
    .unwrap();
    assert!(catalog.definitions().is_empty() && catalog.bindings().is_empty());
    assert_eq!(catalog.canonical_bytes().len(), 56);
    // The empty codec reserves its complete receipt after validation and before
    // encoding. The source constructor still holds its four Vec headers and
    // exact function roster. No work follows encoding before it returns.
    let catalog_cut_p = catalog_floor
        + 4 * size_of::<Vec<()>>()
        + source.correspondence.lowered_functions().len()
            * size_of::<SourceCatalogFunctionV1<'_>>()
        + receipt.retained_storage();
    let catalog_encoding_work = 2 * 56 + b"FE2O3/NATIVE-KIR-IMPORT-CONTRACT-CATALOG/V1\0".len() + 8;
    let catalog_cut_work = budget.work() - catalog_encoding_work;
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (binding, receipt) = bind(&inventory, &catalog, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    budget.charge_work(2).unwrap();
    let before_query = budget.work();
    // One borrowed view plus one separately retained original-ledger/slot/floor
    // record. Identity is repr(transparent) usize in the pinned budget API.
    budget
        .reserve_storage(
            size_of::<View<'_>>()
                + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
                + 2 * size_of::<usize>(),
        )
        .unwrap();
    budget.charge_work(1).unwrap();
    budget.charge_work(1).unwrap();
    let result = (
        budget.work(),
        budget.peak_storage(),
        before_query,
        catalog_cut_work,
        catalog_cut_p,
    );
    drop(binding);
    drop(catalog);
    drop(inventory);
    result
}

#[test]
fn source_catalog_callback_exact_work_storage_and_query_prefix_boundaries() {
    let source = direct();
    let anchor = Anchor::Direct(&source);
    let (w, p, before_query, catalog_cut_work, catalog_cut_p) = oracle_direct(&source);
    let (result, actual_w, actual_p) = run(anchor, w, p, inspect);
    assert!(result.is_ok());
    assert_eq!((actual_w, actual_p), (w, p));
    let (result, used, _) = run(anchor, w - 1, p, inspect);
    let Err(Error::Callback(Resource::Work(error))) = result else {
        panic!("second query must fail");
    };
    assert_eq!((error.actual(), error.limit()), (w, w - 1));
    assert_eq!(used, w - 1);
    let called = Cell::new(false);
    let (result, used, _) = run(anchor, before_query, p, |view, budget| {
        called.set(true);
        inspect(view, budget)
    });
    assert!(called.get());
    let Err(Error::Callback(Resource::Work(error))) = result else {
        panic!("first query must fail");
    };
    assert_eq!(
        (error.actual(), error.limit()),
        (before_query + 1, before_query)
    );
    assert_eq!(used, before_query);
    assert_eq!(
        p, catalog_cut_p,
        "fixture must exercise the actual interior global peak"
    );
    let (result, used, _) = run(anchor, w, p - 1, inspect);
    let Err(Error::Catalog(ProductionSourceOutputCatalogErrorV1::Codec(
        fe2o3_kernel_ir::KernelIrContractCatalogErrorV1::Resource(Resource::Storage(error)),
    ))) = result
    else {
        panic!("interior catalog allocation must be the first denial");
    };
    assert_eq!((error.actual(), error.limit()), (p, p - 1));
    assert_eq!(used, catalog_cut_work);
    assert!(run(anchor, w, p, inspect).0.is_ok());
}

#[test]
fn source_catalog_callback_rejects_same_size_foreign_ledger_and_drops_rejected_result() {
    let source = direct();
    let anchor = Anchor::Direct(&source);
    let mut first = Work::new(W);
    let mut second = Work::new(W);
    let mut budget = Budget::new(&mut first, S);
    let mut foreign = Budget::new(&mut second, S);
    let live = floor(anchor);
    budget.reserve_storage(live).unwrap();
    foreign.reserve_storage(live).unwrap();
    let foreign_id = foreign.work_ledger_identity_v1();
    let result = check(anchor, &mut budget, |_, budget| {
        Ok::<_, Resource>(std::mem::replace(budget, foreign))
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(budget.work_ledger_identity_v1() == foreign_id);
    assert_eq!(budget.storage(), live);
    drop(result);
    drop(budget);
    let mut restored = Budget::new(&mut first, S);
    restored.reserve_storage(live).unwrap();
    check(anchor, &mut restored, inspect).unwrap();
    assert_eq!(restored.storage(), live);
    struct Bomb<'a>(&'a Cell<usize>);
    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected callback result destructor");
        }
    }
    let drops = Cell::new(0);
    let result = check(anchor, &mut restored, |_, budget| {
        budget.release_storage(budget.storage() - live).unwrap();
        Ok::<_, Resource>(Bomb(&drops))
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(drops.get(), 1);
    assert_eq!(restored.storage(), live);
}

#[test]
fn source_catalog_callback_queries_refuse_foreign_ledger_without_replacement() {
    let direct = direct();
    let erased = erased();
    for anchor in [Anchor::Direct(&direct), Anchor::Erased(&erased)] {
        for catalog_query in [false, true] {
            let mut foreign_work = Work::new(W);
            let mut foreign = Budget::new(&mut foreign_work, S);
            foreign.charge_work(PRIOR + 3).unwrap();
            foreign.reserve_storage(SIBLING).unwrap();
            let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
            let (result, _, _) = run(anchor, W, S, |view, _| {
                if catalog_query {
                    view.catalog(&mut foreign).map(|_| ())
                } else {
                    view.original_executable(&mut foreign).map(|_| ())
                }
            });
            assert!(matches!(result, Err(Error::Callback(Resource::Accounting))));
            assert_eq!(
                (foreign.work(), foreign.storage(), foreign.peak_storage()),
                before
            );
            assert!(run(anchor, W, S, inspect).0.is_ok());
        }
    }
}

#[test]
fn source_catalog_callback_queries_refuse_same_ledger_in_a_different_budget_slot() {
    let direct = direct();
    let erased = erased();
    for anchor in [Anchor::Direct(&direct), Anchor::Erased(&erased)] {
        for catalog_query in [false, true] {
            let mut placeholder_work = Work::new(W);
            let placeholder = Budget::new(&mut placeholder_work, S);
            let mut primary_work = Work::new(W);
            let mut budget = Budget::new(&mut primary_work, S);
            let live = floor(anchor);
            budget.reserve_storage(live).unwrap();
            let result = check(anchor, &mut budget, |view, budget| {
                let mut moved = std::mem::replace(budget, placeholder);
                let original_id = moved.work_ledger_identity_v1();
                let before = (moved.work(), moved.storage(), moved.peak_storage());
                let rejected = if catalog_query {
                    view.catalog(&mut moved).map(|_| ())
                } else {
                    view.original_executable(&mut moved).map(|_| ())
                };
                assert!(matches!(rejected, Err(Resource::Accounting)));
                assert_eq!(
                    (moved.work(), moved.storage(), moved.peak_storage()),
                    before
                );
                let placeholder = std::mem::replace(budget, moved);
                assert_eq!(placeholder.work(), 0);
                assert_eq!(placeholder.storage(), 0);
                assert!(budget.work_ledger_identity_v1() == original_id);
                drop(placeholder);
                inspect(view, budget).map(|_| ())
            });
            assert!(result.is_ok());
            assert_eq!(budget.storage(), live);
            assert!(run(anchor, W, S, inspect).0.is_ok());
        }
    }
}

#[test]
fn source_catalog_callback_full_live_floor_is_required_for_queries_and_return() {
    let direct = direct();
    let erased = erased();
    for anchor in [Anchor::Direct(&direct), Anchor::Erased(&erased)] {
        for mode in 0..3 {
            let outer = floor(anchor);
            let guard = 2 * size_of::<usize>()
                + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
                + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
                + size_of::<Result<(), Error<Resource>>>();
            let (result, _, _) = run(anchor, W, S, |view, budget| {
                let before_work = budget.work();
                let callback_floor = budget.storage();
                assert!(callback_floor - 1 > outer + guard);
                budget.release_storage(1).unwrap();
                let query = match mode {
                    0 => view.catalog(budget).map(|_| ()),
                    1 => view.original_executable(budget).map(|_| ()),
                    _ => Ok(()),
                };
                if mode < 2 {
                    assert!(matches!(query, Err(Resource::Accounting)));
                }
                assert_eq!(budget.work(), before_work);
                assert_eq!(budget.storage(), callback_floor - 1);
                // Even swallowing a failed query or making no query cannot
                // bypass the independent full-floor return check.
                Ok::<_, Resource>(())
            });
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            assert!(run(anchor, W, S, inspect).0.is_ok());
        }
    }
}

#[test]
fn source_catalog_callback_rejected_result_and_error_destructors_keep_postcheck_cleanup() {
    struct Bomb<'a>(&'a Cell<usize>);
    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("full-floor rejected result destructor");
        }
    }
    let direct = direct();
    let erased = erased();
    for anchor in [Anchor::Direct(&direct), Anchor::Erased(&erased)] {
        for success in [false, true] {
            let drops = Cell::new(0);
            let (result, _, _) = run(anchor, W, S, |_, budget| {
                budget.release_storage(1).unwrap();
                if success {
                    Ok::<_, Bomb<'_>>(Bomb(&drops))
                } else {
                    Err::<Bomb<'_>, _>(Bomb(&drops))
                }
            });
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            assert_eq!(drops.get(), 1);
            assert!(run(anchor, W, S, inspect).0.is_ok());
        }
    }
}
