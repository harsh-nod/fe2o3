//! Exact inert source fixture helpers, not admitted semantic ownership.
#![allow(dead_code)]
use fe2o3_mir_model::semantic_mir_v1::*;
const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 19;
// Exact helper copies follow; all fixture owners are inert model DATA.
pub(super) fn local(
    tag: u8,
    ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(tag)),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

pub(super) fn block(
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

pub(super) fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

pub(super) fn projection_function_with_locals(
    blocks: Vec<SemanticBasicBlockV1>,
    locals: Vec<SemanticLocalDeclV1>,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(10)),
        SemanticLayoutIdentityV1::from_sha256(bytes(10)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(11)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(12)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(13)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(14)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(15)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

pub(super) fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

pub(super) fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SCALAR_TYPE,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

pub(super) fn bytes(value: u8) -> [u8; 32] {
    [value; 32]
}

pub(super) fn place(index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], SCALAR_TYPE).unwrap()
}

pub(super) fn assign(index: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(index),
        SemanticRvalueV1::new(SCALAR_TYPE, value),
    )))
}

pub(super) fn fixture(
    statements: Vec<SemanticStatementV1>,
    count: u8,
    call_destination: Option<u32>,
) -> SemanticFunctionDeclV1 {
    let terminator = if let Some(index) = call_destination {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(index),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    } else {
        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1))
    };
    projection_function_with_locals(
        vec![
            block(80, statements, terminator),
            block(81, vec![], SemanticTerminatorKindV1::Return),
        ],
        (0..count)
            .map(|index| {
                local(
                    100 + index,
                    SCALAR_TYPE,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                )
            })
            .collect(),
    )
}

pub(super) fn projection_types() -> Vec<SemanticTypeDeclV1> {
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(1)),
            SemanticLayoutIdentityV1::from_sha256(bytes(1)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(2)),
            SemanticLayoutIdentityV1::from_sha256(bytes(2)),
            SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
            SemanticTypeShapeV1::Array {
                element: SCALAR_TYPE,
                length: 4,
            },
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(3)),
            SemanticLayoutIdentityV1::from_sha256(bytes(3)),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    SCALAR_TYPE,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ]
}
pub(super) fn store(place: SemanticPlaceV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
        place,
        constant(7),
        SemanticVolatilityV1::NonVolatile,
        None,
    )))
}
pub(super) fn positive() -> SemanticFunctionDeclV1 {
    fixture(vec![store(place(1))], 4, None)
}
pub(super) fn empty() -> SemanticFunctionDeclV1 {
    fixture(
        vec![assign(1, SemanticRvalueKindV1::Use(constant(7)))],
        4,
        None,
    )
}
pub(super) fn target() -> SemanticTargetDataLayoutV1 {
    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(90)))
}
