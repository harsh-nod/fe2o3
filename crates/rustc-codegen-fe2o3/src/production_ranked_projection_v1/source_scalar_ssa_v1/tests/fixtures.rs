use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirOwnerV1, ProductionSemanticSsaOwnerV1, ProductionSemanticSsaSourceSiteV1,
};

pub(super) const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
pub(super) const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
pub(super) const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

#[derive(Clone, Copy, Debug)]
pub(super) enum Shape {
    Diamond,
    Reversed,
    Bypass,
    Backedge,
    FalseEdge,
    MoveCondition,
    IntegerCondition,
    PartialArm,
    Overwrite,
    ManyNops,
}

pub(super) fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
pub(super) fn constant(bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
    ))
}
pub(super) fn assignment(
    local: u32,
    ty: SemanticTypeIdV1,
    value: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn go(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(target),
    ))
}
fn switch(
    local: u32,
    ty: SemanticTypeIdV1,
    value: u128,
    first: u32,
    second: u32,
    moved: bool,
) -> SemanticTerminatorKindV1 {
    let place = place(local, ty);
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: if moved {
            SemanticOperandV1::Move(place)
        } else {
            SemanticOperandV1::Copy(place)
        },
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                value,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(first),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(second),
            ),
        )
        .unwrap(),
    }
}
pub(super) fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), term),
    )
    .unwrap()
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let make = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            layout,
            shape,
        )
    };
    let scalar = |bits, align, max| {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(align),
            align,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, bits, align),
                SemanticScalarValidityRangeV1::new(0, max),
            )),
            false,
        )
        .unwrap()
    };
    vec![
        make(
            1,
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
        make(
            2,
            scalar(32, 4, u32::MAX as u128),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        make(
            3,
            scalar(8, 1, 1),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ),
    ]
}

pub(super) fn owner(shape: Shape) -> ProductionSemanticSsaOwnerV1 {
    let selector = if matches!(shape, Shape::IntegerCondition) {
        U32
    } else {
        BOOL
    };
    let moved = matches!(shape, Shape::MoveCondition);
    let mut first = Vec::new();
    if moved {
        first.push(assignment(
            4,
            BOOL,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, BOOL))),
        ));
    }
    if matches!(shape, Shape::ManyNops) {
        first.extend((0..10000).map(|_| {
            SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            )
        }));
    }
    let left = if matches!(shape, Shape::PartialArm) {
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Divide,
            left: constant(1),
            right: constant(0),
        }
    } else {
        SemanticRvalueKindV1::Use(constant(7))
    };
    let left_term = match shape {
        Shape::Bypass => switch(1, selector, 0, 3, 5, false),
        Shape::FalseEdge => SemanticTerminatorKindV1::FalseEdge {
            real_target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::FalseEdgeReal,
                SemanticBlockIdV1::from_index(3),
            ),
            imaginary_target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::FalseEdgeImaginary,
                SemanticBlockIdV1::from_index(3),
            ),
        },
        _ => go(3),
    };
    let mut last = Vec::new();
    if matches!(shape, Shape::Overwrite) {
        last.push(assignment(2, U32, SemanticRvalueKindV1::Use(constant(13))));
    }
    last.push(assignment(
        3,
        U32,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, U32))),
    ));
    let mut blocks = vec![
        block(
            20,
            first,
            switch(
                if moved { 4 } else { 1 },
                selector,
                u128::from(matches!(shape, Shape::Reversed)),
                1,
                2,
                moved,
            ),
        ),
        block(21, vec![assignment(2, U32, left)], left_term),
        block(
            22,
            vec![assignment(2, U32, SemanticRvalueKindV1::Use(constant(9)))],
            go(3),
        ),
        block(
            23,
            vec![],
            if matches!(shape, Shape::Backedge) {
                switch(1, selector, 0, 4, 2, false)
            } else {
                go(4)
            },
        ),
        block(24, last, SemanticTerminatorKindV1::Return),
    ];
    if matches!(shape, Shape::Bypass) {
        blocks.push(block(
            25,
            vec![assignment(2, U32, SemanticRvalueKindV1::Use(constant(11)))],
            go(3),
        ));
    }
    let direct = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        selector,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                if selector == BOOL {
                    SemanticAbiExtensionV1::ZeroExtend
                } else {
                    SemanticAbiExtensionV1::None
                },
                0,
                None,
            )
            .unwrap(),
        ),
    ));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([31; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![direct],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [
        (UNIT, SemanticLocalRoleV1::Return),
        (selector, SemanticLocalRoleV1::Argument(0)),
        (U32, SemanticLocalRoleV1::Temporary),
        (U32, SemanticLocalRoleV1::Temporary),
        (BOOL, SemanticLocalRoleV1::Temporary),
    ]
    .into_iter()
    .enumerate()
    .map(|(local, (ty, role))| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([40 + local as u8; 32]),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    })
    .collect();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([50; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"scalar_ssa_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([55; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let source = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([56; 32])),
        types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(source, Default::default()).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(semantic, Default::default()).unwrap();
    owner.verify_replay().unwrap();
    owner
}

pub(super) fn use_site(
    function: &SemanticFunctionDeclV1,
) -> (ProductionSemanticSsaSourceSiteV1, &SemanticOperandV1) {
    function
        .blocks()
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.statements()
                .iter()
                .enumerate()
                .find_map(|(statement, value)| {
                    let SemanticStatementKindV1::Assign(value) = value.kind() else {
                        return None;
                    };
                    if value.destination().local().index() != 3 {
                        return None;
                    }
                    let SemanticRvalueKindV1::Use(operand) = value.value().kind() else {
                        return None;
                    };
                    Some((
                        ProductionSemanticSsaSourceSiteV1::new(
                            SemanticBlockIdV1::from_index(block as u32),
                            Some(statement as u32),
                        ),
                        operand,
                    ))
                })
        })
        .unwrap()
}
