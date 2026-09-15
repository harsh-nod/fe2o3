//! Canonically admitted component source. This is not authenticated frontend
//! issuance: tests exercise expansion/transparency and never lower a kernel.
use fe2o3_mir_model::semantic_mir_v1::*;

include!("canonical_fixture_helpers.rs");

#[path = "tuple_carrier_fixture.rs"]
pub(super) mod tuple_carrier;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mutation {
    None,
    DuplicateBorrow,
    ExtraCapture,
    ProjectedCopy,
    DeadOwner,
    Uninitialized,
    WrongField,
    ReorderedBind,
    WrongNormalEdge,
}

pub(super) fn source(mutation: Mutation) -> AdmittedInertSemanticMirV1 {
    try_source(mutation).unwrap()
}

pub(super) fn try_source(
    mutation: Mutation,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    use SemanticLocalRoleV1::{Argument as Arg, Return as Ret, Temporary as Tmp};
    let types = vec![
        unit(),
        zst(1),
        zst(2),
        zst(3),
        zst(4),
        reference(5, 2),
        reference(6, 3),
        aggregate_type(7, &[5, 6, 4, 4], &[0, 8, 16, 16], 16, true),
        reference(8, 7),
        aggregate_type(9, &[8, 4], &[0, 8], 8, false),
        zst(10),
        zst(11),
        reference(12, 1),
    ];
    let mut statements = vec![borrow(8, 8, 7, 7)];
    match mutation {
        Mutation::DuplicateBorrow => statements.push(borrow(8, 8, 7, 7)),
        Mutation::ExtraCapture => {
            statements.push(assign(9, 9, aggregate(vec![copy(8, 8), zero(4)])))
        }
        Mutation::ProjectedCopy => statements.push(assign(
            7,
            7,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    local_id(8),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(7))
                            .unwrap(),
                    ],
                    ty(7),
                )
                .unwrap(),
            )),
        )),
        Mutation::DeadOwner => statements.insert(
            0,
            SemanticStatementV1::new(loc(), SemanticStatementKindV1::StorageDead(local_id(7))),
        ),
        Mutation::Uninitialized => statements.insert(
            0,
            SemanticStatementV1::new(loc(), SemanticStatementKindV1::Deinitialize(place(7, 7))),
        ),
        Mutation::None
        | Mutation::WrongField
        | Mutation::ReorderedBind
        | Mutation::WrongNormalEdge => (),
    }
    let root = function(
        30,
        true,
        &[],
        0,
        &(0..13)
            .map(|id| (id, if id == 0 { Ret } else { Tmp }))
            .collect::<Vec<_>>(),
        vec![
            block(0, vec![], call(6, vec![], 1, 1, 1)),
            block(
                1,
                vec![
                    assign(10, 10, aggregate(vec![])),
                    assign(11, 11, aggregate(vec![])),
                ],
                call(4, vec![copy(10, 10), copy(11, 11)], 2, 2, 2),
            ),
            block(
                2,
                vec![borrow(12, 12, 1, 1)],
                call(5, vec![copy(12, 12)], 3, 3, 3),
            ),
            block(
                3,
                vec![borrow(5, 5, 2, 2), borrow(6, 6, 3, 3)],
                call(1, vec![copy(5, 5), copy(6, 6)], 7, 7, 4),
            ),
            block(4, statements, call(2, vec![copy(8, 8)], 9, 9, 5)),
            block(5, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"matrix_capture_component".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([40; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let bind = function(
        31,
        false,
        &[5, 6],
        7,
        &[(7, Ret), (5, Arg(0)), (6, Arg(1))],
        vec![block(
            0,
            vec![assign(
                0,
                7,
                aggregate(if mutation == Mutation::ReorderedBind {
                    vec![copy(2, 6), copy(1, 5), zero(4), zero(4)]
                } else {
                    vec![copy(1, 5), copy(2, 6), zero(4), zero(4)]
                }),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let narrow = function(
        32,
        false,
        &[8],
        9,
        &[(9, Ret), (8, Arg(0)), (5, Tmp)],
        vec![
            block(
                0,
                vec![],
                call(
                    3,
                    vec![copy(1, 8)],
                    2,
                    5,
                    if mutation == Mutation::WrongNormalEdge {
                        0
                    } else {
                        1
                    },
                ),
            ),
            block(
                1,
                vec![assign(0, 9, aggregate(vec![copy(1, 8), zero(4)]))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    );
    let projected = SemanticPlaceV1::new(
        local_id(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(7)).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(u32::from(mutation == Mutation::WrongField)),
                ty(5),
            )
            .unwrap(),
        ],
        ty(5),
    )
    .unwrap();
    let getter = function(
        33,
        false,
        &[8],
        5,
        &[(5, Ret), (8, Arg(0))],
        vec![block(
            0,
            vec![assign(
                0,
                5,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([40; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([41; 32]),
        SemanticTypeIdentityV1::from_sha256([42; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([43; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([44; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([45; 32]),
    )
    .unwrap();
    let identity = SemanticDefinedMatrixIdentityV1::new(
        provenance,
        SemanticTypeIdentityV1::from_sha256([46; 32]),
        SemanticTypeIdentityV1::from_sha256([47; 32]),
        SemanticTypeIdentityV1::from_sha256([48; 32]),
        SemanticTypeIdentityV1::from_sha256([49; 32]),
    )
    .unwrap();
    let matrix = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::MatrixAccess {
            subgroup: ty(10),
            epoch: ty(11),
            matrix: ty(2),
            subgroup_brand: identity.matrix_brand(),
            width: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(10), ty(11)], ty(2)).unwrap(),
        provenance,
        identity.execution_brand(),
        identity.epoch(),
        None,
        SemanticFunctionIdentityV1::from_sha256([34; 32]),
    )
    .unwrap();
    let policy = SemanticExecutionCapabilityContractV1::new_kernel_scoped(
        SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
            context: ty(12),
            capability: ty(3),
            policy: identity.policy(),
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(12)], ty(3)).unwrap(),
        provenance,
        SemanticFunctionIdentityV1::from_sha256([35; 32]),
    )
    .unwrap();
    let mut functions = vec![root, bind, narrow, getter];
    let mut callables = (0..4)
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i)))
        .collect::<Vec<_>>();
    callables.extend([
        terminal(
            34,
            &[10, 11],
            2,
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: matrix },
        ),
        terminal(
            35,
            &[12],
            3,
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: policy },
        ),
        terminal(
            36,
            &[],
            1,
            SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: ty(1) },
        ),
    ]);
    let ids = SemanticPolicyGfx950NarrowTypesV1::new([5, 2, 6, 3, 7, 8, 9].map(ty));
    let bind = SemanticPolicyMatrixBindV1::for_defined_function(
        SemanticFunctionIdV1::from_index(1),
        &functions,
        &callables,
        &types,
        ids.bind,
        identity,
    )?;
    let narrow = SemanticPolicyGfx950NarrowV1::for_defined_function(
        SemanticFunctionIdV1::from_index(2),
        &functions,
        &callables,
        &types,
        ids,
        identity,
    )?;
    functions[1] = functions[1].clone().with_defined_capability_contract(
        SemanticDefinedCapabilityContractV1::PolicyMatrixBind(bind),
    )?;
    functions[2] = functions[2].clone().with_defined_capability_contract(
        SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(narrow),
    )?;
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([90; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
}
