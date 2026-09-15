use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "private_context_tests.rs"]
mod private_context;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn provenance() -> SemanticKernelCapabilityProvenanceV1 {
    SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([1; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
    )
    .unwrap()
}

// Component type/ABI fixtures only; these are not production root inputs.
fn types(kind: SemanticPointerKindV1, mutability: SemanticMutabilityV1) -> Vec<SemanticTypeDeclV1> {
    let mut types = (0..4u8)
        .map(|index| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([index + 20; 32]),
                SemanticLayoutIdentityV1::from_sha256([index + 20; 32]),
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

fn policy() -> I {
    I::ExecutionCapability {
        contract: SemanticExecutionCapabilityContractV1::new_kernel_scoped(
            E::NumericalPolicyIssue {
                context: ty(2),
                capability: ty(3),
                policy: SemanticTypeIdentityV1::from_sha256([7; 32]),
            },
            SemanticExecutionCapabilitySignatureV1::new(&[ty(2)], ty(3)).unwrap(),
            provenance(),
            SemanticFunctionIdentityV1::from_sha256([8; 32]),
        )
        .unwrap(),
    }
}

fn callable(
    tag: u8,
    inputs: Vec<SemanticTypeIdV1>,
    output: SemanticTypeIdV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    operation: I,
) -> SemanticCallableDeclV1 {
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([9; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs.clone(),
        output,
        inputs
            .iter()
            .map(|&input| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    input,
                    SemanticAbiPassModeV1::Direct(attributes),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership; inputs.len()])
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    }
}

fn policy_callable() -> SemanticCallableDeclV1 {
    callable(
        8,
        vec![ty(2)],
        ty(3),
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        policy(),
    )
}

fn place(index: u32, value_type: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], value_type).unwrap()
}

fn call(arguments: Vec<SemanticOperandV1>, output: SemanticTypeIdV1) -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        arguments,
        Some(SemanticCallDestinationV1::new(
            place(3, output),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
}

#[test]
fn policy_receiver_copy_and_move_keep_exact_owned_type_edge() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let callable = policy_callable();
    let fact = KernelContextBorrowV1::for_callable(&types, &callable).unwrap();
    assert_eq!(fact.reference_pair(), (ty(2), ty(1)));
    for operand in [
        SemanticOperandV1::Copy(place(2, ty(2))),
        SemanticOperandV1::Move(place(2, ty(2))),
    ] {
        assert!(fact.accepts(&call(vec![operand], ty(3)), 0, ty(1)));
    }
}

#[test]
fn raw_mutable_and_missing_pointee_rosters_are_not_transparent() {
    let callable = policy_callable();
    for (kind, mutability) in [
        (SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        ),
    ] {
        assert!(KernelContextBorrowV1::for_callable(&types(kind, mutability), &callable).is_none());
    }
    assert!(KernelContextBorrowV1::for_callable(&[], &callable).is_none());
}

#[test]
fn source_binding_signature_and_ownership_substitutions_reject() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    for callable in [
        callable(
            9,
            vec![ty(2)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            policy(),
        ),
        callable(
            8,
            vec![ty(2)],
            ty(1),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            policy(),
        ),
        callable(
            8,
            vec![ty(2), ty(2)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            policy(),
        ),
        callable(
            8,
            vec![ty(2)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::ByValue,
            policy(),
        ),
    ] {
        assert!(KernelContextBorrowV1::for_callable(&types, &callable).is_none());
    }
}

#[test]
fn actual_call_receiver_position_arity_output_and_owned_type_reject() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let callable = policy_callable();
    let fact = KernelContextBorrowV1::for_callable(&types, &callable).unwrap();
    let receiver = SemanticOperandV1::Copy(place(2, ty(2)));
    let exact = call(vec![receiver.clone()], ty(3));
    assert!(!fact.accepts(&exact, 1, ty(1)));
    assert!(!fact.accepts(&exact, 0, ty(3)));
    assert!(!fact.accepts(
        &call(vec![receiver.clone(), receiver.clone()], ty(3)),
        0,
        ty(1)
    ));
    assert!(!fact.accepts(&call(vec![receiver], ty(1)), 0, ty(1)));
    assert!(!fact.accepts(
        &call(vec![SemanticOperandV1::Copy(place(2, ty(1)))], ty(3)),
        0,
        ty(1)
    ));
}

#[test]
fn defined_calls_and_unrelated_intrinsics_do_not_gain_terminal_status() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    for callable in [
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        callable(
            8,
            vec![ty(2)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            I::FabsF32,
        ),
    ] {
        assert!(KernelContextBorrowV1::for_callable(&types, &callable).is_none());
    }
}

#[test]
fn global_bind_checks_original_source_signature_and_identity() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let operation = I::CapabilityGlobalBindReadOnly {
        context: ty(1),
        physical: ty(0),
        view: ty(3),
        element: ty(0),
        contract: SemanticCapabilityMemoryContractV1::global_read_only(),
        provenance: provenance(),
        source_identity: SemanticFunctionIdentityV1::from_sha256([8; 32]),
    };
    let exact = callable(
        8,
        vec![ty(2), ty(0)],
        ty(3),
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        operation,
    );
    let fact = KernelContextBorrowV1::for_callable(&types, &exact).unwrap();
    assert_eq!(fact.reference_pair(), (ty(2), ty(1)));
    assert!(fact.accepts(
        &call(
            vec![
                SemanticOperandV1::Copy(place(2, ty(2))),
                SemanticOperandV1::Copy(place(4, ty(0)))
            ],
            ty(3)
        ),
        0,
        ty(1)
    ));
    for (tag, inputs) in [(9, vec![ty(2), ty(0)]), (8, vec![ty(2)])] {
        let changed = callable(
            tag,
            inputs,
            ty(3),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            operation,
        );
        assert!(KernelContextBorrowV1::for_callable(&types, &changed).is_none());
    }
}
