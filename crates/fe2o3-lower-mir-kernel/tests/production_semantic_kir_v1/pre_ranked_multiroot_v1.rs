use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1, LaunchDomain,
    LaunchExtent,
};
use fe2o3_lower_mir_kernel::{
    ProductionMaterializedRankedModuleReceiptV1, ProductionPreRankedKirOwnerV1,
    ProductionRankedSemanticProjectionRootV1, ProductionSourceExecutionLayoutV1,
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1,
};
use fe2o3_pliron::{
    ProductionRankedOperationV1, ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 11;
const EXPORTS: [&str; 2] = ["zeta_entry", "alpha_entry"];

// Both roots reach helper 0; root 1 additionally reaches helper 2. Function
// ordinals are deliberately not the root ordinals and export order is not lexical.
fn shared_helper_owner() -> ProductionSemanticMirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut functions = Vec::new();
    for (ordinal, tag, symbol, workgroup, binding, callee) in [
        (0_u32, 60_u8, None, [1, 1, 1], 0_u8, None),
        (1_u32, 70_u8, Some(EXPORTS[0]), [64, 1, 1], 0xa1_u8, Some(2)),
        (2_u32, 75_u8, None, [1, 1, 1], 0_u8, Some(0)),
        (3_u32, 80_u8, Some(EXPORTS[1]), [4, 4, 4], 0x7a_u8, Some(0)),
    ] {
        let root = symbol.is_some();
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
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
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let blocks = match callee {
            Some(callee) => vec![
                block(
                    tag + 10,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new(
                            SemanticFunctionIdV1::from_index(callee),
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
                        .unwrap(),
                    ),
                ),
                block(tag + 11, vec![], SemanticTerminatorKindV1::Return),
            ],
            None => vec![block(tag + 10, vec![], SemanticTerminatorKindV1::Return)],
        };
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            if root {
                SemanticFunctionRoleV1::KernelRoot
            } else {
                SemanticFunctionRoleV1::InternalHelper
            },
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            source,
            abi,
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(tag + 1)),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            )],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        assert_eq!(functions.len(), usize::try_from(ordinal).unwrap());
        functions.push(match symbol {
            Some(symbol) => {
                let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
                function.with_kernel_entry(SemanticKernelEntryV1::new(
                    SemanticLinkSymbolV1::new(symbol.as_bytes().to_vec()).unwrap(),
                    SemanticKernelBindingIdentityV1::from_sha256(bytes(binding)),
                    SemanticKernelSourceContractV1::new(
                        Some(
                            SemanticKernelLaunchBoundsV1::new(
                                Some(dimensions),
                                Some(dimensions),
                                None,
                            )
                            .unwrap(),
                        ),
                        None,
                        None,
                    )
                    .unwrap(),
                ))
            }
            None => function,
        });
    }
    let semantic = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(3),
        ],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn materialize_shared(
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> ProductionPreRankedKirOwnerV1 {
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        shared_helper_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[
            ProductionSourceLaunchRootInputV1::new(
                "logical_zeta",
                bytes(0xa1),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            ),
            ProductionSourceLaunchRootInputV1::new(
                "logical_alpha",
                bytes(0x7a),
                ProductionSourceLaunchInputV1::new(3, Some([4, 4, 4]), [2, 1, 1]),
            ),
        ],
    )
    .unwrap();
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

fn ranked_root(
    selected: u32,
    export: &str,
    rank: u8,
    layout: ProductionSourceExecutionLayoutV1,
) -> ProductionRankedSemanticProjectionRootV1 {
    // The source helpers perform no memory effects. This is an actual checked
    // ranked candidate, not a fabricated generic-check or translation receipt.
    let kernel = ProductionRankedKernelV1::new(
        export,
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
    let construction =
        ProductionConstructionV1::ranked_kernel("shared_helper_ranked", kernel).unwrap();
    let lowering =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .unwrap();
    ProductionRankedSemanticProjectionRootV1::new(
        SemanticFunctionIdV1::from_index(selected),
        rank,
        lowering,
        format!("func @{export} {{ kernel.return }}"),
        vec![],
        vec![],
    )
}

fn exact_ranked_roots(
    owner: &ProductionPreRankedKirOwnerV1,
) -> Vec<ProductionRankedSemanticProjectionRootV1> {
    owner
        .source_launch()
        .roots()
        .iter()
        .zip(EXPORTS)
        .map(|(root, export)| {
            ranked_root(
                root.selected_root().index(),
                export,
                root.source_rank(),
                root.layout(),
            )
        })
        .collect()
}

fn one_callee(function: &fe2o3_kernel_ir::Function) -> &fe2o3_kernel_ir::FunctionId {
    let mut callees = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, .. } => Some(callee),
            _ => None,
        });
    let callee = callees
        .next()
        .expect("the genuine source call must remain in executable KIR");
    assert!(callees.next().is_none());
    callee
}

