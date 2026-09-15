// Closed-consumer component tests only. These do not fabricate a production
// Context issuer or claim that private memory has been lowered or proved.
use super::*;

fn private_context_allocate() -> I {
    I::ExecutionCapability {
        contract: SemanticExecutionCapabilityContractV1::new_kernel_scoped(
            E::PrivateMemoryAllocate {
                context: ty(2),
                view: ty(3),
                element: ty(0),
                elements: 4,
            },
            SemanticExecutionCapabilitySignatureV1::new(&[ty(2)], ty(3)).unwrap(),
            provenance(),
            SemanticFunctionIdentityV1::from_sha256([8; 32]),
        )
        .unwrap(),
    }
}

#[test]
fn private_allocation_context_copy_and_move_are_exact_shared_consumers() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let callable = callable(
        8,
        vec![ty(2)],
        ty(3),
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        private_context_allocate(),
    );
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
fn private_allocation_context_rejects_source_abi_and_reference_substitutions() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    for (tag, inputs, output, ownership) in [
        (
            9,
            vec![ty(2)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ),
        (
            8,
            vec![ty(2)],
            ty(1),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ),
        (
            8,
            vec![ty(2), ty(2)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ),
        (
            8,
            vec![ty(2)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::ByValue,
        ),
        (
            8,
            vec![ty(1)],
            ty(3),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ),
    ] {
        let changed = callable(tag, inputs, output, ownership, private_context_allocate());
        assert!(KernelContextBorrowV1::for_callable(&types, &changed).is_none());
    }
    let callable = callable(
        8,
        vec![ty(2)],
        ty(3),
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        private_context_allocate(),
    );
    for (kind, mutability) in [
        (SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        ),
    ] {
        let changed_types = super::types(kind, mutability);
        assert!(KernelContextBorrowV1::for_callable(&changed_types, &callable).is_none());
    }
}

#[test]
fn private_allocation_context_rejects_changed_actual_operand_and_destination() {
    let types = types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
    );
    let callable = callable(
        8,
        vec![ty(2)],
        ty(3),
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        private_context_allocate(),
    );
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
        ty(1),
    ));
}
