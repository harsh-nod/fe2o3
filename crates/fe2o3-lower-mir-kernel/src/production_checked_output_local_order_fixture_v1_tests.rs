// Genuine admitted semantic source/SSA/N fixture, not ordinary Rust evidence.
use super::*;

pub(super) fn local_order_source(
    dead: bool,
    duplicate: bool,
    launch_x: u32,
) -> ProductionPreRankedKirOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let arguments = (0..4)
        .map(|_| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                U32,
                SemanticAbiPassModeV1::Direct(attributes),
            ))
        })
        .collect();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        4,
        arguments,
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 4])
    .unwrap();
    let local_types = [
        (UNIT, SemanticLocalRoleV1::Return),
        (U32, SemanticLocalRoleV1::Argument(0)),
        (U32, SemanticLocalRoleV1::Argument(1)),
        (U32, SemanticLocalRoleV1::Argument(2)),
        (U32, SemanticLocalRoleV1::Argument(3)),
        (U32, SemanticLocalRoleV1::Temporary),
        (U32, SemanticLocalRoleV1::Temporary),
        (U32, SemanticLocalRoleV1::Temporary),
        (U64, SemanticLocalRoleV1::Temporary),
        (BOOL, SemanticLocalRoleV1::Temporary),
    ];
    let binary = |dest, op, a, b| {
        assignment(
            dest,
            U32,
            SemanticRvalueKindV1::Binary {
                operation: op,
                left: value(a, U32),
                right: value(b, U32),
            },
        )
    };
    let mut statements = vec![
        binary(5, SemanticBinaryOpV1::BitXor, 1, 2),
        binary(
            6,
            SemanticBinaryOpV1::BitOr,
            3,
            if duplicate { 3 } else { 4 },
        ),
        binary(7, SemanticBinaryOpV1::BitAnd, 5, 6),
    ];
    let blocks = if dead {
        vec![block(31, statements, SemanticTerminatorKindV1::Return)]
    } else {
        statements.extend([
            assignment(
                8,
                U64,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: value(7, U32),
                },
            ),
            assignment(
                9,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: value(8, U64),
                    right: constant(U64, 8, 8),
                },
            ),
        ]);
        vec![
            block(
                31,
                statements,
                SemanticTerminatorKindV1::Assert {
                    condition: value(9, BOOL),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: constant(U64, 8, 8),
                        index: value(8, U64),
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(32, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let dimensions = SemanticWorkgroupDimensionsV1::new([launch_x, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
        source,
        abi,
        local_types
            .into_iter()
            .enumerate()
            .map(|(ordinal, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([40 + ordinal as u8; 32]),
                    ty,
                    role,
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"private_array_relation".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([30; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let mut fixture_types = types();
    fixture_types.truncate(4);
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        fixture_types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([launch_x, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

pub(super) fn local_prefix(
    dead: bool,
    duplicate: bool,
    launch_x: u32,
) -> (Final6, usize, crate::SourceU32LocalOrderRequestV1) {
    use fe2o3_kernel_ir::{
        BinaryOp, CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
        CanonicalKirOperationCoordinateV1,
    };
    let input = fixture6_from_source(
        Profile::Gfx942,
        local_order_source(dead, duplicate, launch_x),
    );
    let neutral = input.receipt.materialized.executable();
    let block = &neutral.module().functions[0].body.as_ref().unwrap().blocks[0];
    let operations: Vec<_> = block
        .operations
        .iter()
        .enumerate()
        .filter_map(|(ordinal, operation)| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitXor | BinaryOp::BitOr | BinaryOp::BitAnd,
                    ..
                }
            )
            .then_some(CanonicalKirOperationCoordinateV1 {
                block: CanonicalKirBlockCoordinateV1 {
                    function: CanonicalKirFunctionCoordinateV1(0),
                    block: 0,
                },
                operation: u32::try_from(ordinal).unwrap(),
            })
        })
        .collect();
    let request = crate::SourceU32LocalOrderRequestV1 {
        expected_source: *neutral.canonical().identity(),
        operations: operations.try_into().unwrap(),
        preference: fe2o3_kernel_opt::U32LocalOrderPreferenceV1::SourceOrder,
    };
    let floor = input.floor;
    (admit6(input, WORK, STORAGE).0.unwrap(), floor, request)
}
