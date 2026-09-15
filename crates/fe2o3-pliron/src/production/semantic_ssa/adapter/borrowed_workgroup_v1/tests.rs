use super::*;
#[path = "candidate_source_tests.rs"]
pub(super) mod candidate_source_tests;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "context_issue_tests.rs"]
mod context_issue_tests;
#[path = "transpose_tests.rs"]
mod transpose_tests;
#[path = "allocation_borrow_tests.rs"]
mod allocation_borrow_tests;
#[path = "subgroup_matrix_tests.rs"]
mod subgroup_matrix_tests;
#[path = "non_assignment_uses_tests.rs"]
mod non_assignment_uses_tests;
#[path = "terminal_dispatch_tests.rs"]
mod terminal_dispatch_tests;
#[path = "callable_facts_cache_v1/tests.rs"]
mod callable_facts_cache_tests;
#[path = "shared_carrier_tests.rs"]
mod shared_carrier_tests;

fn ty(id: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(id)
}
fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, id: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(id)).unwrap()
}
fn local(index: u8, id: u32, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([index + 1; 32]),
        ty(id),
        role,
        source(),
    )
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(source(), kind)
}
fn assign(local: u32, id: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local, id),
        SemanticRvalueV1::new(ty(id), value),
    )))
}
fn borrow(
    local: u32,
    id: u32,
    from: SemanticPlaceV1,
    kind: SemanticBorrowKindV1,
) -> SemanticStatementV1 {
    assign(
        local,
        id,
        SemanticRvalueKindV1::Borrow { kind, place: from },
    )
}
fn alias(to: u32, from: u32, id: u32) -> SemanticStatementV1 {
    assign(
        to,
        id,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(from, id))),
    )
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 1; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), term),
    )
    .unwrap()
}
fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: SemanticPlaceV1,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                destination,
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
fn attributes(reference: bool, is_return: bool) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        if reference {
            SemanticAbiRegularAttributesV1::new(
                !is_return,
                (!is_return).then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                !is_return,
                false,
                true,
            )
        } else {
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true)
        },
        SemanticAbiExtensionV1::None,
        if reference && !is_return { 16 } else { 0 },
        (reference && !is_return).then_some(8),
    )
    .unwrap()
}
fn reference_abi(output: u32) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([180; 32]),
        SemanticLayoutIdentityV1::from_sha256([181; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![ty(5)],
        ty(output),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(5),
            SemanticAbiPassModeV1::Direct(attributes(true, false)),
        ))],
        SemanticAbiValueV1::new(
            ty(output),
            SemanticAbiPassModeV1::Direct(attributes(output == 6, true)),
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap()
}
fn function(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn provenance() -> SemanticKernelCapabilityProvenanceV1 {
    SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([143; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([161; 32]),
        SemanticTypeIdentityV1::from_sha256([162; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([163; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([164; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([165; 32]),
    )
    .unwrap()
}
fn borrowed_callable(reference: u32, bound: bool) -> SemanticCallableDeclV1 {
    let contract = SemanticExecutionCapabilityContractV1::new(
        E::SubgroupDeriveBorrowed {
            workgroup_reference: ty(reference),
            workgroup: ty(4),
            subgroup: ty(7),
            width: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(reference)], ty(7)).unwrap(),
        provenance(),
        SemanticTypeIdentityV1::from_sha256([166; 32]),
        SemanticTypeIdentityV1::from_sha256([167; 32]),
        None,
        SemanticFunctionIdentityV1::from_sha256([170; 32]),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            if bound {
                contract.source_identity()
            } else {
                SemanticFunctionIdentityV1::from_sha256([171; 32])
            },
            SemanticItemDefinitionIdentityV1::from_sha256([170; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([170; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([170; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([170; 32]),
            source(),
            reference_abi(7),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([170; 32]),
    }
}

fn direct_function(
    statements: Vec<SemanticStatementV1>,
    extra_argument: bool,
) -> SemanticFunctionDeclV1 {
    let mut arguments = vec![SemanticOperandV1::Copy(place(3, 5))];
    if extra_argument {
        arguments.push(SemanticOperandV1::Copy(place(2, 5)));
    }
    function(
        175,
        reference_abi(7),
        vec![
            local(0, 7, SemanticLocalRoleV1::Return),
            local(1, 4, SemanticLocalRoleV1::Temporary),
            local(2, 5, SemanticLocalRoleV1::Temporary),
            local(3, 5, SemanticLocalRoleV1::Temporary),
            local(4, 7, SemanticLocalRoleV1::Temporary),
            local(5, 5, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![
            block(0, statements, call(0, arguments, place(4, 7), 1)),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
}
fn direct_statements() -> Vec<SemanticStatementV1> {
    vec![
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        borrow(2, 5, place(1, 4), SemanticBorrowKindV1::Shared),
        alias(3, 2, 5),
    ]
}

#[test]
fn exact_borrowed_terminal_and_reference_forwarding_are_transparent() {
    let body = direct_function(direct_statements(), false);
    let expected = BTreeSet::from([SemanticTransparentBorrowSiteV1 {
        block: 0,
        statement: 1,
    }]);
    assert_eq!(direct_sites(&body, &[borrowed_callable(5, true)]), expected);
    assert_eq!(
        super::super::transparent_borrow_sites_v1(&body, &[borrowed_callable(5, true)]),
        expected
    );
}

#[test]
fn changed_binding_reference_type_and_source_arity_are_not_transparent() {
    let body = direct_function(direct_statements(), false);
    assert!(direct_sites(&body, &[borrowed_callable(5, false)]).is_empty());
    assert!(direct_sites(&body, &[borrowed_callable(8, true)]).is_empty());
    assert!(
        direct_sites(
            &direct_function(direct_statements(), true),
            &[borrowed_callable(5, true)]
        )
        .is_empty()
    );
}

#[test]
fn mutable_raw_projected_and_escaping_reference_flows_reject() {
    let callable = borrowed_callable(5, true);
    let mut statements = direct_statements();
    statements[1] = borrow(2, 5, place(1, 4), SemanticBorrowKindV1::Mutable);
    assert!(direct_sites(&direct_function(statements, false), &[callable.clone()]).is_empty());
    let mut statements = direct_statements();
    statements[1] = assign(
        2,
        5,
        SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable,
            place: place(1, 4),
        },
    );
    assert!(direct_sites(&direct_function(statements, false), &[callable.clone()]).is_empty());
    let mut statements = direct_statements();
    statements.push(statement(SemanticStatementKindV1::Assume(
        SemanticOperandV1::Copy(place(2, 5)),
    )));
    assert!(direct_sites(&direct_function(statements, false), &[callable.clone()]).is_empty());
    let mut statements = direct_statements();
    statements[2] = alias(3, 2, 5);
    statements.push(alias(3, 2, 5));
    assert!(direct_sites(&direct_function(statements, false), &[callable]).is_empty());
}

#[test]
fn shared_forks_are_closed_but_one_escape_poisoning_a_fork_rejects_all() {
    let mut statements = direct_statements();
    statements.push(alias(5, 2, 5));
    let body = direct_function(statements.clone(), false);
    assert_eq!(direct_sites(&body, &[borrowed_callable(5, true)]).len(), 1);
    statements.push(statement(SemanticStatementKindV1::Assume(
        SemanticOperandV1::Copy(place(5, 5)),
    )));
    assert!(
        direct_sites(
            &direct_function(statements, false),
            &[borrowed_callable(5, true)]
        )
        .is_empty()
    );
}

#[test]
fn bounded_classifier_never_returns_a_partial_acceptance() {
    assert!(matches!(
        sites(
            &direct_function(direct_statements(), false),
            &[borrowed_callable(5, true)],
            &[],
            0,
            None,
        ).map_err(flow_work_profile_v1::original_error_for_test),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
}

// Canonical source data for testing reference observability only. The aggregate
// construction below is deliberately NOT an issued Workgroup or machine proof.
fn epoch_source(annotated: bool) -> AdmittedInertSemanticMirV1 {
    let seed = crate::production::semantic_ssa::tests::admitted_helper_semantic();
    let original = &seed.functions()[0];
    let scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let mut types = seed.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([201; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(scalar),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    for index in [2, 3, 4] {
        let (size, align, fields, offsets, repr) = if index == 4 {
            (
                16,
                8,
                vec![ty(1), ty(1), ty(2), ty(3)],
                vec![0, 8, 16, 16],
                SemanticBackendReprV1::scalar_pair(scalar, scalar),
            )
        } else {
            (0, 1, vec![], vec![], SemanticBackendReprV1::memory(true))
        };
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([200 + index; 32]),
            SemanticLayoutIdentityV1::from_sha256([200 + index; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(size),
                align,
                repr,
                false,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        ));
    }
    for (index, pointee, size, alignment) in [(5, 4, 16, 8), (6, 2, 0, 1)] {
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([200 + index; 32]),
                SemanticLayoutIdentityV1::from_sha256([200 + index; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        ty(pointee),
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false)
                    .with_rustc_layout_is_noundef(true)
                    .with_scalar_pointee_info(
                        Some(
                            SemanticAbiPointeeInfoV1::new(
                                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                                size,
                                alignment,
                            )
                            .unwrap(),
                        ),
                        None,
                    ),
            ),
        );
    }
    assert!(
        types
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    let projection_types =
        SemanticWorkgroupEpochProjectionTypesV1::new([ty(5), ty(4), ty(6), ty(2)]);
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(4)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(2), ty(2)).unwrap(),
        ],
        ty(2),
    )
    .unwrap();
    let getter_locals = || {
        vec![
            local(0, 6, SemanticLocalRoleV1::Return),
            local(1, 5, SemanticLocalRoleV1::Argument(0)),
        ]
    };
    let getter = function(
        182,
        reference_abi(6),
        getter_locals(),
        vec![block(
            0,
            vec![borrow(0, 6, projected, SemanticBorrowKindV1::Shared)],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let getter = if annotated {
        let record = SemanticWorkgroupEpochProjectionV1::for_defined_function(
            SemanticFunctionIdV1::from_index(1),
            &getter,
            projection_types,
            provenance(),
            SemanticTypeIdentityV1::from_sha256([166; 32]),
            SemanticTypeIdentityV1::from_sha256([167; 32]),
        )
        .unwrap();
        getter.with_workgroup_epoch_projection(record).unwrap()
    } else {
        getter
    };
    let wrapper = function(
        183,
        reference_abi(6),
        getter_locals(),
        vec![
            block(
                0,
                vec![],
                call(
                    1,
                    vec![SemanticOperandV1::Copy(place(1, 5))],
                    place(0, 6),
                    1,
                ),
            ),
            block(1, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let aggregate = |operands| {
        SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, operands).unwrap()
    };
    let zero = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(1),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 8).unwrap()),
        ))
    };
    let root = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        vec![
            local(0, 0, SemanticLocalRoleV1::Return),
            local(1, 4, SemanticLocalRoleV1::Temporary),
            local(2, 5, SemanticLocalRoleV1::Temporary),
            local(3, 6, SemanticLocalRoleV1::Temporary),
            local(4, 2, SemanticLocalRoleV1::Temporary),
            local(5, 3, SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                0,
                vec![
                    assign(4, 2, aggregate(vec![])),
                    assign(5, 3, aggregate(vec![])),
                    assign(
                        1,
                        4,
                        aggregate(vec![
                            zero(),
                            zero(),
                            SemanticOperandV1::Copy(place(4, 2)),
                            SemanticOperandV1::Copy(place(5, 3)),
                        ]),
                    ),
                    borrow(2, 5, place(1, 4), SemanticBorrowKindV1::Shared),
                ],
                call(
                    2,
                    vec![SemanticOperandV1::Copy(place(2, 5))],
                    place(3, 6),
                    1,
                ),
            ),
            block(
                1,
                vec![statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(1),
                ))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new(
        seed.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, getter, wrapper],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v20(SemanticMirLimitsV1::default())
    .unwrap()
}

#[test]
fn replayed_epoch_getter_preserves_parameter_and_return_transfers_and_storage_kill() {
    let source = epoch_source(true);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let root = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    assert_eq!(bindings.len(), 1);
    let origins: Vec<_> = root
        .block_origins()
        .iter()
        .flat_map(|block| block.statements())
        .collect();
    assert_eq!(
        origins
            .iter()
            .filter(|origin| matches!(
                origin,
                SemanticExpandedStatementOriginV1::ParameterTransfer { .. }
            ))
            .count(),
        2
    );
    assert_eq!(
        origins
            .iter()
            .filter(|origin| matches!(
                origin,
                SemanticExpandedStatementOriginV1::ReturnTransfer { .. }
            ))
            .count(),
        2
    );
    let transparent = execution_sites(&source, &expansion, root, MAX_FLOW_WORK).unwrap();
    assert_eq!(transparent.len(), 2);
    assert_eq!(
        super::super::transparent_borrow_sites_for_execution_v1(
            &source,
            &expansion,
            root,
            MAX_FLOW_WORK,
        )
        .unwrap(),
        transparent,
    );
    assert!(transparent.contains(&SemanticTransparentBorrowSiteV1 {
        block: bindings[0].expanded_entry_block().index(),
        statement: 0
    }));
    let (input, _, _) = semantic_function_ssa_input_v1(
        root.body(),
        Some(source.types()),
        source.callables(),
        &transparent,
    );
    assert!(input.promotable()[1]);
    assert!(
        input
            .blocks()
            .iter()
            .flat_map(|block| block.events())
            .any(|event| *event == SsaEventV1::Kill(SsaVariableIdV1::new(1)))
    );
    let planned = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    assert!(planned.reverse_postorder().iter().any(|&block| {
        planned
            .resolved_events(block)
            .is_some_and(|events| !events.is_empty())
    }));
}

#[test]
fn unannotated_epoch_projection_and_foreign_replay_view_fail_closed() {
    let source = epoch_source(false);
    let expansion =
        SemanticCallExpansionV1::try_new(&source, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    assert!(
        execution_sites(&source, &expansion, view, MAX_FLOW_WORK)
            .unwrap()
            .is_empty()
    );
    let checked = epoch_source(true);
    let checked_expansion =
        SemanticCallExpansionV1::try_new(&checked, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    assert!(execution_sites(&source, &checked_expansion, view, MAX_FLOW_WORK).is_err());
    let checked_view = checked_expansion
        .root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    // The root is not Clone: copying its reference retains the same owner view.
    let alias_view = checked_view;
    let checked_sites =
        execution_sites(&checked, &checked_expansion, checked_view, MAX_FLOW_WORK).unwrap();
    assert_eq!(checked_sites.len(), 2);
    assert_eq!(
        execution_sites(&checked, &checked_expansion, alias_view, MAX_FLOW_WORK).unwrap(),
        checked_sites,
    );
    assert_ne!(
        expansion.source_semantic_sha256(),
        checked_expansion.source_semantic_sha256()
    );
    assert!(matches!(
        execution_sites(&checked, &checked_expansion, view, MAX_FLOW_WORK),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

#[test]
fn production_ssa_owner_replays_the_epoch_transparency_hook_without_rewriting_source() {
    use crate::production::ProductionSemanticMirLimitsV1;
    let source = epoch_source(true);
    let source_identity = *source.semantic_sha256().as_bytes();
    let getter = source.functions()[1].clone();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(owner.source_semantic_sha256(), &source_identity);
    assert_eq!(&owner.source_semantic().functions()[1], &getter);
    let root = SemanticFunctionIdV1::from_index(0);
    assert!(
        owner
            .execution_view_for_root(root)
            .unwrap()
            .has_expanded_calls()
    );
    assert!(!std::ptr::eq(
        owner.execution_plan_for_root(root).unwrap(),
        owner.plan_for_function(root).unwrap()
    ));
}
