use super::super::allocation_borrow::AllocationBorrow;
use super::*;

// Inert address-observability fixtures. These do not claim source issuance or
// initialize the owned Workgroup; the actual source-to-KIR callbacks require
// source replay and the independent SSA issuer/loan checks.
fn types(kind: SemanticPointerKindV1, mutability: SemanticMutabilityV1) -> Vec<SemanticTypeDeclV1> {
    let mut types = (0..5u8)
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
    types[4] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([24; 32]),
        SemanticLayoutIdentityV1::from_sha256([24; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::float(32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    );
    types
}

fn abi(
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

fn allocator(bound: bool, ownership: SemanticSourceArgumentOwnershipV1) -> SemanticCallableDeclV1 {
    let contract = SemanticExecutionCapabilityContractV1::new(
        E::LdsAllocate {
            workgroup: ty(2),
            lds: ty(3),
            element: ty(4),
            elements: 64,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(2)], ty(3)).unwrap(),
        provenance(),
        SemanticTypeIdentityV1::from_sha256([166; 32]),
        SemanticTypeIdentityV1::from_sha256([167; 32]),
        None,
        SemanticFunctionIdentityV1::from_sha256([170; 32]),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([if bound { 170 } else { 171 }; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([170; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([170; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([170; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([170; 32]),
            source(),
            abi(2, 3, ownership),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([170; 32]),
    }
}

fn statements(kind: SemanticBorrowKindV1) -> Vec<SemanticStatementV1> {
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

fn body(statements: Vec<SemanticStatementV1>, extra_argument: bool) -> SemanticFunctionDeclV1 {
    let mut arguments = vec![SemanticOperandV1::Move(place(4, 2))];
    if extra_argument {
        arguments.push(SemanticOperandV1::Copy(place(3, 2)));
    }
    function(
        175,
        abi(1, 0, SemanticSourceArgumentOwnershipV1::ByValue),
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
            block(0, statements, call(0, arguments, place(5, 3), 1)),
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
fn lds_allocation_shared_alias_reborrow_is_transparent_without_erasing_kill() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let callables = [allocator(
        true,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    )];
    let body = body(statements(SemanticBorrowKindV1::Shared), false);
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
    assert!(input
        .blocks()
        .iter()
        .flat_map(|block| block.events())
        .any(|event| *event == SsaEventV1::Kill(SsaVariableIdV1::new(1))));
}

#[test]
fn lds_allocation_requires_typed_reference_and_exact_source_binding() {
    let shared = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let body = body(statements(SemanticBorrowKindV1::Shared), false);
    let valid = allocator(true, SemanticSourceArgumentOwnershipV1::SharedBorrow);
    assert_eq!(
        AllocationBorrow::for_callable(&shared, &valid)
            .unwrap()
            .pair(),
        (ty(2), ty(1))
    );
    assert!(direct_sites(&body, &[valid.clone()]).is_empty());
    for invalid in [
        allocator(false, SemanticSourceArgumentOwnershipV1::SharedBorrow),
        allocator(true, SemanticSourceArgumentOwnershipV1::ByValue),
        allocator(true, SemanticSourceArgumentOwnershipV1::UniqueBorrow),
    ] {
        assert!(AllocationBorrow::for_callable(&shared, &invalid).is_none());
        assert!(typed_direct_sites(&body, &shared, &[invalid]).is_empty());
    }
    assert!(AllocationBorrow::for_callable(&shared[..2], &valid).is_none());
}

#[test]
fn lds_allocation_raw_or_mutable_reference_is_not_transparent() {
    let callables = [allocator(
        true,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    )];
    for (kind, mutability) in [
        (SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        ),
    ] {
        let types = types(kind, mutability);
        assert!(AllocationBorrow::for_callable(&types, &callables[0]).is_none());
        assert!(typed_direct_sites(
            &body(statements(SemanticBorrowKindV1::Shared), false),
            &types,
            &callables
        )
        .is_empty());
    }
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    assert!(typed_direct_sites(
        &body(statements(SemanticBorrowKindV1::Mutable), false),
        &types,
        &callables
    )
    .is_empty());
}

#[test]
fn lds_allocation_extra_argument_deref_use_or_duplicate_definition_retains_storage() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let callables = [allocator(
        true,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    )];
    assert!(typed_direct_sites(
        &body(statements(SemanticBorrowKindV1::Shared), true),
        &types,
        &callables
    )
    .is_empty());
    let mut duplicate = statements(SemanticBorrowKindV1::Shared);
    duplicate.push(alias(3, 4, 2));
    assert!(typed_direct_sites(&body(duplicate, false), &types, &callables).is_empty());
    let mut read = statements(SemanticBorrowKindV1::Shared);
    read.push(assign(
        1,
        1,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1))
                        .unwrap(),
                ],
                ty(1),
            )
            .unwrap(),
        )),
    ));
    assert!(typed_direct_sites(&body(read, false), &types, &callables).is_empty());
}

#[test]
fn lds_allocation_transparency_uses_the_existing_flow_work_ceiling() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let callables = [allocator(
        true,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    )];
    let body = body(statements(SemanticBorrowKindV1::Shared), false);
    let first_charge = callables.len() + body.locals().len();
    assert_eq!(first_charge, 8);
    for limit in 0..first_charge {
        let observed = sites(&body, &callables, &[], limit, Some(&types)).unwrap_err();
        let ProductionSemanticSsaErrorV1::BorrowFlowWork {
            stage,
            phase_work_units,
            ordered_proof_calls,
            remaining_work_units,
            requested_work_units,
            error,
        } = &observed else {
            panic!("exact profiled work-ceiling error required: {observed:?}");
        };
        assert_eq!(*stage, "facts");
        assert_eq!(*phase_work_units, [0; 11]);
        assert_eq!(*ordered_proof_calls, 0);
        assert_eq!(*remaining_work_units, limit);
        assert_eq!(*requested_work_units, first_charge);
        assert_eq!(
            **error,
            ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                required: limit + 1,
                limit,
            },
            "the diagnostic wrapper must preserve the exact original gate"
        );
    }
    assert_eq!(
        sites(&body, &callables, &[], MAX_FLOW_WORK, Some(&types))
            .unwrap()
            .len(),
        2
    );
}
