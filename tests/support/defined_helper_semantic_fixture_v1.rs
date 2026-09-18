// Genuine admitted semantic input shared by source and native template tests.
// No proof-runtime or source/native correspondence authority is implied.
use fe2o3_mir_model::semantic_mir_v1::*;

pub(super) const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
pub(super) const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);

pub(super) fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], WORD).unwrap()
}
pub(super) fn copy(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local))
}
pub(super) fn assign(local: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local),
            SemanticRvalueV1::new(WORD, value),
        )),
    )
}
pub(super) fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, terminator),
    )
    .unwrap()
}
pub(super) fn call(
    target: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: u32,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(target),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(destination),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(next),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

pub(super) fn abi(root: bool) -> SemanticFunctionAbiV1 {
    let scalar = SemanticAbiValueV1::new(
        WORD,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    );
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([if root { 10 } else { 11 }; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
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
        2,
        vec![
            SemanticAbiArgumentV1::source(scalar.clone()),
            SemanticAbiArgumentV1::source(scalar.clone()),
        ],
        if root {
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore)
        } else {
            scalar
        },
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap()
}

pub(super) fn function(
    tag: u8,
    root: bool,
    temporary_count: usize,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut locals = vec![
        (if root { UNIT } else { WORD }, SemanticLocalRoleV1::Return),
        (WORD, SemanticLocalRoleV1::Argument(0)),
        (WORD, SemanticLocalRoleV1::Argument(1)),
    ];
    locals.extend((0..temporary_count).map(|_| (WORD, SemanticLocalRoleV1::Temporary)));
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
        abi(root),
        locals
            .into_iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + i as u8 + 1; 32]),
                    ty,
                    role,
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    if !root {
        return function;
    }
    let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"helper_value_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([31; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ))
}

pub(super) fn subtract() -> SemanticFunctionDeclV1 {
    function(
        60,
        false,
        0,
        vec![block(
            61,
            vec![assign(
                0,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Subtract,
                    left: copy(1),
                    right: copy(2),
                },
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )
}

pub(super) fn source(helpers: Vec<SemanticFunctionDeclV1>) -> AdmittedInertSemanticMirV1 {
    let root = function(
        30,
        true,
        2,
        vec![
            block(31, vec![], call(1, vec![copy(1), copy(2)], 3, 1)),
            block(
                32,
                vec![assign(4, SemanticRvalueKindV1::Use(copy(3)))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let unit = SemanticTypeDeclV1::new(
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
    );
    let word = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([2; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        vec![unit, word],
        vec![],
        vec![],
        vec![],
        std::iter::once(root).chain(helpers).collect(),
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}
