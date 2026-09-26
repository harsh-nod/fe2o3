//! Borrowed semantic dereference decision for the existing owned ordinary table.
//! No new origin producer, authenticated-edge graph, or nominal admission.
//! The strict resource context uses only the exact-function identity check.
use super::*;

struct BorrowedCheckedReferencesV1<'a> {
    origins: &'a [Option<CheckedReferenceOriginV1>],
    option_dominance: &'a SemanticOptionDominanceV1,
    enum_payload_dominance: &'a SemanticEnumPayloadDominanceV1,
}

pub(super) fn require_same_source_v1(
    actual: &SemanticFunctionDeclV1,
    candidate: &SemanticFunctionDeclV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !std::ptr::eq(actual, candidate) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "nominal recipe tables do not borrow the actual source function",
        ));
    }
    Ok(())
}

pub(super) fn origin_for_owned_v1(
    place: &SemanticPlaceV1,
    block_index: usize,
    references: &CheckedReferencesV1,
) -> Result<Option<CheckedReferenceSourceV1>, ProductionRankedProjectionErrorV1> {
    let borrowed = BorrowedCheckedReferencesV1 {
        origins: &references.origins,
        option_dominance: &references.option_dominance,
        enum_payload_dominance: &references.enum_payload_dominance,
    };
    origin_for_borrowed_v1(place, block_index, &borrowed)
}

fn origin_for_borrowed_v1(
    place: &SemanticPlaceV1,
    block_index: usize,
    references: &BorrowedCheckedReferencesV1<'_>,
) -> Result<Option<CheckedReferenceSourceV1>, ProductionRankedProjectionErrorV1> {
    let Some(origin) = checked_reference_origin_for_place(place, references.origins) else {
        return Ok(None);
    };
    if !origin.availability.is_none_or(|availability| {
        capability_availability_allows(
            references.option_dominance,
            references.enum_payload_dominance,
            availability,
            SemanticBlockIdV1::from_index(block_index as u32),
        )
    }) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a checked reference is dereferenced outside its authenticated payload region",
        ));
    }
    Ok(Some(origin.source))
}
