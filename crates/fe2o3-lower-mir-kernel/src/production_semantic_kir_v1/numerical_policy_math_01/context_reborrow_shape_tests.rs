use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[test]
fn only_context_transport_can_retain_an_exact_exclusive_reference() {
    let pointee = SemanticTypeIdV1::from_index(0);
    let reference = SemanticTypeIdV1::from_index(1);
    for (kind, mutability, target, address_space, width, accepted) in [
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            pointee,
            0,
            64,
            true,
        ),
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            pointee,
            0,
            64,
            false,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            reference,
            0,
            64,
            false,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            pointee,
            1,
            64,
            false,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            pointee,
            0,
            32,
            false,
        ),
    ] {
        let declaration = |tag, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                shape,
            )
        };
        // Type-edge component test, not an admitted source or origin receipt.
        let types = vec![
            declaration(1, SemanticTypeShapeV1::Unit),
            declaration(
                2,
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        target,
                        kind,
                        mutability,
                        address_space,
                        width,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
        ];
        assert_eq!(
            math_reference_kind_v1(&types, reference, pointee, Role::Context),
            accepted.then_some(SemanticBorrowKindV1::Mutable)
        );
        for role in [Role::Math, Role::Policy, Role::Bound] {
            assert_eq!(
                math_reference_kind_v1(&types, reference, pointee, role),
                None
            );
        }
        assert!(!numerical_policy_math_shared_type_v1(
            &types, reference, pointee
        ));
        assert_eq!(
            math_reference_kind_v1(&types, pointee, pointee, Role::Context),
            None
        );
        assert_eq!(
            math_reference_kind_v1(
                &types,
                SemanticTypeIdV1::from_index(99),
                pointee,
                Role::Context
            ),
            None
        );
    }
}

#[test]
fn context_reborrow_direction_never_strengthens_a_shared_loan() {
    use SemanticBorrowKindV1::{Mutable, Shared};
    for (source, result, accepted) in [
        (Shared, Shared, true),
        (Mutable, Shared, true),
        (Mutable, Mutable, true),
        (Shared, Mutable, false),
    ] {
        assert_eq!(math_reborrow_kind_matches_v1(source, result), accepted);
    }
}
