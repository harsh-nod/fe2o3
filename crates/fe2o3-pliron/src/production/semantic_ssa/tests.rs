use super::adapter::semantic_edge_role_v1;
use super::*;
use crate::ProductionSemanticMirLimitsV1;
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, InertSemanticMirRequestV1, SemanticAbiIdentityV1,
    SemanticAbiValueV1, SemanticBasicBlockV1, SemanticBlockIdV1, SemanticBlockIdentityV1,
    SemanticBorrowKindV1, SemanticCallDestinationV1, SemanticCallableIdV1, SemanticCanonAbiV1,
    SemanticCompilerIntrinsicIdentityV1, SemanticConstGenericArgumentsIdentityV1,
    SemanticControlFlowEdgeV1, SemanticDirectCallV1, SemanticExternAbiV1, SemanticFunctionAbiV1,
    SemanticFunctionRoleV1, SemanticGenericTypeArgumentsIdentityV1,
    SemanticItemDefinitionIdentityV1, SemanticLayoutIdentityV1, SemanticLocalDeclV1,
    SemanticLocalIdV1, SemanticLocalIdentityV1, SemanticMemoryStoreV1, SemanticMirLimitsV1,
    SemanticMonomorphizationIdentityV1, SemanticNonBodyCallableBindingV1, SemanticProjectionV1,
    SemanticRvalueV1, SemanticStatementV1, SemanticSwitchTargetV1, SemanticSwitchTargetsV1,
    SemanticTargetDataLayoutV1, SemanticTerminatorV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
    SemanticUnwindActionV1, SemanticVolatilityV1,
};

fn test_bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn test_types(union: bool) -> Vec<SemanticTypeDeclV1> {
    let scalar = SemanticTypeIdV1::from_index(1);
    let fields =
        fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1::new(vec![scalar, scalar])
            .unwrap();
    let aggregate_shape = if union {
        SemanticTypeShapeV1::Union(fields)
    } else {
        SemanticTypeShapeV1::Aggregate(fields)
    };
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(40)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(41)),
            SemanticTypeLayoutV1::new(Some(8), 4).unwrap(),
            aggregate_shape,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(42)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(43)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Opaque,
        ),
    ]
}

fn implicit_scope_types() -> Vec<SemanticTypeDeclV1> {
    let zst = |tag| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(tag + 1)),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateLayoutV1::new(vec![], vec![])
                    .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1::new(vec![]).unwrap(),
            ),
        )
    };
    vec![
        zst(86),
        zst(88),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(90)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(91)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Opaque,
        ),
    ]
}

fn test_local(index: u8, ty: u32, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(test_bytes(index)),
        SemanticTypeIdV1::from_index(ty),
        role,
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
    )
}

fn test_place(local: u32, field: Option<u32>) -> SemanticPlaceV1 {
    let projections = field
        .map(|field| {
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Field(field),
                    SemanticTypeIdV1::from_index(1),
                )
                .unwrap(),
            ]
        })
        .unwrap_or_default();
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        projections,
        SemanticTypeIdV1::from_index(if field.is_some() { 1 } else { 0 }),
    )
    .unwrap()
}

fn test_constant_index_place(local: u32, offset: u64, minimum_length: u64) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end: false,
                },
                SemanticTypeIdV1::from_index(1),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap()
}

fn test_dereference_place(local: u32, result_type: u32) -> SemanticPlaceV1 {
    let result_type = SemanticTypeIdV1::from_index(result_type);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, result_type).unwrap(),
        ],
        result_type,
    )
    .unwrap()
}

fn test_store(destination: SemanticPlaceV1, value: SemanticOperandV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            destination,
            value,
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    )
}

fn test_assign(destination: u32, operand: SemanticOperandV1) -> SemanticStatementV1 {
    test_assign_to(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(destination),
            vec![],
            SemanticTypeIdV1::from_index(1),
        )
        .unwrap(),
        operand,
    )
}

fn test_assign_to(destination: SemanticPlaceV1, operand: SemanticOperandV1) -> SemanticStatementV1 {
    let result_type = destination.ty();
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(
            fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(result_type, SemanticRvalueKindV1::Use(operand)),
            ),
        ),
    )
}

fn test_borrow(reference_local: u32, source_local: u32) -> SemanticStatementV1 {
    test_typed_borrow(reference_local, 1, source_local, 0)
}

fn test_typed_borrow(
    reference_local: u32,
    reference_type: u32,
    source_local: u32,
    source_type: u32,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(
            fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1::new(
                test_typed_place(reference_local, reference_type),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(reference_type),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: test_typed_place(source_local, source_type),
                    },
                ),
            ),
        ),
    )
}

fn test_scalar_place(local: u32) -> SemanticPlaceV1 {
    test_typed_place(local, 1)
}

fn test_typed_place(local: u32, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(ty),
    )
    .unwrap()
}

fn test_call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: Option<SemanticCallDestinationV1>,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            destination,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn test_call_with_unwind(
    destination: SemanticCallDestinationV1,
    unwind: SemanticControlFlowEdgeV1,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(destination),
            SemanticUnwindActionV1::Cleanup(unwind),
        )
        .unwrap(),
    )
}

fn test_intrinsic_callable(abi: SemanticFunctionAbiV1) -> SemanticCallableDeclV1 {
    test_operation_callable(
        abi,
        SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
            scope: SemanticTypeIdV1::from_index(0),
            dynamic_lds: SemanticTypeIdV1::from_index(1),
            element_storage: SemanticTypeIdV1::from_index(1),
            elements: 1,
        },
        80,
    )
}

fn test_operation_callable(
    abi: SemanticFunctionAbiV1,
    operation: SemanticCompilerIntrinsicOperationV1,
    tag: u8,
) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(test_bytes(tag)),
            SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(tag + 1)),
            SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(tag + 2)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(tag + 3)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(tag + 4)),
            fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(test_bytes(tag + 5)),
    }
}

