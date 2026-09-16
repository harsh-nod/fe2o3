use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_lower_mir_kernel::{
    ProductionMaterializedRankedModuleReceiptV1, ProductionPreRankedKirOwnerV1,
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1,
};
use fe2o3_pliron::{
    ProductionRankedOperationV1, ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
};

// These limits govern canonical graph admission, not source/planner/translation allocations.
const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;

#[test]
fn inert_v29_cannot_enter_any_executable_lowering_path() {
    use fe2o3_lower_mir_kernel::{
        ProductionPreRankedKirErrorV1, ProductionRankedSemanticProjectionModuleReceiptV1,
    };
    let source = || {
        let b = admitted(false, false);
        let request = InertSemanticMirRequestV1::new_with_callables(
            b.target(),
            b.types().to_vec(),
            b.allocations().to_vec(),
            b.statics().to_vec(),
            b.vtables().to_vec(),
            b.functions().to_vec(),
            b.callables().to_vec(),
            b.roots().to_vec(),
        )
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(
            request
                .admit_exact_v29(SemanticMirLimitsV1::default())
                .unwrap(),
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap()
    };
    let reject = |error| {
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail == "execution capabilities require checked canonical KIR materialization")
        )
    };
    let launch_for = |owner: &ProductionSemanticMirOwnerV1| {
        ProductionSourceLaunchRosterV1::try_new(
            owner.semantic(),
            &[ProductionSourceLaunchRootInputV1::new(
                "logical_root",
                bytes(5),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
            )],
        )
        .unwrap()
    };
    let limits = ProductionSemanticKirLimitsV1::default();
    reject(ProductionSemanticKirOwnerV1::try_lower(source(), limits).unwrap_err());
    let owner = source();
    let launch = launch_for(&owner);
    let receipt = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        owner, vec![ranked_root(launch.roots()[0].layout(), 1, true)]).unwrap();
    reject(
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(receipt, limits)
            .unwrap_err(),
    );
    let owner = source();
    let launch = launch_for(&owner);
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(19).unwrap();
    let error = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        limits,
        &mut budget,
    )
    .unwrap_err();
    let ProductionPreRankedKirErrorV1::Lowering(error) = error else {
        panic!("wrong rejection layer")
    };
    reject(error);
    assert_eq!(budget.storage(), 19);
    assert!(budget.failed_storage().is_none());
}

#[test]
fn inert_v29_nested_role_without_calls_survives_custody_but_not_lowering() {
    let id = SemanticTypeIdV1::from_index;
    let zst = |tag, fields: Vec<SemanticTypeIdV1>| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![0; fields.len()], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        )
    };
    let context = zst(71, vec![id(1); 5]).with_rust_type_kind(SemanticRustTypeKindV1::Execution(
        SemanticExecutionRoleV29::KernelContext,
    ));
    let types = vec![unit_type(), zst(70, vec![]), context, zst(72, vec![id(2)])];
    let local = SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(63)),
        id(3),
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    );
    let owner = owner_from_parts(
        types,
        vec![return_local(), local],
        0,
        vec![block(73, vec![], SemanticTerminatorKindV1::Return)],
        b"semantic_kir_test",
    );
    let encoded = owner.semantic().canonical_encoding().to_vec();
    let hash = owner.semantic().semantic_sha256();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
        &encoded,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let owner =
        ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    ssa.verify_replay().unwrap();
    let owner = ssa.into_source_owner().unwrap();
    assert_eq!(
        owner.semantic().wire_version(),
        SemanticMirWireVersionV1::V29
    );
    assert_eq!(owner.semantic().semantic_sha256(), hash);
    assert_eq!(owner.semantic().canonical_encoding(), encoded);
    assert_eq!(
        owner.semantic().types()[2].rust_type_kind(),
        SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::KernelContext)
    );
    assert_eq!(owner.semantic().callables().len(), 1);
    assert!(
        matches!(ProductionSemanticKirOwnerV1::try_lower(owner, ProductionSemanticKirLimitsV1::default()),
        Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
        if detail == "execution capabilities require checked canonical KIR materialization")
    );
}

