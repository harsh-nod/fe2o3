use super::*;

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

// Inert nominal type slots exercise descriptor selection. Only the production
// owner and shared graph may provide an actual Workgroup SSA issuer value.
fn allocation_types(
    kind: SemanticPointerKindV1,
    mutable: SemanticMutabilityV1,
) -> Vec<SemanticTypeDeclV1> {
    let mut types = (0..4u8)
        .map(|tag| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
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
    types[1] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([2; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
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
                id(0),
                kind,
                mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    types
}

fn allocation_provenance(change: u8) -> SemanticKernelCapabilityProvenanceV1 {
    let tag = |slot| if change == slot { 99 } else { slot + 30 };
    SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(u32::from(change == 1)),
        SemanticKernelBindingIdentityV1::from_sha256([tag(2); 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([tag(3); 32]),
        SemanticTypeIdentityV1::from_sha256([tag(4); 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([tag(5); 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([tag(6); 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([tag(7); 32]),
    )
    .unwrap()
}

fn allocation_contract(issuer: bool, change: u8) -> SemanticExecutionCapabilityContractV1 {
    let operation = if issuer {
        SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
            context: id(2),
            workgroup: id(0),
        }
    } else {
        SemanticExecutionCapabilityOperationV1::LdsAllocate {
            workgroup: id(1),
            lds: id(3),
            element: id(0),
            elements: 64,
        }
    };
    SemanticExecutionCapabilityContractV1::new(
        operation,
        SemanticExecutionCapabilitySignatureV1::new(
            &[id(if issuer { 2 } else { 1 })],
            id(if issuer { 0 } else { 3 }),
        )
        .unwrap(),
        allocation_provenance(change),
        SemanticTypeIdentityV1::from_sha256([if change == 8 { 99 } else { 51 }; 32]),
        SemanticTypeIdentityV1::from_sha256([if change == 9 { 99 } else { 52 }; 32]),
        None,
        SemanticFunctionIdentityV1::from_sha256([if issuer { 70 } else { 71 }; 32]),
    )
    .unwrap()
}

fn allocation_callable(
    contract: SemanticExecutionCapabilityContractV1,
    bound: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> SemanticCallableDeclV1 {
    let input = contract.signature().arguments().next().unwrap();
    let output = contract.signature().output();
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([80; 32]),
        SemanticLayoutIdentityV1::from_sha256([81; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![input],
        output,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            input,
            if input == id(1) {
                SemanticAbiPassModeV1::Direct(attrs)
            } else {
                SemanticAbiPassModeV1::Ignore
            },
        ))],
        SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            if bound {
                contract.source_identity()
            } else {
                SemanticFunctionIdentityV1::from_sha256([99; 32])
            },
            SemanticItemDefinitionIdentityV1::from_sha256([82; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([83; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([84; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([85; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([86; 32]),
    }
}

#[test]
fn allocation_registers_only_the_existing_matching_workgroup_issuer_without_partition() {
    let types = allocation_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let issuer = allocation_contract(true, 0);
    let allocation = allocation_contract(false, 0);
    let callables = [
        allocation_callable(issuer, true, SemanticSourceArgumentOwnershipV1::ByValue),
        allocation_callable(
            allocation,
            true,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ),
    ];
    let mut bindings = BTreeMap::new();
    register_allocation_workgroup_issuers(&types, &callables, &mut bindings).unwrap();
    assert_eq!(bindings.len(), 1);
    assert!(matches!(bindings.get(&id(0)),
        Some(SemanticPromotedBindingV1::SubgroupPartitionAuthority { contract, source_type })
            if *contract == issuer && *source_type == execution_type_identity_v1(&types, id(0)).unwrap()));
    assert!(!bindings.contains_key(&id(1)));
    assert!(!bindings.contains_key(&id(3)));
    let context = KernelContextTypeV1::new("root", [34; 32], [35; 32], [36; 32]);
    let descriptor = bindings[&id(0)];
    let expected = descriptor
        .transport_types(&types, id(0), Some(&context))
        .unwrap();
    let live = descriptor
        .binding_from_transport(
            &types,
            id(0),
            &[ValueDef::new(ValueId(17), expected[0].clone())],
        )
        .unwrap();
    assert_eq!(
        descriptor.transport_values(&live).unwrap(),
        [(ValueId(17), expected[0].clone())]
    );
    assert!(descriptor
        .transport_values(&SemanticValueBindingV1::Unit)
        .is_err());
    assert!(descriptor.transport_types(&types, id(0), None).is_err());
}

#[test]
fn allocation_consumer_never_manufactures_a_missing_or_rebranded_issuer() {
    let types = allocation_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let allocator = allocation_callable(
        allocation_contract(false, 0),
        true,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    );
    let mut bindings = BTreeMap::new();
    register_allocation_workgroup_issuers(&types, &[allocator.clone()], &mut bindings).unwrap();
    assert!(bindings.is_empty());
    for change in 1..=9 {
        let changed = allocation_callable(
            allocation_contract(true, change),
            true,
            SemanticSourceArgumentOwnershipV1::ByValue,
        );
        register_allocation_workgroup_issuers(&types, &[allocator.clone(), changed], &mut bindings)
            .unwrap();
        assert!(
            bindings.is_empty(),
            "issuer substitution {change} was registered"
        );
    }
}

#[test]
fn allocation_transport_rejects_changed_callable_or_shared_ownership() {
    let types = allocation_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let issuer = allocation_callable(
        allocation_contract(true, 0),
        true,
        SemanticSourceArgumentOwnershipV1::ByValue,
    );
    for (bound, ownership) in [
        (false, SemanticSourceArgumentOwnershipV1::SharedBorrow),
        (true, SemanticSourceArgumentOwnershipV1::ByValue),
        (true, SemanticSourceArgumentOwnershipV1::UniqueBorrow),
    ] {
        let changed = allocation_callable(allocation_contract(false, 0), bound, ownership);
        assert!(register_allocation_workgroup_issuers(
            &types,
            &[issuer.clone(), changed],
            &mut BTreeMap::new()
        )
        .is_err());
    }
    let invalid_issuer = allocation_callable(
        allocation_contract(true, 0),
        false,
        SemanticSourceArgumentOwnershipV1::ByValue,
    );
    let allocator = allocation_callable(
        allocation_contract(false, 0),
        true,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    );
    assert!(register_allocation_workgroup_issuers(
        &types,
        &[invalid_issuer, allocator],
        &mut BTreeMap::new()
    )
    .is_err());
}

#[test]
fn allocation_pair_preserves_owned_legacy_case_and_rejects_raw_or_mutable_borrows() {
    let operation = allocation_contract(false, 0).operation();
    let types = allocation_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    assert_eq!(
        borrowed_allocation_pair(&types, operation).unwrap(),
        Some((id(1), id(0)))
    );
    assert_eq!(
        workgroup_reference_pair(&types, allocation_contract(false, 0)).unwrap(),
        (id(1), id(0))
    );
    let owned = SemanticExecutionCapabilityOperationV1::LdsAllocate {
        workgroup: id(0),
        lds: id(3),
        element: id(0),
        elements: 64,
    };
    assert_eq!(borrowed_allocation_pair(&types, owned).unwrap(), None);
    for (kind, mutable) in [
        (SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        ),
    ] {
        assert!(borrowed_allocation_pair(&allocation_types(kind, mutable), operation).is_err());
    }
    assert!(borrowed_allocation_pair(&types[..1], operation).is_err());
    assert!(workgroup_reference_pair(&types, allocation_contract(true, 0)).is_err());
}

#[test]
fn old_allocation_signature_cannot_erase_the_shared_source_abi() {
    let identity = |tag| ExecutionTypeIdentityV1::new([tag; 32]);
    let operation = ExecutionCapabilityOperationV1::LdsAllocate {
        workgroup: identity(1),
        lds: identity(2),
        element: identity(3),
        layout: ExecutionElementLayoutV1 {
            byte_size: 4,
            byte_alignment: 4,
        },
        elements: 64,
    };
    assert!(operation.signature_matches(
        ExecutionCapabilitySignatureV1::new(&[identity(1)], identity(2)).unwrap()
    ));
    assert!(!operation.signature_matches(
        ExecutionCapabilitySignatureV1::new(&[identity(4)], identity(2)).unwrap()
    ));
}