fn source_is_promotable(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
) -> bool {
    let transparent_borrows = transparent_borrow_sites_v1(function, callables);
    semantic_function_ssa_input_v1(function, None, callables, &transparent_borrows)
        .0
        .promotable()[1]
}

fn test_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn test_block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(test_bytes(index)),
        source,
        statements,
        SemanticTerminatorV1::new(source, terminator),
    )
    .unwrap()
}

fn test_function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let scalar = SemanticTypeIdV1::from_index(1);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(50)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(51)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(scalar, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(test_bytes(52)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(53)),
        SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(54)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(55)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(56)),
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            test_local(60, 1, SemanticLocalRoleV1::Return),
            test_local(61, 0, SemanticLocalRoleV1::Argument(0)),
            test_local(62, 1, SemanticLocalRoleV1::Temporary),
            test_local(63, 1, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn test_implicit_scope_function(
    source_type: u32,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let scalar = SemanticTypeIdV1::from_index(2);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(92)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(93)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(scalar, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(test_bytes(94)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(95)),
        SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(96)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(97)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(98)),
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            test_local(99, 2, SemanticLocalRoleV1::Return),
            test_local(100, source_type, SemanticLocalRoleV1::Temporary),
            test_local(101, 2, SemanticLocalRoleV1::Temporary),
            test_local(102, 2, SemanticLocalRoleV1::Temporary),
            test_local(103, 1, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn test_empty_aggregate_assign(local: u32, ty: u32) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(
            fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1::new(
                test_typed_place(local, ty),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(ty),
                    SemanticRvalueKindV1::aggregate(
                        fe2o3_mir_model::semantic_mir_v1::SemanticAggregateKindV1::Aggregate,
                        vec![],
                    )
                    .unwrap(),
                ),
            ),
        ),
    )
}

fn test_storage_dead(local: u32) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)),
    )
}

fn test_discriminant(destination: u32, option: u32) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(
            fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1::new(
                test_typed_place(destination, 2),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(2),
                    SemanticRvalueKindV1::Discriminant(test_typed_place(option, 2)),
                ),
            ),
        ),
    )
}

fn test_option_switch(discriminant: u32, some: u32, otherwise: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(test_typed_place(discriminant, 2)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                test_edge(SemanticEdgeRoleV1::SwitchValue, some),
            )],
            test_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
        )
        .unwrap(),
    }
}

fn test_elided_grid_leader_case(
    source_type: u32,
    borrow_in_some: bool,
    ambiguous_producers: bool,
    kill_source: bool,
    extra_source_use: bool,
) -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
) {
    let mut borrow_statements = Vec::new();
    if kill_source {
        borrow_statements.push(test_storage_dead(1));
    }
    if extra_source_use {
        borrow_statements.push(test_assign_to(
            test_typed_place(7, source_type),
            SemanticOperandV1::Copy(test_typed_place(1, source_type)),
        ));
    }
    borrow_statements.push(test_typed_borrow(4, 2, 1, source_type));
    let consumer = |continuation| {
        test_call(
            1,
            vec![
                SemanticOperandV1::Copy(test_typed_place(2, 2)),
                SemanticOperandV1::Copy(test_typed_place(4, 2)),
            ],
            Some(SemanticCallDestinationV1::new(
                test_typed_place(8, 2),
                test_edge(SemanticEdgeRoleV1::CallReturn, continuation),
            )),
        )
    };

    let blocks = if ambiguous_producers {
        vec![
            test_block(
                112,
                vec![],
                test_call(
                    0,
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        test_typed_place(2, 2),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                ),
            ),
            test_block(
                113,
                vec![test_discriminant(3, 2)],
                test_option_switch(3, 2, 7),
            ),
            test_block(
                114,
                vec![],
                test_call(
                    0,
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        test_typed_place(5, 2),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 3),
                    )),
                ),
            ),
            test_block(
                115,
                vec![test_discriminant(6, 5)],
                test_option_switch(6, 4, 7),
            ),
            test_block(116, borrow_statements, consumer(5)),
            test_block(
                117,
                vec![test_discriminant(9, 8)],
                test_option_switch(9, 6, 7),
            ),
            test_block(118, vec![], SemanticTerminatorKindV1::Return),
            test_block(119, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        let (some_block, otherwise_block) = if borrow_in_some {
            (
                test_block(120, borrow_statements, consumer(4)),
                test_block(121, vec![], SemanticTerminatorKindV1::Return),
            )
        } else {
            (
                test_block(120, vec![], SemanticTerminatorKindV1::Return),
                test_block(121, borrow_statements, consumer(4)),
            )
        };
        let mut blocks = vec![
            test_block(
                118,
                vec![],
                test_call(
                    0,
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        test_typed_place(2, 2),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                ),
            ),
            test_block(
                119,
                vec![test_discriminant(3, 2)],
                test_option_switch(3, 2, 3),
            ),
        ];
        blocks.push(some_block);
        blocks.push(otherwise_block);
        blocks.push(test_block(
            122,
            vec![test_discriminant(9, 8)],
            test_option_switch(9, 5, 6),
        ));
        blocks.push(test_block(123, vec![], SemanticTerminatorKindV1::Return));
        blocks.push(test_block(124, vec![], SemanticTerminatorKindV1::Return));
        blocks
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(122)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(123)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(2),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(test_bytes(124)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(125)),
        SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(126)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(127)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(128)),
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            test_local(129, 2, SemanticLocalRoleV1::Return),
            test_local(130, source_type, SemanticLocalRoleV1::Temporary),
            test_local(131, 2, SemanticLocalRoleV1::Temporary),
            test_local(132, 2, SemanticLocalRoleV1::Temporary),
            test_local(133, 2, SemanticLocalRoleV1::Temporary),
            test_local(134, 2, SemanticLocalRoleV1::Temporary),
            test_local(135, 2, SemanticLocalRoleV1::Temporary),
            test_local(136, source_type, SemanticLocalRoleV1::Temporary),
            test_local(137, 2, SemanticLocalRoleV1::Temporary),
            test_local(138, 2, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    let callables = vec![
        test_operation_callable(
            function.abi().clone(),
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {
                grid_leader: SemanticTypeIdV1::from_index(0),
            },
            139,
        ),
        test_operation_callable(
            function.abi().clone(),
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                disjoint_slice: SemanticTypeIdV1::from_index(2),
                grid_leader: SemanticTypeIdV1::from_index(source_type),
                element: SemanticTypeIdV1::from_index(2),
                raw_index: SemanticTypeIdV1::from_index(2),
            },
            145,
        ),
    ];
    (implicit_scope_types(), function, callables)
}