fn materialize(
    owner: ProductionSemanticMirOwnerV1,
    binding: [u8; 32],
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> ProductionPreRankedKirOwnerV1 {
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "logical_root",
            binding,
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        )],
    )
    .unwrap();
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(ssa, launch, limits, budget).unwrap()
}

fn ranked_root(
    layout: fe2o3_lower_mir_kernel::ProductionSourceExecutionLayoutV1,
    rank: u8,
    include_layout: bool,
) -> fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionRootV1 {
    let operations = if include_layout {
        vec![ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: layout.grid_identity(),
            global_extents: layout.global_extents(),
            workgroup_extents: layout.workgroup_extents(),
            subgroup_size: layout.subgroup_size(),
            full_physical_workgroups: layout.full_physical_workgroups(),
        }]
    } else {
        vec![]
    };
    let kernel = ProductionRankedKernelV1::new(
        "semantic_kir_test",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("pre_ranked_attachment", kernel).unwrap();
    let lowering =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .unwrap();
    fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionRootV1::new(
        SemanticFunctionIdV1::from_index(0),
        rank,
        lowering,
        "func @semantic_kir_test { kernel.return }".to_owned(),
        vec![],
        vec![],
    )
}

#[test]
fn materialized_graph_and_canonical_buffers_move_through_attachment_unchanged() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(19).unwrap();
    let materialized = materialize(
        semantic_owner(false, false),
        bytes(5),
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    );
    assert_eq!(budget.storage(), 19);
    let storage = materialized.executable_storage();
    let origin_storage = materialized.assert_origin_storage();
    budget
        .reserve_storage(storage.retained_storage() + origin_storage.payload_storage())
        .unwrap();
    let graph = materialized.executable();
    let identity = *graph.canonical().identity();
    let functions = graph.module().functions.as_ptr();
    let canonical_buffer = graph.canonical().canonical_bytes().as_ptr();
    let source_sha = *materialized.source_launch().semantic_sha256();
    let layout = materialized.source_launch().roots()[0].layout();
    assert!(!materialized.grants_artifact_or_launch_authority());
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            materialized,
            vec![ranked_root(layout, 1, true)],
        )
        .unwrap();
    assert_eq!(receipt.root_count(), 1);
    let attached =
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
    let retained = attached.pre_ranked_executable().unwrap();
    assert_eq!(retained.canonical().identity(), &identity);
    assert_eq!(retained.module().functions.as_ptr(), functions);
    assert_eq!(
        retained.canonical().canonical_bytes().as_ptr(),
        canonical_buffer
    );
    assert!(std::ptr::eq(attached.module(), retained.module()));
    assert_eq!(attached.pre_ranked_executable_storage(), Some(storage));
    assert_eq!(
        attached.pre_ranked_assert_origin_storage(),
        Some(origin_storage)
    );
    let origins = attached.pre_ranked_assert_origins().unwrap();
    assert!(std::ptr::eq(origins.executable(), retained));
    assert!(
        origins
            .is_materialized_block(
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(0),
                &mut budget,
            )
            .unwrap()
    );
    assert_eq!(
        attached.source_launch_roster().unwrap().semantic_sha256(),
        &source_sha
    );
    assert_eq!(attached.mir_pliron_translation_validations().len(), 1);
    // This is deliberately a later reconstruction audit, not receipt attachment.
    attached.verify_equivalence().unwrap();
    drop(attached);
    budget
        .release_storage(storage.retained_storage() + origin_storage.payload_storage())
        .unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn attachment_rejects_missing_layout_or_changed_rank_before_translation() {
    for (rank, include_layout, expected) in [
        (2, true, "duplicate, missing, or invalid root identity"),
        (1, false, "ranked execution layout changed"),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let materialized = materialize(
            semantic_owner(false, false),
            bytes(5),
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        );
        let payload = materialized.executable_storage().retained_storage()
            + materialized.assert_origin_storage().payload_storage();
        budget.reserve_storage(payload).unwrap();
        let layout = materialized.source_launch().roots()[0].layout();
        let result = ProductionMaterializedRankedModuleReceiptV1::
            from_unvalidated_projection_roster_candidate(materialized, vec![ranked_root(layout, rank, include_layout)]);
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("changed source layout admitted"),
        };
        assert!(error.to_string().contains(expected), "{error}");
        budget.release_storage(payload).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn materialization_rejects_substituted_source_roster_identity() {
    let owner = semantic_owner(false, false);
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let other = semantic_owner(true, false);
    let launch = ProductionSourceLaunchRosterV1::try_new(
        other.semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "logical_root",
            bytes(5),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    assert!(matches!(
        ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget
        ),
        Err(
            fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Lowering(
                ProductionSemanticKirErrorV1::CorrespondenceMismatch
            )
        )
    ));
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn pre_ranked_canonical_failure_restores_caller_floor() {
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic_owner(false, false),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "logical_root",
            bytes(5),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 7);
    budget.reserve_storage(7).unwrap();
    let result = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(fe2o3_lower_mir_kernel::ProductionPreRankedKirErrorV1::Canonical(_))
    ));
    assert_eq!(budget.storage(), 7);
    assert!(budget.failed_storage().is_some());
}

