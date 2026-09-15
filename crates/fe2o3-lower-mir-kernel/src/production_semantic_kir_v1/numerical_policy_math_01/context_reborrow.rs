// This classifies a transport edge, never a Context issuer. The resolver must
// still reach the source-checked root and retain its original storage loan.
fn math_reference_kind_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
    role: Role,
) -> Option<SemanticBorrowKindV1> {
    if numerical_policy_math_shared_type_v1(types, reference, pointee) {
        return Some(SemanticBorrowKindV1::Shared);
    }
    if role != Role::Context {
        return None;
    }
    let SemanticTypeShapeV1::Pointer(pointer) = types.get(reference.index() as usize)?.shape()
    else {
        return None;
    };
    (pointer.kind() == SemanticPointerKindV1::Reference
        && pointer.mutability() == SemanticMutabilityV1::Mutable
        && pointer.pointee() == pointee
        && pointer.address_space() == 0
        && pointer.pointer_width_bits() == 64
        && pointer.metadata() == SemanticPointerMetadataV1::None)
        .then_some(SemanticBorrowKindV1::Mutable)
}

fn math_reborrow_kind_matches_v1(
    source: SemanticBorrowKindV1,
    result: SemanticBorrowKindV1,
) -> bool {
    matches!(
        (source, result),
        (SemanticBorrowKindV1::Shared, SemanticBorrowKindV1::Shared)
            | (SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Shared)
            | (SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Mutable)
    )
}

#[cfg(test)]
#[path = "context_reborrow_shape_tests.rs"]
mod context_reborrow_shape_tests;
