use dialect_mir::pliron::{
    MirProductionLocatorErrorV1, MirProductionPlironLimitsV1, MirProductionPlironResourceV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirErrorV1, ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
    ShellLimits,
};

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn unit_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4)),
        SemanticLayoutIdentityV1::from_sha256(bytes(4)),
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

fn semantic_function() -> SemanticFunctionDeclV1 {
    let type_id = SemanticTypeIdV1::from_index(0);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let block0 = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(10)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop,
        )],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    let target = SemanticBlockIdV1::from_index(0);
    let block1 = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(11)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::FalseEdge {
                real_target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::FalseEdgeReal,
                    target,
                ),
                imaginary_target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::FalseEdgeImaginary,
                    target,
                ),
            },
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(3)),
            type_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(1),
        vec![block0, block1],
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let launch =
        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let contract = SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
    function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_owner_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5)),
        contract,
    ))
}

fn admitted() -> AdmittedInertSemanticMirV1 {
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        vec![semantic_function()],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn exact_owner_retains_nonzero_entry_duplicate_targets_and_edge_roles() {
    let owner =
        ProductionSemanticMirOwnerV1::try_new(admitted(), ProductionSemanticMirLimitsV1::default())
            .unwrap();

    owner.verify_equivalence().unwrap();
    assert_eq!(owner.locator().functions().len(), 1);
    let function = &owner.locator().functions()[0];
    assert_eq!(function.entry_block_id(), SemanticBlockIdV1::from_index(1));
    let arcs = function.blocks()[1].terminator().successors();
    assert_eq!(arcs.len(), 2);
    assert_eq!(arcs[0].role(), SemanticEdgeRoleV1::FalseEdgeReal);
    assert_eq!(arcs[1].role(), SemanticEdgeRoleV1::FalseEdgeImaginary);
    assert_eq!(arcs[0].target(), arcs[1].target());
    assert_eq!(arcs[0].target(), SemanticBlockIdV1::from_index(0));
    assert!(matches!(
        owner
            .resolve_statement(
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(0),
                0,
            )
            .unwrap()
            .kind(),
        SemanticStatementKindV1::Nop
    ));
    assert!(
        owner
            .resolve_terminator(
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(1),
            )
            .is_some()
    );
    assert!(!owner.grants_artifact_or_launch_authority());
}

#[test]
fn independent_owners_have_identical_pointer_free_locators() {
    let first =
        ProductionSemanticMirOwnerV1::try_new(admitted(), ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let second =
        ProductionSemanticMirOwnerV1::try_new(admitted(), ProductionSemanticMirLimitsV1::default())
            .unwrap();

    assert_eq!(first.locator(), second.locator());
    assert_eq!(
        first.semantic().semantic_sha256(),
        second.semantic().semantic_sha256()
    );
}

#[test]
fn semantic_tree_work_is_rejected_before_an_owner_exists() {
    let limits = ProductionSemanticMirLimitsV1::new(
        ShellLimits::default(),
        MirProductionPlironLimitsV1::new(11).unwrap(),
    );
    assert_eq!(
        ProductionSemanticMirOwnerV1::try_new(admitted(), limits).unwrap_err(),
        ProductionSemanticMirErrorV1::Locator(
            MirProductionLocatorErrorV1::MiddleEndResourceLimitExceeded {
                resource: MirProductionPlironResourceV1::TreeWork,
                actual: 12,
                limit: 11,
            }
        )
    );
}

#[test]
fn resolver_rejects_unknown_locators() {
    let owner =
        ProductionSemanticMirOwnerV1::try_new(admitted(), ProductionSemanticMirLimitsV1::default())
            .unwrap();
    assert!(
        owner
            .resolve_function(SemanticFunctionIdV1::from_index(1))
            .is_none()
    );
    assert!(
        owner
            .resolve_block(
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(2),
            )
            .is_none()
    );
    assert!(
        owner
            .resolve_statement(
                SemanticFunctionIdV1::from_index(0),
                SemanticBlockIdV1::from_index(0),
                1,
            )
            .is_none()
    );
}