#[test]
fn explicit_function_limit_above_default_survives_connected_materialization() {
    const FUNCTION_COUNT: usize = 1_025;
    let owner = large_defined_helper_closure_owner_v1(true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let materialized = materialize(
        owner,
        bytes(240),
        ProductionSemanticKirLimitsV1::new(FUNCTION_COUNT, 2 * FUNCTION_COUNT, 0),
        &mut budget,
    );
    let storage = materialized.executable_storage();
    let origin_storage = materialized.assert_origin_storage();
    budget
        .reserve_storage(storage.retained_storage() + origin_storage.payload_storage())
        .unwrap();
    assert_eq!(
        materialized.executable().module().functions.len(),
        FUNCTION_COUNT
    );
    materialized.semantic_ssa().verify_replay().unwrap();
    drop(materialized);
    budget
        .release_storage(storage.retained_storage() + origin_storage.payload_storage())
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn attachment_implementation_does_not_rematerialize_or_replay_lowering() {
    let source = include_str!("../../src/production_pre_ranked_v1.rs");
    let attachment = source
        .split("pub fn try_attach_materialized_ranked_checks(")
        .nth(1)
        .unwrap()
        .split("/// The exact pre-ranked graph")
        .next()
        .unwrap();
    assert!(!attachment.contains("lower_module("));
    assert!(!attachment.contains("verify_equivalence("));
    assert!(!attachment.contains("from_module"));
    assert!(attachment.contains("executable.module()"));
}

fn indexed_identity_v1(index: u32, domain: u8) -> [u8; 32] {
    let mut identity = [domain; 32];
    identity[..4].copy_from_slice(&index.to_be_bytes());
    identity
}

fn large_defined_helper_closure_owner_v1(required_workgroup: bool) -> ProductionSemanticMirOwnerV1 {
    const FUNCTION_COUNT: usize = 1_025;
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut functions = Vec::with_capacity(FUNCTION_COUNT);
    for index in 0..FUNCTION_COUNT {
        let index_u32 = u32::try_from(index).unwrap();
        let role = if index == 0 {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(indexed_identity_v1(index_u32, 230)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            if index == 0 {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if index == 0 {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let blocks = if index + 1 == FUNCTION_COUNT {
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(indexed_identity_v1(index_u32, 231)),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ]
        } else {
            let call = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(index_u32 + 1),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    local_place(0, unit),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(indexed_identity_v1(index_u32, 232)),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
                )
                .unwrap(),
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(indexed_identity_v1(index_u32, 233)),
                    source,
                    vec![],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ]
        };
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(indexed_identity_v1(index_u32, 234)),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256(indexed_identity_v1(index_u32, 235)),
            SemanticMonomorphizationIdentityV1::from_sha256(indexed_identity_v1(index_u32, 236)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(indexed_identity_v1(
                index_u32, 237,
            )),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(indexed_identity_v1(
                index_u32, 238,
            )),
            source,
            abi,
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(indexed_identity_v1(index_u32, 239)),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            )],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        functions.push(if index == 0 {
            function.with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"large_defined_helper_closure_v1".to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(240)),
                SemanticKernelSourceContractV1::new(
                    required_workgroup.then(|| {
                        let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
                        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                            .unwrap()
                    }),
                    None,
                    None,
                )
                .unwrap(),
            ))
        } else {
            function
        });
    }
    let callables = (0..FUNCTION_COUNT)
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(
                u32::try_from(index).unwrap(),
            ))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn optional_source_capture_stays_separately_reserved_through_legacy_attachment() {
    const FLOOR: usize = 23;
    let mut source = ProductionSemanticSsaOwnerV1::try_new(
        semantic_owner(false, false),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let identity = source.identity();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        source.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "logical_root",
            bytes(5),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = source
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let capture_payload = capture.retained_storage();
    assert!(capture_payload > 0);
    budget.reserve_storage(capture_payload).unwrap();
    let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        source,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR + capture_payload);
    assert_eq!(materialized.semantic_ssa().identity(), identity);
    assert_eq!(
        materialized.semantic_ssa().occurrence_storage(),
        Some(capture)
    );
    assert!(materialized.semantic_ssa().occurrences_v1().is_some());
    let graph_storage = materialized.executable_storage();
    let origin_storage = materialized.assert_origin_storage();
    let materialized_payload = graph_storage.retained_storage() + origin_storage.payload_storage();
    budget.reserve_storage(materialized_payload).unwrap();
    let layout = materialized.source_launch().roots()[0].layout();
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            materialized,
            vec![ranked_root(layout, 1, true)],
        )
        .unwrap();
    let attached =
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
    assert_eq!(attached.semantic_ssa_identity(), identity);
    assert_eq!(
        attached.pre_ranked_executable_storage(),
        Some(graph_storage)
    );
    assert_eq!(
        attached.pre_ranked_assert_origin_storage(),
        Some(origin_storage)
    );
    assert_eq!(
        budget.storage(),
        FLOOR + capture_payload + materialized_payload
    );
    let root = SemanticFunctionIdV1::from_index(0);
    assert!(
        attached
            .pre_ranked_assert_origins()
            .unwrap()
            .is_materialized_block(root, root, SemanticBlockIdV1::from_index(0), &mut budget)
            .unwrap()
    );
    attached.verify_equivalence().unwrap();
    drop(attached);
    budget.release_storage(materialized_payload).unwrap();
    assert_eq!(budget.storage(), FLOOR + capture_payload);
    budget.release_storage(capture_payload).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn existing_legacy_constructors_do_not_synthesize_connected_origin_custody() {
    let standalone = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let semantic = semantic_owner(false, false);
    let launch = ProductionSourceLaunchRosterV1::try_new(
        semantic.semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "logical_root",
            bytes(5),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        )],
    )
    .unwrap();
    let roots = vec![ranked_root(launch.roots()[0].layout(), 1, true)];
    let receipt = fe2o3_lower_mir_kernel::ProductionRankedSemanticProjectionModuleReceiptV1::
        from_unvalidated_projection_roster_candidate(semantic, roots)
        .unwrap();
    let ranked = ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    for owner in [standalone, ranked] {
        assert!(owner.pre_ranked_executable().is_none());
        assert!(owner.pre_ranked_executable_storage().is_none());
        assert!(owner.pre_ranked_assert_origins().is_none());
        assert!(owner.pre_ranked_assert_origin_storage().is_none());
        assert!(owner.source_launch_roster().is_none());
        owner.verify_equivalence().unwrap();
    }
}
