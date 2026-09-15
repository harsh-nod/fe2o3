use super::super::context_issue::WorkgroupContextBorrowV1;
use super::*;

#[path = "ordered_context_loan_tests.rs"]
mod ordered_context_loan_tests;

// Inert classifier inputs only. The actual AMD import test owns source authentication.
fn context_types(
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
) -> Vec<SemanticTypeDeclV1> {
    let mut types = (0..4u8)
        .map(|index| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([20 + index; 32]),
                SemanticLayoutIdentityV1::from_sha256([20 + index; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            )
        })
        .collect::<Vec<_>>();
    types[2] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([22; 32]),
        SemanticLayoutIdentityV1::from_sha256([22; 32]),
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
                ty(1),
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    types
}

fn context_abi(
    input: u32,
    output: u32,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([180; 32]),
        SemanticLayoutIdentityV1::from_sha256([181; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![ty(input)],
        ty(output),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(input),
            if input == 2 {
                SemanticAbiPassModeV1::Direct(attributes(true, false))
            } else {
                SemanticAbiPassModeV1::Ignore
            },
        ))],
        SemanticAbiValueV1::new(ty(output), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap()
}

fn context_callable(
    tag: u8,
    ownership: SemanticSourceArgumentOwnershipV1,
    reference: u32,
) -> SemanticCallableDeclV1 {
    let contract = SemanticExecutionCapabilityContractV1::new(
        E::WorkgroupDerive {
            context: ty(reference),
            workgroup: ty(3),
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(reference)], ty(3)).unwrap(),
        provenance(),
        SemanticTypeIdentityV1::from_sha256([166; 32]),
        SemanticTypeIdentityV1::from_sha256([167; 32]),
        None,
        SemanticFunctionIdentityV1::from_sha256([170; 32]),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([170; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([170; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([170; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([170; 32]),
            source(),
            context_abi(2, 3, ownership),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([170; 32]),
    }
}

fn context_statements(kind: SemanticBorrowKindV1) -> Vec<SemanticStatementV1> {
    vec![
        borrow(2, 2, place(1, 1), kind),
        assign(
            3,
            2,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, 2))),
        ),
        borrow(
            4,
            2,
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1))
                        .unwrap(),
                ],
                ty(1),
            )
            .unwrap(),
            kind,
        ),
    ]
}

fn context_function(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    function(
        175,
        context_abi(1, 0, SemanticSourceArgumentOwnershipV1::ByValue),
        vec![
            local(0, 0, SemanticLocalRoleV1::Return),
            local(1, 1, SemanticLocalRoleV1::Argument(0)),
            local(2, 2, SemanticLocalRoleV1::Temporary),
            local(3, 2, SemanticLocalRoleV1::Temporary),
            local(4, 2, SemanticLocalRoleV1::Temporary),
            local(5, 3, SemanticLocalRoleV1::Temporary),
            local(6, 2, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                0,
                statements,
                call(
                    0,
                    vec![SemanticOperandV1::Move(place(4, 2))],
                    place(5, 3),
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
}

#[test]
fn mutable_context_issuance_keeps_exact_forwarding_reborrow_and_storage_kill() {
    let types = context_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
    );
    let callables = [context_callable(
        170,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        2,
    )];
    let body = context_function(context_statements(SemanticBorrowKindV1::Mutable));
    let sites = typed_direct_sites(&body, &types, &callables);
    assert_eq!(
        sites,
        BTreeSet::from([
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 0
            },
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: 2
            },
        ])
    );
    assert_eq!(
        super::super::super::typed_transparent_borrow_sites_v1(&body, &types, &callables),
        sites
    );
    let (input, _, _) = semantic_function_ssa_input_v1(&body, Some(&types), &callables, &sites);
    assert!(input.promotable()[1]);
    assert!(
        input
            .blocks()
            .iter()
            .flat_map(|block| block.events())
            .any(|event| *event == SsaEventV1::Kill(SsaVariableIdV1::new(1)))
    );
    plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
}

#[test]
fn mutable_context_admission_requires_exact_terminal_binding_reference_and_unique_abi() {
    let types = context_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
    );
    let good = context_callable(170, SemanticSourceArgumentOwnershipV1::UniqueBorrow, 2);
    assert!(WorkgroupContextBorrowV1::for_callable(&types, &good).is_some());
    for callable in [
        context_callable(171, SemanticSourceArgumentOwnershipV1::UniqueBorrow, 2),
        context_callable(170, SemanticSourceArgumentOwnershipV1::SharedBorrow, 2),
        context_callable(170, SemanticSourceArgumentOwnershipV1::ByValue, 2),
        context_callable(170, SemanticSourceArgumentOwnershipV1::UniqueBorrow, 1),
        borrowed_callable(5, true),
    ] {
        assert!(WorkgroupContextBorrowV1::for_callable(&types, &callable).is_none());
    }
    for (kind, mutability) in [
        (SemanticPointerKindV1::Raw, SemanticMutabilityV1::Mutable),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        ),
    ] {
        assert!(
            WorkgroupContextBorrowV1::for_callable(&context_types(kind, mutability), &good)
                .is_none()
        );
    }
    assert!(WorkgroupContextBorrowV1::for_callable(&[], &good).is_none());
    let SemanticTerminatorKindV1::Call(call) = call(
        0,
        vec![SemanticOperandV1::Move(place(4, 2))],
        place(5, 3),
        1,
    ) else {
        unreachable!()
    };
    let fact = WorkgroupContextBorrowV1::for_callable(&types, &good).unwrap();
    assert!(fact.accepts(&call, 0, ty(1)));
    assert!(!fact.accepts(&call, 1, ty(1)));
    assert!(!fact.accepts(&call, 0, ty(3)));
}

#[test]
fn mutable_context_chain_rejects_shared_fake_fork_escape_and_redefinition() {
    let types = context_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
    );
    let callables = [context_callable(
        170,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        2,
    )];
    for kind in [SemanticBorrowKindV1::Shared, SemanticBorrowKindV1::Fake] {
        assert!(
            typed_direct_sites(
                &context_function(context_statements(kind)),
                &types,
                &callables
            )
            .is_empty()
        );
    }
    for extra in [
        alias(6, 2, 2),
        alias(3, 2, 2),
        assign(
            6,
            2,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(2, 2),
            },
        ),
    ] {
        let mut statements = context_statements(SemanticBorrowKindV1::Mutable);
        statements.push(extra);
        assert!(typed_direct_sites(&context_function(statements), &types, &callables).is_empty());
    }
    assert!(
        typed_direct_sites(
            &context_function(context_statements(SemanticBorrowKindV1::Mutable)),
            &types,
            &[]
        )
        .is_empty()
    );
}

#[test]
fn mutable_context_transparency_never_revives_dead_owner_storage() {
    let types = context_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
    );
    let callables = [context_callable(
        170,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        2,
    )];
    let mut statements = context_statements(SemanticBorrowKindV1::Mutable);
    statements.insert(
        0,
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
    );
    let body = context_function(statements);
    let sites = typed_direct_sites(&body, &types, &callables);
    assert_eq!(
        sites.len(),
        2,
        "address transparency does not itself establish liveness"
    );
    let (input, _, _) = semantic_function_ssa_input_v1(&body, Some(&types), &callables, &sites);
    assert!(plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).is_err());
}