#[test]
fn pre_ranked_shared_helpers_preserve_sparse_roots_layouts_and_connected_attachment() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let materialized = materialize_shared(&mut budget);
    let storage = materialized.executable_storage();
    let origin_storage = materialized.assert_origin_storage();
    assert_eq!(budget.storage(), FLOOR);
    budget
        .reserve_storage(storage.retained_storage() + origin_storage.payload_storage())
        .unwrap();
    let semantic = materialized.semantic_ssa().source_semantic();
    assert_eq!(
        semantic.roots(),
        &[
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(3),
        ]
    );
    for (caller, expected) in [(1_usize, 2_u32), (2, 0), (3, 0)] {
        let SemanticTerminatorKindV1::Call(call) =
            semantic.functions()[caller].blocks()[0].terminator().kind()
        else {
            panic!("fixture must retain the real shared-helper call graph");
        };
        assert_eq!(call.callee().index(), expected);
    }
    let module = materialized.executable().module();
    assert_eq!(module.functions.len(), 4);
    assert_eq!(
        module
            .kernels
            .iter()
            .map(|kernel| kernel.id.as_str())
            .collect::<Vec<_>>(),
        EXPORTS
    );
    let function = |id: &fe2o3_kernel_ir::FunctionId| {
        module
            .functions
            .iter()
            .find(|function| &function.id == id)
            .unwrap()
    };
    let zeta = function(&module.kernels[0].entry);
    let alpha = function(&module.kernels[1].entry);
    let intermediate = function(one_callee(zeta));
    assert_eq!(one_callee(intermediate), one_callee(alpha));
    let shared = function(one_callee(alpha));
    assert!(
        shared
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .all(|block| block.operations.is_empty())
    );
    assert_eq!(
        module.kernels[0].domain,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64)
        }
    );
    assert_eq!(
        module.kernels[1].domain,
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Static(4),
            z: LaunchExtent::Static(4),
        }
    );
    assert_eq!(
        materialized.source_launch().roots()[0]
            .layout()
            .global_extents(),
        [64, 1, 1]
    );
    assert_eq!(
        materialized.source_launch().roots()[1]
            .layout()
            .global_extents(),
        [8, 4, 4]
    );
    let functions = module.functions.as_ptr();
    let canonical = materialized
        .executable()
        .canonical()
        .canonical_bytes()
        .as_ptr();
    let identity = *materialized.executable().canonical().identity();
    let roots = exact_ranked_roots(&materialized);
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            materialized,
            roots,
        )
        .unwrap();
    let attached =
        ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
    let connected = attached.pre_ranked_executable().unwrap();
    assert_eq!(connected.module().functions.as_ptr(), functions);
    assert_eq!(connected.canonical().canonical_bytes().as_ptr(), canonical);
    assert_eq!(connected.canonical().identity(), &identity);
    assert!(std::ptr::eq(attached.module(), connected.module()));
    assert_eq!(attached.mir_pliron_translation_validations().len(), 2);
    let shared_records = attached
        .correspondence()
        .lowered_functions()
        .iter()
        .filter(|record| record.semantic_function().index() == 0)
        .collect::<Vec<_>>();
    assert_eq!(shared_records.len(), 2);
    assert_eq!(
        shared_records
            .iter()
            .map(|record| record.correspondence_owner().index())
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        shared_records[0].kernel_ir_function(),
        shared_records[1].kernel_ir_function()
    );
    drop(shared_records);
    // This remains the explicit legacy reconstruction audit, after attachment.
    attached.verify_equivalence().unwrap();
    drop(attached);
    budget
        .release_storage(storage.retained_storage() + origin_storage.payload_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn pre_ranked_shared_roster_rejects_missing_reordered_duplicate_and_substituted_rows() {
    for case in [
        "missing",
        "extra",
        "reordered",
        "duplicate",
        "helper-root",
        "rank",
        "layout",
        "export",
        "late-export",
        "late-duplicate-name",
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let materialized = materialize_shared(&mut budget);
        let storage = materialized.executable_storage();
        let origin_storage = materialized.assert_origin_storage();
        budget
            .reserve_storage(storage.retained_storage() + origin_storage.payload_storage())
            .unwrap();
        let source = materialized.source_launch().roots();
        let first = source[0].layout();
        let second = source[1].layout();
        let mut roots = exact_ranked_roots(&materialized);
        let expected = match case {
            "missing" => {
                roots.pop();
                "ranked projection roster is not a complete semantic root bijection"
            }
            "extra" => {
                roots.push(ranked_root(1, EXPORTS[0], 1, first));
                "ranked projection roster is not a complete semantic root bijection"
            }
            "reordered" => {
                roots.swap(0, 1);
                "ranked projection roster has a duplicate, missing, or invalid root identity"
            }
            "duplicate" => {
                roots[1] = ranked_root(1, EXPORTS[0], 1, first);
                "ranked projection roster has a duplicate, missing, or invalid root identity"
            }
            "helper-root" => {
                roots[1] = ranked_root(2, EXPORTS[1], 3, second);
                "ranked projection roster has a duplicate, missing, or invalid root identity"
            }
            "rank" => {
                roots[1] = ranked_root(3, EXPORTS[1], 2, second);
                "ranked projection roster has a duplicate, missing, or invalid root identity"
            }
            "layout" => {
                roots[1] = ranked_root(3, EXPORTS[1], 3, first);
                "ranked execution layout changed after executable materialization"
            }
            "export" => {
                roots[0] = ranked_root(1, EXPORTS[1], 1, first);
                "ranked projection receipt function identity changed"
            }
            "late-export" => {
                roots[1] = ranked_root(3, "substituted_entry", 3, second);
                "ranked projection receipt function identity changed"
            }
            "late-duplicate-name" => {
                roots[1] = ranked_root(3, EXPORTS[0], 3, second);
                "ranked projection roster has a duplicate, missing, or invalid root identity"
            }
            _ => unreachable!(),
        };
        let result = ProductionMaterializedRankedModuleReceiptV1::
            from_unvalidated_projection_roster_candidate(materialized, roots);
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("hostile {case} roster was accepted"),
        };
        assert!(
            matches!(&error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if *detail == expected),
            "wrong diagnostic for {case}: {error}"
        );
        budget
            .release_storage(storage.retained_storage() + origin_storage.payload_storage())
            .unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
