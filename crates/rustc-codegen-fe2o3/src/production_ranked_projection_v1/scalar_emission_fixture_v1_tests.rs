use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn value(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}
fn field(index: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap(),
    )
}
fn constant(bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
    ))
}
fn assignment(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, kind),
    )
    .unwrap()
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let scalar = |tag, bytes, maximum, kind| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, (bytes * 8) as u16, bytes),
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(kind),
        )
    };
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
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
        ),
        scalar(
            2,
            4,
            u128::from(u32::MAX),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        scalar(3, 1, 1, SemanticScalarTypeV1::Bool),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(
                    vec![0, 4],
                    vec![SemanticPaddingV1::new(5, 3).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, BOOL]).unwrap()),
        ),
    ]
}

/// Component fixture, not rustc/source-driver qualification. All source owners,
/// SSA plans, typed certificates and emitted N are produced by the real APIs.
pub(super) fn source(seed: u8) -> (ProductionSemanticSsaOwnerV1, ProductionSourceLaunchRosterV1) {
    let source = SemanticSourceProvenanceV1::unavailable();
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([seed; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            U32,
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let blocks = vec![
        block(
            31,
            vec![assignment(2, U32, SemanticRvalueKindV1::Use(constant(0)))],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            32,
            vec![assignment(
                3,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: value(2, U32),
                    right: value(1, U32),
                },
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: value(3, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            33,
            vec![assignment(
                4,
                PAIR,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    value(2, U32),
                    constant(1),
                )),
            )],
            SemanticTerminatorKindV1::Assert {
                condition: field(1, BOOL),
                expected: false,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: value(2, U32),
                    right: constant(1),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            34,
            vec![assignment(2, U32, SemanticRvalueKindV1::Use(field(0, U32)))],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(35, vec![], SemanticTerminatorKindV1::Return),
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([seed; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([seed; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([seed; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([seed; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([seed; 32]),
        source,
        abi,
        [
            (UNIT, SemanticLocalRoleV1::Return),
            (U32, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Temporary),
            (BOOL, SemanticLocalRoleV1::Temporary),
            (PAIR, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([50 + index as u8; 32]),
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
        SemanticLinkSymbolV1::new(b"recurrence_component".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([seed; 32]),
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
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "component",
            [seed; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch)
}