fn implicit_scope_ssa_input(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> (SsaConstructionInputV1, Vec<SsaVariableIdV1>, usize) {
    let transparent_borrows = transparent_borrow_sites_v1(function, callables);
    semantic_function_ssa_input_v1(function, Some(types), callables, &transparent_borrows)
}

#[test]
fn authenticated_elided_grid_leader_borrow_omits_only_the_zero_bit_source_use() {
    let (types, function, callables) = test_elided_grid_leader_case(0, true, false, false, false);
    let transparent_borrows = transparent_borrow_sites_v1(&function, &callables);
    assert_eq!(transparent_borrows.len(), 1);

    let (input, implicit, analysis_work) =
        semantic_function_ssa_input_v1(&function, Some(&types), &callables, &transparent_borrows);
    assert_eq!(
        input.blocks()[2].events(),
        [
            SsaEventV1::Define(SsaVariableIdV1::new(4)),
            SsaEventV1::Use(SsaVariableIdV1::new(2)),
            SsaEventV1::Use(SsaVariableIdV1::new(4)),
        ]
    );
    assert!(implicit.is_empty());
    assert_ne!(analysis_work, 0);

    let plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &callables,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert!(plan.implicit_entry_variables().is_empty());
    assert!(plan.auxiliary_resources.work_units >= analysis_work);

    let total_work = plan
        .resources()
        .work_units()
        .checked_add(plan.auxiliary_resources.work_units)
        .and_then(|work| work.checked_add(plan.partial_moves.work_units()))
        .unwrap();
    let defaults = SsaPlannerLimitsV1::default();
    let one_work_unit_short = ProductionSemanticSsaLimitsV1::new(
        SsaPlannerLimitsV1::try_new(
            defaults.max_variables(),
            defaults.max_blocks(),
            defaults.max_edges(),
            defaults.max_events(),
            defaults.max_edge_definitions(),
            defaults.max_output_items(),
            defaults.max_storage_words(),
            total_work - 1,
        )
        .unwrap(),
    );
    assert!(matches!(
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &types,
            &callables,
            one_work_unit_short,
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}

#[test]
fn elided_grid_leader_borrow_authentication_fails_closed() {
    for (source_type, borrow_in_some, ambiguous, kill_source, extra_source_use) in [
        (0, false, false, false, false),
        (0, true, true, false, false),
        (1, true, false, false, false),
        (0, true, false, true, false),
        (0, true, false, false, true),
    ] {
        let (types, function, callables) = test_elided_grid_leader_case(
            source_type,
            borrow_in_some,
            ambiguous,
            kill_source,
            extra_source_use,
        );
        assert!(matches!(
            plan_semantic_function_ssa_with_module_v1(
                SemanticFunctionIdV1::from_index(0),
                &function,
                &types,
                &callables,
                ProductionSemanticSsaLimitsV1::default(),
            ),
            Err(ProductionSemanticSsaErrorV1::Planner {
                error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. },
                ..
            }) if variable == SsaVariableIdV1::new(1)
        ));
    }
}

fn plan_test_function(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
) -> Result<ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaErrorV1> {
    plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        function,
        types,
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    )
}

#[test]
fn every_semantic_edge_role_has_a_distinct_nonzero_planner_role() {
    let roles = [
        SemanticEdgeRoleV1::Goto,
        SemanticEdgeRoleV1::SwitchValue,
        SemanticEdgeRoleV1::SwitchOtherwise,
        SemanticEdgeRoleV1::CallReturn,
        SemanticEdgeRoleV1::CallUnwind,
        SemanticEdgeRoleV1::TailCallUnwind,
        SemanticEdgeRoleV1::DropReturn,
        SemanticEdgeRoleV1::DropUnwind,
        SemanticEdgeRoleV1::AssertSuccess,
        SemanticEdgeRoleV1::AssertUnwind,
        SemanticEdgeRoleV1::FalseEdgeReal,
        SemanticEdgeRoleV1::FalseEdgeImaginary,
    ];
    let mapped = roles.map(semantic_edge_role_v1);
    assert!(mapped.iter().all(|role| *role != 0));
    assert_eq!(
        mapped
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        roles.len()
    );
}

#[test]
fn store_move_events_precede_aliasing_destination_uses() {
    let function = test_function(vec![test_block(
        57,
        vec![test_store(
            test_dereference_place(1, 0),
            SemanticOperandV1::Move(test_place(1, None)),
        )],
        SemanticTerminatorKindV1::Return,
    )]);
    let (input, _, _) = semantic_function_ssa_input_v1(&function, None, &[], &BTreeSet::new());

    assert_eq!(
        input.blocks()[0].events(),
        [
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Kill(SsaVariableIdV1::new(1)),
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
        ]
    );
    let error = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap_err();
    assert_eq!(
        error,
        SsaPlannerErrorV1::UndefinedAtUse {
            block: SsaBlockIdV1::new(0),
            event: 2,
            variable: SsaVariableIdV1::new(1),
        }
    );
    assert_eq!(
        error.to_string(),
        "SSA variable 1 is undefined at block 0 event 2"
    );
}

#[test]
fn store_move_events_preserve_a_nonaliasing_destination() {
    let function = test_function(vec![test_block(
        58,
        vec![
            test_assign(2, SemanticOperandV1::Copy(test_scalar_place(1))),
            test_store(
                test_dereference_place(1, 1),
                SemanticOperandV1::Move(test_scalar_place(2)),
            ),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let (input, _, _) = semantic_function_ssa_input_v1(&function, None, &[], &BTreeSet::new());

    assert_eq!(
        input.blocks()[0].events(),
        [
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
            SsaEventV1::Define(SsaVariableIdV1::new(2)),
            SsaEventV1::Use(SsaVariableIdV1::new(2)),
            SsaEventV1::Kill(SsaVariableIdV1::new(2)),
            SsaEventV1::Use(SsaVariableIdV1::new(1)),
        ]
    );
    plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
}

#[test]
fn production_ssa_identity_binds_source_and_function_identity() {
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        0,
        vec![],
        vec![],
        vec![SsaBlockInputV1::new(vec![], vec![])],
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    let make = |source, function_identity| {
        let plans = [ProductionSemanticSsaFunctionPlanV1 {
            function: SemanticFunctionIdV1::from_index(0),
            function_identity: SemanticFunctionIdentityV1::from_sha256(function_identity),
            plan: plan.clone(),
            partial_moves: ProductionSemanticPartialMoveCertificateV1::default(),
            implicit_entry_variables: Box::new([]),
            retained_cross_edge_variables: Box::new([]),
            auxiliary_resources: SemanticSsaAuxiliaryResourcesV1::default(),
        }];
        derive_semantic_ssa_identity_v1(
            &source,
            &plans,
            ProductionSemanticSsaSummaryV1 {
                function_count: 1,
                ..ProductionSemanticSsaSummaryV1::default()
            },
        )
    };
    assert_ne!(make([1; 32], [2; 32]), make([3; 32], [2; 32]));
    assert_ne!(make([1; 32], [2; 32]), make([1; 32], [4; 32]));
}

fn admitted_single_function_semantic() -> AdmittedInertSemanticMirV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let types = vec![SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(131)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(132)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            fe2o3_mir_model::semantic_mir_v1::SemanticFieldsShapeV1::arbitrary(vec![], vec![])
                .unwrap(),
            fe2o3_mir_model::semantic_mir_v1::SemanticRustcVariantsV1::Single { index: 0 },
            fe2o3_mir_model::semantic_mir_v1::SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            fe2o3_mir_model::semantic_mir_v1::SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    )];
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(133)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(134)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(test_bytes(135)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(136)),
        SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(137)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(138)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(139)),
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![test_local(140, 0, SemanticLocalRoleV1::Return)],
        SemanticBlockIdV1::from_index(0),
        vec![test_block(141, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap()
    .with_kernel_entry(
        fe2o3_mir_model::semantic_mir_v1::SemanticKernelEntryV1::new(
            fe2o3_mir_model::semantic_mir_v1::SemanticLinkSymbolV1::new(
                b"semantic_ssa_module_budget_test".to_vec(),
            )
            .unwrap(),
            fe2o3_mir_model::semantic_mir_v1::SemanticKernelBindingIdentityV1::from_sha256(
                test_bytes(143),
            ),
            fe2o3_mir_model::semantic_mir_v1::SemanticKernelSourceContractV1::new(None, None, None)
                .unwrap(),
        ),
    );
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(test_bytes(142))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn module_limit_policy_has_exact_fixed_hard_bounds() {
    let production = ProductionSemanticSsaModuleLimitsV1::production();
    assert_eq!(
        [
            production.max_variables(),
            production.max_blocks(),
            production.max_edges(),
            production.max_events(),
            production.max_edge_definitions(),
            production.max_output_items(),
            production.max_storage_words(),
            production.max_work_units(),
        ],
        [
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_VARIABLES_V1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_BLOCKS_V1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGES_V1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EVENTS_V1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGE_DEFINITIONS_V1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_OUTPUT_ITEMS_V1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_STORAGE_WORDS_V1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_WORK_UNITS_V1,
        ]
    );
    assert_eq!(
        production,
        ProductionSemanticSsaLimitsV1::default().module()
    );
    let planner = SsaPlannerLimitsV1::default();
    assert_eq!(
        [
            production.max_variables(),
            production.max_blocks(),
            production.max_edges(),
            production.max_events(),
            production.max_edge_definitions(),
            production.max_output_items(),
            production.max_storage_words(),
            production.max_work_units(),
        ],
        [
            planner.max_variables() * 4,
            planner.max_blocks() * 4,
            planner.max_edges() * 4,
            planner.max_events() * 4,
            planner.max_edge_definitions() * 4,
            planner.max_output_items() * 4,
            planner.max_storage_words() * 4,
            planner.max_work_units() * 4,
        ]
    );
    assert!(ProductionSemanticSsaModuleLimitsV1::try_new(0, 1, 0, 0, 0, 0, 1, 1).is_ok());

    let invalid = [
        ProductionSemanticSsaModuleLimitsV1::try_new(
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_VARIABLES_V1 + 1,
            1,
            0,
            0,
            0,
            0,
            1,
            1,
        ),
        ProductionSemanticSsaModuleLimitsV1::try_new(0, 0, 0, 0, 0, 0, 1, 1),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            0,
            1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGES_V1 + 1,
            0,
            0,
            0,
            1,
            1,
        ),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            0,
            1,
            0,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EVENTS_V1 + 1,
            0,
            0,
            1,
            1,
        ),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            0,
            1,
            0,
            0,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGE_DEFINITIONS_V1 + 1,
            0,
            1,
            1,
        ),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            0,
            1,
            0,
            0,
            0,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_OUTPUT_ITEMS_V1 + 1,
            1,
            1,
        ),
        ProductionSemanticSsaModuleLimitsV1::try_new(0, 1, 0, 0, 0, 0, 0, 1),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            0,
            1,
            0,
            0,
            0,
            0,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_STORAGE_WORDS_V1 + 1,
            1,
        ),
        ProductionSemanticSsaModuleLimitsV1::try_new(0, 1, 0, 0, 0, 0, 1, 0),
        ProductionSemanticSsaModuleLimitsV1::try_new(
            0,
            1,
            0,
            0,
            0,
            0,
            1,
            HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_WORK_UNITS_V1 + 1,
        ),
        ProductionSemanticSsaModuleLimitsV1::try_new(usize::MAX, 1, 0, 0, 0, 0, 1, 1),
    ];
    assert!(
        invalid.into_iter().all(|result| {
            result == Err(ProductionSemanticSsaModuleLimitsErrorV1::InvalidLimits)
        })
    );
}

