use fe2o3_kernel_ir::{BlockId, LaunchDomain, WorkgroupSize, verify_module};
use fe2o3_lower_mir_kernel::{
    ProductionSemanticKirErrorV1, ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
    ProductionSemanticKirResourceV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1};

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

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn admitted(effectful_statement: bool, unsupported_terminator: bool) -> AdmittedInertSemanticMirV1 {
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
    let statements = vec![SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        if effectful_statement {
            SemanticStatementKindV1::Deinitialize(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], type_id).unwrap(),
            )
        } else {
            SemanticStatementKindV1::Nop
        },
    )];
    let block0 = block(10, statements, SemanticTerminatorKindV1::Return);
    let target = SemanticBlockIdV1::from_index(0);
    let block1_terminator = if unsupported_terminator {
        SemanticTerminatorKindV1::FalseEdge {
            real_target: SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::FalseEdgeReal, target),
            imaginary_target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::FalseEdgeImaginary,
                target,
            ),
        }
    } else {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            target,
        ))
    };
    let block1 = block(11, vec![], block1_terminator);
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
    let function = function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"semantic_kir_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5)),
        contract,
    ));
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
}

fn semantic_owner(
    effectful_statement: bool,
    unsupported_terminator: bool,
) -> ProductionSemanticMirOwnerV1 {
    ProductionSemanticMirOwnerV1::try_new(
        admitted(effectful_statement, unsupported_terminator),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn exact_nonzero_entry_lowers_to_verified_kir_with_correspondence() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();

    lowered.verify_equivalence().unwrap();
    verify_module(lowered.module()).unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks[0].id, BlockId(1));
    assert_eq!(body.blocks[1].id, BlockId(0));
    assert_eq!(
        lowered.correspondence().blocks()[0]
            .semantic_block()
            .index(),
        1
    );
    assert_eq!(
        lowered.correspondence().blocks()[1].source_statement_count(),
        1
    );
    assert_eq!(
        lowered.module().kernels[0].domain,
        LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
        }
    );
    assert_eq!(
        lowered.module().kernels[0].workgroup_size,
        Some(WorkgroupSize::new(64, 1, 1))
    );
    assert!(!lowered.grants_artifact_or_launch_authority());
}

#[test]
fn independent_lowerings_are_deterministic() {
    let first = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let second = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(first.module(), second.module());
    assert_eq!(first.correspondence(), second.correspondence());
}

#[test]
fn unsupported_statement_and_terminator_fail_closed() {
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            semantic_owner(true, false),
            ProductionSemanticKirLimitsV1::default(),
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            block: Some(0),
            statement: Some(0),
            ..
        })
    ));
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            semantic_owner(false, true),
            ProductionSemanticKirLimitsV1::default(),
        ),
        Err(ProductionSemanticKirErrorV1::Unsupported {
            block: Some(1),
            statement: None,
            ..
        })
    ));
}

#[test]
fn lowering_limits_are_enforced_before_materialization() {
    assert!(matches!(
        ProductionSemanticKirOwnerV1::try_lower(
            semantic_owner(false, false),
            ProductionSemanticKirLimitsV1::new(1, 1, 1),
        ),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Blocks,
            actual: 2,
            limit: 1,
        })
    ));
}