#[test]
fn module_accounting_exceeds_one_function_budget_then_fails_the_module_budget() {
    let input = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        0,
        vec![],
        vec![],
        vec![SsaBlockInputV1::new(vec![], vec![])],
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    let resources = plan.resources().clone();
    let function_plan = ProductionSemanticSsaFunctionPlanV1 {
        function: SemanticFunctionIdV1::from_index(0),
        function_identity: SemanticFunctionIdentityV1::from_sha256(test_bytes(130)),
        plan,
        partial_moves: ProductionSemanticPartialMoveCertificateV1::default(),
        implicit_entry_variables: Box::new([]),
        retained_cross_edge_variables: Box::new([]),
        auxiliary_resources: SemanticSsaAuxiliaryResourcesV1::default(),
    };
    let twice = |value: usize| value.checked_mul(2).unwrap();
    let planner = SsaPlannerLimitsV1::try_new(
        0,
        resources.input_blocks(),
        resources.input_edges(),
        resources.input_events(),
        resources.input_edge_definitions(),
        resources.output_items(),
        resources.storage_words(),
        resources.work_units(),
    )
    .unwrap();
    let module = ProductionSemanticSsaModuleLimitsV1::try_new(
        0,
        twice(resources.input_blocks()),
        twice(resources.input_edges()),
        twice(resources.input_events()),
        twice(resources.input_edge_definitions()),
        twice(resources.output_items()),
        twice(resources.storage_words()),
        twice(resources.work_units()),
    )
    .unwrap();
    let limits = ProductionSemanticSsaLimitsV1::with_module_limits(planner, module);
    let mut summary = ProductionSemanticSsaSummaryV1 {
        function_count: 2,
        ..ProductionSemanticSsaSummaryV1::default()
    };
    accumulate_summary_v1(&mut summary, &function_plan, 0, limits).unwrap();
    accumulate_summary_v1(&mut summary, &function_plan, 0, limits).unwrap();
    assert!(summary.input_blocks() > planner.max_blocks());
    assert!(summary.storage_words() > planner.max_storage_words());
    assert!(summary.work_units() > planner.max_work_units());
    assert_eq!(summary.input_blocks(), module.max_blocks());
    assert_eq!(summary.storage_words(), module.max_storage_words());
    assert_eq!(summary.work_units(), module.max_work_units());

    assert_eq!(
        accumulate_summary_v1(&mut summary, &function_plan, 0, limits),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::Blocks,
            required: module.max_blocks() + resources.input_blocks(),
            limit: module.max_blocks(),
        })
    );
}

#[test]
fn module_resource_boundaries_are_inclusive_and_overflow_safe() {
    let module = ProductionSemanticSsaModuleLimitsV1::try_new(1, 1, 1, 1, 1, 1, 1, 1).unwrap();
    let limits =
        ProductionSemanticSsaLimitsV1::with_module_limits(SsaPlannerLimitsV1::default(), module);
    let exact = ProductionSemanticSsaSummaryV1 {
        promotable_variables: 1,
        input_blocks: 1,
        input_edges: 1,
        input_events: 1,
        input_edge_definitions: 1,
        output_items: 1,
        storage_words: 1,
        work_units: 1,
        ..ProductionSemanticSsaSummaryV1::default()
    };
    accounting::enforce_module_resource_limits_v1(exact, limits).unwrap();
    for (resource, hostile) in [
        (
            SsaPlannerResourceV1::Variables,
            ProductionSemanticSsaSummaryV1 {
                promotable_variables: 2,
                ..exact
            },
        ),
        (
            SsaPlannerResourceV1::Blocks,
            ProductionSemanticSsaSummaryV1 {
                input_blocks: 2,
                ..exact
            },
        ),
        (
            SsaPlannerResourceV1::Edges,
            ProductionSemanticSsaSummaryV1 {
                input_edges: 2,
                ..exact
            },
        ),
        (
            SsaPlannerResourceV1::Events,
            ProductionSemanticSsaSummaryV1 {
                input_events: 2,
                ..exact
            },
        ),
        (
            SsaPlannerResourceV1::EdgeDefinitions,
            ProductionSemanticSsaSummaryV1 {
                input_edge_definitions: 2,
                ..exact
            },
        ),
        (
            SsaPlannerResourceV1::OutputItems,
            ProductionSemanticSsaSummaryV1 {
                output_items: 2,
                ..exact
            },
        ),
        (
            SsaPlannerResourceV1::StorageWords,
            ProductionSemanticSsaSummaryV1 {
                storage_words: 2,
                ..exact
            },
        ),
        (
            SsaPlannerResourceV1::WorkUnits,
            ProductionSemanticSsaSummaryV1 {
                work_units: 2,
                ..exact
            },
        ),
    ] {
        assert_eq!(
            accounting::enforce_module_resource_limits_v1(hostile, limits),
            Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource,
                required: 2,
                limit: 1,
            })
        );
    }
    assert_eq!(
        accounting::enforce_module_resource_limits_v1(
            ProductionSemanticSsaSummaryV1 {
                promotable_variables: usize::MAX,
                memory_variables: 1,
                ..ProductionSemanticSsaSummaryV1::default()
            },
            limits,
        ),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
}

#[test]
fn permissive_module_ceilings_preserve_admitted_source_identity() {
    let semantic = admitted_single_function_semantic();
    let broad_limits = ProductionSemanticSsaLimitsV1::default();
    let (broad_plans, broad_summary, broad_identity) =
        construct_semantic_ssa_plans_v1(&semantic, broad_limits).unwrap();
    let variables = broad_summary
        .promotable_variables()
        .checked_add(broad_summary.memory_variables())
        .unwrap();
    let exact_module = ProductionSemanticSsaModuleLimitsV1::try_new(
        variables,
        broad_summary.input_blocks(),
        broad_summary.input_edges(),
        broad_summary.input_events(),
        broad_summary.input_edge_definitions(),
        broad_summary.output_items(),
        broad_summary.storage_words(),
        broad_summary.work_units(),
    )
    .unwrap();
    let exact_limits = ProductionSemanticSsaLimitsV1::with_module_limits(
        SsaPlannerLimitsV1::default(),
        exact_module,
    );
    let (exact_plans, exact_summary, exact_identity) =
        construct_semantic_ssa_plans_v1(&semantic, exact_limits).unwrap();
    assert_eq!(broad_plans, exact_plans);
    assert_eq!(broad_summary, exact_summary);
    assert_eq!(broad_identity, exact_identity);

    let rejecting_module = ProductionSemanticSsaModuleLimitsV1::try_new(
        variables,
        broad_summary.input_blocks(),
        broad_summary.input_edges(),
        broad_summary.input_events(),
        broad_summary.input_edge_definitions(),
        broad_summary.output_items(),
        broad_summary.storage_words(),
        broad_summary.work_units() - 1,
    )
    .unwrap();
    assert!(matches!(
        construct_semantic_ssa_plans_v1(
            &semantic,
            ProductionSemanticSsaLimitsV1::with_module_limits(
                SsaPlannerLimitsV1::default(),
                rejecting_module,
            ),
        ),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}

#[test]
fn semantic_ssa_owner_replays_under_the_fixed_module_envelope() {
    let source_owner = ProductionSemanticMirOwnerV1::try_new(
        admitted_single_function_semantic(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        source_owner,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(
        owner.summary().function_count(),
        owner.source_semantic().functions().len()
    );
    assert!(!owner.grants_proof_or_artifact_authority());
}

#[test]
fn semantic_adapter_resource_limits_are_inclusive_and_fail_closed() {
    let function = test_function(vec![test_block(
        0,
        vec![],
        SemanticTerminatorKindV1::Return,
    )]);
    let function_id = SemanticFunctionIdV1::from_index(0);
    let baseline = plan_semantic_function_ssa_v1(
        function_id,
        &function,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let storage = baseline
        .resources()
        .storage_words()
        .checked_add(baseline.auxiliary_resources.storage_words)
        .and_then(|value| value.checked_add(baseline.partial_moves.state_entries()))
        .unwrap();
    let work = baseline
        .resources()
        .work_units()
        .checked_add(baseline.auxiliary_resources.work_units)
        .and_then(|value| value.checked_add(baseline.partial_moves.work_units()))
        .unwrap();
    let defaults = SsaPlannerLimitsV1::default();
    let limits = |storage_words, work_units| {
        ProductionSemanticSsaLimitsV1::new(
            SsaPlannerLimitsV1::try_new(
                defaults.max_variables(),
                defaults.max_blocks(),
                defaults.max_edges(),
                defaults.max_events(),
                defaults.max_edge_definitions(),
                defaults.max_output_items(),
                storage_words,
                work_units,
            )
            .unwrap(),
        )
    };

    plan_semantic_function_ssa_v1(function_id, &function, limits(storage, work)).unwrap();
    assert!(limits(storage, work - 1).module().max_work_units() > work);
    assert!(matches!(
        plan_semantic_function_ssa_v1(function_id, &function, limits(storage - 1, work)),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::StorageWords,
            ..
        })
    ));
    assert!(matches!(
        plan_semantic_function_ssa_v1(function_id, &function, limits(storage, work - 1)),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}

#[test]
fn retained_cfg_storage_detects_edges_and_cycles() {
    let variable = SsaVariableIdV1::new(0);
    let edge =
        |target| SsaEdgeInputV1::new(SsaEdgeRoleV1::new(1), SsaBlockIdV1::new(target), vec![]);
    let cross_edge = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        1,
        vec![false],
        vec![variable],
        vec![
            SsaBlockInputV1::new(vec![], vec![edge(1)]),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(variable)], vec![]),
        ],
    );
    let cross_edge_plan =
        plan_ssa_with_limits_v1(&cross_edge, SsaPlannerLimitsV1::default()).unwrap();
    assert_eq!(
        retained_cross_edge_variables_v1(&cross_edge, &cross_edge_plan),
        [variable]
    );

    let cyclic = SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        1,
        vec![false],
        vec![variable],
        vec![SsaBlockInputV1::new(
            vec![SsaEventV1::Use(variable)],
            vec![edge(0)],
        )],
    );
    let cyclic_plan = plan_ssa_with_limits_v1(&cyclic, SsaPlannerLimitsV1::default()).unwrap();
    assert_eq!(
        retained_cross_edge_variables_v1(&cyclic, &cyclic_plan),
        [variable]
    );
}

#[test]
fn implicit_workgroup_lds_scope_certifies_an_elided_temporary_producer() {
    let types = implicit_scope_types();
    let function = test_implicit_scope_function(
        0,
        vec![test_block(
            104,
            vec![test_typed_borrow(2, 2, 1, 0)],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let callables = [test_intrinsic_callable(function.abi().clone())];

    let plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &function,
        &types,
        &callables,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(plan.implicit_entry_variables(), [SsaVariableIdV1::new(1)]);
}

#[test]
fn implicit_workgroup_lds_scope_rejects_ordinary_and_nontransparent_extra_uses() {
    let types = implicit_scope_types();
    let ordinary_use = test_implicit_scope_function(
        0,
        vec![test_block(
            105,
            vec![
                test_typed_borrow(2, 2, 1, 0),
                test_assign_to(
                    test_typed_place(3, 2),
                    SemanticOperandV1::Copy(test_typed_place(1, 0)),
                ),
            ],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let ordinary_callables = [test_intrinsic_callable(ordinary_use.abi().clone())];
    let (ordinary_input, ordinary_implicit, _) =
        implicit_scope_ssa_input(&ordinary_use, &types, &ordinary_callables);
    assert!(ordinary_implicit.is_empty());
    assert!(ordinary_input.promotable()[1]);
    assert!(matches!(
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &ordinary_use,
            &types,
            &ordinary_callables,
            ProductionSemanticSsaLimitsV1::default(),
        ),
        Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. },
            ..
        }) if variable == SsaVariableIdV1::new(1)
    ));

    let nontransparent_use = test_implicit_scope_function(
        0,
        vec![test_block(
            106,
            vec![test_typed_borrow(2, 2, 1, 0), test_typed_borrow(3, 2, 1, 0)],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let nontransparent_callables = [test_intrinsic_callable(nontransparent_use.abi().clone())];
    let (nontransparent_input, nontransparent_implicit, _) =
        implicit_scope_ssa_input(&nontransparent_use, &types, &nontransparent_callables);
    assert!(nontransparent_implicit.is_empty());
    assert!(!nontransparent_input.promotable()[1]);
}

#[test]
fn implicit_workgroup_lds_scope_rejects_kills_and_definitions() {
    let types = implicit_scope_types();
    let killed = test_implicit_scope_function(
        0,
        vec![test_block(
            107,
            vec![test_typed_borrow(2, 2, 1, 0), test_storage_dead(1)],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let killed_callables = [test_intrinsic_callable(killed.abi().clone())];
    assert!(
        implicit_scope_ssa_input(&killed, &types, &killed_callables)
            .1
            .is_empty()
    );
    assert!(matches!(
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &killed,
            &types,
            &killed_callables,
            ProductionSemanticSsaLimitsV1::default(),
        ),
        Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. },
            ..
        }) if variable == SsaVariableIdV1::new(1)
    ));

    let defined = test_implicit_scope_function(
        0,
        vec![test_block(
            108,
            vec![
                test_empty_aggregate_assign(1, 0),
                test_typed_borrow(2, 2, 1, 0),
            ],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let defined_callables = [test_intrinsic_callable(defined.abi().clone())];
    let defined_plan = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &defined,
        &types,
        &defined_callables,
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert!(defined_plan.implicit_entry_variables().is_empty());
}

#[test]
fn implicit_workgroup_lds_scope_rejects_an_unnamed_same_shaped_zst() {
    let types = implicit_scope_types();
    let function = test_implicit_scope_function(
        1,
        vec![test_block(
            109,
            vec![test_typed_borrow(2, 2, 1, 1)],
            test_call(
                1,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let callables = [
        test_intrinsic_callable(function.abi().clone()),
        test_operation_callable(
            function.abi().clone(),
            SemanticCompilerIntrinsicOperationV1::DynamicLdsIntoCollectiveRawParts {
                dynamic_lds: SemanticTypeIdV1::from_index(1),
                raw_parts: SemanticTypeIdV1::from_index(2),
                element_storage: SemanticTypeIdV1::from_index(2),
                element: SemanticTypeIdV1::from_index(2),
            },
            110,
        ),
    ];

    assert!(authenticated_ambient_workgroup_lds_scope_zst_v1(
        &types,
        &callables,
        SemanticTypeIdV1::from_index(0),
    ));
    assert!(!authenticated_ambient_workgroup_lds_scope_zst_v1(
        &types,
        &callables,
        SemanticTypeIdV1::from_index(1),
    ));
    let (input, implicit, _) = implicit_scope_ssa_input(&function, &types, &callables);
    assert!(input.promotable()[1]);
    assert!(implicit.is_empty());
    assert!(matches!(
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            &types,
            &callables,
            ProductionSemanticSsaLimitsV1::default(),
        ),
        Err(ProductionSemanticSsaErrorV1::Planner {
            error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. },
            ..
        }) if variable == SsaVariableIdV1::new(1)
    ));
}

#[test]
fn transparent_borrow_accepts_one_direct_compiler_intrinsic_consumer() {
    let function = test_function(vec![test_block(
        64,
        vec![test_borrow(2, 1)],
        test_call(0, vec![SemanticOperandV1::Copy(test_scalar_place(2))], None),
    )]);
    let callables = [test_intrinsic_callable(function.abi().clone())];

    assert!(source_is_promotable(&function, &callables));
}

#[test]
fn transparent_borrow_rejects_an_escaping_reference() {
    let function = test_function(vec![test_block(
        65,
        vec![
            test_borrow(2, 1),
            test_assign(3, SemanticOperandV1::Copy(test_scalar_place(2))),
        ],
        test_call(0, vec![SemanticOperandV1::Copy(test_scalar_place(2))], None),
    )]);
    let callables = [test_intrinsic_callable(function.abi().clone())];

    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn transparent_borrow_rejects_multiple_intrinsic_consumers() {
    let function = test_function(vec![
        test_block(
            66,
            vec![test_borrow(2, 1)],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_scalar_place(2))],
                Some(SemanticCallDestinationV1::new(
                    test_scalar_place(3),
                    test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
            ),
        ),
        test_block(
            67,
            vec![],
            test_call(0, vec![SemanticOperandV1::Copy(test_scalar_place(2))], None),
        ),
    ]);
    let callables = [test_intrinsic_callable(function.abi().clone())];

    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn transparent_borrow_rejects_an_ordinary_call_consumer() {
    let function = test_function(vec![test_block(
        68,
        vec![test_borrow(2, 1)],
        test_call(0, vec![SemanticOperandV1::Copy(test_scalar_place(2))], None),
    )]);
    let callables = [SemanticCallableDeclV1::defined(
        SemanticFunctionIdV1::from_index(0),
    )];

    assert!(!source_is_promotable(&function, &callables));
}

#[test]
fn partial_move_certificate_allows_disjoint_sibling_fields() {
    let function = test_function(vec![test_block(
        70,
        vec![
            test_assign(2, SemanticOperandV1::Move(test_place(1, Some(0)))),
            test_assign(3, SemanticOperandV1::Move(test_place(1, Some(1)))),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let plan = plan_test_function(&function, &test_types(false)).unwrap();
    assert_eq!(plan.partial_move_certificate().projected_moves(), 2);
    assert!(plan.partial_move_certificate().work_units() != 0);
}

#[test]
fn partial_move_certificate_clears_an_exactly_reinitialized_field() {
    let function = test_function(vec![test_block(
        69,
        vec![
            test_assign(2, SemanticOperandV1::Move(test_place(1, Some(0)))),
            test_assign_to(
                test_place(1, Some(0)),
                SemanticOperandV1::Move(test_scalar_place(2)),
            ),
            test_assign(3, SemanticOperandV1::Copy(test_place(1, None))),
        ],
        SemanticTerminatorKindV1::Return,
    )]);

    let plan = plan_test_function(&function, &test_types(false)).unwrap();
    assert_eq!(plan.partial_move_certificate().projected_moves(), 1);
}

#[test]
fn partial_move_certificate_does_not_clear_a_moved_sibling() {
    let function = test_function(vec![test_block(
        70,
        vec![
            test_assign(2, SemanticOperandV1::Move(test_place(1, Some(0)))),
            test_assign_to(
                test_place(1, Some(1)),
                SemanticOperandV1::Move(test_scalar_place(2)),
            ),
            test_assign(3, SemanticOperandV1::Copy(test_place(1, None))),
        ],
        SemanticTerminatorKindV1::Return,
    )]);

    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            statement: Some(2),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn partial_move_certificate_rejects_same_parent_and_child_reuse() {
    for second in [
        SemanticOperandV1::Copy(test_place(1, Some(0))),
        SemanticOperandV1::Copy(test_place(1, None)),
    ] {
        let function = test_function(vec![test_block(
            71,
            vec![
                test_assign(2, SemanticOperandV1::Move(test_place(1, Some(0)))),
                test_assign(3, second),
            ],
            SemanticTerminatorKindV1::Return,
        )]);
        assert!(matches!(
            plan_test_function(&function, &test_types(false)),
            Err(ProductionSemanticSsaErrorV1::PartialMove {
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ..
            })
        ));
    }
}

#[test]
fn partial_move_certificate_merges_maybe_moved_state_at_join() {
    let function = test_function(vec![
        test_block(
            72,
            vec![],
            SemanticTerminatorKindV1::FalseEdge {
                real_target: test_edge(SemanticEdgeRoleV1::FalseEdgeReal, 1),
                imaginary_target: test_edge(SemanticEdgeRoleV1::FalseEdgeImaginary, 2),
            },
        ),
        test_block(
            73,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        test_block(
            74,
            vec![],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        test_block(
            75,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 3,
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn partial_move_certificate_reaches_a_loop_fixed_point() {
    let function = test_function(vec![
        test_block(
            76,
            vec![],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        test_block(
            77,
            vec![
                test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(0)))),
                test_assign(3, SemanticOperandV1::Move(test_place(1, Some(0)))),
            ],
            SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
    ]);
    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 1,
            statement: Some(0),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn partial_move_certificate_rejects_union_and_missing_type_context() {
    let function = test_function(vec![test_block(
        78,
        vec![test_assign(
            2,
            SemanticOperandV1::Move(test_place(1, Some(0))),
        )],
        SemanticTerminatorKindV1::Return,
    )]);
    assert!(matches!(
        plan_test_function(&function, &test_types(true)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::UnionField,
            ..
        })
    ));
    assert!(matches!(
        plan_semantic_function_ssa_v1(
            SemanticFunctionIdV1::from_index(0),
            &function,
            ProductionSemanticSsaLimitsV1::default(),
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            violation: SemanticPartialMoveViolationV1::MissingTypeContext,
            ..
        })
    ));
}

#[test]
fn partial_move_constant_index_identity_ignores_minimum_length() {
    let function = test_function(vec![test_block(
        79,
        vec![
            test_assign(
                2,
                SemanticOperandV1::Move(test_constant_index_place(1, 0, 1)),
            ),
            test_assign(
                3,
                SemanticOperandV1::Copy(test_constant_index_place(1, 0, 8)),
            ),
        ],
        SemanticTerminatorKindV1::Return,
    )]);

    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            statement: Some(1),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));
}

#[test]
fn projected_call_destination_reinitializes_only_its_return_edge() {
    let destination = SemanticCallDestinationV1::new(
        test_place(1, Some(0)),
        test_edge(SemanticEdgeRoleV1::CallReturn, 1),
    );
    let unwind = test_edge(SemanticEdgeRoleV1::CallUnwind, 2);
    let function = test_function(vec![
        test_block(
            80,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            test_call_with_unwind(destination, unwind),
        ),
        test_block(
            81,
            vec![test_assign(3, SemanticOperandV1::Copy(test_place(1, None)))],
            SemanticTerminatorKindV1::Return,
        ),
        test_block(
            82,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);

    assert!(matches!(
        plan_test_function(&function, &test_types(false)),
        Err(ProductionSemanticSsaErrorV1::PartialMove {
            block: 2,
            statement: Some(0),
            violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ..
        })
    ));

    let return_only = test_function(vec![
        test_block(
            83,
            vec![test_assign(
                2,
                SemanticOperandV1::Move(test_place(1, Some(0))),
            )],
            test_call(
                0,
                vec![],
                Some(SemanticCallDestinationV1::new(
                    test_place(1, Some(0)),
                    test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                )),
            ),
        ),
        test_block(
            84,
            vec![test_assign(3, SemanticOperandV1::Copy(test_place(1, None)))],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    plan_test_function(&return_only, &test_types(false)).unwrap();
}
